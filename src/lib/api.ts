import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { PaletteEvent } from "./palette.svelte";

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
  /** A duplicate OpenRGB reports for the same physical device (hidden unless Settings asks). */
  phantom?: boolean;
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
/** A reference to a palette slot (follows the theme); `lighten` > 0 mixes towards white, < 0 towards black. */
export interface SlotRef {
  slot: string;
  lighten?: number;
}
/** A fixed colour or a palette slot. */
export type Color = Rgb | SlotRef;
export type ParamValue = number | boolean | string | Color | Color[] | Record<string, Color> | Record<string, unknown> | unknown[] | null;

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
  kind: "slider" | "angle" | "color" | "colors" | "bool" | "paint" | "data";
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
  show_phantom_devices: boolean;
  builtin_enabled: Record<string, boolean>;
  theme_rules: ThemeRule[];
  global_notifications: Record<string, NamedTrigger>;
  desktop_notifications: boolean;
  /** GPU the gpu.* monitors read: "auto" or an id such as "nvidia:0". */
  sysmon_gpu?: string;
}

export type ThemeSourceId = "manual" | "omarchy" | "kde" | "pywal" | "gtk" | "external";
export interface ProfileTheme {
  source: ThemeSourceId;
  manual_palette?: Record<string, Rgb> | null;
  overrides?: Record<string, Rgb>;
}
export interface AnimRef {
  id: string;
  name: string;
}
export interface DesktopMatch {
  /** Part of the application name (case-insensitive); empty = any. */
  app: string;
  urgency?: "low" | "normal" | "critical" | null;
  /** Text the summary must contain; empty = anything. */
  contains: string;
}
export interface AiMatch {
  /** Tool name (claude, codex...); empty = any. */
  from: string;
  /** permission, idle...; empty = any. */
  kind: string;
}
export interface EverySpec {
  every_secs: number;
  for_secs: number;
}
export interface ThresholdSpec {
  metric: string;
  op: "gt" | "lt";
  value: number;
  for_secs: number;
  hysteresis: number;
  cooldown_secs: number;
}
export interface NotifSources {
  desktop?: DesktopMatch | null;
  omarchy?: string[];
  ai?: AiMatch | null;
  /** Always on: loops over the scene while enabled. */
  always?: boolean;
  every?: EverySpec | null;
  threshold?: ThresholdSpec | null;
}
/** A notification: what to show, where, in which colour, and what fires it. */
export interface NamedTrigger {
  /** Library animation id or name (empty when `sequence` is used). */
  anim: string;
  /** A sequence stored in the automation itself; wins over `anim`. */
  sequence?: Animation | null;
  keys?: string[];
  /** `@slot` or `#rrggbb`. */
  color?: string | null;
  duration?: number | null;
  device?: string | null;
  enabled?: boolean;
  devices?: string[];
  repeat?: number | null;
  speed?: number | null;
  priority?: number;
  replace?: boolean;
  sources?: NotifSources;
}
export interface EventHookStatus {
  kind: string;
  event: string;
  installed: boolean;
  path: string;
  script_text: string;
  command: string;
}
export interface Profile {
  schema_version: number;
  name: string;
  theme: ProfileTheme;
  presets?: AnimRef[];
  triggers?: Record<string, NamedTrigger>;
  enabled?: boolean;
  builtin?: boolean;
  requires?: string | null;
}
export interface ProfileInfo {
  name: string;
  file: string;
  openrgb: boolean;
  builtin: boolean;
  enabled: boolean;
  requires: string | null;
  source: ThemeSourceId;
  /** A few palette colours for the card (empty when the source is dynamic). */
  strip: Rgb[];
  triggers: number;
}
export interface SourceStatus {
  id: ThemeSourceId;
  detected: boolean;
  name: string | null;
  error: string | null;
  strip: Rgb[];
}
export interface ThemeStatus {
  active: ThemeSourceId;
  runtime_override: boolean;
  external: boolean;
  error: string | null;
  sources: SourceStatus[];
  palette: { name: string; source: string; slots: Record<string, Rgb>; generation: number };
}
export interface HookStatus {
  available: boolean;
  installed: boolean;
  path: string;
  script: string;
  command: string;
}
export interface ThemeRule {
  theme: string;
  profile: string;
}

export interface ProfileList {
  profiles: ProfileInfo[];
  corrupt: string[];
}

export interface ServerStatus {
  running: boolean;
  gui_open: boolean;
  service_installed: boolean;
  service_enabled: boolean;
  protocol: number | null;
}

export interface TrayLabels {
  show: string;
  profiles: string;
  no_profiles: string;
  lights_off: string;
  lights_on: string;
  quit: string;
}

// ---------- animations (mirrors src-tauri/src/animations.rs) ----------

export interface Stop {
  pos: number;
  color: Color;
}
export type LoopMode = "loop" | "pingpong" | "once";
export interface Frame {
  /** LedRef -> colour. */
  leds: Record<string, Color>;
  duration_ms?: number | null;
}
export type AnimKind =
  | { type: "effect"; effect: string; params: Record<string, ParamValue> }
  | { type: "frames"; frames: Frame[]; frame_ms: number; loop: LoopMode; crossfade: number }
  | { type: "path"; path: string[]; palette: Stop[]; speed: number; length: number; gap: number; reverse: boolean; tail_fade: number }
  | { type: "palette"; cells: Record<string, number>; palette: Stop[]; speed: number; reverse: boolean }
  | { type: "sequence"; steps: SeqStep[]; loop: boolean };
