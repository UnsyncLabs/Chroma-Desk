/** Theme vs effect: what a layer or animation is for in the Quick tab. Mirrors `Role` in compositor.rs. */
import type { Animation, Layer, Role } from "./api";

/** Plain engine effects that are transparent overlays; every other one is a base look. */
const OVERLAY_EFFECTS = ["glitch", "starlight", "strobe", "ripple", "blink", "orbit", "sequence", "meter"];

export const effectRole = (effect: string): Role => (OVERLAY_EFFECTS.includes(effect) ? "effect" : "theme");

/** Role of a library animation, inferred like `animations::infer_role` when the file has none. */
export function animRole(a: Animation): Role {
  if (a.role) return a.role;
  if (a.kind.type === "sequence") return "effect";
  if (a.tags.includes("notification")) return "effect";
  return a.kind.type === "effect" ? effectRole(a.kind.effect) : "theme";
}

/** Role of a scene layer: its own, else the one of the animation it plays, else from its effect. */
export function layerRole(l: Layer): Role {
  if (l.role) return l.role;
  if (l.effect === "animation") {
    const snap = l.params.snapshot as unknown as Animation | null;
    if (snap) return animRole(snap);
  }
  return effectRole(l.effect);
}
