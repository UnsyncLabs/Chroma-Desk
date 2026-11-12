/**
 * Browser-only stand-in for `@tauri-apps/api` (core + event), used by `MOCK=1 npm run dev`
 * to develop and screenshot the UI without the desktop shell or hardware.
 * It is aliased in vite.config.js only when MOCK is set; production never imports it.
 */
import type { Device, EffectDef, Scene } from "../api";

type Handler = (e: { payload: unknown }) => void;
const listeners = new Map<string, Set<Handler>>();
const emit = (event: string, payload: unknown) => listeners.get(event)?.forEach((h) => h({ payload }));

export async function listen(event: string, cb: Handler) {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event)!.add(cb);
  return () => listeners.get(event)?.delete(cb);
}

const KEYS = [
  "Escape","F1","F2","F3","F4","F5","F6","F7","F8","F9","F10","F11","F12","Print Screen","Scroll Lock","Pause/Break",
  "`","1","2","3","4","5","6","7","8","9","0","-","=","Backspace","Insert","Home","Page Up",
  "Tab","Q","W","E","R","T","Y","U","I","O","P","[","]","\\ (ANSI)","Delete","End","Page Down",
  "Caps Lock","A","S","D","F","G","H","J","K","L",";","'","Enter",
  "Left Shift","Z","X","C","V","B","N","M",",",".","/","Right Shift","Up Arrow",
  "Left Control","Left Windows","Left Alt","Space","Right Alt","Right Fn","Menu","Right Control","Left Arrow","Down Arrow","Right Arrow",
].map((k) => `Key: ${k}`);

function device(id: number, name: string, type: string, zones: Device["zones"], names: string[]): Device {
  const n = names.length;
  return {
    id, key: `${name}@${id}`, name, vendor: "Mock", description: "", version: "1.0", serial: "", location: "mock", device_type: type,
    active_mode: 0, modes: [{ id: 0, name: "Static", has_speed: false, speed: null, speed_min: null, speed_max: null, has_brightness: false, brightness: null, brightness_min: null, brightness_max: null, has_direction: false, direction: null, color_mode: 1, colors: [], colors_min: null, colors_max: null, can_save: false }],
    zones, led_names: names, colors: Array.from({ length: n }, () => [10, 10, 10]),
  } as Device;
}

function makeDevices(): Device[] {
  const kbNames = [...KEYS, "Media: Volume Dial", "Key: Media Play/Pause", "Xbox Game Bar"];
  while (kbNames.length < 114) kbNames.push("");
  const map: (number | null)[] = Array(19 * 6).fill(null);
  KEYS.forEach((_, i) => (map[i] = i));
  const z = (id: number, name: string, num: number, offset: number, extra: Partial<Device["zones"][0]> = {}) => ({
    id, name, zone_type: "Linear", num_leds: num, leds_min: num, leds_max: num, resizable: false, offset, matrix: null, ...extra,
  });
  return [
    device(0, "Razer Huntsman V3 Pro TKL", "Keyboard", [z(0, "Keyboard", 114, 0, { zone_type: "Matrix", matrix: { width: 19, height: 6, map } })], kbNames),
    device(1, "Corsair Lighting Node Core", "LEDStrip", [z(0, "Corsair RGB Header 1", 24, 0, { leds_min: 0, leds_max: 96, resizable: true }), z(1, "Corsair RGB Header 2", 0, 24, { leds_min: 0, leds_max: 96, resizable: true })], Array(24).fill("")),
    device(2, "ENE DRAM", "DRam", [z(0, "DRAM", 8, 0)], Array(8).fill("")),
    device(3, "Gigabyte GeForce RTX 4070 SUPER", "Gpu", [1, 2, 3, 4, 5].map((i) => z(i - 1, `GPU zone ${i}`, 1, i - 1, { zone_type: "Single" })), Array(5).fill("")),
    device(4, "G502 HERO Gaming Mouse", "Mouse", [z(0, "Primary", 1, 0, { zone_type: "Single" }), z(1, "Logo", 1, 1, { zone_type: "Single" })], ["", ""]),
  ];
}

