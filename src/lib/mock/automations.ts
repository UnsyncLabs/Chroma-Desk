/** The scheduler of the mock backend: always-on, every-N-seconds and threshold automations, like `automations.rs`. */
import type { NamedTrigger } from "../api";
import { fakeValue } from "./sysmon";

/** Port of `ThresholdState::update`. */
export class ThresholdState {
  since: number | null = null;
  armed = true;
  lastFire: number | null = null;
  update(cfg: { op: string; value: number; for_secs: number; hysteresis: number; cooldown_secs: number }, v: number | null, now: number): boolean {
    if (v === null) {
      this.since = null;
      return false;
    }
    const gt = cfg.op !== "lt";
    const holds = gt ? v > cfg.value : v < cfg.value;
    const released = gt ? v <= cfg.value - cfg.hysteresis : v >= cfg.value + cfg.hysteresis;
    if (!this.armed) {
      if (released) this.armed = true;
      this.since = null;
      return false;
    }
    if (!holds) {
      this.since = null;
      return false;
    }
    this.since ??= now;
    const cooled = this.lastFire === null || now - this.lastFire >= cfg.cooldown_secs * 1000;
    if (now - this.since >= cfg.for_secs * 1000 && cooled) {
      this.armed = false;
      this.since = null;
      this.lastFire = now;
      return true;
    }
    return false;
  }
}

interface Entry {
  json: string;
  nextEvery: number | null;
  th: ThresholdState;
}

export interface SchedCtx {
  notifications: () => Record<string, NamedTrigger>;
  /** Starts the named notification for `secs` seconds (null = its own length). */
  start: (name: string, secs: number | null) => void;
  /** Whether an overlay of that notification is still playing. */
  active: (name: string) => boolean;
  stop: (name: string) => void;
}

/** Always-on automations paused by "Stop all" (name -> JSON of the automation when paused). */
export const paused = new Map<string, string>();

const scheduled = (n: NamedTrigger) => n.enabled !== false && !!(n.sources?.always || n.sources?.every || n.sources?.threshold);

export function makeScheduler(ctx: SchedCtx) {
  const entries = new Map<string, Entry>();
  return {
    entries,
    tick(now = Date.now()) {
      const all = ctx.notifications();
      for (const [k, json] of [...paused]) if (!all[k] || JSON.stringify(all[k]) !== json) paused.delete(k);
      for (const [k, e] of [...entries]) {
        const n = all[k];
        if (!n || !scheduled(n) || JSON.stringify(n) !== e.json) {
          entries.delete(k);
          ctx.stop(k);
        }
      }
      for (const [name, n] of Object.entries(all)) {
        if (!scheduled(n)) continue;
        let e = entries.get(name);
        if (!e) {
          e = { json: JSON.stringify(n), nextEvery: n.sources?.every ? now + n.sources.every.every_secs * 1000 : null, th: new ThresholdState() };
          entries.set(name, e);
        }
        try {
          if (n.sources?.always && !paused.has(name) && !ctx.active(name)) ctx.start(name, 3600);
          const ev = n.sources?.every;
          if (ev && e.nextEvery !== null && now >= e.nextEvery) {
            e.nextEvery = now - e.nextEvery > ev.every_secs * 1000 ? now + ev.every_secs * 1000 : e.nextEvery + ev.every_secs * 1000;
            ctx.start(name, ev.for_secs);
          }
          const th = n.sources?.threshold;
          if (th && e.th.update(th, fakeValue(th.metric, now), now)) ctx.start(name, null);
        } catch {
          /* nothing to show it on */
        }
      }
    },
  };
}
