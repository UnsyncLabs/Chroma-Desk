/** Built-in profiles (Omarchy follow-theme, Cyberpunk), duplicating one, editing a profile theme, and the lights following Omarchy. */
import { assert, sleep } from "../lib.mjs";

const OM = "Omarchy (follow theme)";
const card = (n) => `li[data-profile="${n}"]`;
const swatch = (t, slot) => t.page.$eval(`[data-swatch="${slot}"]`, (e) => getComputedStyle(e).backgroundColor);

export default async function (t, url) {
  await t.load(url);
  await t.nav("^Profiles");
  await t.waitFor(() => t.page.$(card("Cyberpunk")), "built-in cards");

  // Built-in cards: badge, palette strip, theme source; Omarchy ships disabled.
  assert.ok(await t.page.$(`${card("Cyberpunk")} .badge.b`), "Built-in badge");
  assert.equal((await t.page.$$(`${card("Cyberpunk")} .strip span`)).length, 7, "palette strip");
  assert.ok(await t.page.$(`${card(OM)} .switch input`), "Omarchy has an Enable switch");
  assert.equal(await t.page.$eval(`${card(OM)} button.primary`, (b) => b.disabled), true, "disabled profile cannot be applied");
  assert.match(await t.page.$eval(`${card(OM)} [data-live-theme]`, (e) => e.textContent), /tokyo-night/, "live detected theme");
  await t.shot("profiles-builtin", 1200, 800);
  await t.shot("profiles-builtin", 400, 400);

  // Enable and apply the Omarchy profile.
  await t.clickText(`${card(OM)} .switch`, "Enable");
  await t.waitFor(async () => (await t.page.$eval(`${card(OM)} button.primary`, (b) => b.disabled)) === false, "enabled");
  await t.clickText(`${card(OM)} button`, "^Apply$");
  await t.waitFor(async () => (await t.mockState("theme")).settings.current_profile === OM, "applied");
  const kbKey = "Razer Huntsman V3 Pro TKL@0";
  const sc = await t.scene();
  assert.ok(JSON.stringify(sc.devices[kbKey].layers).includes('"slot":"accent"'), "scene is made of palette slots");

  // Settings -> Themes shows the live palette (tokyo-night accent #7aa2f7).
  await t.nav("^Settings");
  await t.waitFor(() => t.page.$("[data-palette]"), "palette strip");
  assert.equal(await swatch(t, "accent"), "rgb(122, 162, 247)");
  assert.match(await t.page.$eval("[data-palette-name]", (e) => e.textContent), /tokyo-night/);

  // `omarchy theme set catppuccin`: swatches AND the keyboard canvas follow on their own.
  await t.openDevice("Huntsman");
  await t.gotoPaint();
  const w = await t.ledIndex("Key: W");
  await t.waitFor(async () => (await t.fill(w)) === "#7aa2f7", "W is the tokyo-night accent");
  await t.page.evaluate(() => globalThis.__chromaMock.fns.setOmarchyTheme("catppuccin"));
  await t.waitFor(async () => (await t.fill(w)) === "#89b4fa", "W follows catppuccin");
  await t.nav("^Settings");
  await t.waitFor(async () => (await swatch(t, "accent")) === "rgb(137, 180, 250)", "swatches follow catppuccin");
  await t.shot("settings-themes", 1200, 800);
  await t.shot("settings-themes", 400, 400);
  await t.page.evaluate(() => globalThis.__chromaMock.fns.setOmarchyTheme("tokyo-night"));
  await t.waitFor(async () => (await swatch(t, "accent")) === "rgb(122, 162, 247)", "back to tokyo-night");

  // Duplicate the built-in Cyberpunk to edit it.
  await t.nav("^Profiles");
  await t.clickText(`${card("Cyberpunk")} button`, "^Duplicate to edit$");
  await t.page.type("form.edit input[type=text]", " neon");
  await t.page.keyboard.press("Enter");
  const mine = "Cyberpunk (copy) neon";
  await t.waitFor(() => t.page.$(card(mine)), "duplicate listed");
  assert.equal(await t.page.$(`${card(mine)} .badge.b`), null, "the copy is not built-in");
  const dup = (await t.mockState("theme")).profiles[mine];
  assert.equal(dup.theme.manual_palette.accent[0], 0xfc, "keeps the neon palette");
  assert.ok(Object.keys(dup.scene.devices).length >= 5, "scene frozen for the connected devices");

  // Edit its theme: source chips, slot editor, override, pinned preset.
  await t.clickText(`${card(mine)} button`, "^Theme & presets$");
  await t.waitFor(() => t.page.$("[data-theme-editor]"), "theme editor");
  assert.equal(await t.page.$eval('[data-source="manual"]', (e) => e.getAttribute("aria-checked")), "true");
  assert.equal(await t.page.$eval('[data-source="kde"] small', (e) => e.classList.contains("bad")), true, "KDE not detected here, shown without error");
  await t.setColor("#00ff00", '[data-slot-row="accent"]');
  await t.page.select('[data-theme-editor] select[aria-label="Slot to pin"]', "alert");
  await t.clickText("[data-theme-editor] button", "^Pin slot$");
  assert.ok(await t.page.$('[data-override="alert"]'), "override listed");
  await t.setColor("#ff0000", '[data-override="alert"]');
  await t.page.select('[data-theme-editor] select[aria-label="Pick an animation"]', "builtin:starlight");
  await t.clickText("[data-theme-editor] button", "^Pin$");
  await t.shot("theme-editor", 1200, 800);
  await t.shot("theme-editor", 400, 400);
  await t.clickText("[data-theme-editor] button.primary", "^Save theme$");
  await t.waitFor(async () => (await t.mockState("theme")).profiles[mine].theme.overrides.alert, "saved");
  const saved = (await t.mockState("theme")).profiles[mine];
  assert.deepEqual(saved.theme.manual_palette.accent, [0, 255, 0]);
  assert.deepEqual(saved.theme.overrides.alert, [255, 0, 0]);
  assert.equal(saved.presets.at(-1).id, "builtin:starlight", "pinned after the ones it inherited");
  assert.equal(saved.presets[0].id, "builtin:cyberpunk", "kept the Cyberpunk presets (theme first)");
  assert.ok(saved.presets.some((p) => p.id === "builtin:glitch"), "and its effects");

  // Apply it; Quick shows the profile's presets first.
  await t.clickText(`${card(mine)} button`, "^Apply$");
  await t.waitFor(async () => (await t.mockState("theme")).settings.current_profile === mine, "applied copy");
  await t.openDevice("Huntsman");
  await t.waitFor(() => t.page.$("[data-pinned]"), "profile presets heading");
  const pinned = await t.page.$$eval("[data-pinned] + ul [data-anim]", (els) => els.map((e) => e.getAttribute("data-anim")));
  assert.equal(pinned[0], "builtin:cyberpunk", "the profile's themes come first");
  assert.ok(pinned.includes("builtin:neon-scan") && !pinned.includes("builtin:glitch"), "only themes under Themes");
  const pinnedFx = await t.page.$$eval("[data-pinned-effects] + ul [data-anim]", (els) => els.map((e) => e.getAttribute("data-anim")));
  assert.ok(pinnedFx.includes("builtin:glitch") && pinnedFx.includes("builtin:starlight"), "the profile's effects (including the one just pinned) sit under Effects");
  await t.shot("quick-profile-presets", 1200, 800);
  await t.shot("quick-profile-presets", 400, 400);
  await sleep(100);
}
