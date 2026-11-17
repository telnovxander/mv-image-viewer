import { describe, expect, it } from "vitest";
import { chromeVisible } from "./chrome";

const idle = { recentlyMoved: false, overBar: false, dialogOpen: false, empty: false };

describe("chromeVisible", () => {
  it("shows only the image while nothing moves", () => {
    expect(chromeVisible(idle)).toBe(false);
  });

  it("shows the bars after a mouse move, over a bar, with a dialog and on the empty screen", () => {
    expect(chromeVisible({ ...idle, recentlyMoved: true })).toBe(true);
    expect(chromeVisible({ ...idle, overBar: true })).toBe(true);
    expect(chromeVisible({ ...idle, dialogOpen: true })).toBe(true);
    expect(chromeVisible({ ...idle, empty: true })).toBe(true);
  });
});
