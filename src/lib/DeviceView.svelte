<script lang="ts">
  /** One device: Lighting (canvas + layers), Hardware mode and Advanced tabs. */
  import { api, type Device, type Layer, type ModeRequest, type Rgb } from "./api";
  import { t, deviceLabel } from "./i18n/index.svelte";
  import { toastError } from "./state.svelte";
  import { commit, devScene, ensureDev, flush, frames, newLayer, sceneStore, syncPositions } from "./scene.svelte";
  import { buildLayout } from "./layout/buildLayout";
  import LedCanvas, { type Paint } from "./LedCanvas.svelte";
  import LayersPanel from "./LayersPanel.svelte";
  import Hardware from "./Hardware.svelte";
  import Advanced from "./Advanced.svelte";

  let { device }: { device: Device } = $props();

  type Tab = "lighting" | "hardware" | "advanced";
  let tab: Tab = $state("lighting");
  let selected = $state(new Set<number>());
  let selectedId: string | null = $state(null);

  const ds = $derived(devScene(device.key));
  const effects = $derived(ds.mode === "effects");
  const layout = $derived(buildLayout(device, { fanLeds: ds.fan_leds }));
  const n = $derived(device.colors.length);

  // Keep the engine's idea of the LED positions in sync with what is drawn.
  $effect(() => {
    syncPositions([device]);
  });

  // Select the top layer by default.
  $effect(() => {
    if (effects && !ds.layers.some((l) => l.id === selectedId)) selectedId = ds.layers.at(-1)?.id ?? null;
  });

  const layer = $derived(ds.layers.find((l) => l.id === selectedId) ?? null);
  const isPaint = $derived(layer?.effect === "paint");

  function toPaint(l: Layer): Paint {
    const out: Paint = Array.from({ length: n }, () => null);
    const leds = (l.params.leds ?? {}) as Record<string, Rgb>;
    for (const [k, c] of Object.entries(leds)) if (Number(k) < n) out[Number(k)] = c;
    return out;
  }

  /**
   * Paint layer that receives brush strokes. Painting must always work, so this switches the
   * device to effects mode (seeding a Paint layer with the current colours) and picks or
   * creates a Paint layer when the selected one is not paintable.
   */
  function paintTarget(): Layer {
    const d = ensureDev(device.key);
    if (d.mode !== "effects") {
      d.mode = "effects";
      if (!d.layers.length) {
        const leds: Record<string, Rgb> = {};
        device.colors.forEach((c, i) => (leds[i] = [...c] as Rgb));
        d.layers.push(newLayer("paint", { params: { leds } }));
      }
    }
    let l = d.layers.find((x) => x.id === selectedId && x.effect === "paint") ?? d.layers.findLast((x) => x.effect === "paint");
    if (!l) {
      d.layers.push(newLayer("paint", { params: { leds: {} } }));
      l = d.layers.at(-1)!;
    }
    selectedId = l.id;
    return l;
  }

  const same = (a: Rgb | null | undefined, b: Rgb | null | undefined) =>
    (!a && !b) || (!!a && !!b && a[0] === b[0] && a[1] === b[1] && a[2] === b[2]);

  /** Applies only the LEDs that changed in this stroke to the target Paint layer. */
  function onPaint(next: Paint) {
    const prev = canvasColors;
    const l = paintTarget();
    const leds = { ...((l.params.leds ?? {}) as Record<string, Rgb>) };
    next.forEach((c, i) => {
      if (same(c, prev[i])) return;
      if (c) leds[i] = [...c] as Rgb;
      else delete leds[i];
    });
    l.params.leds = leds;
    commit();
  }

  const frame = $derived(frames.map[device.key]);
  const canvasColors: Paint = $derived(isPaint && layer ? toPaint(layer) : (frame ?? device.colors));
  const display = $derived(effects ? (frame ?? null) : null);

  async function enableEffects() {
    const d = ensureDev(device.key);
    if (!d.layers.length) {
      // Start from what the device shows now, as an editable Paint layer.
      const leds: Record<string, Rgb> = {};
      device.colors.forEach((c, i) => (leds[i] = [...c] as Rgb));
      d.layers.push(newLayer("paint", { params: { leds } }));
    }
    d.mode = "effects";
    commit();
  }

  async function enableHardware() {
    const d = ensureDev(device.key);
    d.mode = "hardware";
    await flush();
    const hw = d.hardware;
    const idx = hw ? device.modes.findIndex((m) => m.name === hw.mode) : -1;
    if (hw && idx >= 0) {
      const req: ModeRequest = { mode: idx, speed: hw.speed ?? undefined, brightness: hw.brightness ?? undefined, direction: hw.direction ?? undefined, colors: hw.colors ?? undefined };
      await api.setMode(device.id, req).catch(toastError);
    }
  }

  async function beforeHardwareApply() {
    ensureDev(device.key).mode = "hardware";
    await flush();
  }

  function hardwareApplied(req: ModeRequest, name: string) {
    ensureDev(device.key).hardware = {
      mode: name,
      speed: req.speed ?? null,
      brightness: req.brightness ?? null,
      direction: req.direction ?? null,
      colors: req.colors ?? null,
    };
    commit();
  }

  function addPaintLayer() {
    const l = newLayer("paint", { params: { leds: {} } });
    ensureDev(device.key).layers.push(l);
    selectedId = l.id;
    commit();
  }

  const TABS: Tab[] = ["lighting", "hardware", "advanced"];
