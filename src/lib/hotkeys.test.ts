import { describe, expect, it } from "vitest";
import { HOTKEYS, KEY_NAMES, localizeKeys } from "./hotkeys";
import { LOCALES } from "./locales";

describe("hotkeys", () => {
  it("have a description and key names in every language", () => {
    for (const [code, messages] of Object.entries(LOCALES)) {
      const hotkeys = messages.hotkeys as Record<string, string>;
      const keys = messages.keys as Record<string, string>;
      for (const key of HOTKEYS) expect(hotkeys[key.action], `${code}: hotkeys.${key.action}`).toBeTruthy();
      for (const name of KEY_NAMES) expect(keys[name], `${code}: keys.${name}`).toBeTruthy();
    }
  });

  it("name the keys as the keyboard of the language does", () => {
    const de = LOCALES.de.keys as Record<string, string>;
    expect(localizeKeys("Ctrl+Shift+C", (n) => de[n])).toBe("Strg+Umschalt+C");
    expect(localizeKeys("Home · End", (n) => de[n])).toBe("Pos1 · Ende");
  });

  it("list every shortcut the brief requires", () => {
    const all = HOTKEYS.map((k) => k.keys).join(" ");
    for (const key of ["← →", "Home", "End", "Ctrl+0", "F11", "Esc", "Space", "Shift+R", "I", "T", "Delete", "Ctrl+Z", "Ctrl+Shift+C", "?"]) {
      expect(all).toContain(key);
    }
  });
});
