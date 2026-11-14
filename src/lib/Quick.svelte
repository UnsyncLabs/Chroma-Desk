<script lang="ts">
  /** Quick tab: one click applies a colour, a preset or one of your animations to this device. */
  import type { Animation, Color, Device } from "./api";
  import ColorPicker from "./ColorPicker.svelte";
  import { activeProfile, profiles } from "./profilesStore.svelte";
  import { cloneColor, resolveColor } from "./palette.svelte";
  import { hex } from "./api";
  import { isCompatible } from "./anim/ref";
  import { buildLayout } from "./layout/buildLayout";
  import { animName, applyAnimation, lib } from "./animationsStore.svelte";
  import { t } from "./i18n/index.svelte";
  import { commit, devScene, ensureDev, newLayer, sceneStore } from "./scene.svelte";
  import type { Layer } from "./api";
  import { toast } from "./state.svelte";
  import AnimCard from "./AnimCard.svelte";

  let { device, ondraw }: { device: Device; ondraw: () => void } = $props();

  const layout = $derived(buildLayout(device, { fanLeds: devScene(device.key).fan_leds }));
  const names = $derived(Array.from({ length: device.colors.length }, (_, i) => device.led_names[i] ?? ""));
  const ds = $derived(devScene(device.key));
  /** Animations playing on this device (any enabled layer), so their cards are marked. */
  const playingIds = $derived(new Set(ds.mode === "effects" ? ds.layers.filter((l) => l.enabled && l.effect === "animation").map((l) => l.params.anim_id as string) : []));
  const layerLabel = (l: Layer) => {
    if (l.name) return l.name;
    const snap = l.params.snapshot as unknown as Animation | null;
    if (l.effect === "animation") return snap ? animName(snap) : (lib.list.find((a) => a.id === l.params.anim_id)?.name ?? t("fx.animation"));
    return t(`fx.${l.effect}`);
  };
  /** What is running on this device right now, in words. */
  const nowPlaying = $derived.by(() => {
    if (ds.mode === "hardware") return ds.hardware?.mode ? t("quick.nowHardware", { mode: ds.hardware.mode }) : t("quick.nowHardwareDefault");
    const own = ds.layers.filter((l) => l.enabled).map(layerLabel);
    const global = ds.follow_global ? sceneStore.scene.global_layers.filter((l) => l.enabled).map(layerLabel) : [];
    const all = [...own, ...global];
    return all.length ? all.join(" + ") : t("quick.nowNothing");
  });
  const solidActive = $derived(ds.mode === "effects" && ds.layers.length === 1 && ds.layers[0].effect === "solid");

  /** The active profile's own presets come first, then the rest of the library. */
  const pinnedIds = $derived(new Set((activeProfile.value?.presets ?? []).map((p) => p.id)));
  const pinned = $derived((activeProfile.value?.presets ?? []).flatMap((p) => lib.list.filter((a) => a.id === p.id && isCompatible(a, names))));
  const presets = $derived(lib.list.filter((a) => a.builtin && !pinnedIds.has(a.id) && isCompatible(a, names)));
  const mine = $derived(lib.list.filter((a) => !a.builtin && !pinnedIds.has(a.id) && isCompatible(a, names)));

  let color = $state<Color>([125, 211, 252]);

  function useColor() {
    const d = ensureDev(device.key);
    d.mode = "effects";
    d.layers = [newLayer("solid", { params: { color: cloneColor(color) } })];
    commit();
    toast(t("quick.colourApplied"), "ok", 1800);
  }

  function pick(a: Animation) {
    applyAnimation([device.key], a);
    toast(t("quick.applied"), "ok", 1800);
  }
</script>

<div class="now" data-now-playing>
  <span class="live" aria-hidden="true"></span>
  <span><b>{t("quick.now")}</b> {nowPlaying}</span>
  {#if profiles.settings.current_profile}<small class="muted">{t("quick.nowProfile", { name: profiles.settings.current_profile })}</small>{/if}
</div>

<p class="muted lead">{t("quick.hint")}</p>

<h3>{t("quick.colour")}</h3>
<div class="solid" class:active={solidActive}>
  <ColorPicker value={color} onchange={(c) => (color = c)} label={t("quick.pickColour")} />
  <code>{hex(resolveColor(color))}</code>
  <button class="primary" onclick={useColor}>{t("quick.useColour")}</button>
</div>

{#if pinned.length}
  <h3 data-pinned>{t("quick.profilePresets", { name: activeProfile.value?.name ?? "" })}</h3>
  <ul class="grid">
    {#each pinned as a (a.id)}
      <li><AnimCard anim={a} {layout} {names} active={playingIds.has(a.id)} onpick={() => pick(a)} /></li>
    {/each}
  </ul>
{/if}

<h3>{t("quick.presets")}</h3>
<ul class="grid">
  {#each presets as a (a.id)}
    <li><AnimCard anim={a} {layout} {names} active={playingIds.has(a.id)} onpick={() => pick(a)} /></li>
  {/each}
</ul>

<h3>{t("quick.mine")}</h3>
{#if mine.length}
  <ul class="grid">
    {#each mine as a (a.id)}
      <li><AnimCard anim={a} {layout} {names} active={playingIds.has(a.id)} onpick={() => pick(a)} /></li>
    {/each}
  </ul>
{:else}
  <div class="empty">
    <p>{t("quick.mineEmpty")}</p>
    <button class="primary" onclick={ondraw}>{t("quick.create")}</button>
  </div>
{/if}

<style>
  .lead { margin: 0 0 0.8rem; }
  h3 { margin: 1.2rem 0 0.6rem; }
  .solid { display: flex; align-items: center; gap: 0.7rem; flex-wrap: wrap; background: var(--panel); border: 1px solid var(--border); border-radius: 12px; padding: 0.6rem 0.8rem; }
  .now { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem; padding: 0.55rem 0.8rem; border: 1px solid var(--accent); border-radius: 8px; background: var(--panel-2); margin-bottom: 0.6rem; }
  .now small { margin-left: auto; }
  .live { width: 0.55rem; height: 0.55rem; border-radius: 50%; background: var(--accent); animation: pulse 1.6s ease-in-out infinite; }
  @keyframes pulse { 50% { opacity: 0.3; } }
  @media (prefers-reduced-motion: reduce) { .live { animation: none; } }
  .solid.active { border-color: var(--accent); }
  .grid { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 14rem), 1fr)); gap: 0.7rem; }
  .empty { background: var(--panel); border: 1px dashed var(--border); border-radius: 12px; padding: 1rem; display: flex; gap: 1rem; align-items: center; flex-wrap: wrap; }
  .empty p { margin: 0; color: var(--muted); flex: 1; min-width: 12rem; }
</style>
