<script lang="ts">
  /** "When does it run?": every way an automation can be triggered, as cards you switch on. */
  import { api, type EventHookStatus, type NamedTrigger, type NotifSources } from "./api";
  import ClaudeSetup from "./ClaudeSetup.svelte";
  import { t } from "./i18n/index.svelte";
  import { snippets } from "./automations.svelte";
  import { profiles, saveSettings } from "./profiles.svelte";
  import { copyText, toast, toastError } from "./state.svelte";
  import { METRICS, defaultRange, describeLive, metricLabel, sysmon, unitOf, watchMetrics } from "./sysmon.svelte";

  let { n = $bindable(), name }: { n: NamedTrigger; name: string } = $props();

  const src = $derived<NotifSources>(n.sources ?? {});
  const patch = (p: Partial<NotifSources>) => (n.sources = { ...(n.sources ?? {}), ...p });

  // ---------- command / socket ----------
  let socket: string | null = $state(null);
  let snipTab = $state<"cli" | "socket" | "bash" | "python" | "hypr">("cli");
  const snip = $derived(snippets(name || "my_automation", socket));

  // ---------- Omarchy, desktop ----------
  let events = $state<EventHookStatus[]>([]);
  let omarchyHere = $state(false);
  let desktopStatus = $state("off");
  $effect(() => {
    api.triggerSocketPath().then((p) => (socket = p)).catch(() => {});
    api.themeStatus().then((s) => (omarchyHere = !!s.sources.find((x) => x.id === "omarchy")?.detected)).catch(() => {});
    api.omarchyEventHooks().then((e) => (events = e)).catch(() => {});
    api.desktopStatus().then((s) => (desktopStatus = s)).catch(() => {});
  });
  const hasOmarchy = (ev: string) => !!src.omarchy?.includes(ev);
  function setOmarchy(ev: string, on: boolean) {
    const cur = new Set(src.omarchy ?? []);
    if (on) cur.add(ev);
    else cur.delete(ev);
    patch({ omarchy: [...cur] });
  }
  const setDesktop = (on: boolean) => patch({ desktop: on ? { app: "", urgency: null, contains: "" } : null });
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

  // ---------- every ----------
  const MULT = { s: 1, min: 60, h: 3600 } as const;
  type Unit = keyof typeof MULT;
  const unitFor = (secs: number): Unit => (secs >= 3600 && secs % 3600 === 0 ? "h" : secs >= 60 && secs % 60 === 0 ? "min" : "s");
  // svelte-ignore state_referenced_locally
  let everyUnit = $state<Unit>(unitFor(n.sources?.every?.every_secs ?? 300));
  const everyVal = $derived(Math.round(((src.every?.every_secs ?? 300) / MULT[everyUnit]) * 100) / 100);
  function setEvery(val: number, unit: Unit = everyUnit) {
    everyUnit = unit;
    const secs = Math.min(86400, Math.max(1, Math.round(val * MULT[unit])));
    patch({ every: { every_secs: secs, for_secs: src.every?.for_secs ?? 5 } });
  }

  // ---------- threshold ----------
  const th = $derived(src.threshold);
  const thMetric = $derived(th?.metric ?? "cpu.temp");
  $effect(() => {
    if (!th) return;
    return watchMetrics([th.metric]);
  });
  function enableThreshold() {
    const [lo, hi] = defaultRange("cpu.temp");
    patch({ threshold: { metric: "cpu.temp", op: "gt", value: Math.round((lo + hi) / 2 + 12), for_secs: 5, hysteresis: 2, cooldown_secs: 60 } });
  }
  function pickMetric(m: string) {
    if (!th) return;
    const [lo, hi] = defaultRange(m);
    patch({ threshold: { ...th, metric: m, value: m.endsWith(".temp") ? Math.round((lo + hi) / 2 + 12) : 80 } });
  }
  const setTh = (k: "value" | "for_secs" | "hysteresis" | "cooldown_secs", v: number) => th && patch({ threshold: { ...th, [k]: Number.isFinite(v) ? v : 0 } });

  // ---------- AI ----------
  const ai = $derived(src.ai);
</script>

