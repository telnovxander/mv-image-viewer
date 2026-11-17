// Neighbours are decoded ahead (±2, IV-03) and kept in a cache limited by memory, least recently
// used out first (IV/PROJECT.md: 512 MB by default). The bookkeeping is pure and tested; the
// decoding uses HTMLImageElement.decode() of the WebView.

/** Least-recently-used bookkeeping with a byte budget. */
export class LruBudget<K> {
  private readonly items = new Map<K, number>();
  private total = 0;

  constructor(private readonly limit: number) {}

  get bytes(): number {
    return this.total;
  }

  has(key: K): boolean {
    return this.items.has(key);
  }

  /** Mark as just used. */
  touch(key: K): void {
    const size = this.items.get(key);
    if (size === undefined) return;
    this.items.delete(key);
    this.items.set(key, size);
  }

  /** Add (or refresh) an item; returns the keys pushed out to stay within the budget. The newest
   * item always stays, even if it alone is larger than the budget. */
  add(key: K, bytes: number): K[] {
    if (this.items.has(key)) {
      this.total -= this.items.get(key) ?? 0;
      this.items.delete(key);
    }
    this.items.set(key, bytes);
    this.total += bytes;
    const evicted: K[] = [];
    for (const [oldKey, size] of this.items) {
      if (this.total <= this.limit || oldKey === key) break;
      this.items.delete(oldKey);
      this.total -= size;
      evicted.push(oldKey);
    }
    return evicted;
  }
}

/** The indexes to decode ahead around `index`: nearest first, the direction of travel first. */
export function neighbours(index: number, count: number, direction: 1 | -1, radius = 2): number[] {
  const result: number[] = [];
  for (let step = 1; step <= radius; step++) {
    for (const sign of [direction, -direction]) {
      const i = index + sign * step;
      if (i >= 0 && i < count) result.push(i);
    }
  }
  return result;
}

/** Decoded images by URL. Keeping the element keeps the WebView's decoded copy warm. */
export class ImageCache {
  private readonly images = new Map<string, HTMLImageElement>();
  private readonly budget: LruBudget<string>;

  constructor(limitBytes: number) {
    this.budget = new LruBudget<string>(limitBytes);
  }

  preload(url: string): void {
    if (this.images.has(url)) {
      this.budget.touch(url);
      return;
    }
    const image = new Image();
    image.decoding = "async";
    image.src = url;
    this.images.set(url, image);
    image
      .decode()
      .then(() => {
        for (const old of this.budget.add(url, image.naturalWidth * image.naturalHeight * 4)) this.images.delete(old);
      })
      .catch(() => this.images.delete(url));
  }

  /** The current image counts as used too. */
  touch(url: string): void {
    this.budget.touch(url);
  }
}
