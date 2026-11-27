// The bars over the image (the top bar and the side arrows, IV-20) follow one rule:
//
// - the top bar appears when the mouse moves and hides again CHROME_HIDE_MS after it stops;
// - the side arrows (IV-23) appear only while the mouse is near the left or right edge
//   (EDGE_ZONE), over the arrow; a move in the middle of the photo does not show them;
// - browsing (keys, wheel, the side arrows) and every slideshow step hide them at once;
// - they stay while the mouse is over them, while a button on them has the keyboard focus, while a
//   menu or a dialog is open, and on the empty screen.
//
// IV/PROJECT.md, "Внешний вид"; the "?" help describes the same rule.
export const CHROME_HIDE_MS = 2000;

export interface ChromeState {
  /** The mouse moved within the last CHROME_HIDE_MS and nothing was browsed since. */
  recentlyMoved: boolean;
  /** The mouse is over a bar or an arrow, or one of their buttons has the keyboard focus. */
  overBar: boolean;
  dialogOpen: boolean;
  /** The right-click menu or the "?" menu is open. */
  menuOpen: boolean;
  /** Nothing is open yet: the empty screen keeps its controls in view. */
  empty: boolean;
}

export function chromeVisible(state: ChromeState): boolean {
  return state.empty || state.dialogOpen || state.menuOpen || state.overBar || state.recentlyMoved;
}

export interface Point {
  x: number;
  y: number;
}

/**
 * Whether a mouse move event is a real move. Chromium also sends `mousemove` without any movement
 * when the picture under a still mouse changes (a new image after a key press); that must not
 * bring the bars back.
 */
export function movedFrom(last: Point | null, x: number, y: number): boolean {
  return last === null || last.x !== x || last.y !== y;
}

/** How near the left or right edge the mouse brings up the side arrow, in pixels (IV-23). */
export const EDGE_ZONE = 96;

/** The side whose arrow the mouse calls up: -1 left, 1 right, null in the middle (IV-23). */
export function edgeAt(x: number, width: number, zone = EDGE_ZONE): -1 | 1 | null {
  if (width <= 0) return null;
  const near = Math.min(zone, width / 3);
  if (x < near) return -1;
  if (x > width - near) return 1;
  return null;
}

/** How long the "Esc or F11 — leave full screen" hint stays (IV-23). */
export const FULLSCREEN_HINT_MS = 2500;

/**
 * Esc in Sichel (IV-23): a running slideshow stops first and full screen stays; the next Esc
 * leaves full screen; only then Esc means nothing here.
 */
export function escapeStep(slideshow: boolean, fullscreen: boolean): "stop_slideshow" | "leave_fullscreen" | null {
  if (slideshow) return "stop_slideshow";
  if (fullscreen) return "leave_fullscreen";
  return null;
}
