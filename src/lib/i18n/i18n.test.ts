import { describe, expect, it } from "vitest";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import en from "./en.json";
import es from "./es.json";

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((f) => {
    const p = join(dir, f);
    if (statSync(p).isDirectory()) return sources(p);
    return /\.(svelte|ts)$/.test(f) && !f.endsWith(".test.ts") ? [p] : [];
  });
}

describe("i18n dictionaries", () => {
  it("es has exactly the keys of en", () => {
    expect(Object.keys(es).sort()).toEqual(Object.keys(en).sort());
  });
  it("every literal t('key') used in the sources exists in en", () => {
    const missing: string[] = [];
    for (const f of sources("src")) {
      const text = readFileSync(f, "utf8");
      for (const m of text.matchAll(/\bt\(\s*["']([\w.]+)["']/g)) {
        if (!(m[1] in en)) missing.push(`${f}: ${m[1]}`);
      }
    }
    expect(missing).toEqual([]);
  });
  it("placeholders match between languages", () => {
    const ph = (s: string) => (s.match(/\{\w+\}/g) ?? []).sort().join();
    for (const k of Object.keys(en) as (keyof typeof en)[]) expect(ph(es[k]), k).toBe(ph(en[k]));
  });
  it("dynamic keys (effects, params, error codes) are translated", () => {
    const rs = (d: string): string[] => readdirSync(d).flatMap((f) => (statSync(join(d, f)).isDirectory() ? rs(join(d, f)) : f.endsWith(".rs") ? [readFileSync(join(d, f), "utf8")] : []));
    const src = rs("src-tauri/src").join("\n");
    const missing: string[] = [];
    const need = (k: string) => { if (!(k in en)) missing.push(k); };
    for (const m of src.matchAll(/EffectDef \{ id: "(\w+)"/g)) need(`fx.${m[1]}`);
    for (const m of src.matchAll(/\b(?:slider|angle|color|colors|flag)\("(\w+)"/g)) need(`param.${m[1]}`);
    for (const m of src.matchAll(/ApiError::new\("(\w+)"/g)) need(`err.${m[1]}`);
    for (const c of ["sdk", "io", "no_connection"]) need(`err.${c}`);
    expect(missing).toEqual([]);
  });
});
