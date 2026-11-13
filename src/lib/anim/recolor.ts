/** `animations::recolor`: every colour of an animation becomes `c`, keeping each colour's brightness as shading. */
import type { Animation, Color, Rgb } from "../api";
import { resolveColor } from "../palette.svelte";

export function recolorAnim(a: Animation, c: Rgb): Animation {
  const out = JSON.parse(JSON.stringify(a)) as Animation;
  const shade = (old: Color): Color => {
    const k = Math.max(...resolveColor(old)) / 255;
    return c.map((v) => Math.round(v * k)) as Rgb;
  };
  const k = out.kind;
  if (k.type === "frames") k.frames.forEach((f) => Object.keys(f.leds).forEach((r) => (f.leds[r] = shade(f.leds[r]))));
  else if (k.type === "path" || k.type === "palette") k.palette.forEach((s) => (s.color = shade(s.color)));
  else if (k.type === "sequence") k.steps.forEach((st) => "color" in st.params && (st.params.color = c));
  else {
    const p = k.params as Record<string, unknown>;
    if ("color" in p) p.color = c;
    if (Array.isArray(p.colors)) p.colors = p.colors.map((_, i, l) => (l.length > 1 && i % 3 ? c.map((v) => Math.min(255, Math.round(v + (255 - v) * (i % 3 === 1 ? 0.45 : 0))) ) : c));
    if (!("color" in p) && !("colors" in p)) p.colors = [c];
  }
  return out;
}
