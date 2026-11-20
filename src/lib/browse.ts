// Browsing the images of a folder (IV-03): no wrap-around, a hint at the ends.

export type Edge = "first" | "last" | null;

export interface Step {
  index: number;
  /** Set when the step wanted to go past an end and stayed. */
  edge: Edge;
}

/** Move by `delta` (±1 for arrows, ±page for PageUp/PageDown), stopping at the ends. */
export function step(index: number, count: number, delta: number): Step {
  if (count === 0) return { index: 0, edge: null };
  const target = index + delta;
  if (target < 0) return { index: 0, edge: index === 0 ? "first" : null };
  if (target > count - 1) return { index: count - 1, edge: index === count - 1 ? "last" : null };
  return { index: target, edge: null };
}

/**
 * After the folder was read again: the position of the current file in the new list; if the
 * file is gone, the image that took its place (the same position, within the new list).
 */
export function relocate(newPaths: readonly string[], currentPath: string, oldIndex: number): number {
  const found = newPaths.indexOf(currentPath);
  if (found >= 0) return found;
  return Math.max(0, Math.min(oldIndex, newPaths.length - 1));
}

/**
 * After the image at `index` left a list of `count` images (deleted, IV-07): the index of the
 * image shown next in the shorter list — the following one, or the previous one at the end;
 * -1 when none is left.
 */
export function afterRemoval(index: number, count: number): number {
  const left = count - 1;
  if (left <= 0) return -1;
  return Math.min(index, left - 1);
}
