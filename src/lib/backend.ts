// Calls into the Rust side (src-tauri). Errors arrive as { code, path } (iv-core IvError).
import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";

export interface ImageFile {
  path: string;
  name: string;
  size: number;
  format: "web";
}

export interface BackendError {
  code: "not_found" | "access_denied" | "not_a_file" | "unsupported" | "other";
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
  return inTauri ? convertFileSrc(path) : path;
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
