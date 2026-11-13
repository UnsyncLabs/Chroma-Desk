/** Scene store: the frontend owns the scene and pushes every change to the engine. */
import { api, on, throttle, type Device, type DeviceScene, type EffectDef, type EngineStatus, type Layer, type ParamValue, type Rgb, type Scene } from "./api";
import { buildLayout } from "./layout/buildLayout";
import { confirmDialog, toast, toastError } from "./state.svelte";
import { t } from "./i18n/index.svelte";

export const emptyScene = (): Scene => ({ master_brightness: 1, sync: false, order: [], global_layers: [], devices: {} });
export const emptyDevice = (): DeviceScene => ({ mode: "hardware", layers: [], brightness: 1, fps: null, follow_global: true, fan_leds: {}, hardware: null });

export const sceneStore = $state({
  scene: emptyScene(),
  defs: [] as EffectDef[],
  defaults: {} as Record<string, Record<string, ParamValue>>,
  blackout: false,
  status: "ok" as EngineStatus,
  /** Serialised scene of the last saved/applied profile (for the unsaved indicator). */
  baseline: "",
});

/** Live frames by device key (RGB triples). Replaced wholesale on each event. */
class Frames {
  /** Reassigned wholesale on every event (a `$state.raw` field is reactive on reassignment). */
  map = $state.raw<Record<string, Rgb[]>>({});
}
export const frames = new Frames();

/**
 * Lets the animation editor show its unsaved draft on the hardware: the hook rewrites a *copy* of
 * the scene on its way to the engine, so the real scene (and the dirty state) stays untouched.
 */
export const previewHook: { fn: ((s: Scene) => Scene) | null } = { fn: null };

/** What the engine should run: the scene, with the editor's preview applied if active. */
function outgoing(s: Scene): Scene {
  const snap = $state.snapshot(s) as Scene;
  return previewHook.fn ? previewHook.fn(snap) : snap;
}

const push = throttle((s: Scene) => api.setScene(outgoing(s)).catch(toastError), 40);

/** Sends the scene to the engine (throttled). Call after every mutation. */
export function commit() {
  push(sceneStore.scene);
}

/** Sends the scene immediately and waits for it (before actions that must not race the engine). */
export async function flush() {
  await api.setScene(outgoing(sceneStore.scene)).catch(toastError);
}

export const isDirty = () => JSON.stringify($state.snapshot(sceneStore.scene)) !== sceneStore.baseline;
export const markClean = () => (sceneStore.baseline = JSON.stringify($state.snapshot(sceneStore.scene)));

export function setScene(s: Scene) {
  sceneStore.scene = s;
}

export function devScene(key: string): DeviceScene {
  return sceneStore.scene.devices[key] ?? emptyDevice();
}

/** Mutable device scene (created on first use). */
export function ensureDev(key: string): DeviceScene {
  sceneStore.scene.devices[key] ??= emptyDevice();
  return sceneStore.scene.devices[key];
}

export function newLayer(effect: string, extra: Partial<Layer> = {}): Layer {
  return {
    id: crypto.randomUUID(),
    name: "",
    enabled: true,
    opacity: 1,
    blend: "normal",
    mask: { type: "all" },
    effect,
    params: $state.snapshot(sceneStore.defaults[effect] ?? {}) as Record<string, ParamValue>,
    ...extra,
  };
}

export function setFanLeds(key: string, zone: string, perFan: number) {
  ensureDev(key).fan_leds[zone] = perFan;
  commit();
}

export async function loadEngine() {
  const [scene, defs, defaults, blackout] = await Promise.all([api.getScene(), api.effectDefs(), api.effectDefaults(), api.getBlackout()]);
  sceneStore.defs = defs;
  sceneStore.defaults = defaults;
  sceneStore.blackout = blackout;
  setScene(scene);
  markClean();
}

/** Sends the physical LED positions of every device so position-based effects (and global sync) match the drawing. */
const sentPositions = new Map<string, string>();
export function syncPositions(devices: Device[]) {
  for (const d of devices) {
    const layout = buildLayout(d, { fanLeds: sceneStore.scene.devices[d.key]?.fan_leds });
    const sig = JSON.stringify(layout.positions);
    if (sentPositions.get(d.key) === sig) continue;
    sentPositions.set(d.key, sig);
    api.setLedPositions(d.key, layout.positions).catch(toastError);
  }
}

/** Starts listening to engine events; returns a cleanup function. */
export async function listenEngine(onDevicesChanged: () => void): Promise<() => void> {
  const offs = await Promise.all([
    on<{ key: string; colors: number[] }>("frame", (f) => {
      const cols: Rgb[] = [];
      for (let i = 0; i + 2 < f.colors.length; i += 3) cols.push([f.colors[i], f.colors[i + 1], f.colors[i + 2]]);
      frames.map = { ...frames.map, [f.key]: cols };
    }),
    on<{ state: EngineStatus }>("engine-status", (s) => (sceneStore.status = s.state)),
    on<null>("devices-changed", onDevicesChanged),
    on<{ code: string; detail?: string }>("startup-error", (e) => toastError(e)),
    on<null>("tray-unavailable", () => toast(t("toast.noTray"), "info", 8000)),
    on<null>("request-quit", async () => {
      if (isDirty()) {
        await api.showWindow();
        if (!(await confirmDialog(t("quit.confirmUnsaved"), t("quit.quit")))) return;
      }
      await api.quitApp();
    }),
    on<boolean>("blackout", (b) => (sceneStore.blackout = b)),
    on<Scene>("scene-changed", (s) => {
      setScene(s);
      markClean();
    }),
  ]);
  return () => offs.forEach((f) => f());
}
