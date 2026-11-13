/** Path editor: draw a route by dragging, a gradient travels along it in stroke order. */
import { assert, sleep } from "../lib.mjs";

const KB = "Razer Huntsman V3 Pro TKL@0";
const ROUTE = ["Q", "W", "E", "R", "T", "G", "F", "D", "S", "Z", "X", "C", "V"];

export default async function (t, url) {
  await t.load(url);
  await t.openDevice("Huntsman");
  await t.level("Draw");
  await t.drawKind("Path");
  const idx = {};
  for (const k of ROUTE) idx[k] = await t.ledIndex(`Key: ${k}`);

  // The Path tool is active and drag adds the LEDs in order (an "S" across the keyboard).
  assert.deepEqual(await t.page.$$eval(".tools button.on", (e) => e.map((x) => x.textContent.trim())), ["Path"]);
  await t.dragLeds(ROUTE.map((k) => `Key: ${k}`));
  assert.deepEqual(await t.page.$$eval(".pathLayer .num", (e) => e.map((x) => x.textContent)), ROUTE.map((_, i) => String(i + 1)), "numbered in stroke order");
  assert.match(await t.page.$eval(".count", (e) => e.textContent), /13 LEDs/);
  await t.waitFor(async () => (await t.previewAnim())?.kind?.path?.length === 13, "live preview has the path");
  const prev = await t.previewAnim();
  assert.deepEqual(prev.kind.path, ROUTE.map((k) => `name:Key: ${k}`), "stored by key name, in stroke order");

  // Passing over a LED twice does not duplicate it; undo last point and clear work.
  await t.dragLeds(["Key: C", "Key: V"]);
  assert.match(await t.page.$eval(".count", (e) => e.textContent), /13 LEDs/);
  await t.clickText(".bar button", "Undo last point");
  assert.match(await t.page.$eval(".count", (e) => e.textContent), /12 LEDs/);
  await t.dragLeds(["Key: V"]);
  assert.match(await t.page.$eval(".count", (e) => e.textContent), /13 LEDs/);

  // A gradient travels along the route: the lit head advances in stroke order.
  // One short segment at a time (period > path length) makes the head easy to follow.
  await t.setSlider("Length", 0.25);
  await t.setSlider("Gap", 1);
  await t.setSlider("Speed", 0.4);
  await sleep(200);
  const leds = ROUTE.map((k) => idx[k]);
  const track = async (ms) => {
    const samples = await t.sampleFills(leds, ms, 35);
    return samples.map((f) => {
      let best = -1;
      let bright = 20;
      f.forEach((c, i) => {
        const v = parseInt(c.slice(1, 3), 16) + parseInt(c.slice(3, 5), 16) + parseInt(c.slice(5, 7), 16);
        if (v > bright) [best, bright] = [i, v];
      });
      return best;
    });
  };
  const fwd = (await track(2200)).filter((i) => i >= 0);
  assert.ok(new Set(fwd).size >= 4, `the segment moves over several LEDs: ${[...new Set(fwd)]}`);
  const steps = fwd.slice(1).map((v, i) => (v - fwd[i] + 13) % 13);
  assert.ok(steps.every((s) => s <= 4), `advances forwards along the route: ${steps}`);
  assert.ok(steps.filter((s) => s > 0).length >= 3);
  await t.shot("path", 1200, 800);
  await t.shot("path", 400, 400);

  // Reverse flips the direction.
  await t.clickText(".sliders .row", "Reverse");
  await sleep(200);
  const back = (await track(2200)).filter((i) => i >= 0);
  const bsteps = back.slice(1).map((v, i) => (fwd.length ? (v - back[i] + 13) % 13 : 0));
  assert.ok(bsteps.filter((s) => s >= 9).length >= 3, `moves backwards: ${bsteps}`);
  assert.equal((await t.previewAnim()).kind.reverse, true);

  // Palette editor: adding a stop by clicking the bar and picking a preset.
  const before = (await t.previewAnim()).kind.palette.length;
  const box = await (await t.page.$(".pal .bar")).boundingBox();
  await t.page.mouse.click(box.x + box.width * 0.7, box.y + box.height / 2);
  await t.waitFor(async () => (await t.previewAnim()).kind.palette.length === before + 1, "stop added");
  await t.page.select('.pal select[aria-label="Presets"]', "ocean");
  await t.waitFor(async () => (await t.previewAnim()).kind.palette.length === 4, "preset applied");
  // Drag the first stop to the right.
  const stop = await (await t.page.$(".pal .stop")).boundingBox();
  await t.page.mouse.move(stop.x + stop.width / 2, stop.y + stop.height / 2);
  await t.page.mouse.down();
  await t.page.mouse.move(stop.x + 60, stop.y + stop.height / 2, { steps: 5 });
  await t.page.mouse.up();
  await sleep(200);
  assert.ok((await t.previewAnim()).kind.palette[0].pos > 0.02 || (await t.previewAnim()).kind.palette.some((s) => s.pos > 0.02 && s.pos < 0.3), "stop dragged");

  // Save.
  await t.page.$eval('.save input[placeholder^="e.g."]', (e) => { e.value = "My S"; e.dispatchEvent(new Event("input", { bubbles: true })); });
  await t.clickText(".save button", "^Save as animation$");
  await t.waitFor(async () => (await t.mockState("animations")).length === 1, "saved");
  const saved = (await t.mockState("animations"))[0];
  assert.equal(saved.kind.type, "path");
  assert.deepEqual(saved.kind.path, ROUTE.map((k) => `name:Key: ${k}`));

  // Clear path empties it and disables saving.
  await t.clickText(".bar button", "Clear path");
  assert.match(await t.page.$eval(".count", (e) => e.textContent), /^0 LEDs/);
  assert.equal(await t.page.$eval(".save button.primary, .save button[type=submit]", (e) => e.disabled), true);
}
