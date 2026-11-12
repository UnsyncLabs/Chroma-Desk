<script lang="ts">
  /** Parameter editor generated from the effect schema (sliders, colours, angle, switches). */
  import { hex, fromHex, type EffectDef, type ParamValue, type Rgb } from "./api";
  import { t } from "./i18n/index.svelte";

  let { def, params, oncommit }: { def: EffectDef; params: Record<string, ParamValue>; oncommit: () => void } = $props();

  const num = (k: string, fallback: number) => (typeof params[k] === "number" ? (params[k] as number) : fallback);
  const colorList = (k: string): Rgb[] => (Array.isArray(params[k]) ? (params[k] as Rgb[]) : []);

  function setNum(k: string, e: Event) {
    params[k] = Number((e.currentTarget as HTMLInputElement).value);
    oncommit();
  }
  function setColor(k: string, v: string) {
    params[k] = fromHex(v);
    oncommit();
  }
  function setListColor(k: string, i: number, v: string) {
    const list = colorList(k).map((c) => [...c] as Rgb);
    list[i] = fromHex(v);
    params[k] = list;
    oncommit();
  }
  function addColor(k: string) {
    params[k] = [...colorList(k), [255, 255, 255] as Rgb];
    oncommit();
  }
  function removeColor(k: string) {
    params[k] = colorList(k).slice(0, -1);
    oncommit();
  }
</script>

{#each def.params as p (p.key)}
  {#if p.kind === "slider" || p.kind === "angle"}
    <label class="row">
      <span>{t(`param.${p.key}`)}</span>
      <input type="range" min={p.min} max={p.max} step={p.step} value={num(p.key, p.default as number)} oninput={(e) => setNum(p.key, e)} />
      <output>{Number(num(p.key, p.default as number)).toFixed(p.step >= 1 ? 0 : 2)}{p.kind === "angle" ? "°" : ""}</output>
    </label>
  {:else if p.kind === "color"}
    <label class="row">
      <span>{t(`param.${p.key}`)}</span>
      <input type="color" value={hex((params[p.key] as Rgb) ?? (p.default as Rgb))} oninput={(e) => setColor(p.key, e.currentTarget.value)} />
    </label>
  {:else if p.kind === "colors"}
    <div class="row">
      <span>{t(`param.${p.key}`)}</span>
      <span class="colors">
        {#each colorList(p.key) as c, i (i)}
          <input type="color" value={hex(c)} aria-label={t("hw.colorN", { n: i + 1 })} oninput={(e) => setListColor(p.key, i, e.currentTarget.value)} />
        {/each}
        {#if colorList(p.key).length < p.max}<button onclick={() => addColor(p.key)} aria-label={t("hw.addColor")}>+</button>{/if}
        {#if colorList(p.key).length > p.min}<button onclick={() => removeColor(p.key)} aria-label={t("hw.removeColor")}>−</button>{/if}
      </span>
    </div>
  {:else if p.kind === "bool"}
    <label class="row">
      <span>{t(`param.${p.key}`)}</span>
      <input
        type="checkbox"
        checked={Boolean(params[p.key] ?? p.default)}
        onchange={(e) => {
          params[p.key] = e.currentTarget.checked;
          oncommit();
        }}
      />
    </label>
  {/if}
{/each}

<style>
  .row { display: grid; grid-template-columns: 6rem minmax(0, 1fr) 3rem; align-items: center; gap: 0.5rem; margin: 0.35rem 0; }
  .row > span:first-child { color: var(--muted); font-size: 0.9rem; }
  .row output { text-align: right; font-variant-numeric: tabular-nums; }
  .colors { display: flex; gap: 0.3rem; flex-wrap: wrap; align-items: center; grid-column: 2 / -1; }
  input[type="color"] { width: 2rem; height: 1.7rem; padding: 0; border: none; background: none; }
</style>
