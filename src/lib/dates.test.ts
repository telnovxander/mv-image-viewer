import { describe, expect, it } from "vitest";
import type { DateRow } from "./backend";
import { filterRows, stampText, visibleRows } from "./dates";

const stamp = "2023-05-12T14:30:00";
const row = (mismatch: boolean, taken: boolean): DateRow => ({
  path: "x.jpg",
  taken: taken ? stamp : null,
  from_name: null,
  modified: stamp,
  proposed: stamp,
  mismatch,
  suspicious: false,
});

describe("dates table (IV-14)", () => {
  it("filters", () => {
    const rows = [row(false, true), row(true, true), row(true, false)];
    expect(filterRows(rows, "all")).toHaveLength(3);
    expect(filterRows(rows, "mismatches")).toHaveLength(2);
    expect(filterRows(rows, "no_date_taken")).toHaveLength(1);
  });

  it("shows dates in the language and a dash for none", () => {
    expect(stampText(stamp, "de")).toBe("12.05.23, 14:30:00");
    expect(stampText(null, "de")).toBe("—");
  });

  it("renders only the rows in view of 50 000", () => {
    const [first, last] = visibleRows(22 * 25_000, 440, 50_000, 22);
    expect(first).toBe(24_990);
    expect(last - first).toBeLessThan(45);
  });
});
