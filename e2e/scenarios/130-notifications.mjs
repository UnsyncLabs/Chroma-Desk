/** Notifications: templates, visual editor (devices, keys on the keyboard, colour slot), Test, desktop and Omarchy sources. */
import { assert, sleep } from "../lib.mjs";

const fire = (t, fn, arg) => t.page.evaluate((fn, arg) => globalThis.__chromaMock.fns[fn](arg), fn, arg);
const theme = (t) => t.mockState("theme");
const lastTrigger = async (t) => (await t.mockState("triggers")).at(-1);

export default async function (t, url) {
  await t.load(url);

  // Without a saved profile notifications are global; save one so they can live inside it.
  await t.nav("^Automations");
  await t.waitFor(() => t.page.$("[data-empty]"), "empty state");
  assert.equal((await t.page.$$("[data-template]")).length, 13, "thirteen one-click templates");
  assert.match(await t.page.evaluate(() => document.body.innerText), /no editable profile/i);
  await t.shot("notifications-empty", 1200, 800);
  await t.shot("notifications-empty", 400, 400);

  await t.nav("^Profiles");
  await t.clickText("button", "^Save as");
  await t.page.type("form.edit input[type=text]", "Gaming");
  await t.page.keyboard.press("Enter");
  await t.waitFor(async () => (await theme(t)).settings.current_profile === "Gaming", "profile saved");

  // ---- template with one click: it belongs to the profile
  await t.nav("^Automations");
  await t.waitFor(() => t.page.$("[data-template]"), "templates");
  await t.clickText('[data-template="alert_wasd"]', ".*");
  await t.waitFor(() => t.page.$('[data-notif="alert_wasd"]'), "card created");
  let gaming = (await theme(t)).profiles.Gaming;
  assert.deepEqual(gaming.triggers.alert_wasd.keys, ["W", "A", "S", "D"]);
  assert.equal(gaming.triggers.alert_wasd.color, "@alert");

  // ---- Test button fires on the (mock) hardware with the notification's keys and colour
  await t.clickText('[data-notif="alert_wasd"] [data-test]', ".*");
  await t.waitFor(async () => (await lastTrigger(t))?.name === "alert_wasd", "test fired");
  let trig = await lastTrigger(t);
  assert.deepEqual(trig.keys, ["W", "A", "S", "D"]);
  assert.equal(trig.color, "@alert");
  const ov = await t.mockState("overlays");
  assert.ok(ov.length >= 1, "an overlay is playing");

  // ---- new notification in the visual editor
  await t.clickText("[data-create]", ".*");
  await t.waitFor(() => t.page.$("[data-notif-editor]"), "editor");
  await t.page.$eval("[data-name]", (e) => (e.value = ""));
  await t.page.type("[data-name]", "discord_ping");
  await t.clickText("[data-style=pulse]", ".*");
  // Only the keyboard: untick "Every device", the first device is the keyboard.
  await t.clickText("[data-every]", ".*").catch(async () => t.page.click("[data-every]"));
  assert.ok(await t.page.$("[data-device]"), "device checkboxes");
  // Pick keys on the real keyboard drawing: leave "Whole device", then click Q and ctrl-click E.
  await t.page.click("[data-whole]");
  await t.waitFor(() => t.page.$("svg.canvas"), "keyboard canvas for key selection");
  await t.clickLed("Key: Q");
  await t.clickLed("Key: E", { ctrl: true });
  await t.pickSlot("accent", "[data-notif-editor]");
  assert.match(await t.page.$eval("[data-notif-editor] .trigger", (e) => e.textContent), /@accent/);
  await t.page.click('[data-scope="global"]');
  await t.shot("notif-editor-top", 1200, 800);
  await t.shot("notif-editor-top", 400, 400);

  // Sources: snippets, desktop notifications (with the opt-in), Omarchy events with hooks.
  await t.page.click("[data-snip-tab=python]");
  assert.match(await t.page.$eval("[data-snippet=python]", (e) => e.textContent), /discord_ping/);
  await t.page.click("[data-snip-tab=hypr]");
  assert.match(await t.page.$eval("[data-snippet=hypr]", (e) => e.textContent), /o\.bind\(.*chroma-desk trigger discord_ping/);
  await t.page.click("[data-snip-tab=cli]");
  assert.equal(await t.page.$eval("[data-snippet=cli]", (e) => e.textContent.trim()), "chroma-desk trigger discord_ping");
  await t.page.click("[data-desktop]");
  await t.page.type("[data-desktop-app]", "discord");
  await t.page.click("[data-listen]");
  await t.waitFor(async () => (await theme(t)).settings.desktop_notifications === true, "listening opt-in saved");
  assert.match(await t.page.$eval("[data-desktop-status]", (e) => e.textContent), /listening/);
  await t.page.evaluate(() => document.querySelector('[data-event="omarchy:battery-low"] input[type=checkbox]').click());
  await t.page.click('[data-hook-install="battery-low"]');
  await t.waitFor(async () => (await theme(t)).eventHooks["battery-low"] === true, "hook installed");
  assert.deepEqual((await theme(t)).hookCalls, ["install battery-low"]);
  await t.page.evaluate(() => document.querySelector('[data-event="omarchy:battery-low"] details').setAttribute("open", ""));
  await t.page.$eval('[data-source="desktop"]', (e) => e.scrollIntoView({ block: "start" }));
  await sleep(200);
  await t.shot("notif-editor-sources", 1200, 800);
  await t.shot("notif-editor-sources", 400, 400);

  await t.clickText("[data-save]", ".*");
  await t.waitFor(() => t.page.$('[data-notif="discord_ping"]'), "saved notification listed");
  const st = await theme(t);
  const saved = st.settings.global_notifications.discord_ping;
  assert.equal(saved.anim, "builtin:pulse");
  assert.deepEqual([...saved.keys].sort(), ["E", "Q"], "keys picked on the keyboard");
  assert.equal(saved.color, "@accent");
  assert.equal(saved.devices.length, 1, "only the keyboard");
  assert.equal(saved.sources.desktop.app, "discord");
  assert.deepEqual(saved.sources.omarchy, ["omarchy:battery-low"]);
  await t.shot("notifications-list", 1200, 800);
  await t.shot("notifications-list", 400, 400);

  // ---- sources really fire it
  assert.equal(await fire(t, "fireDesktop", { app: "Discord", summary: "New message" }), 1, "a Discord notification fires it");
  assert.equal((await lastTrigger(t)).name, "discord_ping");
  assert.deepEqual((await lastTrigger(t)).keys.map((k) => k.toLowerCase()).sort(), ["e", "q"]);
  assert.equal(await fire(t, "fireDesktop", { app: "Slack", summary: "x" }), 0, "another app does not");
  assert.equal(await fire(t, "fireEvent", "omarchy:battery-low"), 1, "the Omarchy event fires it");
  assert.equal(await fire(t, "fireEvent", "omarchy:post-update"), 0);

  // ---- toggle it off from the card: nothing fires any more
  await t.clickText('[data-notif="discord_ping"] .switch', "Enabled");
  await t.waitFor(async () => (await theme(t)).settings.global_notifications.discord_ping.enabled === false, "disabled");
  assert.equal(await fire(t, "fireDesktop", { app: "Discord", summary: "x" }), 0);
  await t.clickText('[data-notif="discord_ping"] .switch', "Enabled");
  await t.waitFor(async () => (await theme(t)).settings.global_notifications.discord_ping.enabled !== false, "enabled again");

  // ---- edit it again: remove the hook, then delete the notification
  await t.clickText('[data-notif="discord_ping"] [data-edit]', ".*");
  await t.waitFor(() => t.page.$("[data-notif-editor]"), "editor again");
  assert.equal(await t.page.$eval("[data-name]", (e) => e.value), "discord_ping");
  assert.ok((await t.page.$$eval("svg.canvas [data-led].sel", (e) => e.length)) === 2, "the two keys are shown selected");
  await t.page.click('[data-hook-remove="battery-low"]');
  await t.waitFor(async () => (await theme(t)).eventHooks["battery-low"] === false, "hook removed");
  assert.deepEqual((await theme(t)).hookCalls, ["install battery-low", "remove battery-low"]);
  await t.page.click("[data-back]");
  await t.clickText('[data-notif="discord_ping"] button.danger', ".*");
  await t.clickText("dialog button", "^Delete$");
  await t.waitFor(async () => !(await t.page.$('[data-notif="discord_ping"]')), "deleted");
  assert.equal((await theme(t)).settings.global_notifications.discord_ping, undefined);
  gaming = (await theme(t)).profiles.Gaming;
  assert.ok(gaming.triggers.alert_wasd, "the profile's own notification is untouched");
}
