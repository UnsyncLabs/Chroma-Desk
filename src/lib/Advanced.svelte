<script lang="ts">
  import { api, hex, fromHex, type Device, type Mode, type Rgb } from "./api";

  let { device, onchange }: { device: Device; onchange: (d: Device[]) => void } = $props();

  let status = $state("");
  const err = (e: unknown) => (status = String(e));

  // ---------- Zonas ----------
  const FAN_PRESETS = [
    { name: "Corsair AF120 RGB Elite", leds: 8 },
    { name: "Corsair SP120 RGB Elite", leds: 8 },
    { name: "Corsair SP120 RGB", leds: 4 },
    { name: "Corsair LL120", leds: 16 },
    { name: "Corsair QL120", leds: 34 },
    { name: "Corsair ML120 RGB", leds: 4 },
  ];
  let sizes: Record<number, number> = $state({});
  let presetFans: Record<number, number> = $state({});
  let presetIdx: Record<number, number> = $state({});

  $effect(() => {
    for (const z of device.zones) {
      sizes[z.id] = z.num_leds;
      presetFans[z.id] ??= 3;
      presetIdx[z.id] ??= 0;
    }
  });

  async function resize(zoneId: number, size: number) {
    status = "";
    try {
      onchange(await api.resizeZone(device.id, zoneId, size));
      status = `Zona redimensionada a ${size} LEDs y guardada.`;
    } catch (e) {
      err(e);
    }
  }

  // ---------- Modos ----------
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

  const DIRS = ["Izquierda", "Derecha", "Arriba", "Abajo", "Horizontal", "Vertical"];

  async function applyMode(save = false) {
    if (!mode) return;
    status = "";
    try {
      await api.setMode(device.id, {
        mode: mode.id,
        speed: mode.has_speed ? speed : undefined,
        brightness: mode.has_brightness ? brightness : undefined,
        direction: mode.has_direction ? direction : undefined,
        colors: mode.color_mode === 2 ? modeColors.map(fromHex) : undefined,
        save,
      });
      status = save ? `Modo "${mode.name}" aplicado y guardado en el dispositivo.` : `Modo "${mode.name}" aplicado.`;
    } catch (e) {
      err(e);
    }
  }

  // ---------- LEDs individuales ----------
  let ledColors: string[] = $state([]);
  $effect(() => {
    ledColors = device.colors.map(hex);
  });

  async function sendLeds() {
    try {
      await api.setLeds(device.id, ledColors.map(fromHex) as Rgb[], true);
    } catch (e) {
      err(e);
    }
  }
</script>

