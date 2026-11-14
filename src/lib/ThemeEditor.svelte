<script lang="ts">
  /** Profile → Theme: colour source, palette slots, overrides, pinned presets. */
  import { api, hex, type AnimRef, type Profile, type ProfileTheme, type Rgb, type SourceStatus, type ThemeSourceId } from "./api";
  import ColorPicker from "./ColorPicker.svelte";
  import { t } from "./i18n/index.svelte";
  import { lib } from "./animations.svelte";
  import { DEFAULT_SLOTS, MAIN_SLOTS, SLOTS, palette } from "./palette.svelte";
  import { has, SOURCE_CAP } from "./capabilities.svelte";
  import { refreshActiveProfile } from "./profiles.svelte";
  import { navigate, toast, toastError } from "./state.svelte";

  let { name, onclose }: { name: string; onclose: () => void } = $props();

  const ALL_SOURCES: ThemeSourceId[] = ["manual", "omarchy", "kde", "pywal", "gtk", "external"];
  /** Sources that need a desktop this platform does not have are hidden (unless the profile already uses one). */
  const SOURCES = $derived(ALL_SOURCES.filter((id) => !SOURCE_CAP[id] || has(SOURCE_CAP[id]) || theme.source === id));
  let profile = $state<Profile | null>(null);
  let theme: ProfileTheme = $state({ source: "manual", manual_palette: null, overrides: {} });
  let presets: AnimRef[] = $state([]);
  let sources: SourceStatus[] = $state([]);
  let dirty = $state(false);
  let addPreset = $state("");
  let addOverride = $state("alert");

  $effect(() => {
    api.profileGet(name).then((p) => {
      profile = p;
      theme = { source: p.theme.source, manual_palette: p.theme.manual_palette ? { ...p.theme.manual_palette } : null, overrides: { ...(p.theme.overrides ?? {}) } };
      presets = (p.presets ?? []).map((x) => ({ ...x }));
      dirty = false;
    }).catch(toastError);
    api.themeStatus().then((s) => (sources = s.sources)).catch(() => {});
  });

  const readOnly = $derived(!!profile?.builtin);
  const status = (id: string) => sources.find((s) => s.id === id);
  /** The colours the slot grid shows: this profile's manual palette over the defaults. */
  const manual = $derived({ ...DEFAULT_SLOTS, ...(theme.manual_palette ?? {}) } as Record<string, Rgb>);
  const available = $derived(lib.list.filter((a) => !presets.some((p) => p.id === a.id)));
  const freeSlots = $derived(SLOTS.filter((s) => !(s in (theme.overrides ?? {}))));

  const touch = () => (dirty = true);
  function setSource(id: ThemeSourceId) {
    theme.source = id;
    touch();
  }
  function setSlot(slot: string, c: Rgb) {
    theme.manual_palette = { ...manual, [slot]: c };
    touch();
  }
  function resetPalette() {
    theme.manual_palette = null;
    touch();
  }
  function setOverride(slot: string, c: Rgb) {
    theme.overrides = { ...(theme.overrides ?? {}), [slot]: c };
    touch();
  }
  function removeOverride(slot: string) {
    const o = { ...(theme.overrides ?? {}) };
    delete o[slot];
    theme.overrides = o;
    touch();
  }
  function addPresetNow() {
    const a = lib.list.find((x) => x.id === addPreset);
    if (!a) return;
    presets = [...presets, { id: a.id, name: a.name }];
    addPreset = "";
    touch();
  }
  function move(i: number, d: number) {
    const j = i + d;
    if (j < 0 || j >= presets.length) return;
    const next = [...presets];
    [next[i], next[j]] = [next[j], next[i]];
    presets = next;
    touch();
  }

  async function save() {
    if (!profile) return;
    try {
      await api.profileUpdate(name, $state.snapshot(theme) as ProfileTheme, $state.snapshot(presets) as AnimRef[], profile.triggers ?? {});
      await refreshActiveProfile();
      dirty = false;
      toast(t("theme.saved"), "ok");
    } catch (e) {
      toastError(e);
    }
  }
</script>

