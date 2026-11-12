/**
 * Minimal reactive i18n. Dictionaries are flat JSON files (`en.json`, `es.json`...):
 * adding a language = adding a JSON file and registering it in `DICTS`.
 */
import en from "./en.json";
import es from "./es.json";

type Dict = Record<string, string>;

const DICTS: Record<string, { name: string; dict: Dict }> = {
  en: { name: "English", dict: en },
  es: { name: "Español", dict: es },
};

export const LANGUAGES = Object.entries(DICTS).map(([code, v]) => ({ code, name: v.name }));

const STORE_KEY = "chroma-desk.lang";

function detect(): string {
  try {
    const saved = localStorage.getItem(STORE_KEY);
    if (saved && saved in DICTS) return saved;
  } catch {
    /* storage unavailable */
  }
  const sys = (typeof navigator !== "undefined" ? navigator.language : "en").slice(0, 2).toLowerCase();
  return sys in DICTS ? sys : "en";
}

let locale = $state(detect());

export const getLocale = () => locale;

export function setLocale(code: string) {
  if (!(code in DICTS)) return;
  locale = code;
  try {
    localStorage.setItem(STORE_KEY, code);
  } catch {
    /* storage unavailable */
  }
}

/** Translates `key`, interpolating `{name}` placeholders; falls back to English, then to the key. */
export function t(key: string, params: Record<string, string | number> = {}): string {
  const raw = DICTS[locale]?.dict[key] ?? (en as Dict)[key] ?? key;
  return raw.replace(/\{(\w+)\}/g, (_, k: string) => String(params[k] ?? `{${k}}`));
}

/** Human readable text for anything thrown by the backend (`{code, detail}`) or the JS runtime. */
export function errText(e: unknown): string {
  if (e && typeof e === "object" && "code" in e) {
    const { code, detail } = e as { code: string; detail?: string };
    const key = `err.${code}`;
    const base = key in (en as Dict) ? t(key, { detail: detail ?? "" }) : code;
    return detail && !base.includes(detail) ? `${base} (${detail})` : base;
  }
  return String(e);
}

/** Translated device type name (`type.Keyboard` ...), raw type if unknown. */
export function deviceLabel(type: string): string {
  const k = `type.${type}`;
  return k in (en as Dict) ? t(k) : type;
}
