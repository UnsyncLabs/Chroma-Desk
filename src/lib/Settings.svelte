<script lang="ts">
  /** Settings screen. */
  import { getVersion } from "@tauri-apps/api/app";
  import { api, hex, type HookStatus, type ServerStatus, type SysmonStatus, type ThemeStatus } from "./api";
  import { has } from "./capabilities.svelte";
  import ClaudeSetup from "./ClaudeSetup.svelte";
  import { describeLive, metricLabel, sysmon, watchMetrics } from "./sysmon.svelte";
  import { SLOTS, palette } from "./palette.svelte";
  import { t, deviceLabel, getLocale, setLocale, LANGUAGES } from "./i18n/index.svelte";
  import { app, copyText, loadDevices, tour, toast, toastError } from "./state.svelte";
  import { commit, sceneStore } from "./scene.svelte";
  import { profiles, saveSettings } from "./profilesStore.svelte";

  let autostart = $state(false);
  let server: ServerStatus | null = $state(null);
  let version = $state("");
  let defaults: Record<string, number> = $state({});
  let socket: string | null = $state(null);

  const CLI_EXAMPLE = 'chroma-desk trigger "Alert" --keys W,A,S,D --duration 2s';
  const sockPath = $derived(socket ?? "$XDG_RUNTIME_DIR/chroma-desk.sock");
  const SOCKET_EXAMPLE = $derived(`echo '{"animation":"Alert","keys":["W","A","S","D"],"duration":"2s"}' | socat - UNIX-CONNECT:${sockPath}`);

  async function testAlert() {
    try {
      const r = await api.trigger({ animation: "Alert", keys: ["W", "A", "S", "D"], duration: 2 });
      toast(t("anim.tested", { n: r.devices }), "ok", 2500);
    } catch (e) {
      toastError(e);
    }
  }
  async function stopAll() {
    try {
      const n = await api.stopTriggers();
      toast(t("trg.stopped", { n }), "ok", 2500);
    } catch (e) {
      toastError(e);
    }
  }

  let mon: SysmonStatus | null = $state(null);
  async function loadMon() {
    mon = await api.sysmonStatus().catch(() => null);
  }
  async function chooseGpu(id: string) {
    profiles.settings.sysmon_gpu = id;
    await saveSettings();
    await loadMon();
  }
  $effect(() => {
    loadMon();
    // The readings below are live while the page is open.
    return watchMetrics(["cpu.usage", "cpu.temp", "gpu.usage", "gpu.vram", "gpu.temp", "ram.usage"]);
  });
  const gb = (b: number | null) => (b ? `${(b / 1073741824).toFixed(b >= 10 * 1073741824 ? 0 : 1)} GB` : "");

  const types = $derived([...new Set(app.devices.map((d) => d.device_type))]);

  async function refresh() {
    api.getAutostart().then((v) => (autostart = v)).catch(() => {});
    api.serverStatus().then((s) => (server = s)).catch(() => {});
    getVersion().then((v) => (version = v)).catch(() => {});
    api.triggerSocketPath().then((p) => (socket = p)).catch(() => {});
  }

  $effect(() => {
    refresh();
  });
  $effect(() => {
    if (types.length) api.defaultFps(types).then((d) => (defaults = d)).catch(() => {});
  });

  function changeLanguage(code: string) {
    setLocale(code);
    profiles.settings.language = code;
    saveSettings();
  }

  async function toggleAutostart(on: boolean) {
    try {
      await api.setAutostart(on);
      autostart = on;
    } catch (e) {
      autostart = !on;
      toastError(e);
    }
  }

  let adminBusy = $state(false);
  async function adminAction(f: () => Promise<void>, ok: string) {
    adminBusy = true;
    try {
      await f();
      toast(t(ok), "ok");
    } catch (e) {
      toastError(e);
    } finally {
      adminBusy = false;
      server = await api.serverStatus().catch(() => server);
      autostart = await api.getAutostart().catch(() => autostart);
    }
  }

  async function toggleService(on: boolean) {
    try {
      await api.setServerService(on);
      server = await api.serverStatus();
      toast(t("set.serviceChanged"), "ok");
    } catch (e) {
      toastError(e);
      server = await api.serverStatus().catch(() => server);
    }
  }

  function setFps(type: string, v: string) {
    const n = Number(v);
    const next = { ...profiles.settings.fps_by_type };
    if (v === "" || !(n >= 1)) delete next[type];
    else next[type] = Math.min(60, Math.round(n));
    profiles.settings.fps_by_type = next;
    saveSettings();
  }

  // ---------- themes ----------
  let themeStat = $state<ThemeStatus | null>(null);
  let hook = $state<HookStatus | null>(null);
  let ruleTheme = $state("");
  let ruleProfile = $state("");
  const omarchy = $derived(themeStat?.sources.find((x) => x.id === "omarchy"));
  const userProfiles = $derived(profiles.list.profiles.filter((p) => p.enabled));

  $effect(() => {
    palette.generation;
    api.themeStatus().then((x) => (themeStat = x)).catch(() => {});
  });
  $effect(() => {
    api.omarchyHookStatus().then((h) => (hook = h)).catch(() => {});
  });

  function addRule() {
    const theme = ruleTheme.trim();
    const profile = ruleProfile || userProfiles[0]?.name;
    if (!theme || !profile) return;
    profiles.settings.theme_rules = [...profiles.settings.theme_rules.filter((r) => r.theme.toLowerCase() !== theme.toLowerCase()), { theme, profile }];
    ruleTheme = "";
    saveSettings();
  }
  function removeRule(theme: string) {
    profiles.settings.theme_rules = profiles.settings.theme_rules.filter((r) => r.theme !== theme);
    saveSettings();
  }
  async function toggleHook(install: boolean) {
    try {
      hook = await (install ? api.omarchyHookInstall() : api.omarchyHookRemove());
      toast(t(install ? "themes.hookInstalled" : "themes.hookRemoved"), "ok");
    } catch (e) {
      toastError(e);
    }
  }

  let restarting = $state(false);
  async function restartOpenrgb() {
    restarting = true;
    try {
      await api.restartOpenrgb();
      await reconnect();
      toast(t("set.restarted"), "ok");
    } catch (e) {
      toastError(e);
    } finally {
      restarting = false;
    }
  }

  async function reconnect() {
    await loadDevices(api.connect);
    await refresh();
  }
