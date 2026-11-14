/** What this platform supports (from the backend): the UI hides what is not available. */
import { api } from "./api";

export const caps = $state({ list: null as string[] | null });

export async function loadCapabilities() {
  try {
    caps.list = await api.capabilities();
  } catch {
    caps.list = null;
  }
}

/** True when `id` is available. Unknown (not loaded yet, or an older backend) counts as available. */
export function has(id: string): boolean {
  return caps.list === null || caps.list.includes(id);
}

/** Theme sources that need a Linux desktop component, by source id. */
export const SOURCE_CAP: Record<string, string> = { omarchy: "omarchy", kde: "kde", pywal: "pywal", gtk: "gnome" };
