<script lang="ts">
  /** Draw > Paint: static LED painting into a Paint layer, savable as a one-frame animation. */
  import { api, type Animation, type Color, type Device } from "./api";
  import { cloneColor } from "./palette.svelte";
  import { refMaker } from "./anim/ref";
  import { loadAnimations } from "./animationsStore.svelte";
  import { t } from "./i18n/index.svelte";
  import { buildLayout } from "./layout/buildLayout";
  import { applyStroke, paintTarget, toPaint } from "./paintLayer";
  import { commit, devScene, frames } from "./scene.svelte";
  import { toast, toastError } from "./state.svelte";
  import LedCanvas, { type Paint } from "./LedCanvas.svelte";
  import SaveBar from "./SaveBar.svelte";

  let { device }: { device: Device } = $props();

  let selected = $state(new Set<number>());
  const ds = $derived(devScene(device.key));
  const effects = $derived(ds.mode === "effects");
  const layout = $derived(buildLayout(device, { fanLeds: ds.fan_leds }));
  const n = $derived(device.colors.length);
  const layer = $derived(ds.layers.findLast((l) => l.effect === "paint") ?? null);
  const frame = $derived(frames.map[device.key]);
  const canvasColors: Paint = $derived(layer ? toPaint(layer, n) : (frame ?? device.colors));
  const display = $derived(effects ? (frame ?? null) : null);

  function onPaint(next: Paint) {
    const prev = canvasColors;
    const l = paintTarget(device, null);
    applyStroke(l, prev, next);
    commit();
  }

  async function saveAs(name: string, tags: string[]) {
    const names = Array.from({ length: n }, (_, i) => device.led_names[i] ?? "");
    const ref = refMaker(names);
    const leds: Record<string, Color> = {};
    // Only what was painted: without a paint layer the canvas shows the live effect, which is not a drawing.
    (layer ? toPaint(layer, n) : []).forEach((c, i) => {
      if (c) leds[ref(i)] = cloneColor(c);
    });
    if (!Object.keys(leds).length) {
      toast(t("save.nothing"), "error");
      return;
    }
    const anim: Animation = {
      schema_version: 1,
      id: "",
      name,
      tags,
      created: 0,
      updated: 0,
      builtin: false,
      target: { kind: layout.kind === "keyboard" ? "keyboard" : layout.kind === "linear" ? "linear" : "any", hint: device.name },
      kind: { type: "frames", frames: [{ leds }], frame_ms: 1000, loop: "loop", crossfade: 0 },
    };
    try {
      const saved = await api.animSave(anim);
      await loadAnimations();
      toast(t("save.saved", { name: saved.name }), "ok");
    } catch (e) {
      toastError(e);
    }
  }
</script>

<p class="muted help">{t("paint.help")}</p>
<LedCanvas {layout} colors={canvasColors} {display} transparent={!!layer} bind:selected resetKey={layer?.id ?? ""} onchange={onPaint} />
<div class="save">
  <h4>{t("save.title")}</h4>
  <SaveBar onsave={(name, tags) => saveAs(name, tags)} />
</div>

<style>
  .help { margin: 0 0 0.6rem; }
  .save { margin-top: 1rem; background: var(--panel); border: 1px solid var(--border); border-radius: 10px; padding: 0.7rem 0.9rem; }
  .save h4 { margin: 0 0 0.5rem; }
</style>
