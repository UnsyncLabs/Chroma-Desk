/** Automations (notifications and monitors): named triggers of the active profile plus the global ones, and what the UI does with them. */
import { api, type Animation, type AutomationTemplate, type NamedTrigger, type Profile, type ProfileTheme } from "./api";
import { metricLabel, unitOf } from "./sysmon.svelte";
import { activeProfile, profiles, refreshActiveProfile, saveSettings } from "./profilesStore.svelte";
import { t } from "./i18n/index.svelte";

/** A deep plain copy (never structuredClone a `$state` proxy). */
export const plain = <T,>(v: T): T => JSON.parse(JSON.stringify($state.snapshot(v)));

export type Scope = "profile" | "global";
export interface NotifItem {
  name: string;
  scope: Scope;
  n: NamedTrigger;
}

/** Everything in effect: the profile's own notifications override global ones of the same name. */
export function listNotifications(): NotifItem[] {
  const own = activeProfile.value?.triggers ?? {};
  const global = profiles.settings.global_notifications ?? {};
  const out: NotifItem[] = Object.entries(global).filter(([k]) => !(k in own)).map(([name, n]) => ({ name, scope: "global" as Scope, n }));
  out.push(...Object.entries(own).map(([name, n]) => ({ name, scope: "profile" as Scope, n })));
  return out.sort((a, b) => a.name.localeCompare(b.name));
}

/** The active profile can hold notifications of its own (a saved, non-built-in profile). */
export const canEditProfile = () => !!activeProfile.value && !activeProfile.value.builtin;

export const validName = (n: string) => /^[a-z0-9_-]{1,32}$/.test(n);
export const slugName = (s: string) => s.toLowerCase().trim().replace(/[^a-z0-9_-]+/g, "_").replace(/^_+|_+$/g, "").slice(0, 32);

async function writeProfile(mut: (m: Record<string, NamedTrigger>) => void) {
  const p = activeProfile.value as Profile;
  const triggers = plain(p.triggers ?? {}) as Record<string, NamedTrigger>;
  mut(triggers);
  await api.profileUpdate(p.name, $state.snapshot(p.theme) as ProfileTheme, $state.snapshot(p.presets ?? []) as never, triggers);
  await refreshActiveProfile();
}

function writeGlobal(mut: (m: Record<string, NamedTrigger>) => void) {
  const m = plain(profiles.settings.global_notifications ?? {}) as Record<string, NamedTrigger>;
  mut(m);
  profiles.settings.global_notifications = m;
  return saveSettings();
}

/** Saves (creating or replacing) a notification; `old` is its previous name/scope when editing. */
export async function saveNotification(item: NotifItem, old?: { name: string; scope: Scope }) {
  const n = plain(item.n) as NamedTrigger;
  // Write the new one first and only then drop the old one: a failure never loses the original.
  if (item.scope === "profile") await writeProfile((m) => void (m[item.name] = n));
  else await writeGlobal((m) => void (m[item.name] = n));
  if (old && (old.name !== item.name || old.scope !== item.scope)) await removeFrom(old.scope, old.name);
}

/** A notification with this name already exists in that scope (a profile's own can shadow a global one, that is fine). */
export function nameTaken(scope: Scope, name: string): boolean {
  const map = scope === "profile" ? (activeProfile.value?.triggers ?? {}) : (profiles.settings.global_notifications ?? {});
  return name in map;
}

async function removeFrom(scope: Scope, name: string) {
  if (scope === "profile") await writeProfile((m) => void delete m[name]);
  else await writeGlobal((m) => void delete m[name]);
}
export const deleteNotification = (item: NotifItem) => removeFrom(item.scope, item.name);

export async function setEnabled(item: NotifItem, on: boolean) {
  await saveNotification({ ...item, n: { ...item.n, enabled: on } });
}

// ---------- monitors: display mode (always / periodic) and metrics ----------

/** The sequence has at least one meter step: this automation is a system monitor. */
export const hasMeter = (seq: Animation | null | undefined): boolean => seq?.kind.type === "sequence" && seq.kind.steps.some((s) => s.effect === "meter");

/** Metrics a monitor's meters show, in step order. */
export const meterMetrics = (seq: Animation | null | undefined): string[] => (seq?.kind.type === "sequence" ? seq.kind.steps.filter((s) => s.effect === "meter").map((s) => String(s.params.metric ?? "")) : []);

/** "Always" and "Every N for M" exclude each other: choosing one drops the other. */
export function setDisplayMode(n: NamedTrigger, mode: "always" | "periodic") {
  const so = { ...(n.sources ?? {}) };
  if (mode === "always") {
    so.always = true;
    so.every = null;
  } else {
    so.always = false;
    so.every = so.every ?? { every_secs: 10, for_secs: 7 };
  }
  n.sources = so;
}

/** An `Every` rule must show the overlay for no longer than its period. */
export const timingInvalid = (n: NamedTrigger): boolean => !!n.sources?.every && (!(n.sources.every.every_secs >= 1) || !(n.sources.every.for_secs > 0) || n.sources.every.for_secs > n.sources.every.every_secs);

/** "GPU memory + GPU usage" for meters, "Blink · Orbit · Off" for other steps: what a sequence shows. */
export function sequenceSummary(seq: Animation): string {
  if (seq.kind.type !== "sequence") return "";
  const steps = seq.kind.steps;
  const meters = steps.filter((s) => s.effect === "meter").map((s) => metricLabel(String(s.params.metric ?? "")));
  if (meters.length) return `${[...new Set(meters)].join(" + ")} · ${t("auto.keysCount", { n: Math.max(...steps.map((s) => s.keys?.length ?? 0)) })}`;
  const names = steps.map((s) => (t(`seq.style.${s.effect}`) !== `seq.style.${s.effect}` ? t(`seq.style.${s.effect}`) : s.effect));
  return [...new Set(names)].join(" · ");
}

