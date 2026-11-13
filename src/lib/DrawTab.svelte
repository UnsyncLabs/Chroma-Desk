<script lang="ts">
  /** Draw tab: pick what to draw (static paint, frames, path or palette) and edit it on the canvas. */
  import { untrack } from "svelte";
  import type { Animation, Device } from "./api";
  import { t } from "./i18n/index.svelte";
  import { app, confirmLeave } from "./state.svelte";
  import CellsEditor from "./CellsEditor.svelte";
  import FramesEditor from "./FramesEditor.svelte";
  import PaintEditor from "./PaintEditor.svelte";
  import PathEditor from "./PathEditor.svelte";

  let { device }: { device: Device } = $props();

  type Kind = "paint" | "frames" | "path" | "palette";
  const KINDS: Kind[] = ["paint", "frames", "path", "palette"];

  // The library may have asked to edit something: take it once.
  const wanted = untrack(() => app.pendingEdit);
  const first = wanted && wanted !== "new" && wanted.kind.type !== "effect" && wanted.kind.type !== "sequence" ? wanted : null;
  let initial: Animation | null = $state(first);
  let kind: Kind = $state(first && first.kind.type !== "effect" && first.kind.type !== "sequence" ? first.kind.type : wanted === "new" ? "frames" : "paint");
  /** Bumped to restart an editor from scratch. */
  let session = $state(0);
  $effect(() => {
    if (app.pendingEdit) {
      const w = app.pendingEdit;
      app.pendingEdit = null;
      if (w !== "new" && w.kind.type !== "effect" && w.kind.type !== "sequence") {
        initial = w;
        kind = w.kind.type;
      } else {
        initial = null;
        kind = "frames";
      }
      session++;
    }
  });

  async function pick(k: Kind) {
    if (k === kind) return;
    if (!(await confirmLeave())) return;
    initial = null;
    kind = k;
    session++;
  }
</script>

<div class="kinds" role="group" aria-label={t("draw.kind")}>
  {#each KINDS as k (k)}
    <button class="kind" class:on={kind === k} aria-pressed={kind === k} onclick={() => pick(k)}>
      <b>{t(`draw.k.${k}`)}</b>
      <small>{t(`draw.k.${k}.hint`)}</small>
    </button>
  {/each}
</div>

{#key `${kind}-${session}`}
  {#if kind === "paint"}
    <PaintEditor {device} />
  {:else if kind === "frames"}
    <FramesEditor {device} {initial} />
  {:else if kind === "path"}
    <PathEditor {device} {initial} />
  {:else}
    <CellsEditor {device} {initial} />
  {/if}
{/key}

<style>
  .kinds { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 0.5rem; margin-bottom: 1rem; }
  .kind { display: flex; flex-direction: column; align-items: flex-start; gap: 0.1rem; text-align: left; padding: 0.45rem 0.7rem; }
  .kind small { color: var(--muted); font-size: 0.76rem; line-height: 1.25; }
  .kind.on { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 16%, var(--control)); }
  .kind.on small { color: var(--text); opacity: 0.8; }
  @media (max-width: 700px) { .kinds { grid-template-columns: repeat(2, minmax(0, 1fr)); } .kind small { display: none; } }
</style>
