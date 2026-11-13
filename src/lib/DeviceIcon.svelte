<script lang="ts">
  /** Small line icon for a device type. */
  let { type }: { type: string } = $props();
  const kind = $derived(
    type === "Keyboard" ? "kb" : ["Mouse", "MouseMat"].includes(type) ? "mouse" : type === "DRam" ? "ram" : type === "Gpu" ? "gpu" : ["LEDStrip", "Cooler", "Light"].includes(type) ? "fan" : type === "Motherboard" ? "board" : "dev",
  );
</script>

<svg class="ico" viewBox="0 0 24 24" aria-hidden="true">
  {#if kind === "kb"}<rect x="2" y="6" width="20" height="12" rx="2" /><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M7 14h10" />
  {:else if kind === "mouse"}<rect x="7" y="3" width="10" height="18" rx="5" /><path d="M12 3v6" />
  {:else if kind === "ram"}<rect x="2" y="8" width="20" height="8" rx="1" /><path d="M6 16v3M10 16v3M14 16v3M18 16v3M6 11h4M14 11h4" />
  {:else if kind === "gpu"}<rect x="2" y="7" width="20" height="10" rx="2" /><circle cx="9" cy="12" r="2.5" /><circle cx="16" cy="12" r="2.5" />
  {:else if kind === "fan"}<circle cx="12" cy="12" r="9" /><path d="M12 12c0-4 2-6 4-6M12 12c4 0 6 2 6 4M12 12c0 4-2 6-4 6M12 12c-4 0-6-2-6-4" />
  {:else if kind === "board"}<rect x="3" y="3" width="18" height="18" rx="2" /><rect x="7" y="7" width="5" height="5" /><path d="M15 7h2M15 11h2M7 16h10" />
  {:else}<rect x="4" y="4" width="16" height="16" rx="3" /><circle cx="12" cy="12" r="3" />{/if}
</svg>

<style>
  .ico { width: 1.3rem; height: 1.3rem; fill: none; stroke: currentColor; stroke-width: 1.7; stroke-linecap: round; stroke-linejoin: round; flex: none; }
</style>
