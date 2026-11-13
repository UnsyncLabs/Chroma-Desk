/** First-time-user paths: GPU monitor in one click, Stop all vs always-on, editing a template copy until it fires, threshold hysteresis and cooldown from the UI. */
import { assert, sleep } from "../lib.mjs";

const fns = (t, fn, arg) => t.page.evaluate((fn, arg) => globalThis.__chromaMock.fns[fn](arg), fn, arg);
const global = async (t) => (await t.mockState("theme")).settings.global_notifications;
const running = async (t, name) => (await t.mockState("overlays")).some((o) => o.name === name && o.until > Date.now());
const fired = async (t, name) => (await t.mockState("triggers")).filter((x) => x.name === name).length;
const type = async (t, sel, text) => {
  await t.page.$eval(sel, (e) => e.scrollIntoView({ block: "center" })); // clear of the sticky save bar
  await t.page.click(sel, { clickCount: 3 }); // select the old value, then type over it
  await t.page.keyboard.press("Backspace");
  await t.page.type(sel, text);
};

export default async function (t, url) {
  await t.load(url);
  await t.nav("^Automations");
  await t.waitFor(() => t.page.$("[data-empty]"), "empty state");

  // ---- "GPU usage on the F-row": one click on the template, running at once, the card says what it shows
  await t.clickText('[data-template="gpu_monitor"]', ".*");
  await t.waitFor(() => t.page.$('[data-notif="gpu_monitor"]'), "card");
  assert.match(await t.page.evaluate(() => document.body.innerText), /added and active/i, "the toast says it is already running");
  await t.waitFor(() => running(t, "gpu_monitor"), "gpu monitor running");
  const does = await t.page.$eval('[data-notif="gpu_monitor"] .does', (e) => e.textContent);
  assert.match(does, /GPU memory.*GPU usage.*13 keys/s, `card describes the meter: ${does}`);

  // ---- Stop all pauses always-on automations (it used to restart them within 200 ms)
  await t.clickText("[data-stop]", ".*");
  await t.waitFor(() => t.page.$("[data-paused-banner]"), "paused banner");
  assert.ok(await t.page.$('[data-notif="gpu_monitor"] [data-paused]'), "card shows Paused");
  await sleep(1200);
  assert.equal(await running(t, "gpu_monitor"), false, "stays stopped");
  await t.shot("automations-paused", 1200, 800);
  await t.shot("automations-paused", 400, 400);
  await t.clickText("[data-resume]", ".*");
  await t.waitFor(() => running(t, "gpu_monitor"), "resumed");
  assert.equal(await t.page.$("[data-paused-banner]"), null, "banner gone");
  // Disabling and enabling one paused monitor also resumes it.
  await t.clickText("[data-stop]", ".*");
  await t.waitFor(() => t.page.$('[data-notif="gpu_monitor"] [data-paused]'), "paused again");
  await t.clickText('[data-notif="gpu_monitor"] .switch', "Enabled");
  await sleep(700);
  await t.clickText('[data-notif="gpu_monitor"] .switch', "Enabled");
  await t.waitFor(() => running(t, "gpu_monitor"), "re-enabled runs again");
  assert.equal(await t.page.$("[data-paused-banner]"), null);

  // ---- edit a template copy until it fires: Slack also listens to Teams
  await t.page.$eval("[data-templates]", (d) => (d.open = true));
  await t.clickText('[data-template="slack"]', ".*");
  await t.waitFor(() => t.page.$('[data-notif="slack"]'), "slack card");
  await t.clickText('[data-notif="slack"] [data-edit]', ".*");
  await t.waitFor(() => t.page.$("[data-notif-editor]"), "editor");
  await t.page.$eval("[data-when-picker]", (e) => e.scrollIntoView({ block: "start" }));
  assert.equal(await t.page.$eval("[data-desktop-app]", (e) => e.value), "slack");
  await type(t, "[data-desktop-app]", "slack, teams");
  await t.clickText("[data-save]", ".*");
  await t.waitFor(() => t.page.$('[data-notif="slack"]'), "saved");
  assert.equal((await global(t)).slack.sources.desktop.app, "slack, teams");
  await t.page.evaluate(() => (globalThis.__chromaMock.theme.settings.desktop_notifications = true));
  assert.equal(await fns(t, "fireDesktop", { app: "Microsoft Teams", summary: "hi" }), 1, "the edited copy fires for the new app");
  assert.equal(await fns(t, "fireDesktop", { app: "Slack", summary: "hi" }), 1);
  assert.equal(await fns(t, "fireDesktop", { app: "Zoom", summary: "hi" }), 0);

  // ---- threshold from the UI: fires once, hysteresis keeps it quiet, cooldown spaces the next one
  await t.clickText("[data-create]", ".*");
  await t.waitFor(() => t.page.$("[data-notif-editor]"), "editor");
  await type(t, "[data-name]", "hot");
  await t.page.click('[data-scope="global"]');
  await t.page.$eval("[data-when-picker]", (e) => e.scrollIntoView({ block: "start" }));
  await t.page.click("[data-when-threshold]");
  await t.waitFor(() => t.page.$("[data-th-metric]"), "threshold fields");
  await t.page.select("[data-th-metric]", "cpu.temp");
  await type(t, "[data-th-value]", "50");
  await type(t, "[data-th-for]", "0");
  await t.clickText("details summary", "hysteresis"); // the re-arm options sit in a collapsed section
  await type(t, "[data-th-hyst]", "10");
  await type(t, "[data-th-cool]", "3");
  await t.shot("automation-editor-threshold", 1200, 800);
  await t.shot("automation-editor-threshold", 400, 400);
  await t.clickText("[data-save]", ".*");
  await t.waitFor(() => t.page.$('[data-notif="hot"]'), "saved hot");
  assert.deepEqual({ ...(await global(t)).hot.sources.threshold }, { metric: "cpu.temp", op: "gt", value: 50, for_secs: 0, hysteresis: 10, cooldown_secs: 3 });
  const base = await fired(t, "hot");
  await fns(t, "setMetric", { metric: "cpu.temp", value: 60 });
  await t.waitFor(async () => (await fired(t, "hot")) === base + 1, "fires when above");
  await fns(t, "setMetric", { metric: "cpu.temp", value: 45 }); // below the limit, inside the 10 degree band
  await sleep(600);
  await fns(t, "setMetric", { metric: "cpu.temp", value: 60 });
  await sleep(900);
  assert.equal(await fired(t, "hot"), base + 1, "not re-armed inside the hysteresis band");
  await fns(t, "setMetric", { metric: "cpu.temp", value: 35 }); // released
  await sleep(600);
  await fns(t, "setMetric", { metric: "cpu.temp", value: 60 });
  await t.waitFor(async () => (await fired(t, "hot")) === base + 2, "fires again once re-armed and cooled down", 6000);
  await fns(t, "setMetric", { metric: "cpu.temp", value: null });
}
