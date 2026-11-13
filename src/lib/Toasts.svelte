<script lang="ts">
  import { toasts } from "./state.svelte";
</script>

<div class="toasts" aria-live="polite">
  {#each toasts.list as x (x.id)}
    <div class="toast {x.kind}" role="status">{x.text}</div>
  {/each}
</div>

<style>
  .toasts { position: fixed; bottom: 0.8rem; left: 50%; transform: translateX(-50%); display: flex; flex-direction: column; align-items: center; gap: 0.4rem; width: max-content; max-width: min(28rem, calc(100vw - 2rem)); z-index: 10; pointer-events: none; }
  /* Click-through: a toast must never cover the button the user is about to press. */
  .toast { pointer-events: none; text-align: left; padding: 0.4rem 0.8rem; border-radius: 6px; background: var(--panel); border-left: 4px solid var(--accent); box-shadow: 0 4px 16px #0006; }
  .toast.error { border-left-color: var(--danger); }
  .toast.ok { border-left-color: var(--ok); }
</style>
