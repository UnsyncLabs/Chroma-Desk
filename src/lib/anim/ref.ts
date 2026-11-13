/** Portable LED references (`name:Key: W` / `idx:41`); mirrors `animations.rs`. */
import type { Animation } from "../api";

/** Returns a function that gives the portable reference of LED `i`: by name when unique, else by index. */
export function refMaker(names: string[]): (i: number) => string {
  const count = new Map<string, number>();
  for (const n of names) if (n) count.set(n, (count.get(n) ?? 0) + 1);
  return (i) => {
    const n = names[i];
    return n && count.get(n) === 1 ? `name:${n}` : `idx:${i}`;
  };
}

/** First LED index of each non-empty name. */
export function nameIndex(names: string[]): Map<string, number> {
  const m = new Map<string, number>();
  names.forEach((n, i) => {
    if (n && !m.has(n)) m.set(n, i);
  });
  return m;
}

/** Index of the LED a reference points to on a device with these names, if it exists. */
export function resolveRef(ref: string, index: Map<string, number>, n: number): number | null {
  if (ref.startsWith("name:")) return index.get(ref.slice(5)) ?? null;
  if (ref.startsWith("idx:")) {
    const i = Number(ref.slice(4));
    return Number.isInteger(i) && i >= 0 && i < n ? i : null;
  }
  return null;
}

/** Every distinct LED reference an animation uses (none for effect presets). */
export function refsOf(a: Animation): string[] {
  const k = a.kind;
  switch (k.type) {
    case "effect":
      return [];
    case "frames":
      return [...new Set(k.frames.flatMap((f) => Object.keys(f.leds)))];
    case "path":
      return [...new Set(k.path)];
    case "palette":
      return Object.keys(k.cells);
    case "sequence":
      return [...new Set(k.steps.flatMap((st) => st.keys))];
  }
}

/** `{ ok, total }` references that exist on a device (effect presets are always compatible). */
export function compat(a: Animation, names: string[]): { ok: number; total: number } {
  const index = nameIndex(names);
  const refs = refsOf(a);
  return { ok: refs.filter((r) => resolveRef(r, index, names.length) !== null).length, total: refs.length };
}

/** Compatible enough to be offered on a device: effects always, others when at least half of the LEDs exist. */
export function isCompatible(a: Animation, names: string[]): boolean {
  const { ok, total } = compat(a, names);
  return total === 0 || (ok > 0 && ok / total >= 0.5);
}
