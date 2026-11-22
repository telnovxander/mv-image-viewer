import { describe, expect, it } from "vitest";
import { chromeVisible, movedFrom } from "./chrome";

const idle = { recentlyMoved: false, overBar: false, dialogOpen: false, menuOpen: false, empty: false };

describe("chromeVisible", () => {
  it("shows only the image while nothing moves", () => {
    expect(chromeVisible(idle)).toBe(false);
  });

  it("shows the bars after a mouse move, over a bar, with a dialog or a menu and on the empty screen", () => {
    expect(chromeVisible({ ...idle, recentlyMoved: true })).toBe(true);
    expect(chromeVisible({ ...idle, overBar: true })).toBe(true);
    expect(chromeVisible({ ...idle, dialogOpen: true })).toBe(true);
    expect(chromeVisible({ ...idle, menuOpen: true })).toBe(true);
    expect(chromeVisible({ ...idle, empty: true })).toBe(true);
  });

  it("hides them when browsing clears the recent move (IV-20), unless the mouse is on a bar", () => {
    const afterBrowse = { ...idle, recentlyMoved: false };
    expect(chromeVisible(afterBrowse)).toBe(false);
    expect(chromeVisible({ ...afterBrowse, overBar: true })).toBe(true);
  });
});

describe("movedFrom (IV-20)", () => {
  it("counts only a real move: a mousemove at the same place after a new image does not show the bars", () => {
    expect(movedFrom(null, 10, 10)).toBe(true);
    expect(movedFrom({ x: 10, y: 10 }, 10, 10)).toBe(false);
    expect(movedFrom({ x: 10, y: 10 }, 11, 10)).toBe(true);
  });
});
