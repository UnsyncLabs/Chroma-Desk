<script lang="ts">
  /** Tiny colour strip: a device's LEDs in a row (sampled when there are many). */
  import { hex, type Rgb } from "./api";

  let { colors, max = 48 }: { colors: Rgb[]; max?: number } = $props();

  const sampled = $derived.by(() => {
    if (colors.length <= max) return colors;
    return Array.from({ length: max }, (_, i) => colors[Math.floor((i * colors.length) / max)]);
  });
</script>

<span class="strip" aria-hidden="true">
  {#each sampled as c, i (i)}<span style:background={hex(c)}></span>{/each}
</span>

<style>
  .strip { display: flex; gap: 1px; height: 0.55rem; min-width: 0; }
  .strip span { flex: 1; min-width: 2px; border-radius: 1px; }
</style>
