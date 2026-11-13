/** Notifications: named triggers of the active profile plus the global ones, and what the UI does with them. */
import { api, type NamedTrigger, type Profile, type ProfileTheme } from "./api";
import { activeProfile, profiles, refreshActiveProfile, saveSettings } from "./profiles.svelte";
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
  if (old) await removeFrom(old.scope, old.name);
  if (item.scope === "profile") await writeProfile((m) => void (m[item.name] = n));
  else await writeGlobal((m) => void (m[item.name] = n));
}

async function removeFrom(scope: Scope, name: string) {
  if (scope === "profile") await writeProfile((m) => void delete m[name]);
  else await writeGlobal((m) => void delete m[name]);
}
export const deleteNotification = (item: NotifItem) => removeFrom(item.scope, item.name);

export async function setEnabled(item: NotifItem, on: boolean) {
  await saveNotification({ ...item, n: { ...item.n, enabled: on } });
}

// ---------- templates ----------

export interface Template {
  id: string;
  name: string;
  /** i18n key of the title and description. */
  n: NamedTrigger;
}

export const TEMPLATES: Template[] = [
  { id: "flash_all", name: "flash_all", n: { anim: "builtin:flash", color: "@info", duration: 1.5 } },
  { id: "alert_wasd", name: "alert_wasd", n: { anim: "builtin:blink", color: "@alert", keys: ["W", "A", "S", "D"], repeat: 5, speed: 1.5 } },
  { id: "success_pulse", name: "success", n: { anim: "builtin:pulse", color: "@ok", duration: 1.5 } },
  { id: "low_battery", name: "low_battery", n: { anim: "builtin:blink", color: "@warn", duration: 6, speed: 0.6, sources: { omarchy: ["omarchy:battery-low"] } } },
  { id: "desktop", name: "new_message", n: { anim: "builtin:flash", color: "@accent", duration: 1.2, sources: { desktop: { app: "", urgency: null, contains: "" } } } },
  { id: "low_health", name: "low_health", n: { anim: "builtin:blink", color: "@alert", keys: ["W", "A", "S", "D"], duration: 4, replace: true, priority: 5 } },
];

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
