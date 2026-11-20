// Extensions of the images Spiegelmond shows (the same lists as in iv-core/src/formats.rs; a test
// keeps them equal): for the filter of the open dialog and the URL of an image.
export const WEB_EXTENSIONS = ["jpg", "jpeg", "jfif", "png", "gif", "webp", "bmp", "avif"];

/** HEIC / HEIF: decoded by libheif in iv-core (IV-05). */
export const HEIF_EXTENSIONS = ["heic", "heif", "hif"];

/** TIFF, HEIF and RAW: rendered by iv-core and served through the `ivimg` protocol (IV-05). */
export const DECODED_EXTENSIONS = ["tif", "tiff", "heic", "heif", "hif", "dng", "cr2", "nef", "nrw", "arw", "srw", "orf", "rw2", "pef", "erf", "3fr", "cr3"];

function extensionOf(path: string): string {
  return path.split(".").pop()?.toLowerCase() ?? "";
}

export function isDecoded(path: string): boolean {
  return DECODED_EXTENSIONS.includes(extensionOf(path));
}

export function isHeif(path: string): boolean {
  return HEIF_EXTENSIONS.includes(extensionOf(path));
}
