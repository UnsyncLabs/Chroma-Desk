<script lang="ts">
  /** Footer shared by the editors: live preview switch, Apply, unsaved marker and Save as animation. */
  import type { EditorSession } from "./editorSession.svelte";
  import { t } from "./i18n/index.svelte";
  import SaveBar from "./SaveBar.svelte";

  let { session, empty = false, defaultName = "", defaultTags = [] }: { session: EditorSession; empty?: boolean; defaultName?: string; defaultTags?: string[] } = $props();
</script>

<div class="bar">
  <div class="top">
    <label class="live">
      <input type="checkbox" checked={session.live} onchange={(e) => session.setLive(e.currentTarget.checked)} />
      {t("draw.live")}
    </label>
    <span class="state" class:dirty={session.dirty} role="status">
      {#if session.dirty}● {t("draw.unsaved")}{:else if session.savedId}✓ {t("draw.savedState", { name: session.savedName })}{/if}
    </span>
    <button class="primary" onclick={() => session.apply()} disabled={empty}>{t("draw.apply")}</button>
  </div>
  {#if empty}<p class="muted hint">{t("draw.emptyHint")}</p>{/if}
  <h4>{t("save.title")}</h4>
  <SaveBar onsave={(name, tags, asNew) => session.save(name, tags, asNew)} {defaultName} {defaultTags} canOverwrite={!!session.savedId} busy={session.busy || empty} overwriteName={session.savedName} />
</div>

<style>
  .bar { margin-top: 1rem; background: var(--panel); border: 1px solid var(--border); border-radius: 10px; padding: 0.7rem 0.9rem; }
  .top { display: flex; gap: 1rem; align-items: center; flex-wrap: wrap; margin-bottom: 0.6rem; }
  .live { display: flex; gap: 0.4rem; align-items: center; }
  .state { color: var(--muted); font-size: 0.85rem; margin-right: auto; }
  .state.dirty { color: var(--danger); }
  .hint { margin: 0 0 0.5rem; font-size: 0.88rem; }
  h4 { margin: 0 0 0.5rem; }
</style>
