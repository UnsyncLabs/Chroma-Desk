/**
 * Mock of the theme/profile/notification half of the backend (mirrors themes/, builtin.rs,
 * profiles.rs and triggers.rs closely enough for the UI): state lives on the shared mock object `S`.
 */
import type { Animation, Color, Device, NamedTrigger, Profile, ProfileInfo, ProfileTheme, Rgb, Scene, Settings } from "../api";
import { DEFAULT_SLOTS, resolveColor, SLOTS, type PaletteEvent } from "../palette.svelte";
import builtinProfiles from "./builtin_profiles.json";

export interface ThemeMock {
  profiles: Record<string, Profile & { scene: Scene }>;
  settings: Settings;
  theme: ProfileTheme;
  triggers: Record<string, NamedTrigger>;
  external: Record<string, Rgb>;
  runtimeSource: string | null;
  omarchyPresent: boolean;
  omarchyTheme: string;
  lastPalette: string;
  hook: boolean;
  eventHooks: Record<string, boolean>;
  desktopStatus: string;
  lastThemeName: string;
  hookCalls: string[];
  paletteEpoch: number;
}

const hx = (s: string): Rgb => [parseInt(s.slice(1, 3), 16), parseInt(s.slice(3, 5), 16), parseInt(s.slice(5, 7), 16)];

/** colors.toml of the Omarchy themes the mock can switch between (copied from the real files). */
const OMARCHY: Record<string, Record<string, string>> = {
  "tokyo-night": { accent: "#7aa2f7", background: "#1a1b26", foreground: "#a9b1d6", red: "#f7768e", yellow: "#e0af68", orange: "#eb927b", green: "#9ece6a", cyan: "#449dab", blue: "#7aa2f7", magenta: "#ad8ee6" },
  catppuccin: { accent: "#89b4fa", background: "#1e1e2e", foreground: "#cdd6f4", red: "#f38ba8", yellow: "#f9e2af", orange: "#f6b6ab", green: "#a6e3a1", cyan: "#94e2d5", blue: "#89b4fa", magenta: "#f5c2e7" },
  gruvbox: { accent: "#7daea3", background: "#282828", foreground: "#d4be98", red: "#ea6962", yellow: "#d8a657", orange: "#e78a4e", green: "#a9b665", cyan: "#89b482", blue: "#7daea3", magenta: "#d3869b" },
};
export const OMARCHY_THEMES = Object.keys(OMARCHY);

function omarchyPalette(name: string): Record<string, Rgb> {
  const c = OMARCHY[name] ?? OMARCHY["tokyo-night"];
  const g = (k: string) => hx(c[k]);
  return {
    accent: g("accent"), background: g("background"), foreground: g("foreground"), alert: g("red"), warn: g("yellow"), ok: g("green"), info: g("cyan"), primary: g("blue"), secondary: g("magenta"), tertiary: g("cyan"),
    c1: g("red"), c2: g("orange"), c3: g("yellow"), c4: g("green"), c5: g("cyan"), c6: g("blue"), c7: g("magenta"), c8: g("accent"),
  };
}

export const initialTheme = (): ThemeMock => ({
  profiles: {},
  settings: { language: "en", start_hidden: false, fps_by_type: {}, show_phantom_devices: false, default_profile: null, current_profile: null, builtin_enabled: {}, theme_rules: [], global_notifications: {}, desktop_notifications: false } as Settings,
  theme: { source: "manual" },
  triggers: {},
  external: {},
  runtimeSource: null,
  omarchyPresent: true,
  omarchyTheme: "tokyo-night",
  lastPalette: "",
  hook: false,
  eventHooks: {},
  desktopStatus: "off",
  lastThemeName: "tokyo-night",
  hookCalls: [],
  paletteEpoch: 0,
});

type Ctx = {
  S: ThemeMock;
  emit: (e: string, p: unknown) => void;
  devices: Device[];
  scene: () => Scene;
  setScene: (s: Scene) => void;
  fail: (code: string, detail?: string) => never;
  trigger: (spec: Record<string, unknown>) => unknown;
};
let C: Ctx;
export const bind = (c: Ctx) => (C = c);

const activeSource = () => (C.S.runtimeSource ?? C.S.theme.source) as string;

/** Palette of the active profile theme, like `ThemeCtl::recompute`. */
export function computePalette(): PaletteEvent {
  const S = C.S;
  const src = activeSource();
  let base: Record<string, Rgb> = { ...DEFAULT_SLOTS };
  let name = "Manual";
  if (src === "omarchy" && S.omarchyPresent) {
    base = omarchyPalette(S.omarchyTheme);
    name = S.omarchyTheme;
  } else if (S.theme.manual_palette) {
    base = { ...DEFAULT_SLOTS, ...S.theme.manual_palette };
    name = "Manual";
  }
  const slots = { ...base, ...(S.theme.overrides ?? {}), ...S.external };
  return { palette: { name: Object.keys(S.external).length ? `${name} + external` : name, source: src, slots, generation: ++S.paletteEpoch }, active: src, external: Object.keys(S.external).length > 0, error: null };
}

