// IV-22: the WebView never zooms the whole page, and the zoom of the photo reaches it from anywhere
// in the window (the bars and the side arrows lie over the photo and used to swallow Ctrl+wheel).

/** Ctrl+plus / minus / 0 (also on the number pad): the keys a browser zooms the page with. */
export function isPageZoomKey(event: Pick<KeyboardEvent, "ctrlKey" | "metaKey" | "key">): boolean {
  return (event.ctrlKey || event.metaKey) && ["+", "=", "-", "_", "0", ")"].includes(event.key);
}

/**
 * Stop the page zoom of the WebView for the whole window: Ctrl+wheel (a touchpad pinch arrives as
 * Ctrl+wheel too) and the zoom keys. Only the default is prevented: the image view and the dialogs
 * still get the events and zoom the photo. Returns the function that removes the guard.
 */
export function guardPageZoom(target: Pick<Window, "addEventListener" | "removeEventListener">): () => void {
  const onWheel = (event: WheelEvent) => {
    if (event.ctrlKey || event.metaKey) event.preventDefault();
  };
  const onKey = (event: KeyboardEvent) => {
    if (isPageZoomKey(event)) event.preventDefault();
  };
  const options = { capture: true, passive: false } as const;
  target.addEventListener("wheel", onWheel, options);
  target.addEventListener("keydown", onKey, true);
  return () => {
    target.removeEventListener("wheel", onWheel, options);
    target.removeEventListener("keydown", onKey, true);
  };
}

/** The zoom buttons of the top bar (the magnifiers) ask the image view for one step. */
export const ZOOM_EVENT = "iv-zoom";

export function requestZoom(direction: 1 | -1, target: Pick<Window, "dispatchEvent"> = window): void {
  target.dispatchEvent(new CustomEvent<1 | -1>(ZOOM_EVENT, { detail: direction }));
}
