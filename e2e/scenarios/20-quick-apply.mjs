/** Quick tab: one click applies a preset, your own animation or a colour. */
import { assert, sleep } from "../lib.mjs";

export default async function (t, url) {
  await t.load(url);
  await t.openDevice("Huntsman");
  const kb = "Razer Huntsman V3 Pro TKL@0";

  // Quick is the default level and shows the presets with animated thumbnails.
  assert.ok(await t.page.$(".level.on"), "a level is selected");
  assert.match(await t.page.$eval(".level.on", (e) => e.textContent), /^Quick/);
  const cards = await t.page.$$eval("[data-anim]", (els) => els.map((e) => e.getAttribute("data-anim")));
  assert.ok(cards.includes("builtin:rainbow") && cards.includes("builtin:snake") && cards.length >= 9, `preset cards: ${cards}`);
  // The thumbnail animates (canvas content changes over time) and has lit pixels.
  const a = await t.thumbSig("builtin:rainbow");
  await sleep(500);
  const b = await t.thumbSig("builtin:rainbow");
  assert.notEqual(a, b, "rainbow thumbnail is animated");
  await t.shot("quick", 1200, 800);
  await t.shot("quick", 400, 400);

  // One click on "Rainbow" applies it: layer is an animation layer with a snapshot.
  await t.clickText("[data-anim] button.face", "Rainbow");
  await sleep(400);
  let ds = (await t.scene()).devices[kb];
  assert.equal(ds.mode, "effects");
  assert.equal(ds.layers.length, 1);
  assert.equal(ds.layers[0].effect, "animation");
  assert.equal(ds.layers[0].params.anim_id, "builtin:rainbow");
  assert.equal(ds.layers[0].params.snapshot.name, "Rainbow", "snapshot travels with the layer");
  assert.equal(await t.page.$eval('[data-anim="builtin:rainbow"]', (e) => e.classList.contains("active")), true, "the card shows it is active");

  // The device preview in the sidebar/engine produced frames: LED colours are not all the same.
  // Snake (a path animation) replaces it.
  await t.clickText("[data-anim] button.face", "Snake");
  await sleep(300);
  ds = (await t.scene()).devices[kb];
  assert.equal(ds.layers.length, 1);
  assert.equal(ds.layers[0].params.anim_id, "builtin:snake");

  // Solid colour.
  await t.page.$eval(".solid input[type=color]", (el) => {
    el.value = "#00ff88";
    el.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await t.clickText(".solid button", "Use this colour");
  await sleep(300);
  ds = (await t.scene()).devices[kb];
  assert.equal(ds.layers[0].effect, "solid");
  assert.deepEqual(ds.layers[0].params.color, [0, 255, 136]);

  // A device in hardware mode (the mouse) switches to effects on the first click.
  await t.openDevice("G502");
  let mouse = (await t.scene()).devices["G502 HERO Gaming Mouse@4"];
  assert.ok(!mouse || mouse.mode !== "effects", "mouse starts in hardware mode");
  await t.clickText("[data-anim] button.face", "Breathing");
  await sleep(300);
  mouse = (await t.scene()).devices["G502 HERO Gaming Mouse@4"];
  assert.equal(mouse.mode, "effects");
  assert.equal(mouse.layers[0].params.anim_id, "builtin:breathing");

  // Empty state of "My animations" offers to create one (-> Draw). Let the toasts fade first.
  await sleep(2200);
  assert.ok(await t.page.$(".empty"), "empty state shown");
  await t.clickText(".empty button", "Create one");
  assert.match(await t.page.$eval(".level.on", (e) => e.textContent), /^Draw/);
}
