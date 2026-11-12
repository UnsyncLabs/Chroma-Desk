import { invoke } from "@tauri-apps/api/core";

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

export const api = {
  connect: () => invoke<Device[]>("connect"),
  listDevices: () => invoke<Device[]>("list_devices"),
  rescan: () => invoke<Device[]>("rescan"),
  setColorAll: (color: Rgb) => invoke<string[]>("set_color_all", { color }),
  setDeviceColor: (device: number, color: Rgb) => invoke<void>("set_device_color", { device, color }),
  setLeds: (device: number, colors: Rgb[], ensureDirect = false) =>
    invoke<void>("set_leds", { device, colors, ensureDirect }),
  setMode: (device: number, req: ModeRequest) => invoke<void>("set_mode", { device, req }),
  resizeZone: (device: number, zone: number, size: number) =>
    invoke<Device[]>("resize_zone", { device, zone, size }),
};

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
