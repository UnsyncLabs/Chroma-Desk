<script lang="ts">
  import { onMount } from "svelte";
  import { api, hex, fromHex, type Device } from "$lib/api";
  import { t, deviceLabel } from "$lib/i18n/index.svelte";
  import { app, loadDevices, rescan, toast, toastError, toasts, dismissToast } from "$lib/state.svelte";
  import DirectPaint from "$lib/DirectPaint.svelte";
  import Hardware from "$lib/Hardware.svelte";
  import Advanced from "$lib/Advanced.svelte";

  let selectedKey: string | null = $state(null);
  let tab: "color" | "paint" | "hardware" | "advanced" = $state("paint");
  let globalColor = $state("#da1d1d");
  let deviceColor = $state("#da1d1d");

  const device = $derived(app.devices.find((d) => d.key === selectedKey) ?? app.devices[0] ?? null);

  onMount(async () => {
    await loadDevices(api.connect);
  });

  async function applyGlobal() {
    const errs = await api.setColorAll(fromHex(globalColor)).catch((e) => [String(e)]);
    if (errs.length) toast(errs.join(" · "), "error");
    else toast(t("all.applied"), "ok");
    await loadDevices();
  }

  async function applyDevice() {
    if (!device) return;
    await api.setDeviceColor(device.id, fromHex(deviceColor)).catch(toastError);
    await loadDevices();
  }

  function select(d: Device) {
    selectedKey = d.key;
    deviceColor = hex(d.colors[0] ?? [255, 255, 255]);
  }
</script>

<div class="app">
  <aside>
    <header><h1>chroma<span>desk</span></h1></header>
    <div class="global">
      <input type="color" bind:value={globalColor} aria-label={t("all.color")} />
      <button onclick={applyGlobal} disabled={app.loading}>{t("all.apply")}</button>
    </div>
    <nav aria-label={t("nav.devices")}>
      {#each app.devices as d (d.key)}
        <button class="dev" class:active={d.key === device?.key} onclick={() => select(d)}>
          <span class="dot" style:background={hex(d.colors[0] ?? [40, 40, 40])}></span>
          <span class="name">{d.name}</span>
          <small>{deviceLabel(d.device_type)} · {d.colors.length} LEDs</small>
        </button>
      {/each}
    </nav>
    <footer>
      <button onclick={rescan} disabled={app.loading}>{app.loading ? t("common.loading") : t("nav.rescan")}</button>
    </footer>
  </aside>

  <main>
    {#if app.error}
      <div class="error">
        <p>{app.error}</p>
        <button onclick={() => loadDevices(api.connect)}>{t("common.retry")}</button>
      </div>
    {:else if app.loading && !app.devices.length}
      <p class="muted">{t("common.connecting")}</p>
    {:else if device}
      <div class="title">
        <h2>{device.name}</h2>
        <small class="muted">{device.vendor} · {deviceLabel(device.device_type)}</small>
      </div>
      <div class="tabs" role="tablist">
        {#each ["color", "paint", "hardware", "advanced"] as const as k (k)}
          <button role="tab" aria-selected={tab === k} class:on={tab === k} onclick={() => (tab = k)}>{t(`tab.${k}`)}</button>
        {/each}
      </div>

      {#if tab === "color"}
        <div class="simple">
          <input type="color" bind:value={deviceColor} aria-label={t("all.color")} />
          <button class="primary" onclick={applyDevice}>{t("dev.applyTo", { name: device.name })}</button>
        </div>
      {:else if tab === "paint"}
        {#key device.key}<DirectPaint {device} />{/key}
      {:else if tab === "hardware"}
        {#key device.key}<Hardware {device} />{/key}
      {:else}
        {#key device.key}<Advanced {device} />{/key}
      {/if}
    {/if}
  </main>
</div>

<div class="toasts" aria-live="polite">
  {#each toasts.list as x (x.id)}
    <button class="toast {x.kind}" onclick={() => dismissToast(x.id)}>{x.text}</button>
  {/each}
</div>

<style>
  .app { display: grid; grid-template-columns: 17rem 1fr; height: 100vh; }
  aside { background: var(--panel); display: flex; flex-direction: column; border-right: 1px solid var(--border); min-height: 0; }
  header h1 { font-size: 1.25rem; margin: 1rem; letter-spacing: 0.02em; }
  header h1 span { color: var(--accent); }
  .global { display: flex; gap: 0.5rem; align-items: center; padding: 0 1rem 0.8rem; }
  .global input { width: 2.2rem; height: 1.9rem; padding: 0; border: none; }
  nav { flex: 1; overflow-y: auto; padding: 0 0.5rem; }
  .dev {
    display: grid; grid-template-columns: 0.8rem 1fr; column-gap: 0.6rem; width: 100%;
    text-align: left; margin-bottom: 0.25rem; background: transparent; border-color: transparent;
  }
  .dev.active { background: var(--control); border-color: var(--border); }
  .dev .dot { grid-row: span 2; align-self: center; width: 0.8rem; height: 0.8rem; border-radius: 50%; }
  .dev .name { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dev small { color: var(--muted); }
  footer { padding: 0.8rem 1rem; }
  main { padding: 1.2rem 1.6rem; overflow-y: auto; }
  .tabs { display: flex; gap: 0.3rem; margin: 1rem 0; flex-wrap: wrap; }
  .tabs .on { background: var(--accent); color: var(--accent-fg); }
  .simple { display: flex; gap: 0.6rem; align-items: center; }
  .simple input { width: 3rem; height: 2.2rem; padding: 0; border: none; }
  @media (max-width: 760px) {
    .app { grid-template-columns: 1fr; grid-template-rows: auto 1fr; }
    aside { border-right: none; border-bottom: 1px solid var(--border); display: grid; grid-template-columns: auto 1fr auto; align-items: center; }
    header h1 { margin: 0.5rem 0.8rem; font-size: 1.05rem; }
    .global { padding: 0 0.5rem; }
    nav { grid-column: 1 / -1; display: flex; gap: 0.3rem; overflow-x: auto; padding: 0.3rem 0.5rem 0.5rem; }
    .dev { width: auto; min-width: 9rem; max-width: 13rem; margin: 0; }
    footer { padding: 0.3rem 0.5rem; grid-row: 1; grid-column: 3; }
    main { padding: 0.8rem; }
  }
  @media (max-height: 420px) { header { display: none; } }
  .error { color: var(--danger); }
  .toasts { position: fixed; right: 1rem; bottom: 1rem; display: flex; flex-direction: column; gap: 0.4rem; max-width: min(28rem, calc(100vw - 2rem)); z-index: 10; }
  .toast { text-align: left; background: var(--panel); border-left: 4px solid var(--accent); }
  .toast.error { border-left-color: var(--danger); }
  .toast.ok { border-left-color: var(--ok); }
</style>
