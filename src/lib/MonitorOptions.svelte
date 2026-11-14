<script lang="ts">
  /** Monitor editor top: how it is shown (always / periodic) and which metrics it displays. */
  import type { Animation, NamedTrigger } from "./api";
  import { t } from "./i18n/index.svelte";
  import { meterMetrics, setDisplayMode, timingInvalid } from "./automationsStore.svelte";
  import { METRICS, defaultRange, metricLabel } from "./sysmon.svelte";

  let { n = $bindable(), seq = $bindable() }: { n: NamedTrigger; seq: Animation } = $props();

  const every = $derived(n.sources?.every);
  const mode = $derived(n.sources?.every ? "periodic" : n.sources?.always ? "always" : "");
  const invalid = $derived(timingInvalid(n));

  // Period and duration are typed as a number plus a unit (seconds or minutes).
  const MULT = { s: 1, min: 60 } as const;
  type Unit = keyof typeof MULT;
  const unitFor = (v: number): Unit => (v >= 60 && v % 60 === 0 ? "min" : "s");
  // svelte-ignore state_referenced_locally
  let everyUnit = $state<Unit>(unitFor(n.sources?.every?.every_secs ?? 10));
  // svelte-ignore state_referenced_locally
  let forUnit = $state<Unit>(unitFor(n.sources?.every?.for_secs ?? 7));
  const show = (secs: number, u: Unit) => Math.round((secs / MULT[u]) * 100) / 100;
  function set(which: "every" | "for", val: number, unit: Unit) {
    if (which === "every") everyUnit = unit;
    else forUnit = unit;
    const secs = Math.round(val * MULT[unit] * 100) / 100;
    const cur = n.sources?.every ?? { every_secs: 10, for_secs: 7 };
    n.sources = { ...(n.sources ?? {}), every: { ...cur, ...(which === "every" ? { every_secs: secs } : { for_secs: secs }) } };
  }

  // ---------- metrics ----------
  const COLORS: Record<string, unknown> = { "cpu.usage": { slot: "warn" }, "cpu.temp": { slot: "alert" }, "gpu.usage": [255, 255, 255], "gpu.vram": { slot: "ok" }, "gpu.temp": { slot: "alert" }, "ram.usage": { slot: "secondary" } };
  const steps = $derived(seq.kind.type === "sequence" ? seq.kind.steps : []);
  const used = $derived(meterMetrics(seq));
  function toggle(m: string, on: boolean) {
    if (seq.kind.type !== "sequence") return;
    if (on) {
      const [min, max] = defaultRange(m);
      const keys = steps.find((s) => s.effect === "meter")?.keys ?? [];
      seq.kind.steps.push({ effect: "meter", params: { metric: m, color: COLORS[m] as never, min, max, style: "sweep", sweep_ms: 1200, background: "none" }, keys: [...keys], duration_ms: 3200 });
    } else if (used.length > 1) {
      seq.kind.steps = seq.kind.steps.filter((s) => !(s.effect === "meter" && s.params.metric === m));
    }
  }
</script>

<div class="mon" data-monitor-options>
  <h4>{t("mon.displayMode")}</h4>
  <div class="modes" role="radiogroup" aria-label={t("mon.displayMode")}>
    <button class="mode" class:on={mode === "always"} role="radio" aria-checked={mode === "always"} onclick={() => setDisplayMode(n, "always")} data-dm="always">
      <b>{t("mon.always")}</b><small>{t("mon.alwaysHint")}</small>
    </button>
    <button class="mode" class:on={mode === "periodic"} role="radio" aria-checked={mode === "periodic"} onclick={() => setDisplayMode(n, "periodic")} data-dm="periodic">
      <b>{t("mon.periodic")}</b><small>{t("mon.periodicHint")}</small>
    </button>
  </div>
  {#if every}
    <div class="fields" data-dm-fields>
      <label>{t("mon.every")}
        <span class="inline">
          <input type="number" min="1" step="any" value={show(every.every_secs, everyUnit)} oninput={(e) => set("every", Number(e.currentTarget.value) || 0, everyUnit)} data-dm-every aria-invalid={invalid} />
          <select value={everyUnit} onchange={(e) => set("every", show(every.every_secs, everyUnit), e.currentTarget.value as Unit)} data-dm-every-unit aria-label={t("auto.when.unit")}>
            <option value="s">{t("auto.u.s")}</option><option value="min">{t("auto.u.min")}</option>
          </select>
        </span>
      </label>
      <label>{t("mon.showFor")}
        <span class="inline">
          <input type="number" min="1" step="any" value={show(every.for_secs, forUnit)} oninput={(e) => set("for", Number(e.currentTarget.value) || 0, forUnit)} data-dm-for aria-invalid={invalid} />
          <select value={forUnit} onchange={(e) => set("for", show(every.for_secs, forUnit), e.currentTarget.value as Unit)} data-dm-for-unit aria-label={t("auto.when.unit")}>
            <option value="s">{t("auto.u.s")}</option><option value="min">{t("auto.u.min")}</option>
          </select>
        </span>
      </label>
    </div>
    {#if invalid}<p class="err" role="alert" data-dm-error>{t("mon.timingError")}</p>{/if}
  {/if}

  <h4>{t("mon.metrics")}</h4>
  <p class="muted small">{t("mon.metricsHint")}</p>
  <div class="chips" role="group" aria-label={t("mon.metrics")}>
    {#each METRICS as m (m)}
      <label class="chip" class:on={used.includes(m)}><input type="checkbox" checked={used.includes(m)} disabled={used.includes(m) && used.length === 1} onchange={(e) => toggle(m, e.currentTarget.checked)} data-metric-chip={m} /> {metricLabel(m)}</label>
    {/each}
  </div>
</div>

<style>
  .mon { margin-bottom: 0.8rem; padding-bottom: 0.6rem; border-bottom: 1px solid var(--border); }
  h4 { margin: 0.4rem 0; }
  .small { font-size: 0.82rem; margin: 0 0 0.4rem; }
  .modes { display: grid; grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr)); gap: 0.4rem; }
  .mode { display: flex; flex-direction: column; align-items: flex-start; text-align: left; gap: 0.1rem; }
  .mode small { font-size: 0.74rem; opacity: 0.8; }
  .mode.on { background: var(--accent); color: var(--accent-fg); border-color: var(--accent); }
  .fields { display: flex; gap: 1.2rem; flex-wrap: wrap; margin-top: 0.6rem; }
  .fields label { display: flex; flex-direction: column; gap: 0.2rem; font-size: 0.9rem; color: var(--muted); }
  .inline { display: inline-flex; gap: 0.4rem; align-items: center; }
  .inline input { width: 5.5rem; }
  .err { color: var(--danger); margin: 0.4rem 0 0; font-size: 0.85rem; }
  .chips { display: flex; gap: 0.4rem; flex-wrap: wrap; }
  .chip { display: inline-flex; gap: 0.35rem; align-items: center; background: var(--panel-2); border-radius: 999px; padding: 0.2rem 0.7rem; font-size: 0.88rem; }
  .chip.on { background: color-mix(in srgb, var(--accent) 25%, transparent); }
</style>
