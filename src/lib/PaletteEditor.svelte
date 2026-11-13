<script lang="ts">
  /** Gradient editor: a bar with draggable colour stops, presets, add/remove, per-stop colour. */
  import { hex, type Color, type Stop } from "./api";
  import ColorPicker from "./ColorPicker.svelte";
  import { cloneColor, resolveColor } from "./palette.svelte";
  import { PALETTE_PRESETS, sampleStops, sortStops } from "./anim/palette";
  import { t } from "./i18n/index.svelte";

  let { stops, onchange, cyclic = false }: { stops: Stop[]; onchange: (s: Stop[]) => void; cyclic?: boolean } = $props();

  let sel = $state(0);
  let bar: HTMLDivElement | undefined = $state();
  let drag = -1;

  const sorted = $derived(sortStops(stops));
  const css = $derived(
    `linear-gradient(90deg, ${Array.from({ length: 33 }, (_, i) => {
      const c = sampleStops(sorted, i / 32, cyclic);
      return `rgb(${Math.round(c[0])},${Math.round(c[1])},${Math.round(c[2])}) ${(i / 32) * 100}%`;
    }).join(", ")})`,
  );

  const cur = $derived(sorted[Math.min(sel, sorted.length - 1)]);

  function commit(next: Stop[], keep?: number) {
    onchange(next.map((s) => ({ pos: Math.min(1, Math.max(0, Math.round(s.pos * 1000) / 1000)), color: cloneColor(s.color) })));
    if (keep !== undefined) sel = keep;
  }

  function addAt(e: MouseEvent) {
    if (!bar || (e.target as HTMLElement).closest(".stop")) return;
    const r = bar.getBoundingClientRect();
    const pos = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
    const color: Color = sampleStops(sorted, pos, cyclic).map(Math.round) as [number, number, number];
    const next = sortStops([...sorted, { pos, color }]);
    commit(next, next.findIndex((s) => s.pos === pos));
  }

  function down(i: number, e: PointerEvent) {
    drag = i;
    sel = i;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function move(e: PointerEvent) {
    if (drag < 0 || !bar) return;
    const r = bar.getBoundingClientRect();
    const pos = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
    const next = sortStops(sorted.map((s, i) => (i === drag ? { ...s, pos } : s)));
    // Dragging past a neighbour reorders the stops: keep following the one being dragged.
    drag = sel = next.findIndex((s) => s.pos === pos);
    commit(next);
  }
  function up() {
    drag = -1;
  }
  function keyStop(i: number, e: KeyboardEvent) {
    const d = e.key === "ArrowLeft" ? -0.02 : e.key === "ArrowRight" ? 0.02 : 0;
    if (d) {
      e.preventDefault();
      commit(sorted.map((s, k) => (k === i ? { ...s, pos: s.pos + d } : s)));
    } else if ((e.key === "Delete" || e.key === "Backspace") && sorted.length > 1) {
      e.preventDefault();
      remove(i);
    }
  }

  function setColor(c: Color) {
    commit(sorted.map((s, i) => (i === sel ? { ...s, color: c } : s)));
  }
  function remove(i = sel) {
    if (sorted.length <= 1) return;
    commit(sorted.filter((_, k) => k !== i), Math.max(0, i - 1));
  }
  function addButton() {
    // Between the selected stop and its neighbour (or at the end).
    const a = sorted[Math.min(sel, sorted.length - 1)];
    const b = sorted[sel + 1] ?? { pos: 1, color: a.color };
    const pos = a === b || b.pos === a.pos ? Math.min(1, a.pos + 0.1) : (a.pos + b.pos) / 2;
    const next = sortStops([...sorted, { pos, color: sampleStops(sorted, pos, cyclic).map(Math.round) as [number, number, number] }]);
    commit(next, next.findIndex((s) => s.pos === pos));
  }
</script>

<div class="pal">
  <div class="barwrap">
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="bar" bind:this={bar} style:background={css} onclick={addAt} title={t("pal.addHint")}>
      {#each sorted as s, i (i)}
        <button
          class="stop"
          class:on={i === sel}
          style:left={`${s.pos * 100}%`}
          style:background={hex(resolveColor(s.color))}
          aria-label={t("pal.stopN", { n: i + 1 })}
          title={`${Math.round(s.pos * 100)}%`}
          onpointerdown={(e) => down(i, e)}
          onpointermove={move}
          onpointerup={up}
          onkeydown={(e) => keyStop(i, e)}
        ></button>
      {/each}
    </div>
  </div>
  <div class="row">
    {#if cur}
      <ColorPicker value={cur.color} onchange={setColor} label={t("pal.stopColour")} />
      <span class="muted pos">{Math.round(cur.pos * 100)}%</span>
    {/if}
    <button onclick={addButton} disabled={sorted.length >= 16}>{t("pal.add")}</button>
    <button onclick={() => remove()} disabled={sorted.length <= 1}>{t("pal.remove")}</button>
    <select aria-label={t("pal.presets")} onchange={(e) => { const p = PALETTE_PRESETS.find((x) => x.key === e.currentTarget.value); if (p) commit(p.stops, 0); e.currentTarget.value = ""; }}>
      <option value="">{t("pal.presets")}…</option>
      {#each PALETTE_PRESETS as p (p.key)}<option value={p.key}>{t(`pal.${p.key}`)}</option>{/each}
    </select>
  </div>
</div>

<style>
  .pal { display: flex; flex-direction: column; gap: 0.5rem; }
  .barwrap { padding: 0.3rem 0.7rem 0.9rem; }
  .bar { position: relative; height: 1.6rem; border-radius: 6px; border: 1px solid var(--border); cursor: copy; touch-action: none; }
  .stop { position: absolute; top: 100%; width: 1rem; height: 1rem; margin: 0.15rem 0 0 -0.5rem; padding: 0; border-radius: 50% 50% 50% 50%; border: 2px solid var(--text); cursor: grab; box-shadow: 0 1px 4px #0008; touch-action: none; }
  .stop.on { outline: 2px solid var(--accent); outline-offset: 2px; }
  .row { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; }
  .pos { min-width: 2.6rem; font-variant-numeric: tabular-nums; }
</style>
