// IV-18: the shared view of two frames compared side by side — one zoom and one position for both.

export interface SharedView {
  scale: number;
  x: number;
  y: number;
}

export const FIT: SharedView = { scale: 1, x: 0, y: 0 };
const MIN = 1;
const MAX = 16;

/** The view after a key (+, −, 0, arrows); `null` when the key is not one of them. */
export function viewAfterKey(view: SharedView, key: string, step = 60): SharedView | null {
  const clamp = (scale: number) => Math.min(MAX, Math.max(MIN, scale));
  if (key === "+" || key === "=") return { ...view, scale: clamp(view.scale * 1.25) };
  if (key === "-") {
    const scale = clamp(view.scale / 1.25);
    return scale === 1 ? FIT : { ...view, scale };
  }
  if (key === "0") return FIT;
  if (view.scale === 1) return key.startsWith("Arrow") ? view : null;
  if (key === "ArrowLeft") return { ...view, x: view.x + step };
  if (key === "ArrowRight") return { ...view, x: view.x - step };
  if (key === "ArrowUp") return { ...view, y: view.y + step };
  if (key === "ArrowDown") return { ...view, y: view.y - step };
  return null;
}
