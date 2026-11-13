/**
 * Browser-side renderer of animations: thumbnails, the editor's preview and the mock backend.
 * `frames`, `path` and `palette` follow `effects.rs` exactly; the effect presets are lightweight
 * approximations (good for a thumbnail, not bit-exact with the engine).
 */
import type { Animation, ParamValue, Rgb, Stop } from "../api";
import { nameIndex, resolveRef } from "./ref";
import { sampleStops, sortStops } from "./palette";

/** Colour channels 0..255 plus coverage (0 = transparent). */
export type Px = [number, number, number, number];
export type Pos = [number, number];

type Resolved =
  | { type: "effect"; effect: string; params: Record<string, ParamValue> }
  | { type: "frames"; frames: { leds: [number, Rgb][]; ms: number }[]; loop: string; crossfade: number }
  | { type: "path"; path: number[]; palette: Stop[]; speed: number; length: number; gap: number; reverse: boolean; tail: number }
  | { type: "palette"; cells: [number, number][]; palette: Stop[]; speed: number; reverse: boolean };

/** Resolves LED references against a device's LED names (unknown ones are dropped). */
export function resolveAnim(a: Animation, names: string[]): Resolved {
  const idx = nameIndex(names);
  const n = names.length;
  const k = a.kind;
  const ref = (r: string) => resolveRef(r, idx, n);
  switch (k.type) {
    case "effect":
      return { type: "effect", effect: k.effect, params: k.params };
    case "frames":
      return {
        type: "frames",
        frames: k.frames.map((f) => ({
          leds: Object.entries(f.leds).flatMap(([r, c]) => {
            const i = ref(r);
            return i === null ? [] : [[i, c] as [number, Rgb]];
          }),
          ms: f.duration_ms ?? k.frame_ms,
        })),
        loop: k.loop,
        crossfade: k.crossfade,
      };
    case "path":
      return { type: "path", path: k.path.map(ref).filter((i): i is number => i !== null), palette: sortStops(k.palette), speed: k.speed, length: k.length, gap: k.gap, reverse: k.reverse, tail: k.tail_fade };
    case "palette":
      return {
        type: "palette",
        cells: Object.entries(k.cells).flatMap(([r, p]) => {
          const i = ref(r);
          return i === null ? [] : [[i, p] as [number, number]];
        }),
        palette: sortStops(k.palette),
        speed: k.speed,
        reverse: k.reverse,
      };
  }
}

const clear = (n: number): Px[] => Array.from({ length: n }, () => [0, 0, 0, 0] as Px);
const clamp = (v: number, a: number, b: number) => Math.min(b, Math.max(a, v));
const mod = (a: number, b: number) => ((a % b) + b) % b;
const px = (c: Rgb, a = 1): Px => [c[0], c[1], c[2], a];

function drawFrame(leds: [number, Rgb][], out: Px[]) {
  for (const [i, c] of leds) if (i < out.length) out[i] = px(c);
}

function frames(r: Extract<Resolved, { type: "frames" }>, t: number, n: number): Px[] {
  const out = clear(n);
  const list = r.frames;
  const count = list.length;
  if (!count) return out;
  const durs = list.map((f) => clamp(f.ms, 10, 600000));
  const seq = r.loop === "pingpong" && count > 1 ? [...Array(count).keys(), ...[...Array(count - 2).keys()].map((i) => count - 2 - i)] : [...Array(count).keys()];
  const total = seq.reduce((s, i) => s + durs[i], 0);
  const tm = Math.max(0, t * 1000);
  const once = r.loop === "once";
  let k: number;
  let local: number;
  if (once && tm >= total) {
    k = seq.length - 1;
    local = durs[seq[k]];
  } else {
    let rest = mod(tm, Math.max(total, 1));
    k = 0;
    while (k + 1 < seq.length && rest >= durs[seq[k]]) {
      rest -= durs[seq[k]];
      k++;
    }
    local = rest;
  }
  const cur = seq[k];
  drawFrame(list[cur].leds, out);
  const cf = clamp(r.crossfade, 0, 1);
  const next = k + 1 < seq.length ? seq[k + 1] : once ? null : seq[0];
  if (next === null || cf <= 0 || next === cur) return out;
  const f = clamp((local / durs[cur] - (1 - cf)) / cf, 0, 1);
  if (f <= 0) return out;
  const b = clear(n);
  drawFrame(list[next].leds, b);
  return out.map((a, i) => {
    const q = b[i];
    if (a[3] > 0 && q[3] > 0) return [a[0] + (q[0] - a[0]) * f, a[1] + (q[1] - a[1]) * f, a[2] + (q[2] - a[2]) * f, 1] as Px;
    if (a[3] > 0) return [a[0], a[1], a[2], 1 - f] as Px;
    if (q[3] > 0) return [q[0], q[1], q[2], f] as Px;
    return a;
  });
}

