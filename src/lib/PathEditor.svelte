<script lang="ts">
  /** Path editor: drag over the LEDs to draw a route; a gradient then travels along it. */
  import { onDestroy, onMount, untrack } from "svelte";
  import type { Animation, Device, Rgb, Stop } from "./api";
  import { kindToPath, makeAnimation, pathToKind } from "./anim/draft";
    import { cycleSecs, flatten, renderAnimation } from "./anim/render";
  import { EditorSession, nextUpdated } from "./editorSession.svelte";
  import { t } from "./i18n/index.svelte";
  import { buildLayout } from "./layout/buildLayout";
  import { devScene } from "./scene.svelte";
  import { toast } from "./state.svelte";
  import EditorBar from "./EditorBar.svelte";
  import { animRole } from "./roles";
  import LedCanvas from "./LedCanvas.svelte";
  import PaletteEditor from "./PaletteEditor.svelte";

  let { device, initial = null }: { device: Device; initial?: Animation | null } = $props();
  const init = untrack(() => initial);
  const dev = untrack(() => device);

  const names = Array.from({ length: dev.colors.length }, (_, i) => dev.led_names[i] ?? "");
  const layout = $derived(buildLayout(dev, { fanLeds: devScene(dev.key).fan_leds }));
  const k = init?.kind.type === "path" ? init.kind : null;
  const start = k ? kindToPath(k, names) : null;

  let path = $state<number[]>(start?.path ?? []);
  let palette = $state<Stop[]>(k?.palette ?? [{ pos: 0, color: [200, 255, 220] }, { pos: 0.4, color: [0, 230, 120] }, { pos: 1, color: [0, 60, 90] }]);
  let speed = $state(k?.speed ?? 0.5);
  let length = $state(k?.length ?? 0.35);
  let gap = $state(k?.gap ?? 0.3);
  let tail = $state(k?.tail_fade ?? 0.6);
  let reverse = $state(k?.reverse ?? false);
  const reduced = typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  let playing = $state(!reduced);
  let playT = $state(0);

  const build = (): Animation =>
    makeAnimation(pathToKind($state.snapshot(path) as number[], names, { palette: $state.snapshot(palette) as Stop[], speed, length, gap, reverse, tail }), {
      id: session.savedId || "draft",
      name: session.savedName,
      tags: init?.tags ?? [],
      updated: nextUpdated(),
      target: session.target,
    });
  const session = new EditorSession(dev, build, init);

  const anim = $derived(build());
  const display = $derived.by(() => {
    if (!path.length) return null;
    return flatten(renderAnimation(anim, playing ? playT : cycleSecs(anim) * 0.3, names, layout.positions)) as (Rgb | null)[];
  });
  const blank = $derived(Array.from({ length: names.length }, () => null));

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

  function add(led: number) {
    if (path.includes(led)) return;
    path.push(led);
    session.touch();
  }
  function undoLast() {
    if (!path.length) return;
    path.pop();
    session.touch();
  }
  function clear() {
    path = [];
    session.touch();
  }
  const touch = () => session.touch();

  onMount(() => {
    if (start?.lost) toast(t("draw.lost", { n: start.lost }), "info", 6000);
    session.start();
  });
  onDestroy(() => session.stop());
</script>

<p class="muted help">{t("path.help")}</p>

<div class="bar">
  <button onclick={undoLast} disabled={!path.length}>{t("path.undo")}</button>
  <button onclick={clear} disabled={!path.length}>{t("path.clear")}</button>
  <span class="count muted">{t("path.count", { n: path.length })}</span>
  <button class="play" onclick={() => (playing = !playing)} aria-pressed={playing}>{playing ? "⏸ " + t("frames.pause") : "▶ " + t("frames.play")}</button>
</div>

<LedCanvas {layout} colors={blank} {display} tools={["path"]} {path} onpathadd={add} onchange={() => {}} history={false} />

<div class="grid">
  <div class="pal">
    <h4>{t("pal.title")}</h4>
    <PaletteEditor stops={palette} onchange={(s) => { palette = s; touch(); }} />
  </div>
  <div class="sliders">
    <h4>{t("path.motion")}</h4>
    <label class="row"><span>{t("param.speed")}</span><input type="range" min="0.05" max="3" step="0.01" value={speed} oninput={(e) => { speed = Number(e.currentTarget.value); touch(); }} /><output>{speed.toFixed(2)}</output></label>
    <label class="row"><span>{t("param.length")}</span><input type="range" min="0.05" max="1" step="0.01" value={length} oninput={(e) => { length = Number(e.currentTarget.value); touch(); }} /><output>{length.toFixed(2)}</output></label>
    <label class="row"><span>{t("param.gap")}</span><input type="range" min="0" max="1" step="0.01" value={gap} oninput={(e) => { gap = Number(e.currentTarget.value); touch(); }} /><output>{gap.toFixed(2)}</output></label>
    <label class="row"><span>{t("param.tail_fade")}</span><input type="range" min="0" max="1" step="0.01" value={tail} oninput={(e) => { tail = Number(e.currentTarget.value); touch(); }} /><output>{tail.toFixed(2)}</output></label>
    <label class="row"><span>{t("param.reverse")}</span><input type="checkbox" checked={reverse} onchange={(e) => { reverse = e.currentTarget.checked; touch(); }} /></label>
  </div>
</div>

<EditorBar {session} empty={!path.length} defaultName={init && !init.builtin ? init.name : ""} defaultTags={init?.tags ?? []} defaultRole={init ? animRole(init) : "theme"} />

<style>
  .help { margin: 0 0 0.6rem; }
  .bar { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; margin-bottom: 0.6rem; }
  .count { margin-right: auto; }
  .grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 1rem; margin-top: 0.9rem; }
  .pal, .sliders { background: var(--panel); border: 1px solid var(--border); border-radius: 10px; padding: 0.7rem 0.9rem; }
  h4 { margin: 0 0 0.5rem; }
  .row { display: grid; grid-template-columns: 6rem minmax(0, 1fr) 3rem; align-items: center; gap: 0.5rem; margin: 0.35rem 0; }
  .row > span { color: var(--muted); font-size: 0.9rem; }
  .row output { text-align: right; font-variant-numeric: tabular-nums; }
  @media (max-width: 800px) { .grid { grid-template-columns: minmax(0, 1fr); } }
</style>
