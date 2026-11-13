/** Fake system monitors for the mock backend: values that move over time, with overrides for tests. */
import type { LiveValue, SysmonStatus } from "../api";

const GB = 1073741824;
const overrides: Record<string, number | null> = {};

/** A smooth, deterministic wander for each metric so the meters visibly move. */
export function fakeValue(metric: string, now = Date.now()): number {
  if (overrides[metric] != null) return overrides[metric] as number;
  const s = now / 1000;
  switch (metric) {
    case "cpu.usage":
      return 35 + 25 * Math.sin(s / 4) + 8 * Math.sin(s * 1.7);
    case "cpu.temp":
      return 58 + 16 * Math.sin(s / 7);
    case "gpu.usage":
      return 45 + 40 * Math.sin(s / 5 + 1);
    case "gpu.vram":
      return 30 + 12 * Math.sin(s / 9);
    case "gpu.temp":
      return 52 + 10 * Math.sin(s / 11);
    case "ram.usage":
      return 41 + 3 * Math.sin(s / 13);
  }
  return 0;
}

export function setMetric(metric: string, value: number | null) {
  overrides[metric] = value;
}

export function read(metrics: string[]): LiveValue[] {
  return metrics.map((m) => {
    const value = fakeValue(m);
    const total = m === "gpu.vram" ? 12 * GB : m === "ram.usage" ? 60 * GB : null;
    return { metric: m, value, used: total ? (total * value) / 100 : null, total, unit: m.endsWith(".temp") ? "°C" : "%" };
  });
}

export function status(choice: string): SysmonStatus {
  const gpus = [
    { id: "nvidia:0", name: "NVIDIA GeForce RTX 4070 SUPER", vendor: "nvidia", dedicated: true, vram_total: 12 * GB },
    { id: "amd:card2", name: "AMD GPU (card2)", vendor: "amd", dedicated: false, vram_total: 2 * GB },
  ];
  const selected = gpus.find((g) => g.id === choice)?.id ?? "nvidia:0";
  return { gpu_backend: "nvml", gpus, selected, choice: choice || "auto", cpu_temp_sensor: "k10temp Tctl", metrics: ["cpu.usage", "cpu.temp", "gpu.usage", "gpu.vram", "gpu.temp", "ram.usage"].map((id) => ({ id, available: true })) };
}
