<script lang="ts">
  /** Native (hardware) modes of a device: speed, brightness, direction, colours, save to device. */
  import { api, hex, fromHex, type Device, type Mode } from "./api";
  import { t } from "./i18n/index.svelte";
  import { toast, toastError } from "./state.svelte";

  let { device, onapplied }: { device: Device; onapplied?: (mode: number) => void } = $props();

  let modeIdx = $state(0);
  let speed = $state(0);
  let brightness = $state(0);
  let direction = $state(0);
  let modeColors: string[] = $state([]);

  const mode: Mode | undefined = $derived(device.modes[modeIdx]);

  $effect(() => {
    modeIdx = device.active_mode;
  });
  $effect(() => {
    if (!mode) return;
    speed = mode.speed ?? 0;
    brightness = mode.brightness ?? 0;
    direction = mode.direction ?? 0;
    modeColors = mode.colors.map(hex);
  });

  const DIRS = ["left", "right", "up", "down", "horizontal", "vertical"];

  async function applyMode(save = false) {
    if (!mode) return;
    try {
      await api.setMode(device.id, {
        mode: mode.id,
        speed: mode.has_speed ? speed : undefined,
        brightness: mode.has_brightness ? brightness : undefined,
        direction: mode.has_direction ? direction : undefined,
        colors: mode.color_mode === 2 ? modeColors.map(fromHex) : undefined,
        save,
      });
      onapplied?.(mode.id);
      toast(t(save ? "hw.savedToDevice" : "hw.applied", { mode: mode.name }), "ok");
    } catch (e) {
      toastError(e);
    }
  }
</script>

<section>
  <h3>{t("hw.title")}</h3>
  <p class="muted">{t("hw.hint")}</p>
  <div class="row">
    <select bind:value={modeIdx} aria-label={t("hw.mode")}>
      {#each device.modes as m (m.id)}<option value={m.id}>{m.name}{m.id === device.active_mode ? ` (${t("hw.active")})` : ""}</option>{/each}
    </select>
    <button class="primary" onclick={() => applyMode(false)}>{t("hw.apply")}</button>
    {#if mode?.can_save}<button onclick={() => applyMode(true)}>{t("hw.save")}</button>{/if}
  </div>
  {#if mode}
    {#if mode.has_speed && mode.speed_min !== null && mode.speed_max !== null}
      <label>{t("hw.speed")}
        <input type="range" min={Math.min(mode.speed_min, mode.speed_max)} max={Math.max(mode.speed_min, mode.speed_max)} bind:value={speed} /> {speed}
      </label>
    {/if}
    {#if mode.has_brightness && mode.brightness_min !== null && mode.brightness_max !== null}
      <label>{t("hw.brightness")}
        <input type="range" min={mode.brightness_min} max={mode.brightness_max} bind:value={brightness} /> {brightness}
      </label>
    {/if}
    {#if mode.has_direction}
      <label>{t("hw.direction")}
        <select bind:value={direction}>{#each DIRS as d, i (d)}<option value={i}>{t(`dir.${d}`)}</option>{/each}</select>
      </label>
    {/if}
    {#if mode.color_mode === 2}
      <div class="row">{t("hw.colors")}
        {#each modeColors as _, i (i)}<input type="color" bind:value={modeColors[i]} aria-label={t("hw.colorN", { n: i + 1 })} />{/each}
        {#if mode.colors_max && modeColors.length < mode.colors_max}
          <button onclick={() => (modeColors = [...modeColors, "#ffffff"])} aria-label={t("hw.addColor")}>+</button>
        {/if}
        {#if mode.colors_min !== null && modeColors.length > Math.max(1, mode.colors_min)}
          <button onclick={() => (modeColors = modeColors.slice(0, -1))} aria-label={t("hw.removeColor")}>−</button>
        {/if}
      </div>
    {/if}
  {/if}
</section>

<style>
  section { margin-bottom: 1.4rem; }
  .row { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; margin-bottom: 0.5rem; }
  label { display: flex; align-items: center; gap: 0.5rem; margin: 0.3rem 0; flex-wrap: wrap; }
</style>
