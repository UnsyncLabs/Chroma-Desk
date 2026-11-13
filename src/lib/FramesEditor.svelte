<script lang="ts">
  /** Flipbook editor: a timeline of frames, each painted on the canvas; plays in a loop. */
  import { onDestroy, onMount, untrack } from "svelte";
  import type { Animation, Device, LoopMode, Rgb } from "./api";
  import { blankPaint, clonePaint, framesToKind, kindToFrames, makeAnimation, newFrameId, type FrameDraft } from "./anim/draft";
  import { flatten, renderAnimation } from "./anim/render";
  import { EditorSession, nextUpdated } from "./editorSession.svelte";
  import { t } from "./i18n/index.svelte";
  import { buildLayout } from "./layout/buildLayout";
  import { devScene, frames as liveFrames } from "./scene.svelte";
  import { toast } from "./state.svelte";
  import EditorBar from "./EditorBar.svelte";
  import FrameThumb from "./FrameThumb.svelte";
  import LedCanvas, { type Paint } from "./LedCanvas.svelte";

  let { device, initial = null }: { device: Device; initial?: Animation | null } = $props();
  const init = untrack(() => initial);
  const dev = untrack(() => device);

  const names = Array.from({ length: dev.colors.length }, (_, i) => dev.led_names[i] ?? "");
  const n = names.length;
  const layout = $derived(buildLayout(dev, { fanLeds: devScene(dev.key).fan_leds }));

  // ----- model -----
  const start = init?.kind.type === "frames" ? kindToFrames(init.kind, names) : null;
  let frames = $state<FrameDraft[]>(start?.frames ?? [{ id: newFrameId(), leds: blankPaint(n), ms: null }]);
  let cur = $state(0);
  let frameMs = $state(init?.kind.type === "frames" ? init.kind.frame_ms : 250);
  let loop = $state<LoopMode>(init?.kind.type === "frames" ? init.kind.loop : "loop");
  let crossfade = $state(init?.kind.type === "frames" ? init.kind.crossfade : 0);
  let onion = $state(true);
  let playing = $state(false);
  let playT = $state(0);
  const lost = start?.lost ?? 0;

  const build = (): Animation =>
    makeAnimation(framesToKind($state.snapshot(frames) as FrameDraft[], names, { frameMs, loop, crossfade }), {
      id: session.savedId || "draft",
      name: session.savedName,
      tags: init?.tags ?? [],
      updated: nextUpdated(),
      target: session.target,
    });
  const session = new EditorSession(dev, build, init);

  const empty = $derived(frames.every((f) => f.leds.every((c) => !c)));
  const current = $derived(frames[Math.min(cur, frames.length - 1)]);

  // ----- playback preview (same renderer the thumbnails use) -----
  const preview = $derived.by(() => {
    if (!playing) return null;
    const px = renderAnimation(build(), playT, names, layout.positions);
    return flatten(px) as (Rgb | null)[];
  });
  // Onion skin: the previous frame shows through, dimmed, where this frame is empty.
  const onionDisplay = $derived.by(() => {
    if (playing || !onion || cur === 0) return null;
    const prev = frames[cur - 1]?.leds;
    if (!prev) return null;
    return current.leds.map((c, i) => c ?? (prev[i] ? (prev[i]!.map((v) => Math.round(v * 0.28)) as Rgb) : null));
  });
  const display = $derived(preview ?? onionDisplay);

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

  // ----- editing -----
  function setLeds(next: Paint) {
    current.leds = next;
    session.touch();
  }
  function addFrame() {
    frames.splice(cur + 1, 0, { id: newFrameId(), leds: blankPaint(n), ms: null });
    cur++;
    session.touch();
  }
  function duplicateFrame() {
    frames.splice(cur + 1, 0, { id: newFrameId(), leds: clonePaint(current.leds), ms: current.ms });
    cur++;
    session.touch();
  }
  function removeFrame() {
    if (frames.length <= 1) {
      toast(t("frames.lastOne"), "info", 2500);
      return;
    }
    frames.splice(cur, 1);
    cur = Math.min(cur, frames.length - 1);
    session.touch();
  }
  function moveFrame(dir: -1 | 1) {
    const j = cur + dir;
    if (j < 0 || j >= frames.length) return;
    [frames[cur], frames[j]] = [frames[j], frames[cur]];
    cur = j;
    session.touch();
  }
  function setMs(v: string) {
    current.ms = v === "" ? null : Math.min(60000, Math.max(10, Number(v) || 0));
    session.touch();
  }

  let dragFrom = -1;
  function drop(to: number) {
    if (dragFrom < 0 || dragFrom === to) return;
    const [m] = frames.splice(dragFrom, 1);
    frames.splice(to, 0, m);
    cur = to;
    dragFrom = -1;
    session.touch();
  }

  function onKey(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement | null)?.tagName;
    if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA" || e.ctrlKey || e.metaKey) return;
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      cur = Math.max(0, cur - 1);
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      cur = Math.min(frames.length - 1, cur + 1);
    }
  }

  let selected = $state(new Set<number>());
  onMount(() => {
    if (lost) toast(t("draw.lost", { n: lost }), "info", 6000);
    session.start();
  });
  onDestroy(() => session.stop());
</script>

<svelte:window onkeydown={onKey} />

<p class="muted help">{t("frames.help")}</p>

