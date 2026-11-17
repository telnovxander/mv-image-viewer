// The bars over the image hide themselves: they appear when the mouse moves and disappear after
// this pause, unless the mouse is over them or a dialog is open (IV/PROJECT.md, "Внешний вид").
export const CHROME_HIDE_MS = 2000;

export interface ChromeState {
  /** The mouse moved within the last CHROME_HIDE_MS. */
  recentlyMoved: boolean;
  /** The mouse or the keyboard focus is on a bar. */
  overBar: boolean;
  dialogOpen: boolean;
  /** Nothing is open yet: the empty screen keeps its controls in view. */
  empty: boolean;
}

export function chromeVisible(state: ChromeState): boolean {
  return state.empty || state.dialogOpen || state.overBar || state.recentlyMoved;
}
