import { describe, expect, it } from "vitest";
import formatsRs from "../../crates/iv-core/src/formats.rs?raw";
import { DECODED_EXTENSIONS, isDecoded, WEB_EXTENSIONS } from "./formats";

describe("WEB_EXTENSIONS", () => {
  it("is the same list as in iv-core", () => {
    const list = formatsRs.match(/WEB_EXTENSIONS: &\[&str\] = &\[([^\]]*)\]/)?.[1] ?? "";
    expect([...list.matchAll(/"(\w+)"/g)].map((m) => m[1])).toEqual(WEB_EXTENSIONS);
  });
});

describe("DECODED_EXTENSIONS", () => {
  it("are the TIFF and RAW lists of iv-core", () => {
    const list = (name: string) => [...(formatsRs.match(new RegExp(`${name}: &\\[&str\\] = &\\[([^\\]]*)\\]`))?.[1] ?? "").matchAll(/"(\w+)"/g)].map((m) => m[1]);
    expect([...list("TIFF_EXTENSIONS"), ...list("RAW_EXTENSIONS")]).toEqual(DECODED_EXTENSIONS);
  });

  it("decide the way an image is served", () => {
    expect(isDecoded("C:\\Fotos\\IMG_1.NEF")).toBe(true);
    expect(isDecoded("scan.tiff")).toBe(true);
    expect(isDecoded("photo.jpg")).toBe(false);
  });
});
