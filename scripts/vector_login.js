const puppeteer = require("puppeteer-extra");
const StealthPlugin = require("puppeteer-extra-plugin-stealth");
const fs = require("fs");
const path = require("path");

puppeteer.use(StealthPlugin());

const ENGINE_ROOT = "/home/panjikali/Projects/vector_engine";
const PROFILE = "/home/panjikali/snap/chromium/common/chromium";

// Clear lock file if left over from previous process
["SingletonLock", "SingletonSocket", "SingletonCookie"].forEach(f => {
  const p = path.join(PROFILE, f);
  try { fs.rmSync(p, { force: true, recursive: true }); } catch (e) {}
});

(async () => {
  const browser = await puppeteer.launch({
    headless: false,
    executablePath: "/snap/bin/chromium",
    userDataDir: PROFILE,
    defaultViewport: null,
    ignoreDefaultArgs: ["--enable-automation"],
    args: [
      "--no-sandbox",
      "--disable-setuid-sandbox",
      "--disable-blink-features=AutomationControlled",
      "--disable-infobars",
      "--start-maximized"
    ]
  });

  const page = await browser.newPage();
  await page.goto("https://vectorizer.ai", {
    waitUntil: "networkidle2"
  });

  console.log("\nLOGIN MANUAL (Profil Bersih & Stealth Active):");
  console.log("1. Login akun (Google / Vectorizer)");
  console.log("2. Pilih output SVG");
  console.log("3. Tutup browser\n");
})();