<div class="when" data-when-picker>
  <p class="muted small">{t("auto.when.lead")}</p>

  <div class="card" class:on={!!src.always} data-when="always">
    <label class="head"><input type="checkbox" checked={!!src.always} onchange={(e) => patch({ always: e.currentTarget.checked })} data-when-always /> <b>{t("auto.when.always")}</b></label>
    <p class="muted small">{t("auto.when.alwaysHint")}</p>
  </div>

  <div class="card" class:on={!!src.every} data-when="every">
    <label class="head"><input type="checkbox" checked={!!src.every} onchange={(e) => (e.currentTarget.checked ? setEvery(everyVal) : patch({ every: null }))} data-when-every /> <b>{t("auto.when.every")}</b></label>
    <p class="muted small">{t("auto.when.everyHint")}</p>
    {#if src.every}
      <div class="fields">
        <label>{t("auto.when.everyLabel")}
          <span class="inline">
            <input type="number" min="1" value={everyVal} oninput={(e) => setEvery(Number(e.currentTarget.value) || 1)} data-every-value />
            <select value={everyUnit} onchange={(e) => setEvery(everyVal, e.currentTarget.value as Unit)} data-every-unit aria-label={t("auto.when.unit")}>
              <option value="s">{t("auto.u.s")}</option><option value="min">{t("auto.u.min")}</option><option value="h">{t("auto.u.h")}</option>
            </select>
          </span>
        </label>
        <label>{t("auto.when.forLabel")}
          <span class="inline"><input type="number" min="1" max="3600" value={src.every.for_secs} oninput={(e) => patch({ every: { every_secs: src.every?.every_secs ?? 300, for_secs: Math.max(1, Number(e.currentTarget.value) || 1) } })} data-every-for /> {t("auto.u.s")}</span>
        </label>
      </div>
    {/if}
  </div>

  <div class="card on" data-when="command" data-source="command">
    <div class="head"><b>{t("notif.src.command")}</b></div>
    <p class="muted small">{t("notif.src.commandHint")}</p>
    <div class="tabs" role="tablist">
      {#each ["cli", "socket", "bash", "python", "hypr"] as k (k)}
        <button role="tab" aria-selected={snipTab === k} class:on={snipTab === k} data-snip-tab={k} onclick={() => (snipTab = k as typeof snipTab)}>{t(`notif.snip.${k}`)}</button>
      {/each}
    </div>
    <div class="cmd"><pre data-snippet={snipTab}>{snip[snipTab]}</pre><button onclick={() => copyText(snip[snipTab], t("trg.copied"))} data-copy>{t("trg.copy")}</button></div>
  </div>

  <div class="card" class:on={!!th} data-when="threshold">
    <label class="head"><input type="checkbox" checked={!!th} onchange={(e) => (e.currentTarget.checked ? enableThreshold() : patch({ threshold: null }))} data-when-threshold /> <b>{t("auto.when.threshold")}</b></label>
    <p class="muted small">{t("auto.when.thresholdHint")}</p>
    {#if th}
      <div class="fields">
        <label>{t("seq.metric")}
          <select value={th.metric} onchange={(e) => pickMetric(e.currentTarget.value)} data-th-metric aria-label={t("seq.metric")}>
            {#each METRICS as m (m)}<option value={m}>{metricLabel(m)}</option>{/each}
          </select>
          <small class="live" data-th-live>{t("auto.when.now")}: <b>{describeLive(thMetric, sysmon.values[thMetric])}</b></small>
        </label>
        <label>{t("auto.when.op")}
          <select value={th.op} onchange={(e) => th && patch({ threshold: { ...th, op: e.currentTarget.value as "gt" | "lt" } })} data-th-op aria-label={t("auto.when.op")}>
            <option value="gt">{t("auto.when.above")}</option><option value="lt">{t("auto.when.below")}</option>
          </select>
        </label>
        <label>{t("auto.when.limit")}
          <span class="inline"><input type="number" value={th.value} oninput={(e) => setTh("value", Number(e.currentTarget.value))} data-th-value /> {unitOf(th.metric)}</span>
        </label>
        <label>{t("auto.when.holdFor")}
          <span class="inline"><input type="number" min="0" max="3600" value={th.for_secs} oninput={(e) => setTh("for_secs", Number(e.currentTarget.value))} data-th-for /> {t("auto.u.s")}</span>
        </label>
      </div>
      <details>
        <summary>{t("auto.when.more")}</summary>
        <div class="fields">
          <label>{t("auto.when.hysteresis")}
            <span class="inline"><input type="number" min="0" max="100" value={th.hysteresis} oninput={(e) => setTh("hysteresis", Number(e.currentTarget.value))} data-th-hyst /> {unitOf(th.metric)}</span>
            <small class="muted">{t("auto.when.hysteresisHint")}</small>
          </label>
          <label>{t("auto.when.cooldown")}
            <span class="inline"><input type="number" min="0" max="86400" value={th.cooldown_secs} oninput={(e) => setTh("cooldown_secs", Number(e.currentTarget.value))} data-th-cool /> {t("auto.u.s")}</span>
            <small class="muted">{t("auto.when.cooldownHint")}</small>
          </label>
        </div>
      </details>
    {/if}
  </div>

  <div class="card" class:on={!!src.desktop} data-when="desktop" data-source="desktop">
    <label class="head"><input type="checkbox" checked={!!src.desktop} onchange={(e) => setDesktop(e.currentTarget.checked)} data-desktop /> <b>{t("notif.src.desktop")}</b></label>
    <p class="muted small">{t("notif.src.desktopHint")}</p>
    {#if src.desktop}
      <div class="fields">
        <label>{t("notif.desktop.app")} <input type="text" value={src.desktop.app} oninput={(e) => n.sources?.desktop && (n.sources.desktop.app = e.currentTarget.value)} placeholder="discord, telegram" data-desktop-app /><small class="muted">{t("auto.when.appsHint")}</small></label>
        <label>{t("notif.desktop.urgency")}
          <select value={src.desktop.urgency ?? ""} onchange={(e) => n.sources?.desktop && (n.sources.desktop.urgency = (e.currentTarget.value || null) as never)}>
            <option value="">{t("notif.desktop.anyUrgency")}</option><option value="low">{t("notif.urg.low")}</option><option value="normal">{t("notif.urg.normal")}</option><option value="critical">{t("notif.urg.critical")}</option>
          </select>
        </label>
        <label>{t("notif.desktop.contains")} <input type="text" value={src.desktop.contains} oninput={(e) => n.sources?.desktop && (n.sources.desktop.contains = e.currentTarget.value)} data-desktop-contains /></label>
      </div>
    {/if}
    <label class="check"><input type="checkbox" checked={profiles.settings.desktop_notifications} onchange={(e) => setListening(e.currentTarget.checked)} data-listen /> {t("notif.listen")}</label>
    <p class="privacy small">{t("notif.privacy")}</p>
    <p class="muted small">{t("notif.listenStatus")}: <b data-desktop-status>{desktopStatus}</b></p>
  </div>

  {#if omarchyHere}
    <div class="card" class:on={!!src.omarchy?.length} data-when="omarchy" data-source="omarchy">
      <div class="head"><b>{t("notif.src.omarchy")}</b></div>
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

  <div class="card" class:on={!!ai} data-when="ai" data-source="ai">
    <label class="head"><input type="checkbox" checked={!!ai} onchange={(e) => patch({ ai: e.currentTarget.checked ? { from: "", kind: "" } : null })} data-when-ai /> <b>{t("auto.when.ai")}</b></label>
    <p class="muted small">{t("auto.when.aiHint")}</p>
    {#if ai}
      <div class="fields">
        <label>{t("auto.when.aiFrom")}
          <input type="text" list="ai-tools" value={ai.from} oninput={(e) => n.sources?.ai && (n.sources.ai.from = e.currentTarget.value)} placeholder={t("auto.when.anyTool")} data-ai-from />
          <datalist id="ai-tools"><option value="claude"></option><option value="codex"></option><option value="aider"></option></datalist>
        </label>
        <label>{t("auto.when.aiKind")}
          <select value={ai.kind} onchange={(e) => n.sources?.ai && (n.sources.ai.kind = e.currentTarget.value)} data-ai-kind>
            <option value="">{t("notif.desktop.anyUrgency")}</option><option value="permission">{t("auto.kind.permission")}</option><option value="idle">{t("auto.kind.idle")}</option>
          </select>
        </label>
      </div>
      <h5>{t("claude.title")}</h5>
      <ClaudeSetup />
    {/if}
  </div>
</div>

<style>
  .when { display: flex; flex-direction: column; gap: 0.6rem; }
  .small { font-size: 0.82rem; margin: 0.2rem 0 0.5rem; }
  .card { border: 1px solid var(--border); border-radius: 10px; padding: 0.55rem 0.8rem; background: var(--panel-2); }
  .card.on { border-color: var(--accent); }
  .head { display: flex; align-items: center; gap: 0.5rem; margin: 0; }
  h5 { margin: 0.6rem 0 0.2rem; }
  .fields { display: grid; grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr)); gap: 0.5rem 1rem; margin: 0.4rem 0; }
  .fields label { display: flex; flex-direction: column; gap: 0.2rem; font-size: 0.85rem; color: var(--muted); }
  .inline { display: flex; align-items: center; gap: 0.4rem; color: var(--text); }
  .inline input[type="number"] { width: 6rem; }
  .live { color: var(--muted); }
  .live b { color: var(--ok); }
  .check { display: flex; align-items: center; gap: 0.5rem; margin: 0.3rem 0; }
  .tabs { display: flex; gap: 0.3rem; flex-wrap: wrap; margin: 0.4rem 0; }
  .tabs .on { background: var(--accent); color: var(--accent-fg); }
  .cmd { display: flex; gap: 0.5rem; align-items: flex-start; }
  .cmd pre { flex: 1; min-width: 0; margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; background: var(--panel); border: 1px solid var(--border); border-radius: 8px; padding: 0.5rem 0.7rem; font-size: 0.8rem; }
  .privacy { background: var(--panel); border-left: 3px solid var(--accent); border-radius: 6px; padding: 0.4rem 0.7rem; }
  .ev { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; margin: 0.4rem 0; }
  .ev details { flex-basis: 100%; }
  .ev pre { background: var(--panel); border-radius: 8px; padding: 0.5rem; font-size: 0.75rem; overflow-x: auto; }
  .state { font-size: 0.78rem; color: var(--muted); }
  .state.ok { color: var(--ok); }
  details summary { cursor: pointer; color: var(--muted); }
</style>