</script>

<h2>{t("nav.settings")}</h2>

<section>
  <h3>{t("settings.language")}</h3>
  <select value={getLocale()} onchange={(e) => changeLanguage(e.currentTarget.value)} aria-label={t("settings.language")}>
    {#each LANGUAGES as l (l.code)}<option value={l.code}>{l.name}</option>{/each}
  </select>
</section>

<section>
  <h3>{t("set.startup")}</h3>
  <label class="check"><input type="checkbox" checked={autostart} onchange={(e) => toggleAutostart(e.currentTarget.checked)} /> {t("set.autostart")}</label>
  <p class="muted small">{t("set.autostartHint")}</p>
  <label class="check">
    <input type="checkbox" checked={profiles.settings.start_hidden} onchange={(e) => { profiles.settings.start_hidden = e.currentTarget.checked; saveSettings(); }} />
    {t("set.startHidden")}
  </label>
</section>

<section>
  <h3>{t("set.devices")}</h3>
  <label class="check">
    <input type="checkbox" checked={profiles.settings.show_phantom_devices} onchange={async (e) => { profiles.settings.show_phantom_devices = e.currentTarget.checked; await saveSettings(); await loadDevices(api.listDevices); }} />
    {t("set.showPhantoms")}
  </label>
  <p class="muted small">{t("set.showPhantomsHint")}</p>
</section>

<section>
  <h3>{t("all.master")}</h3>
  <label class="check">
    <input type="range" min="0" max="1" step="0.01" value={sceneStore.scene.master_brightness} oninput={(e) => { sceneStore.scene.master_brightness = Number(e.currentTarget.value); commit(); }} />
    {Math.round(sceneStore.scene.master_brightness * 100)}%
  </label>
</section>

<section>
  <h3>{t("set.fps")}</h3>
  <p class="muted small">{t("set.fpsHint")}</p>
  <div class="fps">
    {#each types as ty (ty)}
      <label>
        <span>{deviceLabel(ty)}</span>
        <input type="number" min="1" max="60" placeholder={String(defaults[ty] ?? "")} value={profiles.settings.fps_by_type[ty] ?? ""} onchange={(e) => setFps(ty, e.currentTarget.value)} />
      </label>
    {/each}
  </div>
</section>

<section>
  <h3>{t("set.server")}</h3>
  <p>
    {server?.running ? t("set.serverRunning") : t("set.serverStopped")}
    {#if server?.protocol}<span class="muted"> · {t("set.protocol", { n: server.protocol })}</span>{/if}
  </p>
  {#if server?.gui_open}<p class="warn">{t("status.guiOpen")}</p>{/if}
  <button onclick={reconnect}>{t("set.reconnect")}</button>
  <button onclick={restartOpenrgb} disabled={restarting} title={t("set.restartHint")}>{restarting ? t("set.restarting") : t("set.restart")}</button>
  {#if server?.can_install}
    <h4>{t("set.win.title")}</h4>
    <p>
      {t("set.win.pawnio")}: <b>{server.pawnio ? t("set.win.yes") : t("set.win.no")}</b>
      {#if !server.pawnio}<button disabled={adminBusy} onclick={() => adminAction(api.installPawnio, "set.win.pawnioDone")}>{adminBusy ? t("common.loading") : t("set.win.installPawnio")}</button>{/if}
    </p>
    <p class="muted small">{t("set.win.pawnioHint")}</p>
    <label class="check">
      <input type="checkbox" checked={server.admin_autostart} disabled={adminBusy} onchange={(e) => { const on = e.currentTarget.checked; adminAction(() => api.setAdminAutostart(on), "set.serviceChanged"); }} />
      {t("set.win.adminAutostart")}
    </label>
    <p class="muted small">{t("set.win.adminHint")}</p>
    <p>
      {t("set.win.elevated")}: <b>{server.elevated ? t("set.win.yes") : t("set.win.no")}</b>
      {#if server.admin_autostart && !server.elevated}<button disabled={adminBusy} onclick={() => api.relaunchElevated().catch(toastError)}>{t("set.win.relaunch")}</button>{/if}
    </p>
  {/if}
  {#if has("systemd_service")}
    <label class="check">
      <input type="checkbox" checked={server?.service_enabled ?? false} onchange={(e) => toggleService(e.currentTarget.checked)} />
      {t("set.service")}
    </label>
    <p class="muted small">{t("set.serviceHint")}</p>
  {/if}
</section>

<section data-themes>
  <h3>{t("themes.title")}</h3>
  <p class="muted small">{t("themes.hint")}</p>
  <h4>{t("themes.current")}</h4>
  <p>
    <b data-palette-name>{palette.name}</b>
    <span class="muted"> · {t(`src.${palette.source}`)}{palette.external ? ` · ${t("themes.external")}` : ""}</span>
  </p>
  {#if palette.error}<p class="warn small">{t("themes.error", { detail: palette.error })}</p>{/if}
  <div class="pal" data-palette>
    {#each SLOTS as s (s)}
      <span class="sw" data-swatch={s} style:background={hex(palette.slots[s] ?? [0, 0, 0])} title={`@${s} ${hex(palette.slots[s] ?? [0, 0, 0])}`}></span>
    {/each}
  </div>

  <h4>{t("themes.rules")}</h4>
  <p class="muted small">{t("themes.rulesHint")}</p>
  {#each profiles.settings.theme_rules as r (r.theme)}
    <div class="rule" data-rule={r.theme}>
      <span>{t("themes.ruleLine")} <b>{r.theme}</b> → <b>{r.profile}</b></span>
      <button onclick={() => removeRule(r.theme)} aria-label={t("common.delete")}>✕</button>
    </div>
  {/each}
  <div class="rule add">
    <input type="text" bind:value={ruleTheme} placeholder={omarchy?.name ?? "tokyo-night"} list="omarchy-themes" aria-label={t("themes.ruleTheme")} />
    <datalist id="omarchy-themes">{#if omarchy?.name}<option value={omarchy.name}></option>{/if}</datalist>
    <span>→</span>
    <select bind:value={ruleProfile} aria-label={t("themes.ruleProfile")}>
      {#each userProfiles as p (p.name)}<option value={p.name}>{p.name}</option>{/each}
    </select>
    <button onclick={addRule} disabled={!ruleTheme.trim() || !userProfiles.length}>{t("themes.addRule")}</button>
  </div>

  {#if has("omarchy")}
  <h4>{t("themes.hook")}</h4>
  {#if hook?.available}
    <p class="muted small">{t("themes.hookHint")}</p>
    <div class="cmd"><code data-hook-command>{hook.command}</code><button onclick={() => copyText(hook?.command ?? "", t("trg.copied"))}>{t("trg.copy")}</button></div>
    <p class="small">{hook.installed ? t("themes.hookIs", { path: hook.path }) : t("themes.hookNot")}</p>
    <div class="row">
      {#if hook.installed}
        <button onclick={() => toggleHook(false)} data-hook="remove">{t("themes.hookRemove")}</button>
      {:else}
        <button class="primary" onclick={() => toggleHook(true)} data-hook="install">{t("themes.hookInstall")}</button>
      {/if}
    </div>
  {:else}
    <p class="muted small">{t("themes.noOmarchy")}</p>
  {/if}
  {/if}
</section>

<section>
  <h3>{t("trg.title")}</h3>
  <p class="muted small">{t("trg.hint")}</p>
  <h4>{t("trg.cli")}</h4>
  <div class="cmd"><code>{CLI_EXAMPLE}</code><button onclick={() => copyText(CLI_EXAMPLE, t("trg.copied"))}>{t("trg.copy")}</button></div>
  <h4>{t("trg.socket")}</h4>
  {#if has("unix_socket")}
    <div class="cmd"><code>{SOCKET_EXAMPLE}</code><button onclick={() => copyText(SOCKET_EXAMPLE, t("trg.copied"))}>{t("trg.copy")}</button></div>
  {:else}
    <div class="cmd"><code>{sockPath}</code><button onclick={() => copyText(sockPath, t("trg.copied"))}>{t("trg.copy")}</button></div>
  {/if}
  <p class="muted small">{t("trg.socketHint")}</p>
  <div class="row">
    <button onclick={testAlert}>{t("trg.test")}</button>
    <button onclick={stopAll}>{t("trg.stop")}</button>
  </div>
</section>

<section data-sysmon>
  <h3>{t("mon.title")}</h3>
  <p class="muted small">{t("mon.lead")}</p>
  {#if mon}
    <label class="inline">{t("mon.gpu")}
      <select value={profiles.settings.sysmon_gpu || "auto"} onchange={(e) => chooseGpu(e.currentTarget.value)} data-sysmon-gpu aria-label={t("mon.gpu")}>
        <option value="auto">{t("mon.auto")}</option>
        {#each mon.gpus as g (g.id)}<option value={g.id}>{g.name}{g.vram_total ? ` (${gb(g.vram_total)})` : ""}</option>{/each}
      </select>
    </label>
    <p class="small" data-sysmon-backend>
      {t("mon.backend")}: <b>{mon.gpu_backend === "none" ? t("mon.none") : mon.gpu_backend}</b>
      {#if mon.selected}· {t("mon.using")}: <b>{mon.gpus.find((g) => g.id === mon?.selected)?.name}</b>{/if}
      · {t("mon.cpuSensor")}: <b>{mon.cpu_temp_sensor ?? t("mon.none")}</b>
    </p>
    <ul class="metrics" data-sysmon-metrics>
      {#each mon.metrics as m (m.id)}
        <li class:off={!m.available}><span>{metricLabel(m.id)}</span><b data-metric-live={m.id}>{m.available ? describeLive(m.id, sysmon.values[m.id]) : t("mon.unavailable")}</b></li>
      {/each}
    </ul>
    <p class="muted small">{t("mon.cost")}</p>
  {/if}
</section>

<section data-ai>
  <h3>{t("claude.title")}</h3>
  <ClaudeSetup />
</section>

<section>
  <h3>{t("set.folder")}</h3>
  <code>{profiles.path}</code>
</section>

<section>
  <h3>{t("tour.settings")}</h3>
  <button onclick={() => (tour.open = true)}>{t("tour.replay")}</button>
</section>

<section>
  <h3>{t("set.about")}</h3>
  <p class="muted">chroma-desk {version} · {t("set.aboutText")}</p>
</section>

<style>
  section { margin-bottom: 1.4rem; }
  .check { display: flex; align-items: center; gap: 0.5rem; margin: 0.3rem 0; }
  .small { font-size: 0.82rem; margin: 0.2rem 0 0.6rem; }
  .fps { display: grid; grid-template-columns: repeat(auto-fill, minmax(13rem, 1fr)); gap: 0.4rem 1rem; }
  .fps label { display: flex; justify-content: space-between; align-items: center; gap: 0.5rem; }
  .fps input { width: 5rem; }
  .warn { color: var(--danger); }
  h4 { margin: 0.8rem 0 0.3rem; font-size: 0.92rem; }
  .cmd { display: flex; gap: 0.5rem; align-items: flex-start; }
  .cmd code { flex: 1; min-width: 0; overflow-wrap: anywhere; background: var(--panel); border: 1px solid var(--border); border-radius: 6px; padding: 0.4rem 0.6rem; font-size: 0.82rem; }
  .row { display: flex; gap: 0.5rem; flex-wrap: wrap; margin-top: 0.6rem; }
  .pal { display: flex; flex-wrap: wrap; gap: 0.25rem; }
  .sw { width: 1.9rem; height: 1.9rem; border-radius: 6px; border: 1px solid var(--border); }
  .rule { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; margin: 0.3rem 0; }
  .metrics { list-style: none; margin: 0.4rem 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(15rem, 1fr)); gap: 0.3rem 1rem; }
  .metrics li { display: flex; justify-content: space-between; gap: 0.6rem; background: var(--panel-2); border-radius: 8px; padding: 0.3rem 0.6rem; font-size: 0.88rem; }
  .metrics li.off { opacity: 0.5; }
  .metrics b { font-variant-numeric: tabular-nums; }
</style>
