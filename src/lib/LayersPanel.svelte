<script lang="ts">
  /** Layer stack editor (shown top layer first). Works for device layers and the global scene. */
  import type { Blend, Layer, Zone } from "./api";
  import { t } from "./i18n/index.svelte";
  import { confirmDialog } from "./state.svelte";
  import { newLayer, sceneStore } from "./scene.svelte";
  import EffectParams from "./EffectParams.svelte";
  import AnimationLayerParams from "./AnimationLayerParams.svelte";
  import { animName } from "./animations.svelte";
  import type { Animation } from "./api";

  let {
    layers,
    oncommit,
    selectedId = $bindable(null),
    zones = null,
    selection = new Set<number>(),
    allowPaint = true,
  }: {
    layers: Layer[];
    oncommit: () => void;
    selectedId?: string | null;
    /** Zones of the device (enables zone masks); null for the global scene. */
    zones?: Zone[] | null;
    selection?: Set<number>;
    allowPaint?: boolean;
  } = $props();

  const BLENDS: Blend[] = ["normal", "add", "multiply", "screen"];
  let newEffect = $state("rainbow");
  let dragFrom = $state(-1);

  const shown = $derived([...layers].reverse());
  const selected = $derived(layers.find((l) => l.id === selectedId) ?? null);
  const def = $derived(sceneStore.defs.find((d) => d.id === selected?.effect));
  /** Effects driven by animation data are created from the Draw tab and the library, not here. */
  const DATA_EFFECTS = ["frames", "path", "palette", "meter", "blink", "orbit", "sequence"];
  const effects = $derived(sceneStore.defs.filter((d) => (allowPaint || d.id !== "paint") && !DATA_EFFECTS.includes(d.id) && (d.id !== "animation" || allowPaint)));
  const label = (l: Layer) => {
    if (l.name) return l.name;
    const snap = l.params.snapshot as unknown as Animation | null;
    return l.effect === "animation" && snap ? `${t("fx.animation")}: ${animName(snap)}` : t(`fx.${l.effect}`);
  };

  function add() {
    const l = newLayer(newEffect);
    layers.push(l);
    selectedId = l.id;
    oncommit();
  }

  async function remove(l: Layer) {
    if (!(await confirmDialog(t("layers.confirmDelete", { name: label(l) }), t("common.delete")))) return;
    const i = layers.findIndex((x) => x.id === l.id);
    if (i >= 0) layers.splice(i, 1);
    if (selectedId === l.id) selectedId = null;
    oncommit();
  }

  /** `dir` -1 moves the layer up in the list (towards the top of the stack). */
  function move(l: Layer, dir: -1 | 1) {
    const i = layers.findIndex((x) => x.id === l.id);
    const j = i - dir;
    if (i < 0 || j < 0 || j >= layers.length) return;
    [layers[i], layers[j]] = [layers[j], layers[i]];
    oncommit();
  }

  function drop(toShown: number) {
    if (dragFrom < 0 || dragFrom === toShown) return;
    const n = layers.length;
    const [moved] = layers.splice(n - 1 - dragFrom, 1);
    layers.splice(n - 1 - toShown, 0, moved);
    dragFrom = -1;
    oncommit();
  }

  function setMask(v: string) {
    if (!selected) return;
    if (v === "all") selected.mask = { type: "all" };
    else if (v.startsWith("zone:")) selected.mask = { type: "zone", id: Number(v.slice(5)) };
    else if (v === "leds" && selected.mask.type !== "leds") selected.mask = { type: "leds", leds: [...selection] };
    oncommit();
  }

  const maskValue = (l: Layer) => (l.mask.type === "zone" ? `zone:${l.mask.id}` : l.mask.type);
</script>