function path(r: Extract<Resolved, { type: "path" }>, t: number, n: number): Px[] {
  const out = clear(n);
  const m = r.path.length;
  if (!m || !r.palette.length) return out;
  const len = clamp(r.length, 0.02, 1);
  const period = Math.max(len + Math.max(r.gap, 0), 0.02);
  const dir = r.reverse ? -1 : 1;
  const tail = clamp(r.tail, 0, 1);
  r.path.forEach((led, k) => {
    if (led >= n) return;
    const s = (k + 0.5) / m;
    const q = mod(s - dir * t * r.speed, period);
    if (q < len) {
      const x = q / len;
      out[led] = px(sampleStops(r.palette, x, false), 1 - tail * x);
    }
  });
  return out;
}

function palette(r: Extract<Resolved, { type: "palette" }>, t: number, n: number): Px[] {
  const out = clear(n);
  const shift = (r.reverse ? -1 : 1) * t * r.speed;
  for (const [i, pos] of r.cells) if (i < n) out[i] = px(sampleStops(r.palette, pos + shift, true));
  return out;
}

// ---------- effect presets (approximations) ----------

export function hsv(h: number, s: number, v: number): Rgb {
  const hh = mod(h, 1) * 6;
  const i = Math.floor(hh);
  const f = hh - i;
  const p = v * (1 - s);
  const q = v * (1 - s * f);
  const t = v * (1 - s * (1 - f));
  const [r, g, b] = [[v, t, p], [q, v, p], [p, v, t], [p, q, v], [t, p, v], [v, p, q]][i % 6];
  return [r * 255, g * 255, b * 255];
}

const num = (p: Record<string, ParamValue>, k: string, d: number) => (typeof p[k] === "number" ? (p[k] as number) : d);
const list = (p: Record<string, ParamValue>, k: string): Rgb[] => (Array.isArray(p[k]) ? (p[k] as Rgb[]) : []);

function project(p: Pos, deg: number): number {
  const a = (deg * Math.PI) / 180;
  const [s, c] = [Math.sin(a), Math.cos(a)];
  const span = Math.max(Math.abs(c) + Math.abs(s), 1e-6);
  return 0.5 + ((p[0] - 0.5) * c + (p[1] - 0.5) * s) / span;
}

const stopsOf = (cols: Rgb[]): Stop[] => cols.map((color, i) => ({ pos: cols.length === 1 ? 0 : i / cols.length, color }));
const hash = (a: number, b: number) => {
  let h = Math.imul(a ^ 0x9e3779b1, 0x85ebca6b) ^ Math.imul(b + 0x7f4a7c15, 0xc2b2ae35);
  h ^= h >>> 15;
  h = Math.imul(h, 0x2c1b3c6d);
  h ^= h >>> 12;
  return (h >>> 0) / 0xffffffff;
};

