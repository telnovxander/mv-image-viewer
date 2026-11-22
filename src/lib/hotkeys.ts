// Keyboard shortcuts of Spiegelmond in one table (IV/PROJECT.md, "Горячие клавиши"): the "?" help
// shows it; a key is `done: true` once the task that implements it is in.
import table from "./hotkeys.json";

export interface Hotkey {
  keys: string;
  action: string;
  /** False until the task that implements it; shown as "coming". */
  done: boolean;
}

export const HOTKEYS: readonly Hotkey[] = table.keys;

/** Key names that differ between keyboards and languages (Ctrl → Strg on German keyboards). */
export const KEY_NAMES = ["Ctrl", "Shift", "Space", "Delete", "Home", "End", "Esc", "wheel", "double-click", "drag", "click"] as const;

/** Replace the key names in a table cell with their translations. */
export function localizeKeys(keys: string, translate: (name: string) => string): string {
  return keys.replace(new RegExp(`(?<![\\w-])(${KEY_NAMES.join("|")})(?![\\w-])`, "g"), (name) => translate(name));
}
