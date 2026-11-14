<script lang="ts">
  /** Visual notification editor: what it shows, where, in which colour, and what fires it. */
  import { api, type Animation, type Color, type Device, type NamedTrigger } from "./api";
  import ColorPicker from "./ColorPicker.svelte";
  import DeviceIcon from "./DeviceIcon.svelte";
  import LedCanvas, { type Paint } from "./LedCanvas.svelte";
  import SequenceEditor from "./SequenceEditor.svelte";
  import MonitorOptions from "./MonitorOptions.svelte";
  import WhenPicker from "./WhenPicker.svelte";
  import { animName, lib } from "./animationsStore.svelte";
  import { t } from "./i18n/index.svelte";
  import { buildLayout } from "./layout/buildLayout";
  import { canEditProfile, hasMeter, nameTaken, timingInvalid, plain, saveNotification, slugName, validName, type NotifItem, type Scope } from "./automationsStore.svelte";
  import { activeProfile } from "./profilesStore.svelte";
  import { app, toast, toastError } from "./state.svelte";

  let { initial, isNew, onclose, onsaved }: { initial: NotifItem; isNew: boolean; onclose: () => void; onsaved: () => void } = $props();

  // svelte-ignore state_referenced_locally
  const old = isNew ? undefined : { name: initial.name, scope: initial.scope };
  // svelte-ignore state_referenced_locally
  let name = $state(initial.name);
  // svelte-ignore state_referenced_locally
  let scope = $state<Scope>(initial.scope);
  // svelte-ignore state_referenced_locally
  let n = $state<NamedTrigger>(plain(initial.n));

  // ---------- what it shows: a library animation, or a sequence of steps ----------
  const blankSeq = (): Animation => ({ schema_version: 1, id: "", name: "sequence", tags: [], created: 0, updated: 1, target: { kind: "keyboard" }, builtin: false, kind: { type: "sequence", steps: [], loop: false } });
  // svelte-ignore state_referenced_locally
  let mode = $state<"animation" | "sequence">(initial.n.sequence ? "sequence" : "animation");
  // svelte-ignore state_referenced_locally
  let seq = $state<Animation>(initial.n.sequence ? plain(initial.n.sequence) : blankSeq());
  function setMode(m: "animation" | "sequence") {
    mode = m;
    if (m === "animation" && !n.anim) n.anim = "builtin:flash";
    if (m === "sequence" && seq.kind.type === "sequence" && !seq.kind.steps.length) {
      seq.kind.steps.push({ effect: "meter", params: { metric: "cpu.usage", color: { slot: "ok" }, min: 0, max: 100, style: "sweep", sweep_ms: 1200, background: "none" }, keys: [], duration_ms: 3200 });
    }
  }

  const STYLES = [
    { id: "flash", anim: "builtin:flash" },
    { id: "pulse", anim: "builtin:pulse" },
    { id: "blink", anim: "builtin:blink" },
    { id: "wave", anim: "builtin:theme-wave" },
    { id: "fill", anim: "builtin:fill" },
  ];
  const style = $derived(STYLES.find((s) => s.anim === n.anim)?.id ?? "");

  // ---------- colour ----------
  const parseColor = (c: string | null | undefined): Color => (c?.startsWith("@") ? { slot: c.slice(1) } : c ? ([1, 3, 5].map((i) => parseInt(c.slice(i, i + 2), 16)) as [number, number, number]) : { slot: "alert" });
  const colorStr = (c: Color): string => (Array.isArray(c) ? "#" + c.map((v) => v.toString(16).padStart(2, "0")).join("") : `@${c.slot}`);
  const ownColors = $derived(!n.color);

  // ---------- where ----------
  const everyDevice = $derived(!n.devices?.length);
  const targets = $derived(everyDevice ? app.devices : app.devices.filter((d) => n.devices?.includes(d.key)));
  const keyboards = $derived(targets.filter((d) => buildLayout(d).kind === "keyboard"));
  const kb: Device | null = $derived(keyboards[0] ?? null);
  const layout = $derived(kb ? buildLayout(kb) : null);
  const blank = $derived<Paint>(kb ? kb.colors.map(() => null) : []);
  // Picking stays on while the keyboard is shown, even if every key gets deselected (an empty list means the whole device when saved).
  // svelte-ignore state_referenced_locally
  let picking = $state(!!initial.n.keys?.length);
  const wholeDevice = $derived(!picking);

  const ledKey = (i: number) => {
    const nm = kb?.led_names[i] ?? "";
    return nm.startsWith("Key: ") ? nm.slice(5) : nm ? `name:${nm}` : `idx:${i}`;
  };
  function ledsOf(keys: string[]): Set<number> {
    const out = new Set<number>();
    if (!kb) return out;
    kb.led_names.forEach((nm, i) => {
      if (keys.some((k) => k === ledKey(i) || (k.startsWith("idx:") && Number(k.slice(4)) === i) || k.toLowerCase() === nm.toLowerCase() || `key: ${k.toLowerCase()}` === nm.toLowerCase())) out.add(i);
    });
    return out;
  }
  // svelte-ignore state_referenced_locally
  let selected = $state(ledsOf(initial.n.keys ?? []));
  // svelte-ignore state_referenced_locally
  let lastKeys = JSON.stringify(initial.n.keys ?? []);
  $effect(() => {
    if (!kb || wholeDevice) return;
    const keys = [...selected].sort((a, b) => a - b).map(ledKey);
    const sig = JSON.stringify(keys);
    if (sig !== lastKeys) {
      lastKeys = sig;
      n.keys = keys;
    }
  });
  function setWhole(on: boolean) {
    picking = !on;
    if (on) {
      n.keys = [];
      lastKeys = "[]";
    } else {
      // Start from the gaming cluster so the canvas shows something useful.
      selected = ledsOf(["W", "A", "S", "D"]);
      const keys = [...selected].map(ledKey);
      lastKeys = JSON.stringify(keys);
      n.keys = keys;
    }
  }
  function toggleDevice(key: string, on: boolean) {
    const cur = n.devices?.length ? [...n.devices] : app.devices.map((d) => d.key);
    const next = on ? [...new Set([...cur, key])] : cur.filter((k) => k !== key);
    n.devices = next.length === app.devices.length || !next.length ? [] : next;
  }
  const deviceOn = (key: string) => everyDevice || !!n.devices?.includes(key);

  // ---------- timing ----------
  const speed = $derived(n.speed ?? 1);
  const auto = $derived(n.duration == null);

  // A sequence with meters is a system monitor: it gets the display-mode control at the top.
  const isMonitor = $derived(mode === "sequence" && hasMeter(seq));

  // ---------- actions ----------
  const clean = (): NamedTrigger => {
    const out = plain(n) as NamedTrigger;
    if (!out.keys?.length) delete out.keys;
    if (!out.devices?.length) delete out.devices;
    if (!out.color) delete out.color;
    if (out.repeat == null || out.repeat < 1) delete out.repeat;
    if (out.speed == null || Math.abs(out.speed - 1) < 0.01) delete out.speed;
    if (!out.priority) delete out.priority;
    if (!out.replace) delete out.replace;
    const so = out.sources;
    if (so) {
      if (!so.omarchy?.length) delete so.omarchy;
      for (const k of ["desktop", "ai", "every", "threshold"] as const) if (!so[k]) delete so[k];
      if (!so.always) delete so.always;
      if (!Object.keys(so).length) delete out.sources;
    }
    if (mode === "sequence") {
      const s2 = plain(seq);
      s2.name = (slugName(name) || "sequence").slice(0, 64);
      out.sequence = s2;
      out.anim = "";
      for (const k of ["color", "repeat", "speed", "duration"] as const) delete out[k];
      // The keys the sequence uses, so devices without them are skipped.
      out.keys = s2.kind.type === "sequence" ? [...new Set(s2.kind.steps.flatMap((st) => st.keys))] : [];
      if (!out.keys.length) delete out.keys;
    } else {
      delete out.sequence;
    }
    return out;
  };

  async function test() {
    try {
      const nm = slugName(name) || "test";
      const r = await api.testAutomation(nm, clean());
      toast(t("notif.tested", { n: r.devices }), "ok", 2200);
    } catch (e) {
      toastError(e);
    }
  }

  async function save() {
    const nm = slugName(name) === name ? name : slugName(name);
    if (!validName(nm)) {
      toast(t("notif.badName"), "error");
      return;
    }
    if (timingInvalid(n)) {
      toast(t("mon.timingError"), "error");
      return;
    }
    if (scope === "profile" && !canEditProfile()) {
      toast(t("notif.needProfile"), "error");
      return;
    }
    // Renaming onto another notification would silently replace it.
    if (nameTaken(scope, nm) && !(old && old.name === nm && old.scope === scope)) {
      toast(t("notif.nameTaken", { name: nm }), "error");
      return;
    }
    try {
      await saveNotification({ name: nm, scope, n: clean() }, old);
      toast(t("notif.saved"), "ok", 2000);
      onsaved();
    } catch (e) {
      toastError(e);
    }
  }
