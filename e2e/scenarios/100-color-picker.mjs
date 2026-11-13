/** ColorPicker: link a colour to a palette slot, paint with it, and watch it follow the theme. */
import { assert, sleep } from "../lib.mjs";

const hexOf = (rgb) => "#" + rgb.map((v) => v.toString(16).padStart(2, "0")).join("");

export default async function (t, url) {
  await t.load(url);
  await t.openDevice("Huntsman");
  await t.gotoPaint();

  // The trigger shows a fixed colour as hex first.
  assert.match(await t.page.$eval(".toolbar .trigger", (e) => e.textContent), /#/);

  // Link the brush to @alert with real clicks (Palette tab, then the slot).
  await t.pickSlot("alert");
  assert.match(await t.page.$eval(".toolbar .trigger", (e) => e.textContent), /@alert/, "linked colours carry an @slot chip");
  await t.shot("picker-linked", 1200, 800);

  await t.clickLed("Key: Q");
  const q = await t.ledIndex("Key: Q");
  const sc = await t.scene();
  const paint = sc.devices["Razer Huntsman V3 Pro TKL@0"].layers.findLast((l) => l.effect === "paint");
  assert.deepEqual(paint.params.leds[q], { slot: "alert" }, "the paint layer stores the slot, not a colour");
  const palette = await t.page.evaluate(() => globalThis.__chromaMock.fns && 1);
  assert.ok(palette, "mock controls available");
  await t.waitFor(async () => (await t.fill(q)) === "#ff3b3b", "Q shows the default alert colour");

  // The theme changes (a game sends a palette): the painted key follows without repainting.
  await t.page.evaluate(() => globalThis.__chromaMock.fns.setPalette({ alert: [0, 255, 0] }));
  await t.waitFor(async () => (await t.fill(q)) === "#00ff00", "Q follows the new alert colour");
  // The picker's swatch shows the new theme colour too.
  await t.clickText(".toolbar .trigger", ".*");
  await t.clickText(".toolbar .pop [role=tab]", "^Palette$");
  const bg = await t.page.$eval('.toolbar .pop button[data-slot="alert"] .sw', (e) => getComputedStyle(e).backgroundColor);
  assert.equal(bg, "rgb(0, 255, 0)", "swatch updated");
  await t.shot("picker-open", 1200, 800);
  await t.shot("picker-open", 400, 400);
  await t.page.keyboard.press("Escape");

  // Custom colours stay fixed.
  await t.setColor("#0000ff");
  assert.match(await t.page.$eval(".toolbar .trigger", (e) => e.getAttribute("title")), /#0000ff/);
  await t.clickLed("Key: W");
  const w = await t.ledIndex("Key: W");
  const sc2 = await t.scene();
  const paint2 = sc2.devices["Razer Huntsman V3 Pro TKL@0"].layers.findLast((l) => l.effect === "paint");
  assert.deepEqual(paint2.params.leds[w], [0, 0, 255]);
  await t.page.evaluate(() => globalThis.__chromaMock.fns.setPalette({ accent: [1, 2, 3] }));
  await sleep(300);
  assert.equal(await t.fill(w), "#0000ff", "a custom colour does not follow the theme");
}
