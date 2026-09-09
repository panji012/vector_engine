const puppeteer = require("puppeteer-extra");
const StealthPlugin = require("puppeteer-extra-plugin-stealth");
const fs = require("fs");
const path = require("path");
const https = require("https");
const http = require("http");
const { execSync } = require("child_process");

process.env.DISPLAY = process.env.DISPLAY || ":0";
puppeteer.use(StealthPlugin());

// ==========================================
// ⚙️ KONFIGURASI VECTOR ENGINE - STASIUN 0 (REMIXER)
// ==========================================
const ENGINE_ROOT = "/home/panjikali/Projects/vector_engine";
const INPUT_DIR = path.join(ENGINE_ROOT, "input");
const TEMP_REMIX_DIR = path.join(ENGINE_ROOT, "temp_remix");
const PROFILE = "/home/panjikali/snap/chromium/common/chromium";
const ERROR_SCREENSHOT_DIR = path.join(ENGINE_ROOT, "error_logs");
const MANIFEST_PATH = path.join(ENGINE_ROOT, "manifest.json");

const LINK_FILE_PATH = path.join(ENGINE_ROOT, "link", "gemini_canvas.txt");
let defaultCanvasUrl = "https://share.gemini.google/eSESkPvM47rw";
if (fs.existsSync(LINK_FILE_PATH)) {
    const savedUrl = fs.readFileSync(LINK_FILE_PATH, "utf8").trim();
    if (savedUrl) defaultCanvasUrl = savedUrl;
}
const TARGET_LINK = process.env.TARGET_LINK || defaultCanvasUrl;
const STYLE_LEVEL = process.env.STYLE_LEVEL || "Easy";
const COLOR_MATCH = process.env.COLOR_MATCH || "Kunci";
const CANVAS_RATIO = process.env.CANVAS_RATIO || "1:1";
const CUSTOM_PROMPT = process.env.CUSTOM_PROMPT || "";
const TOOL_ANTI_GELAP = process.env.TOOL_ANTI_GELAP === "true";
const TOOL_BG_BERSIH = process.env.TOOL_BG_BERSIH === "true";
const COLOR_THEME = process.env.COLOR_THEME || "";
const HTTP_URLS_RAW = process.env.HTTP_URLS || "";
const COOLDOWN_MS = 20000; // 20 detik jeda diskusi & pendinginan AI Canvas

const wait = ms => new Promise(r => setTimeout(r, ms));

[INPUT_DIR, TEMP_REMIX_DIR, ERROR_SCREENSHOT_DIR].forEach(dir => {
    if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
});

// Helper: Download image from HTTP/HTTPS URL safely
function downloadHttpImage(url, destPath) {
    return new Promise((resolve, reject) => {
        const file = fs.createWriteStream(destPath);
        const protocol = url.startsWith("https") ? https : http;
        protocol.get(url, (response) => {
            if (response.statusCode === 301 || response.statusCode === 302) {
                return downloadHttpImage(response.headers.location, destPath).then(resolve).catch(reject);
            }
            if (response.statusCode !== 200) {
                return reject(new Error(`HTTP error ${response.statusCode}`));
            }
            response.pipe(file);
            file.on('finish', () => {
                file.close(() => resolve(destPath));
            });
        }).on('error', (err) => {
            fs.unlink(destPath, () => {});
            reject(err);
        });
    });
}

// Find highest sequential number in input/
function getNextSequentialName(prefix = "vector_stock") {
    let highestNum = 0;
    const pattern = new RegExp(`^${prefix}_(\\d+)\\.`, 'i');
    
    if (fs.existsSync(INPUT_DIR)) {
        const files = fs.readdirSync(INPUT_DIR);
        for (const f of files) {
            const m = f.match(pattern);
            if (m) {
                const val = parseInt(m[1], 10);
                if (val > highestNum) highestNum = val;
            }
        }
    }
    const nextNum = highestNum + 1;
    const formattedNum = String(nextNum).padStart(4, '0');
    return `${prefix}_${formattedNum}.jpg`;
}

// Build system prompt master rules (Priority 1: Manual User Prompt, Priority 2: Preset Prompt 1 file)
function buildMasterPrompt() {
    if (CUSTOM_PROMPT && CUSTOM_PROMPT.trim().length > 0) {
        return CUSTOM_PROMPT.trim();
    }

    const presetPath = path.join(ENGINE_ROOT, "prompt", "1_spot_illustration.txt");
    if (fs.existsSync(presetPath)) {
        const presetContent = fs.readFileSync(presetPath, "utf8").trim();
        if (presetContent) return presetContent;
    }

    return "";
}

