<script lang="ts">
  /** "All devices": master brightness, lights off, quick colour and the global synchronised scene. */
  import { api, fromHex, type Device, type Rgb } from "./api";
  import { t, deviceLabel } from "./i18n/index.svelte";
  import { app, confirmDialog, toast, toastError } from "./state.svelte";
  import { commit, devScene, ensureDev, frames, newLayer, sceneStore } from "./scene.svelte";
  import LayersPanel from "./LayersPanel.svelte";
  import Strip from "./Strip.svelte";

  const scene = $derived(sceneStore.scene);
  let color = $state("#da1d1d");
  let globalSel: string | null = $state(null);
  let dragFrom = $state(-1);

  /** Devices in global-canvas order: saved order first, then the rest. */
  const ordered = $derived.by(() => {
    const byKey = new Map(app.devices.map((d) => [d.key, d]));
    const out: Device[] = [];
    for (const k of scene.order) {
      const d = byKey.get(k);
      if (d) {
        out.push(d);
        byKey.delete(k);
      }
    }
    return [...out, ...byKey.values()];
  });

  function setOrder(list: Device[]) {
    scene.order = list.map((d) => d.key);
    commit();
  }

  function moveDevice(i: number, dir: -1 | 1) {
    const list = [...ordered];
    const j = i + dir;
    if (j < 0 || j >= list.length) return;
    [list[i], list[j]] = [list[j], list[i]];
    setOrder(list);
  }

  function dropOn(i: number) {
    if (dragFrom < 0 || dragFrom === i) return;
    const list = [...ordered];
    const [m] = list.splice(dragFrom, 1);
    list.splice(i, 0, m);
    dragFrom = -1;
    setOrder(list);
  }

  async function applyColor() {
    if (!(await confirmDialog(t("all.confirmColor"), t("all.apply")))) return;
    const c = fromHex(color);
    for (const d of app.devices) {
      const ds = ensureDev(d.key);
      ds.mode = "effects";
      ds.layers = [newLayer("solid", { params: { color: [...c] as Rgb } })];
    }
    commit();
    toast(t("all.applied"), "ok");
  }

  async function toggleBlackout() {
    const on = !sceneStore.blackout;
    try {
      await api.setBlackout(on);
      sceneStore.blackout = on;
    } catch (e) {
      toastError(e);
    }
  }

  /** Enables the global scene: devices that follow it are switched to Effects mode. */
  async function enableSync() {
    if (!(await confirmDialog(t("sync.confirmEnable"), t("sync.enable")))) return false;
    for (const d of app.devices) if (devScene(d.key).follow_global) ensureDev(d.key).mode = "effects";
    if (!scene.global_layers.length) {
      const l = newLayer("rainbow");
      scene.global_layers.push(l);
      globalSel = l.id;
    }
    scene.sync = true;
    commit();
    return true;
  }
</script>

<h2>{t("nav.all")}</h2>

<section class="bar">
  <label>{t("all.master")}
    <input type="range" min="0" max="1" step="0.01" value={scene.master_brightness} oninput={(e) => { scene.master_brightness = Number(e.currentTarget.value); commit(); }} />
    {Math.round(scene.master_brightness * 100)}%
  </label>
  <button class={sceneStore.blackout ? "primary" : ""} onclick={toggleBlackout}>{sceneStore.blackout ? t("all.lightsOn") : t("all.lightsOff")}</button>
  <span class="quick">
    <input type="color" bind:value={color} aria-label={t("all.color")} />
    <button onclick={applyColor}>{t("all.apply")}</button>
  </span>
</section>

<section>
  <h3>{t("sync.title")}</h3>
  <p class="muted">{t("sync.hint")}</p>
  <label class="check">
    <input
      type="checkbox"
      checked={scene.sync}
      onchange={async (e) => {
        const box = e.currentTarget;
        if (box.checked) {
          if (!(await enableSync())) box.checked = false;
        } else {
          scene.sync = false;
          commit();
        }
      }}
    />
    {t("sync.enable")}
  </label>

  {#if scene.sync}
    <div class="syncgrid">
      <LayersPanel layers={scene.global_layers} oncommit={commit} bind:selectedId={globalSel} allowPaint={false} />
      <div class="order">
        <h4>{t("sync.order")}</h4>
        <ul>
          {#each ordered as d, i (d.key)}
            {@const ds = devScene(d.key)}
            <li draggable="true" ondragstart={() => (dragFrom = i)} ondragover={(e) => e.preventDefault()} ondrop={() => dropOn(i)} ondragend={() => (dragFrom = -1)}>
              <span class="grip" aria-hidden="true">⠿</span>
              <label class="follow" title={t("dev.followGlobal")}>
                <input type="checkbox" checked={ds.follow_global} onchange={(e) => { const x = ensureDev(d.key); x.follow_global = e.currentTarget.checked; if (x.follow_global) x.mode = "effects"; commit(); }} aria-label={t("dev.followGlobal")} />
                <span class="n">{d.name}</span>
              </label>
              <button onclick={() => moveDevice(i, -1)} disabled={i === 0} aria-label={t("layers.up")}>↑</button>
              <button onclick={() => moveDevice(i, 1)} disabled={i === ordered.length - 1} aria-label={t("layers.down")}>↓</button>
            </li>
          {/each}
        </ul>
        <p class="muted small">{t("sync.effectsOnly")}</p>
      </div>
    </div>
  {/if}
</section>

<section>
  <h3>{t("nav.devices")}</h3>
  <ul class="cards">
    {#each app.devices as d (d.key)}
      <li>
        <button class="card" onclick={() => (app.view = { kind: "device", key: d.key })}>
          <b>{d.name}</b>
          <small class="muted">{deviceLabel(d.device_type)} · {d.colors.length} LEDs · {devScene(d.key).mode === "effects" ? t("mode.effects") : t("mode.hardware")}</small>
          <Strip colors={frames.map[d.key] ?? d.colors} />
        </button>
      </li>
    {/each}
  </ul>
</section>

<style>
  section { margin-bottom: 1.6rem; }
  .bar { display: flex; gap: 1rem; flex-wrap: wrap; align-items: center; background: var(--panel); border: 1px solid var(--border); border-radius: 10px; padding: 0.7rem 0.9rem; }
  .bar label { display: flex; gap: 0.5rem; align-items: center; }
  .quick { display: flex; gap: 0.4rem; align-items: center; margin-left: auto; }
  .quick input { width: 2.2rem; height: 1.9rem; padding: 0; border: none; background: none; }
  .check { display: flex; gap: 0.5rem; align-items: center; margin-bottom: 0.7rem; }
  .syncgrid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(14rem, 18rem); gap: 1rem; align-items: start; }
  .order { background: var(--panel); border: 1px solid var(--border); border-radius: 10px; padding: 0.8rem; }
  .order h4 { margin: 0 0 0.5rem; }
  .order ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.25rem; }
  .order li { display: flex; align-items: center; gap: 0.3rem; cursor: grab; }
  .follow { display: flex; align-items: center; gap: 0.4rem; flex: 1; min-width: 0; }
  .n { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .grip { color: var(--muted); }
  .small { font-size: 0.8rem; }
  .cards { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(15rem, 1fr)); gap: 0.6rem; }
  .card { width: 100%; display: flex; flex-direction: column; gap: 0.35rem; text-align: left; padding: 0.6rem 0.8rem; }
  @media (max-width: 800px) { .syncgrid { grid-template-columns: minmax(0, 1fr); } .quick { margin-left: 0; } }
</style>
