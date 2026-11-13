/**
 * `npm run e2e`: starts the MOCK=1 dev server, runs every scenario in e2e/scenarios in a fresh
 * browser page (the mock backend lives in the page, so each scenario starts clean) and stops
 * the server again. Usage: `npm run e2e [-- name-filter]`.
 */
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, openSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = dirname(here);
const PORT = Number(process.env.E2E_PORT ?? 1430);
const URL_ = `http://127.0.0.1:${PORT}`;
const CHROMIUM = process.env.CHROMIUM ?? ["/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/google-chrome"].find(existsSync);

if (!existsSync(join(here, "node_modules", "puppeteer-core"))) {
  console.error("e2e dependencies missing: run `npm install --prefix e2e` first.");
  process.exit(2);
}
if (!CHROMIUM) {
  console.error("No Chromium found (set CHROMIUM=/path/to/chromium).");
  process.exit(2);
}
const { default: puppeteer } = await import("puppeteer-core");
const { T } = await import("./lib.mjs");

const shotDir = join(here, "screenshots");
mkdirSync(shotDir, { recursive: true });
const log = openSync(join(here, "vite.log"), "w");
const server = spawn("npx", ["vite", "dev", "--port", String(PORT), "--strictPort", "--host", "127.0.0.1"], {
  cwd: root,
  env: { ...process.env, MOCK: "1" },
  stdio: ["ignore", log, log],
  detached: true,
});
const stopServer = () => {
  try {
    process.kill(-server.pid); // the whole process group (npx -> vite)
  } catch {}
};
process.on("SIGINT", () => (stopServer(), process.exit(130)));
process.on("exit", stopServer);

let failures = 0;
try {
  for (let i = 0; ; i++) {
    try {
      if ((await fetch(URL_)).ok) break;
    } catch {}
    if (i > 150) throw new Error("dev server did not start (see e2e/vite.log)");
    await new Promise((r) => setTimeout(r, 200));
  }
  const filter = process.argv[2];
  const files = readdirSync(join(here, "scenarios")).filter((f) => f.endsWith(".mjs") && (!filter || f.includes(filter))).sort();
  const browser = await puppeteer.launch({ executablePath: CHROMIUM, headless: true, args: ["--no-sandbox"] });
  for (const f of files) {
    const mod = await import(join(here, "scenarios", f));
    // A fresh browser context per scenario: no shared localStorage, cookies or mock state.
    const ctx = await browser.createBrowserContext();
    const page = await ctx.newPage();
    await page.setViewport({ width: 1200, height: 800 });
    const t = new T(page, shotDir);
    const t0 = Date.now();
    try {
      await mod.default(t, URL_);
      t.noErrors();
      console.log(`PASS ${f} (${Date.now() - t0} ms)`);
    } catch (e) {
      failures++;
      console.log(`FAIL ${f}: ${e.stack?.split("\n").slice(0, 6).join("\n") ?? e}`);
      console.log("  page text: " + (await page.evaluate(() => document.body.innerText.replace(/\s+/g, " ").slice(-350)).catch(() => "")));
      if (t.errors.length) console.log("  browser errors:\n  " + t.errors.join("\n  "));
      await page.screenshot({ path: join(shotDir, `FAIL-${f}.png`) }).catch(() => {});
    }
    await ctx.close();
  }
  await browser.close();
} finally {
  stopServer();
}
console.log(failures ? `${failures} scenario(s) failed` : "all e2e scenarios passed");
process.exit(failures ? 1 : 0);
