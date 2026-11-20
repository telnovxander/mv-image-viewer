// Calls into the Rust side (src-tauri). Errors arrive as { code, path } (iv-core IvError).
import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";
import { isDecoded } from "./formats";

export interface ImageFile {
  path: string;
  name: string;
  size: number;
  format: "web" | "decoded";
}

export interface BackendError {
  code: "not_found" | "access_denied" | "not_a_file" | "unsupported" | "no_orientation_tag" | "changed_meanwhile" | "verify_failed" | "broken" | "no_codec" | "trash_removable" | "trash_network" | "trash_unavailable" | "trash_failed" | "restore_failed" | "restore_occupied" | "copy_failed" | "open_with_failed" | "other";
  path: string;
}

export function isBackendError(value: unknown): value is BackendError {
  return typeof value === "object" && value !== null && "code" in value && "path" in value;
}

export const inTauri = isTauri();

export async function initialPath(): Promise<string | null> {
  return inTauri ? invoke<string | null>("initial_path") : null;
}

export async function openImage(path: string): Promise<ImageFile> {
  return invoke<ImageFile>("open_image", { path });
}

/** One image of a folder (iv-core FolderEntry). */
export interface FolderEntry {
  path: string;
  name: string;
  size: number;
  modified: number;
}

/** The images of the folder of `file`, sorted; read in the background. */
export async function listFolder(file: string, order: "name" | "modified" | "taken"): Promise<FolderEntry[]> {
  return invoke<FolderEntry[]>("list_folder", { file, order });
}

/** The URL of a local file for <img>, through the Tauri asset protocol. */
export function pathUrl(path: string): string {
  // The browser preview (no Tauri) only shows its demo picture, whose path already is a URL.
  if (!inTauri) return path;
  // TIFF and RAW are rendered by iv-core (`ivimg` protocol); the rest the WebView reads itself.
  return isDecoded(path) ? convertFileSrc(path, "ivimg") : convertFileSrc(path);
}

/** The thumbnail of an image for the film strip (IV-08, `ivthumb` protocol of iv-core). */
export function thumbUrl(path: string): string {
  return inTauri ? convertFileSrc(path, "ivthumb") : path;
}

export function imageUrl(file: ImageFile): string {
  return pathUrl(file.path);
}

const SETTINGS_KEY = "spiegelmond.settings";

/** The settings file text, or null before the first save. */
export async function loadSettingsText(): Promise<string | null> {
  if (inTauri) return invoke<string | null>("load_settings");
  try {
    return localStorage.getItem(SETTINGS_KEY);
  } catch {
    return null;
  }
}

export async function saveSettingsText(text: string): Promise<void> {
  if (inTauri) return invoke<void>("save_settings", { text });
  try {
    localStorage.setItem(SETTINGS_KEY, text);
  } catch {
    // Browser preview without storage: the settings live until the page reloads.
  }
}

/** EXIF of an image (iv-core ExifInfo); null without EXIF. */
export interface ExifInfo {
  orientation: number | null;
  taken: string | null;
  camera: string | null;
  lens: string | null;
  exposure: number | null;
  f_number: number | null;
  iso: number | null;
  focal_length: number | null;
  gps: [number, number] | null;
  width: number | null;
  height: number | null;
}

export async function exifOf(path: string): Promise<ExifInfo | null> {
  return inTauri ? invoke<ExifInfo | null>("exif_of", { path }) : null;
}

/** Open the place on OpenStreetMap in the browser (only after a click). */
let heifCheck: Promise<boolean> | null = null;

/** Whether HEIC can be decoded (libheif loaded); asked once. */
export async function heifAvailable(): Promise<boolean> {
  if (!inTauri) return false;
  heifCheck ??= invoke<boolean>("heif_available").catch(() => false);
  return heifCheck;
}

export async function openMap(latitude: number, longitude: number): Promise<void> {
  if (inTauri) await invoke("open_map", { latitude, longitude });
}

/** Writing the view rotation into a JPEG (iv-core orientation): plan → confirm → apply → undo. */
export interface OrientationPlan {
  path: string;
  from: number;
  to: number;
}

export interface JournalEntry {
  id: number;
  path: string;
  field: string;
  old: number;
  new: number;
  undoes?: number;
}

export async function planRotation(path: string, degrees: number): Promise<OrientationPlan> {
  return invoke<OrientationPlan>("plan_rotation", { path, degrees });
}

export async function applyRotation(plan: OrientationPlan): Promise<JournalEntry> {
  return invoke<JournalEntry>("apply_rotation", { plan });
}

export async function undoRotation(entry: JournalEntry): Promise<JournalEntry> {
  return invoke<JournalEntry>("undo_rotation", { entry });
}

/** A file moved to the Recycle Bin (iv-core recycle.rs): what Ctrl+Z needs to bring it back. */
export interface Trashed {
  path: string;
  deleted_at: number;
}

/** Delete: only to the Recycle Bin; refused where the bin does not work (IV-07). */
export async function trashFile(path: string): Promise<Trashed> {
  return invoke<Trashed>("trash_file", { path });
}

export async function restoreFile(trashed: Trashed): Promise<void> {
  return invoke("restore_file", { trashed });
}

/** Ctrl+C: the image as shown, with the view rotation (degrees clockwise). */
export async function copyImage(path: string, degrees: number): Promise<void> {
  return invoke("copy_image", { path, degrees });
}

/** Ctrl+Shift+C: the full path. */
export async function copyPath(path: string): Promise<void> {
  return invoke("copy_path", { path });
}

export async function revealFile(path: string): Promise<void> {
  return invoke("reveal_file", { path });
}

/** Windows' "Open with" dialog. */
export async function openWith(path: string): Promise<void> {
  return invoke("open_with", { path });
}
