<script lang="ts">
  import { api, hex, fromHex, throttle, type Device, type Rgb, type Zone } from "./api";

  let { device }: { device: Device } = $props();

  // Zona con matriz (el teclado). Si no hay, se usa una cuadrícula simple.
  const zone: Zone = $derived(device.zones.find((z) => z.matrix) ?? device.zones[0]);

  type Cell = { led: number; label: string; row: number; col: number } | null;

  const grid: Cell[][] = $derived.by(() => {
    if (zone.matrix) {
      const { width, height, map } = zone.matrix;
      return Array.from({ length: height }, (_, r) =>
        Array.from({ length: width }, (_, c) => {
          const v = map[r * width + c];
          if (v === null || v === undefined) return null;
          const led = zone.offset + v;
          return { led, label: label(device.led_names[led]), row: r, col: c };
        }),
      );
    }
    const cols = 16;
    return Array.from({ length: Math.ceil(zone.num_leds / cols) }, (_, r) =>
      Array.from({ length: cols }, (_, c) => {
        const i = r * cols + c;
        if (i >= zone.num_leds) return null;
        const led = zone.offset + i;
        return { led, label: label(device.led_names[led]) || String(i), row: r, col: c };
      }),
    );
  });

  function label(name = "") {
    return name.replace(/^Key:\s*/i, "").replace(/^Keyboard\s*/i, "");
  }

  let colors: Rgb[] = $state([]);
  let selected = $state(new Set<number>());
  let current = $state("#ff0000");
  let tool: "brush" | "select" | "picker" = $state("brush");
  let recent: string[] = $state(["#ff0000", "#00ff00", "#0000ff", "#ffffff", "#ff8800", "#aa00ff"]);
  let live = $state(true);
  let status = $state("");

  $effect(() => {
    colors = device.colors.map((c) => [...c] as Rgb);
  });

  const push = throttle((cols: Rgb[]) => {
    api.setLeds(device.id, cols).catch((e) => (status = String(e)));
  }, 33);

  let directReady = false;
  async function send() {
    if (!live) return;
    if (!directReady) {
      await api.setLeds(device.id, colors, true).catch((e) => (status = String(e)));
      directReady = true;
      return;
    }
    push(colors.map((c) => [...c] as Rgb));
  }

  function remember(h: string) {
    recent = [h, ...recent.filter((x) => x !== h)].slice(0, 12);
  }

  function paint(leds: Iterable<number>) {
    const c = fromHex(current);
    for (const l of leds) colors[l] = [...c] as Rgb;
    remember(current);
    send();
  }

  // --- interacción con ratón ---
  let dragging = false;
  let dragStart: { row: number; col: number } | null = null;
  let dragEnd: { row: number; col: number } | null = $state(null);
  let additive = false;

  function down(cell: NonNullable<Cell>, e: PointerEvent) {
    if (tool === "picker") {
      current = hex(colors[cell.led]);
      tool = "brush";
      return;
    }
    dragging = true;
    dragStart = { row: cell.row, col: cell.col };
    dragEnd = dragStart;
    additive = e.ctrlKey || e.shiftKey;
    if (tool === "brush") paint([cell.led]);
    else {
      const s = additive ? new Set(selected) : new Set<number>();
      if (additive && s.has(cell.led)) s.delete(cell.led);
      else s.add(cell.led);
      selected = s;
    }
  }

  function enter(cell: NonNullable<Cell>) {
    if (!dragging) return;
    dragEnd = { row: cell.row, col: cell.col };
    if (tool === "brush") paint([cell.led]);
    else if (dragStart) {
      const s = additive ? new Set(selected) : new Set<number>();
      for (const l of rect(dragStart, dragEnd)) s.add(l);
      selected = s;
    }
  }

  function rect(a: { row: number; col: number }, b: { row: number; col: number }) {
    const out: number[] = [];
    for (let r = Math.min(a.row, b.row); r <= Math.max(a.row, b.row); r++)
      for (let c = Math.min(a.col, b.col); c <= Math.max(a.col, b.col); c++) {
        const cell = grid[r]?.[c];
        if (cell) out.push(cell.led);
      }
    return out;
  }

  function up() {
    dragging = false;
    dragStart = null;
    dragEnd = null;
  }

  // --- grupos predefinidos ---
  const allCells = $derived(grid.flat().filter(Boolean) as NonNullable<Cell>[]);
  const byLabel = (pred: (l: string) => boolean) => allCells.filter((c) => pred(c.label)).map((c) => c.led);

  const groups: Record<string, () => number[]> = {
    Todo: () => allCells.map((c) => c.led),
    WASD: () => byLabel((l) => ["W", "A", "S", "D"].includes(l.toUpperCase())),
    Flechas: () => byLabel((l) => /^(Up|Down|Left|Right)( Arrow)?$/i.test(l)),
    "F1–F12": () => byLabel((l) => /^F([1-9]|1[0-2])$/i.test(l)),
    Letras: () => byLabel((l) => /^[A-Z]$/i.test(l)),
    Números: () => byLabel((l) => /^[0-9]$/.test(l)),
    Modificadores: () =>
      byLabel((l) => /(Shift|Ctrl|Control|Alt|Win|Super|Fn|Caps|Menu|Tab)/i.test(l)),
  };

  function selectGroup(name: string, e: MouseEvent) {
    const leds = groups[name]();
    selected = e.ctrlKey || e.shiftKey ? new Set([...selected, ...leds]) : new Set(leds);
  }

  function fillSelection() {
    paint(selected.size ? selected : allCells.map((c) => c.led));
  }

  function clearAll() {
    for (const c of allCells) colors[c.led] = [0, 0, 0];
    send();
  }

  const inDrag = (led: number) =>
    tool === "select" && dragStart && dragEnd && rect(dragStart, dragEnd).includes(led);
