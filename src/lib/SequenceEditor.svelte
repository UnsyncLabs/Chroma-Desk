<script lang="ts">
  /**
   * Sequence editor: a list of steps (meter, blink, orbit, library animation, colour, pause) played one after
   * another, with the keys of each step picked in order on the keyboard drawing and a live preview.
   */
  import { onDestroy, untrack } from "svelte";
  import { hex, type Animation, type Color, type Device, type ParamValue, type Rgb, type SeqStep } from "./api";
  import ColorPicker from "./ColorPicker.svelte";
  import LedCanvas from "./LedCanvas.svelte";
  import { animName, lib } from "./animationsStore.svelte";
  import { cycleSecs, flatten, renderAnimation } from "./anim/render";
  import { nameIndex, refMaker, resolveRef } from "./anim/ref";
  import { t } from "./i18n/index.svelte";
  import type { Layout } from "./layout/buildLayout";
  import { resolveColor } from "./palette.svelte";
  import { METRICS, defaultRange, describeLive, isTemp, metricLabel, sysmon, unitOf, watchMetrics } from "./sysmon.svelte";

  let { seq = $bindable(), kb, layout }: { seq: Animation; kb: Device | null; layout: Layout | null } = $props();

  type Style = "meter" | "blink" | "orbit" | "animation" | "solid" | "hold";
  const STYLES: Style[] = ["meter", "blink", "orbit", "animation", "solid", "hold"];
  const PATHY: string[] = ["meter", "blink", "orbit"];

  const steps = $derived(seq.kind.type === "sequence" ? seq.kind.steps : []);
  let sel = $state(0);
  const step = $derived<SeqStep | null>(steps[Math.min(sel, steps.length - 1)] ?? null);

  const names = $derived(kb ? Array.from({ length: kb.colors.length }, (_, i) => kb.led_names[i] ?? "") : []);
  const refOf = $derived(refMaker(names));
  const index = $derived(nameIndex(names));
  const ledsOf = (st: SeqStep): number[] => st.keys.map((r) => resolveRef(r, index, names.length)).filter((i): i is number => i !== null);

  // ---------- step defaults and helpers ----------
  const slot = (s: string): Color => ({ slot: s });
  function fresh(style: Style): SeqStep {
    switch (style) {
      case "meter":
        return { effect: "meter", params: { metric: "cpu.usage", color: slot("ok"), min: 0, max: 100, style: "sweep", sweep_ms: 1200, background: "none" }, keys: [], duration_ms: 3200 };
      case "blink":
        return { effect: "blink", params: { color: slot("alert"), count: 5, on_ms: 150, off_ms: 150 }, keys: [], duration_ms: 1500 };
      case "orbit":
        return { effect: "orbit", params: { color: slot("accent"), laps: 1, lap_ms: 800, trail: 0.5 }, keys: [], duration_ms: 800 };
      case "animation":
        return { effect: "animation", params: {}, keys: [], duration_ms: 2000, anim: null };
      case "solid":
        return { effect: "solid", params: { color: slot("accent") }, keys: [], duration_ms: 1000 };
      case "hold":
        return { effect: "hold", params: {}, keys: [], duration_ms: 500 };
    }
  }
  const styleOf = (st: SeqStep): Style => (STYLES.includes(st.effect as Style) ? (st.effect as Style) : "solid");
  const num = (st: SeqStep, k: string, d: number) => (typeof st.params[k] === "number" ? (st.params[k] as number) : d);
  const colorOf = (st: SeqStep): Color => (st.params.color as Color | undefined) ?? slot("accent");
  const set = (st: SeqStep, k: string, v: ParamValue) => {
    st.params[k] = v;
    syncDuration(st);
  };
  /** Blink and orbit last exactly as long as their own cycle. */
  function syncDuration(st: SeqStep) {
    if (st.effect === "blink") st.duration_ms = Math.round(num(st, "count", 5) * (num(st, "on_ms", 150) + num(st, "off_ms", 150)));
    if (st.effect === "orbit") st.duration_ms = Math.round(num(st, "laps", 1) * num(st, "lap_ms", 800));
  }

  function add(style: Style) {
    if (seq.kind.type !== "sequence" || seq.kind.steps.length >= 64) return;
    seq.kind.steps.push(fresh(style));
    sel = seq.kind.steps.length - 1;
  }
  function remove(i: number) {
    if (seq.kind.type !== "sequence") return;
    seq.kind.steps.splice(i, 1);
    sel = Math.max(0, Math.min(sel, seq.kind.steps.length - 1));
  }
  function move(i: number, to: number) {
    if (seq.kind.type !== "sequence" || to < 0 || to >= seq.kind.steps.length || i === to) return;
    const [x] = seq.kind.steps.splice(i, 1);
    seq.kind.steps.splice(to, 0, x);
    sel = to;
  }

  // ---------- drag and drop (arrows also work: they are the keyboard-accessible way) ----------
  let dragFrom: number | null = $state(null);
  let dragOver: number | null = $state(null);

  // ---------- keys, in order ----------
  function addKey(led: number) {
    if (!step) return;
    const r = refOf(led);
    if (!step.keys.includes(r)) step.keys.push(r);
  }
  const unpicked = () => {
    if (step) step.keys = [];
  };
  function undoKey() {
    step?.keys.pop();
  }
  /** Quick sets of keys, in the order they should fill / orbit. */
  const KEYSETS: { id: string; keys: string[] }[] = [
    { id: "frow", keys: ["Escape", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12"] },
    { id: "numrow", keys: ["`", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "=", "Backspace"] },
    { id: "wasd", keys: ["W", "D", "S", "A"] },
    { id: "arrows", keys: ["Up Arrow", "Right Arrow", "Down Arrow", "Left Arrow"] },
  ];
  function useKeys(ids: string[]) {
    if (!step) return;
    const out: string[] = [];
    for (const k of ids) {
      const i = index.get(`Key: ${k}`);
      if (i !== undefined) out.push(refOf(i));
    }
    step.keys = out;
  }
  const keysetAvailable = (ids: string[]) => ids.some((k) => index.has(`Key: ${k}`));

  // ---------- live values ----------
  const metricsUsed = $derived([...new Set(steps.filter((s) => s.effect === "meter").map((s) => String(s.params.metric ?? "")).filter(Boolean))]);
  $effect(() => {
    const ids = metricsUsed;
    return watchMetrics(ids.length ? ids : ["cpu.usage"]);
  });
  function pickMetric(st: SeqStep, m: string) {
    const [lo, hi] = defaultRange(m);
    st.params.metric = m;
    st.params.min = lo;
    st.params.max = hi;
  }

  // ---------- preview ----------
  const reduced = typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  // svelte-ignore state_referenced_locally
  let playing = $state(!reduced);
  let playT = $state(0);
  let raf = 0;
  $effect(() => {
    if (!playing) return;
    const t0 = performance.now() - untrack(() => playT) * 1000;
    const loop = (now: number) => {
      playT = (now - t0) / 1000;
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  });
  onDestroy(() => cancelAnimationFrame(raf));
  const total = $derived(Math.max(cycleSecs(seq), 0.1));
  const frame = $derived.by(() => {
    if (!layout || !names.length) return null;
    const probe: Animation = { ...seq, kind: seq.kind.type === "sequence" ? { ...seq.kind, loop: true } : seq.kind };
    const px = renderAnimation(probe, playing ? playT : total * 0.45, names, layout.positions);
    const flat = flatten(px);
    return px.map((p, i) => (p[3] > 0.02 ? flat[i] : null)) as (Rgb | null)[];
  });
  const blank = $derived<(Color | null)[]>(names.map(() => null));
  const pathNow = $derived(step && PATHY.includes(step.effect) ? ledsOf(step) : []);
  const startOf = (i: number) => steps.slice(0, i).reduce((s, x) => s + x.duration_ms, 0) / 1000;

  // ---------- summaries ----------
  const secs = (ms: number) => `${(ms / 1000).toFixed(ms % 1000 ? 1 : 0)} s`;
  function summary(st: SeqStep): string {
    switch (styleOf(st)) {
      case "meter":
        return metricLabel(String(st.params.metric ?? ""));
      case "blink":
        return t("seq.blinkSummary", { n: num(st, "count", 5) });
      case "orbit":
        return t("seq.orbitSummary", { n: num(st, "laps", 1) });
      case "animation":
        return st.anim ? animName(st.anim) : t("seq.noAnim");
      case "solid":
        return t("seq.solidSummary");
      default:
        return t("seq.holdSummary");
    }
  }
  const dotColor = (st: SeqStep): string | null => (st.params.color ? hex(paletteRgb(st.params.color as Color)) : null);
  const paletteRgb = (c: Color) => resolveColor(c);

  const animOptions = $derived(lib.list.filter((a) => a.kind.type !== "sequence"));
  function chooseAnim(st: SeqStep, id: string) {
    const a = lib.list.find((x) => x.id === id);
    st.anim = a ? JSON.parse(JSON.stringify(a)) : null;
  }
</script>

<div class="seq" data-sequence>
  <div class="head">
    <h4>{t("seq.steps")}</h4>
    <span class="muted small">{t("seq.total", { s: total.toFixed(1) })}</span>
    <label class="check loop"><input type="checkbox" checked={seq.kind.type === "sequence" && seq.kind.loop} onchange={(e) => seq.kind.type === "sequence" && (seq.kind.loop = e.currentTarget.checked)} data-seq-loop /> {t("seq.loop")}</label>
  </div>

  <ol class="steps" data-steps>
    {#each steps as st, i (st)}
      <li
        class="step"
        class:sel={i === sel}
        class:over={dragOver === i && dragFrom !== null && dragFrom !== i}
        data-step={i}
        draggable="true"
        ondragstart={(e) => {
          dragFrom = i;
          e.dataTransfer?.setData("text/plain", String(i));
        }}
        ondragover={(e) => {
          e.preventDefault();
          dragOver = i;
        }}
        ondragleave={() => (dragOver = null)}
        ondrop={(e) => {
          e.preventDefault();
          if (dragFrom !== null) move(dragFrom, i);
          dragFrom = dragOver = null;
        }}
        ondragend={() => (dragFrom = dragOver = null)}
      >
        <div class="bar">
          <span class="grip" aria-hidden="true" title={t("seq.drag")}>⠿</span>
          <button class="pick" onclick={() => (sel = i)} aria-label={t("seq.select", { n: i + 1 })} aria-expanded={i === sel} data-step-select>
            <span class="num">{i + 1}</span>
            {#if dotColor(st)}<span class="dot" style:background={dotColor(st)}></span>{/if}
            <b>{t(`seq.style.${styleOf(st)}`)}</b>
            <span class="sum">{summary(st)}</span>
            <span class="dur">{secs(st.duration_ms)}</span>
          </button>
          <span class="acts">
            <button onclick={() => move(i, i - 1)} disabled={i === 0} aria-label={t("seq.up", { n: i + 1 })} data-step-up>↑</button>
            <button onclick={() => move(i, i + 1)} disabled={i === steps.length - 1} aria-label={t("seq.down", { n: i + 1 })} data-step-down>↓</button>
            <button class="danger" onclick={() => remove(i)} aria-label={t("seq.delete", { n: i + 1 })} data-step-delete>✕</button>
          </span>
        </div>

        {#if i === sel}
          <div class="params" data-step-params>
            {#if styleOf(st) === "meter"}
              <label class="line">
                <span>{t("seq.metric")}</span>
                <select value={String(st.params.metric ?? "cpu.usage")} onchange={(e) => pickMetric(st, e.currentTarget.value)} data-metric aria-label={t("seq.metric")}>
                  {#each METRICS as m (m)}<option value={m}>{metricLabel(m)}</option>{/each}
                </select>
                <output class="liveval" data-live>{describeLive(String(st.params.metric), sysmon.values[String(st.params.metric)])}</output>
              </label>
              <div class="line">
                <span>{t("seq.colour")}</span>
                <ColorPicker value={colorOf(st)} onchange={(c) => set(st, "color", c as unknown as ParamValue)} label={t("seq.colour")} />
                <label class="check"><input type="checkbox" checked={!!st.params.gradient} onchange={(e) => set(st, "gradient", e.currentTarget.checked)} /> {t("seq.gradient")}</label>
              </div>
              <label class="line">
                <span>{t("seq.min")}</span><input type="number" value={num(st, "min", 0)} oninput={(e) => set(st, "min", Number(e.currentTarget.value))} data-min aria-label={t("seq.min")} />
                <small class="muted">{unitOf(String(st.params.metric))} · {t("seq.minHint")}</small>
              </label>
              <label class="line">
                <span>{t("seq.max")}</span><input type="number" value={num(st, "max", 100)} oninput={(e) => set(st, "max", Number(e.currentTarget.value))} data-max aria-label={t("seq.max")} />
                <small class="muted">{isTemp(String(st.params.metric)) ? t("seq.maxHintTemp") : t("seq.maxHint")}</small>
              </label>
              <label class="line">
                <span>{t("seq.fill")}</span>
                <select value={String(st.params.style ?? "sweep")} onchange={(e) => set(st, "style", e.currentTarget.value)} aria-label={t("seq.fill")}>
                  <option value="sweep">{t("seq.fill.sweep")}</option><option value="live">{t("seq.fill.live")}</option>
                </select>
              </label>
              <label class="line">
                <span>{t("seq.background")}</span>
                <select value={String(st.params.background ?? "none")} onchange={(e) => set(st, "background", e.currentTarget.value)} aria-label={t("seq.background")}>
                  <option value="none">{t("seq.bg.none")}</option><option value="dim">{t("seq.bg.dim")}</option><option value="off">{t("seq.bg.off")}</option>
                </select>
              </label>
              <label class="line">
                <span>{t("seq.show")}</span>
                <input type="range" min="1" max="30" step="0.5" value={st.duration_ms / 1000} oninput={(e) => (st.duration_ms = Math.round(Number(e.currentTarget.value) * 1000))} data-duration aria-label={t("seq.show")} />
                <output>{secs(st.duration_ms)}</output>
              </label>
            {:else if styleOf(st) === "blink"}
              <div class="line"><span>{t("seq.colour")}</span><ColorPicker value={colorOf(st)} onchange={(c) => set(st, "color", c as unknown as ParamValue)} label={t("seq.colour")} /></div>
              <label class="line"><span>{t("seq.times")}</span><input type="number" min="1" max="50" value={num(st, "count", 5)} oninput={(e) => set(st, "count", Math.max(1, Number(e.currentTarget.value) || 1))} data-count aria-label={t("seq.times")} /></label>
              <label class="line"><span>{t("seq.onMs")}</span><input type="range" min="40" max="1000" step="10" value={num(st, "on_ms", 150)} oninput={(e) => set(st, "on_ms", Number(e.currentTarget.value))} aria-label={t("seq.onMs")} /><output>{num(st, "on_ms", 150)} ms</output></label>
              <label class="line"><span>{t("seq.offMs")}</span><input type="range" min="40" max="1000" step="10" value={num(st, "off_ms", 150)} oninput={(e) => set(st, "off_ms", Number(e.currentTarget.value))} aria-label={t("seq.offMs")} /><output>{num(st, "off_ms", 150)} ms</output></label>
            {:else if styleOf(st) === "orbit"}
              <div class="line"><span>{t("seq.colour")}</span><ColorPicker value={colorOf(st)} onchange={(c) => set(st, "color", c as unknown as ParamValue)} label={t("seq.colour")} /></div>
              <label class="line"><span>{t("seq.laps")}</span><input type="number" min="1" max="20" value={num(st, "laps", 1)} oninput={(e) => set(st, "laps", Math.max(1, Number(e.currentTarget.value) || 1))} data-laps aria-label={t("seq.laps")} /></label>
              <label class="line"><span>{t("seq.lap")}</span><input type="range" min="200" max="3000" step="50" value={num(st, "lap_ms", 800)} oninput={(e) => set(st, "lap_ms", Number(e.currentTarget.value))} aria-label={t("seq.lap")} /><output>{num(st, "lap_ms", 800)} ms</output></label>
              <label class="line"><span>{t("seq.trail")}</span><input type="range" min="0" max="1" step="0.05" value={num(st, "trail", 0.5)} oninput={(e) => set(st, "trail", Number(e.currentTarget.value))} aria-label={t("seq.trail")} /><output>{Math.round(num(st, "trail", 0.5) * 100)} %</output></label>
            {:else if styleOf(st) === "animation"}
              <label class="line">
                <span>{t("seq.animation")}</span>
                <select value={st.anim?.id ?? ""} onchange={(e) => chooseAnim(st, e.currentTarget.value)} aria-label={t("seq.animation")} data-step-anim>
                  <option value="">{t("notif.pickAnim")}</option>
                  {#each animOptions as a (a.id)}<option value={a.id}>{animName(a)}</option>{/each}
                </select>
              </label>
              <label class="line"><span>{t("seq.show")}</span><input type="range" min="0.5" max="30" step="0.5" value={st.duration_ms / 1000} oninput={(e) => (st.duration_ms = Math.round(Number(e.currentTarget.value) * 1000))} aria-label={t("seq.show")} /><output>{secs(st.duration_ms)}</output></label>
            {:else if styleOf(st) === "solid"}
              <div class="line"><span>{t("seq.colour")}</span><ColorPicker value={colorOf(st)} onchange={(c) => set(st, "color", c as unknown as ParamValue)} label={t("seq.colour")} /></div>
              <label class="line"><span>{t("seq.show")}</span><input type="range" min="0.2" max="30" step="0.1" value={st.duration_ms / 1000} oninput={(e) => (st.duration_ms = Math.round(Number(e.currentTarget.value) * 1000))} aria-label={t("seq.show")} /><output>{secs(st.duration_ms)}</output></label>
            {:else}
              <label class="line"><span>{t("seq.wait")}</span><input type="range" min="0.1" max="30" step="0.1" value={st.duration_ms / 1000} oninput={(e) => (st.duration_ms = Math.round(Number(e.currentTarget.value) * 1000))} aria-label={t("seq.wait")} /><output>{secs(st.duration_ms)}</output></label>
              <p class="muted small">{t("seq.holdHint")}</p>
            {/if}
            {#if PATHY.includes(st.effect)}
              <p class="muted small">{t(st.effect === "meter" ? "seq.keysMeter" : st.effect === "orbit" ? "seq.keysOrbit" : "seq.keysBlink")}</p>
            {:else if st.effect !== "hold"}
              <p class="muted small">{t("seq.keysOptional")}</p>
            {/if}
          </div>
        {/if}
      </li>
    {/each}
  </ol>

  <div class="adds" role="group" aria-label={t("seq.add")}>
    <span class="muted small">{t("seq.add")}</span>
    {#each STYLES as s (s)}
      <button onclick={() => add(s)} data-add-step={s}>+ {t(`seq.style.${s}`)}</button>
    {/each}
  </div>

  {#if step && kb && layout}
    {#if step.effect !== "hold"}
      <div class="keys" data-seq-keys>
        <div class="keyhead">
          <h4>{t("seq.keysTitle", { n: Math.min(sel, steps.length - 1) + 1 })}</h4>
          <span class="muted small" data-key-count>{step.keys.length ? t("seq.keysCount", { n: step.keys.length }) : t("seq.keysNone")}</span>
        </div>
        <div class="quick">
          {#each KEYSETS as k (k.id)}
            {#if keysetAvailable(k.keys)}<button onclick={() => useKeys(k.keys)} data-keyset={k.id}>{t(`seq.set.${k.id}`)}</button>{/if}
          {/each}
          <button onclick={undoKey} disabled={!step.keys.length} data-key-undo>{t("seq.undoKey")}</button>
          <button onclick={unpicked} disabled={!step.keys.length} data-key-clear>{t("seq.clearKeys")}</button>
        </div>
        <LedCanvas {layout} colors={blank} display={frame} tools={["path"]} path={pathNow} onpathadd={addKey} onchange={() => {}} history={false} />
        <p class="muted small">{t("seq.pickHint", { name: kb.name })}</p>
      </div>
    {/if}
    <div class="play">
      <button onclick={() => (playing = !playing)} data-seq-play aria-pressed={playing}>{playing ? t("seq.pause") : t("seq.play")}</button>
      {#if !playing}
        <input type="range" min="0" max={total} step="0.05" value={playT % total} oninput={(e) => (playT = Number(e.currentTarget.value))} aria-label={t("seq.scrub")} />
        <output class="muted small">{(playT % total).toFixed(1)} s · {t("seq.stepAt", { n: steps.findIndex((_, i) => startOf(i + 1) > playT % total) + 1 })}</output>
      {/if}
    </div>
  {:else if !kb}
    <p class="muted small">{t("seq.noKeyboard")}</p>
  {/if}
</div>

<style>
  .seq { display: flex; flex-direction: column; gap: 0.6rem; }
  .head { display: flex; align-items: center; gap: 0.8rem; flex-wrap: wrap; }
  h4 { margin: 0; }
  .small { font-size: 0.82rem; margin: 0.2rem 0; }
  .loop { margin-left: auto; }
  .steps { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.4rem; }
  .step { border: 1px solid var(--border); border-radius: 10px; background: var(--panel-2); }
  .step.sel { border-color: var(--accent); }
  .step.over { outline: 2px dashed var(--accent); }
  .bar { display: flex; align-items: center; gap: 0.4rem; padding: 0.2rem 0.4rem; }
  .grip { cursor: grab; color: var(--muted); padding: 0 0.2rem; user-select: none; }
  .pick { flex: 1; min-width: 0; display: flex; align-items: center; gap: 0.5rem; background: transparent; border: 0; text-align: left; padding: 0.35rem 0.2rem; }
  .num { display: inline-grid; place-items: center; width: 1.5rem; height: 1.5rem; border-radius: 50%; background: var(--accent); color: var(--accent-fg); font-size: 0.78rem; font-weight: 700; flex: none; }
  .dot { width: 0.85rem; height: 0.85rem; border-radius: 50%; border: 1px solid #ffffff40; flex: none; }
  .sum { color: var(--muted); font-size: 0.85rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .dur { margin-left: auto; font-variant-numeric: tabular-nums; color: var(--muted); font-size: 0.82rem; flex: none; }
  .acts { display: flex; gap: 0.2rem; flex: none; }
  .acts button { padding: 0.15rem 0.5rem; }
  .params { border-top: 1px solid var(--border); padding: 0.5rem 0.8rem 0.6rem; }
  .line { display: grid; grid-template-columns: 8rem minmax(0, 1fr) auto; gap: 0.6rem; align-items: center; margin: 0.35rem 0; }
  .line > span { color: var(--muted); font-size: 0.9rem; }
  div.line { display: flex; gap: 0.8rem; flex-wrap: wrap; }
  div.line > span { min-width: 7.2rem; }
  .line input[type="number"] { width: 6rem; }
  .line output { text-align: right; font-variant-numeric: tabular-nums; }
  .liveval { color: var(--ok); font-weight: 600; white-space: nowrap; }
  .check { display: flex; align-items: center; gap: 0.4rem; }
  .adds { display: flex; gap: 0.4rem; flex-wrap: wrap; align-items: center; }
  .keys { border-top: 1px solid var(--border); padding-top: 0.6rem; }
  .keyhead { display: flex; align-items: baseline; gap: 0.8rem; flex-wrap: wrap; }
  .quick { display: flex; gap: 0.4rem; flex-wrap: wrap; margin: 0.4rem 0; }
  .play { display: flex; gap: 0.6rem; align-items: center; flex-wrap: wrap; }
  .play input[type="range"] { flex: 1; min-width: 8rem; }
  @media (max-width: 560px) { .line { grid-template-columns: 1fr; } }
</style>
