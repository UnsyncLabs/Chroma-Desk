/** Automations: templates, sequence editor (steps, ordered keys), every "When" trigger, Test, thresholds, System monitor, Claude Code. */
import { assert, sleep } from "../lib.mjs";

const fns = (t, fn, arg) => t.page.evaluate((fn, arg) => globalThis.__chromaMock.fns[fn](arg), fn, arg);
const theme = (t) => t.mockState("theme");
const global = async (t) => (await theme(t)).settings.global_notifications;

async function openEditor(t, name) {
  await t.clickText(`[data-notif="${name}"] [data-edit]`, ".*");
  await t.waitFor(() => t.page.$("[data-notif-editor]"), "editor");
}
const stepKeys = (t, name) => global(t).then((g) => g[name].sequence.kind.steps.map((s) => s.keys.map((k) => k.replace("name:Key: ", ""))));

export default async function (t, url) {
  await t.load(url);
  await t.nav("^Automations");
  await t.waitFor(() => t.page.$("[data-empty]"), "empty state");
  const tpls = await t.page.$$eval("[data-template]", (e) => e.map((x) => x.getAttribute("data-template")));
  for (const id of ["gpu_monitor", "cpu_monitor", "discord", "telegram", "slack", "matrix", "ai_needs_you"]) assert.ok(tpls.includes(id), `template ${id}`);
  assert.match(await t.page.evaluate(() => document.body.innerText), /Notifications & monitors/);
  await t.shot("automations-empty", 1200, 800);
  await t.shot("automations-empty", 400, 400);

  // ---- create from each built-in template (real clicks)
  for (const id of ["gpu_monitor", "cpu_monitor", "discord", "telegram", "slack", "matrix", "ai_needs_you"]) {
    await t.waitFor(() => t.page.$(`[data-template="${id}"]`), `template ${id}`);
    if (id !== "gpu_monitor") await t.page.$eval("[data-templates]", (d) => (d.open = true)).catch(() => {});
    await t.clickText(`[data-template="${id}"]`, ".*");
    await t.waitFor(() => t.page.$(`[data-notif="${id}"]`), `card ${id}`);
  }
  const g = await global(t);
  assert.equal(g.gpu_monitor.sources.always, true);
  assert.deepEqual(g.gpu_monitor.sequence.kind.steps.map((s) => s.params.metric), ["gpu.vram", "gpu.usage"]);
  assert.equal(g.gpu_monitor.sequence.kind.steps[0].keys.length, 13, "Esc + F1..F12");
  assert.equal(g.cpu_monitor.sequence.kind.steps[1].params.min, 30);
  assert.equal(g.cpu_monitor.sequence.kind.steps[1].params.max, 95);
  assert.equal(g.discord.sources.desktop.app, "discord");
  assert.equal(g.slack.sources.desktop.app, "slack");
  assert.match(g.matrix.sources.desktop.app, /element.*fractal.*nheko.*cinny/);
  assert.equal(g.discord.sequence.kind.steps.length, 4);
  assert.deepEqual(g.ai_needs_you.sources.ai, { from: "", kind: "" });
  assert.equal(g.ai_needs_you.color, "@alert");
  // Groups and When chips.
  const groups = await t.page.$$eval("[data-group]", (e) => e.map((x) => x.getAttribute("data-group")));
  assert.deepEqual(groups, ["monitor", "notification"]);
  assert.match(await t.page.$eval('[data-notif="gpu_monitor"] [data-when-chips]', (e) => e.textContent), /Always/);
  assert.match(await t.page.$eval('[data-notif="discord"] [data-when-chips]', (e) => e.textContent), /Desktop: discord/);
  assert.match(await t.page.$eval('[data-notif="ai_needs_you"] [data-when-chips]', (e) => e.textContent), /AI assistant/);
  await t.shot("automations-list", 1200, 800);
  await t.shot("automations-list", 400, 400);

  // ---- the "Always" monitor runs as an overlay on top of the scene
  await t.waitFor(async () => (await t.mockState("overlays")).some((o) => o.name === "gpu_monitor"), "always-on monitor running");
  await t.waitFor(async () => (await t.mockState("overlays")).some((o) => o.name === "cpu_monitor"), "cpu monitor running");

  // ---- the bar is drawn ON TOP of the running scene: unfilled keys keep the rainbow underneath, not black
  await fns(t, "setMetric", { metric: "cpu.usage", value: 20 });
  await t.openDevice("Huntsman");
  await t.level("Layers").catch(() => {});
  const hexRgb = (h) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
  const colorsOf = async () => {
    const f = async (name) => hexRgb(await t.fill(await t.ledIndex(name)));
    return { first: await f("Key: `"), unfilled: [await f("Key: 9"), await f("Key: ="), await f("Key: Backspace")] };
  };
  // CPU 20 % of 14 keys = 2.8 keys (during the usage step); wait for that step of the loop.
  await t.waitFor(async () => {
    const { first, unfilled } = await colorsOf();
    return first[0] > 200 && first[1] > 60 && first[1] < 200 && first[2] < 60 && unfilled.every((c) => c[0] + c[1] + c[2] > 120);
  }, "orange bar on the first keys while the rest keeps showing the scene", 12000);
  await t.shot("monitor-over-scene", 1200, 800);
  await fns(t, "setMetric", { metric: "cpu.usage", value: null });
  await t.nav("^Automations");

  // ---- sequence editor: edit the Discord template
  await openEditor(t, "discord");
  assert.equal(await t.page.$$eval("[data-step]", (e) => e.length), 4);
  await t.clickText("[data-step='1'] [data-step-select]", ".*");
  await t.waitFor(() => t.page.$("[data-seq-keys] svg.canvas"), "keys canvas");
  const nums = await t.page.$$eval("svg.canvas .pathLayer text", (e) => e.map((x) => x.textContent.trim()));
  assert.deepEqual(nums, ["1", "2", "3", "4"], "orbit keys are numbered in order");
  await t.shot("automation-editor-sequence", 1200, 800);
  // Reorder with the arrows, add a step, delete it again.
  await t.clickText("[data-step='1'] [data-step-down]", ".*");
  await t.waitFor(async () => (await t.page.$eval("[data-step='2'] .pick", (e) => e.textContent)).includes("Orbit"), "step moved down");
  await t.clickText("[data-step='2'] [data-step-up]", ".*");
  await t.clickText("[data-add-step=solid]", ".*");
  assert.equal(await t.page.$$eval("[data-step]", (e) => e.length), 5);
  await t.clickText("[data-step='4'] [data-step-delete]", ".*");
  assert.equal(await t.page.$$eval("[data-step]", (e) => e.length), 4);
  // HTML5 drag and drop (synthetic events): move step 4 (pause) to the top.
  await t.page.evaluate(() => {
    const [a, b] = [document.querySelector("[data-step='3']"), document.querySelector("[data-step='0']")];
    const dt = new DataTransfer();
    a.dispatchEvent(new DragEvent("dragstart", { dataTransfer: dt, bubbles: true }));
    b.dispatchEvent(new DragEvent("dragover", { dataTransfer: dt, bubbles: true, cancelable: true }));
    b.dispatchEvent(new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true }));
  });
  await t.waitFor(async () => (await t.page.$eval("[data-step='0'] .pick", (e) => e.textContent)).includes("Off"), "dragged to the top");
  await t.clickText("[data-step='0'] [data-step-down]", ".*");
  await t.clickText("[data-step='1'] [data-step-down]", ".*");
  await t.clickText("[data-step='2'] [data-step-down]", ".*");
  await t.clickText("[data-save]", ".*");
  await t.waitFor(() => t.page.$('[data-notif="discord"]'), "saved");
  assert.deepEqual((await global(t)).discord.sequence.kind.steps.map((s) => s.effect), ["blink", "orbit", "orbit", "hold"], "order preserved after the shuffles");

  // ---- new automation: a meter whose keys are picked in order on the keyboard
  await t.clickText("[data-create]", ".*");
  await t.waitFor(() => t.page.$("[data-notif-editor]"), "editor");
  await t.page.$eval("[data-name]", (e) => (e.value = ""));
  await t.page.type("[data-name]", "my_meter");
  await t.clickText("[data-mode=sequence]", ".*");
  await t.waitFor(() => t.page.$("[data-step='0']"), "first step");
  assert.match(await t.page.$eval("[data-live]", (e) => e.textContent), /\d+ %/, "live CPU value next to the metric");
  await t.page.select("[data-metric]", "gpu.vram");
  await t.waitFor(() => t.page.$eval("[data-live]", (e) => /GB/.test(e.textContent)), "VRAM shows used/total");
  assert.match(await t.page.$eval("[data-live]", (e) => e.textContent), /\d+(\.\d)?\/12 GB · \d+ %/);
  await t.waitFor(() => t.page.$("[data-seq-keys] svg.canvas"), "canvas");
  await t.clickLed("Key: F3");
  await t.clickLed("Key: F1");
  await t.clickLed("Key: F2");
  await t.waitFor(async () => (await t.page.$$eval("svg.canvas .pathLayer text", (e) => e.length)) === 3, "three numbers");
  assert.deepEqual(await t.page.$$eval("svg.canvas .pathLayer text", (e) => e.map((x) => x.textContent.trim())), ["1", "2", "3"]);
  await t.shot("automation-editor-meter", 1200, 800);
  await t.shot("automation-editor-meter", 400, 400);
  await t.clickText("[data-key-undo]", ".*");
  await t.clickLed("Key: F4");
  // Min/max for a temperature metric come from the metric.
  await t.page.select("[data-metric]", "cpu.temp");
  assert.equal(await t.page.$eval("[data-min]", (e) => e.value), "30");
  assert.equal(await t.page.$eval("[data-max]", (e) => e.value), "95");
  // Colour picker linking to @warn with real clicks.
  await t.pickSlot("warn", "[data-step-params]");

  // ---- every "When" trigger type
  await t.page.$eval("[data-when-picker]", (e) => e.scrollIntoView({ block: "start" }));
  await t.page.click("[data-when-always]");
  await t.page.click("[data-when-every]");
  await t.page.$eval("[data-every-value]", (e) => (e.value = ""));
  await t.page.type("[data-every-value]", "2");
  await t.page.select("[data-every-unit]", "min");
  await t.page.$eval("[data-every-for]", (e) => (e.value = ""));
  await t.page.type("[data-every-for]", "7");
  await t.page.click("[data-when-threshold]");
  await t.waitFor(() => t.page.$("[data-th-metric]"), "threshold fields");
  await t.page.select("[data-th-metric]", "cpu.temp");
  await t.page.$eval("[data-th-value]", (e) => (e.value = ""));
  await t.page.type("[data-th-value]", "80");
  await t.page.$eval("[data-th-for]", (e) => (e.value = ""));
  await t.page.type("[data-th-for]", "0");
  assert.match(await t.page.$eval("[data-th-live]", (e) => e.textContent), /\d+ °C/);
  await t.page.click("[data-desktop]");
  await t.page.type("[data-desktop-app]", "myapp, other");
  await t.page.click("[data-when-ai]");
  await t.waitFor(() => t.page.$("[data-claude]"), "Claude setup inside the AI trigger");
  await t.page.type("[data-ai-from]", "claude");
  await t.page.select("[data-ai-kind]", "permission");
  await t.page.$eval("[data-when-picker]", (e) => e.scrollIntoView({ block: "start" }));
  await t.shot("automation-editor-when", 1200, 800);
  await t.shot("automation-editor-when", 400, 400);
  // Test button sends the unsaved automation to the engine.
  await t.clickText("[data-test-editor]", ".*");
  await t.waitFor(async () => (await theme(t)).testCalls.includes("my_meter"), "test fired");
  await t.page.click('[data-scope="global"]');
  await t.clickText("[data-save]", ".*");
  await t.waitFor(() => t.page.$('[data-notif="my_meter"]'), "saved");
  const m = (await global(t)).my_meter;
  assert.deepEqual(m.sequence.kind.steps[0].keys.map((k) => k.replace("name:Key: ", "")), ["F3", "F1", "F4"], "keys kept in picking order (F2 undone)");
  assert.equal(m.sequence.kind.steps[0].params.metric, "cpu.temp");
  assert.deepEqual(m.sequence.kind.steps[0].params.color, { slot: "warn" });
  assert.equal(m.sources.always, true);
  assert.deepEqual(m.sources.every, { every_secs: 120, for_secs: 7 });
  assert.deepEqual({ ...m.sources.threshold }, { metric: "cpu.temp", op: "gt", value: 80, for_secs: 0, hysteresis: 2, cooldown_secs: 60 });
  assert.equal(m.sources.desktop.app, "myapp, other");
  assert.deepEqual(m.sources.ai, { from: "claude", kind: "permission" });
  assert.deepEqual(m.keys.map((k) => k.replace("name:Key: ", "")), ["F3", "F1", "F4"]);
  await t.shot("automations-list-2", 1200, 800);

  // ---- threshold firing (mock sensors: force the value above and below the limit)
  const before = (await t.mockState("triggers")).filter((x) => x.name === "my_meter").length;
  await fns(t, "setMetric", { metric: "cpu.temp", value: 95 });
  await t.waitFor(async () => (await t.mockState("triggers")).filter((x) => x.name === "my_meter").length > before, "threshold fired");
  const fired = (await t.mockState("triggers")).filter((x) => x.name === "my_meter").length;
  await sleep(1200);
  assert.equal((await t.mockState("triggers")).filter((x) => x.name === "my_meter").length, fired, "does not re-fire while above (hysteresis / cooldown)");
  await fns(t, "setMetric", { metric: "cpu.temp", value: null });

  // ---- AI event fires the AI automations
  assert.equal(await fns(t, "fireAi", { from: "claude", kind: "permission" }), 2, "ai_needs_you and my_meter");
  assert.equal(await fns(t, "fireAi", { from: "codex", kind: "idle" }), 1, "only the open rule");

  // ---- desktop sources: Slack, Matrix clients
  await t.page.evaluate(() => (globalThis.__chromaMock.theme.settings.desktop_notifications = true));
  assert.equal(await fns(t, "fireDesktop", { app: "Slack", summary: "hi" }), 1);
  assert.equal(await fns(t, "fireDesktop", { app: "Element Desktop", summary: "hi" }), 1);
  assert.equal(await fns(t, "fireDesktop", { app: "org.gnome.Fractal", summary: "hi" }), 1);
  assert.equal(await fns(t, "fireDesktop", { app: "Telegram Desktop", summary: "hi" }), 1);
  assert.equal(await fns(t, "fireDesktop", { app: "discord", summary: "hi" }), 1);
  assert.equal(await fns(t, "fireDesktop", { app: "Firefox", summary: "hi" }), 0);

  // ---- disabling a monitor stops it
  await t.clickText('[data-notif="gpu_monitor"] .switch', "Enabled");
  await t.waitFor(async () => !(await t.mockState("overlays")).some((o) => o.name === "gpu_monitor" && o.until > Date.now()), "monitor stopped when disabled");

  // ---- Settings: System monitor and Claude Code
  await t.nav("^Settings");
  await t.waitFor(() => t.page.$("[data-sysmon]"), "system monitor section");
  await t.waitFor(async () => /GB · \d+ %/.test(await t.page.$eval('[data-metric-live="gpu.vram"]', (e) => e.textContent)), "live VRAM reading");
  assert.match(await t.page.$eval("[data-sysmon-backend]", (e) => e.textContent), /nvml.*RTX 4070 SUPER.*k10temp Tctl/s);
  const v1 = await t.page.$eval('[data-metric-live="cpu.usage"]', (e) => e.textContent);
  await sleep(2600);
  const v2 = await t.page.$eval('[data-metric-live="cpu.usage"]', (e) => e.textContent);
  assert.notEqual(v1, v2, "live values move");
  await t.page.select("[data-sysmon-gpu]", "amd:card2");
  await t.waitFor(async () => (await theme(t)).settings.sysmon_gpu === "amd:card2", "GPU choice saved");
  await t.page.select("[data-sysmon-gpu]", "auto");
  await t.page.$eval("[data-sysmon]", (e) => e.scrollIntoView({ block: "start" }));
  await t.shot("settings-sysmon", 1200, 800);
  await t.shot("settings-sysmon", 400, 400);

  await t.page.$eval("[data-ai]", (e) => e.scrollIntoView({ block: "start" }));
  assert.match(await t.page.$eval("[data-claude-state]", (e) => e.textContent), /not installed/);
  await t.page.evaluate(() => document.querySelector("[data-ai] [data-claude-details]").setAttribute("open", ""));
  assert.match(await t.page.$eval("[data-claude-snippet]", (e) => e.textContent), /"Notification"[\s\S]*chroma-desk event ai:attention --from claude/);
  await t.shot("settings-claude", 1200, 800);
  await t.shot("settings-claude", 400, 400);
  await t.clickText("[data-ai] [data-claude-install]", ".*");
  await t.waitFor(() => t.page.$("[data-ai] [data-claude-remove]"), "installed");
  let cl = await t.mockState("claude");
  assert.equal(cl.file.theme, "dark", "user settings kept");
  assert.equal(cl.file.hooks.Stop[0].hooks[0].command, "echo done", "user hooks kept");
  assert.equal(cl.file.hooks.Notification[0].hooks[0].command, "chroma-desk event ai:attention --from claude");
  assert.equal(cl.backups.length, 1, "a backup was made");
  assert.match(await t.page.$eval("[data-claude-backup]", (e) => e.textContent), /backup/);
  await t.clickText("[data-ai] [data-claude-remove]", ".*");
  await t.waitFor(() => t.page.$("[data-ai] [data-claude-install]"), "removed");
  cl = await t.mockState("claude");
  assert.equal(cl.file.hooks.Notification, undefined);
  assert.equal(cl.file.hooks.Stop[0].hooks[0].command, "echo done");
  // A broken settings.json is never touched.
  await fns(t, "breakClaudeFile");
  await t.nav("^Automations");
  await t.nav("^Settings");
  await t.waitFor(() => t.page.$("[data-claude-invalid]"), "invalid file warning");
  assert.ok(await t.page.$eval("[data-claude-install]", (e) => e.disabled), "install disabled for an invalid file");
}
