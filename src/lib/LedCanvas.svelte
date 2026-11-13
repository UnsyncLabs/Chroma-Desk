<script lang="ts">
  /**
   * Generic LED canvas: draws any device from a `Layout` and offers the shared tools
   * (brush, selection, eyedropper, fill, eraser), quick groups and undo/redo.
   *
   * It edits a `colors` model (`null` = transparent, only for layers) and reports every
   * change through `onchange`; the parent decides where that goes (hardware or a Paint layer).
   */
  import { hex, type Color, type Rgb } from "./api";
  import ColorPicker from "./ColorPicker.svelte";
  import { cloneColor, resolveColor, sameColor } from "./palette.svelte";
  import type { Layout, LedItem } from "./layout/buildLayout";
  import { KEY_GROUPS } from "./layout/keyGroups";
  import { t } from "./i18n/index.svelte";

  export type Paint = (Color | null)[];
  export type Tool = "brush" | "select" | "picker" | "eraser" | "path" | "gradient";

  let {
    layout,
    colors,
    display = null,
    editable = true,
    transparent = false,
    selected = $bindable(new Set<number>()),
    resetKey = "",
    onchange,
    tools = ["brush", "select", "picker", "eraser"],
    path = null,
    onpathadd,
    ongradient,
    history = true,
  }: {
    layout: Layout;
    /** The editable model. */
    colors: Paint;
    /** What to draw (e.g. the live animation frame); defaults to `colors`. */
    display?: (Color | null)[] | null;
    editable?: boolean;
    /** Allow erasing to transparent (layers). */
    transparent?: boolean;
    selected?: Set<number>;
    /** Changing it clears the undo history (e.g. another layer was selected). */
    resetKey?: string;
    onchange: (next: Paint) => void;
    /** Tools offered (the first one is active at start). The eraser still needs `transparent`. */
    tools?: Tool[];
    /** LED indices of a drawn path, in stroke order: numbered on the canvas. */
    path?: number[] | null;
    /** Path tool: called for every new LED the stroke passes over. */
    onpathadd?: (led: number) => void;
    /** Gradient tool: called with `LED -> 0..1 position` for the dragged A -> B line. */
    ongradient?: (cells: Map<number, number>) => void;
    /** Built-in undo/redo (off when the parent keeps its own history). */
    history?: boolean;
  } = $props();

  // svelte-ignore state_referenced_locally
  let tool = $state<Tool>(tools[0] ?? "select");
  const hasBrush = $derived(tools.includes("brush"));
  const hasHistory = $derived(history && (hasBrush || tools.includes("eraser")));
  let current = $state<Color>([255, 0, 0]);
  let recent: Color[] = $state([[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255], [255, 136, 0], [170, 0, 255]]);

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

  // The eraser only exists on layers; fall back to the brush when it disappears.
  $effect(() => {
    if (!transparent && tool === "eraser") tool = tools.find((x) => x !== "eraser") ?? "select";
  });

  const clone = (p: Paint): Paint => p.map((c) => (c ? cloneColor(c) : null));
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
  function apply(leds: Iterable<number>, value: Color | null, snapshot: boolean) {
    if (snapshot) pushHistory();
    const next = clone(colors);
    for (const l of leds) if (l < next.length) next[l] = value ? cloneColor(value) : null;
    onchange(next);
  }

  function remember(h: Color) {
    recent = [cloneColor(h), ...recent.filter((x) => !sameColor(x, h))].slice(0, 12);
  }

  // ---------- pointer interaction ----------
  let dragging = false;
  let additive = false;
  let startItem: LedItem | null = null;
  let endItem: LedItem | null = $state(null);
  let lastLed = -1;
  let base = new Set<number>();
  /** Gradient drag line, in layout units. */
  let gradLine: { a: LedItem; b: LedItem } | null = $state(null);

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
      // Pick what is on screen (the live frame), not only the edited layer.
      const c = (display ?? colors)[it.led];
      if (c) current = cloneColor(c);
      tool = hasBrush ? "brush" : (tools.find((x) => x !== "picker") ?? "select");
      return;
    }
    if (!editable && tool !== "select") return;
    if (tool === "path") {
      dragging = true;
      lastLed = it.led;
      onpathadd?.(it.led);
      return;
    }
    if (tool === "gradient") {
      dragging = true;
      startItem = endItem = it;
      gradLine = { a: it, b: it };
      return;
    }
    dragging = true;
    startItem = endItem = it;
    lastLed = it.led;
    additive = e.ctrlKey || e.shiftKey;
    base = new Set(additive ? selected : []);
    if (tool === "brush") {
      apply([it.led], current, true);
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
    if (tool === "path") onpathadd?.(it.led);
    else if (tool === "gradient" && startItem) gradLine = { a: startItem, b: it };
    else if (tool === "brush") apply([it.led], current, false);
    else if (tool === "eraser") apply([it.led], null, false);
    else if (startItem) selected = new Set([...base, ...rectSelect(startItem, it)]);
  }

  function finishGradient() {
    const g = gradLine;
    gradLine = null;
    if (!g || g.a.led === g.b.led) return;
    const [ax, ay] = [g.a.x + g.a.w / 2, g.a.y + g.a.h / 2];
    const [dx, dy] = [g.b.x + g.b.w / 2 - ax, g.b.y + g.b.h / 2 - ay];
    const len2 = dx * dx + dy * dy;
    const targets = selected.size ? layout.items.filter((i) => selected.has(i.led)) : layout.items;
    const cells = new Map<number, number>();
    for (const it of targets) {
      const k = ((it.x + it.w / 2 - ax) * dx + (it.y + it.h / 2 - ay) * dy) / len2;
      cells.set(it.led, Math.min(1, Math.max(0, k)));
    }
    ongradient?.(cells);
  }

  function onUp() {
    if (dragging && tool === "gradient") finishGradient();
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
    apply(selected.size ? selected : allLeds, current, true);
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
      if (!hasHistory) return;
      e.preventDefault();
      if (e.shiftKey) doRedo();
      else doUndo();
    } else if (mod && k === "y") {
      if (!hasHistory) return;
      e.preventDefault();
      doRedo();
    } else if (mod && k === "a") {
      e.preventDefault();
      selected = new Set(layout.items.map((i) => i.led));
    } else if (!mod && k === "escape") selected = new Set();
    else if (!mod && k === "b" && tools.includes("brush")) tool = "brush";
    else if (!mod && k === "s" && tools.includes("select")) tool = "select";
    else if (!mod && k === "i" && tools.includes("picker")) tool = "picker";
    else if (!mod && k === "e" && transparent && tools.includes("eraser")) tool = "eraser";
    else if (!mod && k === "f" && editable && hasBrush) fill();
  }

  const px = (i: number, c: Rgb | null | undefined) => (c ? hex(c) : "#1b1b24");
  const dark = (c: Rgb | null | undefined) => !c || c[0] * 0.3 + c[1] * 0.59 + c[2] * 0.11 < 140;
  /** What a LED shows right now (slots resolved against the active palette). */
  const shown = (led: number): Rgb | null => {
    const c = (display ?? colors)[led];
    return c ? resolveColor(c) : null;
  };
  const inDragRect = $derived(
    tool === "select" && startItem && endItem ? new Set(rectSelect(startItem, endItem)) : new Set<number>(),
  );
