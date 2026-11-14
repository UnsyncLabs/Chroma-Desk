/** "Use as: Theme / Effect" when saving decides where Quick shows an animation; the library shows the role. */
import { assert, sleep } from "../lib.mjs";

export default async function (t, url) {
  await t.load(url);
  const kb = "Razer Huntsman V3 Pro TKL@0";
  await t.nav("^Animations");
  await t.clickText(".headBtns button", "^New animation");
  await t.waitFor(() => t.page.$(".timeline"), "frames editor");
  await t.setColor("#ff00ff");
  await t.clickLed("Key: Space");
  assert.equal(await t.page.$eval('[data-role-pick="theme"]', (e) => e.getAttribute("aria-checked")), "true", "defaults to theme");
  await t.page.click('[data-role-pick="effect"]');
  await t.page.click('.save input[placeholder^="e.g."]');
  await t.page.keyboard.type("My overlay");
  await t.page.keyboard.press("Enter");
  await t.waitFor(async () => (await t.mockState("animations")).length === 1, "saved");
  const a = (await t.mockState("animations"))[0];
  assert.equal(a.role, "effect", "the chosen role is stored");

  // It shows up under Effects in Quick (not under Themes) and stacks on the running theme.
  await t.level("Quick");
  assert.ok(await t.page.$(`[data-effects] [data-anim="${a.id}"]`), "listed under Effects");
  assert.equal(await t.page.$(`[data-themes] [data-anim="${a.id}"]`), null, "not under Themes");
  await t.clickText("[data-theme-list] [data-anim] button.face", "Rainbow");
  await t.page.click(`[data-effects] [data-anim="${a.id}"] button.face`);
  await sleep(300);
  assert.deepEqual((await t.scene()).devices[kb].layers.map((l) => l.role), ["theme", "effect"]);
  assert.match(await t.page.$eval("[data-chip-effect]", (e) => e.textContent), /^Effect: My overlay/);

  // The library shows the role badge on every card.
  await t.nav("^Animations");
  await t.page.waitForSelector(".chip.role");
  const role = (id) => t.page.$eval(`[data-anim="${id}"] .chip.role`, (e) => e.getAttribute("data-role"));
  assert.equal(await role(a.id), "effect");
  assert.equal(await role("builtin:cyberpunk"), "theme");
  assert.equal(await role("builtin:glitch"), "effect");
  assert.equal(await role("builtin:starlight"), "effect");
  assert.equal(await role("builtin:neon-scan"), "theme");

  // Re-editing keeps the role as the default and it can be flipped back to a theme.
  await t.clickText(`[data-anim="${a.id}"] .actions button`, "^Edit$");
  await t.waitFor(() => t.page.$(".timeline"), "editor reopened");
  assert.equal(await t.page.$eval('[data-role-pick="effect"]', (e) => e.getAttribute("aria-checked")), "true", "keeps its role");
  await t.page.click('[data-role-pick="theme"]');
  await t.clickText(".save button", "^Update");
  await t.waitFor(async () => (await t.mockState("animations"))[0].role === "theme", "role changed");
}
