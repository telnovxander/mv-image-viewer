// Interface preferences, stored as JSON in %LOCALAPPDATA%\Spiegelmond\settings.json by the Rust side
// (MV/STANDARD.md). Outside the app (browser preview, tests) they live in localStorage.
import { loadSettingsText, saveSettingsText } from "./backend";

export type ThemePreference = "system" | "light" | "dark";
export type Mode = "sichel" | "vollmond";
/** The stage behind the image: the theme background (indigo / silver), black, or a checkerboard. */
export type Background = "theme" | "black" | "checker";

/** The order of the images of a folder (iv-core SortOrder); "taken" uses EXIF from IV-04. */
export type SortOrder = "name" | "modified" | "taken";

export const SORT_ORDERS: readonly SortOrder[] = ["name", "modified", "taken"];

/** Memory for decoded neighbours, MB (IV/PROJECT.md). */
export const DEFAULT_CACHE_MB = 512;

export interface Settings {
  theme: ThemePreference;
  mode: Mode;
  background: Background;
  /** Interface language code; null = not chosen yet, follow Windows. */
  language: string | null;
  sort: SortOrder;
  cacheMb: number;
}

export const DEFAULT_SETTINGS: Settings = {
  theme: "system",
  mode: "sichel",
  background: "theme",
  language: null,
  sort: "name",
  cacheMb: DEFAULT_CACHE_MB,
};

export const BACKGROUNDS: readonly Background[] = ["theme", "black", "checker"];

function oneOf<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return allowed.includes(value as T) ? (value as T) : fallback;
}

/** Parse stored settings; unknown or broken values fall back to the defaults. */
export function parseSettings(raw: string | null): Settings {
  if (!raw) return { ...DEFAULT_SETTINGS };
  try {
    const value = JSON.parse(raw) as Record<string, unknown>;
    return {
      theme: oneOf(value.theme, ["system", "light", "dark"], DEFAULT_SETTINGS.theme),
      mode: oneOf(value.mode, ["sichel", "vollmond"], DEFAULT_SETTINGS.mode),
      background: oneOf(value.background, BACKGROUNDS, DEFAULT_SETTINGS.background),
      language:
        typeof value.language === "string" && /^[a-z]{2}(-[a-z]{2})?$/i.test(value.language) ? value.language : null,
      sort: oneOf(value.sort, SORT_ORDERS, DEFAULT_SETTINGS.sort),
      cacheMb:
        typeof value.cacheMb === "number" && Number.isFinite(value.cacheMb) && value.cacheMb >= 64 && value.cacheMb <= 8192
          ? Math.round(value.cacheMb)
          : DEFAULT_CACHE_MB,
    };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

let current: Settings = { ...DEFAULT_SETTINGS };

/** Read the settings once at start, before the first render. */
export async function initSettings(): Promise<Settings> {
  current = parseSettings(await loadSettingsText());
  return current;
}

export function loadSettings(): Settings {
  return current;
}

export function saveSettings(settings: Settings): void {
  current = settings;
  void saveSettingsText(JSON.stringify(settings, null, 2));
}
