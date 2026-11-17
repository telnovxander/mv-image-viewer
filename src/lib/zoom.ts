// Zoom and pan of the image as pure functions (IV-02). The image is drawn at its natural size,
// moved to (x, y) and scaled by `scale` from its top-left corner: screen = (x, y) + image * scale.

export interface Size {
  width: number;
  height: number;
}

export interface Point {
  x: number;
  y: number;
}

/** How the scale was chosen: fit and width follow the window size, custom keeps its scale. */
export type ZoomMode = "fit" | "actual" | "width" | "custom";

export interface View {
  mode: ZoomMode;
  scale: number;
  x: number;
  y: number;
}

export const MIN_SCALE = 0.01;
export const MAX_SCALE = 32;
/** One step of Ctrl+plus / Ctrl+minus. */
export const KEY_STEP = 1.25;
/** Above this scale pixels are drawn sharp (image-rendering: pixelated). */
export const PIXELATED_FROM = 2;
/** How far one arrow key press moves an enlarged image, in screen pixels. */
export const ARROW_STEP = 64;

export function clampScale(scale: number): number {
  return Math.min(MAX_SCALE, Math.max(MIN_SCALE, scale));
}

/** The whole image in the window, never enlarged past 100 %. */
export function fitScale(image: Size, viewport: Size): number {
  if (image.width <= 0 || image.height <= 0) return 1;
  return clampScale(Math.min(1, viewport.width / image.width, viewport.height / image.height));
}

/** The image as wide as the window (small images are enlarged too). */
export function widthScale(image: Size, viewport: Size): number {
  if (image.width <= 0) return 1;
  return clampScale(viewport.width / image.width);
}

/**
 * Keep the image in the window: a side smaller than the window is centred, a larger one may move
 * only so far that no empty strip appears next to it.
 */
export function clampPan(view: View, image: Size, viewport: Size): View {
  const axis = (position: number, imageLength: number, viewportLength: number) => {
    const length = imageLength * view.scale;
    if (length <= viewportLength) return (viewportLength - length) / 2;
    return Math.min(0, Math.max(viewportLength - length, position));
  };
  return { ...view, x: axis(view.x, image.width, viewport.width), y: axis(view.y, image.height, viewport.height) };
}

/** The view of a mode: the scale it implies, centred (width: the top of the image in view). */
export function viewFor(mode: Exclude<ZoomMode, "custom">, image: Size, viewport: Size): View {
  const scale = mode === "fit" ? fitScale(image, viewport) : mode === "width" ? widthScale(image, viewport) : 1;
  const centred = clampPan({ mode, scale, x: 0, y: 0 }, image, viewport);
  if (mode === "width") return clampPan({ ...centred, y: 0 }, image, viewport);
  return clampPan({ ...centred, x: (viewport.width - image.width * scale) / 2, y: (viewport.height - image.height * scale) / 2 }, image, viewport);
}

/** Zoom to `scale` keeping the image point under `at` (screen coordinates) where it is. */
export function zoomAt(view: View, scale: number, at: Point, image: Size, viewport: Size): View {
  const next = clampScale(scale);
  const imageX = (at.x - view.x) / view.scale;
  const imageY = (at.y - view.y) / view.scale;
  return clampPan({ mode: "custom", scale: next, x: at.x - imageX * next, y: at.y - imageY * next }, image, viewport);
}

/** Ctrl+wheel and the touchpad pinch (Chromium sends it as Ctrl+wheel): smooth, by deltaY. */
export function wheelFactor(deltaY: number): number {
  return Math.exp(-deltaY * 0.002);
}

export function pan(view: View, dx: number, dy: number, image: Size, viewport: Size): View {
  return clampPan({ ...view, x: view.x + dx, y: view.y + dy }, image, viewport);
}

/** True when the image is larger than the window on some side, so arrows and dragging move it. */
export function canPan(view: View, image: Size, viewport: Size): boolean {
  return image.width * view.scale > viewport.width + 0.5 || image.height * view.scale > viewport.height + 0.5;
}

/** A fit or width view follows the new window size; a custom one keeps its scale. */
export function resize(view: View, image: Size, viewport: Size): View {
  if (view.mode === "custom") return clampPan(view, image, viewport);
  return viewFor(view.mode, image, viewport);
}

/** Double click: fit ↔ 100 % at the clicked point. */
export function toggleAt(view: View, at: Point, image: Size, viewport: Size): View {
  if (view.mode === "fit") return { ...zoomAt(view, 1, at, image, viewport), mode: "actual" };
  return viewFor("fit", image, viewport);
}

/** View-only rotation (R / Shift+R, IV-04): clockwise degrees. The file is not changed. */
export type Rotation = 0 | 90 | 180 | 270;

export function rotate(rotation: Rotation, clockwise: boolean): Rotation {
  return (((rotation + (clockwise ? 90 : 270)) % 360) as Rotation);
}

/** The size the rotated image takes on screen before scaling: sides swap at 90° and 270°. */
export function rotatedSize(natural: Size, rotation: Rotation): Size {
  return rotation === 90 || rotation === 270 ? { width: natural.height, height: natural.width } : natural;
}

/** A 2D affine matrix [a, b, c, d, e, f]: screen = (a·x + c·y + e, b·x + d·y + f). */
export type Matrix = [number, number, number, number, number, number];

/**
 * Where the natural-size image goes on screen (CSS transform origin 0 0): turned around its
 * centre, the turned box put at (view.x, view.y) and scaled like an unrotated image of
 * `rotatedSize`.
 */
export function imageMatrix(view: View, natural: Size, rotation: Rotation): Matrix {
  const radians = (rotation * Math.PI) / 180;
  const cos = Math.round(Math.cos(radians));
  const sin = Math.round(Math.sin(radians));
  const box = rotatedSize(natural, rotation);
  const s = view.scale;
  // p' = (x, y) + s · (box/2 + R · (p − natural/2))
  const cx = natural.width / 2;
  const cy = natural.height / 2;
  return [
    s * cos,
    s * sin,
    -s * sin,
    s * cos,
    view.x + s * (box.width / 2 - (cos * cx - sin * cy)),
    view.y + s * (box.height / 2 - (sin * cx + cos * cy)),
  ];
}

export function imageTransform(view: View, natural: Size, rotation: Rotation): string {
  return `matrix(${imageMatrix(view, natural, rotation).join(", ")})`;
}
