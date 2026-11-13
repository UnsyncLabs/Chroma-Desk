<script lang="ts">
  /** Draw tab: pick what to draw (static paint, frames, path or palette) and edit it on the canvas. */
  import type { Animation, Device } from "./api";
  import { t } from "./i18n/index.svelte";
  import PaintEditor from "./PaintEditor.svelte";

  let { device, initial = null }: { device: Device; initial?: Animation | "new" | null } = $props();

  type Kind = "paint";
  const KINDS: Kind[] = ["paint"];
  let kind: Kind = $state("paint");
</script>

<PaintEditor {device} />
