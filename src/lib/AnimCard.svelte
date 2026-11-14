<script lang="ts">
  /** A library entry: animated thumbnail, name, kind, compatibility and tags. */
  import type { Snippet } from "svelte";
  import type { Animation } from "./api";
  import type { Layout } from "./layout/buildLayout";
  import { compat } from "./anim/ref";
  import { animName, kindLabel } from "./animationsStore.svelte";
  import { animRole } from "./roles";
  import { t } from "./i18n/index.svelte";
  import AnimThumb from "./AnimThumb.svelte";

  let {
    anim,
    layout,
    names,
    active = false,
    onpick,
    onreplace,
    pickLabel = "",
    children,
  }: {
    anim: Animation;
    layout: Layout;
    names: string[];
    /** Currently applied on the device. */
    active?: boolean;
    /** Makes the thumbnail a button (Quick). */
    onpick?: (e: MouseEvent) => void;
    /** Secondary action: replace everything with this one (Quick). */
    onreplace?: () => void;
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
  {#if active}<span class="badge" aria-hidden="true">✓</span>{/if}
  <div class="info">
    <b class="name" title={name}>{name}</b>
    <span class="meta">
      <span class="chip">{kindLabel(anim)}</span>
      <span class="chip role" data-role={animRole(anim)} title={t(`quick.role${animRole(anim) === "theme" ? "Theme" : "Effect"}Hint`)}>{t(`anim.role.${animRole(anim)}`)}</span>
      {#if anim.builtin}<span class="chip builtin">{t("anim.builtin")}</span>{/if}
      {#if c.total > 0}
        <span class="chip" class:warn={c.ok < c.total} title={t("anim.compatHint")}>{t("anim.compat", { ok: c.ok, total: c.total })}</span>
      {/if}
      {#each anim.tags.slice(0, 3) as tag (tag)}<span class="chip tag">#{tag}</span>{/each}
    </span>
  </div>
  {#if onreplace}<button class="only" onclick={onreplace} title={t("quick.onlyHint")}>{t("quick.only")}</button>{/if}
  {@render children?.()}
</article>

<style>
  .card { background: var(--panel); border: 1px solid var(--border); border-radius: 12px; padding: 0.55rem; display: flex; flex-direction: column; gap: 0.5rem; min-width: 0; position: relative; }
  .card.active { border-color: var(--ok); box-shadow: 0 0 0 1px var(--ok) inset; }
  .badge { position: absolute; top: 0.9rem; right: 0.9rem; width: 1.4rem; height: 1.4rem; border-radius: 50%; background: var(--ok); color: var(--bg, #000); display: grid; place-items: center; font-size: 0.85rem; font-weight: 700; pointer-events: none; }
  .only { font-size: 0.75rem; padding: 0.15rem 0.5rem; align-self: flex-start; }
  .face { display: block; width: 100%; padding: 0; background: none; border: 1px solid transparent; border-radius: 9px; cursor: default; }
  button.face { cursor: pointer; }
  button.face:hover:not(:disabled), button.face:focus-visible { border-color: var(--accent); }
  .info { display: flex; flex-direction: column; gap: 0.3rem; min-width: 0; }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { display: flex; flex-wrap: wrap; gap: 0.25rem; }
  .chip { font-size: 0.72rem; border: 1px solid var(--border); border-radius: 999px; padding: 0.03rem 0.5rem; color: var(--muted); white-space: nowrap; }
  .chip.role { color: var(--text); }
  .chip.builtin { color: var(--accent); border-color: color-mix(in srgb, var(--accent) 50%, transparent); }
  .chip.warn { color: var(--danger); border-color: color-mix(in srgb, var(--danger) 60%, transparent); }
  .chip.tag { color: var(--muted); }
</style>
