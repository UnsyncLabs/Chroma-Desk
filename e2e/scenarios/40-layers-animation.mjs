/** Layers level: an "Animation" layer type with a library picker and the update-from-library offer. */
import { assert, sleep } from "../lib.mjs";

export default async function (t, url) {
  await t.load(url);
  await t.openDevice("Huntsman");
  await t.level("Layers");
  const kb = "Razer Huntsman V3 Pro TKL@0";

  // Add an Animation layer.
  await t.page.select(".panel .add select", "animation");
  await t.clickText(".panel .add button", "^Add layer$");
  await sleep(300);
  let ds = (await t.scene()).devices[kb];
  assert.equal(ds.layers.at(-1).effect, "animation");
  // Pick "Wave" from the library select.
  await t.page.select('.panel .detail select[aria-label="Animation"]', "builtin:wave");
  await sleep(300);
  ds = (await t.scene()).devices[kb];
  assert.equal(ds.layers.at(-1).params.anim_id, "builtin:wave");
  assert.equal(ds.layers.at(-1).params.snapshot.name, "Wave");
  assert.match(await t.page.$eval(".panel li.sel .name", (e) => e.textContent), /Animation: Wave/);

  // A user animation is picked, then changed in the library: the layer keeps its copy and offers to update.
  await t.page.evaluate(() => {
    const m = globalThis.__chromaMock;
    m.animations.push({ schema_version: 1, id: "u-x", name: "Mine", tags: [], created: 1, updated: 5, builtin: false, target: { kind: "any" }, kind: { type: "effect", effect: "solid", params: { color: [1, 2, 3] } } });
  });
  await t.nav("^Animations"); // reloads the library
  await t.waitFor(() => t.page.$('[data-anim="u-x"]'), "user animation listed");
  await t.openDevice("Huntsman");
  await t.level("Layers");
  await t.clickText(".panel li .name", "Animation: Wave");
  await t.page.select('.panel .detail select[aria-label="Animation"]', "u-x");
  await sleep(300);
  ds = (await t.scene()).devices[kb];
  assert.equal(ds.layers.at(-1).params.snapshot.updated, 5);
  assert.equal(await t.page.$(".detail .note button"), null, "up to date: no update offer");
  await t.page.evaluate(() => {
    const a = globalThis.__chromaMock.animations[0];
    a.updated = 9;
    a.kind.params.color = [200, 0, 0];
  });
  await t.nav("^Animations");
  await t.waitFor(() => t.page.$('[data-anim="u-x"]'), "reloaded");
  await t.openDevice("Huntsman");
  await t.level("Layers");
  assert.equal((await t.scene()).devices[kb].layers.at(-1).params.snapshot.updated, 5, "layer still holds its own copy");
  await t.clickText(".detail .note button", "^Update$");
  await sleep(300);
  ds = (await t.scene()).devices[kb];
  assert.equal(ds.layers.at(-1).params.snapshot.updated, 9);
  assert.deepEqual(ds.layers.at(-1).params.snapshot.kind.params.color, [200, 0, 0]);
  assert.equal(await t.page.$(".detail .note button"), null, "offer gone after updating");

  // Deleting the library entry: the layer keeps playing its snapshot and says so.
  await t.page.evaluate(() => (globalThis.__chromaMock.animations = []));
  await t.nav("^Animations");
  await sleep(300);
  await t.openDevice("Huntsman");
  await t.level("Layers");
  assert.match(await t.page.$eval(".detail .note", (e) => e.textContent), /keeps playing its saved copy/);
  await t.shot("layers-animation", 1200, 800);
}
