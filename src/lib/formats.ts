// Extensions of the images Spiegelmond shows (the same lists as in iv-core/src/formats.rs; a test
// keeps them equal): for the filter of the open dialog and the URL of an image.
export const WEB_EXTENSIONS = ["jpg", "jpeg", "jfif", "png", "gif", "webp", "bmp", "avif"];

/** TIFF and RAW: rendered by iv-core and served through the `ivimg` protocol (IV-05). */
export const DECODED_EXTENSIONS = ["tif", "tiff", "dng", "cr2", "nef", "nrw", "arw", "srw", "orf", "rw2", "pef", "erf", "3fr"];

export function isDecoded(path: string): boolean {
  const extension = path.split(".").pop()?.toLowerCase() ?? "";
  return DECODED_EXTENSIONS.includes(extension);
}
