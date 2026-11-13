/**
 * Layout providers: turn an OpenRGB device into drawable LED shapes.
 * Output coordinates are in "key units" (1 = a standard key); the canvas scales them.
 */
import type { Device, Zone } from "../api";
import { KEYS, shortLabel, type KeyRect } from "./keyboardLayouts";

export type LayoutKind = "keyboard" | "matrix" | "linear" | "single" | "multi";

export interface LedItem {
  /** Index of the LED inside the device. */
  led: number;
  x: number;
  y: number;
  w: number;
  h: number;
  shape: "rect" | "circle";
  label: string;
  /** Raw OpenRGB LED name (tooltip, group matching). */
  name: string;
  zone: number;
}

export interface LayoutTitle {
  x: number;
  y: number;
  text: string;
}

export interface Layout {
  kind: LayoutKind;
  items: LedItem[];
  titles: LayoutTitle[];
  width: number;
  height: number;
  /** Zones with 0 LEDs (e.g. ARGB headers with nothing configured). */
  emptyZones: string[];
  /** LEDs that exist but are not drawn (unnamed, outside any matrix). */
  hidden: number[];
  /** Normalised 0..1 position of every LED, used by the animation engine. */
  positions: [number, number][];
}

export interface LayoutOptions {
  /** LEDs per fan, by zone name: draws resizable zones as rings. */
  fanLeds?: Record<string, number>;
}

/** Minimum number of recognised key names for a device to be drawn as a keyboard. */
const MIN_KEYS = 10;

const ledName = (d: Device, i: number) => d.led_names[i] ?? "";

function matrixCell(zone: Zone, led: number): { row: number; col: number } | null {
  if (!zone.matrix) return null;
  const rel = led - zone.offset;
  const idx = zone.matrix.map.indexOf(rel);
  if (idx < 0) return null;
  return { row: Math.floor(idx / zone.matrix.width), col: idx % zone.matrix.width };
}

function zoneOf(d: Device, led: number): Zone | undefined {
  return d.zones.find((z) => led >= z.offset && led < z.offset + z.num_leds);
}

// ---------- keyboard ----------

function keyboardLayout(d: Device): Layout | null {
  const n = d.led_names.length;
  const iso = d.led_names.includes("Key: \\ (ISO)") || d.led_names.includes("Key: Enter (ISO)");
  const placed = new Map<number, KeyRect>();
  const taken = new Set<string>();
  for (let i = 0; i < n; i++) {
    const name = ledName(d, i);
    let r = KEYS[name];
    if (!r || taken.has(name)) continue;
    // ISO boards have a short left shift (the extra key sits beside it).
    if (iso && name === "Key: Left Shift") r = { ...r, w: 1.25 };
    taken.add(name);
    placed.set(i, r);
  }
  if (placed.size < MIN_KEYS) return null;

  const items: LedItem[] = [];
  const hidden: number[] = [];
  const side: { led: number; row: number; col: number }[] = [];
  const extras: number[] = [];

  let right = 0;
  let top = Infinity;
  for (const r of placed.values()) {
    right = Math.max(right, r.x + r.w);
    top = Math.min(top, r.y);
  }
  for (let i = 0; i < n; i++) {
    const zone = zoneOf(d, i)?.id ?? 0;
    const r = placed.get(i);
    const name = ledName(d, i);
    if (r) {
      items.push({ led: i, ...r, y: r.y - top, shape: "rect", label: shortLabel(name), name, zone });
    } else if (name !== "") {
      extras.push(i);
    } else {
      const z = zoneOf(d, i);
      const cell = z ? matrixCell(z, i) : null;
      if (cell) side.push({ led: i, ...cell });
      else hidden.push(i);
    }
  }

  const height = Math.max(...items.map((it) => it.y + it.h));
  // Extras row above the keys, wrapped to the width of the keyboard.
  const perRow = Math.max(6, Math.floor(right / 1.1));
  const extraRows = Math.ceil(extras.length / perRow);
  const dy = extras.length ? extraRows * 1.1 + 0.4 : 0;
  for (const it of items) it.y += dy;
  extras.forEach((led, k) => {
    const name = ledName(d, led);
    items.push({
      led,
      x: (k % perRow) * 1.1,
      y: Math.floor(k / perRow) * 1.1,
      w: 1,
      h: 1,
      shape: "rect",
      label: shortLabel(name),
      name,
      zone: zoneOf(d, led)?.id ?? 0,
    });
  });

  // Matrix LEDs with no known position: compact band on the right.
  let width = right;
  if (side.length) {
    const minCol = Math.min(...side.map((s) => s.col));
    const minRow = Math.min(...side.map((s) => s.row));
    for (const s of side) {
      items.push({
        led: s.led,
        x: right + 1 + (s.col - minCol) * 0.55,
        y: dy + (s.row - minRow) * 0.55,
        w: 0.5,
        h: 0.5,
        shape: "rect",
        label: "",
        name: "",
        zone: zoneOf(d, s.led)?.id ?? 0,
      });
    }
    width = Math.max(...items.map((it) => it.x + it.w));
  }
  return { kind: "keyboard", items, titles: [], width, height: dy + height, emptyZones: [], hidden, positions: [] };
}

// ---------- generic zone layouts ----------

interface Block {
  items: LedItem[];
  width: number;
  height: number;
}

