// The bars over the image (the top bar and the side arrows, IV-20) follow one rule:
//
// - they appear when the mouse moves and hide again CHROME_HIDE_MS after it stops;
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
