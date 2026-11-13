/**
 * Browser-only stand-in for `@tauri-apps/api` (core + event), used by `MOCK=1 npm run dev`
 * to develop and screenshot the UI without the desktop shell or hardware, and by the e2e tests
 * (`npm run e2e`). It is aliased in vite.config.js only when MOCK is set; production never imports it.
 *
 * State lives on `globalThis.__chromaMock` so tests can assert on what the "backend" received.
 */
import type { Animation, Device, EffectDef, ParamDef, Scene } from "../api";
import builtinsJson from "./builtins.json";
import { renderAnimation, renderEffect, flatten, type Px } from "../anim/render";
import { buildLayout } from "../layout/buildLayout";

type Handler = (e: { payload: unknown }) => void;
interface MockOverlay {
  id: number;
  anim: Animation;
  device: string | null;
  keys: string[];
  start: number;
  until: number;
}
interface MockState {
  listeners: Map<string, Set<Handler>>;
  scene: Scene;
  visible: string[];
  started: boolean;
  t0: number;
  /** User animations (the library on disk). */
  animations: Animation[];
  /** Files "on disk" by path (import/export). */
  files: Record<string, Animation>;
  nextOpen: string | null;
  nextSave: string | null;
  triggers: unknown[];
  overlays: MockOverlay[];
  counter: number;
  setSceneCalls: number;
}
// Vite may instantiate this module twice (once per aliased specifier); keep one shared state.
const G = globalThis as unknown as { __chromaMock?: MockState };
const M = (G.__chromaMock ??= {
  listeners: new Map<string, Set<Handler>>(),
  scene: { master_brightness: 1, sync: false, order: [], global_layers: [], devices: {} } as Scene,
  visible: [],
  started: false,
  t0: Date.now(),
  animations: [],
  files: {},
  nextOpen: null,
  nextSave: null,
  triggers: [],
  overlays: [],
  counter: 0,
  setSceneCalls: 0,
});
const listeners = M.listeners;
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

const devices = makeDevices();
const L = (id: string, effect: string, params: Record<string, unknown> = {}) => ({ id, name: "", enabled: true, opacity: 1, blend: "normal", mask: { type: "all" }, effect, params }) as Scene["devices"][string]["layers"][number];
const base = { mode: "effects", brightness: 1, fps: null, follow_global: true, hardware: null } as const;
if (!M.started) M.scene.devices[devices[0].key] = { ...base, fan_leds: {}, layers: [L("a", "rainbow", { speed: 0.25, angle: 0 }), L("b", "paint", { leds: { 36: [255, 0, 0], 52: [255, 0, 0] } })] };
if (!M.started) M.scene.devices[devices[1].key] = { ...base, fan_leds: { "Corsair RGB Header 1": 8 }, layers: [L("c", "chase", { colors: [[255, 0, 0]], reverse: false })] };

// ---------- fake engine ----------

const layoutCache = new Map<string, [number, number][]>();
const positionsOf = (d: Device) => {
  if (!layoutCache.has(d.key)) layoutCache.set(d.key, buildLayout(d).positions);
  return layoutCache.get(d.key)!;
};

/** Composes a device the way the engine would (layers bottom to top, then overlays), without blend modes. */
function renderDevice(d: Device, ds: Scene["devices"][string], t: number, now: number): number[] {
  const n = d.colors.length;
  const names = Array.from({ length: n }, (_, i) => d.led_names[i] ?? "");
  const pos = positionsOf(d);
  let buf: [number, number, number][] = Array.from({ length: n }, () => [0, 0, 0]);
  const over = (px: Px[], mask?: Set<number>) => {
    const fl = px.map((p, i) => ({ p, i }));
    for (const { p, i } of fl) {
      if (p[3] <= 0 || (mask && !mask.has(i))) continue;
      const b = buf[i];
      buf[i] = [b[0] + (p[0] - b[0]) * p[3], b[1] + (p[1] - b[1]) * p[3], b[2] + (p[2] - b[2]) * p[3]];
    }
  };
  for (const l of ds.layers) {
    if (!l.enabled) continue;
    if (l.effect === "paint") {
      const leds = (l.params.leds ?? {}) as Record<string, number[]>;
      over(Array.from({ length: n }, (_, i) => (leds[i] ? [leds[i][0], leds[i][1], leds[i][2], 1] : [0, 0, 0, 0]) as Px));
    } else if (l.effect === "animation") {
      const snap = l.params.snapshot as unknown as Animation | undefined;
      if (snap) over(renderAnimation(snap, t, names, pos).map((p) => [p[0], p[1], p[2], p[3] * l.opacity] as Px));
    } else {
      over(renderEffect(l.effect, l.params, t, pos, n).map((p) => [p[0], p[1], p[2], p[3] * l.opacity] as Px));
    }
  }
  M.overlays = M.overlays.filter((o) => o.until > now);
  for (const o of M.overlays) {
    if (o.device && !d.name.toLowerCase().includes(o.device.toLowerCase()) && d.key !== o.device) continue;
    const mask = o.keys.length ? new Set(names.flatMap((nm, i) => (o.keys.some((k) => nm.toLowerCase() === k.toLowerCase() || nm.toLowerCase() === `key: ${k.toLowerCase()}`) ? [i] : []))) : undefined;
    over(renderAnimation(o.anim, (now - o.start) / 1000, names, pos), mask);
  }
  buf = flatten(buf.map((c) => [c[0], c[1], c[2], 1] as Px));
  return buf.flat();
}