function matrixBlock(d: Device, z: Zone): Block {
  const m = z.matrix!;
  const items: LedItem[] = [];
  m.map.forEach((rel, idx) => {
    if (rel === null) return;
    const led = z.offset + rel;
    const name = ledName(d, led);
    items.push({
      led,
      x: idx % m.width,
      y: Math.floor(idx / m.width),
      w: 0.92,
      h: 0.92,
      shape: "rect",
      label: shortLabel(name),
      name,
      zone: z.id,
    });
  });
  return { items, width: m.width, height: m.height };
}

/** `count` LEDs per ring, rings side by side (wrapping after 4). */
function ringsBlock(d: Device, z: Zone, perFan: number): Block {
  const dot = 0.7;
  const radius = Math.max(1, (perFan * (dot + 0.15)) / (2 * Math.PI));
  const size = 2 * radius + dot;
  const fans = Math.floor(z.num_leds / perFan);
  const perRow = 4;
  const items: LedItem[] = [];
  for (let i = 0; i < z.num_leds; i++) {
    const fan = Math.min(Math.floor(i / perFan), fans - 1);
    const k = i - fan * perFan;
    const cx = (fan % perRow) * (size + 0.6) + size / 2;
    const cy = Math.floor(fan / perRow) * (size + 0.6) + size / 2;
    const a = -Math.PI / 2 + (2 * Math.PI * k) / perFan;
    const led = z.offset + i;
    items.push({
      led,
      x: cx + radius * Math.cos(a) - dot / 2,
      y: cy + radius * Math.sin(a) - dot / 2,
      w: dot,
      h: dot,
      shape: "circle",
      label: "",
      name: ledName(d, led),
      zone: z.id,
    });
  }
  const cols = Math.min(fans, perRow);
  return { items, width: cols * (size + 0.6) - 0.6, height: Math.ceil(fans / perRow) * (size + 0.6) - 0.6 };
}

function rowBlock(d: Device, z: Zone, perRow = 24): Block {
  const items: LedItem[] = [];
  for (let i = 0; i < z.num_leds; i++) {
    const led = z.offset + i;
    items.push({
      led,
      x: i % perRow,
      y: Math.floor(i / perRow),
      w: 0.85,
      h: 0.85,
      shape: "circle",
      label: "",
      name: ledName(d, led),
      zone: z.id,
    });
  }
  return { items, width: Math.min(z.num_leds, perRow), height: Math.ceil(z.num_leds / perRow) };
}

function discBlock(d: Device, z: Zone, size = 1.6): Block {
  const led = z.offset;
  return {
    items: [{ led, x: 0, y: 0, w: size, h: size, shape: "circle", label: "", name: ledName(d, led), zone: z.id }],
    width: size,
    height: size,
  };
}

function zoneBlock(d: Device, z: Zone, opts: LayoutOptions): Block {
  if (z.matrix) return matrixBlock(d, z);
  if (z.num_leds === 1) return discBlock(d, z);
  const f = opts.fanLeds?.[z.name] ?? 0;
  if (f >= 3 && z.num_leds >= f) return ringsBlock(d, z, f);
  return rowBlock(d, z);
}

function genericLayout(d: Device, opts: LayoutOptions): Layout {
  const zones = d.zones.filter((z) => z.num_leds > 0);
  const emptyZones = d.zones.filter((z) => z.num_leds === 0).map((z) => z.name);
  const items: LedItem[] = [];
  const titles: LayoutTitle[] = [];
  let kind: LayoutKind = "multi";
  let width = 1;
  let y = 0;

  if (zones.length === 1) {
    const z = zones[0];
    const b = zoneBlock(d, z, opts);
    items.push(...b.items);
    width = b.width;
    y = b.height;
    kind = z.matrix ? "matrix" : z.num_leds === 1 ? "single" : "linear";
  } else if (zones.length > 1 && zones.every((z) => z.num_leds === 1)) {
    // Many single-LED zones (GPU, motherboard accents): a row of labelled discs.
    zones.forEach((z, k) => {
      const b = discBlock(d, z, 1.4);
      for (const it of b.items) items.push({ ...it, x: k * 2.2 });
      titles.push({ x: k * 2.2 + 0.7, y: 1.4 + 0.5, text: z.name });
    });
    width = zones.length * 2.2 - 0.8;
    y = 2.2;
  } else {
    for (const z of zones) {
      titles.push({ x: 0, y: y + 0.4, text: z.name });
      y += 0.8;
      const b = zoneBlock(d, z, opts);
      for (const it of b.items) items.push({ ...it, y: it.y + y });
      width = Math.max(width, b.width);
      y += b.height + 0.8;
    }
  }
  return { kind, items, titles, width, height: Math.max(y, 1), emptyZones, hidden: [], positions: [] };
}

/** Normalised positions for the engine: every LED, drawn or not (hidden ones sit at the centre). */
function computePositions(d: Device, layout: Layout): [number, number][] {
  const out: [number, number][] = Array.from({ length: d.colors.length || d.led_names.length }, () => [0.5, 0.5]);
  const w = Math.max(layout.width, 1e-6);
  const h = Math.max(layout.height, 1e-6);
  for (const it of layout.items) {
    if (it.led < out.length) out[it.led] = [(it.x + it.w / 2) / w, (it.y + it.h / 2) / h];
  }
  return out;
}

export function buildLayout(d: Device, opts: LayoutOptions = {}): Layout {
  const layout = keyboardLayout(d) ?? genericLayout(d, opts);
  layout.positions = computePositions(d, layout);
  return layout;
}
