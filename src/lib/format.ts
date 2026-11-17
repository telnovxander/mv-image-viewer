// Numbers, sizes and dates in the language of the interface, always through Intl (MV/I18N.md).
import type { TFunction } from "i18next";

const UNITS = ["b", "kb", "mb", "gb", "tb"] as const;

/** A file size with base 1024: ru "1 234,5 КБ", de "1.234,5 KB", en "1,234.5 KB". */
export function formatSize(bytes: number, language: string, t: TFunction): string {
  let value = bytes;
  let unit = 0;
  while (value >= 1024 * 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  if (unit === 0 && value >= 1024) {
    value /= 1024;
    unit = 1;
  }
  const number = new Intl.NumberFormat(language, { maximumFractionDigits: unit === 0 ? 0 : 1 }).format(value);
  return `${number} ${t(`units.${UNITS[unit]}`)}`;
}

/** Date and time of a file in the short local form. */
export function formatDateTime(ms: number, language: string): string {
  return new Intl.DateTimeFormat(language, { dateStyle: "short", timeStyle: "short" }).format(new Date(ms));
}
