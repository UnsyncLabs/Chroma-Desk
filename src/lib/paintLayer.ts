/** Paint-layer plumbing shared by the Layers view and the Draw > Paint editor. */
import type { Device, Layer, Rgb } from "./api";
import type { Paint } from "./LedCanvas.svelte";
import { ensureDev, newLayer } from "./scene.svelte";

/** The Paint layer as an editable array (`null` = transparent). */
export function toPaint(l: Layer, n: number): Paint {
  const out: Paint = Array.from({ length: n }, () => null);
  const leds = (l.params.leds ?? {}) as Record<string, Rgb>;
  for (const [k, c] of Object.entries(leds)) if (Number(k) < n) out[Number(k)] = c;
  return out;
}

/**
 * Paint layer that receives brush strokes. Painting must always work, so this switches the
 * device to effects mode (seeding a Paint layer with the current colours) and picks or creates a
 * Paint layer when the selected one is not paintable. Returns the layer and its id.
 */
export function paintTarget(device: Device, selectedId: string | null): Layer {
  const d = ensureDev(device.key);
  if (d.mode !== "effects") {
    d.mode = "effects";
    if (!d.layers.length) {
      const leds: Record<string, Rgb> = {};
      device.colors.forEach((c, i) => (leds[i] = [...c] as Rgb));
      d.layers.push(newLayer("paint", { params: { leds } }));
    }
  }
  let l = d.layers.find((x) => x.id === selectedId && x.effect === "paint") ?? d.layers.findLast((x) => x.effect === "paint");
  if (!l) {
    d.layers.push(newLayer("paint", { params: { leds: {} } }));
    l = d.layers.at(-1)!;
  }
  return l;
}

const same = (a: Rgb | null | undefined, b: Rgb | null | undefined) => (!a && !b) || (!!a && !!b && a[0] === b[0] && a[1] === b[1] && a[2] === b[2]);

/** Writes into `layer` only the LEDs that differ between `prev` and `next`. */
export function applyStroke(layer: Layer, prev: Paint, next: Paint) {
  const leds = { ...((layer.params.leds ?? {}) as Record<string, Rgb>) };
  next.forEach((c, i) => {
    if (same(c, prev[i])) return;
    if (c) leds[i] = [...c] as Rgb;
    else delete leds[i];
  });
  layer.params.leds = leds;
}