<div class="panel">
  <h3>{t("layers.title")}</h3>
  <div class="add">
    <select bind:value={newEffect} aria-label={t("layers.effect")}>
      {#each effects as d (d.id)}<option value={d.id}>{t(`fx.${d.id}`)}</option>{/each}
    </select>
    <button class="primary" onclick={add}>{t("layers.add")}</button>
  </div>

  {#if !layers.length}
    <p class="muted">{t("layers.empty")}</p>
  {/if}

  <ul>
    {#each shown as l, i (l.id)}
      <li
        class:sel={l.id === selectedId}
        class:off={!l.enabled}
        draggable="true"
        ondragstart={() => (dragFrom = i)}
        ondragover={(e) => e.preventDefault()}
        ondrop={() => drop(i)}
        ondragend={() => (dragFrom = -1)}
      >
        <input
          type="checkbox"
          checked={l.enabled}
          aria-label={t("layers.visible")}
          title={t("layers.visible")}
          onchange={(e) => {
            l.enabled = e.currentTarget.checked;
            oncommit();
          }}
        />
        <button class="name" onclick={() => (selectedId = l.id)} aria-pressed={l.id === selectedId}>{label(l)}</button>
        <button onclick={() => move(l, -1)} disabled={i === 0} aria-label={t("layers.up")} title={t("layers.up")}>↑</button>
        <button onclick={() => move(l, 1)} disabled={i === shown.length - 1} aria-label={t("layers.down")} title={t("layers.down")}>↓</button>
        <button class="danger" onclick={() => remove(l)} aria-label={t("common.delete")} title={t("common.delete")}>✕</button>
      </li>
    {/each}
  </ul>

  {#if selected && def}
    <div class="detail">
      <label class="row">
        <span>{t("layers.name")}</span>
        <input type="text" value={selected.name} placeholder={t(`fx.${selected.effect}`)} oninput={(e) => { selected.name = e.currentTarget.value; oncommit(); }} />
      </label>
      <label class="row">
        <span>{t("layers.opacity")}</span>
        <input type="range" min="0" max="1" step="0.01" value={selected.opacity} oninput={(e) => { selected.opacity = Number(e.currentTarget.value); oncommit(); }} />
        <output>{Math.round(selected.opacity * 100)}%</output>
      </label>
      <label class="row">
        <span>{t("layers.blend")}</span>
        <select value={selected.blend} onchange={(e) => { selected.blend = e.currentTarget.value as Blend; oncommit(); }}>
          {#each BLENDS as b (b)}<option value={b}>{t(`blend.${b}`)}</option>{/each}
        </select>
      </label>
      {#if zones}
        <label class="row">
          <span>{t("layers.mask")}</span>
          <select value={maskValue(selected)} onchange={(e) => setMask(e.currentTarget.value)}>
            <option value="all">{t("layers.maskAll")}</option>
            {#each zones.filter((z) => z.num_leds > 0) as z (z.id)}<option value={`zone:${z.id}`}>{z.name}</option>{/each}
            <option value="leds">{t("layers.maskLeds", { n: selected.mask.type === "leds" ? selected.mask.leds.length : 0 })}</option>
          </select>
        </label>
        <button
          disabled={!selection.size}
          onclick={() => {
            selected.mask = { type: "leds", leds: [...selection] };
            oncommit();
          }}>{t("layers.useSelection", { n: selection.size })}</button
        >
      {/if}
      {#if selected.effect === "animation"}<AnimationLayerParams layer={selected} {oncommit} />{/if}
      <EffectParams {def} params={selected.params} {oncommit} />
    </div>
  {/if}
</div>

<style>
  .panel { background: var(--panel); border: 1px solid var(--border); border-radius: 10px; padding: 0.8rem; }
  .add { display: flex; gap: 0.4rem; margin-bottom: 0.6rem; }
  .add select { flex: 1; min-width: 0; }
  ul { list-style: none; margin: 0 0 0.6rem; padding: 0; display: flex; flex-direction: column; gap: 0.25rem; }
  li { display: flex; align-items: center; gap: 0.3rem; padding: 0.2rem; border-radius: 6px; border: 1px solid transparent; }
  li.sel { background: var(--panel-2); border-color: var(--accent); }
  li.off .name { opacity: 0.5; }
  .name { flex: 1; text-align: left; background: transparent; border-color: transparent; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  li button:not(.name) { padding: 0.15rem 0.45rem; }
  .detail { border-top: 1px solid var(--border); padding-top: 0.6rem; }
  .row { display: grid; grid-template-columns: 6rem minmax(0, 1fr) 3rem; align-items: center; gap: 0.5rem; margin: 0.35rem 0; }
  .row > span:first-child { color: var(--muted); font-size: 0.9rem; }
  .row input[type="text"], .row select { grid-column: 2 / -1; min-width: 0; }
  .row output { text-align: right; }
</style>
