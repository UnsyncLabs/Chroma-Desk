/** Conversions between what the editors hold (LED indices of one device) and portable animations. */
import type { AnimKind, Animation, Color, LoopMode, Stop } from "../api";
import { cloneColor } from "../palette.svelte";
import { nameIndex, refMaker, resolveRef } from "./ref";

export type Paint = (Color | null)[];
export interface FrameDraft {
  /** Stable key for the timeline. */
  id: number;
  leds: Paint;
  /** Per-frame duration; `null` = the animation's default. */
  ms: number | null;
}

let nextId = 1;
export const newFrameId = () => nextId++;
export const blankPaint = (n: number): Paint => Array.from({ length: n }, () => null);
export const clonePaint = (p: Paint): Paint => p.map((c) => (c ? cloneColor(c) : null));

export function framesToKind(frames: FrameDraft[], names: string[], o: { frameMs: number; loop: LoopMode; crossfade: number }): AnimKind {
  const ref = refMaker(names);
  return {
    type: "frames",
    frames: frames.map((f) => {
      const leds: Record<string, Color> = {};
      f.leds.forEach((c, i) => {
        if (c) leds[ref(i)] = cloneColor(c);
      });
      return f.ms == null ? { leds } : { leds, duration_ms: f.ms };
    }),
    frame_ms: o.frameMs,
    loop: o.loop,
    crossfade: o.crossfade,
  };
}

/** Frames of a saved animation on this device; `lost` counts references the device does not have. */
export function kindToFrames(kind: AnimKind & { type: "frames" }, names: string[]): { frames: FrameDraft[]; lost: number } {
  const idx = nameIndex(names);
  const lostRefs = new Set<string>();
  const frames = kind.frames.map((f) => {
    const leds = blankPaint(names.length);
    for (const [r, c] of Object.entries(f.leds)) {
      const i = resolveRef(r, idx, names.length);
      if (i === null) lostRefs.add(r);
      else leds[i] = cloneColor(c);
    }
    return { id: newFrameId(), leds, ms: f.duration_ms ?? null };
  });
  return { frames: frames.length ? frames : [{ id: newFrameId(), leds: blankPaint(names.length), ms: null }], lost: lostRefs.size };
}

export function pathToKind(path: number[], names: string[], o: { palette: Stop[]; speed: number; length: number; gap: number; reverse: boolean; tail: number }): AnimKind {
  const ref = refMaker(names);
  return { type: "path", path: path.map(ref), palette: o.palette, speed: o.speed, length: o.length, gap: o.gap, reverse: o.reverse, tail_fade: o.tail };
}

export function kindToPath(kind: AnimKind & { type: "path" }, names: string[]): { path: number[]; lost: number } {
  const idx = nameIndex(names);
  const out: number[] = [];
  let lost = 0;
  for (const r of kind.path) {
    const i = resolveRef(r, idx, names.length);
    if (i === null) lost++;
    else out.push(i);
  }
  return { path: out, lost };
}

export function cellsToKind(cells: Record<number, number>, names: string[], o: { palette: Stop[]; speed: number; reverse: boolean }): AnimKind {
  const ref = refMaker(names);
  const out: Record<string, number> = {};
  for (const [i, p] of Object.entries(cells)) out[ref(Number(i))] = Math.round(p * 1000) / 1000;
  return { type: "palette", cells: out, palette: o.palette, speed: o.speed, reverse: o.reverse };
}

export function kindToCells(kind: AnimKind & { type: "palette" }, names: string[]): { cells: Record<number, number>; lost: number } {
  const idx = nameIndex(names);
  const cells: Record<number, number> = {};
  let lost = 0;
  for (const [r, p] of Object.entries(kind.cells)) {
    const i = resolveRef(r, idx, names.length);
    if (i === null) lost++;
    else cells[i] = p;
  }
  return { cells, lost };
}

/** Wraps a kind into a full animation document (the backend fills ids and timestamps when saving). */
export function makeAnimation(kind: AnimKind, o: { id?: string; name?: string; tags?: string[]; updated: number; target: Animation["target"] }): Animation {
  return { schema_version: 1, id: o.id ?? "", name: o.name ?? "", tags: o.tags ?? [], created: 0, updated: o.updated, builtin: false, target: o.target, kind };
}
