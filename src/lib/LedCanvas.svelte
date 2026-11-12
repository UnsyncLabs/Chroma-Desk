<script lang="ts">
  /**
   * Generic LED canvas: draws any device from a `Layout` and offers the shared tools
   * (brush, selection, eyedropper, fill, eraser), quick groups and undo/redo.
   *
   * It edits a `colors` model (`null` = transparent, only for layers) and reports every
   * change through `onchange`; the parent decides where that goes (hardware or a Paint layer).
   */
  import { hex, fromHex, type Rgb } from "./api";
  import type { Layout, LedItem } from "./layout/buildLayout";
  import { KEY_GROUPS } from "./layout/keyGroups";
  import { t } from "./i18n/index.svelte";

  export type Paint = (Rgb | null)[];

  let {
    layout,
    colors,
    display = null,
    editable = true,
    transparent = false,
    selected = $bindable(new Set<number>()),
    resetKey = "",
    onchange,
  }: {
    layout: Layout;
    /** The editable model. */
    colors: Paint;
    /** What to draw (e.g. the live animation frame); defaults to `colors`. */
    display?: Rgb[] | null;
    editable?: boolean;
    /** Allow erasing to transparent (layers). */
    transparent?: boolean;
    selected?: Set<number>;
    /** Changing it clears the undo history (e.g. another layer was selected). */
    resetKey?: string;
    onchange: (next: Paint) => void;
  } = $props();

  type Tool = "brush" | "select" | "picker" | "eraser";
  let tool = $state<Tool>("brush");
  let current = $state("#ff0000");
  let recent: string[] = $state(["#ff0000", "#00ff00", "#0000ff", "#ffffff", "#ff8800", "#aa00ff"]);

  const MAX_HISTORY = 100;
  let undo: Paint[] = $state([]);
  let redo: Paint[] = $state([]);
  let lastReset = "";
  $effect(() => {
    if (resetKey !== lastReset) {
      lastReset = resetKey;
      undo = [];
      redo = [];
    }
  });

  const clone = (p: Paint): Paint => p.map((c) => (c ? ([...c] as Rgb) : null));
  const itemByLed = $derived(new Map(layout.items.map((i) => [i.led, i])));
  const pad = 0.5;

  function pushHistory() {
    undo = [...undo.slice(-(MAX_HISTORY - 1)), clone(colors)];
    redo = [];
  }
  function doUndo() {
    const prev = undo.at(-1);
    if (!prev) return;
    redo = [...redo, clone(colors)];
    undo = undo.slice(0, -1);
    onchange(prev);
  }
  function doRedo() {
    const next = redo.at(-1);
    if (!next) return;
    undo = [...undo, clone(colors)];
    redo = redo.slice(0, -1);
    onchange(next);
  }

  /** Applies `value` to `leds`; `snapshot` records an undo step first. */
  function apply(leds: Iterable<number>, value: Rgb | null, snapshot: boolean) {
    if (snapshot) pushHistory();
    const next = clone(colors);
    for (const l of leds) if (l < next.length) next[l] = value ? ([...value] as Rgb) : null;
    onchange(next);
  }

  function remember(h: string) {
    recent = [h, ...recent.filter((x) => x !== h)].slice(0, 12);
  }

  // ---------- pointer interaction ----------
  let dragging = false;
  let additive = false;
  let startItem: LedItem | null = null;
  let endItem: LedItem | null = $state(null);
  let lastLed = -1;
  let base = new Set<number>();

  function itemAt(e: PointerEvent): LedItem | null {
    const el = document.elementFromPoint(e.clientX, e.clientY)?.closest("[data-led]");
    const led = el ? Number(el.getAttribute("data-led")) : NaN;
    return Number.isNaN(led) ? null : (itemByLed.get(led) ?? null);
  }

  function rectSelect(a: LedItem, b: LedItem): number[] {
    const x0 = Math.min(a.x + a.w / 2, b.x + b.w / 2);
    const x1 = Math.max(a.x + a.w / 2, b.x + b.w / 2);
    const y0 = Math.min(a.y + a.h / 2, b.y + b.h / 2);
    const y1 = Math.max(a.y + a.h / 2, b.y + b.h / 2);
    return layout.items
      .filter((i) => i.x + i.w / 2 >= x0 - 1e-6 && i.x + i.w / 2 <= x1 + 1e-6 && i.y + i.h / 2 >= y0 - 1e-6 && i.y + i.h / 2 <= y1 + 1e-6)
      .map((i) => i.led);
  }

  function onDown(e: PointerEvent) {
    const it = itemAt(e);
    if (!it) {
      if (tool === "select" && !(e.ctrlKey || e.shiftKey)) selected = new Set();
      return;
    }
    if (tool === "picker") {
      const c = colors[it.led];
      if (c) current = hex(c);
      tool = "brush";
      return;
    }
    if (!editable && tool !== "select") return;
    dragging = true;
    startItem = endItem = it;
    lastLed = it.led;
    additive = e.ctrlKey || e.shiftKey;
    base = new Set(additive ? selected : []);
    if (tool === "brush") {
      apply([it.led], fromHex(current), true);
      remember(current);
    } else if (tool === "eraser") {
      apply([it.led], null, true);
    } else {
      const s = new Set(base);
      if (additive && s.has(it.led)) s.delete(it.led);
      else s.add(it.led);
      selected = s;
    }
  }

  function onMove(e: PointerEvent) {
    if (!dragging) return;
    const it = itemAt(e);
    if (!it || it.led === lastLed) return;
    lastLed = it.led;
    endItem = it;
    if (tool === "brush") apply([it.led], fromHex(current), false);
    else if (tool === "eraser") apply([it.led], null, false);
    else if (startItem) selected = new Set([...base, ...rectSelect(startItem, it)]);
  }

  function onUp() {
    dragging = false;
    startItem = endItem = null;
    lastLed = -1;
  }

  // ---------- groups and bulk actions ----------
  const allLeds = $derived(colors.map((_, i) => i));
  const groups = $derived.by(() => {
    const out: { id: string; label: string; leds: number[] }[] = [
      { id: "all", label: t("group.all"), leds: layout.items.map((i) => i.led) },
    ];
    if (layout.kind === "keyboard") {
      for (const g of KEY_GROUPS) {
        const leds = layout.items.filter((i) => g.match(i.name)).map((i) => i.led);
        if (leds.length) out.push({ id: g.id, label: t(g.label), leds });
      }
    }
    // Zone groups (only when the device has several drawn zones).
    const zones = new Set(layout.items.map((i) => i.zone));
    if (zones.size > 1 && layout.kind !== "keyboard") {
      for (const z of zones) {
        out.push({ id: `z${z}`, label: t("group.zone", { n: z + 1 }), leds: layout.items.filter((i) => i.zone === z).map((i) => i.led) });
      }
    }
    return out;
  });

  function selectGroup(leds: number[], e: MouseEvent) {
    selected = e.ctrlKey || e.shiftKey ? new Set([...selected, ...leds]) : new Set(leds);
  }

  function fill() {
    apply(selected.size ? selected : allLeds, fromHex(current), true);
    remember(current);
  }
  function clearAll() {
    apply(allLeds, transparent ? null : [0, 0, 0], true);
  }

  // ---------- shortcuts ----------
  function onKey(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement | null)?.tagName;
    if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA") return;
    const mod = e.ctrlKey || e.metaKey;
    const k = e.key.toLowerCase();
    if (mod && k === "z") {
      e.preventDefault();
      if (e.shiftKey) doRedo();
      else doUndo();
    } else if (mod && k === "y") {
      e.preventDefault();
      doRedo();
    } else if (mod && k === "a") {
      e.preventDefault();
      selected = new Set(layout.items.map((i) => i.led));
    } else if (!mod && k === "escape") selected = new Set();
    else if (!mod && k === "b") tool = "brush";
    else if (!mod && k === "s") tool = "select";
    else if (!mod && k === "i") tool = "picker";
    else if (!mod && k === "e" && transparent) tool = "eraser";
    else if (!mod && k === "f" && editable) fill();
  }

  const px = (i: number, c: Rgb | null | undefined) => (c ? hex(c) : "#1b1b24");
  const dark = (c: Rgb | null | undefined) => !c || c[0] * 0.3 + c[1] * 0.59 + c[2] * 0.11 < 140;
  const shown = (led: number) => (display ?? colors)[led] ?? null;
  const inDragRect = $derived(
    tool === "select" && startItem && endItem ? new Set(rectSelect(startItem, endItem)) : new Set<number>(),
  );
