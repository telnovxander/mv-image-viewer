import { describe, expect, it } from "vitest";
import { CELL, centeredScroll, visibleRange } from "./strip";

describe("film strip (IV-08)", () => {
  it("renders only the cells in view and a few around", () => {
    expect(visibleRange(0, CELL * 10, 10_000)).toEqual([0, 16]);
    const [first, last] = visibleRange(CELL * 5000, CELL * 12, 10_000);
    expect(first).toBe(4994);
    expect(last).toBe(5018);
    expect(last - first).toBeLessThan(40);
    expect(visibleRange(0, 800, 3)).toEqual([0, 3]);
    expect(visibleRange(0, 800, 0)).toEqual([0, 0]);
  });

  it("puts the current image in the middle, within the ends", () => {
    expect(centeredScroll(0, CELL * 9, 100)).toBe(0);
    expect(centeredScroll(50, CELL * 9, 100)).toBe(50 * CELL + CELL / 2 - (CELL * 9) / 2);
    expect(centeredScroll(99, CELL * 9, 100)).toBe(100 * CELL - CELL * 9);
    expect(centeredScroll(5, CELL * 20, 8)).toBe(0);
  });
});