export function renderEffect(effectId: string, p: Record<string, ParamValue>, t: number, pos: Pos[], n: number): Px[] {
  const at = (i: number): Pos => pos[i] ?? [i / Math.max(n, 1), 0.5];
  const all = (f: (i: number) => Px) => Array.from({ length: n }, (_, i) => f(i));
  switch (effectId) {
    case "solid":
      return all(() => px((p.color as Rgb) ?? [255, 0, 0]));
    case "rainbow": {
      const [sp, ang, sc] = [num(p, "speed", 0.25), num(p, "angle", 0), num(p, "scale", 1)];
      return all((i) => px(hsv(project(at(i), ang) * sc - t * sp, 1, 1)));
    }
    case "spectrum":
      return all(() => px(hsv(t * num(p, "speed", 0.15), num(p, "saturation", 1), 1)));
    case "color_wave": {
      const pal = stopsOf(list(p, "colors"));
      const [sp, ang, sc] = [num(p, "speed", 0.25), num(p, "angle", 0), num(p, "scale", 1)];
      return all((i) => px(sampleStops(pal, project(at(i), ang) * sc - t * sp, true)));
    }
    case "gradient": {
      const pal = list(p, "colors");
      const [sp, ang] = [num(p, "speed", 0), num(p, "angle", 0)];
      const st = stopsOf(pal.length > 1 ? pal : [...pal, ...pal]);
      return all((i) => px(sampleStops(st, project(at(i), ang) - t * sp, sp !== 0)));
    }
    case "breathing": {
      const pal = list(p, "colors");
      const period = Math.max(num(p, "period", 4), 0.05);
      const cyc = t / period;
      const k = Math.floor(cyc);
      const level = 0.5 - 0.5 * Math.cos(2 * Math.PI * (cyc - k));
      const c = pal.length ? pal[mod(k, pal.length)] : [0, 0, 0];
      return all(() => px([c[0] * level, c[1] * level, c[2] * level] as Rgb));
    }
    case "strobe": {
      const pal = list(p, "colors");
      const cyc = t * num(p, "rate", 4);
      const k = Math.floor(cyc);
      const on = cyc - k < num(p, "duty", 0.5) && pal.length > 0;
      return all(() => px(on ? pal[mod(k, pal.length)] : [0, 0, 0]));
    }
    case "chase": {
      const pal = list(p, "colors");
      const [len, gap, sp] = [num(p, "length", 0.3), num(p, "gap", 0.4), num(p, "speed", 0.6)];
      const period = Math.max(len + gap, 0.01);
      const dir = p.reverse ? -1 : 1;
      return all((i) => {
        const u = i / Math.max(n, 1) - dir * t * sp;
        const q = mod(u, period);
        if (q < len && pal.length) return px(pal[mod(Math.floor(u / period), pal.length)], 1 - q / len);
        return [0, 0, 0, 0] as Px;
      });
    }
    case "starlight": {
      const pal = list(p, "colors");
      const dur = Math.max(num(p, "duration", 1.5), 0.05);
      const cycle = dur / clamp(num(p, "density", 0.35), 0.01, 1);
      return all((i) => {
        const shifted = t + hash(i, 99) * cycle;
        const k = Math.floor(shifted / cycle);
        const local = shifted - k * cycle;
        const start = hash(i, k) * Math.max(cycle - dur, 0);
        if (local < start || local >= start + dur) return [0, 0, 0, 0] as Px;
        const lvl = Math.sin((Math.PI * (local - start)) / dur);
        const c = !p.random && pal.length ? pal[Math.floor(hash(i, k + 7) * pal.length) % pal.length] : hsv(hash(i, k + 3), 1, 1);
        return px(c, lvl);
      });
    }
    default:
      return all((i) => px(hsv(at(i)[0] - t * 0.2, 0.7, 0.8)));
  }
}

/** Renders a resolved animation at time `t` (seconds): one pixel per LED of the device. */
export function renderResolved(r: Resolved, t: number, positions: Pos[], n: number): Px[] {
  switch (r.type) {
    case "effect":
      return renderEffect(r.effect, r.params, t, positions, n);
    case "frames":
      return frames(r, t, n);
    case "path":
      return path(r, t, n);
    case "palette":
      return palette(r, t, n);
  }
}

export function renderAnimation(a: Animation, t: number, names: string[], positions: Pos[]): Px[] {
  return renderResolved(resolveAnim(a, names), t, positions, names.length);
}

/** Layers `top` over `base` (opaque black when missing) and returns plain 8-bit colours. */
export function flatten(top: Px[], base?: Rgb[] | null): Rgb[] {
  return top.map((p, i) => {
    const b = base?.[i] ?? [0, 0, 0];
    const a = p[3];
    return [Math.round(b[0] + (p[0] - b[0]) * a), Math.round(b[1] + (p[1] - b[1]) * a), Math.round(b[2] + (p[2] - b[2]) * a)] as Rgb;
  });
}

export const hasContent = (px: Px[]) => px.some((p) => p[3] > 0);

/** Seconds a full pass of the animation takes (used to pick a still frame for reduced motion). */
export function cycleSecs(a: Animation): number {
  const k = a.kind;
  if (k.type === "frames") return Math.max(0.05, k.frames.reduce((s, f) => s + (f.duration_ms ?? k.frame_ms), 0) / 1000);
  if (k.type === "path") return clamp((k.length + k.gap) / Math.max(Math.abs(k.speed), 0.01), 0.2, 60);
  if (k.type === "palette") return clamp(1 / Math.max(Math.abs(k.speed), 0.01), 0.2, 60);
  return 1;
}