<div class="editor" data-theme-editor>
  <div class="head">
    <h3>{t("theme.title", { name })}</h3>
    <button onclick={onclose}>{t("common.close")}</button>
  </div>
  {#if readOnly}<p class="muted">{t("theme.readOnly")}</p>{/if}

  <h4>{t("theme.source")}</h4>
  <div class="sources" role="radiogroup" aria-label={t("theme.source")}>
    {#each SOURCES as id (id)}
      {@const st = status(id)}
      <button class="src" class:on={theme.source === id} role="radio" aria-checked={theme.source === id} data-source={id} disabled={readOnly} onclick={() => setSource(id)}>
        <b>{t(`src.${id}`)}</b>
        <small class:ok={st?.detected} class:bad={st && !st.detected}>
          {#if id === "manual"}{t("theme.ownPalette")}{:else if id === "external"}{t("theme.fromGame")}{:else if st?.detected}{t("theme.detected")}{st.name ? ` · ${st.name}` : ""}{:else}{t("theme.notAvailable")}{/if}
        </small>
      </button>
    {/each}
  </div>
  <p class="muted small">{t(`src.${theme.source}.hint`)}</p>

  <h4>{t("theme.slots")}</h4>
  {#if theme.source === "manual"}
    <div class="slots">
      {#each [...MAIN_SLOTS, ...SLOTS.filter((s) => !MAIN_SLOTS.includes(s))] as s (s)}
        <label class="slot" data-slot-row={s}>
          <span>{t(`slot.${s}`)}</span>
          <ColorPicker value={manual[s]} allowSlots={false} onchange={(c) => setSlot(s, c as Rgb)} label={t(`slot.${s}`)} />
        </label>
      {/each}
    </div>
    <button class="small-btn" onclick={resetPalette} disabled={readOnly || !theme.manual_palette}>{t("theme.resetPalette")}</button>
  {:else}
    <p class="muted small">{t("theme.slotsDynamic", { name: palette.name })}</p>
    <div class="strip" aria-hidden="true">{#each SLOTS as s (s)}<span style:background={hex(palette.slots[s] ?? [0, 0, 0])} title={`@${s}`}></span>{/each}</div>
  {/if}

  <h4>{t("theme.overrides")}</h4>
  <p class="muted small">{t("theme.overridesHint")}</p>
  {#each Object.entries(theme.overrides ?? {}) as [s, c] (s)}
    <div class="ov" data-override={s}>
      <b>@{s}</b>
      <ColorPicker value={c} allowSlots={false} onchange={(nc) => setOverride(s, nc as Rgb)} label={t(`slot.${s}`)} />
      <button onclick={() => removeOverride(s)} disabled={readOnly} aria-label={t("common.delete")}>✕</button>
    </div>
  {/each}
  {#if !readOnly && freeSlots.length}
    <div class="ov add">
      <select bind:value={addOverride} aria-label={t("theme.overrideSlot")}>{#each freeSlots as s (s)}<option value={s}>{t(`slot.${s}`)}</option>{/each}</select>
      <button onclick={() => setOverride(freeSlots.includes(addOverride as never) ? addOverride : freeSlots[0], palette.slots[addOverride] ?? [255, 0, 0])}>{t("theme.addOverride")}</button>
    </div>
  {/if}

  <h4>{t("theme.presets")}</h4>
  <p class="muted small">{t("theme.presetsHint")}</p>
  <ul class="presets">
    {#each presets as p, i (p.id)}
      <li data-preset={p.id}>
        <span>{lib.list.find((a) => a.id === p.id)?.name ?? p.name}</span>
        <span class="btns">
          <button onclick={() => move(i, -1)} disabled={readOnly || i === 0} aria-label={t("common.up")}>↑</button>
          <button onclick={() => move(i, 1)} disabled={readOnly || i === presets.length - 1} aria-label={t("common.down")}>↓</button>
          <button onclick={() => { presets = presets.filter((x) => x.id !== p.id); touch(); }} disabled={readOnly} aria-label={t("theme.unpin")}>✕</button>
        </span>
      </li>
    {/each}
  </ul>
  {#if !readOnly}
    <div class="ov add">
      <select bind:value={addPreset} aria-label={t("theme.pickPreset")}>
        <option value="">{t("theme.pickPreset")}…</option>
        {#each available as a (a.id)}<option value={a.id}>{a.name}</option>{/each}
      </select>
      <button onclick={addPresetNow} disabled={!addPreset}>{t("theme.pin")}</button>
    </div>
  {/if}

  <h4>{t("theme.notifications")}</h4>
  <p class="muted small">{t("theme.notificationsHint", { n: Object.keys(profile?.triggers ?? {}).length })}</p>
  <button onclick={() => navigate({ kind: "notifications" })}>{t("theme.openNotifications")}</button>

  {#if !readOnly}
    <div class="foot">
      <button class="primary" onclick={save} disabled={!dirty}>{t("theme.save")}</button>
      {#if dirty}<span class="muted">{t("prof.unsaved")}</span>{/if}
    </div>
  {/if}
</div>

<style>
  .editor { background: var(--panel); border: 1px solid var(--border); border-radius: 12px; padding: 0.8rem 1rem 1rem; margin: 0.4rem 0 1rem; }
  .head { display: flex; justify-content: space-between; align-items: center; gap: 0.6rem; flex-wrap: wrap; }
  h3 { margin: 0.2rem 0; }
  h4 { margin: 1.1rem 0 0.3rem; }
  .small { font-size: 0.82rem; margin: 0.2rem 0 0.5rem; }
  .sources { display: grid; grid-template-columns: repeat(auto-fill, minmax(9.5rem, 1fr)); gap: 0.4rem; }
  .src { display: flex; flex-direction: column; align-items: flex-start; gap: 0.15rem; text-align: left; }
  .src.on { background: var(--accent); color: var(--accent-fg); border-color: var(--accent); }
  .src small { font-size: 0.72rem; opacity: 0.85; }
  .src small.ok { color: var(--ok); }
  .src.on small.ok { color: inherit; }
  .src small.bad { color: var(--muted); }
  .slots { display: grid; grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr)); gap: 0.35rem 0.8rem; }
  .slot { display: flex; align-items: center; justify-content: space-between; gap: 0.5rem; }
  .slot > span { color: var(--muted); font-size: 0.88rem; }
  .strip { display: flex; height: 1.6rem; border-radius: 6px; overflow: hidden; border: 1px solid var(--border); }
  .strip span { flex: 1; }
  .small-btn { margin-top: 0.6rem; }
  .ov { display: flex; align-items: center; gap: 0.6rem; margin: 0.3rem 0; flex-wrap: wrap; }
  .presets { list-style: none; padding: 0; margin: 0 0 0.5rem; display: flex; flex-direction: column; gap: 0.25rem; }
  .presets li { display: flex; justify-content: space-between; align-items: center; gap: 0.5rem; background: var(--panel-2); border-radius: 8px; padding: 0.3rem 0.6rem; }
  .btns { display: flex; gap: 0.25rem; }
  .foot { display: flex; gap: 0.8rem; align-items: center; margin-top: 1.1rem; }
</style>
