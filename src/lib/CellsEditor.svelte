<script lang="ts">
  /** Palette editor: shape a region with the gradient tool; its colours then cycle through the palette. */
  import { onDestroy, onMount, untrack } from "svelte";
  import type { Animation, Device, Rgb, Stop } from "./api";
  import { cellsToKind, kindToCells, makeAnimation } from "./anim/draft";
  import { PALETTE_PRESETS, sampleStops, sortStops } from "./anim/palette";
  import { cycleSecs, flatten, renderAnimation } from "./anim/render";
  import { EditorSession, nextUpdated } from "./editorSession.svelte";
  import { t } from "./i18n/index.svelte";
  import { buildLayout } from "./layout/buildLayout";
  import { devScene } from "./scene.svelte";
  import { toast } from "./state.svelte";
  import EditorBar from "./EditorBar.svelte";
  import LedCanvas, { type Paint } from "./LedCanvas.svelte";
  import PaletteEditor from "./PaletteEditor.svelte";

  let { device, initial = null }: { device: Device; initial?: Animation | null } = $props();
  const init = untrack(() => initial);
  const dev = untrack(() => device);

  const names = Array.from({ length: dev.colors.length }, (_, i) => dev.led_names[i] ?? "");
  const layout = $derived(buildLayout(dev, { fanLeds: devScene(dev.key).fan_leds }));
  const k = init?.kind.type === "palette" ? init.kind : null;
  const start = k ? kindToCells(k, names) : null;

  type Cells = Record<number, number>;
  let cells = $state<Cells>(start?.cells ?? {});
  let palette = $state<Stop[]>(k?.palette ?? PALETTE_PRESETS[1].stops.map((s) => ({ pos: s.pos, color: [...s.color] as Rgb })));
  let speed = $state(k?.speed ?? 0.3);
  let reverse = $state(k?.reverse ?? false);
  const reduced = typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  let playing = $state(!reduced);
  let playT = $state(0);
  let selected = $state(new Set<number>());

  // Own history: the cells cannot be rebuilt from colours.
  let undo = $state<Cells[]>([]);
  let redo = $state<Cells[]>([]);
  const snap = (): Cells => ({ ...$state.snapshot(cells) });

  const build = (): Animation =>
    makeAnimation(cellsToKind(snap(), names, { palette: $state.snapshot(palette) as Stop[], speed, reverse }), {
      id: session.savedId || "draft",
      name: session.savedName,
      tags: init?.tags ?? [],
      updated: nextUpdated(),
      target: session.target,
    });
  const session = new EditorSession(dev, build, init);

  const count = $derived(Object.keys(cells).length);
  const anim = $derived(build());
  /** What the cells look like with no rotation: the shape the user is drawing. */
  const shape: Paint = $derived.by(() => {
    const sorted = sortStops(palette);
    return Array.from({ length: names.length }, (_, i) => (cells[i] === undefined ? null : (sampleStops(sorted, cells[i], true).map(Math.round) as Rgb)));
  });
  const display = $derived.by(() => {
    if (!count) return null;
    return flatten(renderAnimation(anim, playing ? playT : cycleSecs(anim) * 0.3, names, layout.positions)) as (Rgb | null)[];
  });

  $effect(() => {
    if (!playing) return;
    let raf = 0;
    const t0 = performance.now() - playT * 1000;
    const loopFn = (now: number) => {
      playT = (now - t0) / 1000;
      raf = requestAnimationFrame(loopFn);
    };
    raf = requestAnimationFrame(loopFn);
    return () => cancelAnimationFrame(raf);
  });

  function remember() {
    undo = [...undo.slice(-49), snap()];
    redo = [];
  }
  function setCells(next: Cells) {
    cells = next;
    session.touch();
  }
  /** Gradient tool: assigns palette positions to the dragged region. */
  function gradient(m: Map<number, number>) {
    remember();
    const next = snap();
    for (const [led, pos] of m) next[led] = Math.round(pos * 1000) / 1000;
    setCells(next);
  }
  /** Eraser / clear: drop the cells of LEDs that became transparent. */
  function changed(next: Paint) {
    const nextCells = snap();
    let any = false;
    next.forEach((c, i) => {
      if (!c && nextCells[i] !== undefined) {
        delete nextCells[i];
        any = true;
      }
    });
    if (!any) return;
    remember();
    setCells(nextCells);
  }
  function doUndo() {
    const prev = undo.at(-1);
    if (!prev) return;
    redo = [...redo, snap()];
    undo = undo.slice(0, -1);
    setCells(prev);
  }
  function doRedo() {
    const nx = redo.at(-1);
    if (!nx) return;
    undo = [...undo, snap()];
    redo = redo.slice(0, -1);
    setCells(nx);
  }
  function onKey(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement | null)?.tagName;
    if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA" || !(e.ctrlKey || e.metaKey)) return;
    const key = e.key.toLowerCase();
    if (key === "z") {
      e.preventDefault();
      if (e.shiftKey) doRedo();
      else doUndo();
    } else if (key === "y") {
      e.preventDefault();
      doRedo();
    }
  }

  onMount(() => {
    if (start?.lost) toast(t("draw.lost", { n: start.lost }), "info", 6000);
    session.start();
  });
  onDestroy(() => session.stop());
