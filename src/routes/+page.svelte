<script lang="ts">
  import { onMount } from "svelte";
  import { api, hex, on, type Device, type Rgb } from "$lib/api";
  import { t, deviceLabel, getLocale } from "$lib/i18n/index.svelte";
  import { app, loadDevices, maybeStartTour, navigate, rescan } from "$lib/state.svelte";
  import { loadAnimations } from "$lib/animations.svelte";
  import Animations from "$lib/Animations.svelte";
  import { frames, isDirty, listenEngine, loadEngine, sceneStore, syncPositions } from "$lib/scene.svelte";
  import { loadProfiles, profiles } from "$lib/profiles.svelte";
  import { listenPalette, loadPalette } from "$lib/palette.svelte";
  import { toastError } from "$lib/state.svelte";
  import Profiles from "$lib/Profiles.svelte";
  import Notifications from "$lib/Notifications.svelte";
  import AllDevices from "$lib/AllDevices.svelte";
  import DeviceView from "$lib/DeviceView.svelte";
  import Settings from "$lib/Settings.svelte";
  import Toasts from "$lib/Toasts.svelte";
  import Confirm from "$lib/Confirm.svelte";
  import Tour from "$lib/Tour.svelte";

  let guiOpen = $state(false);

  const device = $derived(app.view.kind === "device" ? (app.devices.find((d) => d.key === (app.view as { key: string }).key) ?? null) : null);

  async function start() {
    await api.waitStartup();
    await loadProfiles();
    await loadPalette();
    loadAnimations();
    await loadDevices(api.connect);
    if (app.error) return;
    await loadEngine();
    syncPositions(app.devices);
    api.serverStatus().then((s) => (guiOpen = s.gui_open)).catch(() => {});
    maybeStartTour();
    // Dev aid (screenshots): `#d=<index>` opens that device.
    const m = import.meta.env.DEV ? location.hash.match(/^#d=(\d+)/) : null;
    if (import.meta.env.DEV && location.hash === "#profiles") app.view = { kind: "profiles" };
    if (import.meta.env.DEV && location.hash === "#notifications") app.view = { kind: "notifications" };
    if (import.meta.env.DEV && location.hash === "#animations") app.view = { kind: "animations" };
    if (m && app.devices[Number(m[1])]) app.view = { kind: "device", key: app.devices[Number(m[1])].key };
  }

  onMount(() => {
    let off = () => {};
    start();
    // A profile applied from the tray changes `current_profile` behind the UI's back.
    const offScene = on<unknown>("scene-changed", () => loadProfiles());
    // A theme rule or a script switched the profile behind the UI's back: refresh the list and the tray.
    const offApplied = on<unknown>("profile-applied", () => loadProfiles().then(() => api.refreshTray().catch(() => {})));
    const offPalette = listenPalette((msg) => toastError({ code: "palette_source", detail: msg }));
    listenEngine(async () => {
      await loadDevices(api.listDevices);
      syncPositions(app.devices);
    }).then((f) => (off = f));
    return () => {
      off();
      offScene.then((f) => f());
      offApplied.then((f) => f());
      offPalette.then((f) => f());
    };
  });

  // Keep the tray menu in the UI language and in sync with the profile list.
  $effect(() => {
    getLocale();
    profiles.list;
    api.setTrayLabels({
      show: t("tray.show"),
      profiles: t("nav.profiles"),
      no_profiles: t("tray.noProfiles"),
      lights_off: t("all.lightsOff"),
      lights_on: t("all.lightsOn"),
      quit: t("tray.quit"),
    }).catch(() => {});
  });

  // Tell the engine which devices are on screen: only those get live preview frames.
  $effect(() => {
    const keys = app.view.kind === "all" ? app.devices.map((d) => d.key) : app.view.kind === "device" ? [app.view.key] : [];
    api.setVisibleDevices(keys).catch(() => {});
  });

  /** Representative colour of a device for the sidebar dot. */
  function dotColor(d: Device): string {
    const cols: Rgb[] = frames.map[d.key] ?? d.colors;
    const lit = cols.find((c) => c[0] + c[1] + c[2] > 40) ?? cols[0] ?? [40, 40, 40];
    return hex(lit);
  }

  const go = (v: typeof app.view) => navigate(v);
</script>

<div class="app">
  <aside>
    <header><h1>chroma<span>desk</span></h1></header>
    <nav aria-label={t("nav.devices")}>
      <button class="dev" class:active={app.view.kind === "all"} onclick={() => go({ kind: "all" })}>
        <span class="dot all"></span>
        <span class="name">{t("nav.all")}</span>
        <small>{t("nav.allHint", { n: app.devices.length })}</small>
      </button>
      {#if app.loading && !app.devices.length}
        {#each [0, 1, 2, 3] as i (i)}<div class="skeleton" aria-hidden="true"></div>{/each}
      {/if}
      {#each app.devices as d (d.key)}
        <button class="dev" class:active={device?.key === d.key} onclick={() => go({ kind: "device", key: d.key })}>
          <span class="dot" style:background={dotColor(d)}></span>
          <span class="name">{d.name}</span>
          <small>{deviceLabel(d.device_type)} · {d.colors.length} LEDs</small>
        </button>
      {/each}
    </nav>
    <footer>
      <button class="dev" class:active={app.view.kind === "animations"} onclick={() => go({ kind: "animations" })}>
        <span class="name">{t("nav.animations")}</span>
        <small>{t("nav.animationsHint")}</small>
      </button>
      <button class="dev" class:active={app.view.kind === "notifications"} onclick={() => go({ kind: "notifications" })} data-nav="notifications">
        <span class="name">{t("nav.notifications")}</span>
        <small>{t("nav.notificationsHint")}</small>
      </button>
      <button class="dev" class:active={app.view.kind === "profiles"} onclick={() => go({ kind: "profiles" })}>
        <span class="name">{t("nav.profiles")}{#if isDirty()}<span class="dirty" title={t("prof.unsaved")}> ●</span>{/if}</span>
        <small>{profiles.settings.current_profile ?? t("prof.none")}</small>
      </button>
      <button class="dev" class:active={app.view.kind === "settings"} onclick={() => go({ kind: "settings" })}>
        <span class="name">{t("nav.settings")}</span>
      </button>
      <button onclick={rescan} disabled={app.loading}>{app.loading ? t("common.loading") : t("nav.rescan")}</button>
    </footer>
  </aside>

  <main>
    {#if sceneStore.status !== "ok"}
      <p class="banner warn" role="status">{t(sceneStore.status === "server_down" ? "status.serverDown" : "status.reconnecting")}</p>
    {/if}
    {#if guiOpen}
      <p class="banner warn" role="status">
        {t("status.guiOpen")}
        <button onclick={() => (guiOpen = false)}>{t("common.dismiss")}</button>
      </p>
    {/if}
    {#if sceneStore.blackout}
      <p class="banner" role="status">
        {t("all.blackoutOn")}
        <button onclick={async () => { await api.setBlackout(false); sceneStore.blackout = false; }}>{t("all.lightsOn")}</button>
      </p>
    {/if}

    {#if app.error}
      <div class="empty">
        <h2>{t("empty.errorTitle")}</h2>
        <p class="error">{app.error}</p>
        <p class="muted">{t("empty.errorHint")}</p>
        <button class="primary" onclick={start}>{t("common.retry")}</button>
      </div>
    {:else if app.loading && !app.devices.length}
      <div class="skeleton big" aria-hidden="true"></div>
      <p class="muted">{t("common.connecting")}</p>
    {:else if app.view.kind === "settings"}
      <Settings />
    {:else if app.view.kind === "animations"}
      <Animations />
    {:else if app.view.kind === "profiles"}
      <Profiles />
    {:else if app.view.kind === "notifications"}
      <Notifications />
    {:else if !app.devices.length}
      <div class="empty">
        <h2>{t("empty.noDevices")}</h2>
        <p class="muted">{t("empty.noDevicesHint")}</p>
        <button class="primary" onclick={rescan}>{t("nav.rescan")}</button>
      </div>
    {:else if device}
      {#key device.key}<DeviceView {device} />{/key}
    {:else}
      <AllDevices />
    {/if}
  </main>
</div>

<Toasts />
<Confirm />
<Tour />

<style>
  .app { display: grid; grid-template-columns: 17rem minmax(0, 1fr); height: 100vh; }
  aside { background: var(--panel); display: flex; flex-direction: column; border-right: 1px solid var(--border); min-height: 0; }
  header h1 { font-size: 1.25rem; margin: 1rem; letter-spacing: 0.02em; }
  header h1 span { color: var(--accent); }
  nav { flex: 1; overflow-y: auto; padding: 0 0.5rem; }
  .dev {
    display: grid; grid-template-columns: 0.8rem 1fr; column-gap: 0.6rem; width: 100%;
    text-align: left; margin-bottom: 0.25rem; background: transparent; border-color: transparent;
  }
  .dev.active { background: var(--control); border-color: var(--border); }
  .dev .dot { grid-row: span 2; align-self: center; width: 0.8rem; height: 0.8rem; border-radius: 50%; }
  .dev .dot.all { background: conic-gradient(red, yellow, lime, cyan, blue, magenta, red); }
  .dev .name { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; grid-column: 2; }
  .dev small { color: var(--muted); grid-column: 2; }
  footer { padding: 0.6rem 0.5rem 0.8rem; display: flex; flex-direction: column; gap: 0.4rem; }
  footer .dev { grid-template-columns: 1fr; margin: 0; }
  .dirty { color: var(--danger); }
  footer .dev .name, footer .dev small { grid-column: 1; }
  main { padding: 1.2rem 1.6rem; overflow-y: auto; min-width: 0; }
  .banner { background: var(--panel-2); border-radius: 8px; padding: 0.5rem 0.8rem; display: flex; gap: 0.6rem; align-items: center; margin: 0 0 0.8rem; flex-wrap: wrap; }
  .banner.warn { border-left: 4px solid var(--danger); }
  .empty { max-width: 28rem; margin: 3rem auto; text-align: center; }
  .error { color: var(--danger); }
  .skeleton { height: 2.6rem; margin: 0 0 0.25rem; border-radius: 6px; background: linear-gradient(90deg, var(--panel-2), var(--control), var(--panel-2)); background-size: 200% 100%; animation: shimmer 1.4s infinite linear; }
  .skeleton.big { height: 16rem; }
  @keyframes shimmer { to { background-position: -200% 0; } }
  @media (prefers-reduced-motion: reduce) { .skeleton { animation: none; } }
  @media (max-width: 760px) {
    .app { grid-template-columns: minmax(0, 1fr); grid-template-rows: auto minmax(0, 1fr); }
    aside { border-right: none; border-bottom: 1px solid var(--border); display: grid; grid-template-columns: 1fr auto; align-items: center; }
    header h1 { margin: 0.5rem 0.8rem; font-size: 1.05rem; }
    nav { grid-column: 1 / -1; grid-row: 2; display: flex; gap: 0.3rem; overflow-x: auto; padding: 0.3rem 0.5rem 0.5rem; }
    .dev { width: auto; min-width: 9rem; max-width: 13rem; margin: 0; }
    footer { flex-direction: row; padding: 0.3rem 0.5rem; grid-row: 1; grid-column: 2; }
    main { padding: 0.8rem; }
  }
  @media (max-height: 420px) { header { display: none; } }
</style>