</script>

<svelte:window onpointerup={onUp} onkeydown={onKey} />

<div class="toolbar" role="toolbar" aria-label={t("canvas.tools")}>
  <div class="tools">
    {#if tools.includes("brush")}
      <button class:on={tool === "brush"} onclick={() => (tool = "brush")} title={t("tool.brush") + " (B)"} aria-label={t("tool.brush")} aria-pressed={tool === "brush"}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 4l-9 9m-2 1c-2 0-3 1.5-3 3.5 0 1-1 2-2 2.5 3 .5 7 .5 8-2.5 0-1.5-.5-3-3-3z" /></svg>
        <span>{t("tool.brush")}</span>
      </button>
    {/if}
    {#if tools.includes("path")}
      <button class:on={tool === "path"} onclick={() => (tool = "path")} title={t("tool.path.hint")} aria-label={t("tool.path")} aria-pressed={tool === "path"}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 18c4 0 4-12 8-12s4 12 8 12" /><circle cx="4" cy="18" r="1.5" /><circle cx="20" cy="18" r="1.5" /></svg>
        <span>{t("tool.path")}</span>
      </button>
    {/if}
    {#if tools.includes("gradient")}
      <button class:on={tool === "gradient"} onclick={() => (tool = "gradient")} title={t("tool.gradient.hint")} aria-label={t("tool.gradient")} aria-pressed={tool === "gradient"}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 20L20 4" /><circle cx="5" cy="19" r="1.8" /><circle cx="19" cy="5" r="1.8" /></svg>
        <span>{t("tool.gradient")}</span>
      </button>
    {/if}
    {#if tools.includes("select")}
      <button class:on={tool === "select"} onclick={() => (tool = "select")} title={t("tool.select.hint") + " (S)"} aria-label={t("tool.select")} aria-pressed={tool === "select"}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="4" width="16" height="16" stroke-dasharray="3 3" /></svg>
        <span>{t("tool.select")}</span>
      </button>
    {/if}
    {#if tools.includes("picker")}
      <button class:on={tool === "picker"} onclick={() => (tool = "picker")} title={t("tool.picker.hint") + " (I)"} aria-label={t("tool.picker")} aria-pressed={tool === "picker"}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3c3 4 6 7 6 10a6 6 0 0 1-12 0c0-3 3-6 6-10z" /></svg>
        <span>{t("tool.picker")}</span>
      </button>
    {/if}
    {#if transparent && tools.includes("eraser")}
      <button class:on={tool === "eraser"} onclick={() => (tool = "eraser")} title={t("tool.eraser.hint") + " (E)"} aria-label={t("tool.eraser")} aria-pressed={tool === "eraser"}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 15l8-9 6 6-6 7H8zM9 20h11" /></svg>
        <span>{t("tool.eraser")}</span>
      </button>
    {/if}
  </div>
  {#if hasBrush}
    <span class="color"><ColorPicker value={current} onchange={(c) => (current = c)} label={t("canvas.color")} /></span>
    <button onclick={fill} disabled={!editable} title="F">{selected.size ? t("canvas.fillSel", { n: selected.size }) : t("canvas.fillAll")}</button>
  {/if}
  {#if tools.includes("select")}
    <button onclick={() => (selected = new Set())} disabled={!selected.size} title="Esc">{t("canvas.deselect")}</button>
  {/if}
  {#if hasBrush || tools.includes("eraser")}
    <button onclick={clearAll} disabled={!editable}>{transparent ? t("canvas.clearLayer") : t("canvas.allOff")}</button>
  {/if}
  {#if hasHistory}
    <span class="hist">
      <button onclick={doUndo} disabled={!undo.length} aria-label={t("canvas.undo")} title="Ctrl+Z">↶</button>
      <button onclick={doRedo} disabled={!redo.length} aria-label={t("canvas.redo")} title="Ctrl+Shift+Z">↷</button>
    </span>
  {/if}
</div>

{#if hasBrush}
  <div class="recent">
    {#each recent as r (JSON.stringify(r))}
      <button class="swatch" style:background={hex(resolveColor(r))} title={hex(resolveColor(r))} onclick={() => (current = cloneColor(r))} aria-label={hex(resolveColor(r))}></button>
    {/each}
  </div>
{/if}

{#if tools.includes("select")}
<div class="groups">
  <span>{t("canvas.groups")}</span>
  {#each groups as g (g.id)}
    <button onclick={(e) => selectGroup(g.leds, e)}>{g.label}</button>
  {/each}
</div>

{/if}

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
  {#if path && path.length}
    {@const pts = path.map((l) => itemByLed.get(l)).filter((x): x is LedItem => !!x)}
    <g class="pathLayer" pointer-events="none">
      <polyline points={pts.map((i) => `${i.x + i.w / 2},${i.y + i.h / 2}`).join(" ")} fill="none" stroke="var(--accent)" stroke-width="0.07" stroke-linejoin="round" stroke-linecap="round" opacity="0.9" />
      {#each pts as it, k (it.led)}
        <circle cx={it.x + it.w / 2} cy={it.y + it.h / 2} r="0.24" fill="var(--accent)" />
        <text x={it.x + it.w / 2} y={it.y + it.h / 2} class="num" font-size="0.3">{k + 1}</text>
      {/each}
    </g>
  {/if}
  {#if gradLine && gradLine.a.led !== gradLine.b.led}
    <g pointer-events="none">
      <line x1={gradLine.a.x + gradLine.a.w / 2} y1={gradLine.a.y + gradLine.a.h / 2} x2={gradLine.b.x + gradLine.b.w / 2} y2={gradLine.b.y + gradLine.b.h / 2} stroke="#fff" stroke-width="0.08" stroke-dasharray="0.2 0.12" />
      <circle cx={gradLine.a.x + gradLine.a.w / 2} cy={gradLine.a.y + gradLine.a.h / 2} r="0.2" fill="#fff" />
      <circle cx={gradLine.b.x + gradLine.b.w / 2} cy={gradLine.b.y + gradLine.b.h / 2} r="0.2" fill="#000" stroke="#fff" stroke-width="0.06" />
    </g>
  {/if}
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
  .num { text-anchor: middle; dominant-baseline: central; fill: var(--accent-fg); font-weight: 700; font-family: inherit; }
  .hint { font-size: 0.8rem; color: var(--muted); }
</style>
