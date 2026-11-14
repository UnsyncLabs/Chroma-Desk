/** Animation library store and the actions shared by the library page, Quick and the editors. */
import { api, type Animation, type Device } from "./api";
import { buildLayout } from "./layout/buildLayout";
import { app, copyText, toast, toastError } from "./state.svelte";
import { commit, ensureDev, newLayer } from "./scene.svelte";
import { t } from "./i18n/index.svelte";

export const lib = $state({
  list: [] as Animation[],
  corrupt: [] as string[],
  loaded: false,
  /** Device used for previews/compatibility in the library page ("" = automatic). */
  targetKey: "",
});

export async function loadAnimations() {
  try {
    const l = await api.animList();
    lib.list = l.animations;
    lib.corrupt = l.corrupt;
    lib.loaded = true;
  } catch (e) {
    toastError(e);
  }
}

/** Name shown to the user: built-in presets are translated. */
export function animName(a: Animation): string {
  if (!a.builtin) return a.name;
  const key = `anim.b.${a.id.replace(/^builtin:/, "")}`;
  const tr = t(key);
  return tr === key ? a.name : tr;
}

export const kindLabel = (a: Animation) => t(`anim.kind.${a.kind.type}`);

/** Device used for previews: the chosen one, else the first keyboard, else the first device. */
export function previewDevice(): Device | null {
  return app.devices.find((d) => d.key === lib.targetKey) ?? app.devices.find((d) => buildLayout(d).kind === "keyboard") ?? app.devices[0] ?? null;
}

/** Puts `anim` on the given devices (replacing their layers) and switches them to Effects mode. */
export function applyAnimation(keys: string[], anim: Animation) {
  const snap = $state.snapshot(anim) as Animation;
  for (const k of keys) {
    const d = ensureDev(k);
    d.mode = "effects";
    d.layers = [newLayer("animation", { params: { anim_id: snap.id, snapshot: snap as unknown as Record<string, unknown> } })];
  }
  commit();
}

/** Plays the animation over the scene for a few seconds (the "Test" button). */
export async function testAnimation(a: Animation, deviceKey?: string) {
  try {
    const r = await api.trigger({ animation: a.id, device: deviceKey, duration: 3 });
    toast(t("anim.tested", { n: r.devices }), "ok", 2500);
  } catch (e) {
    toastError(e);
  }
}

export function triggerCommand(a: Animation): string {
  const name = a.name.includes('"') ? a.id : a.name;
  return `chroma-desk trigger "${name}"`;
}

export async function copyTriggerCommand(a: Animation) {
  const cmd = triggerCommand(a);
  await copyText(cmd, t("anim.copied", { cmd }));
}
