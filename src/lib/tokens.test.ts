import { describe, expect, it } from "vitest";
import tokensCss from "../tokens.css?raw";
import { contrastRatio } from "./contrast";

// Every source file of the interface as text, except tests (Vite reads them, no Node API needed).
const SOURCES = import.meta.glob(["../**/*.{css,ts,tsx}", "!../**/*.test.ts", "!../**/*.test.tsx"], {
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

describe("fonts (IV-20: the same tokens as Halbmond)", () => {
  it("never use Courier or a generic monospace that falls back to it", () => {
    for (const [path, text] of Object.entries(SOURCES)) expect(text, path).not.toMatch(/courier/i);
    expect(/--font-mono:([^;]*);/.exec(tokensCss)?.[1]).not.toMatch(/(^|,)\s*monospace\s*$/);
  });

  it("are the font tokens of Halbmond (FM-21)", async () => {
    const halbmond = {
      ui: '"Segoe UI Variable Text", "Segoe UI Variable", "Segoe UI", system-ui, sans-serif',
      mono: '"Cascadia Mono", Consolas, "Lucida Console", ui-monospace, Menlo, "DejaVu Sans Mono"',
      weight: "500",
      strong: "650",
    };
    expect(/--font-ui:\s*([^;]*);/.exec(tokensCss)?.[1]).toBe(halbmond.ui);
    expect(/--font-mono:\s*([^;]*);/.exec(tokensCss)?.[1]).toBe(halbmond.mono);
    expect(/--font-weight:\s*(\d+)/.exec(tokensCss)?.[1]).toBe(halbmond.weight);
    expect(/--font-weight-strong:\s*(\d+)/.exec(tokensCss)?.[1]).toBe(halbmond.strong);
  });
});

describe("checkboxes (IV-20)", () => {
  it("keep their label next to the box in every row", () => {
    const styles = Object.entries(SOURCES).find(([path]) => path.endsWith("/styles.css"))?.[1] ?? "";
    expect(styles).toMatch(/\.field--row:has\(> input\[type="checkbox"\]:first-child\) \{\s*justify-content: flex-start;/);
  });
});
