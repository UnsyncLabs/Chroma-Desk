<script lang="ts">
  /**
   * One colour control for the whole app: a *palette slot* (linked to the active theme, it changes
   * with the theme) or a *custom* fixed colour. The trigger shows the colour; a linked colour also
   * carries a small `@slot` chip.
   */
  import { hex, fromHex, type Color, type Rgb } from "./api";
  import { t } from "./i18n/index.svelte";
  import { MAIN_SLOTS, SLOTS, isSlot, palette, resolveColor } from "./palette.svelte";

  let {
    value,
    onchange,
    label = "",
    allowSlots = true,
  }: {
    value: Color;
    onchange: (c: Color) => void;
    /** Accessible name of the trigger. */
    label?: string;
    allowSlots?: boolean;
  } = $props();

  let open = $state(false);
  // svelte-ignore state_referenced_locally
  let tab = $state<"palette" | "custom">(allowSlots && isSlot(value) ? "palette" : isSlot(value) || !allowSlots ? "custom" : "custom");
  let root: HTMLElement | undefined = $state();
  let popStyle = $state("");

  const rgb = $derived(resolveColor(value));
  const linked = $derived(isSlot(value) ? value.slot : null);
  const rest = SLOTS.filter((s) => !MAIN_SLOTS.includes(s));

  function toggle() {
    open = !open;
    if (open) {
      tab = allowSlots && isSlot(value) ? "palette" : allowSlots && !isSlot(value) ? "palette" : "custom";
      if (!isSlot(value)) tab = "custom";
      const r = root?.getBoundingClientRect();
      // Keep the popover inside the window (small windows too): shift it left near the right edge,
      // open it upwards when there is no room below and more room above.
      if (r) {
        const w = Math.min(280, window.innerWidth - 32);
        const dx = Math.min(0, window.innerWidth - 8 - (r.left + w));
        const up = window.innerHeight - r.bottom < 340 && r.top > window.innerHeight - r.bottom;
        popStyle = `left:${dx}px;` + (up ? "top:auto;bottom:calc(100% + 0.3rem);max-height:" + Math.max(160, r.top - 12) + "px;overflow:auto;" : "");
      }
    }
  }
  function onWindowDown(e: PointerEvent) {
    if (open && root && !root.contains(e.target as Node)) open = false;
  }
  /** Escape closes the popover and goes no further (it would also clear a canvas selection). */
  function onKey(e: KeyboardEvent) {
    if (open && e.key === "Escape") {
      open = false;
      e.stopPropagation();
    }
  }
  const pick = (slot: string) => onchange({ slot });
  const lighten = (v: number) => isSlot(value) && onchange({ slot: value.slot, ...(v ? { lighten: v } : {}) });
</script>

<svelte:window onpointerdown={onWindowDown} onkeydowncapture={onKey} />

