/** Editing an existing animation, saving over it or as a copy, and the unsaved-changes guard. */
import { assert, sleep } from "../lib.mjs";

const KB = "Razer Huntsman V3 Pro TKL@0";
const slots = (t) => t.page.$$eval(".timeline .slot", (e) => e.length);
const setName = (t, v) =>
  t.page.$eval('.save input[placeholder^="e.g."]', (e, v) => {
    e.value = v;
    e.dispatchEvent(new Event("input", { bubbles: true }));
  }, v);

export default async function (t, url) {
  await t.load(url);
  const original = (await t.scene()).devices[KB].layers.map((l) => l.effect);

  // --- build and save a 2-frame animation
  await t.openDevice("Huntsman");
  await t.level("Draw");
  await t.drawKind("Frames");
  await t.setColor("#ff0000");
  await t.clickLed("Key: W");
  await t.clickText(".frameBar button", "^Add frame$");
  await t.clickLed("Key: A");
  await setName(t, "Two steps");
  await t.clickText(".save button", "^Save as animation$");
  await t.waitFor(async () => (await t.mockState("animations")).length === 1, "saved");
  const v1 = (await t.mockState("animations"))[0];

  // --- unsaved changes guard: switching the draw kind, the level, and the sidebar all ask first.
  await t.clickLed("Key: S");
  assert.match(await t.page.$eval(".state", (e) => e.textContent), /Unsaved changes/);
  await t.drawKind("Path");
  assert.ok(await t.page.$("dialog[open]"), "confirm dialog when switching kind with unsaved work");
  await t.clickText("dialog button", "^Cancel$");
  assert.equal(await slots(t), 2, "still editing the frames");
  await t.level("Quick");
  assert.ok(await t.page.$("dialog[open]"), "confirm dialog when changing level");
  await t.clickText("dialog button", "^Cancel$");
  await t.nav("^Animations");
  assert.ok(await t.page.$("dialog[open]"), "confirm dialog when leaving through the sidebar");
  assert.match(await t.page.$eval("dialog", (e) => e.textContent), /unsaved changes/i);
  await t.clickText("dialog button", "^Cancel$");
  assert.ok(await t.page.$(".timeline"), "editor still on screen");

  // --- update the saved entry (overwrite, same id)
  await t.clickText(".save button", "^Update");
  await t.waitFor(async () => (await t.mockState("animations"))[0].updated > v1.updated, "updated");
  let list = await t.mockState("animations");
  assert.equal(list.length, 1);
  assert.equal(list[0].id, v1.id);
  assert.equal(list[0].kind.frames.length, 2);
  assert.ok(Object.keys(list[0].kind.frames[1].leds).includes("name:Key: S"), "the change was saved");

  // --- leave for real: the live preview goes away and the device is back to its own layers.
  await t.nav("^Animations");
  await sleep(300);
  assert.equal(await t.page.$("dialog[open]"), null, "no prompt when everything is saved");
  assert.deepEqual((await t.scene()).devices[KB].layers.map((l) => l.effect), original, "preview removed, scene restored");

  // --- Edit from the library opens the editor with the saved frames, then "Save as new" makes a copy.
  await t.clickText(`[data-anim="${v1.id}"] .actions button`, "^Edit$");
  await t.waitFor(() => t.page.$(".timeline"), "editor opened");
  assert.equal(await slots(t), 2);
  assert.equal(await t.page.$eval('.save input[placeholder^="e.g."]', (e) => e.value), "Two steps");
  assert.equal(await t.fill(await t.ledIndex("Key: W")), "#ff0000", "frame 1 has W painted");
  await setName(t, "Two steps copy");
  await t.clickText(".save button", "^Save as new$");
  await t.waitFor(async () => (await t.mockState("animations")).length === 2, "copy saved");
  list = await t.mockState("animations");
  assert.deepEqual(list.map((a) => a.name).sort(), ["Two steps", "Two steps copy"]);

  // Saving with a taken name is refused with a clear message.
  await setName(t, "Rainbow");
  await t.clickText(".save button", "^Save as new$");
  await t.waitFor(async () => /already exists/.test(await t.page.evaluate(() => document.body.innerText)), "name clash message");

  // Apply (without saving) puts the draft on the device as its layer, and it stays after leaving.
  await t.clickText(".bar button", "^Apply to this device$");
  await sleep(200);
  await t.level("Quick");
  await sleep(300);
  const ds = (await t.scene()).devices[KB];
  assert.equal(ds.layers.length, 1);
  assert.equal(ds.layers[0].effect, "animation");
  assert.notEqual(ds.layers[0].params.anim_id, "draft");
}
