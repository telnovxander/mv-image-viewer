import { describe, expect, it } from "vitest";
import { FIT, viewAfterKey } from "./compare";

describe("comparing two frames (IV-18)", () => {
  it("zooms and pans both with the keyboard", () => {
    const zoomed = viewAfterKey(FIT, "+");
    expect(zoomed?.scale).toBe(1.25);
    expect(viewAfterKey(zoomed ?? FIT, "ArrowLeft")?.x).toBe(60);
    expect(viewAfterKey(FIT, "ArrowLeft")).toEqual(FIT);
    expect(viewAfterKey({ scale: 4, x: 10, y: 10 }, "0")).toEqual(FIT);
    expect(viewAfterKey({ scale: 1.25, x: 10, y: 0 }, "-")).toEqual(FIT);
    expect(viewAfterKey(FIT, "q")).toBeNull();
  });
});
