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
});
