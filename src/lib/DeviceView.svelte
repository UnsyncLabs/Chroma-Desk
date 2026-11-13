<script lang="ts">
  /** One device: Lighting (Quick / Draw / Layers), Hardware mode and Advanced tabs. */
  import { api, type Device, type ModeRequest } from "./api";
  import { t, deviceLabel } from "./i18n/index.svelte";
  import { app, confirmLeave, toastError } from "./state.svelte";
  import { commit, devScene, ensureDev, flush, frames, newLayer, syncPositions } from "./scene.svelte";
  import { buildLayout } from "./layout/buildLayout";
  import { applyStroke, paintTarget, toPaint } from "./paintLayer";
  import LedCanvas, { type Paint } from "./LedCanvas.svelte";
  import LayersPanel from "./LayersPanel.svelte";
  import Hardware from "./Hardware.svelte";
  import Advanced from "./Advanced.svelte";
  import Quick from "./Quick.svelte";
  import DrawTab from "./DrawTab.svelte";

  let { device }: { device: Device } = $props();

  type Tab = "lighting" | "hardware" | "advanced";
  type Level = "quick" | "draw" | "layers";
  let tab: Tab = $state("lighting");
  let level: Level = $state(app.pendingEdit ? "draw" : "quick");
  let selected = $state(new Set<number>());
  let selectedId: string | null = $state(null);

  // The library asked to edit something here.
  $effect(() => {
    if (app.pendingEdit) {
      tab = "lighting";
      level = "draw";
    }
  });

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

  /** Applies only the LEDs that changed in this stroke to the target Paint layer. */
  function onPaint(next: Paint) {
    const prev = canvasColors;
    const l = paintTarget(device, selectedId);
    selectedId = l.id;
    applyStroke(l, prev, next);
    commit();
  }

  const frame = $derived(frames.map[device.key]);
  const canvasColors: Paint = $derived(isPaint && layer ? toPaint(layer, n) : (frame ?? device.colors));
  const display = $derived(effects ? (frame ?? null) : null);

  async function enableEffects() {
    const d = ensureDev(device.key);
    if (!d.layers.length) {
      // Start from what the device shows now, as an editable Paint layer.
      const leds: Record<string, [number, number, number]> = {};
      device.colors.forEach((c, i) => (leds[i] = [...c] as [number, number, number]));
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

  async function setTab(k: Tab) {
    if (k === tab) return;
    if (tab === "lighting" && level === "draw" && !(await confirmLeave())) return;
    tab = k;
  }

  async function setLevel(l: Level) {
    if (l === level) return;
    if (level === "draw" && !(await confirmLeave())) return;
    level = l;
  }

  const TABS: Tab[] = ["lighting", "hardware", "advanced"];
  const LEVELS: Level[] = ["quick", "draw", "layers"];
</script>

<div class="title">
  <h2>{device.name}</h2>
  <small class="muted">{device.vendor ? `${device.vendor} · ` : ""}{deviceLabel(device.device_type)} · {n} LEDs</small>
</div>

<div class="tabs" role="tablist" aria-label={device.name}>
  {#each TABS as k (k)}
    <button role="tab" aria-selected={tab === k} class:on={tab === k} onclick={() => setTab(k)}>{t(`tab.${k}`)}</button>
  {/each}
</div>

{#if tab === "lighting"}
  <div class="levels" role="group" aria-label={t("level.label")}>
    {#each LEVELS as l (l)}
      <button class="level" class:on={level === l} aria-pressed={level === l} onclick={() => setLevel(l)}>
        <b>{t(`level.${l}`)}</b>
        <small>{t(`level.${l}.hint`)}</small>
      </button>
    {/each}
  </div>

  {#if level === "quick"}
    <Quick {device} ondraw={() => setLevel("draw")} />
  {:else if level === "draw"}
    <DrawTab {device} />
  {:else}
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
    {/if}
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
  .levels { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 0.5rem; margin: 0 0 1.1rem; max-width: 44rem; }
  .level { display: flex; flex-direction: column; align-items: flex-start; gap: 0.1rem; text-align: left; padding: 0.55rem 0.8rem; border-radius: 10px; }
  .level small { color: var(--muted); font-size: 0.78rem; line-height: 1.25; }
  .level.on { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 16%, var(--control)); }
  .level.on small { color: var(--text); opacity: 0.8; }
  @media (max-width: 560px) { .level small { display: none; } .level { padding: 0.45rem 0.6rem; } }
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