/** Recomputes and tells the UI when something changed. */
export function refreshPalette() {
  const ev = computePalette();
  const sig = JSON.stringify([ev.palette.name, ev.palette.slots, ev.active, ev.external]);
  if (sig !== C.S.lastPalette) {
    C.S.lastPalette = sig;
    C.emit("palette-changed", ev);
  }
  return ev;
}

// ---------- built-in profiles ----------

const builtins = () => (builtinProfiles as unknown as Profile[]).map((p) => ({ ...JSON.parse(JSON.stringify(p)) as Profile, builtin: true }));
const isBuiltin = (n: string) => builtins().some((b) => b.name.toLowerCase() === n.trim().toLowerCase());
const slot = (s: string, lighten?: number): Color => (lighten ? { slot: s, lighten } : { slot: s });

function layer(id: string, effect: string, params: Record<string, unknown>, opacity = 1, mask: { type: string; leds?: number[] } = { type: "all" }) {
  return { id, name: "", enabled: true, opacity, blend: "normal", mask, effect, params } as Scene["devices"][string]["layers"][number];
}

/** `builtin::scene_for` for the mock devices. */
export function builtinScene(name: string): Scene {
  const cyber = name === "Cyberpunk";
  const scene: Scene = { master_brightness: 1, sync: false, order: [], global_layers: [], devices: {} };
  for (const d of C.devices) {
    const keyLeds = d.led_names.flatMap((n, i) => (["W", "A", "S", "D", "Up Arrow", "Down Arrow", "Left Arrow", "Right Arrow"].some((k) => n === `Key: ${k}`) ? [i] : []));
    let layers;
    if (d.device_type === "Keyboard") {
      layers = cyber
        ? [layer("bg", "solid", { color: slot("tertiary", -0.78) }), layer("scan", "color_wave", { colors: [slot("primary"), slot("secondary"), slot("primary")], speed: 0.35, angle: 90, scale: 1.5 }, 0.8), layer("keys", "solid", { color: slot("accent") }, 1, { type: "leds", leds: keyLeds }), layer("glitch", "glitch", { colors: [slot("primary"), slot("secondary"), slot("accent")], interval: 6, duration: 0.25, density: 0.3, size: 0.14 })]
        : [layer("bg", "solid", { color: slot("background", 0.14) }), layer("wave", "color_wave", { colors: [slot("accent"), slot("secondary"), slot("accent")], speed: 0.12, angle: 0, scale: 1 }, 0.7), layer("keys", "solid", { color: slot("accent") }, 1, { type: "leds", leds: keyLeds })];
    } else if (["LEDStrip", "Cooler", "Light"].includes(d.device_type)) {
      layers = [layer("chase", "chase", { colors: cyber ? [slot("primary"), slot("secondary")] : [slot("primary")], speed: 0.5, length: 0.3, gap: 0.4 })];
    } else if (cyber && ["DRam", "Motherboard", "Gpu"].includes(d.device_type)) {
      layers = [layer("wave", "color_wave", { colors: [slot("primary"), slot("secondary"), slot("tertiary")], speed: 0.3, angle: 0, scale: 1 })];
    } else {
      layers = [layer("breath", "breathing", { colors: cyber ? [slot("accent"), slot("secondary")] : [slot("accent")], period: 5 })];
    }
    scene.devices[d.key] = { mode: "effects", layers, brightness: 1, fps: null, follow_global: true, fan_leds: {}, hardware: null };
  }
  return scene;
}

function stripOf(p: Profile): Rgb[] {
  if (p.theme.source !== "manual") return [];
  const m = { ...DEFAULT_SLOTS, ...(p.theme.manual_palette ?? {}) };
  return ["background", "accent", "primary", "secondary", "tertiary", "alert", "ok"].map((k) => m[k]);
}

const info = (p: Profile, file = ""): ProfileInfo => ({ name: p.name, file, openrgb: false, builtin: !!p.builtin, enabled: p.enabled !== false, requires: p.requires ?? null, source: p.theme.source, strip: stripOf(p), triggers: Object.keys(p.triggers ?? {}).length });

export function listProfiles() {
  const S = C.S;
  const b = builtins()
    .filter((p) => (p.requires === "omarchy" ? S.omarchyPresent : true))
    .map((p) => ({ ...p, enabled: S.settings.builtin_enabled[p.name] ?? p.enabled }));
  const user = Object.values(S.profiles).sort((a, b2) => a.name.localeCompare(b2.name));
  return { profiles: [...b.map((p) => info(p)), ...user.map((p) => info(p, `${p.name.toLowerCase()}.json`)), ...(S.profiles.__none ? [] : [])], corrupt: [] as string[] };
}