if (!M.started) setInterval(() => {
  const now = Date.now();
  const t = (now - M.t0) / 1000;
  for (const key of M.visible) {
    const d = devices.find((x) => x.key === key);
    const ds = M.scene.devices[key];
    if (!d || ds?.mode !== "effects") continue;
    emit("frame", { key, colors: renderDevice(d, ds, t, now) });
  }
}, 60);

M.started = true;

// ---------- effect schema (mirrors effect_defs in effects.rs) ----------

const slider = (key: string, min: number, max: number, step: number, def: number): ParamDef => ({ key, kind: "slider", min, max, step, default: def });
const angle = (key: string, def: number): ParamDef => ({ key, kind: "angle", min: 0, max: 360, step: 1, default: def });
const color = (key: string, def: number[]): ParamDef => ({ key, kind: "color", min: 0, max: 0, step: 0, default: def as never });
const colors = (key: string, def: number[][]): ParamDef => ({ key, kind: "colors", min: 1, max: 8, step: 0, default: def as never });
const flag = (key: string, def: boolean): ParamDef => ({ key, kind: "bool", min: 0, max: 1, step: 1, default: def });
const data = (key: string, def: unknown): ParamDef => ({ key, kind: "data", min: 0, max: 0, step: 0, default: def as never });
const RED = [255, 0, 0], BLUE = [0, 80, 255], WHITE = [255, 255, 255];
const PAL = [{ pos: 0, color: RED }, { pos: 1, color: BLUE }];
const DEFS: EffectDef[] = [
  { id: "solid", params: [color("color", RED)] },
  { id: "paint", params: [{ key: "leds", kind: "paint", min: 0, max: 0, step: 0, default: {} }] },
  { id: "gradient", params: [colors("colors", [RED, BLUE]), angle("angle", 0), slider("speed", -1, 1, 0.01, 0)] },
  { id: "breathing", params: [colors("colors", [RED]), slider("period", 0.5, 12, 0.1, 4)] },
  { id: "spectrum", params: [slider("speed", 0.02, 2, 0.01, 0.15), slider("saturation", 0, 1, 0.01, 1)] },
  { id: "rainbow", params: [slider("speed", -2, 2, 0.01, 0.25), angle("angle", 0), slider("scale", 0.2, 4, 0.05, 1)] },
  { id: "color_wave", params: [colors("colors", [RED, BLUE]), slider("speed", -2, 2, 0.01, 0.25), angle("angle", 0), slider("scale", 0.2, 4, 0.05, 1)] },
  { id: "starlight", params: [slider("density", 0.05, 1, 0.01, 0.35), slider("duration", 0.2, 6, 0.05, 1.5), flag("random", true), colors("colors", [WHITE])] },
  { id: "strobe", params: [colors("colors", [WHITE]), slider("rate", 0.5, 20, 0.1, 4), slider("duty", 0.05, 0.95, 0.01, 0.5)] },
  { id: "chase", params: [colors("colors", [RED]), slider("speed", 0.05, 3, 0.01, 0.6), slider("length", 0.05, 1, 0.01, 0.3), slider("gap", 0, 1, 0.01, 0.4), slider("segment", 0, 64, 1, 0), flag("reverse", false)] },
  { id: "frames", params: [data("frames", []), slider("frame_ms", 20, 2000, 10, 200), data("loop", "loop"), slider("crossfade", 0, 1, 0.01, 0)] },
  { id: "path", params: [data("path", []), data("palette", PAL), slider("speed", 0.05, 3, 0.01, 0.5), slider("length", 0.05, 1, 0.01, 0.35), slider("gap", 0, 1, 0.01, 0.3), slider("tail_fade", 0, 1, 0.01, 0.6), flag("reverse", false)] },
  { id: "palette", params: [data("cells", {}), data("palette", PAL), slider("speed", -2, 2, 0.01, 0.3), flag("reverse", false)] },
  { id: "animation", params: [data("anim_id", ""), data("snapshot", null)] },
  { id: "ripple", params: [slider("rate", 0.1, 5, 0.05, 0.8), slider("speed", 0.2, 3, 0.01, 0.9), slider("width", 0.02, 0.5, 0.01, 0.12), flag("random", true), colors("colors", [BLUE])] },
];

// ---------- animation library ----------

const builtins = builtinsJson as unknown as Animation[];
const allAnims = () => [...builtins, ...M.animations];
const fail = (code: string, detail = ""): never => {
  throw { code, detail };
};
const clone = <T,>(v: T): T => JSON.parse(JSON.stringify(v));

