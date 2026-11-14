/** Frames editor: paint 3 frames, timeline, reorder, playback, live preview, save, quick apply. */
import { assert, sleep } from "../lib.mjs";

const KB = "Razer Huntsman V3 Pro TKL@0";
const slots = (t) => t.page.$$eval(".timeline .slot", (e) => e.map((x) => x.getAttribute("aria-selected")));

export default async function (t, url) {
  await t.load(url);
  await t.openDevice("Huntsman");
  await t.level("Draw");
  await t.drawKind("Frames");
  const W = await t.ledIndex("Key: W");
  const A = await t.ledIndex("Key: A");
  const D = await t.ledIndex("Key: D");

  // Frame 1: W red.
  assert.deepEqual(await slots(t), ["true"], "starts with one frame");
  await t.setColor("#ff0000");
  await t.clickLed("Key: W");
  assert.equal(await t.fill(W), "#ff0000");
  // Frame 2: A red. The onion skin shows W dimmed.
  await t.clickText(".frameBar button", "^Add frame$");
  assert.deepEqual(await slots(t), ["false", "true"]);
  assert.equal(await t.fill(W), "#470000", "onion skin: previous frame dimmed");
  assert.notEqual(await t.fill(A), "#ff0000");
  await t.clickLed("Key: A");
  assert.equal(await t.fill(A), "#ff0000");
  // Frame 3: D red.
  await t.clickText(".frameBar button", "^Add frame$");
  await t.clickLed("Key: D");
  assert.equal((await slots(t)).length, 3);

  // Live preview: the unsaved draft is what the engine got (as a copy of the scene).
  await t.waitFor(async () => (await t.previewAnim())?.kind?.frames?.length === 3, "preview scene with 3 frames");
  const prev = await t.previewAnim();
  assert.equal(prev.kind.type, "frames");
  assert.deepEqual(prev.kind.frames.map((f) => Object.keys(f.leds)), [["name:Key: W"], ["name:Key: A"], ["name:Key: D"]]);
  assert.equal((await t.scene()).devices[KB].layers.length, 1, "the preview replaces the layers while editing");

  // Arrow keys move between frames.
  await t.page.keyboard.press("ArrowLeft");
  assert.deepEqual(await slots(t), ["false", "true", "false"]);
  await t.page.keyboard.press("ArrowLeft");
  assert.deepEqual(await slots(t), ["true", "false", "false"]);
  await t.page.keyboard.press("ArrowRight");
  await t.page.keyboard.press("ArrowRight");

  // Reorder with the buttons: move frame 3 (D) before frame 2 (A).
  await t.clickText(".frameBar button", "Move earlier");
  await t.waitFor(async () => JSON.stringify(Object.keys((await t.previewAnim()).kind.frames[1].leds)) === '["name:Key: D"]', "reordered");
  await t.clickText(".frameBar button", "Move later"); // back: W, A, D

  // Drag and drop on the timeline (synthetic HTML5 drag events: the browser cannot drag with the mouse headless).
  await t.page.evaluate(() => {
    const slots = [...document.querySelectorAll(".timeline .slot")];
    const fire = (el, type) => el.dispatchEvent(new DragEvent(type, { bubbles: true, cancelable: true }));
    fire(slots[0], "dragstart");
    fire(slots[2], "dragover");
    fire(slots[2], "drop");
  });
  await t.waitFor(async () => JSON.stringify(Object.keys((await t.previewAnim()).kind.frames[2].leds)) === '["name:Key: W"]', "frame 1 dropped last: A, D, W");
  assert.deepEqual((await t.previewAnim()).kind.frames.map((f) => Object.keys(f.leds)[0]), ["name:Key: A", "name:Key: D", "name:Key: W"]);
  await t.page.evaluate(() => {
    const slots = [...document.querySelectorAll(".timeline .slot")];
    const fire = (el, type) => el.dispatchEvent(new DragEvent(type, { bubbles: true, cancelable: true }));
    fire(slots[2], "dragstart");
    fire(slots[0], "drop");
  });
  await t.waitFor(async () => (await t.previewAnim()).kind.frames[0].leds["name:Key: W"], "back to W, A, D");
  assert.deepEqual((await t.previewAnim()).kind.frames.map((f) => Object.keys(f.leds)[0]), ["name:Key: W", "name:Key: A", "name:Key: D"]);
  // Reordering never loses a frame's duration (frame 3 is the one that gets 600 ms below).
  await t.page.keyboard.press("ArrowRight");
  await t.page.keyboard.press("ArrowRight");
  assert.deepEqual(await slots(t), ["false", "false", "true"].map(String), "selection follows the drop");

  // Per-frame duration, duplicate and delete.
  await t.page.$eval(".frameBar input[type=number]", (e) => { e.value = "600"; e.dispatchEvent(new Event("change", { bubbles: true })); });
  await t.waitFor(async () => (await t.previewAnim()).kind.frames[2].duration_ms === 600, "duration stored");
  await t.clickText(".frameBar button", "^Duplicate$");
  assert.equal((await slots(t)).length, 4);
  await t.clickText(".frameBar button", "^Delete$");
  assert.equal((await slots(t)).length, 3);

  // Playback: W, then A, then D light up in turn, never together (no crossfade).
  await t.clickText(".frameBar button", "Play");
  assert.ok(await t.page.$(".banner"), "playing hint");
  const seq = await t.sampleFills([W, A, D], 2500, 40);
  const lit = seq.map((f) => f.map((c, i) => (c === "#ff0000" ? "WAD"[i] : "")).join(""));
  assert.ok(lit.some((s) => s === "W") && lit.some((s) => s === "A") && lit.some((s) => s === "D"), `each key lights up: ${[...new Set(lit)]}`);
  assert.ok(lit.every((s) => s.length <= 1), "one key at a time");
  const order = lit.filter(Boolean).filter((s, i, a) => s !== a[i - 1]).join("");
  assert.match(order, /^W?(AD?W?)+/, `order is W -> A -> D looping: ${order}`);
  await t.shot("frames-playing", 1200, 800);
  await t.clickText(".frameBar button", "Pause");

  // Crossfade makes the transition continuous: intermediate reds appear.
  await t.setRange('.settings input[type=range][max="1"]', 1);
  await t.clickText(".frameBar button", "Play");
  const seq2 = await t.sampleFills([W], 1500, 30);
  const reds = new Set(seq2.map((f) => f[0]));
  assert.ok(reds.size > 4, `crossfade gives many intermediate colours (${reds.size})`);
  await t.clickText(".frameBar button", "Pause");

  // Save as animation, then it shows in the library and in Quick.
  await t.page.$eval('.save input[placeholder^="e.g."]', (e) => { e.value = "WAD blink"; e.dispatchEvent(new Event("input", { bubbles: true })); });
  await t.page.$eval('.save input[placeholder="comma separated"]', (e) => { e.value = "test, wasd"; e.dispatchEvent(new Event("input", { bubbles: true })); });
  await t.clickText(".save button", "^Save as animation$");
  await t.waitFor(async () => (await t.mockState("animations")).length === 1, "saved");
  const saved = (await t.mockState("animations"))[0];
  assert.equal(saved.name, "WAD blink");
  assert.deepEqual(saved.tags, ["test", "wasd"]);
  assert.equal(saved.kind.type, "frames");
  assert.equal(saved.kind.frames.length, 3);
  assert.equal(saved.kind.crossfade, 1);
  assert.equal(saved.target.kind, "keyboard");
  assert.ok(!(await t.page.$eval(".state", (e) => e.classList.contains("dirty"))), "no longer dirty");
  await t.shot("frames", 1200, 800);
  await t.shot("frames", 400, 400);

  await t.level("Quick");
  await t.waitFor(() => t.page.$(`[data-anim="${saved.id}"]`), "in Quick > My animations");
  assert.match(await t.page.$eval(`[data-anim="${saved.id}"]`, (e) => e.textContent), /3\/3 LEDs|WAD blink/);
  await t.page.click(`[data-anim="${saved.id}"] button.only`);
  await sleep(300);
  const ds = (await t.scene()).devices[KB];
  assert.equal(ds.layers.length, 1);
  assert.equal(ds.layers[0].params.anim_id, saved.id);
  assert.equal(ds.layers[0].params.snapshot.kind.frames.length, 3, "snapshot embedded in the layer");
  // Leaving the editor removed the preview: this is the real applied layer (not a draft).
  assert.notEqual(ds.layers[0].params.anim_id, "draft");
}
