/**
 * Where the meters of the browser-side renderer get their live readings (`cpu.temp`, `gpu.vram`...).
 * The app's sensor store registers itself; the mock backend registers with a higher priority.
 */
type Source = (metric: string) => number | null;
let current: { source: Source; priority: number } | null = null;

export function setLive(source: Source, priority = 0) {
  if (!current || priority >= current.priority) current = { source, priority };
}

/** The current reading of a metric in its own unit (percent or °C), or null when unknown. */
export const live = (metric: string): number | null => current?.source(metric) ?? null;
