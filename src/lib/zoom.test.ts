import { describe, expect, it } from "vitest";
import {
  canPan,
  clampPan,
  fitScale,
  MAX_SCALE,
  MIN_SCALE,
  pan,
  resize,
  toggleAt,
  viewFor,
  wheelFactor,
  widthScale,
  zoomAt,
  type View,
} from "./zoom";

const photo = { width: 4000, height: 3000 };
const window = { width: 1000, height: 600 };

/** The image point under a screen point. */
const imagePoint = (view: View, x: number, y: number) => ({ x: (x - view.x) / view.scale, y: (y - view.y) / view.scale });

describe("fit and width", () => {
  it("fit shows the whole image and never enlarges a small one", () => {
    expect(fitScale(photo, window)).toBeCloseTo(0.2);
    expect(fitScale({ width: 200, height: 100 }, window)).toBe(1);
  });

  it("width fills the width, also for small images", () => {
    expect(widthScale(photo, window)).toBeCloseTo(0.25);
    expect(widthScale({ width: 500, height: 100 }, window)).toBe(2);
  });

  it("fit is centred", () => {
    const view = viewFor("fit", photo, window);
    expect(view.x).toBeCloseTo((1000 - 800) / 2);
    expect(view.y).toBeCloseTo(0);
  });

  it("width starts at the top of the image", () => {
    const view = viewFor("width", photo, window);
    expect(view).toMatchObject({ x: 0, y: 0 });
  });
});

describe("zoomAt", () => {
  it("keeps the point under the cursor in place", () => {
    const start = viewFor("fit", photo, window);
    for (const [x, y, scale] of [[300, 200, 1], [850, 500, 3.7], [500, 300, 0.5]]) {
      const before = imagePoint(start, x, y);
      const after = zoomAt(start, scale, { x, y }, photo, window);
      const point = imagePoint(after, x, y);
      expect(point.x).toBeCloseTo(before.x, 6);
      expect(point.y).toBeCloseTo(before.y, 6);
    }
  });

  it("at the very edge, keeping the image in the window wins over keeping the point", () => {
    const view = zoomAt(viewFor("fit", photo, window), 3.7, { x: 900, y: 550 }, photo, window);
    expect(view.x).toBe(1000 - 4000 * 3.7);
  });

  it("stays within the limits", () => {
    const start = viewFor("fit", photo, window);
    expect(zoomAt(start, 1000, { x: 0, y: 0 }, photo, window).scale).toBe(MAX_SCALE);
    expect(zoomAt(start, 0, { x: 0, y: 0 }, photo, window).scale).toBe(MIN_SCALE);
  });

  it("centres an image that becomes smaller than the window", () => {
    const view = zoomAt(viewFor("actual", photo, window), 0.1, { x: 10, y: 10 }, photo, window);
    expect(view.x).toBeCloseTo((1000 - 400) / 2);
    expect(view.y).toBeCloseTo((600 - 300) / 2);
  });

  it("zooms in and out smoothly with the wheel", () => {
    expect(wheelFactor(-100)).toBeGreaterThan(1);
    expect(wheelFactor(100)).toBeLessThan(1);
    expect(wheelFactor(100) * wheelFactor(-100)).toBeCloseTo(1);
  });
});

describe("pan", () => {
  it("never leaves an empty strip next to an enlarged image", () => {
    const view = viewFor("actual", photo, window);
    const far = pan(view, 10_000, 10_000, photo, window);
    expect(far).toMatchObject({ x: 0, y: 0 });
    const other = pan(view, -10_000, -10_000, photo, window);
    expect(other.x).toBe(1000 - 4000);
    expect(other.y).toBe(600 - 3000);
  });

  it("keeps a smaller image centred", () => {
    const view = viewFor("fit", photo, window);
    expect(pan(view, 300, 0, photo, window).x).toBeCloseTo(view.x);
    expect(canPan(view, photo, window)).toBe(false);
    expect(canPan(viewFor("actual", photo, window), photo, window)).toBe(true);
  });

  it("clampPan keeps an image in view after any change", () => {
    const view = clampPan({ mode: "custom", scale: 1, x: 500, y: -5000 }, photo, window);
    expect(view).toMatchObject({ x: 0, y: 600 - 3000 });
  });
});

describe("double click and resize", () => {
  it("toggles fit ↔ 100 % at the clicked point", () => {
    const fit = viewFor("fit", photo, window);
    const before = imagePoint(fit, 400, 300);
    const actual = toggleAt(fit, { x: 400, y: 300 }, photo, window);
    expect(actual.mode).toBe("actual");
    expect(actual.scale).toBe(1);
    const point = imagePoint(actual, 400, 300);
    expect(point.x).toBeCloseTo(before.x, 6);
    expect(point.y).toBeCloseTo(before.y, 6);
    expect(toggleAt(actual, { x: 0, y: 0 }, photo, window)).toEqual(fit);
  });

  it("fit follows the window, a custom zoom keeps its scale", () => {
    const bigger = { width: 2000, height: 1500 };
    expect(resize(viewFor("fit", photo, window), photo, bigger).scale).toBeCloseTo(0.5);
    const custom = zoomAt(viewFor("fit", photo, window), 0.7, { x: 500, y: 300 }, photo, window);
    expect(resize(custom, photo, bigger).scale).toBe(0.7);
  });
});