async function main() {
    console.log("🚀 [NODE 0] Memulai AI Concept Remixer Engine (Puppeteer)...");
    
    // Parse HTTP URLs
    const urls = HTTP_URLS_RAW.match(/https?:\/\/[^\s,]+/gi) || [];
    let localFiles = [];

    // Check temp_remix/ for any pre-uploaded local files
    if (fs.existsSync(TEMP_REMIX_DIR)) {
        const files = fs.readdirSync(TEMP_REMIX_DIR);
        localFiles = files.filter(f => f.match(/\.(jpg|jpeg|png|webp)$/i)).map(f => path.join(TEMP_REMIX_DIR, f));
    }

    // Determine input mode and target batch count
    let targetBatchCount = 0;
    if (urls.length > 0) {
        targetBatchCount = urls.length;
        console.log(`-> Target: ${urls.length} gambar dari link HTTP siap diremix ke Gemini Canvas...`);
    } else if (localFiles.length > 0) {
        targetBatchCount = localFiles.length;
        console.log(`-> Target: ${localFiles.length} gambar lokal siap diremix ke Gemini Canvas...`);
    } else {
        console.log("⚠️ [NODE 0] Antrean gambar acuan (HTTP / Lokal) kosong!");
        process.exit(0);
    }

    // Single-instance process cleanup to avoid Chromium lock file conflicts & orphan windows
    try {
        execSync("pkill -f 'chromium.*--type=renderer' || true");
    } catch (e) {}
    ["SingletonLock", "SingletonSocket", "SingletonCookie"].forEach(f => {
        const p = path.join(PROFILE, f);
        try { fs.rmSync(p, { force: true, recursive: true }); } catch (e) {}
    });

    const isHeadless = process.env.HEADLESS === "true";
    const browser = await puppeteer.launch({
        headless: isHeadless ? "new" : false,
        executablePath: "/snap/bin/chromium",
        userDataDir: PROFILE,
        defaultViewport: null,
        args: [
            "--no-sandbox",
            "--disable-setuid-sandbox",
            "--start-maximized",
            `--user-data-dir=${PROFILE}`,
            "--profile-directory=Default",
            "--disable-profile-picker",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-popup-blocking",
            "--disable-features=IsolateOrigins,site-per-process"
        ]
    });

    const page = await browser.newPage();
    const client = await page.target().createCDPSession();
    await client.send('Page.setDownloadBehavior', { behavior: 'allow', downloadPath: INPUT_DIR });
    await client.send('Browser.setPermission', {
        permission: { name: 'automatic-downloads' },
        setting: 'granted',
    }).catch(() => {});

    console.log(`-> Meluncur ke Gemini AI Canvas: ${TARGET_LINK}`);
    await page.goto(TARGET_LINK, { waitUntil: "networkidle2", timeout: 60000 });
    await wait(8000);

    const masterPrompt = buildMasterPrompt();
    let successCount = 0;
    let failedCount = 0;

    // Record existing files in input/ before batch execution
    const initialInputFiles = new Set(fs.existsSync(INPUT_DIR) ? fs.readdirSync(INPUT_DIR) : []);

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

        if (!targetFrame) {
            const ssPath = path.join(ERROR_SCREENSHOT_DIR, `missing_iframe_remix_${Date.now()}.png`);
            await page.screenshot({ path: ssPath });
            throw new Error(`Gagal menemukan Iframe Gemini Canvas AI! Screenshot disimpan di ${ssPath}`);
        }

        // Anti-Duplication Input: Separate HTTP URLs vs Local Uploads
        if (urls.length > 0) {
            console.log(`-> 🔗 Menginput ${urls.length} link HTTP gambar sekaligus ke #urlInput Canvas...`);
            await targetFrame.evaluate((urlList) => {
                let httpBox = document.getElementById('urlInput');
                if (!httpBox) {
                    const textareas = Array.from(document.querySelectorAll('textarea, input[type="text"]'));
                    httpBox = textareas.find(t => t.placeholder && (t.placeholder.includes('http') || t.placeholder.includes('URL') || t.placeholder.includes('Pinterest')));
                }
                if (httpBox) {
                    httpBox.value = urlList.join("\n");
                    httpBox.dispatchEvent(new Event('input', { bubbles: true }));
                    httpBox.dispatchEvent(new Event('change', { bubbles: true }));
                }
            }, urls);
            await wait(1500);
        } else if (localFiles.length > 0) {
            console.log(`-> 📤 Mengunggah ${localFiles.length} berkas gambar acuan lokal ke Canvas AI...`);
            let fileInputEl = await targetFrame.$('#fileInput');
            if (!fileInputEl) fileInputEl = await targetFrame.$('input[type="file"]');
            if (fileInputEl) {
                await fileInputEl.uploadFile(...localFiles);
                await wait(3000);
            }
        }

        // Inject master prompt directive
        await targetFrame.evaluate((promptText) => {
            let promptBox = document.getElementById('promptInput');
            if (!promptBox) {
                const textareas = Array.from(document.querySelectorAll('textarea, input[type="text"], div[contenteditable="true"]'));
                promptBox = textareas.find(t => t.placeholder && (t.placeholder.includes('prompt') || t.placeholder.includes('Konsep') || t.placeholder.includes('Tanyakan') || t.placeholder.includes('Ketik')));
            }
            if (promptBox) {
                if (promptBox.tagName === 'DIV') promptBox.innerText = promptText;
                else promptBox.value = promptText;
                promptBox.dispatchEvent(new Event('input', { bubbles: true }));
                promptBox.dispatchEvent(new Event('change', { bubbles: true }));
            }
        }, masterPrompt);
        // Click parameter UI buttons directly on Canvas Gemini page
        await targetFrame.evaluate(({ styleLvl, colorMtch, canvasRat, t1, t2 }) => {
            if (typeof setStyle === 'function') setStyle(styleLvl);
            if (typeof setColor === 'function') setColor(colorMtch);
            if (typeof setCanvas === 'function') setCanvas(canvasRat);

            const tool1El = document.getElementById('tool1');
            if (tool1El) tool1El.checked = t1;
            const tool2El = document.getElementById('tool2');
            if (tool2El) tool2El.checked = t2;
        }, {
            styleLvl: STYLE_LEVEL,
            colorMtch: COLOR_MATCH,
            canvasRat: CANVAS_RATIO,
            t1: TOOL_ANTI_GELAP,
            t2: TOOL_BG_BERSIH
        }).catch(() => {});
        await wait(1000);

        // Click "Mulai Proses (Batch)" button (#btnStart)
        console.log("-> 🚀 Memerintahkan robot menekan 'Mulai Proses (Batch)' (#btnStart)...");
        await targetFrame.evaluate(() => {
            let btn = document.getElementById('btnStart');
            if (!btn) {
                const buttons = Array.from(document.querySelectorAll('button, a'));
                btn = buttons.find(b => b.innerText && (b.innerText.includes('Mulai') || b.innerText.includes('Batch') || b.innerText.includes('Generate') || b.innerText.includes('Kirim')));
            }
            if (btn) btn.click();
        });

        // Snapshot file lama di input/ dan Downloads/ sebelum batch dijalankan
        const DOWNLOADS_DIR = path.join(require("os").homedir(), "Downloads");
        const initialJpgFiles = fs.readdirSync(INPUT_DIR).filter(f => f.match(/\.(jpg|jpeg|png|webp)$/i));
        const initialJpgCount = initialJpgFiles.length;
        const initialFileSet = new Set(initialJpgFiles);
        const initialDownloadsFiles = new Set(fs.existsSync(DOWNLOADS_DIR) ? fs.readdirSync(DOWNLOADS_DIR) : []);

        console.log(`-> 📡 Memantau folder input/ & auto-sync Downloads/ via Event Watcher (Baseline file lama: ${initialJpgCount} berkas)...`);
        console.log(`-> ⏱️ Mode Watcher Inaktivitas: Jika tidak ada unduhan baru selama 30 detik, Gemini Canvas otomatis ditutup.`);

        // Helper untuk memproses file unduhan baru
        function processNewFiles() {
            let processedAny = false;

            // 1. Auto-sync file baru dari Downloads/ ke input/
            if (fs.existsSync(DOWNLOADS_DIR)) {
                const dlFiles = fs.readdirSync(DOWNLOADS_DIR);
                const newDlFiles = dlFiles.filter(f => !initialDownloadsFiles.has(f) && f.match(/^Remix_.*\.(jpg|jpeg|png|webp)$/i) && !f.endsWith('.crdownload'));
                
                for (const dlf of newDlFiles) {
                    const srcPath = path.join(DOWNLOADS_DIR, dlf);
                    const destPath = path.join(INPUT_DIR, dlf);
                    try {
                        fs.renameSync(srcPath, destPath);
                        initialDownloadsFiles.add(dlf);
                    } catch (e) {
                        try {
                            fs.copyFileSync(srcPath, destPath);
                            fs.unlinkSync(srcPath);
                            initialDownloadsFiles.add(dlf);
                        } catch (err) {}
                    }
                }
            }

            // 2. Scan input/ directory untuk file baru dan rename secara sekuensial
            const currentFiles = fs.readdirSync(INPUT_DIR);
            const unrenamedNewFiles = currentFiles.filter(f => !initialFileSet.has(f) && f.match(/\.(jpg|jpeg|png|webp)$/i) && !f.startsWith("vector_stock_") && !f.endsWith('.crdownload'));

            for (const nf of unrenamedNewFiles) {
                const targetFilename = getNextSequentialName();
                const oldPath = path.join(INPUT_DIR, nf);
                const newPath = path.join(INPUT_DIR, targetFilename);
                try {
                    fs.renameSync(oldPath, newPath);
                } catch (e) {
                    continue;
                }
                
                initialFileSet.add(targetFilename);
                initialFileSet.add(nf);
                successCount++;
                processedAny = true;

                console.log(`[ITEM_REMIX_SUCCESS] Absen Berkas Baru (#${successCount}/${targetBatchCount}): ${nf} -> ${targetFilename}`);
            }

            return processedAny;
        }

        // Jalankan Event Watcher + 30 Detik Inactivity Timer
        await new Promise((resolve) => {
            let inactivityTimeout = null;

            const resetInactivityTimer = () => {
                if (inactivityTimeout) clearTimeout(inactivityTimeout);
                inactivityTimeout = setTimeout(() => {
                    console.log(`-> ℹ️ Watcher terdeteksi 30 detik tanpa aktivitas unduhan baru. Gemini Canvas selesai & ditutup!`);
                    cleanup();
                    resolve();
                }, 30000);
            };

            const handleFileEvent = () => {
                const newFound = processNewFiles();
                if (newFound) {
                    resetInactivityTimer(); // Reset 30s timer karena ada aktivitas download baru!
                    if (targetBatchCount > 0 && successCount >= targetBatchCount) {
                        console.log(`-> ✅ LENGKAP! Seluruh ${successCount}/${targetBatchCount} berkas fisik baru telah terverifikasi mendarat di input/!`);
                        cleanup();
                        resolve();
                    }
                }
            };

            // Pasang fs.watch di Downloads dan input
            const watchers = [];
            try {
                if (fs.existsSync(DOWNLOADS_DIR)) {
                    watchers.push(fs.watch(DOWNLOADS_DIR, (eventType, filename) => {
                        if (filename && !filename.endsWith('.crdownload')) {
                            handleFileEvent();
                        }
                    }));
                }
            } catch (e) {}

            try {
                watchers.push(fs.watch(INPUT_DIR, (eventType, filename) => {
                    if (filename && !filename.endsWith('.crdownload')) {
                        handleFileEvent();
                    }
                }));
            } catch (e) {}

            const cleanup = () => {
                if (inactivityTimeout) clearTimeout(inactivityTimeout);
                watchers.forEach(w => {
                    try { w.close(); } catch (e) {}
                });
            };

            // Cek awal & mulai timer 30 detik pertama
            handleFileEvent();
            resetInactivityTimer();
        });

        // Final sync check
        await wait(1000);
        processNewFiles();

    } catch (err) {
        console.error(`[ITEM_REMIX_FAILED] ❌ [ERROR Node 0] ${err.message}`);
        failedCount++;
    }

    // Clean processed source files from temp_remix/
    localFiles.forEach(fp => {
        try { fs.unlinkSync(fp); } catch (e) {}
    });

    await browser.close();
    console.log(`\n🎉 [NODE 0] Selesai! ${successCount} gambar fisik berhasil terverifikasi & tersimpan di input/ untuk Stasiun 2.`);
}

main().catch(err => {
    console.error("Fatal Node 0 Error:", err);
    process.exit(1);
});
