<script lang="ts">
  /** Profiles screen: save, apply, duplicate, rename, delete, default, import/export. */
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { api, hex, type ProfileInfo, type ThemeStatus } from "./api";
  import ThemeEditor from "./ThemeEditor.svelte";
  import { palette } from "./palette.svelte";
  import { t } from "./i18n/index.svelte";
  import { confirmDialog, toast, toastError } from "./state.svelte";
  import { isDirty } from "./scene.svelte";
  import { applyProfile, loadProfiles, profiles, saveProfileAs, saveSettings } from "./profilesStore.svelte";

  type Edit = { mode: "saveas" | "rename" | "duplicate"; target: string };
  let edit: Edit | null = $state(null);
  let name = $state("");
  let orgb: string[] = $state([]);
  let orgbPick = $state("");

  const current = $derived(profiles.settings.current_profile);
  /** Which profile's theme editor is open. */
  let editing: string | null = $state(null);
  let themeStat = $state<ThemeStatus | null>(null);
  $effect(() => {
    palette.generation;
    api.themeStatus().then((s) => (themeStat = s)).catch(() => {});
  });
  const detected = (src: string) => themeStat?.sources.find((x) => x.id === src);

  async function setEnabled(p: ProfileInfo, on: boolean) {
    try {
      await api.setBuiltinEnabled(p.name, on);
      await loadProfiles();
    } catch (e) {
      toastError(e);
    }
  }

  function begin(mode: Edit["mode"], target: string, initial: string) {
    edit = { mode, target };
    name = initial;
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!edit) return;
    try {
      const exists = profiles.list.profiles.some((p) => p.name === name.trim());
      if (edit.mode === "saveas" && exists && name.trim() !== current && !(await confirmDialog(t("prof.confirmOverwrite", { name: name.trim() }), t("prof.overwrite")))) return;
      if (edit.mode === "saveas") await saveProfileAs(name);
      else if (edit.mode === "rename") {
        await api.renameProfile(edit.target, name);
        await loadProfiles();
      } else {
        await api.duplicateProfile(edit.target, name);
        await loadProfiles();
      }
      toast(t("prof.done"), "ok");
      edit = null;
    } catch (err) {
      toastError(err);
    }
  }

  async function apply(p: ProfileInfo) {
    if (isDirty() && !(await confirmDialog(t("prof.confirmDiscard"), t("prof.apply")))) return;
    try {
      await applyProfile(p.name);
      toast(t("prof.applied", { name: p.name }), "ok");
    } catch (e) {
      toastError(e);
    }
  }

  async function remove(p: ProfileInfo) {
    if (!(await confirmDialog(t("prof.confirmDelete", { name: p.name }), t("common.delete")))) return;
    try {
      await api.deleteProfile(p.name);
      await loadProfiles();
    } catch (e) {
      toastError(e);
    }
  }

  async function toggleDefault(p: ProfileInfo) {
    profiles.settings.default_profile = profiles.settings.default_profile === p.name ? null : p.name;
    await saveSettings();
  }

  async function exportOne(p: ProfileInfo) {
    try {
      const dest = await save({ defaultPath: `${p.name}.json`, filters: [{ name: "JSON", extensions: ["json"] }] });
      if (dest) {
        await api.exportProfile(p.name, dest);
        toast(t("prof.exported"), "ok");
      }
    } catch (e) {
      toastError(e);
    }
  }

  async function importFile() {
    try {
      const src = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
      if (typeof src === "string") {
        const info = await api.importProfile(src);
        await loadProfiles();
        toast(t("prof.imported", { name: info.name }), "ok");
      }
    } catch (e) {
      toastError(e);
    }
  }

  async function loadOrgb() {
    try {
      orgb = await api.listOpenrgbProfiles();
      orgbPick = orgb[0] ?? "";
      if (!orgb.length) toast(t("prof.noOrgb"), "info");
    } catch (e) {
      toastError(e);
    }
  }

  async function importOrgb() {
    try {
      const info = await api.importOpenrgbProfile(orgbPick);
      await loadProfiles();
      toast(t("prof.imported", { name: info.name }), "ok");
    } catch (e) {
      toastError(e);
    }
  }

  $effect(() => {
    loadProfiles();
  });
</script>

<h2>{t("nav.profiles")}</h2>

