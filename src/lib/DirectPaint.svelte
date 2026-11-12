<script lang="ts">
  /** Paints LEDs straight onto the device (interim editor, replaced by layers). */
  import { api, throttle, type Device, type Rgb } from "./api";
  import LedCanvas, { type Paint } from "./LedCanvas.svelte";
  import { buildLayout } from "./layout/buildLayout";
  import { fans, toastError } from "./state.svelte";

  let { device }: { device: Device } = $props();

  const layout = $derived(buildLayout(device, { fanLeds: fans.map[device.key] }));
  let colors: Paint = $state([]);
  let selected = $state(new Set<number>());
  let direct = false;

  $effect(() => {
    colors = device.colors.map((c) => [...c] as Rgb);
  });

  const push = throttle((cols: Rgb[]) => api.setLeds(device.id, cols).catch(toastError), 33);

  async function change(next: Paint) {
    colors = next;
    const cols = next.map((c) => c ?? ([0, 0, 0] as Rgb));
    if (!direct) {
      direct = true;
      await api.setLeds(device.id, cols, true).catch(toastError);
    } else push(cols);
  }
</script>

<LedCanvas {layout} {colors} bind:selected resetKey={device.key} onchange={change} />
