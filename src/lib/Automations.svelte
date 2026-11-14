<script lang="ts">
  /** Automations: monitors, timers and notifications as cards, templates and the editor. */
  import { api, hex, type Animation, type NamedTrigger } from "./api";
  import { t } from "./i18n/index.svelte";
  import { app, confirmDialog, toast, toastError } from "./state.svelte";
  import { animName, lib, loadAnimations } from "./animationsStore.svelte";
  import { activeProfile } from "./profilesStore.svelte";
  import { palette, resolveColor } from "./palette.svelte";
  import { recolorAnim } from "./anim/recolor";
  import { buildLayout } from "./layout/buildLayout";
  import AnimThumb from "./AnimThumb.svelte";
  import AutomationEditor from "./AutomationEditor.svelte";
  import { metricLabel, watchMetrics } from "./sysmon.svelte";
  import { allTemplates, canEditProfile, categoryOf, deleteNotification, describeKeys, listNotifications, loadTemplates, plain, saveNotification, setEnabled, slugName, templateName, tplStore, whenChips, type Category, type NotifItem, type Scope, type Template } from "./automationsStore.svelte";

  let editing: { item: NotifItem; isNew: boolean } | null = $state(null);

  const items = $derived(listNotifications());
  const keyboard = $derived(app.devices.find((d) => buildLayout(d).kind === "keyboard") ?? app.devices[0] ?? null);
  const layout = $derived(keyboard ? buildLayout(keyboard) : null);
  const names = $derived(keyboard ? Array.from({ length: keyboard.colors.length }, (_, i) => keyboard.led_names[i] ?? "") : []);
  const defaultScope = $derived<Scope>(canEditProfile() ? "profile" : "global");
  const templates = $derived(allTemplates());

  $effect(() => {
    if (!lib.loaded) loadAnimations();
    if (!tplStore.loaded) loadTemplates();
  });
  // Live meters in the card thumbnails need readings.
  $effect(() => {
    const ids = items.flatMap((i) => (i.n.sequence?.kind.type === "sequence" ? i.n.sequence.kind.steps.map((s) => String(s.params.metric ?? "")).filter(Boolean) : []));
    if (!ids.length) return;
    return watchMetrics([...new Set(ids)]);
  });

  const SECTIONS: Category[] = ["monitor", "timer", "notification"];
  const grouped = $derived(SECTIONS.map((c) => ({ c, list: items.filter((i) => categoryOf(i.n) === c) })).filter((g) => g.list.length));

  const animOf = (n: NamedTrigger): Animation | null => n.sequence ?? lib.list.find((a) => a.id === n.anim || a.name === n.anim) ?? null;
  const rgbOf = (c: string) => (c.startsWith("@") ? resolveColor({ slot: c.slice(1) }) : ([1, 3, 5].map((i) => parseInt(c.slice(i, i + 2), 16)) as [number, number, number]));
  /** The animation the way it will look: recoloured with the notification's colour. */
  function previewOf(n: NamedTrigger): Animation | null {
    const a = animOf(n);
    if (!a) return null;
    palette.generation;
    return n.color && !n.sequence ? recolorAnim(a, rgbOf(n.color)) : a;
  }

  const used = (name: string) => items.some((i) => i.name === name);
  function freeName(base: string) {
    let n = slugName(base) || "automation";
    let i = 2;
    while (used(n)) n = `${slugName(base)}_${i++}`.slice(0, 32);
    return n;
  }

  function create() {
    editing = { item: { name: freeName("automation"), scope: defaultScope, n: { anim: "builtin:flash", color: "@accent", duration: 2 } }, isNew: true };
  }
  async function fromTemplate(tpl: Template) {
    const item: NotifItem = { name: freeName(templateName(tpl.id)), scope: defaultScope, n: plain(tpl.n) };
    try {
      await saveNotification(item);
      toast(t("auto.created", { name: item.name }), "ok", 4000);
    } catch (e) {
      toastError(e);
    }
  }

  async function test(i: NotifItem) {
    try {
      const r = await api.trigger({ animation: i.name });
      toast(t("notif.tested", { n: r.devices }), "ok", 2200);
    } catch (e) {
      toastError(e);
    }
  }
  async function toggle(i: NotifItem, on: boolean) {
    try {
      await setEnabled(i, on);
    } catch (e) {
      toastError(e);
    }
  }
  async function remove(i: NotifItem) {
    if (!(await confirmDialog(t("auto.confirmDelete", { name: i.name }), t("common.delete")))) return;
    try {
      await deleteNotification(i);
    } catch (e) {
      toastError(e);
    }
  }
  /** Always-on automations that "Stop all" paused (they stay off until resumed or edited). */
  let paused: string[] = $state([]);
  const refreshPaused = () => api.pausedAutomations().then((p) => (paused = p), () => {});
  $effect(() => {
    void items; // editing, disabling or deleting an automation un-pauses it in the backend
    void refreshPaused();
  });
  async function stopAll() {
    try {
      toast(t("trg.stopped", { n: await api.stopTriggers() }), "ok", 2000);
      await refreshPaused();
    } catch (e) {
      toastError(e);
    }
  }
  async function resume() {
    try {
      await api.resumeTriggers();
      await refreshPaused();
      toast(t("auto.resumed"), "ok", 2000);
    } catch (e) {
      toastError(e);
    }
  }

  /** "GPU memory + GPU usage" for meters, "Blink · Orbit · Off" for other steps: what the card actually shows. */
  function sequenceSummary(seq: Animation): string {
    if (seq.kind.type !== "sequence") return "";
    const steps = seq.kind.steps;
    const meters = steps.filter((s) => s.effect === "meter").map((s) => metricLabel(String(s.params.metric ?? "")));
    if (meters.length) return `${[...new Set(meters)].join(" + ")} · ${t("auto.keysCount", { n: Math.max(...steps.map((s) => s.keys?.length ?? 0)) })}`;
    const names = steps.map((s) => (t(`seq.style.${s.effect}`) !== `seq.style.${s.effect}` ? t(`seq.style.${s.effect}`) : s.effect));
    return [...new Set(names)].join(" · ");
  }
  const doesLabel = (n: NamedTrigger) => (n.sequence ? sequenceSummary(n.sequence) || t("auto.sequenceOf", { n: 0 }) : (animOf(n) ? animName(animOf(n) as Animation) : n.anim));
