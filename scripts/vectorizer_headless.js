const puppeteer = require("puppeteer-extra");
const StealthPlugin = require("puppeteer-extra-plugin-stealth");
puppeteer.use(StealthPlugin());
const fs = require("fs");
const path = require("path");

if (!process.env.DISPLAY) {
    process.env.DISPLAY = ":0.0";
}

const ENGINE_ROOT = "/home/panjikali/Projects/vector_engine";
const INPUT = path.join(ENGINE_ROOT, "input");
const STOCK_READY = path.join(ENGINE_ROOT, "stock_ready");
const PROFILE = "/home/panjikali/snap/chromium/common/chromium";
const MANIFEST_PATH = path.join(ENGINE_ROOT, "manifest.json");
const SITE = "https://vectorizer.ai";

// Ensure directories exist
if (!fs.existsSync(INPUT)) fs.mkdirSync(INPUT, { recursive: true });
if (!fs.existsSync(STOCK_READY)) fs.mkdirSync(STOCK_READY, { recursive: true });
if (!fs.existsSync(PROFILE)) fs.mkdirSync(PROFILE, { recursive: true });

const wait = ms => new Promise(r => setTimeout(r, ms));

function loadManifest() {
    if (fs.existsSync(MANIFEST_PATH)) {
        try {
            return JSON.parse(fs.readFileSync(MANIFEST_PATH, "utf-8"));
        } catch (e) {
            console.error("Error reading manifest.json:", e);
        }
    }
    return { updated_at: new Date().toISOString(), files: {} };
}

function updateManifestRecord(filename, status, errorMsg = null) {
    const manifest = loadManifest();
    manifest.updated_at = new Date().toISOString();
    if (!manifest.files[filename]) {
        manifest.files[filename] = { added_at: new Date().toISOString() };
    }
    manifest.files[filename].status = status;
    manifest.files[filename].station_2 = status;
    manifest.files[filename].updated_at = new Date().toISOString();
    if (errorMsg) {
        manifest.files[filename].error = errorMsg;
    }
    fs.writeFileSync(MANIFEST_PATH, JSON.stringify(manifest, null, 2), "utf-8");
}

async function setDownload(page, dir) {
    const client = await page.target().createCDPSession();
    await client.send("Page.setDownloadBehavior", {
        behavior: "allow",
        downloadPath: dir
    });
}

// FUNGSI ASLIMU DENGAN TAMBAHAN "PANTANG MENYERAH" (Dipertahankan 100%)
async function clickText(page, text) {
    const elements = await page.$$('button, a, div, span');
    for (const el of elements) {
        const value = await page.evaluate(e => e.innerText || "", el);
        if (value.trim().toLowerCase() === text.toLowerCase()) {
            try {
                await el.click();
                return true; 
            } catch (e) {
                console.log(`   [!] Tombol '${text}' tertutup layer, mencoba tombol berikutnya...`);
                continue;
            }
        }
    }
    return false;
}

async function checkForCaptcha(page) {
    try {
        return await page.evaluate(() => {
            const bodyText = document.body ? document.body.innerText.toLowerCase() : "";
            const hasCloudflare = !!document.querySelector('iframe[src*="challenges.cloudflare.com"]');
            const hasCaptchaFrame = !!document.querySelector('iframe[src*="recaptcha"], iframe[src*="hcaptcha"]');
            const hasTextChallenge = bodyText.includes("verify you are human") || 
                                     (bodyText.includes("cloudflare") && bodyText.includes("security")) ||
                                     bodyText.includes("too many requests") ||
                                     bodyText.includes("please complete the captcha");
            return hasCloudflare || hasCaptchaFrame || hasTextChallenge;
        });
    } catch (e) {
        return false;
    }
}

const disableManifest = process.env.DISABLE_MANIFEST === "true" || process.argv.includes("--no-manifest");

async function nativeClickText(page, text) {
    const handles = await page.$$('button, a, div[role="button"]');
    for (const h of handles) {
        const val = await page.evaluate(el => el.innerText || "", h);
        if (val.trim().toLowerCase() === text.toLowerCase()) {
            try {
                await h.click();
                return true;
            } catch (e) {
                continue;
            }
        }
    }
    return false;
}

