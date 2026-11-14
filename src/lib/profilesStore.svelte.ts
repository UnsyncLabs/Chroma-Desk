/** Profiles and persistent settings store. */
import { api, type Profile, type ProfileList, type Settings, type Scene } from "./api";
import { setLocale } from "./i18n/index.svelte";
import { flush, markClean, setScene } from "./scene.svelte";
import { toastError } from "./state.svelte";

export const profiles = $state({
  list: { profiles: [], corrupt: [] } as ProfileList,
  path: "",
  settings: { language: null, start_hidden: false, default_profile: null, current_profile: null, fps_by_type: {}, show_phantom_devices: false, builtin_enabled: {}, theme_rules: [], global_notifications: {}, desktop_notifications: false } as Settings,
});

/** The active profile's full document (theme, presets, notifications); replaced wholesale on reload. */
class ActiveProfile {
  value = $state.raw<Profile | null>(null);
}
export const activeProfile = new ActiveProfile();

/** Re-reads the active profile (after switching or editing it). */
export async function refreshActiveProfile() {
  const name = profiles.settings.current_profile;
  if (!name) {
    activeProfile.value = null;
    return;
  }
  try {
    activeProfile.value = await api.profileGet(name);
  } catch {
    activeProfile.value = null;
  }
}

export async function loadProfiles() {
  try {
    const [list, settings, path] = await Promise.all([api.listProfiles(), api.getSettings(), api.profilesPath()]);
    profiles.list = list;
    profiles.settings = settings;
    profiles.path = path;
    if (settings.language) setLocale(settings.language);
    await refreshActiveProfile();
  } catch (e) {
    toastError(e);
  }
}

/** Persists `profiles.settings` (and pushes fps caps to the engine). */
export async function saveSettings() {
  try {
    await api.setSettings($state.snapshot(profiles.settings) as Settings);
  } catch (e) {
    toastError(e);
  }
}

/** Saves the live scene as profile `name` and makes it the current one. */
export async function saveProfileAs(name: string) {
  await flush();
  const info = await api.saveProfile(name);
  profiles.settings.current_profile = info.name;
  markClean();
  await loadProfiles();
  return info;
}

/** Applies a profile and adopts its scene in the UI. */
export async function applyProfile(name: string): Promise<Scene> {
  const scene = await api.applyProfile(name);
  setScene(scene);
  markClean();
  profiles.settings.current_profile = name;
  await loadProfiles();
  return scene;
}
