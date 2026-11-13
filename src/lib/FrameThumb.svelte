<script lang="ts">
  /** Static miniature of one frame (timeline). */
  import type { Rgb } from "./api";
  import type { Layout } from "./layout/buildLayout";

  let { leds, layout }: { leds: (Rgb | null)[]; layout: Layout } = $props();
  let canvas: HTMLCanvasElement | undefined = $state();

  const W = 150;
  const PAD = 0.3;
  const ratio = $derived(Math.min(1, Math.max(0.2, (layout.height + PAD * 2) / (layout.width + PAD * 2))));

  $effect(() => {
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;
    const k = canvas.width / (layout.width + PAD * 2);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    for (const it of layout.items) {
      const c = leds[it.led];
      ctx.fillStyle = c ? `rgb(${c[0]},${c[1]},${c[2]})` : "#242430";
      const x = (it.x + PAD) * k;
      const y = (it.y + PAD) * k;
      ctx.beginPath();
      if (it.shape === "circle") ctx.arc(x + (it.w * k) / 2, y + (it.h * k) / 2, (it.w * k) / 2, 0, Math.PI * 2);
      else ctx.rect(x, y, it.w * k, it.h * k);
      ctx.fill();
    }
  });
</script>

<canvas bind:this={canvas} width={W} height={Math.round(W * ratio)} aria-hidden="true"></canvas>

<style>
  canvas { width: 100%; height: auto; display: block; background: var(--canvas-bg); border-radius: 4px; }
</style>