</script>

<div class="top">
  <button onclick={onclose} data-back>← {t("notif.back")}</button>
  <h2>{isNew ? t("auto.new") : t("auto.editing", { name: initial.name })}</h2>
</div>

<div class="form" data-notif-editor>
  {#if isMonitor}
    <section data-monitor-section>
      <h3>{t("mon.title")}</h3>
      <MonitorOptions bind:n bind:seq />
    </section>
  {/if}

  <section>
    <h3>{t("notif.name")}</h3>
    <input type="text" bind:value={name} maxlength="32" aria-label={t("notif.name")} data-name onblur={() => (name = slugName(name) || name)} />
    <p class="muted small">{t("notif.nameHint")}</p>
    <div class="radios" role="radiogroup" aria-label={t("notif.scope")}>
      <label><input type="radio" name="scope" checked={scope === "profile"} disabled={!canEditProfile()} onchange={() => (scope = "profile")} data-scope="profile" /> {activeProfile.value && !activeProfile.value.builtin ? t("notif.thisProfileOpt", { name: activeProfile.value.name }) : t("notif.thisProfileNone")}</label>
      <label><input type="radio" name="scope" checked={scope === "global"} onchange={() => (scope = "global")} data-scope="global" /> {t("notif.allProfilesOpt")}</label>
    </div>
    {#if !canEditProfile()}<p class="muted small">{t("notif.scopeGlobalOnly")}</p>{/if}
  </section>

  <section>
    <h3>{t("notif.what")}</h3>
    <div class="modes" role="radiogroup" aria-label={t("auto.mode")}>
      <button class="mode" class:on={mode === "animation"} role="radio" aria-checked={mode === "animation"} onclick={() => setMode("animation")} data-mode="animation">
        <b>{t("auto.mode.animation")}</b><small>{t("auto.mode.animation.hint")}</small>
      </button>
      <button class="mode" class:on={mode === "sequence"} role="radio" aria-checked={mode === "sequence"} onclick={() => setMode("sequence")} data-mode="sequence">
        <b>{t("auto.mode.sequence")}</b><small>{t("auto.mode.sequence.hint")}</small>
      </button>
    </div>
    {#if mode === "sequence"}
      <SequenceEditor bind:seq {kb} {layout} />
    {:else}
      <div class="styles" role="radiogroup" aria-label={t("notif.style")}>
        {#each STYLES as s (s.id)}
          <button class="style" class:on={style === s.id} role="radio" aria-checked={style === s.id} data-style={s.id} onclick={() => (n.anim = s.anim)}>
            <b>{t(`notif.style.${s.id}`)}</b><small>{t(`notif.style.${s.id}.hint`)}</small>
          </button>
        {/each}
      </div>
      <label class="line">
        <span>{t("notif.fromLibrary")}</span>
        <select value={style ? "" : n.anim} onchange={(e) => e.currentTarget.value && (n.anim = e.currentTarget.value)} aria-label={t("notif.fromLibrary")} data-library>
          <option value="">{style ? t("notif.pickAnim") : ""}</option>
          {#each lib.list as a (a.id)}<option value={a.id}>{animName(a)}</option>{/each}
        </select>
      </label>
      <label class="line"><span>{t("notif.speed")}</span><input type="range" min="0.25" max="4" step="0.05" value={speed} oninput={(e) => (n.speed = Number(e.currentTarget.value))} aria-label={t("notif.speed")} /><output>{speed.toFixed(2)}×</output></label>
      <label class="line"><span>{t("notif.repeat")}</span><input type="number" min="0" max="20" value={n.repeat ?? 0} oninput={(e) => (n.repeat = Number(e.currentTarget.value) || undefined)} aria-label={t("notif.repeat")} /><small class="muted">{t("notif.repeatHint")}</small></label>
      <label class="line">
        <span>{t("notif.duration")}</span>
        <input type="range" min="0.5" max="30" step="0.5" disabled={auto} value={n.duration ?? 3} oninput={(e) => (n.duration = Number(e.currentTarget.value))} aria-label={t("notif.duration")} />
        <output>{auto ? t("notif.auto") : `${n.duration} s`}</output>
      </label>
      <label class="check"><input type="checkbox" checked={auto} onchange={(e) => (n.duration = e.currentTarget.checked ? undefined : 3)} /> {t("notif.autoDuration")}</label>
    {/if}
  </section>

  <section>
    <h3>{t("notif.where")}</h3>
    <div class="devs" role="group" aria-label={t("notif.devices")}>
      <label class="dev all"><input type="checkbox" checked={everyDevice} onchange={(e) => (n.devices = e.currentTarget.checked ? [] : app.devices.slice(0, 1).map((d) => d.key))} data-every /> {t("notif.everyDevice")}</label>
      {#each app.devices as d (d.key)}
        <label class="dev"><input type="checkbox" checked={deviceOn(d.key)} onchange={(e) => toggleDevice(d.key, e.currentTarget.checked)} data-device={d.key} /><DeviceIcon type={d.device_type} /> <span>{d.name}</span></label>
      {/each}
    </div>
    {#if mode === "sequence"}
      <p class="muted small">{t("auto.keysPerStep")}</p>
    {:else if kb && layout}
      <label class="check"><input type="checkbox" checked={wholeDevice} onchange={(e) => setWhole(e.currentTarget.checked)} data-whole /> {t("notif.wholeDevice")}</label>
      {#if !wholeDevice}
        <p class="muted small">{t("notif.keysHint", { name: kb.name })}</p>
        <LedCanvas {layout} colors={blank} editable={false} tools={["select"]} bind:selected onchange={() => {}} />
      {/if}
    {/if}
  </section>

  <section>
    <h3>{t("notif.colour")}</h3>
    {#if mode === "animation"}
      <div class="line">
        <ColorPicker value={parseColor(n.color)} onchange={(c) => (n.color = colorStr(c))} label={t("notif.colour")} />
        <label class="check"><input type="checkbox" checked={ownColors} onchange={(e) => (n.color = e.currentTarget.checked ? null : "@alert")} data-own-colours /> {t("notif.ownColours")}</label>
      </div>
      <p class="muted small">{t("notif.colourHint")}</p>
    {/if}
    <label class="line"><span>{t("notif.priority")}</span><input type="range" min="-10" max="100" step="1" value={n.priority ?? 0} oninput={(e) => (n.priority = Number(e.currentTarget.value))} aria-label={t("notif.priority")} /><output>{n.priority ?? 0}</output></label>
    <div class="radios" role="radiogroup" aria-label={t("notif.stacking")}>
      <label><input type="radio" name="stack" checked={!n.replace} onchange={() => (n.replace = false)} /> {t("notif.stack")}</label>
      <label><input type="radio" name="stack" checked={!!n.replace} onchange={() => (n.replace = true)} data-replace /> {t("notif.replace")}</label>
    </div>
  </section>

  <section>
    <h3>{t("auto.whenTitle")}</h3>
    <WhenPicker bind:n {name} hideTiming={isMonitor} />
  </section>

  <div class="foot">
    <button class="primary" onclick={save} data-save>{t("auto.save")}</button>
    <button onclick={test} data-test-editor>{t("notif.test")}</button>
    <button onclick={onclose}>{t("common.cancel")}</button>
  </div>
</div>

<style>
  .top { display: flex; align-items: center; gap: 0.8rem; flex-wrap: wrap; }
  .top h2 { margin: 0.3rem 0; }
  .form { display: flex; flex-direction: column; gap: 1rem; max-width: 52rem; }
  section { background: var(--panel); border: 1px solid var(--border); border-radius: 12px; padding: 0.8rem 1rem; }
  h3 { margin: 0 0 0.5rem; }
  .small { font-size: 0.82rem; margin: 0.2rem 0 0.5rem; }
  .radios { display: flex; gap: 1rem; flex-wrap: wrap; margin-top: 0.4rem; }
  .modes { display: grid; grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr)); gap: 0.4rem; margin-bottom: 0.8rem; }
  .mode { display: flex; flex-direction: column; align-items: flex-start; text-align: left; gap: 0.1rem; }
  .mode small { font-size: 0.74rem; opacity: 0.8; }
  .mode.on { background: var(--accent); color: var(--accent-fg); border-color: var(--accent); }
  .styles { display: grid; grid-template-columns: repeat(auto-fill, minmax(8.5rem, 1fr)); gap: 0.4rem; margin-bottom: 0.7rem; }
  .style { display: flex; flex-direction: column; align-items: flex-start; text-align: left; gap: 0.1rem; }
  .style small { font-size: 0.72rem; opacity: 0.8; }
  .style.on { background: var(--accent); color: var(--accent-fg); border-color: var(--accent); }
  .line { display: grid; grid-template-columns: 8rem minmax(0, 1fr) 4.5rem; gap: 0.6rem; align-items: center; margin: 0.4rem 0; }
  .line > span { color: var(--muted); font-size: 0.9rem; }
  .line small { grid-column: 3; }
  .line input[type="number"] { width: 5rem; }
  .line output { text-align: right; font-variant-numeric: tabular-nums; }
  div.line { display: flex; gap: 0.8rem; flex-wrap: wrap; }
  .check { display: flex; align-items: center; gap: 0.5rem; margin: 0.3rem 0; }
  .devs { display: grid; grid-template-columns: repeat(auto-fill, minmax(14rem, 1fr)); gap: 0.3rem 0.8rem; margin-bottom: 0.6rem; }
  .dev { display: flex; align-items: center; gap: 0.45rem; background: var(--panel-2); border-radius: 8px; padding: 0.3rem 0.6rem; }
  .dev span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dev.all { grid-column: 1 / -1; }
  .foot { display: flex; gap: 0.6rem; flex-wrap: wrap; position: sticky; bottom: 0; background: var(--bg); padding: 0.6rem 0; }
  @media (max-width: 560px) { .line { grid-template-columns: 1fr; } .line small { grid-column: 1; } }
</style>
