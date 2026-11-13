/** Notifications in depth: the Test button shows the colour on the real canvas, rename safety, global vs profile precedence, Omarchy only when detected. */
import { assert, sleep } from "../lib.mjs";

const theme = (t) => t.mockState("theme");

/** `#rrggbb` / `rgb(...)` fill to [r, g, b]. */
const rgbOf = (f) => {
  if (!f) return null;
  const m = /^#([0-9a-f]{6})$/i.exec(f);
  if (m) return [0, 2, 4].map((i) => parseInt(m[1].slice(i, i + 2), 16));
  const r = /rgb\((\d+)[ ,]+(\d+)[ ,]+(\d+)/.exec(f);
  return r ? [Number(r[1]), Number(r[2]), Number(r[3])] : null;
};
// The default alert slot colour, exactly (the base scene of the mock is a rainbow that may pass near red).
const isRed = (c) => !!c && c[0] === 255 && c[1] === 59 && c[2] === 59;

export default async function (t, url) {
  await t.load(url);
  // An editable profile so notifications can live in it.
  await t.nav("^Profiles");
  await t.clickText("button", "^Save as");
  await t.page.type("form.edit input[type=text]", "Gaming");
  await t.page.keyboard.press("Enter");
  await t.waitFor(async () => (await theme(t)).settings.current_profile === "Gaming", "profile saved");

  // ---- Test on the real canvas: a notification on W only, colour @alert, long enough to look at.
  await t.nav("^Automations");
  await t.clickText("[data-create]", ".*");
  await t.waitFor(() => t.page.$("[data-notif-editor]"), "editor");
  await t.page.$eval("[data-name]", (e) => (e.value = ""));
  await t.page.type("[data-name]", "wcheck");
  await t.clickText("[data-style=fill]", ".*");
  await t.setRange('input[aria-label="Duration"]', 20);
  await t.page.click("[data-every]");
  await t.page.click("[data-whole]");
  await t.waitFor(() => t.page.$("svg.canvas"), "key canvas");
  // The default selection is WASD; keep only W.
  for (const k of ["Key: A", "Key: S", "Key: D"]) await t.clickLed(k, { ctrl: true });
  // Deselecting the last key must not make the keyboard vanish under the cursor.
  await t.clickLed("Key: W", { ctrl: true });
  assert.ok(await t.page.$("svg.canvas"), "the keyboard stays while no key is selected");
  await t.clickLed("Key: W", { ctrl: true });
  await t.pickSlot("alert", "[data-notif-editor]");
  await t.clickText("[data-save]", ".*");
  await t.waitFor(() => t.page.$('[data-notif="wcheck"]'), "saved");
  const saved = (await theme(t)).profiles.Gaming.triggers.wcheck;
  assert.deepEqual(saved.keys, ["W"], "only W");
  assert.equal(saved.color, "@alert");

  await t.clickText('[data-notif="wcheck"] [data-test]', ".*");
  await t.waitFor(async () => (await t.mockState("overlays")).length >= 1, "overlay playing");
  await t.openDevice("Huntsman");
  await t.gotoPaint();
  const w = await t.ledIndex("Key: W");
  const q = await t.ledIndex("Key: Q");
  let sawRed = false;
  let qRed = false;
  const t0 = Date.now();
  while (Date.now() - t0 < 3000 && !sawRed) {
    sawRed = isRed(rgbOf(await t.fill(w)));
    qRed ||= isRed(rgbOf(await t.fill(q)));
    await sleep(60);
  }
  assert.ok(sawRed, "W is drawn in the @alert colour on the canvas");
  assert.ok(!qRed, "Q is untouched");
  await t.page.evaluate(() => (globalThis.__chromaMock.overlays = []));

  // ---- rename never overwrites another notification
  await t.nav("^Automations");
  await t.clickText("[data-create]", ".*");
  await t.page.$eval("[data-name]", (e) => (e.value = ""));
  await t.page.type("[data-name]", "second");
  await t.clickText("[data-save]", ".*");
  await t.waitFor(() => t.page.$('[data-notif="second"]'), "second saved");
  await t.clickText('[data-notif="second"] [data-edit]', ".*");
  await t.waitFor(() => t.page.$("[data-notif-editor]"), "editor second");
  await t.page.$eval("[data-name]", (e) => (e.value = ""));
  await t.page.type("[data-name]", "wcheck");
  await t.clickText("[data-save]", ".*");
  await sleep(400);
  assert.ok(await t.page.$("[data-notif-editor]"), "still editing: the name is taken");
  let st = await theme(t);
  assert.deepEqual(st.profiles.Gaming.triggers.wcheck.keys, ["W"], "the other notification is intact");
  assert.ok(st.profiles.Gaming.triggers.second, "and the edited one is still there");
  await t.page.$eval("[data-name]", (e) => (e.value = ""));
  await t.page.type("[data-name]", "renamed");
  await t.clickText("[data-save]", ".*");
  await t.waitFor(() => t.page.$('[data-notif="renamed"]'), "renamed");
  st = await theme(t);
  assert.ok(!st.profiles.Gaming.triggers.second && st.profiles.Gaming.triggers.renamed, "rename moves it");

  // ---- global vs profile: the profile's own notification wins, a global one is shared
  await t.clickText("[data-create]", ".*");
  await t.page.$eval("[data-name]", (e) => (e.value = ""));
  await t.page.type("[data-name]", "ping");
  await t.page.click('[data-scope="global"]');
  await t.pickSlot("ok", "[data-notif-editor]");
  await t.clickText("[data-save]", ".*");
  await t.waitFor(() => t.page.$('[data-notif="ping"]'), "global ping");
  assert.equal((await theme(t)).settings.global_notifications.ping.color, "@ok");
  await t.clickText('[data-notif="ping"] [data-test]', ".*");
  await t.waitFor(async () => (await t.mockState("triggers")).at(-1)?.color === "@ok", "global colour used");
  await t.clickText("[data-create]", ".*");
  await t.page.$eval("[data-name]", (e) => (e.value = ""));
  await t.page.type("[data-name]", "ping");
  await t.pickSlot("warn", "[data-notif-editor]");
  await t.clickText("[data-save]", ".*");
  await t.waitFor(async () => (await theme(t)).profiles.Gaming.triggers.ping, "profile ping");
  await t.waitFor(() => t.page.$('[data-notif="ping"]'), "list again");
  await t.clickText('[data-notif="ping"] [data-test]', ".*");
  await t.waitFor(async () => (await t.mockState("triggers")).at(-1)?.color === "@warn", "the profile's own colour wins");
  assert.equal((await t.page.$$('[data-notif="ping"]')).length, 1, "one card for the shadowed name");

  // ---- Omarchy events are only offered when Omarchy is there
  await t.page.evaluate(() => (globalThis.__chromaMock.theme.omarchyPresent = false));
  await t.clickText('[data-notif="ping"] [data-edit]', ".*");
  await t.waitFor(() => t.page.$("[data-notif-editor]"), "editor");
  await sleep(300);
  assert.ok(!(await t.page.$('[data-source="omarchy"]')), "no Omarchy section without Omarchy");
  assert.ok(await t.page.$('[data-source="desktop"]'), "the desktop source is always there");
  await t.page.click("[data-back]");
  await t.nav("^Profiles");
  await sleep(300);
  assert.ok(!(await t.page.$('[data-profile="Omarchy (follow theme)"]')), "the Omarchy profile is not listed");
  assert.ok(await t.page.$('[data-profile="Cyberpunk"]'), "Cyberpunk is");
  await t.page.evaluate(() => (globalThis.__chromaMock.theme.omarchyPresent = true));
  await t.nav("^Animations");
  await t.nav("^Profiles");
  await t.waitFor(() => t.page.$('[data-profile="Omarchy (follow theme)"]'), "listed again once detected");

  // ---- a built-in profile's notifications are shown but read-only (no error toasts from the backend)
  await t.clickText('[data-profile="Cyberpunk"] button', "^Apply$");
  await t.waitFor(async () => (await theme(t)).settings.current_profile === "Cyberpunk", "Cyberpunk applied");
  await t.nav("^Automations");
  await t.waitFor(() => t.page.$('[data-notif="alert"]'), "its notifications are listed");
  assert.equal(await t.page.$eval('[data-notif="alert"] [data-edit]', (e) => e.disabled), true, "edit is disabled");
  assert.equal(await t.page.$eval('[data-notif="alert"] button.danger', (e) => e.disabled), true, "delete is disabled");
  assert.ok(await t.page.$('[data-notif="alert"] [data-test]'), "Test still works");
  assert.ok(await t.page.$('[data-notif="ping"]'), "global notifications stay available");
  await t.shot("notifications-builtin", 1200, 800);
  await t.shot("notifications-builtin", 400, 400);
  t.noErrors();
}
