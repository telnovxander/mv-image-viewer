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

/** A plain number in the language of the interface. */
export function numberText(value: number, language: string, maximumFractionDigits = 0): string {
  return new Intl.NumberFormat(language, { maximumFractionDigits }).format(value);
}

/** Exposure time: "1/250" below one second, "2,5" (seconds) from one second up. */
export function exposureText(seconds: number, language: string): string {
  if (seconds >= 1) return numberText(seconds, language, 1);
  return `1/${numberText(Math.round(1 / seconds), language)}`;
}

/** A date taken from EXIF ("2023-05-12T14:30:00", camera local time, no zone) in the language. */
export function formatTaken(iso: string, language: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})$/.exec(iso);
  if (!match) return iso;
  const [year, month, day, hour, minute] = match.slice(1).map(Number);
  // Built and shown in UTC, so the camera's local time is shown as it is, whatever the zone here.
  const date = new Date(Date.UTC(year, month - 1, day, hour, minute));
  return new Intl.DateTimeFormat(language, { dateStyle: "medium", timeStyle: "short", timeZone: "UTC" }).format(date);
}