<div class="timeline" role="listbox" aria-label={t("frames.timeline")}>
  {#each frames as f, i (f.id)}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      class="slot"
      class:on={i === cur}
      role="option"
      tabindex="0"
      aria-selected={i === cur}
      aria-label={t("frames.frameN", { n: i + 1 })}
      draggable="true"
      ondragstart={() => (dragFrom = i)}
      ondragover={(e) => e.preventDefault()}
      ondrop={() => drop(i)}
      onclick={() => (cur = i)}
      onkeydown={(e) => (e.key === "Enter" || e.key === " ") && ((e.preventDefault(), (cur = i)))}
    >
      <FrameThumb leds={f.leds} {layout} />
      <span class="meta"><b>{i + 1}</b><span class="muted">{f.ms ?? frameMs} ms</span></span>
    </div>
  {/each}
  <button class="addslot" onclick={addFrame} aria-label={t("frames.add")} title={t("frames.add")}>+</button>
</div>

<div class="frameBar">
  <button class="play primary" onclick={() => { playing = !playing; if (playing) playT = 0; }} aria-pressed={playing}>{playing ? "⏸ " + t("frames.pause") : "▶ " + t("frames.play")}</button>
  <button onclick={addFrame}>{t("frames.add")}</button>
  <button onclick={duplicateFrame}>{t("frames.duplicate")}</button>
  <button onclick={removeFrame} disabled={frames.length <= 1}>{t("frames.delete")}</button>
  <button onclick={() => moveFrame(-1)} disabled={cur === 0} aria-label={t("frames.left")} title={t("frames.left")}>◀</button>
  <button onclick={() => moveFrame(1)} disabled={cur >= frames.length - 1} aria-label={t("frames.right")} title={t("frames.right")}>▶</button>
  <label class="inline">
    <span>{t("frames.thisMs")}</span>
    <input type="number" min="10" max="60000" step="10" placeholder={String(frameMs)} value={current.ms ?? ""} onchange={(e) => setMs(e.currentTarget.value)} />
  </label>
</div>

{#if playing}<p class="banner">{t("frames.playingHint")}</p>{/if}

<LedCanvas
  {layout}
  colors={current.leds}
  {display}
  editable={!playing}
  transparent
  bind:selected
  resetKey={`frame-${current.id}`}
  onchange={setLeds}
  tools={["brush", "select", "picker", "eraser"]}
/>

<div class="settings">
  <label class="inline">
    <span>{t("frames.speed")}</span>
    <input type="range" min="20" max="1000" step="10" value={frameMs} oninput={(e) => { frameMs = Number(e.currentTarget.value); session.touch(); }} />
    <output>{frameMs} ms</output>
  </label>
  <label class="inline">
    <span>{t("frames.loop")}</span>
    <select value={loop} onchange={(e) => { loop = e.currentTarget.value as LoopMode; session.touch(); }}>
      <option value="loop">{t("frames.loop.loop")}</option>
      <option value="pingpong">{t("frames.loop.pingpong")}</option>
      <option value="once">{t("frames.loop.once")}</option>
    </select>
  </label>
  <label class="inline">
    <span>{t("param.crossfade")}</span>
    <input type="range" min="0" max="1" step="0.05" value={crossfade} oninput={(e) => { crossfade = Number(e.currentTarget.value); session.touch(); }} />
    <output>{Math.round(crossfade * 100)}%</output>
  </label>
  <label class="inline check">
    <input type="checkbox" checked={onion} onchange={(e) => (onion = e.currentTarget.checked)} />
    <span>{t("frames.onion")}</span>
  </label>
</div>

<EditorBar {session} {empty} defaultName={init && !init.builtin ? init.name : ""} defaultTags={init?.tags ?? []} />

<style>
  .help { margin: 0 0 0.6rem; }
  .timeline { display: flex; gap: 0.5rem; overflow-x: auto; padding: 0.3rem 0.1rem 0.6rem; margin-bottom: 0.5rem; }
  .slot { flex: 0 0 8.5rem; background: var(--panel); border: 2px solid var(--border); border-radius: 8px; padding: 0.3rem; cursor: pointer; display: flex; flex-direction: column; gap: 0.2rem; }
  .slot.on { border-color: var(--accent); background: var(--panel-2); }
  .slot:focus-visible { outline: 2px solid var(--accent); }
  .meta { display: flex; justify-content: space-between; font-size: 0.78rem; }
  .addslot { flex: 0 0 3rem; font-size: 1.6rem; border-style: dashed; border-radius: 8px; }
  .frameBar { display: flex; gap: 0.4rem; flex-wrap: wrap; align-items: center; margin-bottom: 0.7rem; }
  .play { min-width: 6.5rem; }
  .inline { display: flex; align-items: center; gap: 0.4rem; }
  .inline > span { color: var(--muted); font-size: 0.88rem; }
  .frameBar input[type="number"] { width: 5rem; }
  .banner { background: var(--panel-2); border-radius: 8px; padding: 0.4rem 0.8rem; margin: 0 0 0.6rem; }
  .settings { display: flex; gap: 0.8rem 1.4rem; flex-wrap: wrap; align-items: center; margin-top: 0.8rem; }
  .settings output { min-width: 3.2rem; font-variant-numeric: tabular-nums; }
  .check { gap: 0.4rem; }
</style>
