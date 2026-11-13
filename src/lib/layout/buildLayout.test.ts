import { describe, expect, it } from "vitest";
import { buildLayout } from "./buildLayout";
import type { Device } from "../api";

/** The real Razer Huntsman V3 Pro TKL: 114 LEDs, 19x6 matrix, many unnamed LEDs. */
function huntsman(): Device {
  const keys = [
    "Escape", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
    "Print Screen", "Scroll Lock", "Pause/Break", "`", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "=",
    "Backspace", "Insert", "Home", "Page Up", "Tab", "Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "[", "]",
    "\\ (ANSI)", "Delete", "End", "Page Down", "Caps Lock", "A", "S", "D", "F", "G", "H", "J", "K", "L", ";", "'",
    "Enter", "Left Shift", "Z", "X", "C", "V", "B", "N", "M", ",", ".", "/", "Right Shift", "Up Arrow",
    "Left Control", "Left Windows", "Left Alt", "Space", "Right Alt", "Right Fn", "Menu", "Right Control",
    "Left Arrow", "Down Arrow", "Right Arrow",
  ].map((k) => `Key: ${k}`);
  const names = [...keys, "Media: Volume Dial", "Key: Media Play/Pause", "Xbox Game Bar"];
  while (names.length < 114) names.push("");
  const map: (number | null)[] = Array(19 * 6).fill(null);
  keys.forEach((_, i) => (map[i] = i));
  return {
    id: 0, name: "Razer Huntsman V3 Pro TKL", vendor: "", description: "", version: "", serial: "", location: "",
    device_type: "Keyboard", active_mode: 0, modes: [], led_names: names, colors: names.map(() => [0, 0, 0]),
    key: "k", zones: [{ id: 0, name: "Keyboard", zone_type: "Matrix", num_leds: 114, leds_min: 114, leds_max: 114,
      resizable: false, offset: 0, matrix: { width: 19, height: 6, map } }],
  } as unknown as Device;
}

describe("keyboard layout", () => {
  const d = huntsman();
  const l = buildLayout(d);
  const by = (n: string) => l.items.find((i) => i.name === n)!;

  it("is detected as keyboard", () => expect(l.kind).toBe("keyboard"));
  it("WASD are 1u keys on the expected columns", () => {
    for (const k of ["W", "A", "S", "D"]) expect(by(`Key: ${k}`).w).toBe(1);
    expect(by("Key: A").x).toBeCloseTo(1.75);
    expect(by("Key: W").x).toBeCloseTo(2.5);
    expect(by("Key: W").y).toBeLessThan(by("Key: A").y);
  });
  it("wide keys have real widths", () => {
    expect(by("Key: Space").w).toBe(6.25);
    expect(by("Key: Left Shift").w).toBe(2.25);
    expect(by("Key: Right Shift").w).toBe(2.75);
    expect(by("Key: Enter").w).toBe(2.25);
    expect(by("Key: Backspace").w).toBe(2);
  });
  it("puts the media buttons above the navigation cluster: Xbox, Play/Pause, round volume dial", () => {
    const [x, p, v] = [by("Xbox Game Bar"), by("Key: Media Play/Pause"), by("Media: Volume Dial")];
    const esc = by("Key: Escape");
    expect(Math.max(x.y, p.y, v.y) + 1).toBeLessThanOrEqual(esc.y);
    expect(x.x).toBeLessThan(p.x);
    expect(p.x).toBeLessThan(v.x);
    expect(v.shape).toBe("circle");
    const navLeft = by("Key: Insert").x;
    const navRight = by("Key: Page Up").x + by("Key: Page Up").w;
    expect(x.x).toBeGreaterThanOrEqual(navLeft - 0.5);
    expect(v.x + v.w).toBeLessThanOrEqual(navRight + 1e-6);
    expect(v.x + v.w).toBeGreaterThan(navRight - 1.3);
  });
  it("falls back to a top-left extras row without a navigation cluster", () => {
    const d2 = huntsman();
    d2.led_names = d2.led_names.map((n) => (/Insert|Home|Page Up/.test(n) ? "" : n));
    const l2 = buildLayout(d2);
    const x = l2.items.find((i) => i.name === "Xbox Game Bar")!;
    expect(x.x).toBeLessThan(3.5);
    expect(x.y).toBeLessThan(l2.items.find((i) => i.name === "Key: Escape")!.y);
  });
  it("hides unnamed LEDs outside the matrix but keeps positions for all", () => {
    expect(l.hidden.length).toBe(114 - 87 - 3);
    expect(l.positions.length).toBe(114);
  });
  it("no two keys overlap", () => {
    const main = l.items.filter((i) => i.name.startsWith("Key: ") && !i.name.includes("Media"));
    for (const a of main) for (const b of main) {
      if (a === b) continue;
      const ox = a.x < b.x + b.w - 1e-6 && b.x < a.x + a.w - 1e-6;
      const oy = a.y < b.y + b.h - 1e-6 && b.y < a.y + a.h - 1e-6;
      expect(ox && oy, `${a.name} / ${b.name}`).toBe(false);
    }
  });
});