/** One step of a sequence: `meter`, `blink`, `orbit`, `hold`, `animation` or any effect id. */
export interface SeqStep {
  effect: string;
  params: Record<string, ParamValue>;
  /** Ordered LED references: the route of meter/blink/orbit, the lit LEDs of the other effects. */
  keys: string[];
  duration_ms: number;
  anim?: Animation | null;
}
export interface Animation {
  schema_version: number;
  id: string;
  name: string;
  description?: string | null;
  tags: string[];
  created: number;
  updated: number;
  target: { kind: "keyboard" | "linear" | "any"; hint?: string | null };
  builtin: boolean;
  kind: AnimKind;
}
export interface AnimList {
  animations: Animation[];
  corrupt: string[];
}
export interface TriggerSpec {
  animation: string;
  device?: string;
  keys?: string[];
  duration?: number | string;
  color?: string;
  repeat?: number;
  priority?: number;
  speed?: number;
  replace?: boolean;
}
export interface TriggerResult {
  overlay: number;
  devices: number;
  duration: number;
}

export interface GpuInfo {
  id: string;
  name: string;
  vendor: string;
  dedicated: boolean;
  vram_total: number | null;
}
export interface SysmonStatus {
  gpu_backend: "nvml" | "nvidia-smi" | "sysfs" | "none";
  gpus: GpuInfo[];
  selected: string | null;
  choice: string;
  cpu_temp_sensor: string | null;
  metrics: { id: string; available: boolean }[];
}
export interface LiveValue {
  metric: string;
  value: number | null;
  /** Bytes (memory metrics). */
  used: number | null;
  total: number | null;
  unit: string;
}
export interface AutomationTemplate {
  id: string;
  category: "monitor" | "notification";
  notification: NamedTrigger;
}
export interface ClaudeStatus {
  path: string;
  exists: boolean;
  valid: boolean;
  installed: boolean;
  command: string;
  snippet: string;
  backup: string | null;
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
  getAutostart: () => invoke<boolean>("get_autostart"),
  setAutostart: (enabled: boolean) => invoke<void>("set_autostart", { enabled }),
  serverStatus: () => invoke<ServerStatus>("server_status"),
  setServerService: (enabled: boolean) => invoke<void>("set_server_service", { enabled }),
  setTrayLabels: (labels: TrayLabels) => invoke<void>("set_tray_labels", { labels }),
  refreshTray: () => invoke<void>("refresh_tray"),
  showWindow: () => invoke<void>("show_window"),
  quitApp: () => invoke<void>("quit_app"),
  defaultFps: (types: string[]) => invoke<Record<string, number>>("default_fps", { types }),
  getBlackout: () => invoke<boolean>("get_blackout"),
  setBlackout: (on: boolean) => invoke<void>("set_blackout", { on }),
  animList: () => invoke<AnimList>("anim_list"),
  animGet: (id: string) => invoke<Animation>("anim_get", { id }),
  animSave: (anim: Animation) => invoke<Animation>("anim_save", { anim }),
  animDelete: (id: string) => invoke<void>("anim_delete", { id }),
  animDuplicate: (id: string, name: string) => invoke<Animation>("anim_duplicate", { id, name }),
  animImport: (path: string) => invoke<Animation>("anim_import", { path }),
  animExport: (id: string, path: string) => invoke<void>("anim_export", { id, path }),
  animApply: (deviceKey: string, id: string) => invoke<Scene>("anim_apply", { deviceKey, id }),
  trigger: (spec: TriggerSpec) => invoke<TriggerResult>("trigger", { spec }),
  stopTriggers: () => invoke<number>("stop_triggers"),
  triggerSocketPath: () => invoke<string | null>("trigger_socket_path"),
  getPalette: () => invoke<PaletteEvent>("get_palette"),
  themeStatus: () => invoke<ThemeStatus>("theme_status"),
  paletteReload: () => invoke<boolean>("palette_reload"),
  profileGet: (name: string) => invoke<Profile>("profile_get", { name }),
  profileUpdate: (name: string, theme: ProfileTheme, presets: AnimRef[], triggers: Record<string, NamedTrigger>) => invoke<void>("profile_update", { name, theme, presets, triggers }),
  setBuiltinEnabled: (name: string, on: boolean) => invoke<void>("set_builtin_enabled", { name, on }),
  omarchyHookStatus: () => invoke<HookStatus>("omarchy_hook_status"),
  omarchyHookInstall: () => invoke<HookStatus>("omarchy_hook_install"),
  omarchyHookRemove: () => invoke<HookStatus>("omarchy_hook_remove"),
  omarchyEventHooks: () => invoke<EventHookStatus[]>("omarchy_event_hooks"),
  omarchyEventHookInstall: (kind: string) => invoke<EventHookStatus[]>("omarchy_event_hook_install", { kind }),
  omarchyEventHookRemove: (kind: string) => invoke<EventHookStatus[]>("omarchy_event_hook_remove", { kind }),
  desktopStatus: () => invoke<string>("desktop_status"),
  testAutomation: (name: string, automation: NamedTrigger) => invoke<TriggerResult>("test_automation", { name, automation }),
  sysmonStatus: () => invoke<SysmonStatus>("sysmon_status"),
  sysmonRead: (metrics: string[]) => invoke<LiveValue[]>("sysmon_read", { metrics }),
  automationTemplates: () => invoke<AutomationTemplate[]>("automation_templates"),
  claudeStatus: () => invoke<ClaudeStatus>("claude_status"),
  claudeInstall: () => invoke<ClaudeStatus>("claude_install"),
  claudeRemove: () => invoke<ClaudeStatus>("claude_remove"),
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
