import { describe, expect, it } from "vitest";
import { LruBudget, neighbours } from "./preload";

describe("LruBudget", () => {
  it("pushes out the least recently used items to stay within the budget", () => {
    const lru = new LruBudget<string>(100);
    expect(lru.add("a", 40)).toEqual([]);
    expect(lru.add("b", 40)).toEqual([]);
    lru.touch("a");
    expect(lru.add("c", 40)).toEqual(["b"]);
    expect(lru.has("a")).toBe(true);
    expect(lru.bytes).toBe(80);
  });

  it("keeps the newest item even if it alone is too large", () => {
    const lru = new LruBudget<string>(100);
    lru.add("a", 50);
    expect(lru.add("huge", 500)).toEqual(["a"]);
    expect(lru.has("huge")).toBe(true);
  });

  it("refreshing an item replaces its size", () => {
    const lru = new LruBudget<string>(100);
    lru.add("a", 60);
    lru.add("a", 30);
    expect(lru.bytes).toBe(30);
  });
});

describe("neighbours", () => {
  it("are ±2 around the image, the direction of travel first", () => {
    expect(neighbours(5, 10, 1)).toEqual([6, 4, 7, 3]);
    expect(neighbours(5, 10, -1)).toEqual([4, 6, 3, 7]);
  });

  it("stop at the ends of the folder", () => {
    expect(neighbours(0, 3, 1)).toEqual([1, 2]);
    expect(neighbours(2, 3, 1)).toEqual([1, 0]);
    expect(neighbours(0, 1, 1)).toEqual([]);
  });
});
