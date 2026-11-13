/** Animation library page: search/filter, apply, test, save from paint, rename, duplicate, export/import, delete. */
import { assert, sleep } from "../lib.mjs";

const ids = (t) => t.page.$$eval("[data-anim]", (els) => els.map((e) => e.getAttribute("data-anim")));
const card = (id) => `[data-anim="${id}"]`;

export default async function (t, url) {
  await t.load(url);
  await t.nav("^Animations");
  await t.waitFor(async () => (await ids(t)).length >= 9, "9 presets");
  await t.shot("library-empty", 1200, 800);
  await t.shot("library-empty", 400, 400);
  assert.ok(await t.page.$(".empty"), "friendly empty state for 'mine'");

  // Search and filters.
  await t.page.type(".search", "snake");
  await sleep(200);
  assert.deepEqual(await ids(t), ["builtin:snake"]);
  await t.page.$eval(".search", (e) => { e.value = ""; e.dispatchEvent(new Event("input", { bubbles: true })); });
  await t.clickText(".pill", "^Mine");
  assert.deepEqual(await ids(t), []);
  assert.ok(await t.page.$(".none"), "no-matches message");
  await t.clickText(".pill", "^Built-in");
  assert.equal((await ids(t)).length, 9);
  await t.clickText(".pill", "^All");

  // Apply to the selected preview device (the keyboard).
  await t.clickText(`${card("builtin:heartbeat")} .actions button`, "^Apply$");
  await sleep(300);
  let ds = (await t.scene()).devices["Razer Huntsman V3 Pro TKL@0"];
  assert.equal(ds.layers[0].params.anim_id, "builtin:heartbeat");

  // Apply to all devices (menu + confirmation dialog).
  await t.clickText(`${card("builtin:wave")} summary`, "More actions");
  await t.clickText(`${card("builtin:wave")} .items button`, "Apply to all devices");
  await t.clickText("dialog button", "^Apply to all devices$");
  await sleep(400);
  const sc = await t.scene();
  assert.equal(Object.keys(sc.devices).length, 5, "every device now has a scene");
  for (const d of Object.values(sc.devices)) assert.equal(d.layers[0].params.anim_id, "builtin:wave");

  // Test = trigger overlay.
  await t.clickText(`${card("builtin:alert")} .actions button`, "^Test$");
  await sleep(300);
  const trig = await t.mockState("triggers");
  assert.equal(trig.length, 1);
  assert.equal(trig[0].animation, "builtin:alert");

  // Customize a preset: duplicates it and opens the editor on the device.
  await t.clickText(`${card("builtin:snake")} .actions button`, "^Customize$");
  await t.waitFor(() => t.page.$(".level.on"), "device opened");
  assert.match(await t.page.$eval(".level.on", (e) => e.textContent), /^Draw/);
  const lib = await t.page.evaluate(() => globalThis.__chromaMock.animations.map((a) => a.name));
  assert.deepEqual(lib, ["Snake (copy)"], "a user copy was created");
  await t.nav("^Animations");

  // Rename the copy.
  await t.waitFor(async () => (await ids(t)).length === 10, "10 cards");
  const copyId = (await ids(t)).find((i) => i.startsWith("u-"));
  await t.clickText(`${card(copyId)} summary`, "More actions");
  await t.clickText(`${card(copyId)} .items button`, "^Rename$");
  await t.page.$eval(`${card(copyId)} .rename input`, (e) => { e.value = "Green worm"; e.dispatchEvent(new Event("input", { bubbles: true })); });
  await t.clickText(`${card(copyId)} .rename button`, "^OK$");
  await t.waitFor(async () => (await t.page.$eval(`${card(copyId)} .name`, (e) => e.textContent)) === "Green worm", "renamed");
  // Name clash is refused with a translated error toast.
  await t.clickText(`${card(copyId)} summary`, "More actions");
  await t.clickText(`${card(copyId)} .items button`, "^Rename$");
  await t.page.$eval(`${card(copyId)} .rename input`, (e) => { e.value = "rainbow"; e.dispatchEvent(new Event("input", { bubbles: true })); });
  await t.clickText(`${card(copyId)} .rename button`, "^OK$");
  await t.waitFor(() => t.page.$(".toast, [role=status].error, .toasts *"), "toast");
  assert.match(await t.page.evaluate(() => document.body.innerText), /already exists/);
  await t.clickText(`${card(copyId)} .rename button`, "^Cancel$");

  // Duplicate.
  await t.clickText(`${card(copyId)} summary`, "More actions");
  await t.clickText(`${card(copyId)} .items button`, "^Duplicate$");
  await t.waitFor(async () => (await ids(t)).length === 11, "duplicate added");

  // Export + import into a fresh name.
  await t.page.evaluate(() => (globalThis.__chromaMock.nextSave = "/tmp/worm.json"));
  await t.clickText(`${card(copyId)} summary`, "More actions");
  await t.clickText(`${card(copyId)} .items button`, "^Export");
  await sleep(200);
  assert.ok((await t.mockState("files"))["/tmp/worm.json"], "file exported");
  await t.page.evaluate(() => (globalThis.__chromaMock.nextOpen = "/tmp/worm.json"));
  await t.clickText(".headBtns button", "^Import");
  await t.waitFor(async () => (await ids(t)).length === 12, "imported");
  const names = await t.page.$$eval("[data-anim] .name", (e) => e.map((x) => x.textContent));
  assert.ok(names.includes("Green worm (2)"), `imported with suffix: ${names}`);

  // Delete with confirmation (cancel first, then confirm).
  await t.clickText(`${card(copyId)} summary`, "More actions");
  await t.clickText(`${card(copyId)} .items button`, "^Delete$");
  await t.clickText("dialog button", "^Cancel$");
  assert.equal((await ids(t)).length, 12, "cancel keeps it");
  await t.clickText(`${card(copyId)} summary`, "More actions");
  await t.clickText(`${card(copyId)} .items button`, "^Delete$");
  await t.clickText("dialog button", "^Delete$");
  await t.waitFor(async () => (await ids(t)).length === 11, "deleted");
  await t.shot("library", 1200, 800);
  await t.shot("library", 400, 400);
}
