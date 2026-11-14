<script lang="ts">
  /** A library entry: animated thumbnail, name, kind, compatibility and tags. */
  import type { Snippet } from "svelte";
  import type { Animation } from "./api";
  import type { Layout } from "./layout/buildLayout";
  import { compat } from "./anim/ref";
  import { animName, kindLabel } from "./animationsStore.svelte";
  import { t } from "./i18n/index.svelte";
  import AnimThumb from "./AnimThumb.svelte";

  let {
    anim,
    layout,
    names,
    active = false,
    onpick,
    pickLabel = "",
    children,
  }: {
    anim: Animation;
    layout: Layout;
    names: string[];
    /** Currently applied on the device. */
    active?: boolean;
    /** Makes the thumbnail a button (Quick). */
    onpick?: () => void;
    pickLabel?: string;
    /** Footer actions. */
    children?: Snippet;
  } = $props();

  const c = $derived(compat(anim, names));
  const name = $derived(animName(anim));
</script>

<article class="card" class:active data-anim={anim.id}>
  {#if onpick}
    <button class="face" onclick={onpick} aria-label={pickLabel || name} aria-pressed={active}>
      <AnimThumb {anim} {layout} {names} />
    </button>
  {:else}
    <div class="face"><AnimThumb {anim} {layout} {names} /></div>
  {/if}
  <div class="info">
    <b class="name" title={name}>{name}</b>
    <span class="meta">
      <span class="chip">{kindLabel(anim)}</span>
      {#if anim.builtin}<span class="chip builtin">{t("anim.builtin")}</span>{/if}
      {#if c.total > 0}
        <span class="chip" class:warn={c.ok < c.total} title={t("anim.compatHint")}>{t("anim.compat", { ok: c.ok, total: c.total })}</span>
      {/if}
      {#each anim.tags.slice(0, 3) as tag (tag)}<span class="chip tag">#{tag}</span>{/each}
    </span>
  </div>
  {@render children?.()}
</article>

<style>
  .card { background: var(--panel); border: 1px solid var(--border); border-radius: 12px; padding: 0.55rem; display: flex; flex-direction: column; gap: 0.5rem; min-width: 0; position: relative; }
  .card.active { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent) inset; }
  .face { display: block; width: 100%; padding: 0; background: none; border: 1px solid transparent; border-radius: 9px; cursor: default; }
  button.face { cursor: pointer; }
  button.face:hover:not(:disabled), button.face:focus-visible { border-color: var(--accent); }
  .info { display: flex; flex-direction: column; gap: 0.3rem; min-width: 0; }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { display: flex; flex-wrap: wrap; gap: 0.25rem; }
  .chip { font-size: 0.72rem; border: 1px solid var(--border); border-radius: 999px; padding: 0.03rem 0.5rem; color: var(--muted); white-space: nowrap; }
  .chip.builtin { color: var(--accent); border-color: color-mix(in srgb, var(--accent) 50%, transparent); }
  .chip.warn { color: var(--danger); border-color: color-mix(in srgb, var(--danger) 60%, transparent); }
  .chip.tag { color: var(--muted); }
</style>