</script>

{#snippet tplGrid(list: Template[])}
  <ul class="tpls">
    {#each list as tpl (tpl.id)}
      <li>
        <button class="tpl" data-template={tpl.id} onclick={() => fromTemplate(tpl)}>
          <span class="tcat">{t(`auto.cat.${tpl.category}`)}</span>
          <b>{t(`notif.tpl.${tpl.id}`)}</b>
          <small>{t(`notif.tpl.${tpl.id}.hint`)}</small>
        </button>
      </li>
    {/each}
  </ul>
{/snippet}

{#if editing}
  <AutomationEditor initial={editing.item} isNew={editing.isNew} onclose={() => (editing = null)} onsaved={() => (editing = null)} />
{:else}
  <div class="head">
    <div>
      <h2>{t("nav.notifications")}</h2>
      <p class="sub">{t("nav.notificationsHint")}</p>
    </div>
    <span class="btns">
      <button class="primary" onclick={create} data-create>{t("auto.create")}</button>
      <button onclick={stopAll} data-stop title={t("auto.stopHint")}>{t("trg.stop")}</button>
    </span>
  </div>
  {#if paused.length}
    <p class="paused" role="status" data-paused-banner>
      <span>{t("auto.pausedBanner", { n: paused.length })}</span>
      <button onclick={resume} data-resume>{t("auto.resume")}</button>
    </p>
  {/if}
  <p class="muted lead">{t("auto.lead")}</p>
  <p class="muted small">
    {#if canEditProfile()}{t("notif.scopeProfile", { name: activeProfile.value?.name ?? "" })}{:else}{t("notif.scopeGlobalOnly")}{/if}
  </p>

  {#if !items.length}
    <section class="empty" data-empty>
      <h3>{t("auto.emptyTitle")}</h3>
      <p class="muted">{t("auto.emptyHint")}</p>
      {@render tplGrid(templates)}
      <button class="primary" onclick={create}>{t("auto.create")}</button>
    </section>
  {:else}
    {#each grouped as g (g.c)}
      <h3 class="group" data-group={g.c}>{t(`auto.cat.${g.c}s`)} <span class="count">{g.list.length}</span></h3>
      <ul class="cards">
        {#each g.list as i (i.name)}
          {@const a = previewOf(i.n)}
          {@const locked = i.scope === "profile" && !!activeProfile.value?.builtin}
          <li class="card" class:off={i.n.enabled === false} data-notif={i.name}>
            <div class="prev">
              {#if a && layout}<AnimThumb anim={a} {layout} {names} />{:else}<span class="muted small">{t("notif.noPreview")}</span>{/if}
            </div>
            <div class="body">
              <div class="title">
                <code>{i.name}</code>
                <span class="badge">{i.scope === "global" ? t("notif.allProfiles") : t("notif.thisProfile")}</span>
                {#if paused.includes(i.name)}<span class="badge warn" data-paused>{t("auto.paused")}</span>{/if}
              </div>
              <p class="chips" data-when-chips>{#each whenChips(i.n) as c (c)}<span class="chip when">{c}</span>{/each}</p>
              <p class="does">
                {doesLabel(i.n)}
                {#if !i.n.sequence}· {describeKeys(i.n.keys)}{/if}
                {#if i.n.color && !i.n.sequence}· <span class="dot" style:background={hex(rgbOf(i.n.color))}></span> {i.n.color.startsWith("@") ? i.n.color : ""}{/if}
              </p>
              {#if locked}<p class="muted small">{t("notif.builtinLocked")}</p>{/if}
              <div class="row">
                <label class="switch"><input type="checkbox" disabled={locked} checked={i.n.enabled !== false} onchange={(e) => toggle(i, e.currentTarget.checked)} aria-label={t("notif.enabled")} /> {t("notif.enabled")}</label>
                <button onclick={() => test(i)} data-test>{t("notif.test")}</button>
                <button onclick={() => (editing = { item: plain(i), isNew: false })} data-edit disabled={locked} title={locked ? t("notif.builtinLocked") : ""}>{t("notif.edit")}</button>
                <button class="danger" onclick={() => remove(i)} aria-label={t("common.delete")} disabled={locked} title={locked ? t("notif.builtinLocked") : ""}>✕</button>
              </div>
            </div>
          </li>
        {/each}
      </ul>
    {/each}
    <details class="more" data-templates>
      <summary>{t("auto.templates")}</summary>
      {@render tplGrid(templates)}
    </details>
  {/if}
{/if}

<style>
  .head { display: flex; justify-content: space-between; align-items: flex-start; gap: 0.6rem; flex-wrap: wrap; }
  .head h2 { margin: 0.2rem 0 0; }
  .sub { margin: 0; color: var(--muted); font-size: 0.9rem; }
  .btns { display: flex; gap: 0.4rem; flex-wrap: wrap; }
  .paused { display: flex; gap: 0.6rem; align-items: center; flex-wrap: wrap; margin: 0.6rem 0 0; padding: 0.45rem 0.7rem; border-radius: 10px; background: color-mix(in srgb, var(--warn, #e6a700) 18%, transparent); border: 1px solid color-mix(in srgb, var(--warn, #e6a700) 50%, transparent); font-size: 0.88rem; }
  .badge.warn { color: var(--text); border-color: var(--warn, #e6a700); }
  .lead { margin: 0.6rem 0 0.2rem; max-width: 46rem; }
  .small { font-size: 0.82rem; margin: 0 0 1rem; }
  .group { margin: 1.1rem 0 0.4rem; font-size: 1rem; display: flex; align-items: center; gap: 0.5rem; }
  .count { font-size: 0.72rem; background: var(--panel-2); border-radius: 999px; padding: 0.05rem 0.5rem; color: var(--muted); }
  .empty { background: var(--panel); border: 1px dashed var(--border); border-radius: 14px; padding: 1.2rem 1.4rem; max-width: 56rem; }
  .empty h3 { margin: 0 0 0.3rem; }
  .tpls { list-style: none; margin: 0.8rem 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 14rem), 1fr)); gap: 0.5rem; }
  .tpl { width: 100%; display: flex; flex-direction: column; align-items: flex-start; gap: 0.15rem; text-align: left; height: 100%; }
  .tpl small { color: var(--muted); font-size: 0.78rem; }
  .tcat { font-size: 0.66rem; text-transform: uppercase; letter-spacing: 0.06em; color: var(--accent); }
  .cards { list-style: none; margin: 0 0 1rem; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 24rem), 1fr)); gap: 0.8rem; }
  .card { display: grid; grid-template-columns: 8.5rem minmax(0, 1fr); gap: 0.8rem; background: var(--panel); border: 1px solid var(--border); border-radius: 12px; padding: 0.7rem; }
  .card.off { opacity: 0.65; }
  .prev { align-self: start; border-radius: 8px; overflow: hidden; background: var(--canvas-bg); min-height: 3rem; display: flex; align-items: center; justify-content: center; }
  .title { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; }
  .title code { font-size: 1rem; font-weight: 600; }
  .badge { font-size: 0.7rem; border: 1px solid var(--border); border-radius: 999px; padding: 0.05rem 0.5rem; color: var(--muted); }
  .does { margin: 0.25rem 0; font-size: 0.85rem; color: var(--muted); display: flex; gap: 0.35rem; align-items: center; flex-wrap: wrap; }
  .dot { width: 0.8rem; height: 0.8rem; border-radius: 50%; display: inline-block; border: 1px solid #ffffff40; }
  .chips { margin: 0.35rem 0 0.2rem; display: flex; gap: 0.3rem; flex-wrap: wrap; }
  .chip { font-size: 0.72rem; background: var(--panel-2); border-radius: 999px; padding: 0.1rem 0.55rem; }
  .chip.when { background: color-mix(in srgb, var(--accent) 22%, transparent); color: var(--text); font-weight: 600; }
  .row { display: flex; gap: 0.4rem; align-items: center; flex-wrap: wrap; margin-top: 0.4rem; }
  .switch { display: inline-flex; align-items: center; gap: 0.3rem; font-size: 0.85rem; margin-right: auto; }
  .more { margin-top: 0.5rem; }
  .more summary { cursor: pointer; color: var(--muted); }
  @media (max-width: 520px) { .card { grid-template-columns: minmax(0, 1fr); } .prev { max-width: 12rem; } }
</style>
