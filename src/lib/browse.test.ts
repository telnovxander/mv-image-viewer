import { describe, expect, it } from "vitest";
import { afterRemoval, relocate, step } from "./browse";

describe("step", () => {
  it("moves within the folder", () => {
    expect(step(3, 10, 1)).toEqual({ index: 4, edge: null });
    expect(step(3, 10, -1)).toEqual({ index: 2, edge: null });
  });

  it("does not wrap around and reports the end it hit", () => {
    expect(step(9, 10, 1)).toEqual({ index: 9, edge: "last" });
    expect(step(0, 10, -1)).toEqual({ index: 0, edge: "first" });
  });

  it("a page jump stops at the end without a hint the first time", () => {
    expect(step(7, 10, 5)).toEqual({ index: 9, edge: null });
    expect(step(2, 10, -5)).toEqual({ index: 0, edge: null });
  });

  it("handles an empty folder", () => {
    expect(step(0, 0, 1)).toEqual({ index: 0, edge: null });
  });
});

describe("relocate", () => {
  it("finds the current file in the new list", () => {
    expect(relocate(["a", "new", "b", "c"], "b", 1)).toBe(2);
  });

  it("takes the image at the same place when the file vanished", () => {
    expect(relocate(["a", "c"], "b", 1)).toBe(1);
    expect(relocate(["a"], "c", 2)).toBe(0);
    expect(relocate([], "c", 2)).toBe(0);
  });
});

describe("afterRemoval (IV-07)", () => {
  it("shows the next image, the previous one at the end, none when the folder is empty", () => {
    expect(afterRemoval(0, 3)).toBe(0);
    expect(afterRemoval(1, 3)).toBe(1);
    expect(afterRemoval(2, 3)).toBe(1);
    expect(afterRemoval(0, 1)).toBe(-1);
  });
});
