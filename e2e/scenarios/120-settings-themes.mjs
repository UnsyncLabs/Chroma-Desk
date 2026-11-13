/** Settings -> Themes: automatic profile rules, the Omarchy hook (shows the command, installs and removes). */
import { assert } from "../lib.mjs";

const theme = (t) => t.mockState("theme");

export default async function (t, url) {
  await t.load(url);
  // Two profiles to switch between.
  await t.nav("^Profiles");
  for (const n of ["Work", "Night"]) {
    await t.clickText("button", "^Save as");
    await t.page.$eval("form.edit input[type=text]", (e) => (e.value = ""));
    await t.page.type("form.edit input[type=text]", n);
    await t.page.keyboard.press("Enter");
    await t.waitFor(async () => (await theme(t)).settings.current_profile === n, `saved ${n}`);
  }

  await t.nav("^Settings");
  await t.waitFor(() => t.page.$("[data-themes]"), "themes section");

  // Rule: when the Omarchy theme becomes catppuccin, apply "Work".
  await t.page.type('[data-themes] input[aria-label="Omarchy theme name"]', "catppuccin");
  await t.page.select('[data-themes] select[aria-label="Profile to apply"]', "Work");
  await t.clickText("[data-themes] button", "^Add rule$");
  await t.waitFor(() => t.page.$('[data-rule="catppuccin"]'), "rule listed");
  assert.deepEqual((await theme(t)).settings.theme_rules, [{ theme: "catppuccin", profile: "Work" }]);

  // The hook: the exact command is shown; nothing is installed until the button is pressed.
  assert.match(await t.page.$eval("[data-hook-command]", (e) => e.textContent), /^omarchy hook install theme-set .*50-chroma-desk$/);
  assert.equal((await theme(t)).hook, false);
  await t.shot("settings-themes-rules", 1200, 800);
  await t.shot("settings-themes-rules", 400, 400);
  await t.page.click('[data-hook="install"]');
  await t.waitFor(async () => (await theme(t)).hook === true, "hook installed");
  await t.waitFor(() => t.page.$('[data-hook="remove"]'), "remove button");
  await t.page.click('[data-hook="remove"]');
  await t.waitFor(async () => (await theme(t)).hook === false, "hook removed");
  assert.deepEqual((await theme(t)).hookCalls, ["install theme-set", "remove theme-set"]);

  // `omarchy theme set catppuccin` happens: the rule switches the profile by itself.
  assert.equal((await theme(t)).settings.current_profile, "Night");
  await t.page.evaluate(() => globalThis.__chromaMock.fns.setOmarchyTheme("catppuccin"));
  await t.waitFor(async () => (await theme(t)).settings.current_profile === "Work", "profile switched by the rule");
  await t.nav("^Profiles");
  await t.waitFor(async () => /Current profile:\s*Work/.test(await t.page.evaluate(() => document.body.innerText)), "UI shows the new current profile");

  // Removing the rule.
  await t.nav("^Settings");
  await t.page.click('[data-rule="catppuccin"] button');
  await t.waitFor(async () => (await theme(t)).settings.theme_rules.length === 0, "rule removed");
}
