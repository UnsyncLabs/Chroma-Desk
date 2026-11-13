<script lang="ts">
  /** "Set up Claude Code": shows the exact hook, adds it to Claude Code's settings.json (merged, with a backup) or removes it. */
  import { api, type ClaudeStatus } from "./api";
  import { t } from "./i18n/index.svelte";
  import { copyText, toast, toastError } from "./state.svelte";

  let st: ClaudeStatus | null = $state(null);
  let busy = $state(false);

  async function load() {
    try {
      st = await api.claudeStatus();
    } catch (e) {
      toastError(e);
    }
  }
  $effect(() => {
    load();
  });

  async function run(install: boolean) {
    busy = true;
    try {
      st = await (install ? api.claudeInstall() : api.claudeRemove());
      toast(t(install ? "claude.installed" : "claude.removed"), "ok", 2500);
    } catch (e) {
      toastError(e);
    } finally {
      busy = false;
    }
  }

  const GENERIC = "chroma-desk event ai:attention --from <tool> [--kind permission|idle]";
</script>

<div class="claude" data-claude>
  <p class="muted small">{t("claude.lead")}</p>
  {#if st}
    <p class="state">
      <span class="pill" class:ok={st.installed} data-claude-state>{st.installed ? t("claude.on") : t("claude.off")}</span>
      <span class="muted small">{st.path}{st.exists ? "" : ` (${t("claude.missing")})`}</span>
    </p>
    {#if !st.valid}<p class="warn small" data-claude-invalid>{t("claude.invalid")}</p>{/if}
    <details data-claude-details>
      <summary>{t("claude.show")}</summary>
      <p class="small">{t("claude.willAdd", { path: st.path })}</p>
      <pre data-claude-snippet>{st.snippet}</pre>
      <p class="muted small">{t("claude.safe")}</p>
    </details>
    <div class="row">
      {#if st.installed}
        <button onclick={() => run(false)} disabled={busy} data-claude-remove>{t("claude.remove")}</button>
      {:else}
        <button class="primary" onclick={() => run(true)} disabled={busy || !st.valid} data-claude-install>{t("claude.setup")}</button>
      {/if}
      <button onclick={() => copyText(st?.command ?? "", t("trg.copied"))}>{t("claude.copyCmd")}</button>
    </div>
    {#if st.backup}<p class="muted small" data-claude-backup>{t("claude.backup", { path: st.backup })}</p>{/if}
  {/if}
  <details class="other">
    <summary>{t("claude.other")}</summary>
    <p class="small">{t("claude.otherHint")}</p>
    <div class="cmd"><pre>{GENERIC}</pre><button onclick={() => copyText(GENERIC, t("trg.copied"))}>{t("trg.copy")}</button></div>
    <p class="muted small">{t("claude.clearHint")}</p>
  </details>
</div>

<style>
  .small { font-size: 0.82rem; margin: 0.2rem 0 0.5rem; }
  .state { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; margin: 0.3rem 0; }
  .pill { font-size: 0.78rem; border: 1px solid var(--border); border-radius: 999px; padding: 0.1rem 0.6rem; color: var(--muted); }
  .pill.ok { color: var(--ok); border-color: var(--ok); }
  .warn { color: var(--warn, #e0a030); }
  pre { background: var(--panel-2); border: 1px solid var(--border); border-radius: 8px; padding: 0.5rem 0.7rem; font-size: 0.78rem; overflow-x: auto; margin: 0.3rem 0; }
  .row { display: flex; gap: 0.5rem; flex-wrap: wrap; margin: 0.4rem 0; }
  .cmd { display: flex; gap: 0.5rem; align-items: flex-start; }
  .cmd pre { flex: 1; min-width: 0; white-space: pre-wrap; overflow-wrap: anywhere; }
  details { margin: 0.3rem 0; }
  summary { cursor: pointer; color: var(--muted); }
</style>
