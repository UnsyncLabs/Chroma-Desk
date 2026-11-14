/** Monitor timing (Always vs Periodic every/for, metrics) and the Notifications/Automations section of the Animations library. */
import { assert, sleep } from "../lib.mjs";

const global = async (t) => (await t.mockState("theme")).settings.global_notifications;
const setNum = (t, sel, v) =>
  t.page.$eval(
    sel,
    (e, v) => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(e, String(v));
      e.dispatchEvent(new Event("input", { bubbles: true }));
    },
    v,
  );
const running = async (t, name) => (await t.mockState("overlays")).some((o) => o.name === name);

export default async function (t, url) {
  await t.load(url);
  await t.nav("^Automations");
  await t.waitFor(() => t.page.$("[data-empty]"), "empty state");
  const tpls = await t.page.$$eval("[data-template]", (e) => e.map((x) => x.getAttribute("data-template")));
  assert.ok(tpls.includes("gpu_temp_monitor"), "GPU temperature monitor template offered");
  await t.clickText('[data-template="gpu_monitor"]', ".*");
  await t.waitFor(() => t.page.$('[data-notif="gpu_monitor"]'), "gpu monitor card");
  await t.page.$eval("[data-templates]", (d) => (d.open = true));
  await t.clickText('[data-template="gpu_temp_monitor"]', ".*");
  await t.waitFor(() => t.page.$('[data-notif="gpu_temp_monitor"]'), "gpu temp card");
  assert.deepEqual((await global(t)).gpu_temp_monitor.sequence.kind.steps.map((s) => s.params.metric), ["gpu.temp", "gpu.usage"]);

  // ---- Display mode: default Always, no duplicate timing cards below
  await t.clickText('[data-notif="gpu_monitor"] [data-edit]', ".*");
  await t.waitFor(() => t.page.$("[data-monitor-options]"), "monitor options");
  assert.equal(await t.page.$eval('[data-dm="always"]', (e) => e.getAttribute("aria-checked")), "true");
  assert.equal(await t.page.$eval('[data-dm="periodic"]', (e) => e.getAttribute("aria-checked")), "false");
  assert.equal(await t.page.$('[data-when="always"]'), null, "timing lives in Display mode only");
  await t.shot("monitor-display-mode", 1200, 800);
  await t.shot("monitor-display-mode", 400, 400);

  // Periodic: every 3 s, visible for 2 s. Visible time above the period is refused.
  await t.clickText('[data-dm="periodic"]', ".*");
  await t.waitFor(() => t.page.$("[data-dm-fields]"), "timing fields");
  assert.equal(await t.page.$eval('[data-dm="always"]', (e) => e.getAttribute("aria-checked")), "false", "mutually exclusive");
  assert.equal(await t.page.$eval("[data-dm-every]", (e) => e.value), "10");
  assert.equal(await t.page.$eval("[data-dm-for]", (e) => e.value), "7");
  await setNum(t, "[data-dm-every]", 3);
  await t.waitFor(() => t.page.$("[data-dm-error]"), "7 s > 3 s is flagged");
  await t.clickText("[data-save]", ".*");
  await sleep(300);
  assert.ok(await t.page.$("[data-notif-editor]"), "invalid timing does not save");
  assert.ok(!(await global(t)).gpu_monitor.sources.every, "nothing stored");
  await setNum(t, "[data-dm-for]", 2);
  await t.waitFor(async () => !(await t.page.$("[data-dm-error]")), "error gone");
  // Minutes unit: 3 min is 180 s.
  await t.page.select("[data-dm-every-unit]", "min");
  await t.waitFor(async () => (await t.page.$eval("[data-dm-every]", (e) => e.value)) === "3", "3 min kept");
  await t.page.select("[data-dm-every-unit]", "s");
  await setNum(t, "[data-dm-every]", 3);

  // Metrics: add GPU temperature, drop VRAM (the last one cannot be removed).
  await t.clickText('[data-metric-chip="gpu.temp"]', ".*");
  await t.clickText('[data-metric-chip="gpu.vram"]', ".*");
  await t.clickText("[data-save]", ".*");
  await t.waitFor(() => t.page.$('[data-notif="gpu_monitor"]'), "saved");
  const g = (await global(t)).gpu_monitor;
  assert.deepEqual(g.sources.every, { every_secs: 3, for_secs: 2 });
  assert.ok(!g.sources.always, "always dropped");
  assert.deepEqual(g.sequence.kind.steps.map((s) => s.params.metric), ["gpu.usage", "gpu.temp"]);
  assert.equal(g.sequence.kind.steps[1].keys.length, 13, "new meter reuses the F-row");
  assert.match(await t.page.$eval('[data-notif="gpu_monitor"] [data-when-chips]', (e) => e.textContent), /Every 3 s for 2 s/);
  const groups = await t.page.$$eval("[data-group]", (e) => e.map((x) => x.getAttribute("data-group")));
  assert.deepEqual(groups, ["monitor"], "a periodic monitor stays a monitor");

  // The overlay appears, lasts for_secs, then hides until the next period.
  await t.waitFor(() => running(t, "gpu_monitor"), "overlay shows up", 9000);
  await t.waitFor(async () => !(await running(t, "gpu_monitor")), "overlay hides again", 5000);

  // Back to Always: periodic is dropped.
  await t.clickText('[data-notif="gpu_monitor"] [data-edit]', ".*");
  await t.waitFor(() => t.page.$("[data-monitor-options]"), "options");
  await t.clickText('[data-dm="always"]', ".*");
  await t.clickText("[data-save]", ".*");
  await t.waitFor(() => t.page.$('[data-notif="gpu_monitor"]'), "saved again");
  const g2 = (await global(t)).gpu_monitor;
  assert.ok(g2.sources.always && !g2.sources.every);

  // ---- a non-monitor timer keeps the classic cards, also exclusive
  await t.clickText("[data-create]", ".*");
  await t.waitFor(() => t.page.$("[data-notif-editor]"), "editor");
  assert.equal(await t.page.$("[data-monitor-options]"), null, "plain animations have no monitor options");
  await t.clickText("[data-when-always]", ".*");
  await t.clickText("[data-when-every]", ".*");
  assert.equal(await t.page.$eval("[data-when-always]", (e) => e.checked), false, "every switches always off");
  await t.clickText("[data-back]", ".*");

  // ---- Animations library: the user's automations appear in their own section
  await t.nav("^Animations");
  await t.waitFor(() => t.page.$("[data-library-automations]"), "library automations section");
  await t.waitFor(() => t.page.$('[data-lib-auto="gpu_monitor"]'), "gpu_monitor listed");
  assert.ok(await t.page.$('[data-lib-auto="gpu_temp_monitor"]'));
  assert.ok(await t.page.$('[data-lib-auto="gpu_monitor"] [data-lib-auto-seq]'), "embedded sequence is visible");
  assert.match(await t.page.$eval('[data-lib-auto="gpu_monitor"] [data-lib-auto-when]', (e) => e.textContent), /Always/);
  await t.shot("library-automations", 1200, 800);
  await t.shot("library-automations", 400, 400);
  await t.clickText('[data-lib-auto="gpu_monitor"] [data-lib-auto-test]', ".*");
  await t.waitFor(() => t.page.$eval("body", (b) => /Played on/i.test(b.innerText) || /tested|Test/i.test(b.innerText)), "test fired");
  // Save the sequence into the library.
  await t.clickText('[data-lib-auto="gpu_monitor"] [data-lib-auto-saveseq]', ".*");
  await t.waitFor(async () => (await t.mockState("animations")).some((a) => a.name === "gpu_monitor" && a.kind.type === "sequence"), "sequence saved to the library");
  // Edit jumps to the automation editor.
  await t.clickText('[data-lib-auto="gpu_temp_monitor"] [data-lib-auto-edit]', ".*");
  await t.waitFor(() => t.page.$("[data-notif-editor]"), "editor opened from the library");
  assert.equal(await t.page.$eval("[data-name]", (e) => e.value), "gpu_temp_monitor");
  assert.ok(await t.page.$("[data-monitor-options]"));
}
