// The film strip (IV-08) is virtualized by hand: only the thumbnails in view (and a few around)
// exist in the page, so a folder of 10 000 photos scrolls like a folder of 10.

/** Width of one thumbnail cell with its gap, in CSS pixels. */
export const CELL = 88;

/** The cells to render for a strip scrolled to `scrollLeft` and `width` wide: [first, last). */
export function visibleRange(scrollLeft: number, width: number, count: number, overscan = 6): [number, number] {
  if (count <= 0 || width <= 0) return [0, 0];
  const first = Math.max(0, Math.floor(scrollLeft / CELL) - overscan);
  const last = Math.min(count, Math.ceil((scrollLeft + width) / CELL) + overscan);
  return [first, Math.max(first, last)];
}

/** The scroll position that puts cell `index` in the middle of a strip `width` wide. */
export function centeredScroll(index: number, width: number, count: number): number {
  const max = Math.max(0, count * CELL - width);
  return Math.min(max, Math.max(0, index * CELL + CELL / 2 - width / 2));
}
