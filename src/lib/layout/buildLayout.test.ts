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
  it("puts extras in a row above the keys", () => {
    const dial = by("Media: Volume Dial");
    expect(dial.y).toBeLessThan(by("Key: Escape").y);
    expect(by("Xbox Game Bar").y).toBeLessThan(by("Key: Escape").y);
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