let devices = makeDevices();
let scene: Scene = { master_brightness: 1, sync: false, order: [], global_layers: [], devices: {} };
let visible: string[] = [];
const L = (id: string, effect: string, params: Record<string, unknown> = {}) => ({ id, name: "", enabled: true, opacity: 1, blend: "normal", mask: { type: "all" }, effect, params }) as Scene["devices"][string]["layers"][number];
const base = { mode: "effects", brightness: 1, fps: null, follow_global: true, hardware: null } as const;
scene.devices[devices[0].key] = { ...base, fan_leds: {}, layers: [L("a", "rainbow", { speed: 0.25, angle: 0 }), L("b", "paint", { leds: { 36: [255, 0, 0], 52: [255, 0, 0] } })] };
scene.devices[devices[1].key] = { ...base, fan_leds: { "Corsair RGB Header 1": 8 }, layers: [L("c", "chase", { colors: [[255, 0, 0]], reverse: false })] };

const hsv = (h: number): number[] => {
  const f = (n: number) => Math.round(255 * (0.5 + 0.5 * Math.cos(2 * Math.PI * (h + n / 3))));
  return [f(0), f(2), f(1)];
};

// Fake engine: a rainbow over every visible device in effects mode (paint layers shown as painted).
setInterval(() => {
  const t = Date.now() / 3000;
  for (const key of visible) {
    const d = devices.find((x) => x.key === key);
    const ds = scene.devices[key];
    if (!d || ds?.mode !== "effects") continue;
    const flat: number[] = [];
    d.colors.forEach((_, i) => {
      const paint = ds.layers.find((l) => l.effect === "paint")?.params.leds as Record<string, number[]> | undefined;
      flat.push(...(paint?.[i] ?? hsv(i / d.colors.length - t)));
    });
    emit("frame", { key, colors: flat });
  }
}, 100);

const DEFS: EffectDef[] = [
  { id: "solid", params: [{ key: "color", kind: "color", min: 0, max: 0, step: 0, default: [255, 0, 0] }] },
  { id: "paint", params: [{ key: "leds", kind: "paint", min: 0, max: 0, step: 0, default: {} }] },
  { id: "rainbow", params: [{ key: "speed", kind: "slider", min: -2, max: 2, step: 0.01, default: 0.25 }, { key: "angle", kind: "angle", min: 0, max: 360, step: 1, default: 0 }] },
  { id: "chase", params: [{ key: "colors", kind: "colors", min: 1, max: 8, step: 0, default: [[255, 0, 0]] }, { key: "reverse", kind: "bool", min: 0, max: 1, step: 1, default: false }] },
];

export async function invoke(cmd: string, args: Record<string, unknown> = {}): Promise<unknown> {
  switch (cmd) {
    case "connect": case "list_devices": case "rescan": case "resize_zone": return devices;
    case "get_scene": return scene;
    case "set_scene": scene = args.scene as Scene; return null;
    case "set_visible_devices": visible = args.keys as string[]; return null;
    case "effect_defs": return DEFS;
    case "effect_defaults": return Object.fromEntries(DEFS.map((d) => [d.id, Object.fromEntries(d.params.map((p) => [p.key, p.default]))]));
    case "get_blackout": return false;
    case "get_settings": return { language: "en", start_hidden: false, fps_by_type: {}, default_profile: null };
    case "list_profiles": return { profiles: [{ name: "Gaming", file: "gaming.json", openrgb: false }, { name: "OpenRGB: yado1", file: "o.json", openrgb: true }], corrupt: ["broken.json"] };
    case "profiles_path": return "~/.config/chroma-desk/profiles";
    case "get_autostart": return false;
    case "server_status": return { running: true, version: "mock", gui_open: false, service_installed: false, service_active: false };
    default: return null;
  }
}
