const puppeteer = require("puppeteer-extra");
const StealthPlugin = require("puppeteer-extra-plugin-stealth");
const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");

puppeteer.use(StealthPlugin());

// ==========================================
// ⚙️ KONFIGURASI VECTOR ENGINE - STASIUN 4
// ==========================================
const ENGINE_ROOT = "/home/panjikali/Projects/vector_engine";
const READY_DIR = path.join(ENGINE_ROOT, "output");
const PROFILE = "/home/panjikali/snap/chromium/common/chromium";
const DOWNLOAD_DIR = path.join(ENGINE_ROOT, "temp_csv");
const ERROR_SCREENSHOT_DIR = path.join(ENGINE_ROOT, "error_logs");

// FILE SISTEM ABSEN & MASTER CSV
const ABSEN_FILE = path.join(ENGINE_ROOT, "absen_sukses.txt");
const MASTER_CSV = path.join(ENGINE_ROOT, "master_katalog.csv");

const BATCH_SIZE = parseInt(process.env.BATCH_SIZE || "5", 10);
const TARGET_LINK = process.env.TARGET_LINK || "https://gemini.google.com/app/1c8f6f7e10330ba6";

const wait = ms => new Promise(r => setTimeout(r, ms));

// Pastikan folder-folder pendukung tersedia
[READY_DIR, DOWNLOAD_DIR, ERROR_SCREENSHOT_DIR].forEach(dir => {
    if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
});

// Buat Master CSV dengan Header jika belum ada
if (!fs.existsSync(MASTER_CSV)) {
    fs.writeFileSync(MASTER_CSV, "Filename,Title,Description,Keywords,Category,Releases\n", "utf-8");
}

// Buat File Absen jika belum ada
if (!fs.existsSync(ABSEN_FILE)) {
    fs.writeFileSync(ABSEN_FILE, "", "utf-8");
}

