const puppeteer = require("puppeteer-extra");
const StealthPlugin = require("puppeteer-extra-plugin-stealth");
const fs = require("fs");
const path = require("path");

puppeteer.use(StealthPlugin());

const ENGINE_ROOT = "/home/panjikali/Projects/vector_engine";
const INPUT = path.join(ENGINE_ROOT, "input");
const OUTPUT = path.join(ENGINE_ROOT, "vectorizer_svg");
const PROFILE = "/home/panjikali/snap/chromium/common/chromium";
const MANIFEST_PATH = path.join(ENGINE_ROOT, "manifest.json");
const PROCESSED_DIR = path.join(ENGINE_ROOT, "input_processed");
const FAILED_DIR = path.join(ENGINE_ROOT, "input_failed");
const SITE = "https://vectorizer.ai";

// Ensure directories exist
if (!fs.existsSync(INPUT)) fs.mkdirSync(INPUT, { recursive: true });
if (!fs.existsSync(OUTPUT)) fs.mkdirSync(OUTPUT, { recursive: true });
if (!fs.existsSync(PROFILE)) fs.mkdirSync(PROFILE, { recursive: true });
if (!fs.existsSync(PROCESSED_DIR)) fs.mkdirSync(PROCESSED_DIR, { recursive: true });
if (!fs.existsSync(FAILED_DIR)) fs.mkdirSync(FAILED_DIR, { recursive: true });

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

async function processFile(page, filePath) {
    const fileName = path.basename(filePath);
    const fileBaseName = path.parse(fileName).name;
    console.log(`\nProcessing: ${fileName}`);

    await page.goto(SITE, { waitUntil: "networkidle2", timeout: 0 });
    await wait(4000);

    if (await checkForCaptcha(page)) {
        const captchaPath = path.join(ENGINE_ROOT, "server", "static", "captcha_screenshot.png");
        await page.screenshot({ path: captchaPath, fullPage: false });
        console.error(`[CAPTCHA_DETECTED] 🤖 Terdeteksi CAPTCHA/Cloudflare di halaman awal! Screenshot tersimpan.`);
        throw new Error("CAPTCHA_DETECTED: Terdeteksi CAPTCHA di Vectorizer.ai");
    }

    const input = await page.waitForSelector('input[type="file"]', { timeout: 60000 });
    await input.uploadFile(filePath);
    console.log("Uploading...");

    await page.waitForFunction(() => location.pathname.includes("/edit"), { timeout: 0 });
    console.log("Vectorization complete.");
    
    // EKSTRA SABAR: Jeda 6 detik sebelum mengeklik apa pun agar halaman stabil
    await wait(6000);

    if (await checkForCaptcha(page)) {
        const captchaPath = path.join(ENGINE_ROOT, "server", "static", "captcha_screenshot.png");
        await page.screenshot({ path: captchaPath, fullPage: false });
        console.error(`[CAPTCHA_DETECTED] 🤖 Terdeteksi CAPTCHA saat konversi! Screenshot tersimpan.`);
        throw new Error("CAPTCHA_DETECTED: Terdeteksi CAPTCHA di Vectorizer.ai");
    }

    console.log("Opening download panel...");
    let panelOpened = await page.evaluate(() => {
        const btn = document.querySelector('#App-DownloadLink');
        if (btn) {
            btn.click();
            return true;
        }
        return false;
    });

    if (!panelOpened) {
        console.log("   [!] ID tidak ditemukan, menggunakan klik native cadangan (Download/Unduh)...");
        panelOpened = await nativeClickText(page, "Unduh") || await nativeClickText(page, "Download");
    }
    
    // Waktu tunggu agar panel/halaman download sempurna terbuka
    await wait(6000); 

    console.log("Downloading SVG...");
    // 🎯 KLIK FINAL DOWNLOAD: Utamakan ikon gambar paling akhir di panel pop-up
    let downloadClicked = await page.evaluate(() => {
        const icons = document.querySelectorAll('img[alt="download icon"], img[alt*="download"], img[alt*="unduh"]');
        if (icons.length > 0) {
            icons[icons.length - 1].click();
            return true;
        }
        return false;
    });

    if (!downloadClicked) {
        console.log("   [!] Ikon tidak ditemukan, menggunakan klik teks cadangan (Unduh/Download)...");
        downloadClicked = await nativeClickText(page, "Unduh") || await nativeClickText(page, "Download");
    }

    const before = new Set(fs.readdirSync(OUTPUT));
    let downloaded = false;
    let downloadedFile = null;

    for (let i = 0; i < 15; i++) {
        await wait(2000);
        const after = fs.readdirSync(OUTPUT);
        const newFiles = after.filter(f => !before.has(f) && !f.endsWith(".crdownload"));
        if (newFiles.length > 0) {
            downloaded = true;
            downloadedFile = newFiles[0];
            break;
        }
        if (i % 2 === 0) {
            console.log(`   [...] Menunggu file SVG terunduh... (${(i + 1) * 2}s)`);
        }
    }

    if (!downloaded) {
        const captchaPath = path.join(ENGINE_ROOT, "server", "static", "captcha_screenshot.png");
        try {
            await page.screenshot({ path: captchaPath, fullPage: false });
        } catch (sErr) {}

        const isRealCaptcha = await checkForCaptcha(page);
        if (isRealCaptcha) {
            console.error(`[CAPTCHA_DETECTED] 🤖 Terdeteksi CAPTCHA / Cloudflare di Vectorizer.ai! Screenshot tersimpan.`);
            throw new Error("CAPTCHA_DETECTED: Terdeteksi CAPTCHA di Vectorizer.ai");
        } else {
            console.error(`[DOWNLOAD_TIMEOUT] Kendala unduh berkas (Bukan CAPTCHA). Screenshot tersimpan.`);
            throw new Error("DOWNLOAD_TIMEOUT: Gagal mengunduh SVG dari Vectorizer.ai");
        }
    }

    // ENSURE SVG FILE IS RENAMED TO MATCH INPUT FILE EXACTLY
    const targetSvgName = `${fileBaseName}.svg`;
    const actualSvgPath = path.join(OUTPUT, downloadedFile);
    const targetSvgPath = path.join(OUTPUT, targetSvgName);

    if (downloadedFile !== targetSvgName && fs.existsSync(actualSvgPath)) {
        if (fs.existsSync(targetSvgPath)) fs.unlinkSync(targetSvgPath);
        fs.renameSync(actualSvgPath, targetSvgPath);
    }
    console.log(`Downloaded: ${targetSvgName}`);

    // JIKA MODE ABSENSI AKTIF: ARSIPKAN FILE KE FOLDER PROCESSED
    if (!disableManifest) {
        const archivePath = path.join(PROCESSED_DIR, fileName);
        if (fs.existsSync(archivePath)) fs.unlinkSync(archivePath);
        if (fs.existsSync(filePath)) fs.renameSync(filePath, archivePath);
        console.log(`Archived to input_processed: ${fileName}`);
        updateManifestRecord(fileName, "success");
    } else {
        console.log(`[MODE UJI COBA] File ${fileName} tetap tersimpan di folder input/ (tidak dipindahkan).`);
    }

    // 🎯 JEDA 12 DETIK SEBELUM MEMPROSES GAMBAR BENIKUTNYA
    console.log(`[JEDA SABAR 12s] ⏳ Menunggu 12 detik sebelum memasukkan gambar berikutnya ke Vectorizer...`);
    await wait(12000);
}

