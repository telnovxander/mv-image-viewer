import { describe, expect, it } from "vitest";
import en from "../../locales/en.json";
import batch from "./BatchSection.tsx?raw";
import dates from "./DatesSection.tsx?raw";
import duplicates from "./DuplicatesSection.tsx?raw";
import similar from "./SimilarSection.tsx?raw";
import tools from "./ToolsWindow.tsx?raw";

// IV-24: every control of the Vollmond tools explains itself (one hint layer, src/tools/TipLayer.tsx).
const files = { batch, dates, duplicates, similar, tools };

describe("the Vollmond tools explain themselves (IV-24)", () => {
  it("every button has a hint", () => {
    for (const [name, source] of Object.entries(files)) {
      for (const match of source.matchAll(/<button\b(?:=>|[^>])*>/g)) {
        expect(match[0], `${name}: ${match[0].slice(0, 80)}`).toMatch(/tip\(t\("tips\.\w+"\)\)/);
      }
    }
  });

  it("every hint used exists in en.json", () => {
    for (const source of Object.values(files)) {
      for (const match of source.matchAll(/tip\(t\("tips\.(\w+)"\)\)/g)) expect(en.tips, match[1]).toHaveProperty(match[1]);
    }
  });

  it("every tool says what it is for, and the empty states lead to the next step", () => {
    for (const [name, source] of Object.entries(files)) expect(source, name).toMatch(/tools__why/);
    expect(tools).toMatch(/tools\.next_choose/);
    expect(tools).toMatch(/tools\.next_read/);
    expect(en.tools.index).toBe("Read the folder");
  });
});
