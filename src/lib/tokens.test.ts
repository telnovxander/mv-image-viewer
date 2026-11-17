import { describe, expect, it } from "vitest";
import tokensCss from "../tokens.css?raw";
import { contrastRatio } from "./contrast";

// Every source file of the interface as text, except tests (Vite reads them, no Node API needed).
const SOURCES = import.meta.glob(["../**/*.{css,ts,tsx}", "!../**/*.test.ts"], {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

/** Token values of one theme block of tokens.css. */
function themeTokens(theme: "light" | "dark"): Record<string, string> {
  const block = new RegExp(`:root\\[data-theme="${theme}"\\]\\s*\\{([^}]*)\\}`).exec(tokensCss);
  if (!block) throw new Error(`no ${theme} block in tokens.css`);
  const tokens: Record<string, string> = {};
  for (const [, name, value] of block[1].matchAll(/--([\w-]+):\s*(#[0-9a-fA-F]{6})/g)) {
    tokens[name] = value;
  }
  return tokens;
}

const BACKGROUNDS = ["bg", "surface", "surface-alt"];

describe.each(["light", "dark"] as const)("%s theme passes WCAG AA", (theme) => {
  const t = themeTokens(theme);

  it.each(["text", "text-muted", "danger"])("%s text on backgrounds (4.5:1)", (fg) => {
    for (const bg of BACKGROUNDS) expect(contrastRatio(t[fg], t[bg]), `${fg} on ${bg}`).toBeGreaterThanOrEqual(4.5);
  });

  it("text on the selected row (4.5:1)", () => {
    expect(contrastRatio(t.text, t.selection)).toBeGreaterThanOrEqual(4.5);
  });

  it.each(["focus", "accent-amber"])("%s graphics on backgrounds (3:1)", (fg) => {
    for (const bg of BACKGROUNDS) expect(contrastRatio(t[fg], t[bg]), `${fg} on ${bg}`).toBeGreaterThanOrEqual(3);
  });

  it("focus on the selected row (3:1)", () => {
    expect(contrastRatio(t.focus, t.selection)).toBeGreaterThanOrEqual(3);
  });
});

describe("colours", () => {
  it("are defined only in tokens.css", () => {
    const offenders = Object.entries(SOURCES)
      .filter(([path]) => !path.endsWith("/tokens.css"))
      .filter(([, text]) => /#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(/.test(text))
      .map(([path]) => path);
    expect(Object.keys(SOURCES).length).toBeGreaterThan(5);
    expect(offenders).toEqual([]);
  });
});
