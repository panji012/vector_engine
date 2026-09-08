const puppeteer = require("puppeteer-extra");
const StealthPlugin = require("puppeteer-extra-plugin-stealth");
const fs = require("fs");
const path = require("path");
const https = require("https");
const http = require("http");
const { execSync } = require("child_process");

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

const TARGET_LINK = process.env.TARGET_LINK || "https://gemini.google.com/app/4f103880bda96c54";
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

// Build system prompt master rules
function buildMasterPrompt() {
    let styleInstruction = "";
    if (STYLE_LEVEL === 'Off') {
        styleInstruction = "STYLE INSTRUCTION (OFF): Do not apply standard stylistic mimicry. Focus purely on rules below and STRICTLY CLONE style from Style Reference Image.";
    } else if (STYLE_LEVEL === 'Hard') {
        styleInstruction = "HARD MODE: Completely change and redesign ALL objects so they do not look copied. DO NOT TRACE. Create new objects fitting the exact same theme. Mimic font style with new text.";
    } else if (STYLE_LEVEL === 'Easy') {
        styleInstruction = "EASY MODE: Maintain theme, vibe, and visual atmosphere. DO NOT TRACE. Recreate main object from scratch with unique shape & pose. Mimic typography with new words. Microstock safe.";
    } else {
        styleInstruction = "Maintain artistic feel but add fresh touch.";
    }

    const colorInstruction = COLOR_MATCH === 'Kunci'
        ? "CRITICAL DESIGN COLOR RULE: Extract exact HEX color codes, luminance, and palette from original design. Reuse exact same color grading."
        : (COLOR_MATCH === 'Moderat' ? "Maintain base colors of design with adjusted nuances." : "Create a new color style fitting prompt.");

    const lightingInstruction = TOOL_ANTI_GELAP
        ? "\n<RULE: EXACT LUMINANCE & BRIGHTNESS>\nLock luminance. Maintain 100% brightness and exact shading ratios.\n</RULE>" : "";

    const bgCleanInstruction = !TOOL_BG_BERSIH
        ? "\n<RULE: CLEAN ORIGINAL BACKGROUND>\nExtract background color, force a completely clean, smooth, solid background.\n</RULE>" : "";

    const canvasInstruction = CANVAS_RATIO === '16:9'
        ? "\n<RULE: CANVAS RATIO>\nOutput MUST be in WIDE 16:9 aspect ratio.\n</RULE>"
        : (CANVAS_RATIO === '5:6' ? "\n<RULE: CANVAS RATIO>\nOutput MUST be in VERTICAL 5:6 aspect ratio.\n</RULE>"
        : "\n<RULE: CANVAS RATIO>\nOutput MUST be in SQUARE 1:1 aspect ratio.\n</RULE>");

    let paletteInstruction = "";
    if (COLOR_THEME === 'Vintage' || COLOR_THEME === 'Retro') {
        paletteInstruction = `\n<RULE: DYNAMIC ${COLOR_THEME.toUpperCase()} COLOR PALETTE>\nGenerate aesthetic 4-color ${COLOR_THEME.toUpperCase()} palette. 1 solid background color, remaining 3 colors for main objects.\n</RULE>`;
    }

    return `<MASTER DIRECTIVE>
1. Style Rule: ${styleInstruction}
2. Design Color Rule: ${colorInstruction}
3. Custom Prompt: ${CUSTOM_PROMPT || 'Follow instructions perfectly.'}
<RULE: NO WATERMARKS>Completely ignore and remove usernames, watermarks, or text stamps.</RULE>
${lightingInstruction}
${bgCleanInstruction}
${canvasInstruction}
${paletteInstruction}
<RULE: SINGLE OUTPUT ONLY>Generate EXACTLY ONE (1) final image. Do NOT split into grids.</RULE>
</MASTER DIRECTIVE>`;
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

    // Clean profile lock if present
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
    await client.send('Page.setDownloadBehavior', { behavior: 'allow', downloadPath: INPUT_DIR });

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
        await wait(1500);

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

        // Monitor batch rendering & downloads directly from input/ every 25 seconds
        console.log(`-> 📡 Memantau folder input/ setiap 25 detik hingga ${targetBatchCount} berkas fisik mendarat (Maks 20 Menit)...`);
        for (let w = 1; w <= 48; w++) {
            await wait(25000);

            // Scan input/ directory for newly downloaded files from Gemini Canvas
            const currentFiles = fs.readdirSync(INPUT_DIR);
            const newFiles = currentFiles.filter(f => !initialInputFiles.has(f) && f.match(/\.(jpg|jpeg|png|webp)$/i) && !f.startsWith("vector_stock_"));

            for (const nf of newFiles) {
                const targetFilename = getNextSequentialName();
                const oldPath = path.join(INPUT_DIR, nf);
                const newPath = path.join(INPUT_DIR, targetFilename);
                fs.renameSync(oldPath, newPath);
                initialInputFiles.add(targetFilename);

                console.log(`[ITEM_REMIX_SUCCESS] Downloaded: Remix Output Saved to input/${targetFilename}`);
                successCount++;
            }

            if (successCount >= targetBatchCount) {
                console.log(`-> ✅ LENGKAP! Seluruh ${successCount}/${targetBatchCount} berkas fisik telah mendarat di input/ dalam ${((w * 25) / 60).toFixed(1)} menit!`);
                break;
            } else {
                console.log(`   [...] Memantau unduhan fisik di folder input/... (${w * 25}s | ${successCount}/${targetBatchCount} berkas mendarat)`);
            }
        }

        // Final check after completion wait
        await wait(5000);
        const finalFiles = fs.readdirSync(INPUT_DIR);
        const remainingNew = finalFiles.filter(f => !initialInputFiles.has(f) && f.match(/\.(jpg|jpeg|png|webp)$/i) && !f.startsWith("vector_stock_"));
        for (const nf of remainingNew) {
            const targetFilename = getNextSequentialName();
            const oldPath = path.join(INPUT_DIR, nf);
            const newPath = path.join(INPUT_DIR, targetFilename);
            fs.renameSync(oldPath, newPath);
            initialInputFiles.add(targetFilename);

            console.log(`[ITEM_REMIX_SUCCESS] Downloaded: Remix Output Saved to input/${targetFilename}`);
            successCount++;
        }

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