<span class="cp" bind:this={root} data-colorpicker>
  <button type="button" class="trigger" aria-haspopup="dialog" aria-expanded={open} aria-label={label || t("cp.label")} title={linked ? `@${linked}` : hex(rgb)} onclick={toggle}>
    <span class="sw" style:background={hex(rgb)}></span>
    {#if linked}<span class="chip">@{linked}</span>{:else}<span class="hexv">{hex(rgb)}</span>{/if}
  </button>
  {#if open}
    <div class="pop" role="dialog" aria-label={t("cp.label")} style={popStyle}>
      {#if allowSlots}
        <div class="tabs" role="tablist">
          <button type="button" role="tab" aria-selected={tab === "palette"} class:on={tab === "palette"} onclick={() => (tab = "palette")}>{t("cp.palette")}</button>
          <button type="button" role="tab" aria-selected={tab === "custom"} class:on={tab === "custom"} onclick={() => (tab = "custom")}>{t("cp.custom")}</button>
        </div>
      {/if}
      {#if tab === "palette" && allowSlots}
        <p class="hint">{t("cp.paletteHint", { name: palette.name })}</p>
        <div class="slots">
          {#each [...MAIN_SLOTS, ...rest] as s (s)}
            <button type="button" class="slot" class:on={linked === s} data-slot={s} title={`@${s} · ${hex(palette.slots[s] ?? [0, 0, 0])}`} aria-label={t(`slot.${s}`)} aria-pressed={linked === s} onclick={() => pick(s)}>
              <span class="sw big" style:background={hex(palette.slots[s] ?? [0, 0, 0])}>{#if linked === s}<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M10 14a4 4 0 0 0 5.6 0l3-3a4 4 0 0 0-5.6-5.6l-1 1M14 10a4 4 0 0 0-5.6 0l-3 3a4 4 0 0 0 5.6 5.6l1-1" /></svg>{/if}</span>
              <small>{t(`slot.${s}`)}</small>
            </button>
          {/each}
        </div>
        {#if linked}
          <label class="light"><span>{t("cp.lighten")}</span><input type="range" min="-0.8" max="0.8" step="0.05" value={isSlot(value) ? (value.lighten ?? 0) : 0} oninput={(e) => lighten(Number(e.currentTarget.value))} aria-label={t("cp.lighten")} /></label>
        {/if}
      {:else}
        <div class="custom">
          <input type="color" value={hex(rgb)} aria-label={t("cp.custom")} oninput={(e) => onchange(fromHex(e.currentTarget.value) as Rgb)} />
          <code>{hex(rgb)}</code>
          {#if linked}<small>{t("cp.unlinkHint")}</small>{/if}
        </div>
      {/if}
    </div>
  {/if}
</span>

<style>
  .cp { position: relative; display: inline-block; }
  .trigger { display: inline-flex; align-items: center; gap: 0.4rem; padding: 0.2rem 0.5rem 0.2rem 0.25rem; }
  .sw { width: 1.5rem; height: 1.5rem; border-radius: 5px; border: 1px solid #ffffff40; display: inline-flex; align-items: center; justify-content: center; flex: none; }
  .sw.big { width: 2rem; height: 2rem; }
  .sw svg { width: 1rem; height: 1rem; fill: none; stroke: #fff; stroke-width: 2.4; stroke-linecap: round; filter: drop-shadow(0 0 2px #000); }
  .chip { font-size: 0.75rem; background: var(--accent); color: var(--accent-fg); border-radius: 999px; padding: 0.05rem 0.5rem; }
  .hexv { font-family: monospace; font-size: 0.8rem; color: var(--muted); }
  .pop { font-family: Inter, system-ui, sans-serif; position: absolute; z-index: 30; top: calc(100% + 0.3rem); left: 0; width: min(17.5rem, calc(100vw - 2rem)); background: var(--panel); border: 1px solid var(--border); border-radius: 10px; padding: 0.6rem; box-shadow: 0 10px 30px #0008; }
  .tabs { display: flex; gap: 0.3rem; margin-bottom: 0.5rem; }
  .tabs .on { background: var(--accent); color: var(--accent-fg); }
  .hint { margin: 0 0 0.5rem; font-size: 0.78rem; color: var(--muted); }
  .slots { display: grid; grid-template-columns: repeat(5, 1fr); gap: 0.3rem; }
  .slot { display: flex; flex-direction: column; align-items: center; gap: 0.15rem; padding: 0.25rem 0.1rem; background: none; border: 1px solid transparent; }
  .slot.on { border-color: var(--accent); background: var(--panel-2); }
  .slot small { font-size: 0.62rem; color: var(--muted); overflow: hidden; text-overflow: ellipsis; max-width: 100%; white-space: nowrap; }
  .light { display: grid; grid-template-columns: auto 1fr; gap: 0.5rem; align-items: center; margin-top: 0.6rem; font-size: 0.82rem; color: var(--muted); }
  .custom { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; }
  .custom input { width: 3rem; height: 2.4rem; padding: 0; border: none; background: none; }
  .custom small { color: var(--muted); flex-basis: 100%; }
</style>