<section class="bar">
  <span>
    {t("prof.current")}: <b>{current ?? t("prof.none")}</b>
    {#if isDirty()}<span class="dirty" title={t("prof.unsaved")}>● {t("prof.unsaved")}</span>{/if}
  </span>
  <span class="actions">
    {#if current}<button class="primary" onclick={() => saveProfileAs(current).then(() => toast(t("prof.done"), "ok")).catch(toastError)}>{t("prof.save")}</button>{/if}
    <button onclick={() => begin("saveas", "", current ?? "")}>{t("prof.saveAs")}</button>
  </span>
</section>

{#if edit}
  <form class="edit" onsubmit={submit}>
    <label>{t("prof.name")} <input type="text" bind:value={name} maxlength="64" required /></label>
    <button class="primary" type="submit">{t("common.ok")}</button>
    <button type="button" onclick={() => (edit = null)}>{t("common.cancel")}</button>
  </form>
{/if}

{#if !profiles.list.profiles.length}
  <p class="muted">{t("prof.empty")}</p>
{/if}

<ul class="list">
  {#each profiles.list.profiles as p (p.builtin ? `b:${p.name}` : p.file)}
    {@const live = p.source === "omarchy" ? detected("omarchy") : null}
    <li class:builtin={p.builtin} class:off={p.builtin && !p.enabled} data-profile={p.name}>
      <div class="top">
        <span class="n">
          <b>{p.name}</b>
          {#if p.builtin}<span class="badge b">{t("prof.badgeBuiltin")}</span>{/if}
          {#if p.name === current}<span class="badge">{t("prof.badgeCurrent")}</span>{/if}
          {#if p.name === profiles.settings.default_profile}<span class="badge def">{t("prof.badgeDefault")}</span>{/if}
          {#if p.openrgb}<span class="badge">OpenRGB</span>{/if}
        </span>
        <span class="theme" title={t("prof.themeSource")}>
          {#if p.strip.length}
            <span class="strip" aria-hidden="true">{#each p.strip as c, i (i)}<span style:background={hex(c)}></span>{/each}</span>
          {:else if live?.strip.length}
            <span class="strip" aria-hidden="true">{#each live.strip as c, i (i)}<span style:background={hex(c)}></span>{/each}</span>
          {/if}
          <small>{t(`src.${p.source}`)}{#if live?.name}&nbsp;·&nbsp;<b data-live-theme>{live.name}</b>{/if}</small>
        </span>
      </div>
      <span class="btns">
        {#if p.builtin}
          {#if p.requires}
            <label class="switch"><input type="checkbox" checked={p.enabled} onchange={(e) => setEnabled(p, e.currentTarget.checked)} aria-label={t("prof.enable")} /> {t("prof.enable")}</label>
          {/if}
          <button class="primary" onclick={() => apply(p)} disabled={!p.enabled}>{t("prof.apply")}</button>
          <button onclick={() => begin("duplicate", p.name, `${p.name} (copy)`)}>{t("prof.duplicateToEdit")}</button>
          <button onclick={() => (editing = editing === p.name ? null : p.name)}>{t("prof.theme")}</button>
        {:else}
          <button class="primary" onclick={() => apply(p)}>{t("prof.apply")}</button>
          <button onclick={() => (editing = editing === p.name ? null : p.name)}>{t("prof.editTheme")}</button>
          <button onclick={() => toggleDefault(p)}>{p.name === profiles.settings.default_profile ? t("prof.unsetDefault") : t("prof.setDefault")}</button>
          <button onclick={() => begin("duplicate", p.name, `${p.name} (2)`)}>{t("prof.duplicate")}</button>
          <button onclick={() => begin("rename", p.name, p.name)}>{t("prof.rename")}</button>
          <button onclick={() => exportOne(p)}>{t("prof.export")}</button>
          <button class="danger" onclick={() => remove(p)}>{t("common.delete")}</button>
        {/if}
      </span>
      {#if p.builtin && !p.enabled}<p class="muted small full">{t("prof.disabledHint")}</p>{/if}
      {#if editing === p.name}<div class="full"><ThemeEditor name={p.name} onclose={() => (editing = null)} /></div>{/if}
    </li>
  {/each}
</ul>

{#if profiles.list.corrupt.length}
  <p class="warn">{t("prof.corrupt", { files: profiles.list.corrupt.join(", ") })}</p>
{/if}

<section class="imports">
  <h3>{t("prof.import")}</h3>
  <div class="row">
    <button onclick={importFile}>{t("prof.importFile")}</button>
    <button onclick={loadOrgb}>{t("prof.importOrgb")}</button>
    {#if orgb.length}
      <select bind:value={orgbPick} aria-label={t("prof.importOrgb")}>{#each orgb as o (o)}<option value={o}>{o}</option>{/each}</select>
      <button class="primary" onclick={importOrgb}>{t("prof.import")}</button>
    {/if}
  </div>
  <p class="muted small">{t("prof.folder")}: <code>{profiles.path}</code></p>
</section>

<style>
  .bar { display: flex; justify-content: space-between; gap: 1rem; flex-wrap: wrap; align-items: center; background: var(--panel); border: 1px solid var(--border); border-radius: 10px; padding: 0.7rem 0.9rem; margin-bottom: 1rem; }
  .actions, .btns, .row { display: flex; gap: 0.4rem; flex-wrap: wrap; align-items: center; }
  .dirty { color: var(--danger); margin-left: 0.6rem; }
  .edit { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; margin-bottom: 1rem; }
  .list { list-style: none; margin: 0 0 1rem; padding: 0; display: flex; flex-direction: column; gap: 0.4rem; }
  .list li { display: flex; justify-content: space-between; gap: 0.6rem; flex-wrap: wrap; align-items: center; background: var(--panel); border: 1px solid var(--border); border-radius: 10px; padding: 0.6rem 0.8rem; }
  .list li.off { opacity: 0.8; }
  .top { display: flex; flex-direction: column; gap: 0.35rem; min-width: 0; }
  .theme { display: flex; align-items: center; gap: 0.6rem; color: var(--muted); }
  .strip { display: inline-flex; width: 8rem; height: 0.9rem; border-radius: 5px; overflow: hidden; border: 1px solid var(--border); }
  .strip span { flex: 1; }
  .badge.b { color: var(--accent-fg); background: var(--accent); border-color: var(--accent); }
  .switch { display: inline-flex; align-items: center; gap: 0.35rem; margin-right: 0.4rem; }
  .full { flex-basis: 100%; margin: 0; }
  .badge { font-size: 0.72rem; border: 1px solid var(--border); border-radius: 999px; padding: 0.05rem 0.5rem; margin-left: 0.3rem; color: var(--muted); }
  .badge.def { color: var(--accent); border-color: var(--accent); }
  .warn { color: var(--danger); }
  .small { font-size: 0.8rem; }
  .imports { margin-top: 1.4rem; }
</style>
