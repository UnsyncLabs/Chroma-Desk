/** Quick tab: cards and effects toggle as layers, running layers are chips, active state is green. */
import { assert, sleep } from "../lib.mjs";

export default async function (t, url) {
  await t.load(url);
  await t.openDevice("Huntsman");
  const kb = "Razer Huntsman V3 Pro TKL@0";
  const layers = async () => (await t.scene()).devices[kb]?.layers ?? [];
  const green = (sel) => t.page.$eval(sel, (e) => { const ok = getComputedStyle(document.documentElement).getPropertyValue("--ok").trim(); const probe = document.createElement("i"); probe.style.color = ok; document.body.append(probe); const want = getComputedStyle(probe).color; probe.remove(); return [getComputedStyle(e).borderColor, want]; });

  // The device starts with the profile's layers, shown as chips; remove them all with the chips' X.
  assert.ok((await layers()).length > 0, "starts with scene layers");
  assert.equal(await t.page.$$eval("[data-now-playing] .lchip .x", (e) => e.length), (await layers()).length, "chips match the layers");
  while ((await layers()).length) {
    await t.page.click("[data-now-playing] .lchip .x");
    await sleep(150);
  }

  // Add a card, an effect and a second card: three layers.
  await t.clickText("[data-anim] button.face", "Rainbow");
  await t.page.click('[data-fx="glitch"]');
  await t.clickText("[data-anim] button.face", "Breathing");
  await sleep(300);
  assert.deepEqual((await layers()).map((l) => l.effect), ["animation", "glitch", "animation"]);
  assert.equal(await t.page.$$eval("[data-now-playing] .lchip .x", (e) => e.length), 3, "one chip per layer");

  // Active state is green (the --ok token), not the accent.
  const [bc, want] = await green('[data-anim="builtin:rainbow"]');
  assert.equal(bc, want, "active card border uses --ok");
  assert.ok(await t.page.$('[data-anim="builtin:rainbow"] .badge'), "check badge");
  assert.equal(await t.page.$eval('[data-fx="glitch"]', (e) => e.getAttribute("aria-pressed")), "true");

  // Clicking again removes the layer.
  await t.page.click('[data-fx="glitch"]');
  await t.clickText("[data-anim] button.face", "Breathing");
  await sleep(300);
  assert.deepEqual((await layers()).map((l) => l.params.anim_id), ["builtin:rainbow"]);
  assert.equal(await t.page.$eval('[data-fx="glitch"]', (e) => e.getAttribute("aria-pressed")), "false");

  // Chip removal.
  await t.page.click('[data-fx="chase"]');
  await sleep(200);
  assert.equal((await layers()).length, 2);
  await t.page.click('[data-now-playing] [data-layer="chase"] .x');
  await sleep(300);
  assert.deepEqual((await layers()).map((l) => l.effect), ["animation"]);
  await t.page.click('[data-now-playing] [data-layer="animation"] .x');
  await sleep(300);
  assert.equal((await layers()).length, 0);
  assert.equal(await t.page.$eval('[data-anim="builtin:rainbow"]', (e) => e.classList.contains("active")), false);
}
