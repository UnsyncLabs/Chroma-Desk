/** Live readings of the system monitors (CPU, GPU, RAM), polled only while something on screen shows them. */
import { api, type LiveValue, type SysmonStatus } from "./api";
import { setLive } from "./anim/live";
import { t } from "./i18n/index.svelte";

export const sysmon = $state({ values: {} as Record<string, LiveValue>, status: null as SysmonStatus | null });

const watchers = new Map<symbol, string[]>();
let timer: ReturnType<typeof setInterval> | null = null;

// The browser-side renderer (editor preview, thumbnails) reads meters from here.
setLive((m) => sysmon.values[m]?.value ?? null, 0);

async function poll() {
  const ids = [...new Set([...watchers.values()].flat())];
  if (!ids.length) return;
  try {
    const list = await api.sysmonRead(ids);
    for (const v of list) sysmon.values[v.metric] = v;
  } catch {
    /* sensors are optional */
  }
}

/** Starts polling `ids` once per second until the returned function is called. Use inside `$effect`. */
export function watchMetrics(ids: string[]): () => void {
  const key = Symbol("watch");
  watchers.set(key, ids);
  void poll();
  timer ??= setInterval(poll, 1000);
  return () => {
    watchers.delete(key);
    if (!watchers.size && timer) {
      clearInterval(timer);
      timer = null;
    }
  };
}

export const METRICS = ["cpu.usage", "cpu.temp", "gpu.usage", "gpu.vram", "gpu.temp", "ram.usage"] as const;
export const isTemp = (m: string) => m.endsWith(".temp");
export const unitOf = (m: string) => (isTemp(m) ? "°C" : "%");
/** Default `[min, max]` of a metric (a temperature scale of 30-95 °C, percentages 0-100). */
export const defaultRange = (m: string): [number, number] => (isTemp(m) ? [30, 95] : [0, 100]);

const gb = (b: number) => (b / 1073741824).toFixed(b >= 10 * 1073741824 ? 0 : 1);

/** "VRAM 2/12 GB · 17 %", "62 °C", "35 %" or "…" while there is no reading yet. */
export function describeLive(metric: string, v: LiveValue | undefined): string {
  if (!v || v.value === null) return "…";
  const val = isTemp(metric) ? `${Math.round(v.value)} °C` : `${Math.round(v.value)} %`;
  if (v.used !== null && v.total) return `${gb(v.used)}/${gb(v.total)} GB · ${val}`;
  return val;
}

export const metricLabel = (m: string) => t(`metric.${m}`);
