<script lang="ts">
  /** Settings screen. */
  import { getVersion } from "@tauri-apps/api/app";
  import { api, type ServerStatus } from "./api";
  import { t, deviceLabel, getLocale, setLocale, LANGUAGES } from "./i18n/index.svelte";
  import { app, loadDevices, toast, toastError } from "./state.svelte";
  import { commit, sceneStore } from "./scene.svelte";
  import { profiles, saveSettings } from "./profiles.svelte";

  let autostart = $state(false);
  let server: ServerStatus | null = $state(null);
  let version = $state("");
  let defaults: Record<string, number> = $state({});

  const types = $derived([...new Set(app.devices.map((d) => d.device_type))]);

  async function refresh() {
    api.getAutostart().then((v) => (autostart = v)).catch(() => {});
    api.serverStatus().then((s) => (server = s)).catch(() => {});
    getVersion().then((v) => (version = v)).catch(() => {});
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
  <h3>{t("set.folder")}</h3>
  <code>{profiles.path}</code>
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
</style>