export function getProfile(name: string): Profile {
  const S = C.S;
  const b = builtins().find((x) => x.name.toLowerCase() === name.trim().toLowerCase());
  if (b) return { ...b, enabled: S.settings.builtin_enabled[b.name] ?? b.enabled };
  const p = S.profiles[name.trim()];
  if (!p) C.fail("profile_not_found", name);
  return JSON.parse(JSON.stringify(p));
}

function adopt(p: Profile) {
  C.S.theme = JSON.parse(JSON.stringify(p.theme));
  C.S.triggers = JSON.parse(JSON.stringify(p.triggers ?? {}));
  C.S.external = {};
  C.S.runtimeSource = null;
  refreshPalette();
}

export function applyProfile(name: string): Scene {
  const S = C.S;
  const p = getProfile(name);
  let scene: Scene;
  if (p.builtin) {
    if (p.requires === "omarchy" && !S.omarchyPresent) C.fail("profile_unavailable", p.name);
    if ((S.settings.builtin_enabled[p.name] ?? p.enabled !== false) === false) C.fail("profile_disabled", p.name);
    scene = builtinScene(p.name);
  } else scene = JSON.parse(JSON.stringify(S.profiles[p.name].scene));
  C.setScene(scene);
  adopt(p);
  S.settings.current_profile = p.name;
  C.emit("scene-changed", scene);
  return scene;
}

function checkName(n: string) {
  const name = n.trim();
  if (!name || name.length > 64) C.fail("profile_name_invalid", name);
  if (isBuiltin(name)) C.fail("profile_exists", name);
  return name;
}

export function saveProfile(nameIn: string) {
  const S = C.S;
  const name = checkName(nameIn);
  const prev = S.profiles[name];
  const p = { schema_version: 2, name, theme: prev?.theme ?? JSON.parse(JSON.stringify(S.theme)), presets: prev?.presets ?? [], triggers: prev?.triggers ?? JSON.parse(JSON.stringify(S.triggers)), enabled: true, scene: JSON.parse(JSON.stringify(C.scene())) } as Profile & { scene: Scene };
  S.profiles[name] = p;
  S.settings.current_profile = name;
  return info(p, `${name.toLowerCase()}.json`);
}

export function updateProfile(name: string, theme: ProfileTheme, presets: Profile["presets"], triggers: Record<string, NamedTrigger>) {
  const S = C.S;
  if (isBuiltin(name)) C.fail("profile_builtin", name);
  const p = S.profiles[name];
  if (!p) C.fail("profile_not_found", name);
  p.theme = JSON.parse(JSON.stringify(theme));
  p.presets = presets;
  p.triggers = JSON.parse(JSON.stringify(triggers));
  if (S.settings.current_profile === name) adopt(p);
}

export function duplicateProfile(name: string, to: string) {
  const S = C.S;
  const nn = checkName(to);
  if (S.profiles[nn]) C.fail("profile_exists", nn);
  const src = getProfile(name);
  const scene = src.builtin ? builtinScene(src.name) : JSON.parse(JSON.stringify(S.profiles[src.name].scene));
  const p = { ...JSON.parse(JSON.stringify(src)), name: nn, builtin: false, enabled: true, requires: null, scene } as Profile & { scene: Scene };
  S.profiles[nn] = p;
  return info(p, `${nn.toLowerCase()}.json`);
}

export function deleteProfile(name: string) {
  if (isBuiltin(name)) C.fail("profile_builtin", name);
  if (!C.S.profiles[name]) C.fail("profile_not_found", name);
  delete C.S.profiles[name];
  if (C.S.settings.current_profile === name) C.S.settings.current_profile = null;
}

export function renameProfile(old: string, to: string) {
  if (isBuiltin(old)) C.fail("profile_builtin", old);
  const nn = checkName(to);
  const p = C.S.profiles[old];
  if (!p) C.fail("profile_not_found", old);
  delete C.S.profiles[old];
  p.name = nn;
  C.S.profiles[nn] = p;
  if (C.S.settings.current_profile === old) C.S.settings.current_profile = nn;
  return info(p, `${nn}.json`);
}

// ---------- theme status, hooks, events ----------

export function themeStatus() {
  const S = C.S;
  const ev = computePalette();
  const sources = (["manual", "omarchy", "kde", "pywal", "gtk", "external"] as const).map((id) => ({
    id,
    detected: id === "manual" || id === "external" || (id === "omarchy" && S.omarchyPresent) || id === "gtk",
    name: id === "omarchy" && S.omarchyPresent ? S.omarchyTheme : id === "gtk" ? "GNOME blue" : null,
    error: null,
    strip: id === "omarchy" && S.omarchyPresent ? (() => { const p = omarchyPalette(S.omarchyTheme); return ["background", "accent", "primary", "secondary", "tertiary", "alert", "ok"].map((k) => p[k]); })() : [],
  }));
  return { active: ev.active, runtime_override: !!S.runtimeSource, external: ev.external, error: null, sources, palette: ev.palette };
}