<section>
  <h3>Zonas</h3>
  <table>
    <thead><tr><th>Zona</th><th>Tipo</th><th>LEDs</th><th>Rango</th><th></th></tr></thead>
    <tbody>
      {#each device.zones as z}
        <tr>
          <td>{z.name}</td>
          <td>{z.zone_type}{z.matrix ? ` ${z.matrix.width}×${z.matrix.height}` : ""}</td>
          <td>
            {#if z.resizable}
              <input type="number" min={z.leds_min} max={z.leds_max} bind:value={sizes[z.id]} />
            {:else}
              {z.num_leds}
            {/if}
          </td>
          <td>{z.resizable ? `${z.leds_min}–${z.leds_max}` : "fijo"}</td>
          <td>
            {#if z.resizable}
              <button onclick={() => resize(z.id, sizes[z.id])} disabled={sizes[z.id] === z.num_leds}>Aplicar</button>
            {/if}
          </td>
        </tr>
        {#if z.resizable}
          <tr class="preset">
            <td colspan="5">
              Preset:
              <input type="number" min="1" max="12" bind:value={presetFans[z.id]} />
              ×
              <select bind:value={presetIdx[z.id]}>
                {#each FAN_PRESETS as p, i}<option value={i}>{p.name} ({p.leds} LEDs)</option>{/each}
              </select>
              = <b>{presetFans[z.id] * FAN_PRESETS[presetIdx[z.id]].leds}</b>
              <button onclick={() => resize(z.id, presetFans[z.id] * FAN_PRESETS[presetIdx[z.id]].leds)}>Usar preset</button>
            </td>
          </tr>
        {/if}
      {/each}
    </tbody>
  </table>
  <p class="hint">Los tamaños se guardan en <code>~/.config/chroma-desk/config.json</code> y se vuelven a aplicar al conectar.</p>
</section>

<section>
  <h3>Modo</h3>
  <div class="row">
    <select bind:value={modeIdx}>
      {#each device.modes as m}<option value={m.id}>{m.name}{m.id === device.active_mode ? " (activo)" : ""}</option>{/each}
    </select>
    <button onclick={() => applyMode(false)}>Aplicar</button>
    {#if mode?.can_save}<button onclick={() => applyMode(true)}>Guardar en el dispositivo</button>{/if}
  </div>
  {#if mode}
    {#if mode.has_speed && mode.speed_min !== null && mode.speed_max !== null}
      <label>Velocidad <input type="range" min={Math.min(mode.speed_min, mode.speed_max)} max={Math.max(mode.speed_min, mode.speed_max)} bind:value={speed} /> {speed}</label>
    {/if}
    {#if mode.has_brightness && mode.brightness_min !== null && mode.brightness_max !== null}
      <label>Brillo <input type="range" min={mode.brightness_min} max={mode.brightness_max} bind:value={brightness} /> {brightness}</label>
    {/if}
    {#if mode.has_direction}
      <label>Dirección
        <select bind:value={direction}>{#each DIRS as d, i}<option value={i}>{d}</option>{/each}</select>
      </label>
    {/if}
    {#if mode.color_mode === 2}
      <div class="row">Colores:
        {#each modeColors as _, i}<input type="color" bind:value={modeColors[i]} />{/each}
        {#if mode.colors_max && modeColors.length < mode.colors_max}
          <button onclick={() => (modeColors = [...modeColors, "#ffffff"])}>+</button>
        {/if}
        {#if mode.colors_min !== null && modeColors.length > Math.max(1, mode.colors_min)}
          <button onclick={() => (modeColors = modeColors.slice(0, -1))}>−</button>
        {/if}
      </div>
    {/if}
  {/if}
</section>

<section>
  <h3>LEDs individuales ({device.led_names.length})</h3>
  <div class="leds">
    {#each device.led_names as n, i}
      <label class="led" title={n}>
        <input type="color" bind:value={ledColors[i]} onchange={sendLeds} />
        <span>{i}: {n}</span>
      </label>
    {/each}
  </div>
</section>

<section class="info">
  <h3>Información</h3>
  <dl>
    <dt>Fabricante</dt><dd>{device.vendor || "—"}</dd>
    <dt>Descripción</dt><dd>{device.description || "—"}</dd>
    <dt>Versión</dt><dd>{device.version || "—"}</dd>
    <dt>Serie</dt><dd>{device.serial || "—"}</dd>
    <dt>Ubicación</dt><dd>{device.location || "—"}</dd>
  </dl>
</section>

{#if status}<p class="status">{status}</p>{/if}

<style>
  section { margin-bottom: 1.4rem; }
  h3 { margin: 0 0 0.5rem; font-size: 1rem; }
  table { border-collapse: collapse; width: 100%; }
  th, td { text-align: left; padding: 0.3rem 0.5rem; border-bottom: 1px solid #ffffff14; }
  td input[type="number"] { width: 5rem; }
  .preset td { font-size: 0.85rem; opacity: 0.85; }
  .preset input { width: 3.5rem; }
  .row { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; margin-bottom: 0.5rem; }
  label { display: flex; align-items: center; gap: 0.5rem; margin: 0.3rem 0; }
  .leds { display: grid; grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr)); gap: 0.25rem; max-height: 16rem; overflow: auto; }
  .led { font-size: 0.78rem; margin: 0; }
  .led span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .led input { width: 1.8rem; height: 1.4rem; padding: 0; border: none; }
  .hint { font-size: 0.8rem; opacity: 0.7; }
  dl { display: grid; grid-template-columns: max-content 1fr; gap: 0.2rem 1rem; font-size: 0.85rem; }
  dt { opacity: 0.6; }
  dd { margin: 0; word-break: break-all; }
  .status { color: var(--accent); }
</style>
