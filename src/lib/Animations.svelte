<script lang="ts">
  /** Animation library: browse, search, apply, test, edit, duplicate, rename, delete, import and export. */
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { api, type Animation } from "./api";
  import { isCompatible } from "./anim/ref";
  import { animName, applyAnimation, copyTriggerCommand, kindLabel, lib, loadAnimations, previewDevice, testAnimation } from "./animations.svelte";
  import { t } from "./i18n/index.svelte";
  import { buildLayout } from "./layout/buildLayout";
  import { devScene } from "./scene.svelte";
  import { app, confirmDialog, navigate, toast, toastError } from "./state.svelte";
  import AnimCard from "./AnimCard.svelte";

  type Filter = "all" | "builtin" | "mine" | "compatible";
  const FILTERS: Filter[] = ["all", "builtin", "mine", "compatible"];
  let query = $state("");
  let filter: Filter = $state("all");
  let renaming: string | null = $state(null);
  let renameText = $state("");

  const device = $derived(previewDevice());
  const layout = $derived(device ? buildLayout(device, { fanLeds: devScene(device.key).fan_leds }) : null);
  const names = $derived(device ? Array.from({ length: device.colors.length }, (_, i) => device.led_names[i] ?? "") : []);

  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return lib.list.filter((a) => {
      if (filter === "builtin" && !a.builtin) return false;
      if (filter === "mine" && a.builtin) return false;
      if (filter === "compatible" && !isCompatible(a, names)) return false;
      if (!q) return true;
      return animName(a).toLowerCase().includes(q) || a.name.toLowerCase().includes(q) || a.tags.some((x) => x.toLowerCase().includes(q));
    });
  });
  const hasMine = $derived(lib.list.some((a) => !a.builtin));

  const closeMenu = (e: Event) => ((e.currentTarget as HTMLElement).closest("details") as HTMLDetailsElement | null)?.removeAttribute("open");

  async function edit(a: Animation) {
    if (!device) return;
    let target = a;
    if (a.builtin) {
      try {
        target = await api.animDuplicate(a.id, uniqueName(`${animName(a)} (${t("anim.copySuffix")})`));
        await loadAnimations();
      } catch (e) {
        toastError(e);
        return;
      }
    }
    app.pendingEdit = target;
    await navigate({ kind: "device", key: device.key });
  }

  function uniqueName(base: string): string {
    const taken = new Set(lib.list.map((a) => a.name.toLowerCase()));
    let n = base;
    for (let i = 2; taken.has(n.toLowerCase()); i++) n = `${base} ${i}`;
    return n;
  }

  async function newAnimation() {
    if (!device) return;
    app.pendingEdit = "new";
    await navigate({ kind: "device", key: device.key });
  }

  function applyHere(a: Animation) {
    if (!device) return;
    applyAnimation([device.key], a);
    toast(t("anim.applied", { name: animName(a), device: device.name }), "ok");
  }

  async function applyAll(a: Animation, e: Event) {
    closeMenu(e);
    if (!(await confirmDialog(t("anim.confirmAll", { name: animName(a) }), t("anim.applyAll")))) return;
    applyAnimation(app.devices.map((d) => d.key), a);
    toast(t("anim.appliedAll", { name: animName(a) }), "ok");
  }

  async function duplicate(a: Animation, e: Event) {
    closeMenu(e);
    try {
      const c = await api.animDuplicate(a.id, uniqueName(`${animName(a)} (${t("anim.copySuffix")})`));
      await loadAnimations();
      toast(t("anim.duplicated", { name: c.name }), "ok");
    } catch (err) {
      toastError(err);
    }
  }

  function beginRename(a: Animation, e: Event) {
    closeMenu(e);
    renaming = a.id;
    renameText = a.name;
  }

  async function submitRename(a: Animation, e: SubmitEvent) {
    e.preventDefault();
    try {
      const full = await api.animGet(a.id);
      await api.animSave({ ...full, name: renameText });
      renaming = null;
      await loadAnimations();
      toast(t("anim.renamed"), "ok");
    } catch (err) {
      toastError(err);
    }
  }

  async function remove(a: Animation, e: Event) {
    closeMenu(e);
    if (!(await confirmDialog(t("anim.confirmDelete", { name: a.name }), t("common.delete")))) return;
    try {
      await api.animDelete(a.id);
      await loadAnimations();
      toast(t("anim.deleted"), "ok");
    } catch (err) {
      toastError(err);
    }
  }

  async function exportOne(a: Animation, e: Event) {
    closeMenu(e);
    try {
      const dest = await save({ defaultPath: `${a.name}.json`, filters: [{ name: "JSON", extensions: ["json"] }] });
      if (dest) {
        await api.animExport(a.id, dest);
        toast(t("anim.exported"), "ok");
      }
    } catch (err) {
      toastError(err);
    }
  }

  async function importFile() {
    try {
      const src = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
      if (typeof src === "string") {
        const a = await api.animImport(src);
        await loadAnimations();
        toast(t("anim.imported", { name: a.name }), "ok");
      }
    } catch (err) {
      toastError(err);
    }
  }

  $effect(() => {
    loadAnimations();
  });
</script>

<div class="head">
  <div>
    <h2>{t("nav.animations")}</h2>
    <p class="muted lead">{t("anim.lead")}</p>
  </div>
  <div class="headBtns">
    <button onclick={importFile}>{t("anim.import")}</button>
    <button class="primary" onclick={newAnimation} disabled={!device}>{t("anim.new")}</button>
  </div>
</div>

