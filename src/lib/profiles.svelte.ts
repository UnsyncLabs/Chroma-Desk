/** Profiles and persistent settings store. */
import { api, type ProfileList, type Settings, type Scene } from "./api";
import { setLocale } from "./i18n/index.svelte";
import { flush, markClean, setScene } from "./scene.svelte";
import { toastError } from "./state.svelte";

export const profiles = $state({
  list: { profiles: [], corrupt: [] } as ProfileList,
  path: "",
  settings: { language: null, start_hidden: false, default_profile: null, current_profile: null, fps_by_type: {} } as Settings,
});

export async function loadProfiles() {
  try {
    const [list, settings, path] = await Promise.all([api.listProfiles(), api.getSettings(), api.profilesPath()]);
    profiles.list = list;
    profiles.settings = settings;
    profiles.path = path;
    if (settings.language) setLocale(settings.language);
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
