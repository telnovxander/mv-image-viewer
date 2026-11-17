// The window title: "name — 1 / 340" (IV/PROJECT.md), the program name with nothing open.

export const APP_NAME = "Spiegelmond";

export function windowTitle(name: string | null, index: number, count: number, language: string): string {
  if (name === null || count < 1) return APP_NAME;
  const number = new Intl.NumberFormat(language);
  return `${name} — ${number.format(index)} / ${number.format(count)}`;
}