<div class="bar">
  <input type="search" class="search" placeholder={t("anim.search")} aria-label={t("anim.search")} bind:value={query} />
  <div class="chips" role="group" aria-label={t("anim.filter")}>
    {#each FILTERS as f (f)}
      <button class="pill" class:on={filter === f} aria-pressed={filter === f} onclick={() => (filter = f)}>{t(`anim.f.${f}`)}</button>
    {/each}
  </div>
  <label class="dev">
    <span class="muted">{t("anim.previewOn")}</span>
    <select value={device?.key ?? ""} onchange={(e) => (lib.targetKey = e.currentTarget.value)} aria-label={t("anim.previewOn")}>
      {#each app.devices as d (d.key)}<option value={d.key}>{d.name}</option>{/each}
    </select>
  </label>
</div>

{#if lib.corrupt.length}
  <p class="warn">{t("anim.corrupt", { files: lib.corrupt.join(", ") })}</p>
{/if}

{#if !hasMine && lib.loaded}
  <div class="empty">
    <div>
      <b>{t("anim.emptyTitle")}</b>
      <p class="muted">{t("anim.emptyHint")}</p>
    </div>
    <button class="primary" onclick={newAnimation} disabled={!device}>{t("anim.emptyAction")}</button>
  </div>
{/if}

{#if layout && device}
  <ul class="grid">
    {#each shown as a (a.id)}
      <li>
        <AnimCard anim={a} {layout} {names}>
          {#if renaming === a.id}
            <form class="rename" onsubmit={(e) => submitRename(a, e)}>
              <input type="text" bind:value={renameText} maxlength="64" required aria-label={t("anim.name")} />
              <button class="primary" type="submit">{t("common.ok")}</button>
              <button type="button" onclick={() => (renaming = null)}>{t("common.cancel")}</button>
            </form>
          {:else}
            <div class="actions">
              <button class="primary" onclick={() => applyHere(a)}>{t("anim.apply")}</button>
              <button onclick={() => testAnimation(a, device.key)} title={t("anim.testHint")}>{t("anim.test")}</button>
              {#if a.kind.type !== "effect" && a.kind.type !== "sequence"}
                <button onclick={() => edit(a)}>{a.builtin ? t("anim.customize") : t("anim.edit")}</button>
              {/if}
              <details class="menu">
                <summary aria-label={t("anim.more")} title={t("anim.more")}>⋯</summary>
                <div class="items" role="menu">
                  <button role="menuitem" onclick={(e) => applyAll(a, e)}>{t("anim.applyAll")}</button>
                  <button role="menuitem" onclick={(e) => duplicate(a, e)}>{t("anim.duplicate")}</button>
                  {#if !a.builtin}<button role="menuitem" onclick={(e) => beginRename(a, e)}>{t("anim.rename")}</button>{/if}
                  <button role="menuitem" onclick={(e) => exportOne(a, e)}>{t("anim.export")}</button>
                  <button role="menuitem" onclick={(e) => { closeMenu(e); copyTriggerCommand(a); }}>{t("anim.copyCommand")}</button>
                  {#if !a.builtin}<button role="menuitem" class="danger" onclick={(e) => remove(a, e)}>{t("common.delete")}</button>{/if}
                </div>
              </details>
            </div>
          {/if}
        </AnimCard>
      </li>
    {:else}
      <li class="none muted">{t("anim.noMatches")}</li>
    {/each}
  </ul>
{/if}

<style>
  .head { display: flex; justify-content: space-between; gap: 1rem; flex-wrap: wrap; align-items: flex-start; }
  .lead { margin: 0 0 0.8rem; max-width: 40rem; }
  .headBtns { display: flex; gap: 0.5rem; }
  .bar { display: flex; gap: 0.7rem; align-items: center; flex-wrap: wrap; margin-bottom: 1rem; }
  .search { min-width: 12rem; flex: 1 1 12rem; max-width: 20rem; padding: 0.4rem 0.6rem; }
  .chips { display: flex; gap: 0.3rem; flex-wrap: wrap; }
  .pill { border-radius: 999px; padding: 0.25rem 0.8rem; }
  .pill.on { background: var(--accent); color: var(--accent-fg); border-color: transparent; }
  .dev { display: flex; align-items: center; gap: 0.4rem; margin-left: auto; min-width: 0; }
  .dev select { min-width: 0; max-width: 14rem; }
  .warn { color: var(--danger); }
  .empty { display: flex; gap: 1rem; align-items: center; justify-content: space-between; flex-wrap: wrap; background: var(--panel); border: 1px dashed var(--border); border-radius: 12px; padding: 0.8rem 1rem; margin-bottom: 1rem; }
  .empty p { margin: 0.2rem 0 0; }
  .grid { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 15rem), 1fr)); gap: 0.8rem; }
  .none { grid-column: 1 / -1; padding: 1rem 0; }
  .actions { display: flex; gap: 0.35rem; flex-wrap: wrap; align-items: center; }
  .rename { display: flex; gap: 0.35rem; flex-wrap: wrap; }
  .rename input { flex: 1; min-width: 6rem; }
  .menu { position: relative; margin-left: auto; }
  .menu summary { list-style: none; cursor: pointer; background: var(--control); border: 1px solid var(--border); border-radius: 6px; padding: 0.35rem 0.6rem; line-height: 1; user-select: none; }
  .menu summary::-webkit-details-marker { display: none; }
  .menu[open] summary, .menu summary:hover { border-color: var(--accent); }
  .items { position: absolute; right: 0; bottom: calc(100% + 0.3rem); z-index: 5; background: var(--panel-2); border: 1px solid var(--border); border-radius: 8px; padding: 0.3rem; display: flex; flex-direction: column; gap: 0.15rem; min-width: 11rem; box-shadow: 0 8px 24px #0006; }
  .items button { text-align: left; background: transparent; border-color: transparent; }
  .items button:hover { background: var(--control); }
  .items .danger { color: var(--danger); }
</style>
