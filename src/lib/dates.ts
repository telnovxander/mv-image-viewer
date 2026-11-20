// The dates table of Vollmond (IV-14): filters and the text of a date, pure and tested.
import type { DateRow, Stamp } from "./backend";

export type DateFilter = "all" | "mismatches" | "no_date_taken";

export function filterRows(rows: readonly DateRow[], filter: DateFilter): DateRow[] {
  if (filter === "mismatches") return rows.filter((r) => r.mismatch);
  if (filter === "no_date_taken") return rows.filter((r) => r.taken === null);
  return rows.slice();
}

/** A stamp shown in the language of the interface (it is a local wall-clock time, no zone). */
export function stampText(stamp: Stamp | null, language: string): string {
  if (!stamp) return "—";
  const [y, mo, d, h, mi, s] = stamp.split(/\D/).map(Number);
  const date = new Date(y, mo - 1, d, h, mi, s);
  return new Intl.DateTimeFormat(language, { dateStyle: "short", timeStyle: "medium" }).format(date);
}

/** The visible rows of a list scrolled to `top`, `height` high, with fixed `rowHeight`. */
export function visibleRows(top: number, height: number, count: number, rowHeight: number, overscan = 10): [number, number] {
  if (count <= 0 || height <= 0) return [0, 0];
  const first = Math.max(0, Math.floor(top / rowHeight) - overscan);
  const last = Math.min(count, Math.ceil((top + height) / rowHeight) + overscan);
  return [first, Math.max(first, last)];
}
