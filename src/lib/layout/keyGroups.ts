/** Quick-selection groups for keyboards, defined on exact OpenRGB LED names. */
export interface KeyGroup {
  id: string;
  /** i18n key of the group's display name. */
  label: string;
  match: (ledName: string) => boolean;
}

const key = (n: string) => n.replace(/^Key:\s*/, "");

export const KEY_GROUPS: KeyGroup[] = [
  { id: "wasd", label: "group.wasd", match: (n) => ["Key: W", "Key: A", "Key: S", "Key: D"].includes(n) },
  { id: "arrows", label: "group.arrows", match: (n) => /^Key: (Up|Down|Left|Right) Arrow$/.test(n) },
  { id: "fkeys", label: "group.fkeys", match: (n) => /^Key: F([1-9]|1[0-2])$/.test(n) },
  { id: "letters", label: "group.letters", match: (n) => /^Key: [A-Z]$/.test(n) },
  { id: "numbers", label: "group.numbers", match: (n) => /^Key: [0-9]$/.test(n) },
  {
    id: "modifiers",
    label: "group.modifiers",
    match: (n) => /^Key: (Left|Right) (Shift|Control|Alt|Windows|Fn)$/.test(n) || ["Key: Caps Lock", "Key: Tab", "Key: Menu"].includes(n),
  },
  { id: "numpad", label: "group.numpad", match: (n) => key(n).startsWith("Number Pad") || n === "Key: Num Lock" },
];
