<script lang="ts">
  import { onMount } from "svelte";
  import { api, hex, fromHex, DEVICE_LABEL, type Device } from "$lib/api";
  import Keyboard from "$lib/Keyboard.svelte";
  import Advanced from "$lib/Advanced.svelte";

  let devices: Device[] = $state([]);
  let selectedId: number | null = $state(null);
  let tab: "color" | "keyboard" | "advanced" = $state("color");
  let loading = $state(true);
  let error = $state("");
  let globalColor = $state("#da1d1d");
  let deviceColor = $state("#da1d1d");
  let notice = $state("");

  const device = $derived(devices.find((d) => d.id === selectedId) ?? null);
  const hasMatrix = $derived(!!device?.zones.some((z) => z.matrix));

  async function load(fn: () => Promise<Device[]>) {
    loading = true;
    error = "";
    try {
      devices = await fn();
      if (selectedId === null || !devices.some((d) => d.id === selectedId)) selectedId = devices[0]?.id ?? null;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => load(api.connect));

  async function applyGlobal() {
    notice = "";
    const errs = await api.setColorAll(fromHex(globalColor)).catch((e) => [String(e)]);
    notice = errs.length ? errs.join(" · ") : "Color aplicado a todos los dispositivos.";
    await load(api.listDevices);
  }

  async function applyDevice() {
    if (!device) return;
    notice = "";
    await api.setDeviceColor(device.id, fromHex(deviceColor)).catch((e) => (notice = String(e)));
    await load(api.listDevices);
  }

  function select(d: Device) {
    selectedId = d.id;
    if (tab === "keyboard" && !d.zones.some((z) => z.matrix)) tab = "color";
    deviceColor = hex(d.colors[0] ?? [255, 255, 255]);
  }
</script>

<div class="app">
  <aside>
    <header>
      <h1>chroma<span>desk</span></h1>
    </header>

    <div class="global">
      <label>Todo
        <input type="color" bind:value={globalColor} />
      </label>
      <button onclick={applyGlobal} disabled={loading}>Aplicar a todo</button>
    </div>

    <nav>
      {#each devices as d}
        <button class="dev" class:active={d.id === selectedId} onclick={() => select(d)}>
          <span class="dot" style:background={hex(d.colors[0] ?? [40, 40, 40])}></span>
          <span class="name">{d.name}</span>
          <small>{DEVICE_LABEL[d.device_type] ?? d.device_type} · {d.colors.length} LEDs</small>
        </button>
      {/each}
    </nav>

    <footer>
      <button onclick={() => load(api.rescan)} disabled={loading}>{loading ? "Cargando…" : "Re-detectar"}</button>
    </footer>
  </aside>

  <main>
    {#if error}
      <div class="error">
        <p>{error}</p>
        <button onclick={() => load(api.connect)}>Reintentar</button>
      </div>
    {:else if loading && !devices.length}
      <p class="muted">Conectando con OpenRGB…</p>
    {:else if device}
      <div class="title">
        <h2>{device.name}</h2>
        <small>{device.vendor} · {DEVICE_LABEL[device.device_type] ?? device.device_type}</small>
      </div>

      <div class="tabs">
        <button class:on={tab === "color"} onclick={() => (tab = "color")}>Color</button>
        {#if hasMatrix}<button class:on={tab === "keyboard"} onclick={() => (tab = "keyboard")}>Teclado</button>{/if}
        <button class:on={tab === "advanced"} onclick={() => (tab = "advanced")}>Avanzado</button>
      </div>

      {#if tab === "color"}
        <div class="simple">
          <input type="color" bind:value={deviceColor} />
          <button onclick={applyDevice}>Aplicar a {device.name}</button>
        </div>
        <div class="preview">
          {#each device.colors as c}<span style:background={hex(c)}></span>{/each}
        </div>
      {:else if tab === "keyboard"}
        {#key device.id}<Keyboard {device} />{/key}
      {:else}
        {#key device.id}<Advanced {device} onchange={(d) => (devices = d)} />{/key}
      {/if}

      {#if notice}<p class="notice">{notice}</p>{/if}
    {/if}
  </main>
</div>

<style>
  :global(:root) {
    --bg: #111116;
    --panel: #18181f;
    --text: #e8e8ef;
    --accent: #7dd3fc;
    color-scheme: dark;
    font-family: Inter, system-ui, sans-serif;
    font-size: 15px;
    color: var(--text);
    background: var(--bg);
  }
  :global(body) { margin: 0; }
  :global(button) {
    background: #2a2a35; color: var(--text); border: 1px solid #ffffff1a;
    border-radius: 6px; padding: 0.35rem 0.75rem; cursor: pointer; font: inherit;
  }
  :global(button:hover:not(:disabled)) { border-color: var(--accent); }
  :global(button:disabled) { opacity: 0.45; cursor: default; }
  :global(input, select) {
    background: #22222b; color: var(--text); border: 1px solid #ffffff1a; border-radius: 5px; padding: 0.25rem 0.4rem; font: inherit;
  }
  .app { display: grid; grid-template-columns: 17rem 1fr; height: 100vh; }
  aside { background: var(--panel); display: flex; flex-direction: column; border-right: 1px solid #ffffff10; min-height: 0; }
  header h1 { font-size: 1.25rem; margin: 1rem; letter-spacing: 0.02em; }
  header h1 span { color: var(--accent); }
  .global { display: flex; gap: 0.5rem; align-items: center; padding: 0 1rem 0.8rem; }
  .global label { display: flex; align-items: center; gap: 0.4rem; }
  .global input { width: 2.2rem; height: 1.9rem; padding: 0; border: none; }
  nav { flex: 1; overflow-y: auto; padding: 0 0.5rem; }
  .dev {
    display: grid; grid-template-columns: 0.8rem 1fr; column-gap: 0.6rem; width: 100%;
    text-align: left; margin-bottom: 0.25rem; background: transparent; border-color: transparent;
  }
  .dev.active { background: #2a2a35; border-color: #ffffff1a; }
  .dev .dot { grid-row: span 2; align-self: center; width: 0.8rem; height: 0.8rem; border-radius: 50%; box-shadow: 0 0 6px currentColor; }
  .dev .name { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dev small { opacity: 0.6; }
  footer { padding: 0.8rem 1rem; }
  main { padding: 1.2rem 1.6rem; overflow-y: auto; }
  .title h2 { margin: 0; }
  .title small { opacity: 0.6; }
  .tabs { display: flex; gap: 0.3rem; margin: 1rem 0; }
  .tabs .on { background: var(--accent); color: #111; }
  .simple { display: flex; gap: 0.6rem; align-items: center; }
  .simple input { width: 3rem; height: 2.2rem; padding: 0; border: none; }
  .preview { display: flex; flex-wrap: wrap; gap: 3px; margin-top: 1rem; }
  .preview span { width: 14px; height: 14px; border-radius: 3px; }
  /* Ventanas estrechas (tiling): la lista de dispositivos pasa a una barra superior. */
  @media (max-width: 760px) {
    .app { grid-template-columns: 1fr; grid-template-rows: auto 1fr; }
    aside { border-right: none; border-bottom: 1px solid #ffffff10; }
    header h1 { margin: 0.5rem 0.8rem; font-size: 1.05rem; }
    aside { display: grid; grid-template-columns: auto 1fr auto; align-items: center; }
    .global { padding: 0 0.5rem; }
    nav { grid-column: 1 / -1; display: flex; gap: 0.3rem; overflow-x: auto; padding: 0.3rem 0.5rem 0.5rem; }
    .dev { width: auto; min-width: 9rem; max-width: 13rem; margin: 0; }
    footer { padding: 0.3rem 0.5rem; grid-row: 1; grid-column: 3; }
    main { padding: 0.8rem; }
  }
  @media (max-height: 420px) {
    header { display: none; }
  }
  .error { color: #ff8080; }
  .muted { opacity: 0.6; }
  .notice { color: var(--accent); }
</style>
