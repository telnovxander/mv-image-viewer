// Languages of the interface. They are found from the files in locales/ (MV/I18N.md):
// a new language is a new file, no code changes.
export type Messages = { _meta: { name: string } } & Record<string, unknown>;

const FILES = import.meta.glob<Messages>("../../locales/*.json", { eager: true, import: "default" });

export const DEFAULT_LANGUAGE = "en";

/** Language code → messages, e.g. { en: {...}, ru: {...} }. */
export const LOCALES: Record<string, Messages> = Object.fromEntries(
  Object.entries(FILES).map(([path, messages]) => [path.replace(/^.*\/([\w-]+)\.json$/, "$1"), messages]),
);

export interface Language {
  code: string;
  name: string;
}

/** Languages for the selector: code and own name, English first, then by code. */
export function availableLanguages(locales: Record<string, Messages> = LOCALES): Language[] {
  return Object.entries(locales)
    .map(([code, messages]) => ({ code, name: messages._meta.name }))
    .sort((a, b) => (a.code === DEFAULT_LANGUAGE ? -1 : b.code === DEFAULT_LANGUAGE ? 1 : a.code.localeCompare(b.code)));
}

/**
 * The language to start with: the saved choice if it still exists, otherwise the first system
 * language we have (by its base code: "de-CH" → "de"), otherwise English.
 */
export function pickLanguage(saved: string | null, systemLanguages: readonly string[], codes: readonly string[]): string {
  if (saved && codes.includes(saved)) return saved;
  for (const tag of systemLanguages) {
    const lower = tag.toLowerCase();
    if (codes.includes(lower)) return lower;
    const base = lower.split("-")[0];
    if (codes.includes(base)) return base;
  }
  return DEFAULT_LANGUAGE;
}
