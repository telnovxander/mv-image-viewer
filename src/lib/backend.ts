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

/** The URL of a local file for <img>, through the Tauri asset protocol. */
export function imageUrl(file: ImageFile): string {
  return convertFileSrc(file.path);
}
