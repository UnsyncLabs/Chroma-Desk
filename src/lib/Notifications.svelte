<script lang="ts">
  /** Notifications: visual list, templates and the editor. */
  import { api, hex, type Animation } from "./api";
  import { t } from "./i18n/index.svelte";
  import { app, confirmDialog, toast, toastError } from "./state.svelte";
  import { animName, lib, loadAnimations } from "./animations.svelte";
  import { activeProfile, profiles } from "./profiles.svelte";
  import { palette, resolveColor } from "./palette.svelte";
  import { recolorAnim } from "./anim/recolor";
  import { buildLayout } from "./layout/buildLayout";
  import AnimThumb from "./AnimThumb.svelte";
  import NotifEditor from "./NotifEditor.svelte";
  import { plain, TEMPLATES, canEditProfile, deleteNotification, describeKeys, listNotifications, saveNotification, setEnabled, slugName, type NotifItem, type Scope } from "./notifications.svelte";

  let editing: { item: NotifItem; isNew: boolean } | null = $state(null);

  const items = $derived(listNotifications());
  const keyboard = $derived(app.devices.find((d) => buildLayout(d).kind === "keyboard") ?? app.devices[0] ?? null);
  const layout = $derived(keyboard ? buildLayout(keyboard) : null);
  const names = $derived(keyboard ? Array.from({ length: keyboard.colors.length }, (_, i) => keyboard.led_names[i] ?? "") : []);
  const defaultScope = $derived<Scope>(canEditProfile() ? "profile" : "global");

  $effect(() => {
    if (!lib.loaded) loadAnimations();
  });

  const animOf = (n: NotifItem["n"]): Animation | null => lib.list.find((a) => a.id === n.anim || a.name === n.anim) ?? null;
  /** The animation the way it will look: recoloured with the notification's colour. */
  function previewOf(n: NotifItem["n"]): Animation | null {
    const a = animOf(n);
    if (!a) return null;
    palette.generation;
    const c = n.color ? (n.color.startsWith("@") ? resolveColor({ slot: n.color.slice(1) }) : ([1, 3, 5].map((i) => parseInt((n.color as string).slice(i, i + 2), 16)) as [number, number, number])) : null;
    return c ? recolorAnim(a, c) : a;
  }

  function used(name: string) {
    return items.some((i) => i.name === name);
  }
  function freeName(base: string) {
    let n = slugName(base) || "notification";
    let i = 2;
    while (used(n)) n = `${slugName(base)}_${i++}`.slice(0, 32);
    return n;
  }

  function create() {
    editing = { item: { name: freeName("notification"), scope: defaultScope, n: { anim: "builtin:flash", color: "@accent", duration: 2 } }, isNew: true };
  }
  async function fromTemplate(id: string) {
    const tpl = TEMPLATES.find((x) => x.id === id);
    if (!tpl) return;
    const item: NotifItem = { name: freeName(tpl.name), scope: defaultScope, n: plain(tpl.n) };
    try {
      await saveNotification(item);
      toast(t("notif.created", { name: item.name }), "ok", 2500);
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
    if (!(await confirmDialog(t("notif.confirmDelete", { name: i.name }), t("common.delete")))) return;
    try {
      await deleteNotification(i);
    } catch (e) {
      toastError(e);
    }
  }
  async function stopAll() {
    try {
      toast(t("trg.stopped", { n: await api.stopTriggers() }), "ok", 2000);
    } catch (e) {
      toastError(e);
    }
  }

  const sourceChips = (n: NotifItem["n"]) => {
    const out: string[] = [t("notif.src.command")];
    if (n.sources?.desktop) out.push(`${t("notif.src.desktopShort")}${n.sources.desktop.app ? `: ${n.sources.desktop.app}` : ""}`);
    for (const e of n.sources?.omarchy ?? []) out.push(t(`notif.ev.${e.replace("omarchy:", "")}`));
    return out;
  };
</script>

{#if editing}
  <NotifEditor
    initial={editing.item}
    isNew={editing.isNew}
    onclose={() => (editing = null)}
    onsaved={() => (editing = null)}
  />
{:else}
  <div class="head">
    <h2>{t("nav.notifications")}</h2>
    <span class="btns">
      <button class="primary" onclick={create} data-create>{t("notif.create")}</button>
      <button onclick={stopAll}>{t("trg.stop")}</button>
    </span>
  </div>
  <p class="muted lead">{t("notif.lead")}</p>
  <p class="muted small">
    {#if canEditProfile()}{t("notif.scopeProfile", { name: activeProfile.value?.name ?? "" })}{:else}{t("notif.scopeGlobalOnly")}{/if}
  </p>

  {#if !items.length}
    <section class="empty" data-empty>
      <h3>{t("notif.emptyTitle")}</h3>
      <p class="muted">{t("notif.emptyHint")}</p>
      <ul class="tpls">
        {#each TEMPLATES as tpl (tpl.id)}
          <li>
            <button class="tpl" data-template={tpl.id} onclick={() => fromTemplate(tpl.id)}>
              <b>{t(`notif.tpl.${tpl.id}`)}</b>
              <small>{t(`notif.tpl.${tpl.id}.hint`)}</small>
            </button>
          </li>
        {/each}
      </ul>
      <button class="primary" onclick={create}>{t("notif.create")}</button>
    </section>
  {:else}
    <ul class="cards">
      {#each items as i (i.name)}
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
            </div>
            <p class="does">
              {a ? animName(a) : i.n.anim}
              · {describeKeys(i.n.keys)}
              {#if i.n.color}· <span class="dot" style:background={hex(resolveColor(i.n.color.startsWith("@") ? { slot: i.n.color.slice(1) } : ([1, 3, 5].map((k) => parseInt((i.n.color as string).slice(k, k + 2), 16)) as [number, number, number])))}></span> {i.n.color.startsWith("@") ? i.n.color : ""}{/if}
            </p>
            {#if locked}<p class="muted small">{t("notif.builtinLocked")}</p>{/if}
            <p class="chips">{#each sourceChips(i.n) as c (c)}<span class="chip">{c}</span>{/each}</p>
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
    <details class="more">
      <summary>{t("notif.moreTemplates")}</summary>
      <ul class="tpls">
        {#each TEMPLATES as tpl (tpl.id)}
          <li><button class="tpl" data-template={tpl.id} onclick={() => fromTemplate(tpl.id)}><b>{t(`notif.tpl.${tpl.id}`)}</b><small>{t(`notif.tpl.${tpl.id}.hint`)}</small></button></li>
        {/each}
      </ul>
    </details>
  {/if}
{/if}

<style>
  .head { display: flex; justify-content: space-between; align-items: center; gap: 0.6rem; flex-wrap: wrap; }
  .head h2 { margin: 0.2rem 0; }
  .btns { display: flex; gap: 0.4rem; flex-wrap: wrap; }
  .lead { margin: 0.2rem 0 0.2rem; max-width: 46rem; }
  .small { font-size: 0.82rem; margin: 0 0 1rem; }
  .empty { background: var(--panel); border: 1px dashed var(--border); border-radius: 14px; padding: 1.2rem 1.4rem; max-width: 52rem; }
  .empty h3 { margin: 0 0 0.3rem; }
  .tpls { list-style: none; margin: 0.8rem 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 14rem), 1fr)); gap: 0.5rem; }
  .tpl { width: 100%; display: flex; flex-direction: column; align-items: flex-start; gap: 0.15rem; text-align: left; height: 100%; }
  .tpl small { color: var(--muted); font-size: 0.78rem; }
  .cards { list-style: none; margin: 0 0 1rem; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 22rem), 1fr)); gap: 0.8rem; }
  .card { display: grid; grid-template-columns: 8.5rem minmax(0, 1fr); gap: 0.8rem; background: var(--panel); border: 1px solid var(--border); border-radius: 12px; padding: 0.7rem; }
  .card.off { opacity: 0.65; }
  .prev { align-self: start; border-radius: 8px; overflow: hidden; background: var(--canvas-bg); min-height: 3rem; display: flex; align-items: center; justify-content: center; }
  .title { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; }
  .title code { font-size: 1rem; font-weight: 600; }
  .badge { font-size: 0.7rem; border: 1px solid var(--border); border-radius: 999px; padding: 0.05rem 0.5rem; color: var(--muted); }
  .does { margin: 0.25rem 0; font-size: 0.88rem; color: var(--muted); display: flex; gap: 0.35rem; align-items: center; flex-wrap: wrap; }
  .dot { width: 0.8rem; height: 0.8rem; border-radius: 50%; display: inline-block; border: 1px solid #ffffff40; }
  .chips { margin: 0.2rem 0 0.5rem; display: flex; gap: 0.3rem; flex-wrap: wrap; }
  .chip { font-size: 0.72rem; background: var(--panel-2); border-radius: 999px; padding: 0.1rem 0.55rem; }
  .row { display: flex; gap: 0.4rem; align-items: center; flex-wrap: wrap; }
  .switch { display: inline-flex; align-items: center; gap: 0.3rem; font-size: 0.85rem; margin-right: auto; }
  .more { margin-top: 0.5rem; }
  .more summary { cursor: pointer; color: var(--muted); }
  @media (max-width: 520px) { .card { grid-template-columns: minmax(0, 1fr); } .prev { max-width: 12rem; } }
</style>