</script>

<svelte:window onpointerup={up} />

<div class="toolbar">
  <div class="tools">
    <button class:on={tool === "brush"} onclick={() => (tool = "brush")} title="Pintar teclas">🖌 Pincel</button>
    <button class:on={tool === "select"} onclick={() => (tool = "select")} title="Seleccionar (arrastrar = rectángulo, Ctrl = añadir)">⬚ Seleccionar</button>
    <button class:on={tool === "picker"} onclick={() => (tool = "picker")} title="Tomar color de una tecla">💧 Cuentagotas</button>
  </div>
  <label class="color">
    <input type="color" bind:value={current} />
    <span>{current}</span>
  </label>
  <button onclick={fillSelection}>🪣 Rellenar {selected.size ? `selección (${selected.size})` : "todo"}</button>
  <button onclick={() => (selected = new Set())} disabled={!selected.size}>Quitar selección</button>
  <button onclick={clearAll}>Apagar todo</button>
  <label class="live"><input type="checkbox" bind:checked={live} /> En vivo</label>
</div>

<div class="recent">
  {#each recent as r}
    <button class="swatch" style:background={r} title={r} onclick={() => (current = r)} aria-label={r}></button>
  {/each}
</div>

<div class="groups">
  <span>Grupos:</span>
  {#each Object.keys(groups) as g}
    <button onclick={(e) => selectGroup(g, e)}>{g}</button>
  {/each}
</div>

<div class="kb" style:--cols={grid[0]?.length ?? 1}>
  {#each grid as row}
    {#each row as cell}
      {#if cell}
        {@const c = colors[cell.led] ?? [0, 0, 0]}
        <button
          class="key"
          class:sel={selected.has(cell.led) || inDrag(cell.led)}
          style:--k={hex(c)}
          style:--fg={c[0] * 0.3 + c[1] * 0.59 + c[2] * 0.11 > 140 ? "#111" : "#eee"}
          title={`${cell.label || "LED"} (#${cell.led})`}
          onpointerdown={(e) => down(cell, e)}
          onpointerenter={() => enter(cell)}
        >
          {cell.label}
        </button>
      {:else}
        <div class="gap"></div>
      {/if}
    {/each}
  {/each}
</div>

{#if status}<p class="err">{status}</p>{/if}

<style>
  .toolbar, .groups, .recent { display: flex; flex-wrap: wrap; gap: 0.5rem; align-items: center; margin-bottom: 0.6rem; }
  .tools { display: flex; gap: 0.25rem; }
  .tools button.on { background: var(--accent); color: #111; }
  .color { display: flex; align-items: center; gap: 0.4rem; font-family: monospace; }
  .color input { width: 2.4rem; height: 2rem; border: none; padding: 0; background: none; }
  .live { margin-left: auto; }
  .swatch { width: 1.6rem; height: 1.6rem; padding: 0; border-radius: 4px; border: 1px solid #fff3; }
  .groups span { opacity: 0.7; }
  .kb {
    display: grid;
    grid-template-columns: repeat(var(--cols), minmax(1.6rem, 1fr));
    gap: 4px;
    user-select: none;
    padding: 0.8rem;
    background: #0c0c10;
    border-radius: 10px;
    overflow-x: auto;
  }
  .key {
    aspect-ratio: 1;
    min-width: 1.6rem;
    padding: 2px;
    font-size: 0.62rem;
    line-height: 1.05;
    border-radius: 5px;
    border: 1px solid #ffffff22;
    background: var(--k);
    color: var(--fg);
    box-shadow: 0 0 10px -2px var(--k);
    overflow: hidden;
    word-break: break-word;
    cursor: crosshair;
  }
  .key.sel { outline: 2px solid var(--accent); outline-offset: 1px; }
  .gap { aspect-ratio: 1; }
  .err { color: #ff6b6b; }
</style>
