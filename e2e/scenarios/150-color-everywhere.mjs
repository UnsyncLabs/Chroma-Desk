/** One colour control everywhere (Quick, Paint, effect parameters, Frames, Path, Palette, notifications), thumbnails follow the theme, and the popover fits small windows. */
import { assert, sleep } from "../lib.mjs";

/** Colour inputs that are not inside the unified picker (none expected in these views). */
const strayInputs = (t) => t.page.$$eval("input[type=color]", (els) => els.filter((e) => !e.closest("[data-colorpicker]")).length);
const pickers = (t) => t.page.$$eval("[data-colorpicker]", (els) => els.length);

export default async function (t, url) {
  await t.load(url);
  await t.openDevice("Huntsman");

  // Quick: the solid colour uses the picker and can link to a slot.
  assert.equal(await strayInputs(t), 0, "Quick has no raw colour input");
  assert.ok((await pickers(t)) >= 1, "Quick has the picker");
  await t.pickSlot("accent", ".solid");
  await t.clickText(".solid button", "Use this colour");
  await sleep(300);
  const kb = "Razer Huntsman V3 Pro TKL@0";
  let ds = (await t.scene()).devices[kb];
  assert.deepEqual(ds.layers[0].params.color, { slot: "accent" }, "Quick stores the slot");

  // Quick thumbnails of themed presets follow a palette change.
  const before = await t.thumbSig("builtin:accent-breathing");
  assert.ok(before !== null, "themed preset card is there");
  await t.page.evaluate(() => globalThis.__chromaMock.fns.setPalette({ accent: [255, 0, 0], primary: [0, 255, 0] }));
  await sleep(700);
  // Compare the same animation phase: sample a few frames of each palette and look at the dominant channel.
  const dominant = () =>
    t.page.evaluate(() => {
      const c = document.querySelector('[data-anim="builtin:accent-breathing"] canvas');
      const d = c.getContext("2d").getImageData(0, 0, c.width, c.height).data;
      let r = 0, g = 0, b = 0;
      for (let i = 0; i < d.length; i += 4) { r += d[i]; g += d[i + 1]; b += d[i + 2]; }
      return { r, g, b };
    });
  let seen = null;
  for (let i = 0; i < 12; i++) {
    const v = await dominant();
    if (v.r > 0 && v.r > v.b * 1.5 && v.r > v.g * 1.5) seen = v;
    await sleep(150);
  }
  assert.ok(seen, "the Accent breathing thumbnail turns red when @accent turns red");
  await t.page.evaluate(() => globalThis.__chromaMock.fns.setPalette({ accent: [124, 92, 255], primary: [62, 166, 255] }));

  // Paint, Frames, Path, Palette editors.
  await t.level("Draw");
  for (const kind of ["Paint", "Frames", "Path", "Palette"]) {
    await t.drawKind(kind);
    assert.ok((await pickers(t)) >= 1, `${kind} has the picker`);
    assert.equal(await strayInputs(t), 0, `${kind} has no raw colour input`);
  }

  // Effect parameters (Layers): a Solid layer has a colour parameter.
  await t.level("Layers");
  await t.page.select(".panel .add select", "solid");
  await t.clickText(".panel .add button", "^Add layer$");
  await sleep(300);
  assert.ok((await pickers(t)) >= 1, "layer parameters use the picker");
  assert.equal(await strayInputs(t), 0, "no raw colour input in the layer parameters");
  await t.pickSlot("tertiary", ".panel .detail");
  ds = (await t.scene()).devices[kb];
  assert.deepEqual(ds.layers.at(-1).params.color, { slot: "tertiary" }, "effect parameter stores the slot");

  // Notifications editor.
  await t.nav("^Notifications");
  await t.clickText("[data-create]", ".*");
  await t.waitFor(() => t.page.$("[data-notif-editor]"), "editor");
  assert.ok((await pickers(t)) >= 1, "notification editor has the picker");
  assert.equal(await strayInputs(t), 0, "no raw colour input in the editor");

  // 400 px wide window: the open popover stays inside the window.
  await t.page.setViewport({ width: 400, height: 400 });
  await sleep(300);
  await t.page.$eval("[data-notif-editor] [data-colorpicker]", (e) => e.scrollIntoView({ block: "center" }));
  await t.clickText("[data-notif-editor] .trigger", ".*");
  await t.waitFor(() => t.page.$("[data-notif-editor] .pop"), "popover");
  await sleep(150);
  const r = await t.page.$eval("[data-notif-editor] .pop", (e) => {
    const b = e.getBoundingClientRect();
    return { l: b.left, r: b.right, t: b.top, b: b.bottom, w: innerWidth, h: innerHeight };
  });
  assert.ok(r.l >= 0 && r.r <= r.w, `popover horizontally inside the window: ${JSON.stringify(r)}`);
  await t.shot("picker-in-editor-small", 400, 400);
  await t.page.keyboard.press("Escape");
  await t.page.setViewport({ width: 1200, height: 800 });
  t.noErrors();
}
