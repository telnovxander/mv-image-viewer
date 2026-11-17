import type { Mode, ThemePreference } from "./settings";

export type Theme = "light" | "dark";

/** The theme to show for a preference; "system" follows the Windows setting. */
export function resolveTheme(preference: ThemePreference, systemDark: boolean): Theme {
  if (preference === "system") return systemDark ? "dark" : "light";
  return preference;
}

/** Vollmond on, Sichel off: the mode toggle. */
export function toggleMode(mode: Mode): Mode {
  return mode === "sichel" ? "vollmond" : "sichel";
}

export function applyAppearance(theme: Theme, mode: Mode, root: HTMLElement = document.documentElement) {
  root.dataset.theme = theme;
  root.dataset.mode = mode;
}
