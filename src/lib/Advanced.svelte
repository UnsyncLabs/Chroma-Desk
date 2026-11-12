<script lang="ts">
  /** Advanced tab: zone sizes + fan presets, per-LED table and device information. */
  import { api, hex, fromHex, type Device, type Rgb } from "./api";
  import { t } from "./i18n/index.svelte";
  import { setDevices, toast, toastError } from "./state.svelte";
  import { setFanLeds } from "./scene.svelte";

  let { device }: { device: Device } = $props();

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

  async function resize(zoneId: number, size: number, perFan = 0) {
    try {
      const zone = device.zones.find((z) => z.id === zoneId);
      setDevices(await api.resizeZone(device.id, zoneId, size));
      // Ring drawing: remember how many LEDs each fan has (0 = plain strip).
      if (zone) setFanLeds(device.key, zone.name, perFan);
      toast(t("adv.resized", { n: size }), "ok");
    } catch (e) {
      toastError(e);
    }
  }

  // ---------- per-LED table ----------
  let ledColors: string[] = $state([]);
  $effect(() => {
    ledColors = device.colors.map(hex);
  });

  async function sendLeds() {
    try {
      await api.setLeds(device.id, ledColors.map(fromHex) as Rgb[], true);
    } catch (e) {
      toastError(e);
    }
  }
</script>

<section>
  <h3>{t("adv.zones")}</h3>
  <div class="scroll">
    <table>
      <thead><tr><th>{t("adv.zone")}</th><th>{t("adv.type")}</th><th>{t("adv.leds")}</th><th>{t("adv.range")}</th><th></th></tr></thead>
      <tbody>
        {#each device.zones as z (z.id)}
          <tr>
            <td>{z.name}</td>
            <td>{z.zone_type}{z.matrix ? ` ${z.matrix.width}×${z.matrix.height}` : ""}</td>
            <td>
              {#if z.resizable}
                <input type="number" min={z.leds_min} max={z.leds_max} bind:value={sizes[z.id]} aria-label={t("adv.leds")} />
              {:else}
                {z.num_leds}
              {/if}
            </td>
            <td>{z.resizable ? `${z.leds_min}–${z.leds_max}` : t("adv.fixed")}</td>
            <td>
              {#if z.resizable}
                <button onclick={() => resize(z.id, sizes[z.id])} disabled={sizes[z.id] === z.num_leds}>{t("adv.apply")}</button>
              {/if}
            </td>
          </tr>
          {#if z.resizable}
            <tr class="preset">
              <td colspan="5">
                {t("adv.preset")}
                <input type="number" min="1" max="12" bind:value={presetFans[z.id]} aria-label={t("adv.fans")} />
                ×
                <select bind:value={presetIdx[z.id]} aria-label={t("adv.fanModel")}>
                  {#each FAN_PRESETS as p, i (p.name)}<option value={i}>{p.name} ({p.leds} LEDs)</option>{/each}
                </select>
                = <b>{presetFans[z.id] * FAN_PRESETS[presetIdx[z.id]].leds}</b>
                <button onclick={() => resize(z.id, presetFans[z.id] * FAN_PRESETS[presetIdx[z.id]].leds, FAN_PRESETS[presetIdx[z.id]].leds)}>{t("adv.usePreset")}</button>
              </td>
            </tr>
          {/if}
        {/each}
      </tbody>
    </table>
  </div>
  <p class="hint">{t("adv.sizesHint")}</p>
</section>

<section>
  <h3>{t("adv.ledTable", { n: device.led_names.length })}</h3>
  <div class="leds">
    {#each device.led_names as n, i (i)}
      <label class="led" title={n}>
        <input type="color" bind:value={ledColors[i]} onchange={sendLeds} aria-label={n || t("canvas.ledN", { n: i })} />
        <span>{i}: {n}</span>
      </label>
    {/each}
  </div>
</section>

<section class="info">
  <h3>{t("adv.info")}</h3>
  <dl>
    <dt>{t("adv.vendor")}</dt><dd>{device.vendor || "—"}</dd>
    <dt>{t("adv.description")}</dt><dd>{device.description || "—"}</dd>
    <dt>{t("adv.version")}</dt><dd>{device.version || "—"}</dd>
    <dt>{t("adv.serial")}</dt><dd>{device.serial || "—"}</dd>
    <dt>{t("adv.location")}</dt><dd>{device.location || "—"}</dd>
    <dt>{t("adv.key")}</dt><dd>{device.key}</dd>
  </dl>
</section>

<style>
  section { margin-bottom: 1.4rem; }
  .scroll { overflow-x: auto; }
  table { border-collapse: collapse; width: 100%; }
  th, td { text-align: left; padding: 0.3rem 0.5rem; border-bottom: 1px solid var(--border); }
  td input[type="number"] { width: 5rem; }
  .preset td { font-size: 0.85rem; color: var(--muted); }
  .preset input { width: 3.5rem; }
  label { display: flex; align-items: center; gap: 0.5rem; margin: 0.3rem 0; }
  .leds { display: grid; grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr)); gap: 0.25rem; max-height: 16rem; overflow: auto; }
  .led { font-size: 0.78rem; margin: 0; }
  .led span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .led input { width: 1.8rem; height: 1.4rem; padding: 0; border: none; }
  .hint { font-size: 0.8rem; color: var(--muted); }
  dl { display: grid; grid-template-columns: max-content 1fr; gap: 0.2rem 1rem; font-size: 0.85rem; }
  dt { color: var(--muted); }
  dd { margin: 0; word-break: break-all; }
</style>
