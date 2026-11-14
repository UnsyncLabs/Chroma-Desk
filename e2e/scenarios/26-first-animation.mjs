/** A first-time user: Animations page -> New animation -> paint two frames -> save -> find it in Quick -> apply. */
import { assert, sleep } from "../lib.mjs";

export default async function (t, url) {
  await t.load(url);
  await t.nav("^Animations");
  await t.clickText(".headBtns button", "^New animation");
  await t.waitFor(() => t.page.$(".timeline"), "frames editor opened by 'New animation'");
  // Nothing drawn yet: Apply/Save are off and the screen says why.
  assert.match(await t.page.evaluate(() => document.body.innerText), /Paint at least one LED/);
  await t.shot("first-empty", 1200, 800);
  await t.shot("first-empty", 400, 400);
  await t.setColor("#00ccff");
  await t.clickLed("Key: Space");
  await t.clickText(".frameBar button", "^Add frame$");
  await t.clickLed("Key: B");
  assert.doesNotMatch(await t.page.evaluate(() => document.body.innerText), /Paint at least one LED/);
  // Name it and save with Enter (as a user would).
  await t.page.click('.save input[placeholder^="e.g."]');
  await t.page.keyboard.type("Space pulse");
  await t.page.keyboard.press("Enter");
  await t.waitFor(async () => (await t.mockState("animations")).length === 1, "saved with Enter");
  await t.shot("first-saved", 1200, 800);
  const a = (await t.mockState("animations"))[0];
  // Quick shows it under "My animations" and one click applies it.
  await t.level("Quick");
  await t.page.click(`[data-anim="${a.id}"] button.only`);
  await sleep(300);
  const ds = (await t.scene()).devices["Razer Huntsman V3 Pro TKL@0"];
  assert.equal(ds.layers[0].params.anim_id, a.id);
  // The library page lists it with a Test button; the keyboard is in Effects mode now so it works.
  await t.nav("^Animations");
  await t.clickText(`[data-anim="${a.id}"] .actions button`, "^Test$");
  await t.waitFor(async () => (await t.mockState("triggers")).length === 1, "test trigger");
  await t.shot("first-library", 1200, 800);
  await t.shot("first-library", 400, 400);
}