function saveAnim(a: Animation): Animation {
  const name = a.name.trim();
  if (!name || name.length > 64) fail("anim_name_invalid", name);
  if (!a.id || a.id.startsWith("builtin:")) a.id = `u-${++M.counter}`;
  if (allAnims().some((x) => x.id !== a.id && x.name.toLowerCase() === name.toLowerCase())) fail("anim_exists", name);
  const prev = M.animations.find((x) => x.id === a.id);
  const out: Animation = { ...clone(a), name, builtin: false, created: prev?.created ?? Date.now(), updated: Math.max(Date.now(), (prev?.updated ?? 0) + 1) };
  M.animations = prev ? M.animations.map((x) => (x.id === a.id ? out : x)) : [...M.animations, out];
  return clone(out);
}

function lookup(key: string): Animation {
  const k = key.trim();
  const all = allAnims();
  const hit = all.find((a) => a.id === k) ?? all.find((a) => a.name === k) ?? all.find((a) => a.name.toLowerCase() === k.toLowerCase()) ?? all.find((a) => a.id.replace("builtin:", "") === k.toLowerCase());
  return hit ?? fail("anim_not_found", k);
}

function trigger(spec: Record<string, unknown>) {
  const anim = lookup(String(spec.animation ?? ""));
  const devs = devices.filter((d) => M.scene.devices[d.key]?.mode === "effects" && (!spec.device || d.name.toLowerCase().includes(String(spec.device).toLowerCase()) || d.key === spec.device));
  if (!devs.length) fail("trigger_no_device");
  const secs = typeof spec.duration === "number" ? spec.duration : 3;
  const id = ++M.counter;
  M.overlays.push({ id, anim, device: (spec.device as string) ?? null, keys: (spec.keys as string[]) ?? [], start: Date.now(), until: Date.now() + secs * 1000 });
  M.triggers.push({ ...spec, at: Date.now() });
  return { overlay: id, devices: devs.length, duration: secs };
}

export async function invoke(cmd: string, args: Record<string, unknown> = {}): Promise<unknown> {
  switch (cmd) {
    case "connect": case "list_devices": case "rescan": case "resize_zone": return devices;
    case "get_scene": return M.scene;
    case "set_scene": M.scene = args.scene as Scene; M.setSceneCalls++; return null;
    case "set_visible_devices": M.visible = args.keys as string[]; return null;
    case "effect_defs": return DEFS;
    case "effect_defaults": return Object.fromEntries(DEFS.map((d) => [d.id, Object.fromEntries(d.params.map((p) => [p.key, p.default]))]));
    case "get_blackout": return false;
    case "get_settings": return { language: "en", start_hidden: false, fps_by_type: {}, default_profile: null, current_profile: null };
    case "list_profiles": return { profiles: [{ name: "Gaming", file: "gaming.json", openrgb: false }, { name: "OpenRGB: yado1", file: "o.json", openrgb: true }], corrupt: ["broken.json"] };
    case "profiles_path": return "~/.config/chroma-desk/profiles";
    case "get_autostart": return false;
    case "server_status": return { running: true, version: "mock", gui_open: false, service_installed: false, service_active: false };
    // animation library
    case "anim_list": return { animations: clone(allAnims()), corrupt: [] };
    case "anim_get": return clone(lookup(String(args.id)));
    case "anim_save": return saveAnim(clone(args.anim as Animation));
    case "anim_delete": {
      const id = String(args.id);
      if (id.startsWith("builtin:")) fail("anim_builtin", id);
      if (!M.animations.some((a) => a.id === id)) fail("anim_not_found", id);
      M.animations = M.animations.filter((a) => a.id !== id);
      return null;
    }
    case "anim_duplicate": {
      const c = clone(lookup(String(args.id)));
      c.id = "";
      return saveAnim({ ...c, name: String(args.name) });
    }
    case "anim_export": M.files[String(args.path)] = { ...clone(lookup(String(args.id))), builtin: false }; return null;
    case "anim_import": {
      const f = M.files[String(args.path)];
      if (!f) fail("io", "no such file");
      const c = clone(f);
      c.id = "";
      const base = c.name;
      for (let i = 2; allAnims().some((x) => x.name.toLowerCase() === c.name.toLowerCase()); i++) c.name = `${base} (${i})`;
      return saveAnim(c);
    }
    case "trigger": return trigger(args.spec as Record<string, unknown>);
    case "stop_triggers": { const n = M.overlays.length; M.overlays = []; return n; }
    case "trigger_socket_path": return "/run/user/1000/chroma-desk.sock";
    // native dialogs (tauri-plugin-dialog goes through invoke)
    case "plugin:dialog|open": { const p = M.nextOpen ?? "/mock/import.json"; M.nextOpen = null; return p; }
    case "plugin:dialog|save": { const p = M.nextSave ?? "/mock/export.json"; M.nextSave = null; return p; }
    default: return null;
  }
}

