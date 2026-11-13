/** The active palette (theme colours by slot) and colour helpers shared by the UI and the renderers. */
import { api, on, type Color, type Rgb, type SlotRef } from "./api";

export const SLOTS = ["background", "foreground", "accent", "primary", "secondary", "tertiary", "alert", "warn", "ok", "info", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8"] as const;
export type SlotName = (typeof SLOTS)[number];
/** Slots shown first in pickers (the rest are the c1..c8 ramp). */
export const MAIN_SLOTS: SlotName[] = ["accent", "primary", "secondary", "tertiary", "alert", "warn", "ok", "info", "foreground", "background"];

const h = (s: string): Rgb => [parseInt(s.slice(1, 3), 16), parseInt(s.slice(3, 5), 16), parseInt(s.slice(5, 7), 16)];
/** Same colours as `Palette::default_manual` in Rust. */
export const DEFAULT_SLOTS: Record<string, Rgb> = {
  background: h("#0b0b12"), foreground: h("#e6e6f0"), accent: h("#7c5cff"), primary: h("#3ea6ff"), secondary: h("#ff5caa"), tertiary: h("#2fe0c0"),
  alert: h("#ff3b3b"), warn: h("#ffb020"), ok: h("#2ee66b"), info: h("#3ec9ff"),
  c1: h("#ff3b3b"), c2: h("#ff8a2b"), c3: h("#ffd23f"), c4: h("#2ee66b"), c5: h("#2fe0c0"), c6: h("#3ea6ff"), c7: h("#a66cff"), c8: h("#ff5caa"),
};

export interface PaletteEvent {
  palette: { name: string; source: string; slots: Record<string, Rgb>; generation: number };
  active: string;
  external: boolean;
  error: string | null;
}

export const palette = $state({
  name: "Default",
  source: "manual",
  slots: { ...DEFAULT_SLOTS } as Record<string, Rgb>,
  generation: 0,
  external: false,
  error: null as string | null,
});

export const isSlot = (c: unknown): c is SlotRef => !!c && typeof c === "object" && !Array.isArray(c) && typeof (c as SlotRef).slot === "string";

const lighten = (c: Rgb, k: number): Rgb => {
  const target = k >= 0 ? 255 : 0;
  const a = Math.min(1, Math.abs(k));
  return c.map((v) => Math.round(v + (target - v) * a)) as Rgb;
};

/** A colour as RGB: fixed colours as they are, slots from `slots` (the active palette by default). */
export function resolveColor(c: Color, slots: Record<string, Rgb> = palette.slots): Rgb {
  if (!isSlot(c)) return c;
  const base = slots[c.slot] ?? slots.foreground ?? [255, 255, 255];
  return c.lighten ? lighten(base, c.lighten) : base;
}

/** Same colour value (slots compare by name and lightening). */
export const sameColor = (a: Color | null | undefined, b: Color | null | undefined) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);
export const cloneColor = (c: Color): Color => (isSlot(c) ? { ...c } : ([...c] as Rgb));

export function applyPalette(e: PaletteEvent) {
  palette.name = e.palette.name;
  palette.source = e.active;
  palette.slots = { ...DEFAULT_SLOTS, ...e.palette.slots };
  palette.generation = e.palette.generation;
  palette.external = e.external;
  palette.error = e.error;
}

export async function loadPalette() {
  try {
    applyPalette(await api.getPalette());
  } catch {
    /* keeps the default palette */
  }
}

/** Follows `palette-changed`; `onError` is told about a newly reported source error. */
export async function listenPalette(onError?: (msg: string) => void): Promise<() => void> {
  return on<PaletteEvent>("palette-changed", (e) => {
    const prev = palette.error;
    applyPalette(e);
    if (e.error && e.error !== prev) onError?.(e.error);
  });
}
