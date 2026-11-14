/** Quick tab: themes (one at a time) and effects (stacking toggles), "Theme: X" / "Effect: X" chips, green active state. */
import { assert, sleep } from "../lib.mjs";

export default async function (t, url) {
  await t.load(url);
  await t.openDevice("Huntsman");
  const kb = "Razer Huntsman V3 Pro TKL@0";
  const layers = async () => (await t.scene()).devices[kb]?.layers ?? [];
  const chips = (sel) => t.page.$$eval(`[data-now-playing] ${sel}`, (e) => e.map((x) => x.firstChild.textContent.trim()));
  const green = (sel) => t.page.$eval(sel, (e) => { const ok = getComputedStyle(document.documentElement).getPropertyValue("--ok").trim(); const probe = document.createElement("i"); probe.style.color = ok; document.body.append(probe); const want = getComputedStyle(probe).color; probe.remove(); return [getComputedStyle(e).borderColor, want]; });

  // The device starts with the profile's layers: one "Theme: ..." chip plus one chip per effect.
  assert.ok((await layers()).length > 0, "starts with scene layers");
  assert.equal(await t.page.$$eval("[data-now-playing] [data-chip-theme]", (e) => e.length), 1, "one theme chip");
  assert.equal(await t.page.$("[data-themes]") !== null && await t.page.$("[data-effects]") !== null, true, "two sections");

  // Remove everything with the chips' X, then start clean.
  while ((await layers()).length) {
    await t.page.click("[data-now-playing] .lchip .x");
    await sleep(150);
  }

  // A theme card, then an effect card: the effect stacks on top of the theme.
  await t.clickText("[data-theme-list] [data-anim] button.face", "Rainbow");
  await t.clickText("[data-effects] [data-anim] button.face", "Glitch");
  await sleep(300);
  assert.deepEqual((await layers()).map((l) => l.role), ["theme", "effect"]);
  assert.deepEqual(await chips("[data-chip-theme]"), ["Theme: Rainbow"]);
  assert.deepEqual(await chips("[data-chip-effect]"), ["Effect: Glitch"]);
  const [bc, want] = await green('[data-anim="builtin:rainbow"]');
  assert.equal(bc, want, "active theme card border uses --ok");
  assert.ok(await t.page.$('[data-anim="builtin:rainbow"] .badge'), "check badge");

  // Swapping the theme keeps the effect (and puts the new look underneath it).
  await t.clickText("[data-theme-list] [data-anim] button.face", "Breathing");
  await sleep(300);
  assert.deepEqual((await layers()).map((l) => `${l.role}:${l.params.anim_id}`), ["theme:builtin:breathing", "effect:builtin:glitch"]);
  assert.equal(await t.page.$eval('[data-anim="builtin:rainbow"]', (e) => e.classList.contains("active")), false);
  assert.equal(await t.page.$eval('[data-anim="builtin:glitch"]', (e) => e.classList.contains("active")), true, "the effect stays on");

  // The Cyberpunk theme lays down the device's real layers (keyboard: background + scan + WASD) without touching effects.
  await t.clickText("[data-theme-list] [data-anim] button.face", "Cyberpunk");
  await sleep(400);
  const ls = await layers();
  assert.deepEqual(ls.map((l) => l.role), ["theme", "theme", "theme", "effect"]);
  assert.deepEqual(ls.slice(0, 3).map((l) => l.effect), ["solid", "color_wave", "solid"]);
  assert.deepEqual(await chips("[data-chip-theme]"), ["Theme: Cyberpunk"], "several layers read as one theme");
  assert.equal(await t.page.$eval('[data-anim="builtin:cyberpunk"]', (e) => e.classList.contains("active")), true);

  // A solid colour is a theme too.
  await t.setColor("#00ff88", ".solid");
  await t.clickText(".solid button", "Use this colour");
  await sleep(300);
  assert.deepEqual((await layers()).map((l) => l.effect), ["solid", "animation"]);
  assert.deepEqual(await chips("[data-chip-effect]"), ["Effect: Glitch"]);

  // Plain engine effects: ripple is an effect (toggle), spectrum a theme.
  await t.page.click('[data-effects] [data-fx="ripple"]');
  await sleep(200);
  assert.equal(await t.page.$eval('[data-fx="ripple"]', (e) => e.getAttribute("aria-pressed")), "true");
  assert.equal((await layers()).at(-1).role, "effect");
  await t.page.click('[data-themes] [data-fx="spectrum"]');
  await sleep(200);
  assert.deepEqual((await layers()).map((l) => l.effect), ["spectrum", "animation", "ripple"], "theme swapped underneath, both effects kept");
  await t.page.click('[data-effects] [data-fx="ripple"]');
  await sleep(200);
  assert.equal(await t.page.$eval('[data-fx="ripple"]', (e) => e.getAttribute("aria-pressed")), "false");

  // Toggling an effect off and chip removal.
  await t.page.click('[data-effects] [data-anim="builtin:glitch"] button.face');
  await sleep(300);
  assert.deepEqual((await layers()).map((l) => l.effect), ["spectrum"]);
  await t.page.click("[data-now-playing] [data-chip-theme] .x");
  await sleep(300);
  assert.equal((await layers()).length, 0);
}
