<script lang="ts">
  /** Animated thumbnail: the device layout drawn on a small canvas, animated only while on screen. */
  import type { Animation } from "./api";
  import type { Layout } from "./layout/buildLayout";
  import { cycleSecs, flatten, renderResolved, resolveAnim } from "./anim/render";

  let { anim, layout, names }: { anim: Animation; layout: Layout; names: string[] } = $props();

  const W = 360;
  const PAD = 0.3;
  let canvas: HTMLCanvasElement | undefined = $state();
  let onScreen = $state(false);

  const ratio = $derived(Math.min(1, Math.max(0.2, (layout.height + PAD * 2) / (layout.width + PAD * 2))));
  const resolved = $derived(resolveAnim(anim, names));
  const reduced = typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;

  function draw(t: number) {
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;
    const px = flatten(renderResolved(resolved, t, layout.positions, names.length));
    const k = canvas.width / (layout.width + PAD * 2);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    for (const it of layout.items) {
      const c = px[it.led];
      const lit = c && c[0] + c[1] + c[2] > 12;
      ctx.fillStyle = lit ? `rgb(${c[0]},${c[1]},${c[2]})` : "#1c1c26";
      const x = (it.x + PAD) * k;
      const y = (it.y + PAD) * k;
      ctx.beginPath();
      if (it.shape === "circle") ctx.arc(x + (it.w * k) / 2, y + (it.h * k) / 2, (it.w * k) / 2, 0, Math.PI * 2);
      else ctx.roundRect(x, y, it.w * k, it.h * k, Math.min(3, it.w * k * 0.2));
      ctx.fill();
    }
  }

  $effect(() => {
    if (!canvas) return;
    const io = new IntersectionObserver((e) => (onScreen = e.some((x) => x.isIntersecting)), { threshold: 0.01 });
    io.observe(canvas);
    return () => io.disconnect();
  });

  $effect(() => {
    // Re-run (and redraw) whenever the animation or layout changes.
    void resolved;
    void layout;
    if (!canvas || !onScreen) return;
    if (reduced) {
      draw(cycleSecs(anim) * 0.35);
      return;
    }
    let raf = 0;
    let last = 0;
    const t0 = performance.now();
    const loop = (now: number) => {
      if (now - last >= 40) {
        last = now;
        draw((now - t0) / 1000);
      }
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  });
</script>

<canvas bind:this={canvas} width={W} height={Math.round(W * ratio)} style:aspect-ratio={`${1 / ratio}`} aria-hidden="true"></canvas>

<style>
  canvas { width: 100%; height: auto; display: block; background: var(--canvas-bg); border-radius: 8px; }
</style>