export const hookStatus = () => ({ available: C.S.omarchyPresent, installed: C.S.hook, path: "~/.config/omarchy/hooks/theme-set.d/50-chroma-desk", script: "~/.config/chroma-desk/hooks/50-chroma-desk", command: "omarchy hook install theme-set ~/.config/chroma-desk/hooks/50-chroma-desk" });

const KINDS = ["battery-low", "theme-set", "post-update"];
export const eventHooks = () =>
  KINDS.map((kind) => ({
    kind,
    event: `omarchy:${kind}`,
    installed: !!C.S.eventHooks[kind],
    path: `~/.config/omarchy/hooks/${kind}.d/55-chroma-desk-${kind}`,
    script_text: `#!/bin/bash\n# chroma-desk: fire the notifications that listen to omarchy:${kind}.\nchroma-desk event omarchy:${kind} >/dev/null 2>&1 || true\n`,
    command: `omarchy hook install ${kind} ~/.config/chroma-desk/hooks/55-chroma-desk-${kind}`,
  }));

/** Simulates `omarchy theme set <name>`: the palette follows, theme rules and "theme-set" notifications fire. */
export function setOmarchyTheme(name: string) {
  const S = C.S;
  S.omarchyTheme = name;
  refreshPalette();
  if (S.lastThemeName !== name) {
    S.lastThemeName = name;
    const rule = S.settings.theme_rules.find((r) => r.theme.toLowerCase() === name.toLowerCase());
    if (rule && S.settings.current_profile !== rule.profile) {
      try {
        applyProfile(rule.profile);
        C.emit("profile-applied", { name: rule.profile, reason: "theme_rule", theme: name });
      } catch {
        /* rule target unavailable */
      }
    }
    if (S.eventHooks["theme-set"]) fireEvent("omarchy:theme-set");
  }
}

export function setPalette(slots: Record<string, Rgb>) {
  Object.assign(C.S.external, slots);
  refreshPalette();
}

// ---------- notifications ----------

export const notifications = (): Record<string, NamedTrigger> => ({ ...C.S.settings.global_notifications, ...C.S.triggers });

/** Fires a named notification like `triggers::start` (devices, keys, colour, duration). */
export function fireNamed(name: string, o: Record<string, unknown> = {}) {
  const n = notifications()[name];
  if (!n) return null;
  if (n.enabled === false) C.fail("notification_disabled", name);
  const spec: Record<string, unknown> = { animation: n.anim, keys: o.keys ?? n.keys ?? [], color: o.color ?? n.color, duration: o.duration ?? n.duration, repeat: n.repeat, speed: n.speed, priority: n.priority ?? 0, replace: n.replace, name };
  const targets: (string | null)[] = n.devices && n.devices.length ? n.devices : [n.device ?? null];
  let devices = 0;
  let err: unknown = null;
  for (const t of targets) {
    try {
      const r = C.trigger({ ...spec, device: t }) as { devices: number };
      devices += r.devices;
    } catch (e) {
      err = e;
      if (targets.length === 1) throw e;
    }
  }
  if (!devices) throw err ?? { code: "trigger_no_device", detail: "" };
  return { overlay: 0, devices, duration: 3 };
}

export function fireEvent(event: string) {
  let fired = 0;
  for (const [name, n] of Object.entries(notifications())) {
    if (n.enabled !== false && n.sources?.omarchy?.includes(event)) {
      try {
        fireNamed(name);
        fired++;
      } catch {
        /* nothing to show it on */
      }
    }
  }
  return fired;
}

export function fireDesktop(note: { app: string; urgency?: number; summary: string }) {
  if (!C.S.settings.desktop_notifications) return 0;
  let fired = 0;
  const has = (hay: string, needle: string) => !needle.trim() || hay.toLowerCase().includes(needle.trim().toLowerCase());
  for (const [name, n] of Object.entries(notifications())) {
    const m = n.sources?.desktop;
    if (n.enabled === false || !m) continue;
    const u = note.urgency ?? 1;
    const urgOk = !m.urgency || ({ low: 0, normal: 1, critical: 2 } as Record<string, number>)[m.urgency] === u;
    if (urgOk && has(note.app, m.app) && has(note.summary, m.contains)) {
      try {
        fireNamed(name);
        fired++;
      } catch {
        /* nothing to show it on */
      }
    }
  }
  return fired;
}

export { resolveColor, SLOTS };
export type { Animation };
