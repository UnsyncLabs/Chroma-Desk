<script lang="ts">
  /** Visual notification editor: what it shows, where, in which colour, and what fires it. */
  import { api, type Color, type Device, type EventHookStatus, type NamedTrigger } from "./api";
  import ColorPicker from "./ColorPicker.svelte";
  import DeviceIcon from "./DeviceIcon.svelte";
  import LedCanvas, { type Paint } from "./LedCanvas.svelte";
  import { animName, lib } from "./animations.svelte";
  import { t } from "./i18n/index.svelte";
  import { buildLayout } from "./layout/buildLayout";
  import { canEditProfile, plain, saveNotification, slugName, snippets, validName, type NotifItem, type Scope } from "./notifications.svelte";
  import { activeProfile, profiles, saveSettings } from "./profiles.svelte";
  import { app, copyText, toast, toastError } from "./state.svelte";

  let { initial, isNew, onclose, onsaved }: { initial: NotifItem; isNew: boolean; onclose: () => void; onsaved: () => void } = $props();

  // svelte-ignore state_referenced_locally
  const old = isNew ? undefined : { name: initial.name, scope: initial.scope };
  // svelte-ignore state_referenced_locally
  let name = $state(initial.name);
  // svelte-ignore state_referenced_locally
  let scope = $state<Scope>(initial.scope);
  // svelte-ignore state_referenced_locally
  let n = $state<NamedTrigger>(plain(initial.n));

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
  const wholeDevice = $derived(!n.keys?.length);

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

  // ---------- sources ----------
  let socket: string | null = $state(null);
  let snipTab = $state<"cli" | "socket" | "bash" | "python" | "hypr">("cli");
  let events = $state<EventHookStatus[]>([]);
  let omarchyHere = $state(false);
  let desktopStatus = $state("off");
  const snip = $derived(snippets(name || "my_notification", socket));
  $effect(() => {
    api.triggerSocketPath().then((p) => (socket = p)).catch(() => {});
    api.themeStatus().then((s) => (omarchyHere = !!s.sources.find((x) => x.id === "omarchy")?.detected)).catch(() => {});
    api.omarchyEventHooks().then((e) => (events = e)).catch(() => {});
    api.desktopStatus().then((s) => (desktopStatus = s)).catch(() => {});
  });
  const hasOmarchy = (ev: string) => !!n.sources?.omarchy?.includes(ev);
  function setOmarchy(ev: string, on: boolean) {
    const cur = new Set(n.sources?.omarchy ?? []);
    if (on) cur.add(ev);
    else cur.delete(ev);
    n.sources = { ...(n.sources ?? {}), omarchy: [...cur] };
  }
  function setDesktop(on: boolean) {
    n.sources = { ...(n.sources ?? {}), desktop: on ? { app: "", urgency: null, contains: "" } : null };
  }
  async function setListening(on: boolean) {
    profiles.settings.desktop_notifications = on;
    await saveSettings();
    desktopStatus = await api.desktopStatus().catch(() => (on ? "listening" : "off"));
  }
  async function hook(kind: string, install: boolean) {
    try {
      events = await (install ? api.omarchyEventHookInstall(kind) : api.omarchyEventHookRemove(kind));
      toast(t(install ? "themes.hookInstalled" : "themes.hookRemoved"), "ok", 2000);
    } catch (e) {
      toastError(e);
    }
  }

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
    if (out.sources && !out.sources.desktop && !out.sources.omarchy?.length) delete out.sources;
    if (out.sources && !out.sources.omarchy?.length) delete out.sources.omarchy;
    return out;
  };

  async function test() {
    try {
      const base = clean();
      const spec = { animation: base.anim, keys: base.keys, color: base.color ?? undefined, duration: base.duration ?? undefined, repeat: base.repeat ?? undefined, speed: base.speed ?? undefined, priority: base.priority, replace: base.replace };
      let devices = 0;
      let lastError: unknown = null;
      for (const d of base.devices?.length ? base.devices : [undefined]) {
        try {
          devices += (await api.trigger({ ...spec, device: d })).devices;
        } catch (e) {
          lastError = e;
        }
      }
      if (!devices) throw lastError;
      toast(t("notif.tested", { n: devices }), "ok", 2200);
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
    if (scope === "profile" && !canEditProfile()) {
      toast(t("notif.needProfile"), "error");
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
  <h2>{isNew ? t("notif.new") : t("notif.editing", { name: initial.name })}</h2>
</div>

<div class="form" data-notif-editor>
  <section>
    <h3>{t("notif.name")}</h3>
    <input type="text" bind:value={name} maxlength="32" aria-label={t("notif.name")} data-name onblur={() => (name = slugName(name) || name)} />
    <p class="muted small">{t("notif.nameHint")}</p>
    <div class="radios" role="radiogroup" aria-label={t("notif.scope")}>
      <label><input type="radio" name="scope" checked={scope === "profile"} disabled={!canEditProfile()} onchange={() => (scope = "profile")} data-scope="profile" /> {t("notif.thisProfileOpt", { name: activeProfile.value?.name ?? "" })}</label>
      <label><input type="radio" name="scope" checked={scope === "global"} onchange={() => (scope = "global")} data-scope="global" /> {t("notif.allProfilesOpt")}</label>
    </div>
    {#if !canEditProfile()}<p class="muted small">{t("notif.scopeGlobalOnly")}</p>{/if}
  </section>

  <section>
    <h3>{t("notif.what")}</h3>
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
  </section>

  <section>
    <h3>{t("notif.where")}</h3>
    <div class="devs" role="group" aria-label={t("notif.devices")}>
      <label class="dev all"><input type="checkbox" checked={everyDevice} onchange={(e) => (n.devices = e.currentTarget.checked ? [] : app.devices.slice(0, 1).map((d) => d.key))} data-every /> {t("notif.everyDevice")}</label>
      {#each app.devices as d (d.key)}
        <label class="dev"><input type="checkbox" checked={deviceOn(d.key)} onchange={(e) => toggleDevice(d.key, e.currentTarget.checked)} data-device={d.key} /><DeviceIcon type={d.device_type} /> <span>{d.name}</span></label>
      {/each}
    </div>
    {#if kb && layout}
      <label class="check"><input type="checkbox" checked={wholeDevice} onchange={(e) => setWhole(e.currentTarget.checked)} data-whole /> {t("notif.wholeDevice")}</label>
      {#if !wholeDevice}
        <p class="muted small">{t("notif.keysHint", { name: kb.name })}</p>
        <LedCanvas {layout} colors={blank} editable={false} tools={["select"]} bind:selected onchange={() => {}} />
      {/if}
    {/if}
  </section>

  <section>
    <h3>{t("notif.colour")}</h3>
    <div class="line">
      <ColorPicker value={parseColor(n.color)} onchange={(c) => (n.color = colorStr(c))} label={t("notif.colour")} />
      <label class="check"><input type="checkbox" checked={ownColors} onchange={(e) => (n.color = e.currentTarget.checked ? null : "@alert")} data-own-colours /> {t("notif.ownColours")}</label>
    </div>
    <p class="muted small">{t("notif.colourHint")}</p>
    <label class="line"><span>{t("notif.priority")}</span><input type="range" min="-10" max="10" step="1" value={n.priority ?? 0} oninput={(e) => (n.priority = Number(e.currentTarget.value))} aria-label={t("notif.priority")} /><output>{n.priority ?? 0}</output></label>
    <div class="radios" role="radiogroup" aria-label={t("notif.stacking")}>
      <label><input type="radio" name="stack" checked={!n.replace} onchange={() => (n.replace = false)} /> {t("notif.stack")}</label>
      <label><input type="radio" name="stack" checked={!!n.replace} onchange={() => (n.replace = true)} data-replace /> {t("notif.replace")}</label>
    </div>
  </section>

  <section>
    <h3>{t("notif.when")}</h3>
    <div class="src" data-source="command">
      <h4>{t("notif.src.command")}</h4>
      <p class="muted small">{t("notif.src.commandHint")}</p>
      <div class="tabs" role="tablist">
        {#each ["cli", "socket", "bash", "python", "hypr"] as k (k)}
          <button role="tab" aria-selected={snipTab === k} class:on={snipTab === k} data-snip-tab={k} onclick={() => (snipTab = k as typeof snipTab)}>{t(`notif.snip.${k}`)}</button>
        {/each}
      </div>
      <div class="cmd"><pre data-snippet={snipTab}>{snip[snipTab]}</pre><button onclick={() => copyText(snip[snipTab], t("trg.copied"))} data-copy>{t("trg.copy")}</button></div>
    </div>

    <div class="src" data-source="desktop">
      <label class="check"><input type="checkbox" checked={!!n.sources?.desktop} onchange={(e) => setDesktop(e.currentTarget.checked)} data-desktop /> <b>{t("notif.src.desktop")}</b></label>
      <p class="muted small">{t("notif.src.desktopHint")}</p>
      {#if n.sources?.desktop}
        <div class="fields">
          <label>{t("notif.desktop.app")} <input type="text" value={n.sources.desktop.app} oninput={(e) => n.sources?.desktop && (n.sources.desktop.app = e.currentTarget.value)} placeholder="discord" data-desktop-app /></label>
          <label>{t("notif.desktop.urgency")}
            <select value={n.sources.desktop.urgency ?? ""} onchange={(e) => n.sources?.desktop && (n.sources.desktop.urgency = (e.currentTarget.value || null) as never)}>
              <option value="">{t("notif.desktop.anyUrgency")}</option><option value="low">{t("notif.urg.low")}</option><option value="normal">{t("notif.urg.normal")}</option><option value="critical">{t("notif.urg.critical")}</option>
            </select>
          </label>
          <label>{t("notif.desktop.contains")} <input type="text" value={n.sources.desktop.contains} oninput={(e) => n.sources?.desktop && (n.sources.desktop.contains = e.currentTarget.value)} data-desktop-contains /></label>
        </div>
      {/if}
      <label class="check"><input type="checkbox" checked={profiles.settings.desktop_notifications} onchange={(e) => setListening(e.currentTarget.checked)} data-listen /> {t("notif.listen")}</label>
      <p class="privacy small">{t("notif.privacy")}</p>
      <p class="muted small">{t("notif.listenStatus")}: <b data-desktop-status>{desktopStatus}</b></p>
    </div>

    {#if omarchyHere}
      <div class="src" data-source="omarchy">
        <h4>{t("notif.src.omarchy")}</h4>
        <p class="muted small">{t("notif.src.omarchyHint")}</p>
        {#each events as e (e.kind)}
          <div class="ev" data-event={e.event}>
            <label class="check"><input type="checkbox" checked={hasOmarchy(e.event)} onchange={(ev) => setOmarchy(e.event, ev.currentTarget.checked)} /> {t(`notif.ev.${e.kind}`)}</label>
            <span class="state" class:ok={e.installed}>{e.installed ? t("notif.hookInstalled") : t("notif.hookMissing")}</span>
            {#if e.installed}<button onclick={() => hook(e.kind, false)} data-hook-remove={e.kind}>{t("themes.hookRemove")}</button>
            {:else}<button onclick={() => hook(e.kind, true)} data-hook-install={e.kind}>{t("themes.hookInstall")}</button>{/if}
            <details>
              <summary>{t("notif.hookWhat")}</summary>
              <p class="small">{t("notif.hookWill", { path: e.path })}</p>
              <code class="small">{e.command}</code>
              <pre>{e.script_text}</pre>
            </details>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <div class="foot">
    <button class="primary" onclick={save} data-save>{t("notif.save")}</button>
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
  h4 { margin: 0 0 0.2rem; }
  .small { font-size: 0.82rem; margin: 0.2rem 0 0.5rem; }
  .radios { display: flex; gap: 1rem; flex-wrap: wrap; margin-top: 0.4rem; }
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
  .src { border-top: 1px solid var(--border); padding: 0.7rem 0 0.3rem; margin-top: 0.5rem; }
  .tabs { display: flex; gap: 0.3rem; flex-wrap: wrap; margin: 0.4rem 0; }
  .tabs .on { background: var(--accent); color: var(--accent-fg); }
  .cmd { display: flex; gap: 0.5rem; align-items: flex-start; }
  .cmd pre { flex: 1; min-width: 0; margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; background: var(--panel-2); border: 1px solid var(--border); border-radius: 8px; padding: 0.5rem 0.7rem; font-size: 0.8rem; }
  .fields { display: grid; grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr)); gap: 0.5rem 1rem; margin: 0.4rem 0; }
  .fields label { display: flex; flex-direction: column; gap: 0.2rem; font-size: 0.85rem; color: var(--muted); }
  .privacy { background: var(--panel-2); border-left: 3px solid var(--accent); border-radius: 6px; padding: 0.4rem 0.7rem; }
  .ev { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; margin: 0.4rem 0; }
  .ev details { flex-basis: 100%; }
  .ev pre { background: var(--panel-2); border-radius: 8px; padding: 0.5rem; font-size: 0.75rem; overflow-x: auto; }
  .state { font-size: 0.78rem; color: var(--muted); }
  .state.ok { color: var(--ok); }
  .foot { display: flex; gap: 0.6rem; flex-wrap: wrap; position: sticky; bottom: 0; background: var(--bg); padding: 0.6rem 0; }
  @media (max-width: 560px) { .line { grid-template-columns: 1fr; } .line small { grid-column: 1; } }
</style>