</script>

<svelte:window onpointerup={onUp} onkeydown={onKey} />

<div class="toolbar" role="toolbar" aria-label={t("canvas.tools")}>
  <div class="tools">
    <button class:on={tool === "brush"} onclick={() => (tool = "brush")} title={t("tool.brush") + " (B)"} aria-label={t("tool.brush")} aria-pressed={tool === "brush"}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 4l-9 9m-2 1c-2 0-3 1.5-3 3.5 0 1-1 2-2 2.5 3 .5 7 .5 8-2.5 0-1.5-.5-3-3-3z" /></svg>
      <span>{t("tool.brush")}</span>
    </button>
    <button class:on={tool === "select"} onclick={() => (tool = "select")} title={t("tool.select.hint") + " (S)"} aria-label={t("tool.select")} aria-pressed={tool === "select"}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="4" width="16" height="16" stroke-dasharray="3 3" /></svg>
      <span>{t("tool.select")}</span>
    </button>
    <button class:on={tool === "picker"} onclick={() => (tool = "picker")} title={t("tool.picker.hint") + " (I)"} aria-label={t("tool.picker")} aria-pressed={tool === "picker"}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3c3 4 6 7 6 10a6 6 0 0 1-12 0c0-3 3-6 6-10z" /></svg>
      <span>{t("tool.picker")}</span>
    </button>
    {#if transparent}
      <button class:on={tool === "eraser"} onclick={() => (tool = "eraser")} title={t("tool.eraser.hint") + " (E)"} aria-label={t("tool.eraser")} aria-pressed={tool === "eraser"}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 15l8-9 6 6-6 7H8zM9 20h11" /></svg>
        <span>{t("tool.eraser")}</span>
      </button>
    {/if}
  </div>
  <label class="color">
    <input type="color" bind:value={current} aria-label={t("canvas.color")} />
    <span>{current}</span>
  </label>
  <button onclick={fill} disabled={!editable} title="F">{selected.size ? t("canvas.fillSel", { n: selected.size }) : t("canvas.fillAll")}</button>
  <button onclick={() => (selected = new Set())} disabled={!selected.size} title="Esc">{t("canvas.deselect")}</button>
  <button onclick={clearAll} disabled={!editable}>{transparent ? t("canvas.clearLayer") : t("canvas.allOff")}</button>
  <span class="hist">
    <button onclick={doUndo} disabled={!undo.length} aria-label={t("canvas.undo")} title="Ctrl+Z">↶</button>
    <button onclick={doRedo} disabled={!redo.length} aria-label={t("canvas.redo")} title="Ctrl+Shift+Z">↷</button>
  </span>
</div>

<div class="recent">
  {#each recent as r (r)}
    <button class="swatch" style:background={r} title={r} onclick={() => (current = r)} aria-label={r}></button>
  {/each}
</div>

<div class="groups">
  <span>{t("canvas.groups")}</span>
  {#each groups as g (g.id)}
    <button onclick={(e) => selectGroup(g.leds, e)}>{g.label}</button>
  {/each}
</div>

<!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
<svg
  class="canvas"
  role="application"
  aria-label={t("canvas.label")}
  viewBox={`${-pad} ${-pad} ${layout.width + pad * 2} ${layout.height + pad * 2}`}
  style:max-height={`${Math.max(180, (layout.height + 1) * 70)}px`}
  onpointerdown={onDown}
  onpointermove={onMove}
>
  <defs>
    <filter id="glow" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="0.16" /></filter>
  </defs>
  <g filter="url(#glow)" opacity="0.55" pointer-events="none">
    {#each layout.items as it (it.led)}
      {@const c = shown(it.led)}
      {#if c && (c[0] + c[1] + c[2]) > 30}
        {#if it.shape === "circle"}
          <circle cx={it.x + it.w / 2} cy={it.y + it.h / 2} r={it.w / 2 + 0.06} fill={px(it.led, c)} />
        {:else}
          <rect x={it.x - 0.04} y={it.y - 0.04} width={it.w + 0.08} height={it.h + 0.08} rx="0.14" fill={px(it.led, c)} />
        {/if}
      {/if}
    {/each}
  </g>
  {#each layout.titles as ti (ti.text + ti.y + ti.x)}
    <text x={ti.x} y={ti.y} class="title" font-size="0.45" text-anchor={layout.kind === "multi" && ti.x > 0 ? "middle" : "start"}>{ti.text}</text>
  {/each}
  {#each layout.items as it (it.led)}
    {@const c = shown(it.led)}
    {@const sel = selected.has(it.led) || inDragRect.has(it.led)}
    <g data-led={it.led} class="led" class:sel>
      <title>{it.name || t("canvas.ledN", { n: it.led })} (#{it.led})</title>
      {#if it.shape === "circle"}
        <circle cx={it.x + it.w / 2} cy={it.y + it.h / 2} r={it.w / 2} fill={px(it.led, c)} class="shape" class:empty={!c} />
      {:else}
        <rect x={it.x} y={it.y} width={it.w} height={it.h} rx="0.14" fill={px(it.led, c)} class="shape" class:empty={!c} />
      {/if}
      {#if it.label && it.w >= 0.9}
        <text x={it.x + it.w / 2} y={it.y + it.h / 2} class="cap" class:dark={dark(c)} font-size={it.label.length > 4 ? 0.26 : 0.34}>{it.label}</text>
      {/if}
    </g>
  {/each}
</svg>

{#if layout.emptyZones.length}
  <p class="hint">{t("canvas.emptyZones", { zones: layout.emptyZones.join(", ") })}</p>
{/if}
{#if layout.hidden.length}
  <p class="hint">{t("canvas.hidden", { n: layout.hidden.length })}</p>
{/if}

<style>
  .toolbar, .groups, .recent { display: flex; flex-wrap: wrap; gap: 0.5rem; align-items: center; margin-bottom: 0.6rem; }
  .tools { display: flex; gap: 0.25rem; flex-wrap: wrap; }
  .tools button { display: inline-flex; align-items: center; gap: 0.35rem; }
  .tools button.on { background: var(--accent); color: var(--accent-fg); }
  .tools svg { width: 1.05rem; height: 1.05rem; fill: none; stroke: currentColor; stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; }
  .color { display: flex; align-items: center; gap: 0.4rem; font-family: monospace; }
  .color input { width: 2.4rem; height: 2rem; border: none; padding: 0; background: none; }
  .hist { margin-left: auto; display: flex; gap: 0.25rem; }
  .swatch { width: 1.6rem; height: 1.6rem; padding: 0; border-radius: 4px; }
  .groups span { opacity: 0.7; }
  .canvas { width: 100%; height: auto; display: block; background: var(--canvas-bg); border-radius: 10px; padding: 0.4rem; box-sizing: border-box; user-select: none; touch-action: none; cursor: crosshair; }
  .shape { stroke: #ffffff26; stroke-width: 0.03; }
  .shape.empty { stroke-dasharray: 0.12 0.08; stroke: #ffffff44; }
  .led.sel .shape { stroke: var(--accent); stroke-width: 0.08; }
  .cap { text-anchor: middle; dominant-baseline: central; fill: #f2f2f7; pointer-events: none; font-family: inherit; }
  .cap.dark { fill: #e6e6ee; }
  .cap:not(.dark) { fill: #101015; }
  .title { fill: var(--muted); }
  .hint { font-size: 0.8rem; color: var(--muted); }
</style>
