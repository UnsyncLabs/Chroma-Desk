/** An animation made for another keyboard: partial compatibility is shown and missing LEDs are ignored. */
import { assert, sleep } from "../lib.mjs";

export default async function (t, url) {
  await t.load(url);
  // A file from "another keyboard": 2 keys exist here (W, A), 2 do not, 1 index ref is out of range.
  await t.page.evaluate(() => {
    globalThis.__chromaMock.files["/tmp/foreign.json"] = {
      schema_version: 1, id: "x", name: "Foreign glow", tags: ["shared"], created: 1, updated: 1, builtin: false,
      target: { kind: "keyboard", hint: "Some Other Keyboard" },
      kind: { type: "frames", frame_ms: 300, loop: "loop", crossfade: 0, frames: [
        { leds: { "name:Key: W": [255, 0, 0], "name:Key: A": [255, 0, 0], "name:Key: Numpad 5": [255, 0, 0] } },
        { leds: { "name:Key: Numpad Enter": [0, 255, 0], "idx:900": [0, 0, 255] } },
      ] },
    };
    globalThis.__chromaMock.nextOpen = "/tmp/foreign.json";
  });
  await t.nav("^Animations");
  await t.clickText(".headBtns button", "^Import");
  await t.waitFor(async () => (await t.page.$$("[data-anim]")).length === 10, "imported");
  const id = await t.page.$$eval("[data-anim]", (els) => els.map((e) => e.getAttribute("data-anim")).find((i) => i.startsWith("u-")));
  const chip = await t.page.$eval(`[data-anim="${id}"] .chip.warn`, (e) => e.textContent);
  assert.equal(chip, "2/5 LEDs", "compatibility chip: 2 of 5 referenced LEDs exist here");
  assert.ok(await t.page.$(`[data-anim="${id}"] .chip.warn[title]`), "chip explains itself");

  // The Compatible filter hides it (2/5 < 50 %); effects stay.
  await t.clickText(".pill", "^Compatible");
  const shown = await t.page.$$eval("[data-anim]", (els) => els.map((e) => e.getAttribute("data-anim")));
  assert.ok(!shown.includes(id), "incompatible animations are filtered out");
  assert.ok(shown.includes("builtin:rainbow"));
  await t.clickText(".pill", "^All");

  // Applying it still works: the snapshot is the foreign animation; unknown LEDs are ignored by the engine.
  await t.clickText(`[data-anim="${id}"] .actions button`, "^Apply$");
  await sleep(500);
  const ds = (await t.scene()).devices["Razer Huntsman V3 Pro TKL@0"];
  assert.equal(ds.layers[0].params.snapshot.name, "Foreign glow");
  // Editing tells the user what was left out.
  await t.clickText(`[data-anim="${id}"] .actions button`, "^Edit$");
  await t.waitFor(() => t.page.$(".timeline"), "editor");
  await t.waitFor(async () => /3 LED\(s\).*do not exist/.test(await t.page.evaluate(() => document.body.innerText)), "lost-LED notice");
  assert.equal(await t.fill(await t.ledIndex("Key: W")), "#ff0000");
}
