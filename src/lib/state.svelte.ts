/** Shared UI state: devices, navigation and toasts. */
import { api, type Device } from "./api";
import { errText, t } from "./i18n/index.svelte";

export type View = { kind: "all" } | { kind: "device"; key: string } | { kind: "animations" } | { kind: "profiles" } | { kind: "notifications" } | { kind: "settings" };

export const app = $state({
  devices: [] as Device[],
  loading: true,
  /** Connection error shown as a full-screen empty state. */
  error: "",
  view: { kind: "all" } as View,
  /** An animation the library asked to edit on a device (consumed by that device's Draw tab). */
  pendingEdit: null as import("./api").Animation | "new" | null,
});

/** Hooks of screens that hold unsaved work (the animation editor): consulted before leaving. */
export const leaveGuard: { dirty: (() => boolean) | null } = { dirty: null };

/** True when it is fine to leave the current screen (asks the user when there are unsaved edits). */
export async function confirmLeave(): Promise<boolean> {
  if (!leaveGuard.dirty?.()) return true;
  return confirmDialog(t("draw.confirmLeave"), t("draw.leave"));
}

/** Navigates to `view`, unless the user decides to keep editing. */
export async function navigate(view: View): Promise<boolean> {
  if (!(await confirmLeave())) return false;
  app.view = view;
  return true;
}

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

// ---------- confirmation dialog ----------
export const confirmState = $state<{ open: boolean; text: string; confirmLabel: string; resolve: ((ok: boolean) => void) | null }>({
  open: false,
  text: "",
  confirmLabel: "",
  resolve: null,
});

/** Asks the user to confirm a destructive action. */
export function confirmDialog(text: string, confirmLabel = ""): Promise<boolean> {
  return new Promise((resolve) => {
    confirmState.text = text;
    confirmState.confirmLabel = confirmLabel;
    confirmState.resolve = resolve;
    confirmState.open = true;
  });
}

// ---------- first-run tour ----------
const TOUR_KEY = "chroma-desk.tour";
export const tour = $state({ open: false });

/** Opens the 3-step tour the first time the app runs (and never again once finished or skipped). */
export function maybeStartTour() {
  try {
    if (localStorage.getItem(TOUR_KEY) !== "done") tour.open = true;
  } catch {
    /* storage unavailable: skip the tour */
  }
}

export function finishTour() {
  tour.open = false;
  try {
    localStorage.setItem(TOUR_KEY, "done");
  } catch {
    /* storage unavailable */
  }
}

/** Copies text to the clipboard and tells the user (falls back to showing it). */
export async function copyText(text: string, doneMessage: string) {
  try {
    await navigator.clipboard.writeText(text);
    toast(doneMessage, "ok", 2500);
  } catch {
    toast(text, "info", 9000);
  }
}
