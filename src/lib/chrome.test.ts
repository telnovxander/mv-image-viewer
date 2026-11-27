import { describe, expect, it } from "vitest";
import { chromeVisible, edgeAt, EDGE_ZONE, escapeStep, movedFrom } from "./chrome";

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

describe("the side arrows (IV-23)", () => {
  it("come up only near the left or right edge, not in the middle of the photo", () => {
    expect(edgeAt(10, 1000)).toBe(-1);
    expect(edgeAt(EDGE_ZONE - 1, 1000)).toBe(-1);
    expect(edgeAt(500, 1000)).toBeNull();
    expect(edgeAt(1000 - EDGE_ZONE + 1, 1000)).toBe(1);
    expect(edgeAt(990, 1000)).toBe(1);
  });

  it("a narrow window keeps a middle without arrows", () => {
    expect(edgeAt(100, 240)).toBeNull();
    expect(edgeAt(0, 0)).toBeNull();
  });
});

describe("Esc in full screen and the slideshow (IV-23)", () => {
  it("stops the slideshow first and keeps full screen, then leaves full screen", () => {
    expect(escapeStep(true, true)).toBe("stop_slideshow");
    expect(escapeStep(false, true)).toBe("leave_fullscreen");
    expect(escapeStep(true, false)).toBe("stop_slideshow");
    expect(escapeStep(false, false)).toBeNull();
  });
});
