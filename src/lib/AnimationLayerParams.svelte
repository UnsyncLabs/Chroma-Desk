<script lang="ts">
  /** Parameters of an "animation" layer: pick from the library, or refresh the layer's saved copy. */
  import type { Animation, Layer } from "./api";
  import { animName, lib } from "./animations.svelte";
  import { t } from "./i18n/index.svelte";

  let { layer, oncommit }: { layer: Layer; oncommit: () => void } = $props();

  const snap = $derived(layer.params.snapshot as unknown as Animation | null);
  const id = $derived((layer.params.anim_id as string) ?? "");
  const live = $derived(lib.list.find((a) => a.id === id) ?? null);
  /** The library entry changed after this layer took its copy. */
  const outdated = $derived(!!live && !!snap && live.updated > snap.updated);
  const missing = $derived(!!snap && !live && lib.loaded);

  function use(a: Animation) {
    layer.params.anim_id = a.id;
    layer.params.snapshot = $state.snapshot(a) as unknown as Record<string, unknown>;
    oncommit();
  }

  function choose(value: string) {
    const a = lib.list.find((x) => x.id === value);
    if (a) use(a);
  }
</script>

<div class="row">
  <span>{t("layers.animation")}</span>
  <select value={live ? id : ""} onchange={(e) => choose(e.currentTarget.value)} aria-label={t("layers.animation")}>
    {#if !live}<option value="">{snap ? animName(snap) : t("layers.pickAnimation")}</option>{/if}
    <optgroup label={t("quick.presets")}>
      {#each lib.list.filter((a) => a.builtin) as a (a.id)}<option value={a.id}>{animName(a)}</option>{/each}
    </optgroup>
    {#if lib.list.some((a) => !a.builtin)}
      <optgroup label={t("quick.mine")}>
        {#each lib.list.filter((a) => !a.builtin) as a (a.id)}<option value={a.id}>{a.name}</option>{/each}
      </optgroup>
    {/if}
  </select>
</div>
{#if outdated && live}
  <p class="note">
    {t("layers.animOutdated")}
    <button onclick={() => use(live)}>{t("layers.animUpdate")}</button>
  </p>
{:else if missing}
  <p class="note muted">{t("layers.animMissing")}</p>
{/if}

<style>
  .row { display: grid; grid-template-columns: 6rem minmax(0, 1fr); align-items: center; gap: 0.5rem; margin: 0.35rem 0; }
  .row > span { color: var(--muted); font-size: 0.9rem; }
  .row select { min-width: 0; }
  .note { margin: 0.3rem 0; font-size: 0.85rem; display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; }
</style>
