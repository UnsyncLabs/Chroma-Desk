<script lang="ts">
  /** First-run tour: Quick -> Draw -> Library, three short steps. */
  import { t } from "./i18n/index.svelte";
  import { app, finishTour, tour } from "./state.svelte";

  let step = $state(0);
  let dlg: HTMLDialogElement | undefined = $state();
  const STEPS = ["quick", "draw", "library"] as const;

  $effect(() => {
    if (!dlg) return;
    if (tour.open && !dlg.open) {
      step = 0;
      dlg.showModal();
    } else if (!tour.open && dlg.open) dlg.close();
  });

  function next() {
    if (step < STEPS.length - 1) step++;
    else {
      finishTour();
      if (app.view.kind === "all") app.view = { kind: "animations" };
    }
  }
</script>

<dialog bind:this={dlg} oncancel={(e) => { e.preventDefault(); finishTour(); }} aria-labelledby="tour-title">
  <p class="count muted">{t("tour.step", { n: step + 1, total: STEPS.length })}</p>
  <div class="art" aria-hidden="true">
    {#if step === 0}
      <span class="chip" style="--c:#7dd3fc"></span><span class="chip" style="--c:#f472b6"></span><span class="chip" style="--c:#a3e635"></span><span class="chip" style="--c:#fbbf24"></span>
    {:else if step === 1}
      <span class="frame f1"></span><span class="frame f2"></span><span class="frame f3"></span>
    {:else}
      <span class="lib"></span><span class="lib"></span><span class="lib"></span>
    {/if}
  </div>
  <h2 id="tour-title">{t(`tour.${STEPS[step]}.title`)}</h2>
  <p>{t(`tour.${STEPS[step]}.text`)}</p>
  <div class="dots" aria-hidden="true">
    {#each STEPS as s, i (s)}<span class:on={i === step}></span>{/each}
  </div>
  <div class="actions">
    <button onclick={finishTour}>{t("tour.skip")}</button>
    {#if step > 0}<button onclick={() => step--}>{t("tour.back")}</button>{/if}
    <button class="primary" onclick={next}>{step === STEPS.length - 1 ? t("tour.done") : t("tour.next")}</button>
  </div>
</dialog>

<style>
  dialog { background: var(--panel); color: var(--text); border: 1px solid var(--border); border-radius: 14px; width: min(26rem, 92vw); padding: 1.2rem 1.4rem; }
  dialog::backdrop { background: #000a; }
  .count { margin: 0 0 0.6rem; font-size: 0.8rem; }
  .art { display: flex; gap: 0.5rem; justify-content: center; align-items: center; height: 4.5rem; background: var(--canvas-bg); border-radius: 10px; margin-bottom: 0.9rem; }
  .chip { width: 2.4rem; height: 2.4rem; border-radius: 8px; background: var(--c); }
  .frame { width: 3.2rem; height: 2.2rem; border-radius: 6px; border: 2px solid var(--accent); background: linear-gradient(90deg, #f472b6, #7dd3fc); }
  .f2 { opacity: 0.65; }
  .f3 { opacity: 0.35; }
  .lib { width: 2.6rem; height: 2.6rem; border-radius: 8px; border: 1px solid var(--border); background: var(--panel-2); position: relative; }
  .lib::after { content: ""; position: absolute; inset: 0.6rem; border-radius: 4px; background: linear-gradient(135deg, var(--accent), #a78bfa); }
  h2 { margin: 0 0 0.4rem; font-size: 1.15rem; }
  p { margin: 0 0 0.8rem; line-height: 1.45; }
  .dots { display: flex; gap: 0.35rem; justify-content: center; margin-bottom: 0.9rem; }
  .dots span { width: 0.5rem; height: 0.5rem; border-radius: 50%; background: var(--control); }
  .dots span.on { background: var(--accent); }
  .actions { display: flex; justify-content: flex-end; gap: 0.5rem; flex-wrap: wrap; }
</style>