// ==========================================
// 💉 FUNGSI INJEKSI EXIFTOOL METADATA
// ==========================================
function injectMetadata(filePath, title, desc, keywords) {
    const safeTitle = title.replace(/["'`\\]/g, '').replace(/\n/g, ' ').trim();
    const safeDesc = desc.replace(/["'`\\]/g, '').replace(/\n/g, ' ').trim();
    const safeKw = keywords.replace(/["'`\\]/g, '').replace(/\n/g, ' ').trim();

    const cmd = `exiftool -overwrite_original -Title="${safeTitle}" -ObjectName="${safeTitle}" -Description="${safeDesc}" -ImageDescription="${safeDesc}" -sep "," -Keywords="${safeKw}" -Subject="${safeKw}" "${filePath}"`;

    try {
        execSync(cmd, { stdio: 'ignore' });
        console.log(`   [ExifTool] Injeksi sukses: ${path.basename(filePath)}`);
        return true;
    } catch(e) {
        console.error(`   [!] Gagal injeksi ExifTool pada: ${path.basename(filePath)}`);
        return false;
    }
}

// ==========================================
// 🔍 FUNGSI RADAR: CARI FILE YANG BELUM ABSEN
// ==========================================
function getUnprocessedBatch() {
    if (!fs.existsSync(READY_DIR)) fs.mkdirSync(READY_DIR, { recursive: true });
    
    // Baca buku absen
    const absenData = fs.readFileSync(ABSEN_FILE, 'utf8').split('\n').map(l => l.trim()).filter(Boolean);
    
    const files = fs.readdirSync(READY_DIR);
    const jpgFiles = files.filter(f => f.toLowerCase().endsWith('.jpg'));
    const epsFiles = files.filter(f => f.toLowerCase().endsWith('.eps'));

    let readyPairs = [];
    for (const jpg of jpgFiles) {
        const baseName = path.parse(jpg).name;
        
        // CEK SYARAT: Punya pasangan EPS & BELUM ADA di buku absen
        const hasEps = epsFiles.some(e => path.parse(e).name === baseName);
        if (hasEps && !absenData.includes(baseName)) {
            readyPairs.push(baseName);
        }
    }

    if (readyPairs.length > 0) {
        return readyPairs.slice(0, BATCH_SIZE);
    }
    return null;
}

// ==========================================
// 🤖 FUNGSI UTAMA (PROSES BATCH + APPEND)
// ==========================================
async function processBatch(baseNames) {
    console.log(`\n🚀 MEMPROSES ${baseNames.length} PASANGAN GAMBAR BARU (EPS & JPG)! Membuka Chromium Sesi Login...`);
    
    const oldFiles = fs.readdirSync(DOWNLOAD_DIR);
    for (const f of oldFiles) {
        try { fs.unlinkSync(path.join(DOWNLOAD_DIR, f)); } catch(e) {}
    }

    // Clear profile lock if present
    ["SingletonLock", "SingletonSocket", "SingletonCookie"].forEach(f => {
        const p = path.join(PROFILE, f);
        try { fs.rmSync(p, { force: true, recursive: true }); } catch (e) {}
    });

    const browser = await puppeteer.launch({
        headless: "new",
        executablePath: "/snap/bin/chromium", 
        userDataDir: PROFILE, 
        defaultViewport: null,
        args: [
            "--no-sandbox", 
            "--disable-setuid-sandbox", 
            "--start-maximized",
            "--disable-web-security",
            "--disable-features=IsolateOrigins,site-per-process"
        ]
    });

    const page = await browser.newPage();
    
    const client = await page.target().createCDPSession();
    await client.send('Page.setDownloadBehavior', { behavior: 'allow', downloadPath: DOWNLOAD_DIR });

    console.log(`-> Meluncur ke Canvas AI: ${TARGET_LINK}`);
    await page.goto(TARGET_LINK, { waitUntil: "networkidle2", timeout: 60000 });
    await wait(8000); 

    const jpgPaths = baseNames.map(name => path.join(READY_DIR, `${name}.jpg`));
    let downloadedCsvPath = null;

    try {
        let targetFrame = null;
        let fileInput = null;

        for (const frame of page.frames()) {
            try {
                let inputEl = await frame.$('#puppeteer-file-input');
                if (!inputEl) inputEl = await frame.$('input[type="file"]');
                if (inputEl) { targetFrame = frame; fileInput = inputEl; break; }
            } catch (err) {}
        }

        if (!fileInput || !targetFrame) {
            const ssPath = path.join(ERROR_SCREENSHOT_DIR, `missing_upload_btn_${Date.now()}.png`);
            await page.screenshot({ path: ssPath });
            throw new Error(`Gagal menemukan Iframe Canvas AI! Screenshot disimpan di ${ssPath}`);
        }

        console.log(`-> Menyuntikkan ${jpgPaths.length} gambar ke dalam Canvas AI...`);
        await fileInput.uploadFile(...jpgPaths);
        await wait(3000);

        console.log("-> Memerintahkan robot menekan tombol 'Mulai Proses Batch'...");
        await targetFrame.evaluate(() => {
            const buttons = Array.from(document.querySelectorAll('button'));
            const targetBtn = buttons.find(b => b.innerText && (b.innerText.includes('Mulai') || b.innerText.includes('Batch')));
            if (targetBtn) targetBtn.click();
        });

        console.log("-> 📡 Memantau AI Canvas (Maks 10 Menit)...");
        let isDone = false;
        for (let w = 1; w <= 60; w++) { // 60 * 10 detik = 10 menit
            await wait(10000); 
            isDone = await targetFrame.evaluate(() => {
                const elements = Array.from(document.querySelectorAll('button, div, p, span'));
                return elements.some(el => el.innerText && el.innerText.includes('Proses Selesai'));
            });

            if (isDone) {
                console.log(`-> ✅ AI SELESAI dalam waktu ${((w * 10) / 60).toFixed(1)} menit!`);
                break;
            } else {
                console.log(`   [...] Menunggu AI memproses batch... (${w * 10}s)`);
            }
        }

        if (!isDone) {
            const ssPath = path.join(ERROR_SCREENSHOT_DIR, `timeout_${Date.now()}.png`);
            await page.screenshot({ path: ssPath });
            throw new Error(`Waktu maksimal 10 menit habis, Canvas AI macet/belum selesai. Screenshot disimpan di ${ssPath}`);
        }

        console.log("-> 📥 Mendownload file CSV dari AI...");
        await targetFrame.evaluate(() => {
            const buttons = Array.from(document.querySelectorAll('button'));
            const downloadBtn = buttons.find(b => b.innerText && b.innerText.includes('Download'));
            if (downloadBtn) downloadBtn.click();
        });

        let attempts = 0;
        while (attempts < 15) {
            await wait(2000);
            const files = fs.readdirSync(DOWNLOAD_DIR);
            const csvFile = files.find(f => f.endsWith('.csv'));
            if (csvFile) {
                downloadedCsvPath = path.join(DOWNLOAD_DIR, csvFile);
                break;
            }
            attempts++;
        }

        if (!downloadedCsvPath) {
             const ssPath = path.join(ERROR_SCREENSHOT_DIR, `csv_not_found_${Date.now()}.png`);
             await page.screenshot({ path: ssPath });
             throw new Error(`Gagal mendeteksi file CSV yang didownload. Screenshot disimpan di ${ssPath}`);
        }

        // ==========================================
        // 💉 EKSTRAKSI CSV, SUNTIK EXIFTOOL & APPEND
        // ==========================================
        console.log("-> 💉 Membedah CSV, menyuntik ExifTool, dan menyambung ke Master CSV...");
        const csvRaw = fs.readFileSync(downloadedCsvPath, 'utf8');
        const lines = csvRaw.split('\n').filter(l => l.trim() !== '');
        
        let appendData = "";

        for (let i = 1; i < lines.length; i++) {
            const row = lines[i].match(/(".*?"|[^",\s]+)(?=\s*,|\s*$)/g);
            if (!row || row.length < 4) continue;

            appendData += lines[i] + "\n";

            const filename = row[0].replace(/"/g, '').trim();
            const title = row[1] ? row[1].replace(/"/g, '').trim() : '';
            const desc = row[2] ? row[2].replace(/"/g, '').trim() : '';
            const keywords = row[3] ? row[3].replace(/"/g, '').trim() : '';

            const baseName = filename.replace(/\.[^/.]+$/, "");
            const epsPath = path.join(READY_DIR, `${baseName}.eps`);
            const jpgPath = path.join(READY_DIR, `${baseName}.jpg`);

            if (fs.existsSync(epsPath)) injectMetadata(epsPath, title, desc, keywords);
            if (fs.existsSync(jpgPath)) injectMetadata(jpgPath, title, desc, keywords);
        }

        // APPEND KE MASTER CSV
        fs.appendFileSync(MASTER_CSV, appendData, 'utf8');
        console.log(`-> 📝 Metadata ditambahkan ke: ${path.basename(MASTER_CSV)}`);
        
        // ISI BUKU ABSEN
        for (const name of baseNames) {
            fs.appendFileSync(ABSEN_FILE, name + "\n", 'utf8');
        }
        console.log(`Downloaded: Metadata Batch Injected (${baseNames.length} Files Injected Successfully)`);

    } catch (err) {
        console.error(`Failed: ${baseNames.join(', ')} | ${err.message}`);
    } finally {
        await browser.close().catch(() => {});
    }
}

// ==========================================
// 🔄 EKSEKUSI BATCH
// ==========================================
(async () => {
    console.log("==================================================");
    console.log("🤖 STASIUN 4: AI METADATA & EXIFTOOL INJECTOR 🤖");
    console.log(`Target Folder Deliverables: ${READY_DIR}`);
    console.log("==================================================");

    const batch = getUnprocessedBatch();
    if (batch && batch.length > 0) {
        await processBatch(batch);
    } else {
        console.log("Tidak ada pasangan berkas baru (EPS & JPG) di Stasiun 4 yang memerlukan metadata.");
    }
})();
