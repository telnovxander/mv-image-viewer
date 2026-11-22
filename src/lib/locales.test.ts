import { describe, expect, it } from "vitest";
import { availableLanguages, LOCALES, pickLanguage, type Messages } from "./locales";

const PLURAL_SUFFIX = /_(zero|one|two|few|many|other)$/;

/** Flat keys of a locale; plural keys collapse to "<key>#plural". */
function flatKeys(messages: Record<string, unknown>, prefix = ""): string[] {
  return Object.entries(messages).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    if (value && typeof value === "object") return flatKeys(value as Record<string, unknown>, path);
    return [PLURAL_SUFFIX.test(path) ? path.replace(PLURAL_SUFFIX, "#plural") : path];
  });
}

function leaves(messages: Record<string, unknown>, prefix = ""): [string, string][] {
  return Object.entries(messages).flatMap(([key, value]): [string, string][] => {
    const path = prefix ? `${prefix}.${key}` : key;
    if (value && typeof value === "object") return leaves(value as Record<string, unknown>, path);
    return [[path, String(value)]];
  });
}

function placeholders(text: string): string[] {
  return [...text.matchAll(/\{\{\s*(\w+)[^}]*\}\}/g)].map((m) => m[1]).sort();
}

const english = LOCALES.en;
const codes = Object.keys(LOCALES);

describe("locales", () => {
  it("include English, Russian and German", () => {
    expect(codes).toEqual(expect.arrayContaining(["en", "ru", "de"]));
  });

  describe.each(codes)("%s", (code) => {
    const messages: Messages = LOCALES[code];

    it("has its own name in _meta.name", () => {
      expect(messages._meta.name.trim()).not.toBe("");
    });

    it("has exactly the keys of en.json", () => {
      expect([...new Set(flatKeys(messages))].sort()).toEqual([...new Set(flatKeys(english))].sort());
    });

    it("has every plural form the language needs", () => {
      const needed = new Intl.PluralRules(code).resolvedOptions().pluralCategories;
      const pluralBases = new Set(flatKeys(english).filter((k) => k.endsWith("#plural")).map((k) => k.replace("#plural", "")));
      const own = new Map(leaves(messages));
      for (const base of pluralBases) {
        for (const form of needed) expect(own.has(`${base}_${form}`), `${base}_${form}`).toBe(true);
      }
    });

    it("keeps the placeholders of en.json", () => {
      const englishLeaves = new Map(leaves(english).map(([k, v]) => [k.replace(PLURAL_SUFFIX, ""), placeholders(v)]));
      for (const [key, text] of leaves(messages)) {
        const expected = englishLeaves.get(key.replace(PLURAL_SUFFIX, ""));
        expect(placeholders(text), key).toEqual(expected);
      }
    });
  });
});

describe("pickLanguage", () => {
  const all = ["de", "en", "ru"];

  it("keeps a saved choice that still exists", () => {
    expect(pickLanguage("ru", ["de-DE"], all)).toBe("ru");
  });

  it("follows Windows by the base code, otherwise English", () => {
    expect(pickLanguage(null, ["de-CH", "en-US"], all)).toBe("de");
    expect(pickLanguage("xx", ["ru-RU"], all)).toBe("ru");
    expect(pickLanguage(null, ["fr-FR", "it"], all)).toBe("en");
    expect(pickLanguage(null, [], all)).toBe("en");
  });
});

describe("availableLanguages", () => {
  it("lists languages under their own names, English first", () => {
    const list = availableLanguages();
    expect(list[0]).toEqual({ code: "en", name: "English" });
    expect(list.map((l) => l.name)).toEqual(expect.arrayContaining(["Русский", "Deutsch"]));
  });
});

// No text in components: JSX text and user-facing attributes must come from t().
const COMPONENTS = import.meta.glob(["../**/*.tsx", "!../**/*.test.tsx"], { query: "?raw", import: "default", eager: true }) as Record<string, string>;
// Brand names are never translated (MV/I18N.md).
const ALLOWED = /^(Spiegelmond|Sichel|Vollmond|Moonshine Vibecoder)$/;

describe("components", () => {
  it("contain no hard-coded user text", () => {
    const found: string[] = [];
    for (const [path, source] of Object.entries(COMPONENTS)) {
      for (const match of source.matchAll(/(?<![=-])>\s*([^<>{}\s][^<>{}]*?)\s*</g)) {
        if (/\p{L}/u.test(match[1]) && !ALLOWED.test(match[1].trim()) && !/[=;()]/.test(match[1])) found.push(`${path}: ${match[1]}`);
      }
      for (const match of source.matchAll(/\b(title|aria-label|placeholder|alt)="([^"]+)"/g)) {
        found.push(`${path}: ${match[1]}="${match[2]}"`);
      }
    }
    expect(found).toEqual([]);
  });
});
