<script lang="ts">
  /**
   * Quick tab. THEMES are the base look of a device (one at a time, picking one replaces the
   * current one); EFFECTS are overlays that stack on any theme (toggles). Swapping the theme keeps
   * the running effects.
   */
  import type { Animation, Color, Device, Layer, Role } from "./api";
  import ColorPicker from "./ColorPicker.svelte";
  import { activeProfile, profiles } from "./profilesStore.svelte";
  import { cloneColor, resolveColor } from "./palette.svelte";
  import { api, hex } from "./api";
  import { isCompatible } from "./anim/ref";
  import { buildLayout } from "./layout/buildLayout";
  import { animName, lib } from "./animationsStore.svelte";
  import { t } from "./i18n/index.svelte";
  import { commit, devScene, ensureDev, newLayer, sceneStore } from "./scene.svelte";
  import { animRole, effectRole, layerRole } from "./roles";
  import { toast, toastError } from "./state.svelte";
  import AnimCard from "./AnimCard.svelte";

  let { device, ondraw }: { device: Device; ondraw: () => void } = $props();

  const layout = $derived(buildLayout(device, { fanLeds: devScene(device.key).fan_leds }));
  const names = $derived(Array.from({ length: device.colors.length }, (_, i) => device.led_names[i] ?? ""));
  const ds = $derived(devScene(device.key));
  const layerLabel = (l: Layer) => {
    if (l.name) return l.name;
    const snap = l.params.snapshot as unknown as Animation | null;
    if (l.effect === "animation") return snap ? animName(snap) : (lib.list.find((a) => a.id === l.params.anim_id)?.name ?? t("fx.animation"));
    return t(`fx.${l.effect}`);
  };
  /** The device's own running layers (removable chips) and the global ones it follows (read-only). */
  const ownLayers = $derived(ds.mode === "effects" ? ds.layers.filter((l) => l.enabled) : []);
  const globalLayers = $derived(ds.mode === "effects" && ds.follow_global ? sceneStore.scene.global_layers.filter((l) => l.enabled) : []);
  const themeLayers = $derived(ownLayers.filter((l) => layerRole(l) === "theme"));
  const effectLayers = $derived(ownLayers.filter((l) => layerRole(l) === "effect"));
  /** One chip for the whole theme (a theme may be several layers, e.g. the keyboard's background + scan + keys). */
  const themeLabel = $derived([...new Set(themeLayers.map(layerLabel))].join(" + "));

  /** Whether layer `l` is the library animation `a` (a layered built-in theme/effect is matched by its name). */
  const isAnim = (l: Layer, a: Animation) => (l.effect === "animation" && l.params.anim_id === a.id) || (a.builtin && !!l.name && l.name === a.name && layerRole(l) === animRole(a));
  const animOn = (a: Animation) => ownLayers.some((l) => isAnim(l, a));
  const fxOn = (id: string) => ownLayers.some((l) => l.effect === id);

  /** Plain engine effects with no library preset of their own get a button (the presets are cards below). */
  const DATA_EFFECTS = ["frames", "path", "palette", "meter", "blink", "orbit", "sequence", "animation", "paint", "solid"];
  const presetFxIds = $derived(new Set(lib.list.flatMap((a) => (a.builtin && a.kind.type === "effect" ? [a.kind.effect] : []))));
  const plainFx = $derived(sceneStore.defs.filter((d) => !DATA_EFFECTS.includes(d.id) && !presetFxIds.has(d.id)));
  const themeFx = $derived(plainFx.filter((d) => effectRole(d.id) === "theme"));
  const effectFx = $derived(plainFx.filter((d) => effectRole(d.id) === "effect"));
  const solidActive = $derived(themeLayers.length === 1 && themeLayers[0].effect === "solid");

  /** The active profile's own presets come first, then the rest of the library; split by role. */
  const pinnedIds = $derived(new Set((activeProfile.value?.presets ?? []).map((p) => p.id)));
  const usable = $derived(lib.list.filter((a) => isCompatible(a, names)));
  const pinnedOf = (role: Role) => (activeProfile.value?.presets ?? []).flatMap((p) => usable.filter((a) => a.id === p.id && animRole(a) === role));
  const presetsOf = (role: Role) => usable.filter((a) => a.builtin && !pinnedIds.has(a.id) && animRole(a) === role);
  const mineOf = (role: Role) => usable.filter((a) => !a.builtin && !pinnedIds.has(a.id) && animRole(a) === role);
  const pinnedThemes = $derived(pinnedOf("theme"));
  const pinnedEffects = $derived(pinnedOf("effect"));
  const presetThemes = $derived(presetsOf("theme"));
  const presetEffects2 = $derived(presetsOf("effect"));
  const mineThemes = $derived(mineOf("theme"));
  const mineEffects = $derived(mineOf("effect"));
  const hasMine = $derived(mineThemes.length + mineEffects.length > 0);

  let color = $state<Color>([125, 211, 252]);

  /** Replaces the device's theme layers with `layers` (placed at the bottom) and keeps its effects. */
  function setTheme(layers: Layer[]) {
    const d = ensureDev(device.key);
    const keep = d.mode === "effects" ? d.layers.filter((l) => layerRole(l) === "effect") : [];
    d.mode = "effects";
    d.layers = [...layers, ...keep];
    commit();
  }

  function addEffect(l: Layer) {
    const d = ensureDev(device.key);
    if (d.mode !== "effects") d.layers = [];
    d.mode = "effects";
    d.layers.push(l);
    commit();
  }

  const animLayer = (a: Animation, role: Role): Layer => {
    const snap = $state.snapshot(a) as Animation;
    return newLayer("animation", { role, params: { anim_id: snap.id, snapshot: snap as unknown as Record<string, unknown> } });
  };

  function useColor() {
    setTheme([newLayer("solid", { name: t("quick.colour"), role: "theme", params: { color: cloneColor(color) } })]);
    toast(t("quick.colourApplied"), "ok", 1800);
  }

  /** Theme cards are radio-like: picking one swaps the theme. Built-in layered themes get per-device layers. */
  async function pickTheme(a: Animation) {
    if (animOn(a)) return;
    if (a.builtin && a.tags.includes("layers")) {
      try {
        setTheme(await api.themeLayers(a.name, device.key));
        return;
      } catch (e) {
        toastError(e);
      }
    }
    setTheme([animLayer(a, "theme")]);
  }

  /** Effect cards are toggles that stack on whatever theme is running. */
  function toggleEffect(a: Animation) {
    if (animOn(a)) {
      removeWhere((l) => isAnim(l, a));
    } else {
      addEffect(animLayer(a, "effect"));
    }
  }

  function pickFx(id: string) {
    if (effectRole(id) === "theme") {
      if (!fxOn(id)) setTheme([newLayer(id, { role: "theme" })]);
    } else if (fxOn(id)) {
      removeWhere((l) => l.effect === id);
    } else {
      addEffect(newLayer(id, { role: "effect" }));
    }
  }

  function removeWhere(f: (l: Layer) => boolean) {
    const d = ensureDev(device.key);
    d.layers = d.layers.filter((l) => !f(l));
    commit();
  }
  const removeTheme = () => removeWhere((l) => layerRole(l) === "theme");
