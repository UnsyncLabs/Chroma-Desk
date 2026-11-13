/** Paint, select, fill, eyedropper, eraser and undo with real mouse events. */
import { assert, sleep } from "../lib.mjs";

export default async function (t, url) {
  await t.load(url);

  // --- hardware-mode device (mouse): clicking an LED must work and switch to effects.
  await t.openDevice("G502");
  await t.gotoPaint();
  const leds = await t.page.$$eval("svg.canvas [data-led]", (els) => els.map((e) => e.getAttribute("data-led")));
  assert.ok(leds.length >= 1, "mouse LEDs are drawn");
  await t.setColor("#00ff00");
  await t.clickLed(Number(leds[0]));
  assert.equal(await t.fill(leds[0]), "#00ff00", "brush colours the LED on the canvas");
  const s = (await t.scene()).devices["G502 HERO Gaming Mouse@4"];
  assert.equal(s.mode, "effects", "painting switches the device to effects");
  assert.deepEqual(s.layers.find((l) => l.effect === "paint").params.leds[leds[0]], [0, 255, 0], "the Paint layer holds the stroke");

  // --- keyboard: brush, drag stroke, eraser, undo/redo.
  await t.openDevice("Huntsman");
  await t.gotoPaint();
  await t.setColor("#0000ff");
  await t.clickLed("Key: Q");
  assert.equal(await t.fill(await t.ledIndex("Key: Q")), "#0000ff");
  await t.dragLeds(["Key: A", "Key: S", "Key: D", "Key: F"]);
  for (const k of ["A", "S", "D", "F"]) assert.equal(await t.fill(await t.ledIndex(`Key: ${k}`)), "#0000ff", `drag painted ${k}`);
  // Engine side: the paint layer has the 5 keys.
  const kb = (await t.scene()).devices["Razer Huntsman V3 Pro TKL@0"];
  const paint = kb.layers.findLast((l) => l.effect === "paint");
  assert.equal(Object.keys(paint.params.leds).filter((k) => JSON.stringify(paint.params.leds[k]) === "[0,0,255]").length, 5);

  // Undo removes the whole drag stroke steps one by one; redo brings it back.
  await t.page.keyboard.down("Control");
  await t.page.keyboard.press("z");
  await t.page.keyboard.up("Control");
  await sleep(300);
  assert.notEqual(await t.fill(await t.ledIndex("Key: F")), "#0000ff", "undo reverted the last LED");
  await t.page.keyboard.down("Control");
  await t.page.keyboard.down("Shift");
  await t.page.keyboard.press("z");
  await t.page.keyboard.up("Shift");
  await t.page.keyboard.up("Control");
  await sleep(300);
  assert.equal(await t.fill(await t.ledIndex("Key: F")), "#0000ff", "redo restored it");

  // Eraser makes the painted key transparent.
  await t.tool("Eraser");
  await t.clickLed("Key: Q");
  assert.notEqual(await t.fill(await t.ledIndex("Key: Q")), "#0000ff", "eraser cleared the key");

  // --- select the WASD group and fill the selection.
  await t.tool("Select");
  await t.clickText(".groups button", "^WASD$");
  await t.setColor("#ff00ff");
  await t.clickText(".toolbar button", "Fill selection");
  await sleep(300);
  for (const k of ["W", "A", "S", "D"]) assert.equal(await t.fill(await t.ledIndex(`Key: ${k}`)), "#ff00ff", `fill selection: ${k}`);
  assert.notEqual(await t.fill(await t.ledIndex("Key: E")), "#ff00ff", "E is not in the selection");

  // --- eyedropper picks what is on screen and returns to the brush.
  await t.tool("Eyedropper");
  await t.clickLed("Key: W");
  assert.equal(await t.page.$eval('.toolbar input[type="color"]', (e) => e.value), "#ff00ff", "picked the shown colour");
  assert.deepEqual(await t.page.$$eval(".tools button.on", (e) => e.map((x) => x.textContent.trim())), ["Brush"]);

  // Eyedropper also reads the live frame colour of an unpainted key (rainbow underneath).
  await t.tool("Eyedropper");
  await t.clickLed("Key: Escape");
  const picked = await t.page.$eval('.toolbar input[type="color"]', (e) => e.value);
  assert.notEqual(picked, "#ff00ff");
  assert.match(picked, /^#[0-9a-f]{6}$/);
  assert.notEqual(picked, "#1b1b24", "it is the animated frame colour, not the empty-cell grey");

  // --- save the painting as an animation (a one-frame flipbook referring to keys by name).
  await t.page.$eval('.save input[placeholder^="e.g."]', (e) => { e.value = "My painting"; e.dispatchEvent(new Event("input", { bubbles: true })); });
  await t.clickText(".save button", "^Save as animation$");
  await t.waitFor(async () => (await t.mockState("animations")).length === 1, "painting saved");
  const saved = (await t.mockState("animations"))[0];
  assert.equal(saved.name, "My painting");
  assert.equal(saved.kind.type, "frames");
  assert.equal(saved.kind.frames.length, 1);
  assert.equal(saved.kind.frames[0].leds["name:Key: W"].join(), "255,0,255", "W was painted magenta");
  assert.ok(Object.keys(saved.kind.frames[0].leds).every((k) => k.startsWith("name:") || k.startsWith("idx:")));
}
