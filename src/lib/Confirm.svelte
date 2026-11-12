<script lang="ts">
  /** Modal confirmation dialog driven by `confirmDialog()`. */
  import { t } from "./i18n/index.svelte";
  import { confirmState } from "./state.svelte";

  let dlg: HTMLDialogElement | undefined = $state();

  $effect(() => {
    if (!dlg) return;
    if (confirmState.open && !dlg.open) dlg.showModal();
    else if (!confirmState.open && dlg.open) dlg.close();
  });

  function answer(ok: boolean) {
    confirmState.open = false;
    confirmState.resolve?.(ok);
    confirmState.resolve = null;
  }
</script>

<dialog bind:this={dlg} oncancel={(e) => { e.preventDefault(); answer(false); }}>
  <p>{confirmState.text}</p>
  <div class="actions">
    <button onclick={() => answer(false)}>{t("common.cancel")}</button>
    <button class="primary" onclick={() => answer(true)}>{confirmState.confirmLabel || t("common.ok")}</button>
  </div>
</dialog>

<style>
  dialog { background: var(--panel); color: var(--text); border: 1px solid var(--border); border-radius: 10px; max-width: min(26rem, 90vw); }
  dialog::backdrop { background: #0008; }
  .actions { display: flex; justify-content: flex-end; gap: 0.5rem; }
</style>
