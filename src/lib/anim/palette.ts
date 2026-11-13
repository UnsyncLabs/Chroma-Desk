/** Colour stops: sampling (mirrors `sample_stops` in effects.rs) and presets. */
import type { Color, Rgb, Stop } from "../api";
import { resolveColor } from "../palette.svelte";

const mix = (a: Rgb, b: Rgb, f: number): Rgb => [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f];

export const sortStops = (s: Stop[]): Stop[] => [...s].sort((a, b) => a.pos - b.pos);

/** Stops with every slot resolved to RGB (what the samplers work on). */
export const resolveStops = (s: Stop[]): { pos: number; color: Rgb }[] => s.map((x) => ({ pos: x.pos, color: resolveColor(x.color) }));

/** Colour at `x`; cyclic palettes wrap from the last stop back to the first. `stops` must be sorted. */
export function sampleStops(input: Stop[], x: number, cyclic: boolean): Rgb {
  const stops = resolveStops(input);
  const n = stops.length;
  if (n === 0) return [0, 0, 0];
  if (n === 1) return stops[0].color;
  const first = stops[0];
  const last = stops[n - 1];
  x = cyclic ? ((x % 1) + 1) % 1 : Math.min(1, Math.max(0, x));
  type RS = (typeof stops)[number];
  const wrap = (from: RS, to: RS, at: number) => {
    const span = Math.max(to.pos + 1 - from.pos, 1e-6);
    return mix(from.color, to.color, Math.min(1, Math.max(0, (at - from.pos) / span)));
  };
  if (x <= first.pos) {
    if (!cyclic || x === first.pos) return first.color;
    return wrap(last, first, x + 1);
  }
  if (x >= last.pos) return cyclic ? wrap(last, first, x) : last.color;
  let k = n - 2;
  for (let i = 0; i < n - 1; i++) {
    if (x >= stops[i].pos && x < stops[i + 1].pos) {
      k = i;
      break;
    }
  }
  const a = stops[k];
  const b = stops[k + 1];
  return mix(a.color, b.color, (x - a.pos) / Math.max(b.pos - a.pos, 1e-6));
}

const even = (cols: Color[], loop = false): Stop[] => {
  const c = loop ? [...cols, cols[0]] : cols;
  return c.map((color, i) => ({ pos: c.length === 1 ? 0 : i / (c.length - 1), color }));
};

/** Ready-made gradients; `key` is the i18n suffix (`pal.<key>`). */
export const PALETTE_PRESETS: { key: string; stops: Stop[] }[] = [
  { key: "rainbow", stops: even([[255, 0, 0], [255, 220, 0], [0, 255, 60], [0, 200, 255], [120, 0, 255]], true) },
  { key: "fire", stops: even([[40, 0, 0], [200, 30, 0], [255, 140, 0], [255, 220, 60], [40, 0, 0]]) },
  { key: "ocean", stops: even([[0, 40, 140], [0, 180, 220], [150, 255, 240], [0, 40, 140]]) },
  { key: "aurora", stops: even([[0, 255, 140], [0, 120, 255], [170, 60, 255], [0, 255, 140]]) },
  { key: "sunset", stops: even([[255, 80, 0], [255, 0, 120], [120, 0, 200], [255, 80, 0]]) },
  { key: "mono", stops: even([[255, 255, 255], [0, 0, 0]]) },
];