</script>

<div class="title">
  <h2>{device.name}</h2>
  <small class="muted">{device.vendor ? `${device.vendor} · ` : ""}{deviceLabel(device.device_type)} · {n} LEDs</small>
</div>

<div class="tabs" role="tablist" aria-label={device.name}>
  {#each TABS as k (k)}
    <button role="tab" aria-selected={tab === k} class:on={tab === k} onclick={() => (tab = k)}>{t(`tab.${k}`)}</button>
  {/each}
</div>

{#if tab === "lighting"}
  <div class="mode" role="radiogroup" aria-label={t("mode.label")}>
    <button role="radio" aria-checked={effects} class:on={effects} onclick={enableEffects}>{t("mode.effects")}</button>
    <button role="radio" aria-checked={!effects} class:on={!effects} onclick={enableHardware}>{t("mode.hardware")}</button>
  </div>

  {#if effects}
    <div class="split">
      <div class="canvasCol">
        {#if !isPaint}
          <p class="banner">
            {t("paint.needLayer")}
            <button onclick={addPaintLayer}>{t("paint.addLayer")}</button>
          </p>
        {/if}
        <LedCanvas {layout} colors={canvasColors} {display} transparent={isPaint} bind:selected resetKey={selectedId ?? ""} onchange={onPaint} />
      </div>
      <LayersPanel layers={ds.layers} oncommit={commit} bind:selectedId zones={device.zones} selection={selected} />
    </div>
    <div class="extra">
      <label>{t("dev.brightness")}
        <input type="range" min="0" max="1" step="0.01" value={ds.brightness} oninput={(e) => { ensureDev(device.key).brightness = Number(e.currentTarget.value); commit(); }} />
        {Math.round(ds.brightness * 100)}%
      </label>
      <label>{t("dev.fps")}
        <input
          type="number" min="1" max="60" placeholder={t("dev.fpsAuto")} value={ds.fps ?? ""}
          onchange={(e) => { ensureDev(device.key).fps = e.currentTarget.value ? Number(e.currentTarget.value) : null; commit(); }}
        />
      </label>
      <label>
        <input type="checkbox" checked={ds.follow_global} onchange={(e) => { ensureDev(device.key).follow_global = e.currentTarget.checked; commit(); }} />
        {t("dev.followGlobal")}
      </label>
    </div>
  {:else}
    <div class="hwnote">
      <p>{ds.hardware ? t("mode.hardwareActive", { mode: ds.hardware.mode }) : t("mode.hardwareNote")}</p>
      <button class="primary" onclick={enableEffects}>{t("mode.useEffects")}</button>
    </div>
    <LedCanvas {layout} colors={device.colors} bind:selected onchange={onPaint} />
  {/if}
{:else if tab === "hardware"}
  {#key device.key}<Hardware {device} beforeApply={beforeHardwareApply} onapplied={hardwareApplied} />{/key}
{:else}
  {#key device.key}<Advanced {device} />{/key}
{/if}

<style>
  .title small { display: block; }
  .tabs, .mode { display: flex; gap: 0.3rem; margin: 0.9rem 0; flex-wrap: wrap; }
  .tabs .on, .mode .on { background: var(--accent); color: var(--accent-fg); }
  .split { display: grid; grid-template-columns: minmax(0, 1fr) 20rem; gap: 1rem; align-items: start; }
  .canvasCol { min-width: 0; }
  .banner { background: var(--panel-2); border-radius: 8px; padding: 0.5rem 0.8rem; display: flex; gap: 0.6rem; align-items: center; flex-wrap: wrap; margin: 0 0 0.6rem; }
  .extra { display: flex; gap: 1.2rem; flex-wrap: wrap; margin-top: 0.8rem; align-items: center; }
  .extra label { display: flex; align-items: center; gap: 0.4rem; }
  .extra input[type="number"] { width: 4.5rem; }
  .hwnote { display: flex; gap: 0.8rem; align-items: center; flex-wrap: wrap; }
  @media (max-width: 1000px) {
    .split { grid-template-columns: minmax(0, 1fr); }
  }
</style>
