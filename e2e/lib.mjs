/** Helpers shared by the scenarios: everything goes through real (simulated) mouse and keyboard events. */
import assert from "node:assert/strict";

export { assert };
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export class T {
  constructor(page, shotDir) {
    this.page = page;
    this.shotDir = shotDir;
    this.errors = [];
    page.on("pageerror", (e) => this.errors.push("pageerror: " + e.message));
    page.on("console", (m) => {
      if (m.type() === "error" || (m.type() === "warning" && /svelte|state_unsafe|ownership|reactiv/i.test(m.text()))) this.errors.push(`console.${m.type()}: ${m.text().slice(0, 300)}`);
    });
  }

  async load(url, { tour = false } = {}) {
    const p = this.page;
    // The first-run tour would cover the UI; scenarios that test it ask for it explicitly.
    await p.evaluateOnNewDocument((tour) => {
      try {
        if (!tour) localStorage.setItem("chroma-desk.tour", "done");
      } catch {}
    }, tour);
    await p.goto(url, { waitUntil: "networkidle0" });
    await this.waitFor(() => p.$("aside nav .dev"), "sidebar devices");
  }

  async waitFor(fn, what = "condition", ms = 6000) {
    const t0 = Date.now();
    for (;;) {
      const v = await fn();
      if (v) return v;
      if (Date.now() - t0 > ms) throw new Error(`timeout waiting for ${what}`);
      await sleep(60);
    }
  }

  /** Clicks (real mouse) the first element matching `sel` whose text matches `re`. */
  async clickText(sel, re, { scope = "" } = {}) {
    const h = await this.waitFor(
      () =>
        this.page.evaluateHandle(
          (sel, src, scope) => {
            const root = scope ? document.querySelector(scope) : document;
            const r = new RegExp(src);
            return [...(root?.querySelectorAll(sel) ?? [])].find((e) => (r.test(e.textContent.trim()) || r.test(e.getAttribute("aria-label") || "")) && !e.disabled && e.offsetParent !== null) ?? null;
          },
          sel,
          re,
          scope,
        ).then((h) => (h.asElement() ? h : null)),
      `${sel} /${re}/`,
    );
    await h.asElement().evaluate((e) => e.scrollIntoView({ block: "center" }));
    const b = await h.asElement().boundingBox();
    await this.page.mouse.click(b.x + b.width / 2, b.y + b.height / 2);
    await sleep(120);
  }

  async openDevice(re) {
    await this.clickText("aside nav .dev", re);
    await this.waitFor(() => this.page.$(".title h2"), "device title");
    await sleep(250);
  }

  async nav(re) {
    await this.clickText("aside button", re);
    await sleep(250);
  }

  async tab(re) {
    await this.clickText('[role="tab"]', re);
    await sleep(250);
  }

  /** Fill colour of an LED of the editing canvas (what the user sees). */
  fill(led, scope = "svg.canvas") {
    return this.page.$eval(`${scope} [data-led="${led}"]`, (el) => (el.querySelector("rect,circle") ?? el).getAttribute("fill")).catch(() => null);
  }

  /** Centre (viewport coords) of the LED with this name or index. */
  async ledPos(nameOrIdx, scope = "svg.canvas") {
    return this.page.evaluate(
      (k, scope) => {
        const els = [...document.querySelectorAll(`${scope} [data-led]`)];
        const g = typeof k === "number" ? els.find((e) => e.getAttribute("data-led") === String(k)) : els.find((e) => e.querySelector("title").textContent.startsWith(k + " ("));
        if (!g) return null;
        const r = g.getBoundingClientRect();
        return [r.x + r.width / 2, r.y + r.height / 2];
      },
      nameOrIdx,
      scope,
    );
  }

  /** Scrolls the canvas to the middle of the view so every LED has stable coordinates. */
  async centerCanvas() {
    await this.page.$eval("svg.canvas", (e) => e.scrollIntoView({ block: "center" }));
    await sleep(60);
  }

  async clickLed(nameOrIdx, opts = {}) {
    await this.centerCanvas();
    const p = await this.ledPos(nameOrIdx);
    assert.ok(p, `LED ${nameOrIdx} not on the canvas`);
    await this.page.mouse.move(p[0], p[1]);
    if (opts.ctrl) await this.page.keyboard.down("Control");
    await this.page.mouse.down();
    await this.page.mouse.up();
    if (opts.ctrl) await this.page.keyboard.up("Control");
    await sleep(150);
  }

  /** Drags across LEDs in order, pressing at the first one. */
  async dragLeds(list, { steps = 1 } = {}) {
    await this.centerCanvas();
    const pts = [];
    for (const k of list) {
      const p = await this.ledPos(k);
      assert.ok(p, `LED ${k} not on the canvas`);
      pts.push(p);
    }
    const m = this.page.mouse;
    await m.move(...pts[0]);
    await m.down();
    for (let i = 1; i < pts.length; i++) await m.move(pts[i][0], pts[i][1], { steps });
    await m.up();
    await sleep(200);
  }

  /** Index of the LED with this name on the canvas. */
  ledIndex(name, scope = "svg.canvas") {
    return this.page.evaluate(
      (n, scope) => {
        const g = [...document.querySelectorAll(`${scope} [data-led]`)].find((e) => e.querySelector("title").textContent.startsWith(n + " ("));
        return g ? Number(g.getAttribute("data-led")) : null;
      },
      name,
      scope,
    );
  }

  /** Opens the screen where the paint brush lives (Draw > Paint). */
  async gotoPaint() {
    await this.level("Draw");
  }

  /** Lighting level (Quick / Draw / Layers). */
  async level(name) {
    await this.clickText(".level", `^${name}`);
    await sleep(250);
  }

  /** Canvas pixel sum of an animated thumbnail: changes while it animates. */
  thumbSig(animId) {
    return this.page.evaluate((id) => {
      const c = document.querySelector(`[data-anim="${id}"] canvas`);
      if (!c) return null;
      const d = c.getContext("2d").getImageData(0, 0, c.width, c.height).data;
      let h = 0;
      for (let i = 0; i < d.length; i += 4) h = (h * 31 + d[i] + d[i + 1] * 3 + d[i + 2] * 7) | 0;
      return h;
    }, animId);
  }

  /** Draw kind selector (Paint / Frames / Path / Palette). */
  async drawKind(name) {
    await this.clickText(".kind", `^${name}`);
    await sleep(300);
  }

  /** The animation layer the editor sends to the engine as live preview. */
  async previewAnim(key = "Razer Huntsman V3 Pro TKL@0") {
    const s = await this.scene();
    const l = s.devices[key]?.layers?.[0];
    return l?.effect === "animation" ? l.params.snapshot : null;
  }

  /** Samples the fills of the given LEDs every `every` ms for `ms` ms. */
  async sampleFills(leds, ms, every = 50) {
    const out = [];
    const t0 = Date.now();
    while (Date.now() - t0 < ms) {
      out.push(await this.page.evaluate((leds) => leds.map((l) => document.querySelector(`svg.canvas [data-led="${l}"] .shape`)?.getAttribute("fill")), leds));
      await sleep(every);
    }
    return out;
  }

  /** Sets the range slider of the editor row whose label matches. */
  async setSlider(label, value) {
    const ok = await this.page.evaluate((label, v) => {
      const row = [...document.querySelectorAll(".sliders .row, .settings .inline")].find((r) => r.textContent.trim().startsWith(label));
      const el = row?.querySelector("input[type=range]");
      if (!el) return false;
      el.value = String(v);
      el.dispatchEvent(new Event("input", { bubbles: true }));
      return true;
    }, label, value);
    assert.ok(ok, `slider ${label}`);
  }

  async setRange(sel, value) {
    await this.page.$eval(sel, (el, v) => {
      el.value = v;
      el.dispatchEvent(new Event("input", { bubbles: true }));
    }, String(value));
  }

  async setColor(hex) {
    await this.page.$eval('.toolbar input[type="color"]', (el, v) => {
      el.value = v;
      el.dispatchEvent(new Event("input", { bubbles: true }));
      el.dispatchEvent(new Event("change", { bubbles: true }));
    }, hex);
  }

  async tool(re) {
    await this.clickText(".tools button", re);
  }

  /** Scene held by the mock backend (what the engine would run). */
  scene() {
    return this.page.evaluate(() => JSON.parse(JSON.stringify(globalThis.__chromaMock.scene)));
  }
  mockState(key) {
    return this.page.evaluate((k) => JSON.parse(JSON.stringify(globalThis.__chromaMock[k] ?? null)), key);
  }

  /** Screenshots at the given size: the top of the page and, when it scrolls, the bottom too. */
  async shot(name, w = 1200, h = 800) {
    const p = this.page;
    const old = p.viewport();
    await p.setViewport({ width: w, height: h });
    await sleep(500);
    const dims = await p.evaluate(() => {
      const m = document.querySelector("main") ?? document.scrollingElement;
      m.scrollTop = 0;
      return { max: m.scrollHeight - m.clientHeight };
    });
    await sleep(150);
    await p.screenshot({ path: `${this.shotDir}/${name}-${w}x${h}.png` });
    if (dims.max > 40) {
      await p.evaluate(() => {
        const m = document.querySelector("main") ?? document.scrollingElement;
        m.scrollTop = m.scrollHeight;
      });
      await sleep(150);
      await p.screenshot({ path: `${this.shotDir}/${name}-${w}x${h}-bottom.png` });
    }
    await p.setViewport(old);
    await sleep(200);
  }

  noErrors() {
    assert.deepEqual(this.errors, [], "browser errors: " + this.errors.join("\n"));
  }
}
