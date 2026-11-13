<script lang="ts">
  /** Settings screen. */
  import { getVersion } from "@tauri-apps/api/app";
  import { api, type ServerStatus } from "./api";
  import { t, deviceLabel, getLocale, setLocale, LANGUAGES } from "./i18n/index.svelte";
  import { app, copyText, loadDevices, tour, toast, toastError } from "./state.svelte";
  import { commit, sceneStore } from "./scene.svelte";
  import { profiles, saveSettings } from "./profiles.svelte";

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
  <label class="check">
    <input type="checkbox" checked={server?.service_enabled ?? false} onchange={(e) => toggleService(e.currentTarget.checked)} />
    {t("set.service")}
  </label>
  <p class="muted small">{t("set.serviceHint")}</p>
</section>

<section>
  <h3>{t("trg.title")}</h3>
  <p class="muted small">{t("trg.hint")}</p>
  <h4>{t("trg.cli")}</h4>
  <div class="cmd"><code>{CLI_EXAMPLE}</code><button onclick={() => copyText(CLI_EXAMPLE, t("trg.copied"))}>{t("trg.copy")}</button></div>
  <h4>{t("trg.socket")}</h4>
  <div class="cmd"><code>{SOCKET_EXAMPLE}</code><button onclick={() => copyText(SOCKET_EXAMPLE, t("trg.copied"))}>{t("trg.copy")}</button></div>
  <p class="muted small">{t("trg.socketHint")}</p>
  <div class="row">
    <button onclick={testAlert}>{t("trg.test")}</button>
    <button onclick={stopAll}>{t("trg.stop")}</button>
  </div>
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
</style>
