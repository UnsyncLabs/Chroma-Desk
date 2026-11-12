import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Rgb = [number, number, number];

export interface Matrix {
  width: number;
  height: number;
  map: (number | null)[];
}

export interface Zone {
  id: number;
  name: string;
  zone_type: string;
  num_leds: number;
  leds_min: number;
  leds_max: number;
  resizable: boolean;
  offset: number;
  matrix: Matrix | null;
}

export interface Mode {
  id: number;
  name: string;
  has_speed: boolean;
  speed: number | null;
  speed_min: number | null;
  speed_max: number | null;
  has_brightness: boolean;
  brightness: number | null;
  brightness_min: number | null;
  brightness_max: number | null;
  has_direction: boolean;
  direction: number | null;
  color_mode: number;
  colors: Rgb[];
  colors_min: number | null;
  colors_max: number | null;
  can_save: boolean;
}

export interface Device {
  id: number;
  /** Stable identity (name#serial or name@ordinal). */
  key: string;
  name: string;
  vendor: string;
  description: string;
  version: string;
  serial: string;
  location: string;
  device_type: string;
  active_mode: number;
  modes: Mode[];
  zones: Zone[];
  led_names: string[];
  colors: Rgb[];
}

export interface ModeRequest {
  mode: number;
  speed?: number;
  brightness?: number;
  direction?: number;
  colors?: Rgb[];
  save?: boolean;
}

// ---------- scene (mirrors src-tauri/src/engine) ----------

export type Blend = "normal" | "add" | "multiply" | "screen";
export type Mask = { type: "all" } | { type: "zone"; id: number } | { type: "leds"; leds: number[] };
export type ParamValue = number | boolean | Rgb | Rgb[] | Record<string, Rgb>;

export interface Layer {
  id: string;
  name: string;
  enabled: boolean;
  opacity: number;
  blend: Blend;
  mask: Mask;
  effect: string;
  params: Record<string, ParamValue>;
}

export interface HardwareMode {
  mode: string;
  speed?: number | null;
  brightness?: number | null;
  direction?: number | null;
  colors?: Rgb[] | null;
}

export interface DeviceScene {
  mode: "effects" | "hardware";
  layers: Layer[];
  brightness: number;
  fps: number | null;
  follow_global: boolean;
  fan_leds: Record<string, number>;
  hardware: HardwareMode | null;
}

export interface Scene {
  master_brightness: number;
  sync: boolean;
  order: string[];
  global_layers: Layer[];
  devices: Record<string, DeviceScene>;
}

export interface ParamDef {
  key: string;
  kind: "slider" | "angle" | "color" | "colors" | "bool" | "paint";
  min: number;
  max: number;
  step: number;
  default: ParamValue;
}

export interface EffectDef {
  id: string;
  params: ParamDef[];
}

export interface FrameEvent {
  key: string;
  /** Flat RGB bytes. */
  colors: number[];
}

export interface Settings {
  language: string | null;
  start_hidden: boolean;
  default_profile: string | null;
  current_profile: string | null;
  fps_by_type: Record<string, number>;
}

export interface ProfileInfo {
  name: string;
  file: string;
  openrgb: boolean;
}

export interface ProfileList {
  profiles: ProfileInfo[];
  corrupt: string[];
}

export type EngineStatus = "ok" | "reconnecting" | "server_down";

export const api = {
  connect: () => invoke<Device[]>("connect"),
  listDevices: () => invoke<Device[]>("list_devices"),
  rescan: () => invoke<Device[]>("rescan"),
  setLeds: (device: number, colors: Rgb[], ensureDirect = false) =>
    invoke<void>("set_leds", { device, colors, ensureDirect }),
  setMode: (device: number, req: ModeRequest) => invoke<void>("set_mode", { device, req }),
  resizeZone: (device: number, zone: number, size: number) =>
    invoke<Device[]>("resize_zone", { device, zone, size }),
  getScene: () => invoke<Scene>("get_scene"),
  setScene: (scene: Scene) => invoke<void>("set_scene", { scene }),
  setLedPositions: (key: string, positions: [number, number][]) => invoke<void>("set_led_positions", { key, positions }),
  setVisibleDevices: (keys: string[]) => invoke<void>("set_visible_devices", { keys }),
  effectDefs: () => invoke<EffectDef[]>("effect_defs"),
  effectDefaults: () => invoke<Record<string, Record<string, ParamValue>>>("effect_defaults"),
  setFpsByType: (fps: Record<string, number>) => invoke<void>("set_fps_by_type", { fps }),
  waitStartup: () => invoke<void>("wait_startup"),
  listProfiles: () => invoke<ProfileList>("list_profiles"),
  profilesPath: () => invoke<string>("profiles_path"),
  saveProfile: (name: string) => invoke<ProfileInfo>("save_profile", { name }),
  applyProfile: (name: string) => invoke<Scene>("apply_profile", { name }),
  deleteProfile: (name: string) => invoke<void>("delete_profile", { name }),
  renameProfile: (old: string, newName: string) => invoke<ProfileInfo>("rename_profile", { old, new: newName }),
  duplicateProfile: (name: string, newName: string) => invoke<ProfileInfo>("duplicate_profile", { name, new: newName }),
  exportProfile: (name: string, path: string) => invoke<void>("export_profile", { name, path }),
  importProfile: (path: string) => invoke<ProfileInfo>("import_profile", { path }),
  listOpenrgbProfiles: () => invoke<string[]>("list_openrgb_profiles"),
  importOpenrgbProfile: (name: string) => invoke<ProfileInfo>("import_openrgb_profile", { name }),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<void>("set_settings", { settings }),
  getBlackout: () => invoke<boolean>("get_blackout"),
  setBlackout: (on: boolean) => invoke<void>("set_blackout", { on }),
};

/** Subscribes to a backend event; resolves to the unsubscribe function. */
export const on = <T>(event: string, cb: (payload: T) => void): Promise<UnlistenFn> =>
  listen<T>(event, (e) => cb(e.payload));

export const hex = (c: Rgb) => "#" + c.map((v) => v.toString(16).padStart(2, "0")).join("");
export const fromHex = (h: string): Rgb => [
  parseInt(h.slice(1, 3), 16),
  parseInt(h.slice(3, 5), 16),
  parseInt(h.slice(5, 7), 16),
];

/** Runs `fn` at most every `ms`, making sure the last call is applied. */
export function throttle<A extends unknown[]>(fn: (...a: A) => void, ms: number) {
  let last = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let pending: A | null = null;
  return (...a: A) => {
    pending = a;
    const now = Date.now();
    const run = () => {
      last = Date.now();
      timer = null;
      if (pending) fn(...pending);
      pending = null;
    };
    if (now - last >= ms) run();
    else if (!timer) timer = setTimeout(run, ms - (now - last));
  };
}