async function processFile(page, filePath, isFirstFile = false, fileIndex = 0, totalFiles = 0) {
    const fileName = path.basename(filePath);
    const fileBaseName = path.parse(fileName).name;
    const countStr = totalFiles > 0 ? `[${fileIndex + 1}/${totalFiles}] ` : "";
    console.log(`\n📦 ${countStr}Memproses: ${fileName}`);

    const oldUrl = page.url();

    if (isFirstFile || !page.url().includes("vectorizer.ai")) {
        console.log("   🌐 Membuka situs Vectorizer.ai...");
        await page.goto(SITE, { waitUntil: "networkidle2", timeout: 0 });
        await wait(6000);
    } else {
        console.log("   📜 Menggulir ke area Drop Zone...");
        await page.evaluate(() => {
            const dropZone = document.querySelector('#app-starter-drop-zone') || document.querySelector('.drop_target');
            if (dropZone) dropZone.scrollIntoView({ behavior: 'smooth', block: 'center' });
            else window.scrollTo(0, document.body.scrollHeight || 600);
        }).catch(() => {});
        await wait(8500);
    }

    if (await checkForCaptcha(page)) {
        const captchaPath = path.join(ENGINE_ROOT, "server", "static", "captcha_screenshot.png");
        await page.screenshot({ path: captchaPath, fullPage: false });
        console.error(`🤖 CAPTCHA Terdeteksi! Screenshot tersimpan.`);
        throw new Error("CAPTCHA_DETECTED: Terdeteksi CAPTCHA di Vectorizer.ai");
    }

    let input = await page.$('#app-starter-drop-zone input[type="file"]');
    if (!input) input = await page.$('input[type="file"]');
    if (!input) {
        input = await page.waitForSelector('input[type="file"]', { timeout: 60000 });
    }
    await input.uploadFile(filePath);
    console.log("   📤 Mengunggah berkas JPG ke server...");
    await wait(7000);

    if (!isFirstFile && oldUrl.includes("vectorizer.ai")) {
        console.log("   ⏳ Menunggu server membuat halaman gambar baru...");
        await page.waitForFunction(prev => location.href !== prev, { timeout: 60000 }, oldUrl).catch(() => {});
    }

    console.log("   ⚡ AI Vectorizer sedang memproses gambar...");
    try {
        await page.waitForFunction(() => location.pathname.includes("/edit"), { timeout: 90000 });
    } catch (wErr) {
        const captchaPath = path.join(ENGINE_ROOT, "server", "static", "captcha_screenshot.png");
        try { await page.screenshot({ path: captchaPath, fullPage: false }); } catch (sErr) {}
        const isCaptcha = await checkForCaptcha(page);
        if (isCaptcha) {
            console.error(`🤖 CAPTCHA / Cloudflare Terdeteksi di Vectorizer.ai!`);
            throw new Error("CAPTCHA_DETECTED: Terdeteksi CAPTCHA di Vectorizer.ai");
        } else {
            console.error(`❌ Timeout (90s) menunggu pemrosesan gambar di Vectorizer.ai.`);
            throw new Error("VECTORIZE_TIMEOUT: Timeout 90 detik saat mengolah gambar di Vectorizer.ai");
        }
    }
    console.log("   ✨ Vektorisasi AI Selesai!");
    
    // EKSTRA SABAR: Jeda 13 detik sebelum mengeklik apa pun agar halaman stabil
    console.log("   ⏳ Menstabilkan halaman canvas (13s)...");
    await wait(13000);

    if (await checkForCaptcha(page)) {
        const captchaPath = path.join(ENGINE_ROOT, "server", "static", "captcha_screenshot.png");
        await page.screenshot({ path: captchaPath, fullPage: false });
        console.error(`🤖 CAPTCHA Terdeteksi saat konversi!`);
        throw new Error("CAPTCHA_DETECTED: Terdeteksi CAPTCHA di Vectorizer.ai");
    }

    console.log("   🎨 Membuka panel unduh...");
    let panelOpened = await page.evaluate(() => {
        const btn = document.querySelector('#App-DownloadLink');
        if (btn) {
            btn.click();
            return true;
        }
        return false;
    });

    if (!panelOpened) {
        panelOpened = await nativeClickText(page, "Unduh") || await nativeClickText(page, "Download");
    }
    
    console.log("   ⏳ Memuat panel unduhan (13s)...");
    await wait(13000); 

    const before = new Set(fs.readdirSync(INPUT));

    console.log("   ⬇️ Mengklik tombol unduh SVG...");
    let downloadClicked = await page.evaluate(() => {
        const btn = document.querySelector('#App-DownloadButton');
        if (btn) {
            btn.click();
            return true;
        }
        const buttons = Array.from(document.querySelectorAll('button, a, div[role="button"]'));
        const dlBtn = buttons.find(b => b.innerText && (b.innerText.toLowerCase().includes('download') || b.innerText.toLowerCase().includes('unduh')));
        if (dlBtn) {
            dlBtn.click();
            return true;
        }
        const icons = document.querySelectorAll('img[alt="download icon"], img[alt*="download"], img[alt*="unduh"]');
        if (icons.length > 0) {
            icons[icons.length - 1].click();
            return true;
        }
        return false;
    });

    if (!downloadClicked) {
        downloadClicked = await nativeClickText(page, "Unduh") || await nativeClickText(page, "Download");
    }

    let downloaded = false;
    let downloadedFile = null;

    for (let i = 0; i < 20; i++) {
        await wait(2000);
        const after = fs.readdirSync(INPUT);
        const newFiles = after.filter(f => !before.has(f) && !f.endsWith(".crdownload") && f.endsWith(".svg"));
        if (newFiles.length > 0) {
            downloaded = true;
            downloadedFile = newFiles[0];
            break;
        }
        if (i % 2 === 0) {
            console.log(`   ⏳ Menunggu unduhan berkas SVG... (${(i + 1) * 2}s)`);
        }
    }

    if (!downloaded) {
        const captchaPath = path.join(ENGINE_ROOT, "server", "static", "captcha_screenshot.png");
        try {
            await page.screenshot({ path: captchaPath, fullPage: false });
        } catch (sErr) {}

        const isRealCaptcha = await checkForCaptcha(page);
        if (isRealCaptcha) {
            console.error(`🤖 CAPTCHA Terdeteksi di Vectorizer.ai!`);
            throw new Error("CAPTCHA_DETECTED: Terdeteksi CAPTCHA di Vectorizer.ai");
        } else {
            console.error(`❌ Gagal mengunduh berkas SVG.`);
            throw new Error("DOWNLOAD_TIMEOUT: Gagal mengunduh SVG dari Vectorizer.ai");
        }
    }

    const targetSvgName = `${fileBaseName}.svg`;
    const actualSvgPath = path.join(INPUT, downloadedFile);
    const targetSvgPath = path.join(INPUT, targetSvgName);

    if (downloadedFile !== targetSvgName && fs.existsSync(actualSvgPath)) {
        if (fs.existsSync(targetSvgPath)) fs.unlinkSync(targetSvgPath);
        fs.renameSync(actualSvgPath, targetSvgPath);
    }
    console.log(`✅ BERHASIL: Berkas ${targetSvgName} tersimpan di input/`);

    if (fs.existsSync(filePath) && fs.existsSync(targetSvgPath)) {
        try { fs.unlinkSync(filePath); } catch (e) {}
    }
    updateManifestRecord(fileName, "success");

    console.log(`⏳ Istirahat 19s sebelum gambar berikutnya (Human Pacing)...`);
    await wait(19000);
}

