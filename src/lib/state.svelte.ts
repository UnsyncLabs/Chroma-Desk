/** Shared UI state: devices, navigation and toasts. */
import { api, type Device } from "./api";
import { errText, t } from "./i18n/index.svelte";

export type View = { kind: "all" } | { kind: "device"; key: string } | { kind: "profiles" } | { kind: "settings" };

export const app = $state({
  devices: [] as Device[],
  loading: true,
  /** Connection error shown as a full-screen empty state. */
  error: "",
  view: { kind: "all" } as View,
});

export const currentDevice = () => (app.view.kind === "device" ? app.devices.find((d) => d.key === (app.view as { key: string }).key) : undefined);

// ---------- toasts ----------
export interface Toast {
  id: number;
  kind: "info" | "error" | "ok";
  text: string;
}
export const toasts = $state<{ list: Toast[] }>({ list: [] });
let toastId = 0;

export function toast(text: string, kind: Toast["kind"] = "info", ms = 4500) {
  const id = ++toastId;
  toasts.list = [...toasts.list, { id, kind, text }];
  setTimeout(() => dismissToast(id), ms);
}
export const dismissToast = (id: number) => (toasts.list = toasts.list.filter((x) => x.id !== id));
export const toastError = (e: unknown) => toast(errText(e), "error", 7000);

// ---------- devices ----------
export async function loadDevices(fn: () => Promise<Device[]> = api.listDevices) {
  app.loading = true;
  app.error = "";
  try {
    app.devices = await fn();
    if (app.view.kind === "device" && !app.devices.some((d) => d.key === (app.view as { key: string }).key)) {
      app.view = { kind: "all" };
    }
  } catch (e) {
    app.error = errText(e);
  } finally {
    app.loading = false;
  }
}

export function setDevices(d: Device[]) {
  app.devices = d;
}

export async function rescan() {
  await loadDevices(api.rescan);
  if (!app.error) toast(t("toast.rescanned", { n: app.devices.length }), "ok");
}

// ---------- fan presets (LEDs per fan by zone), per device ----------
const FAN_KEY = "chroma-desk.fans";
function loadFans(): Record<string, Record<string, number>> {
  try {
    return JSON.parse(localStorage.getItem(FAN_KEY) ?? "{}");
  } catch {
    return {};
  }
}
export const fans = $state<{ map: Record<string, Record<string, number>> }>({ map: loadFans() });

export function setFanLeds(deviceKey: string, zone: string, perFan: number) {
  fans.map = { ...fans.map, [deviceKey]: { ...fans.map[deviceKey], [zone]: perFan } };
  try {
    localStorage.setItem(FAN_KEY, JSON.stringify(fans.map));
  } catch {
    /* storage unavailable */
  }
}