</script>

<svelte:window onkeydown={onKey} />

<p class="muted help">{t("cells.help")}</p>

<div class="bar">
  <span class="count muted">{t("cells.count", { n: count })}</span>
  <span class="hist">
    <button onclick={doUndo} disabled={!undo.length} aria-label={t("canvas.undo")} title="Ctrl+Z">↶</button>
    <button onclick={doRedo} disabled={!redo.length} aria-label={t("canvas.redo")} title="Ctrl+Shift+Z">↷</button>
  </span>
  <button class="play" onclick={() => (playing = !playing)} aria-pressed={playing}>{playing ? "⏸ " + t("frames.pause") : "▶ " + t("frames.play")}</button>
</div>

<LedCanvas
  {layout}
  colors={shape}
  {display}
  transparent
  bind:selected
  resetKey="cells"
  onchange={changed}
  tools={["gradient", "select", "eraser"]}
  ongradient={gradient}
  history={false}
/>

<div class="grid">
  <div class="pal">
    <h4>{t("pal.title")}</h4>
    <PaletteEditor stops={palette} cyclic onchange={(s) => { palette = s; session.touch(); }} />
  </div>
  <div class="sliders">
    <h4>{t("path.motion")}</h4>
    <label class="row"><span>{t("param.speed")}</span><input type="range" min="0" max="2" step="0.01" value={speed} oninput={(e) => { speed = Number(e.currentTarget.value); session.touch(); }} /><output>{speed.toFixed(2)}</output></label>
    <label class="row"><span>{t("param.reverse")}</span><input type="checkbox" checked={reverse} onchange={(e) => { reverse = e.currentTarget.checked; session.touch(); }} /></label>
  </div>
</div>

<EditorBar {session} empty={!count} defaultName={init && !init.builtin ? init.name : ""} defaultTags={init?.tags ?? []} />

<style>
  .help { margin: 0 0 0.6rem; }
  .bar { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; margin-bottom: 0.6rem; }
  .count { margin-right: auto; }
  .hist { display: flex; gap: 0.25rem; }
  .grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 1rem; margin-top: 0.9rem; }
  .pal, .sliders { background: var(--panel); border: 1px solid var(--border); border-radius: 10px; padding: 0.7rem 0.9rem; }
  h4 { margin: 0 0 0.5rem; }
  .row { display: grid; grid-template-columns: 6rem minmax(0, 1fr) 3rem; align-items: center; gap: 0.5rem; margin: 0.35rem 0; }
  .row > span { color: var(--muted); font-size: 0.9rem; }
  .row output { text-align: right; font-variant-numeric: tabular-nums; }
  @media (max-width: 800px) { .grid { grid-template-columns: minmax(0, 1fr); } }
</style>