(async () => {
    const { execSync } = require("child_process");
    try {
        execSync("pkill -9 -f chromium || true");
    } catch (e) {}

    // Clear stale lock files / symlinks if left over from previous process
    ["SingletonLock", "SingletonSocket", "SingletonCookie"].forEach(f => {
        const p = path.join(PROFILE, f);
        try { fs.rmSync(p, { force: true, recursive: true }); } catch (e) {}
    });

    const execPath = fs.existsSync("/usr/bin/chromium-browser") ? "/usr/bin/chromium-browser" : "/snap/bin/chromium";
    const browser = await puppeteer.launch({
        headless: "new",
        executablePath: execPath,
        userDataDir: PROFILE,
        defaultViewport: null,
        args: ["--no-sandbox", "--disable-setuid-sandbox", "--disable-dev-shm-usage", "--window-size=1920,1080"]
    });

    const manifest = loadManifest();
    let files = fs.readdirSync(INPUT).filter(f => /\.(png|jpg|jpeg|webp)$/i.test(f));
    
    if (files.length === 0) {
        console.log("No pending files in vectorizer input queue.");
        await browser.close();
        return;
    }

    // STRICT NATURAL NUMERIC SORT (0001, 0002, 0003... 0046)
    files.sort((a, b) => a.localeCompare(b, undefined, { numeric: true, sensitivity: 'base' }));

    console.log(`🟢 Absensi Batch STATUS: ON (Memproses ${files.length} file gambar ke folder input/)`);

    // OPEN ONE SINGLE REUSED BROWSER TAB FOR THE ENTIRE BATCH
    const page = await browser.newPage();
    await page.setUserAgent('Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36');
    await setDownload(page, INPUT);

function findFileInTree(dir, targetName) {
    if (!fs.existsSync(dir)) return false;
    const entries = fs.readdirSync(dir, { withFileTypes: true });
    for (const entry of entries) {
        if (entry.isDirectory()) {
            if (findFileInTree(path.join(dir, entry.name), targetName)) return true;
        } else if (entry.name.toLowerCase() === targetName.toLowerCase()) {
            return true;
        }
    }
    return false;
}

    let isFirst = true;
    for (const file of files) {
        const fileBaseName = path.parse(file).name;
        const targetSvgName = `${fileBaseName}.svg`;
        const svgInInput = path.join(INPUT, targetSvgName);
        const existsInStock = findFileInTree(STOCK_READY, targetSvgName);

        // CEK ABSEN SVG: Jika file SVG sudah pernah terunduh (di folder input atau stock_ready), SKIP!
        if (fs.existsSync(svgInInput) || existsInStock) {
            console.log(`[SKIP ABSEN SVG] File ${targetSvgName} sudah ada. Melewati ${file}...`);
            const src = path.join(INPUT, file);
            if (fs.existsSync(src)) {
                try { fs.unlinkSync(src); } catch (e) {}
            }
            updateManifestRecord(file, "success");
            continue;
        }

        const filePath = path.join(INPUT, file);
        try {
            await processFile(page, filePath, isFirst, files.indexOf(file), files.length);
            isFirst = false;
        } catch (err) {
            console.error(`Failed: ${file} | ${err.message}`);
            updateManifestRecord(file, "failed", err.message);
            isFirst = true;
        }
    }
    
    await page.close().catch(() => {});
    await browser.close();
})();