describe("generic layouts", () => {
  const base = { id: 1, name: "x", vendor: "", description: "", version: "", serial: "", location: "", device_type: "Cooler", active_mode: 0, modes: [] };
  it("draws a resizable zone as rings when a fan preset is set", () => {
    const d = { ...base, key: "c", zones: [{ id: 0, name: "H1", zone_type: "Linear", num_leds: 24, leds_min: 0, leds_max: 96, resizable: true, offset: 0, matrix: null }],
      led_names: Array(24).fill(""), colors: Array(24).fill([0, 0, 0]) } as unknown as Device;
    const l = buildLayout(d, { fanLeds: { H1: 8 } });
    expect(l.items.length).toBe(24);
    expect(l.items.every((i) => i.shape === "circle")).toBe(true);
    // 3 fans side by side: x spread must exceed a single ring.
    expect(l.width).toBeGreaterThan(6);
  });
  it("reports empty zones", () => {
    const d = { ...base, key: "m", zones: [{ id: 0, name: "ARGB", zone_type: "Linear", num_leds: 0, leds_min: 0, leds_max: 60, resizable: true, offset: 0, matrix: null },
      { id: 1, name: "LED_C", zone_type: "Single", num_leds: 1, leds_min: 1, leds_max: 1, resizable: false, offset: 0, matrix: null }],
      led_names: [""], colors: [[0, 0, 0]] } as unknown as Device;
    const l = buildLayout(d);
    expect(l.emptyZones).toEqual(["ARGB"]);
    expect(l.kind).toBe("single");
  });
});

describe("real Huntsman V3 Pro TKL fixture (dumped from OpenRGB)", async () => {
  const fixture = (await import("./fixtures/huntsman-v3-pro-tkl.json")).default as unknown as Device;
  const l = buildLayout(fixture);

  it("every LED with a name is drawn exactly once, at its own index", () => {
    const named = fixture.led_names.map((n, i) => [n, i] as const).filter(([n]) => n !== "");
    for (const [name, i] of named) {
      const items = l.items.filter((it) => it.led === i);
      expect(items.length, `${name} (#${i})`).toBe(1);
      expect(items[0].name).toBe(name);
    }
    // No LED index is drawn twice.
    const leds = l.items.map((it) => it.led);
    expect(new Set(leds).size).toBe(leds.length);
  });
  it("is a TKL: right edge at 18.25u, no numpad band", () => {
    expect(l.kind).toBe("keyboard");
    const keys = l.items.filter((it) => it.name.startsWith("Key: ") && it.shape === "rect" && it.w >= 1 && !it.name.includes("Media"));
    const right = Math.max(...keys.map((k) => k.x + k.w));
    expect(right).toBeCloseTo(18.25);
  });
  it("physical rows line up (shift row, space row)", () => {
    const by = (n: string) => l.items.find((i) => i.name === n)!;
    expect(by("Key: Left Shift").y).toBeCloseTo(by("Key: Z").y);
    expect(by("Key: Space").y).toBeCloseTo(by("Key: Left Alt").y);
    expect(by("Key: Space").x).toBeCloseTo(3.75);
    expect(by("Key: Right Shift").x + by("Key: Right Shift").w).toBeCloseTo(by("Key: Enter").x + by("Key: Enter").w);
    expect(by("Key: \\ (ANSI)").x + by("Key: \\ (ANSI)").w).toBeCloseTo(15);
  });
  it("engine positions cover every LED with values in 0..1", () => {
    expect(l.positions.length).toBe(fixture.colors.length);
    for (const p of l.positions) for (const v of p) { expect(v).toBeGreaterThanOrEqual(0); expect(v).toBeLessThanOrEqual(1); }
  });
});