// ---------- categories and the "When" chip ----------

export type Category = "monitor" | "timer" | "notification";

/** Monitors run all the time or on a measurement, timers on a clock, everything else reacts to an event. */
export function categoryOf(n: NamedTrigger): Category {
  if (n.sources?.always || n.sources?.threshold) return "monitor";
  // A meter that shows up periodically is still a monitor, not a generic timer.
  if (n.sources?.every && hasMeter(n.sequence)) return "monitor";
  if (n.sources?.every) return "timer";
  return "notification";
}

export function fmtEvery(secs: number): string {
  if (secs >= 3600 && secs % 3600 === 0) return t("auto.unit.hours", { n: secs / 3600 });
  if (secs >= 60 && secs % 60 === 0) return t("auto.unit.minutes", { n: secs / 60 });
  return t("auto.unit.seconds", { n: secs });
}

/** Short summaries of what fires an automation: "Always", "Every 5 min", "CPU temperature > 80 °C", "Discord"... */
export function whenChips(n: NamedTrigger): string[] {
  const s = n.sources ?? {};
  const out: string[] = [];
  if (s.always) out.push(t("auto.when.always"));
  if (s.every) out.push(t("auto.when.everyChip", { every: fmtEvery(s.every.every_secs), for: fmtEvery(s.every.for_secs) }));
  if (s.threshold) out.push(`${metricLabel(s.threshold.metric)} ${s.threshold.op === "lt" ? "<" : ">"} ${s.threshold.value} ${unitOf(s.threshold.metric)}${s.threshold.for_secs ? ` · ${fmtEvery(s.threshold.for_secs)}` : ""}`);
  if (s.desktop) out.push(s.desktop.app ? t("auto.when.desktopApp", { app: s.desktop.app }) : t("auto.when.desktop"));
  for (const e of s.omarchy ?? []) out.push(t(`notif.ev.${e.replace("omarchy:", "")}`));
  if (s.ai) out.push(s.ai.from ? t("auto.when.aiFrom", { from: s.ai.from }) : t("auto.when.ai"));
  if (!out.length) out.push(t("auto.when.command"));
  return out;
}

// ---------- templates ----------

export interface Template {
  id: string;
  category: "monitor" | "notification";
  n: NamedTrigger;
}

/** Simple one-click notifications built from library animations. */
const SIMPLE: Template[] = [
  { id: "flash_all", category: "notification", n: { anim: "builtin:flash", color: "@info", duration: 1.5 } },
  { id: "alert_wasd", category: "notification", n: { anim: "builtin:blink", color: "@alert", keys: ["W", "A", "S", "D"], repeat: 5, speed: 1.5 } },
  { id: "success_pulse", category: "notification", n: { anim: "builtin:pulse", color: "@ok", duration: 1.5 } },
  { id: "low_battery", category: "notification", n: { anim: "builtin:blink", color: "@warn", duration: 6, speed: 0.6, sources: { omarchy: ["omarchy:battery-low"] } } },
  { id: "desktop", category: "notification", n: { anim: "builtin:flash", color: "@accent", duration: 1.2, sources: { desktop: { app: "", urgency: null, contains: "" } } } },
  { id: "low_health", category: "notification", n: { anim: "builtin:blink", color: "@alert", keys: ["W", "A", "S", "D"], duration: 4, replace: true, priority: 5 } },
];

/** Name suggested for a copy of a template. */
export const templateName = (id: string) => ({ flash_all: "flash_all", success_pulse: "success", desktop: "new_message" })[id] ?? id;

export const tplStore = $state({ built: [] as AutomationTemplate[], loaded: false });

export async function loadTemplates() {
  try {
    tplStore.built = await api.automationTemplates();
  } catch {
    tplStore.built = [];
  }
  tplStore.loaded = true;
}

/** Every template: the built-in monitors and chat notifications first, then the simple ones. */
export const allTemplates = (): Template[] => [...tplStore.built.map((b) => ({ id: b.id, category: b.category, n: b.notification })), ...SIMPLE];
export const TEMPLATES = SIMPLE;

// ---------- copy-ready snippets ----------

export interface Snippets {
  cli: string;
  socket: string;
  bash: string;
  python: string;
  hypr: string;
}

export function snippets(name: string, socketPath: string | null): Snippets {
  const sock = socketPath ?? "$XDG_RUNTIME_DIR/chroma-desk.sock";
  const line = JSON.stringify({ cmd: "trigger", animation: name });
  return {
    cli: `chroma-desk trigger ${name}`,
    socket: `echo '${line}' | socat - UNIX-CONNECT:${sock}`,
    bash: `#!/usr/bin/env bash\n# Fire the "${name}" notification from any script.\nchroma-desk trigger ${name} || echo "chroma-desk is not running"\n# Override on the fly: --keys W,A,S,D --duration 2s --color '@alert'`,
    python: `import json, os, socket\n\ndef notify(name):\n    path = os.path.join(os.environ["XDG_RUNTIME_DIR"], "chroma-desk.sock")\n    with socket.socket(socket.AF_UNIX) as s:\n        s.connect(path)\n        s.sendall((json.dumps({"cmd": "trigger", "animation": name}) + "\\n").encode())\n        return json.loads(s.makefile().readline())\n\nprint(notify("${name}"))`,
    hypr: `-- ~/.config/hypr/bindings.lua (Omarchy)\no.bind("SUPER + ALT + N", "Notify: ${name}", "chroma-desk trigger ${name}")`,
  };
}

export const describeKeys = (keys: string[] | undefined) => (keys?.length ? keys.join(" ") : t("notif.wholeDevice"));
