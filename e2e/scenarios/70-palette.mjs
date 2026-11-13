/** Palette editor: shape a region with the gradient tool; colours rotate inside it. */
import { assert, sleep } from "../lib.mjs";

const KB = "Razer Huntsman V3 Pro TKL@0";

export default async function (t, url) {
  await t.load(url);
  await t.openDevice("Huntsman");
  await t.level("Draw");
  await t.drawKind("Palette");
  const idx = {};
  for (const k of ["W", "A", "S", "D", "Q"]) idx[k] = await t.ledIndex(`Key: ${k}`);

  assert.deepEqual(await t.page.$$eval(".tools button.on", (e) => e.map((x) => x.textContent.trim())), ["Gradient"]);
  // Select WASD, then drag A -> D with the gradient tool.
  await t.tool("^Select");
  await t.clickText(".groups button", "^WASD$");
  await t.tool("^Gradient");
  await t.dragLeds(["Key: A", "Key: D"]);
  await t.waitFor(async () => Object.keys((await t.previewAnim())?.kind?.cells ?? {}).length === 4, "4 cells in the live preview");
  let cells = (await t.previewAnim()).kind.cells;
  assert.equal(cells["name:Key: A"], 0);
  assert.equal(cells["name:Key: D"], 1);
  assert.ok(cells["name:Key: S"] > 0 && cells["name:Key: S"] < 1, `S between A and D: ${cells["name:Key: S"]}`);
  assert.ok(!("name:Key: Q" in cells), "only the selection got cells");
  assert.match(await t.page.$eval(".count", (e) => e.textContent), /^4 LEDs/);

  // The shape stays put while its colours rotate.
  const seq = await t.sampleFills([idx.A, idx.S, idx.D, idx.Q], 1500, 60);
  const lit = (c) => c !== "#1b1b24" && c !== "#000000";
  assert.ok(seq.every((f) => lit(f[0]) && lit(f[1]) && lit(f[2])), "WASD cells always lit");
  assert.ok(seq.every((f) => !lit(f[3])), "Q (outside the shape) stays dark");
  assert.ok(new Set(seq.map((f) => f[0])).size > 5, "A cycles through colours");
  assert.ok(seq.some((f, i) => i && f[0] !== seq[i - 1][0]), "changes over time");
  await t.shot("palette", 1200, 800);
  await t.shot("palette", 400, 400);

  // Undo / redo (own history) and the eraser.
  await t.page.keyboard.down("Control");
  await t.page.keyboard.press("z");
  await t.page.keyboard.up("Control");
  await t.waitFor(async () => Object.keys((await t.previewAnim()).kind.cells).length === 0, "undone");
  await t.page.keyboard.down("Control");
  await t.page.keyboard.down("Shift");
  await t.page.keyboard.press("z");
  await t.page.keyboard.up("Shift");
  await t.page.keyboard.up("Control");
  await t.waitFor(async () => Object.keys((await t.previewAnim()).kind.cells).length === 4, "redone");
  await t.tool("^Eraser");
  await t.clickLed("Key: W");
  await t.waitFor(async () => Object.keys((await t.previewAnim()).kind.cells).length === 3, "W erased");

  // A second gradient over everything (nothing selected): a whole-keyboard rainbow region.
  await t.tool("^Select");
  await t.clickText(".toolbar button", "Clear selection");
  await t.page.select('.pal select[aria-label="Presets"]', "rainbow");
  await t.tool("^Gradient");
  await t.dragLeds(["Key: Escape", "Key: Right Control"]);
  await t.waitFor(async () => Object.keys((await t.previewAnim()).kind.cells).length > 80, "gradient over the whole keyboard");
  cells = (await t.previewAnim()).kind.cells;
  assert.equal(cells["name:Key: Escape"], 0);
  assert.equal(cells["name:Key: Right Control"], 1);

  // Speed and direction.
  await t.setSlider("Speed", 1.5);
  await t.waitFor(async () => (await t.previewAnim()).kind.speed === 1.5, "speed");

  await t.page.$eval('.save input[placeholder^="e.g."]', (e) => { e.value = "Rainbow rotor"; e.dispatchEvent(new Event("input", { bubbles: true })); });
  await t.clickText(".save button", "^Save as animation$");
  await t.waitFor(async () => (await t.mockState("animations")).length === 1, "saved");
  const saved = (await t.mockState("animations"))[0];
  assert.equal(saved.kind.type, "palette");
  assert.ok(Object.keys(saved.kind.cells).length > 80);
}
