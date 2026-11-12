/**
 * Physical key positions (in key units, 1u = a standard key) indexed by the exact LED
 * name OpenRGB uses (see OpenRGB `RGBController/RGBControllerKeyNames.cpp`).
 * Covers a full-size ANSI board; ISO differences are applied in `buildLayout`.
 */
export interface KeyRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export const KEYS: Record<string, KeyRect> = {};

const put = (name: string, x: number, y: number, w = 1, h = 1) => {
  KEYS[`Key: ${name}`] = { x, y, w, h };
};

// Row 0: Esc + function keys (gaps of half a key between groups of four).
put("Escape", 0, 0);
["F1", "F2", "F3", "F4"].forEach((n, i) => put(n, 2 + i, 0));
["F5", "F6", "F7", "F8"].forEach((n, i) => put(n, 6.5 + i, 0));
["F9", "F10", "F11", "F12"].forEach((n, i) => put(n, 11 + i, 0));
put("Print Screen", 15.25, 0);
put("Scroll Lock", 16.25, 0);
put("Pause/Break", 17.25, 0);

// Row 1: numbers.
put("`", 0, 1.5);
["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-"].forEach((n, i) => put(n, 1 + i, 1.5));
put("=", 12, 1.5);
put("+", 12, 1.5); // alias used by some drivers
put("Backspace", 13, 1.5, 2);
put("Insert", 15.25, 1.5);
put("Home", 16.25, 1.5);
put("Page Up", 17.25, 1.5);

// Row 2: QWERTY.
put("Tab", 0, 2.5, 1.5);
["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "[", "]"].forEach((n, i) => put(n, 1.5 + i, 2.5));
put("\\", 13.5, 2.5, 1.5);
put("\\ (ANSI)", 13.5, 2.5, 1.5);
put("Delete", 15.25, 2.5);
put("End", 16.25, 2.5);
put("Page Down", 17.25, 2.5);

// Row 3: home row.
put("Caps Lock", 0, 3.5, 1.75);
["A", "S", "D", "F", "G", "H", "J", "K", "L", ";", "'"].forEach((n, i) => put(n, 1.75 + i, 3.5));
put("#", 12.75, 3.5);
put("Enter", 12.75, 3.5, 2.25);
put("Enter (ISO)", 13.75, 2.5, 1.25, 2);

// Row 4: bottom letter row.
put("Left Shift", 0, 4.5, 2.25);
put("\\ (ISO)", 1.25, 4.5);
["Z", "X", "C", "V", "B", "N", "M", ",", ".", "/"].forEach((n, i) => put(n, 2.25 + i, 4.5));
put("Right Shift", 12.25, 4.5, 2.75);
put("Up Arrow", 16.25, 4.5);

// Row 5: modifiers + space.
put("Left Control", 0, 5.5, 1.25);
put("Left Windows", 1.25, 5.5, 1.25);
put("Left Fn", 1.25, 5.5, 1.25);
put("Left Alt", 2.5, 5.5, 1.25);
put("Space", 3.75, 5.5, 6.25);
put("Right Alt", 10, 5.5, 1.25);
put("Right Windows", 11.25, 5.5, 1.25);
put("Right Fn", 11.25, 5.5, 1.25);
put("Menu", 12.5, 5.5, 1.25);
put("Right Control", 13.75, 5.5, 1.25);
put("Left Arrow", 15.25, 5.5);
put("Down Arrow", 16.25, 5.5);
put("Right Arrow", 17.25, 5.5);

// Numpad.
put("Num Lock", 18.5, 1.5);
put("Number Pad /", 19.5, 1.5);
put("Number Pad *", 20.5, 1.5);
put("Number Pad -", 21.5, 1.5);
["7", "8", "9"].forEach((n, i) => put(`Number Pad ${n}`, 18.5 + i, 2.5));
put("Number Pad +", 21.5, 2.5, 1, 2);
["4", "5", "6"].forEach((n, i) => put(`Number Pad ${n}`, 18.5 + i, 3.5));
["1", "2", "3"].forEach((n, i) => put(`Number Pad ${n}`, 18.5 + i, 4.5));
put("Number Pad Enter", 21.5, 4.5, 1, 2);
put("Number Pad 0", 18.5, 5.5, 2);
put("Number Pad .", 20.5, 5.5);

/** Short label for a key cap. */
const SHORT: Record<string, string> = {
  Escape: "Esc",
  "Print Screen": "PrtSc",
  "Scroll Lock": "ScrLk",
  "Pause/Break": "Pause",
  Backspace: "⌫",
  "Page Up": "PgUp",
  "Page Down": "PgDn",
  "Caps Lock": "Caps",
  "Left Shift": "Shift",
  "Right Shift": "Shift",
  "Left Control": "Ctrl",
  "Right Control": "Ctrl",
  "Left Alt": "Alt",
  "Right Alt": "Alt",
  "Left Windows": "Win",
  "Right Windows": "Win",
  "Left Fn": "Fn",
  "Right Fn": "Fn",
  "Up Arrow": "↑",
  "Down Arrow": "↓",
  "Left Arrow": "←",
  "Right Arrow": "→",
  "Enter (ISO)": "Enter",
  "\\ (ANSI)": "\\",
  "\\ (ISO)": "\\",
  Space: "",
  "Num Lock": "Num",
};

export function shortLabel(ledName: string): string {
  const base = ledName.replace(/^(Key|Media|Keyboard):\s*/i, "");
  if (base in SHORT) return SHORT[base];
  return base.replace(/^Number Pad /, "").replace(/^Media /, "");
}
