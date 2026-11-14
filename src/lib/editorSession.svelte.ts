/**
 * What the three animation editors share: the live preview on the hardware, the unsaved-changes
 * guard, saving to the library and applying to the device.
 */
import { api, type Animation, type Device } from "./api";
import { applyAnimation, loadAnimations } from "./animationsStore.svelte";
import { t } from "./i18n/index.svelte";
import { buildLayout } from "./layout/buildLayout";
import { commit, devScene, emptyDevice, newLayer, previewHook, sceneStore } from "./scene.svelte";
import { leaveGuard, toast, toastError } from "./state.svelte";

let tick = Date.now();
/** Strictly increasing `updated` for drafts so the engine's per-animation cache notices every edit. */
export const nextUpdated = () => (tick = Math.max(tick + 1, Date.now()));

export class EditorSession {
  /** Id of the library entry being edited ("" = not saved yet). */
  savedId = $state("");
  savedName = $state("");
  dirty = $state(false);
  live = $state(true);
  busy = $state(false);
  readonly names: string[];
  readonly layoutKind: string;

  #device: Device;
  #build: () => Animation;
  #guard: () => boolean;

  constructor(device: Device, build: () => Animation, initial: Animation | null) {
    this.#device = device;
    this.#build = build;
    this.savedId = initial && !initial.builtin ? initial.id : "";
    this.savedName = initial && !initial.builtin ? initial.name : "";
    this.names = Array.from({ length: device.colors.length }, (_, i) => device.led_names[i] ?? "");
    this.layoutKind = buildLayout(device).kind;
    this.#guard = () => this.dirty;
  }

  get target(): Animation["target"] {
    const kind = this.layoutKind === "keyboard" ? "keyboard" : this.layoutKind === "linear" ? "linear" : "any";
    return { kind, hint: this.#device.name };
  }

  /** Call when the editor appears. */
  start() {
    leaveGuard.dirty = this.#guard;
    this.#syncHook();
    commit();
  }

  /** Call when the editor goes away: stops the preview and restores the device's real scene. */
  stop() {
    if (leaveGuard.dirty === this.#guard) leaveGuard.dirty = null;
    previewHook.fn = null;
    commit();
  }

  /** Something changed: mark unsaved and refresh the hardware preview. */
  touch() {
    this.dirty = true;
    this.#syncHook();
    commit();
  }

  setLive(on: boolean) {
    this.live = on;
    this.#syncHook();
    commit();
  }

  #syncHook() {
    if (!this.live) {
      previewHook.fn = null;
      return;
    }
    const key = this.#device.key;
    previewHook.fn = (scene) => {
      const base = scene.devices[key] ?? emptyDevice();
      const anim = this.#build();
      scene.devices[key] = { ...base, mode: "effects", layers: [{ ...newLayerShell(), params: { anim_id: "draft", snapshot: anim as unknown as Record<string, unknown> } }] };
      return scene;
    };
  }

  /** Saves to the library: overwrite the entry being edited, or create a new one. */
  async save(name: string, tags: string[], asNew: boolean) {
    this.busy = true;
    try {
      const draft = this.#build();
      const overwrite = !asNew && this.savedId;
      const saved = await api.animSave({ ...draft, id: overwrite ? this.savedId : "", name, tags, created: 0, updated: 0, target: this.target });
      this.savedId = saved.id;
      this.savedName = saved.name;
      this.dirty = false;
      await loadAnimations();
      toast(t("save.saved", { name: saved.name }), "ok");
      return saved;
    } catch (e) {
      toastError(e);
      return null;
    } finally {
      this.busy = false;
    }
  }

  /** Puts the draft on this device as its (only) animation layer. */
  apply() {
    const anim = this.#build();
    applyAnimation([this.#device.key], { ...anim, id: this.savedId || `draft-${nextUpdated()}`, name: anim.name || this.savedName || t("save.untitled") });
    toast(t("draw.applied"), "ok", 2000);
  }
}

/** A bare animation layer (id/opacity/blend defaults); the preview fills in `params`. */
function newLayerShell() {
  const l = newLayer("animation");
  return { ...l, id: "preview" };
}

export { devScene, sceneStore };