(async () => {
    // Clear stale lock files / symlinks if left over from previous process
    ["SingletonLock", "SingletonSocket", "SingletonCookie"].forEach(f => {
        const p = path.join(PROFILE, f);
        try { fs.rmSync(p, { force: true, recursive: true }); } catch (e) {}
    });

    const browser = await puppeteer.launch({
        headless: "new",
        executablePath: "/snap/bin/chromium",
        userDataDir: PROFILE,
        defaultViewport: null,
        ignoreDefaultArgs: ["--enable-automation"],
        args: [
            "--no-sandbox",
            "--disable-setuid-sandbox",
            "--disable-dev-shm-usage",
            "--disable-blink-features=AutomationControlled",
            "--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
            "--window-size=1920,1080"
        ]
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

    if (disableManifest) {
        console.log(`[MODE UJI COBA / TRIAL] 🟡 Absensi Batch STATUS: OFF (Memproses ${files.length} file gambar secara berurutan tanpa skip)`);
    } else {
        console.log(`[ABSENSI BATCH] 🟢 Absensi Batch STATUS: ON (Memproses ${files.length} file gambar secara berurutan dalam 1 Tab)`);
    }

    // OPEN ONE SINGLE REUSED BROWSER TAB FOR THE ENTIRE BATCH
    const page = await browser.newPage();
    await setDownload(page, OUTPUT);

    for (const file of files) {
        const fileBaseName = path.parse(file).name;
        const targetSvgName = `${fileBaseName}.svg`;
        const svgInOutput = path.join(OUTPUT, targetSvgName);
        const svgInInput = path.join(INPUT, targetSvgName);

        // CEK ABSEN SVG: Jika file SVG sudah pernah terunduh (di folder OUTPUT atau INPUT), SKIP!
        if (fs.existsSync(svgInOutput) || fs.existsSync(svgInInput)) {
            console.log(`[SKIP ABSEN SVG] File ${targetSvgName} sudah ada. Melewati ${file}...`);
            if (!disableManifest) {
                const src = path.join(INPUT, file);
                const dst = path.join(PROCESSED_DIR, file);
                if (fs.existsSync(dst)) fs.unlinkSync(dst);
                if (fs.existsSync(src)) fs.renameSync(src, dst);
                updateManifestRecord(file, "success");
            }
            continue;
        }

        // Cek jika sudah pernah dicatat sukses di manifest (HANYA JIKA MODE ABSENSI AKTIF)
        if (!disableManifest && manifest.files[file] && manifest.files[file].status === "success") {
            console.log(`[SKIP ABSEN] File ${file} sudah pernah selesai sebelumnya. Memindahkan ke input_processed...`);
            const src = path.join(INPUT, file);
            const dst = path.join(PROCESSED_DIR, file);
            if (fs.existsSync(dst)) fs.unlinkSync(dst);
            if (fs.existsSync(src)) fs.renameSync(src, dst);
            continue;
        }

        const filePath = path.join(INPUT, file);
        try {
            await processFile(page, filePath);
        } catch (err) {
            console.error(`Failed: ${file} | ${err.message}`);
            updateManifestRecord(file, "failed", err.message);
            
            // KARANTINA FILE GAGAL (HANYA JIKA MODE ABSENSI AKTIF)
            if (!disableManifest) {
                try {
                    const failDst = path.join(FAILED_DIR, file);
                    if (fs.existsSync(failDst)) fs.unlinkSync(failDst);
                    if (fs.existsSync(filePath)) fs.renameSync(filePath, failDst);
                    console.log(`Quarantined to input_failed: ${file}`);
                } catch (moveErr) {
                    console.error(`Error quarantining ${file}:`, moveErr);
                }
            } else {
                console.log(`[MODE UJI COBA] File gagal ${file} tetap tersimpan di folder input/.`);
            }
        }
    }
    
    await page.close().catch(() => {});
    await browser.close();
})();