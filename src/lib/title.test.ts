import { describe, expect, it } from "vitest";
import { APP_NAME, windowTitle } from "./title";

describe("windowTitle", () => {
  it("is the program name with nothing open", () => {
    expect(windowTitle(null, 0, 0, "en")).toBe(APP_NAME);
  });

  it("shows the name and the position in the folder", () => {
    expect(windowTitle("IMG_0001.jpg", 1, 1, "en")).toBe("IMG_0001.jpg — 1 / 1");
  });

  it("writes numbers in the format of the language", () => {
    expect(windowTitle("a.png", 1200, 34000, "en")).toBe("a.png — 1,200 / 34,000");
    expect(windowTitle("a.png", 1200, 34000, "de")).toBe("a.png — 1.200 / 34.000");
  });
});
