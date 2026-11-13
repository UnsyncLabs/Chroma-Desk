<script lang="ts">
  import { dismissToast, toasts } from "./state.svelte";
</script>

<div class="toasts" aria-live="polite">
  {#each toasts.list as x (x.id)}
    <button class="toast {x.kind}" onclick={() => dismissToast(x.id)}>{x.text}</button>
  {/each}
</div>

<style>
  .toasts { position: fixed; top: 0.8rem; left: 50%; transform: translateX(-50%); display: flex; flex-direction: column; align-items: center; gap: 0.4rem; width: max-content; max-width: min(28rem, calc(100vw - 2rem)); z-index: 10; pointer-events: none; }
  .toast { pointer-events: auto; text-align: left; background: var(--panel); border-left: 4px solid var(--accent); box-shadow: 0 4px 16px #0006; }
  .toast.error { border-left-color: var(--danger); }
  .toast.ok { border-left-color: var(--ok); }
</style>
