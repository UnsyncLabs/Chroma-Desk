/** Quick + Draw on non-keyboard devices (LED strip, RAM, GPU, mouse): thumbnails, apply, editors open. */
import { assert, sleep } from "../lib.mjs";

const DEVICES = [
  ["Corsair", "Corsair Lighting Node Core@1", "strip"],
  ["ENE DRAM", null, "ram"],
  ["GeForce", null, "gpu"],
  ["G502", "G502 HERO Gaming Mouse@4", "mouse"],
];

export default async function (t, url) {
  await t.load(url);
  for (const [re, , tag] of DEVICES) {
    await t.openDevice(re);
    await t.page.waitForSelector("[data-anim]");
    await sleep(400);
    const cards = await t.page.$$eval("[data-anim]", (els) => els.map((e) => e.getAttribute("data-anim")));
    assert.ok(cards.includes("builtin:rainbow"), `${tag}: presets listed`);
    // Every thumbnail has some lit pixels (not a blank black card).
    const lit = await t.page.$$eval("[data-anim] canvas", (cs) => cs.map((c) => { const d = c.getContext("2d").getImageData(0, 0, c.width, c.height).data; let n = 0; for (let i = 0; i < d.length; i += 4) if (d[i] + d[i + 1] + d[i + 2] > 40) n++; return n; }));
    assert.ok(lit.filter((n) => n > 0).length >= Math.min(3, lit.length), `${tag}: thumbnails show something (${lit})`);
    await t.shot(`dev-${tag}-quick`, 1200, 800);
    await t.shot(`dev-${tag}-quick`, 400, 400);
    await t.page.click('[data-anim="builtin:rainbow"] button.only');
    await sleep(300);
    const sc = await t.scene();
    const ds = Object.entries(sc.devices).find(([k]) => k.toLowerCase().includes(re.split(" ").pop().toLowerCase()))?.[1];
    assert.ok(ds, `${tag}: device scene`);
    assert.equal(ds.mode, "effects");
    assert.equal(ds.layers[0].params.anim_id, "builtin:rainbow");
    // Draw: each editor opens without errors.
    await t.level("Draw");
    for (const k of ["Frames", "Path", "Palette", "Paint"]) {
      await t.drawKind(k);
      assert.ok(await t.page.$("svg.canvas"), `${tag}: ${k} canvas`);
    }
    await t.drawKind("Frames");
    await t.shot(`dev-${tag}-frames`, 1200, 800);
    await t.shot(`dev-${tag}-frames`, 400, 400);
    await t.level("Quick");
  }
}
