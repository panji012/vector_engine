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

    // Download HTTP URLs into temp_remix/
    if (urls.length > 0) {
        console.log(`-> Unduh ${urls.length} link HTTP gambar acuan...`);
        for (let i = 0; i < urls.length; i++) {
            const url = urls[i];
            const dest = path.join(TEMP_REMIX_DIR, `http_source_${Date.now()}_${i + 1}.jpg`);
            try {
                await downloadHttpImage(url, dest);
                localFiles.push(dest);
                console.log(`   [HTTP] Berhasil mengunduh: ${path.basename(dest)}`);
            } catch (err) {
                console.error(`   [!] Gagal mengunduh HTTP URL: ${url} (${err.message})`);
            }
        }
    }

    if (localFiles.length === 0) {
        console.log("⚠️ [NODE 0] Antrean gambar acuan (HTTP / Lokal) kosong!");
        process.exit(0);
    }

    console.log(`-> Total ${localFiles.length} gambar acuan siap diremix ke Gemini Canvas...`);

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

    for (let i = 0; i < localFiles.length; i++) {
        const imagePath = localFiles[i];
        console.log(`\n-> [Batch ${i + 1}/${localFiles.length}] Mengunggah gambar acuan: ${path.basename(imagePath)}`);

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
                const ssPath = path.join(ERROR_SCREENSHOT_DIR, `missing_upload_remix_${Date.now()}.png`);
                await page.screenshot({ path: ssPath });
                throw new Error(`Gagal menemukan upload input Iframe Canvas! Screenshot disimpan di ${ssPath}`);
            }

            // Upload image
            await fileInput.uploadFile(imagePath);
            await wait(3000);

            // Inject master prompt text if text area available
            await targetFrame.evaluate((promptText) => {
                const textareas = Array.from(document.querySelectorAll('textarea, input[type="text"], div[contenteditable="true"]'));
                const promptBox = textareas.find(t => t.placeholder && (t.placeholder.includes('prompt') || t.placeholder.includes('Tanyakan') || t.placeholder.includes('Ketik')));
                if (promptBox) {
                    if (promptBox.tagName === 'DIV') promptBox.innerText = promptText;
                    else promptBox.value = promptText;
                    promptBox.dispatchEvent(new Event('input', { bubbles: true }));
                    promptBox.dispatchEvent(new Event('change', { bubbles: true }));
                }
            }, masterPrompt);

            // Click generate button
            console.log("-> Memerintahkan robot menekan 'Mulai Render Remix'...");
            await targetFrame.evaluate(() => {
                const buttons = Array.from(document.querySelectorAll('button'));
                const btn = buttons.find(b => b.innerText && (b.innerText.includes('Mulai') || b.innerText.includes('Batch') || b.innerText.includes('Generate') || b.innerText.includes('Kirim')));
                if (btn) btn.click();
            });

            // Poll for finish
            console.log("-> 📡 Memantau AI Canvas Rendering (Maks 10 Menit)...");
            let isDone = false;
            for (let w = 1; w <= 60; w++) {
                await wait(10000);
                isDone = await targetFrame.evaluate(() => {
                    const elements = Array.from(document.querySelectorAll('button, div, p, span'));
                    return elements.some(el => el.innerText && (el.innerText.includes('Proses Selesai') || el.innerText.includes('Selesai')));
                });

                if (isDone) {
                    console.log(`-> ✅ AI Remixer Selesai dalam ${((w * 10) / 60).toFixed(1)} menit!`);
                    break;
                } else {
                    console.log(`   [...] Merender AI Remix... (${w * 10}s)`);
                }
            }

            // Download generated image
            console.log("-> 📥 Mendownload hasil JPG remix...");
            await targetFrame.evaluate(() => {
                const buttons = Array.from(document.querySelectorAll('button'));
                const dlBtn = buttons.find(b => b.innerText && b.innerText.includes('Download'));
                if (dlBtn) dlBtn.click();
            });

            await wait(4000);

            // Determine output sequential name in input/
            const targetFilename = getNextSequentialName();
            const targetPath = path.join(INPUT_DIR, targetFilename);

            // Look for newly downloaded JPG in input/ and rename it if needed
            const files = fs.readdirSync(INPUT_DIR);
            const downloadedJpg = files.find(f => f.match(/\.jpg$/i) && !f.startsWith("vector_stock_"));
            if (downloadedJpg) {
                fs.renameSync(path.join(INPUT_DIR, downloadedJpg), targetPath);
            }

            console.log(`[ITEM_REMIX_SUCCESS] Downloaded: Remix Output Saved to input/${targetFilename}`);
            successCount++;

        } catch (err) {
            console.error(`[ITEM_REMIX_FAILED] ❌ [ERROR Batch ${i + 1}] ${err.message}`);
            failedCount++;
        }

        // Clean processed source file from temp_remix/
        try { fs.unlinkSync(imagePath); } catch (e) {}

        // Jeda 20 detik antar-proses gambar (simulasi diskusi & pendinginan AI Canvas)
        if (i < localFiles.length - 1) {
            console.log(`\n⏳ [NODE 0] Mengambil jeda 20 detik (simulasi diskusi & pendinginan AI Canvas)...`);
            await wait(COOLDOWN_MS);
        }
    }

    await browser.close();
    console.log(`\n🎉 [NODE 0] Selesai! ${successCount} gambar berhasil diremix & disimpan ke input/ untuk Stasiun 2.`);
}

main().catch(err => {
    console.error("Fatal Node 0 Error:", err);
    process.exit(1);
});
