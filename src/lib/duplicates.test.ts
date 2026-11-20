import { describe, expect, it } from "vitest";
import type { DuplicateGroup } from "./backend";
import { defaultChoice, isProtected, toggleChoice } from "./duplicates";

const file = (path: string) => ({ path, size: 1, modified: 0, taken: null, camera: null, width: null, height: null, hash: "h" });
const groups: DuplicateGroup[] = [{ files: [file("D:\\Fotos\\a.jpg"), file("D:\\Archiv\\a.jpg"), file("D:\\WhatsApp\\a.jpg")], keep: 0 }];

describe("duplicate choice (IV-17)", () => {
  it("deletes all but the kept one, never in a protected folder", () => {
    expect(defaultChoice(groups, [])).toEqual([["D:\\Archiv\\a.jpg", "D:\\WhatsApp\\a.jpg"]]);
    expect(defaultChoice(groups, ["d:/archiv/"])).toEqual([["D:\\WhatsApp\\a.jpg"]]);
    expect(isProtected("D:\\Archiv2\\a.jpg", ["D:\\Archiv"])).toBe(false);
  });

  it("never empties a group", () => {
    const choice = defaultChoice(groups, []);
    expect(toggleChoice(choice, groups, 0, "D:\\Fotos\\a.jpg")).toBe(choice);
    expect(toggleChoice(choice, groups, 0, "D:\\Archiv\\a.jpg")).toEqual([["D:\\WhatsApp\\a.jpg"]]);
  });
});
