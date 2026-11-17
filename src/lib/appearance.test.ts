import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, parseSettings } from "./settings";
import { resolveTheme, toggleMode } from "./theme";

describe("resolveTheme", () => {
  it("follows the system for 'system' and keeps a manual choice", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });
});

describe("toggleMode", () => {
  it("switches between Sichel and Vollmond", () => {
    expect(toggleMode("sichel")).toBe("vollmond");
    expect(toggleMode("vollmond")).toBe("sichel");
  });
});

describe("parseSettings", () => {
  it("reads valid settings", () => {
    expect(parseSettings('{"theme":"dark","mode":"vollmond","background":"checker"}')).toEqual({
      theme: "dark",
      mode: "vollmond",
      background: "checker",
      language: null,
      sort: "name",
      cacheMb: 512,
      slideSeconds: 4,
    });
    expect(parseSettings('{"slideSeconds":10}').slideSeconds).toBe(10);
    expect(parseSettings('{"slideSeconds":0}').slideSeconds).toBe(4);
    expect(parseSettings('{"sort":"modified","cacheMb":1024}')).toMatchObject({ sort: "modified", cacheMb: 1024 });
    expect(parseSettings('{"sort":"size","cacheMb":1}')).toMatchObject({ sort: "name", cacheMb: 512 });
    expect(parseSettings('{"language":"de"}').language).toBe("de");
    expect(parseSettings('{"language":"<script>"}').language).toBeNull();
  });

  it("falls back to defaults for missing, broken or unknown values", () => {
    expect(parseSettings(null)).toEqual(DEFAULT_SETTINGS);
    expect(parseSettings("{not json")).toEqual(DEFAULT_SETTINGS);
    expect(parseSettings('{"theme":"pink","mode":42,"background":"plaid"}')).toEqual(DEFAULT_SETTINGS);
  });
});