</script>

<div class="now" data-now-playing>
  <span class="live" aria-hidden="true"></span>
  <b>{t("quick.now")}</b>
  {#if ds.mode === "hardware"}
    <span>{ds.hardware?.mode ? t("quick.nowHardware", { mode: ds.hardware.mode }) : t("quick.nowHardwareDefault")}</span>
  {:else if !ownLayers.length && !globalLayers.length}
    <span>{t("quick.nowNothing")}</span>
  {/if}
  {#if themeLayers.length}
    {@const label = t("quick.chipTheme", { name: themeLabel })}
    <span class="lchip" data-chip-theme>{label}<button class="x" aria-label={t("quick.removeLayer", { name: label })} title={t("quick.removeLayer", { name: label })} onclick={removeTheme}>✕</button></span>
  {/if}
  {#each effectLayers as l (l.id)}
    {@const label = t("quick.chipEffect", { name: layerLabel(l) })}
    <span class="lchip" data-layer={l.effect} data-chip-effect>{label}<button class="x" aria-label={t("quick.removeLayer", { name: label })} title={t("quick.removeLayer", { name: label })} onclick={() => removeWhere((x) => x.id === l.id)}>✕</button></span>
  {/each}
  {#each globalLayers as l (l.id)}
    <span class="lchip global" title={t("quick.globalLayer")}>{layerLabel(l)}</span>
  {/each}
  {#if profiles.settings.current_profile}<small class="muted">{t("quick.nowProfile", { name: profiles.settings.current_profile })}</small>{/if}
</div>

<p class="muted lead">{t("quick.hint")}</p>

<section data-themes>
  <h3>{t("quick.themes")}</h3>
  <p class="muted sub">{t("quick.themesHint")}</p>

  <div class="solid" class:active={solidActive}>
    <ColorPicker value={color} onchange={(c) => (color = c)} label={t("quick.pickColour")} />
    <code>{hex(resolveColor(color))}</code>
    <button class="primary" onclick={useColor}>{t("quick.useColour")}</button>
  </div>

  {#if themeFx.length}
    <ul class="fxgrid" data-theme-effects>
      {#each themeFx as d (d.id)}
        {@const on = fxOn(d.id)}
        <li>
          <button class="fx" class:active={on} role="radio" aria-checked={on} data-fx={d.id} onclick={() => pickFx(d.id)}>{#if on}<span class="tick" aria-hidden="true">✓</span>{/if}{t(`fx.${d.id}`)}</button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if pinnedThemes.length}
    <h4 data-pinned>{t("quick.profilePresets", { name: activeProfile.value?.name ?? "" })}</h4>
    <ul class="grid" role="radiogroup" aria-label={t("quick.themes")}>
      {#each pinnedThemes as a (a.id)}
        <li><AnimCard anim={a} {layout} {names} active={animOn(a)} onpick={() => pickTheme(a)} /></li>
      {/each}
    </ul>
  {/if}
  <h4>{t("quick.presets")}</h4>
  <ul class="grid" role="radiogroup" aria-label={t("quick.themes")} data-theme-list>
    {#each presetThemes as a (a.id)}
      <li><AnimCard anim={a} {layout} {names} active={animOn(a)} onpick={() => pickTheme(a)} /></li>
    {/each}
  </ul>
  {#if mineThemes.length}
    <h4>{t("quick.mine")}</h4>
    <ul class="grid" role="radiogroup" aria-label={t("quick.themes")}>
      {#each mineThemes as a (a.id)}
        <li><AnimCard anim={a} {layout} {names} active={animOn(a)} onpick={() => pickTheme(a)} /></li>
      {/each}
    </ul>
  {/if}
</section>

<section data-effects>
  <h3>{t("quick.effects")}</h3>
  <p class="muted sub">{t("quick.effectsHint")}</p>

  {#if effectFx.length}
    <ul class="fxgrid" data-effect-fx>
      {#each effectFx as d (d.id)}
        {@const on = fxOn(d.id)}
        <li>
          <button class="fx" class:active={on} aria-pressed={on} data-fx={d.id} onclick={() => pickFx(d.id)}>{#if on}<span class="tick" aria-hidden="true">✓</span>{/if}{t(`fx.${d.id}`)}</button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if pinnedEffects.length}
    <h4 data-pinned-effects>{t("quick.profilePresets", { name: activeProfile.value?.name ?? "" })}</h4>
    <ul class="grid">
      {#each pinnedEffects as a (a.id)}
        <li><AnimCard anim={a} {layout} {names} active={animOn(a)} onpick={() => toggleEffect(a)} /></li>
      {/each}
    </ul>
  {/if}
  <ul class="grid" data-effect-list>
    {#each presetEffects2 as a (a.id)}
      <li><AnimCard anim={a} {layout} {names} active={animOn(a)} onpick={() => toggleEffect(a)} /></li>
    {/each}
    {#each mineEffects as a (a.id)}
      <li><AnimCard anim={a} {layout} {names} active={animOn(a)} onpick={() => toggleEffect(a)} /></li>
    {/each}
  </ul>
</section>

{#if !hasMine}
  <div class="empty">
    <p>{t("quick.mineEmpty")}</p>
    <button class="primary" onclick={ondraw}>{t("quick.create")}</button>
  </div>
{/if}

<style>
  .lead { margin: 0 0 0.8rem; }
  h3 { margin: 1.2rem 0 0.2rem; }
  h4 { margin: 1rem 0 0.5rem; font-size: 0.9rem; color: var(--muted); }
  .sub { margin: 0 0 0.6rem; font-size: 0.85rem; }
  .solid { display: flex; align-items: center; gap: 0.7rem; flex-wrap: wrap; background: var(--panel); border: 1px solid var(--border); border-radius: 12px; padding: 0.6rem 0.8rem; }
  .now { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem; padding: 0.55rem 0.8rem; border: 1px solid var(--ok); border-radius: 8px; background: var(--panel-2); margin-bottom: 0.6rem; }
  .now small { margin-left: auto; }
  .live { width: 0.55rem; height: 0.55rem; border-radius: 50%; background: var(--ok); animation: pulse 1.6s ease-in-out infinite; }
  @keyframes pulse { 50% { opacity: 0.3; } }
  @media (prefers-reduced-motion: reduce) { .live { animation: none; } }
  .solid.active { border-color: var(--ok); }
  .lchip { display: inline-flex; align-items: center; gap: 0.3rem; border: 1px solid var(--ok); color: var(--ok); border-radius: 999px; padding: 0.05rem 0.2rem 0.05rem 0.65rem; font-size: 0.85rem; }
  .lchip.global { border-color: var(--border); color: var(--muted); padding-right: 0.65rem; }
  .lchip .x { border: 0; background: none; color: inherit; cursor: pointer; padding: 0 0.35rem; border-radius: 999px; line-height: 1.4; }
  .lchip .x:hover, .lchip .x:focus-visible { background: color-mix(in srgb, var(--ok) 25%, transparent); }
  .fxgrid { list-style: none; margin: 0.7rem 0 0; padding: 0; display: flex; flex-wrap: wrap; gap: 0.5rem; }
  .fx { display: inline-flex; align-items: center; gap: 0.35rem; }
  .fx.active { border-color: var(--ok); color: var(--ok); box-shadow: 0 0 0 1px var(--ok) inset; }
  .tick { font-weight: 700; }
  .grid { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 14rem), 1fr)); gap: 0.7rem; }
  .empty { background: var(--panel); border: 1px dashed var(--border); border-radius: 12px; padding: 1rem; display: flex; gap: 1rem; align-items: center; flex-wrap: wrap; }
  .empty p { margin: 0; color: var(--muted); flex: 1; min-width: 12rem; }
</style>
