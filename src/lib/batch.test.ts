import { describe, expect, it } from "vitest";
import { BUILT_IN, EMPTY_CHAIN, parsePresets, stepsOf } from "./batch";

describe("batch chains (IV-19)", () => {
  it("turns a chain into steps", () => {
    expect(stepsOf(EMPTY_CHAIN)).toEqual([]);
    expect(stepsOf(BUILT_IN[0].chain)).toEqual([
      { step: "resize", long_side: 2048 },
      { step: "fit_size", max_bytes: 2_097_152 },
      { step: "convert", to: "jpeg" },
      { step: "strip_gps" },
    ]);
  });

  it("reads stored presets and drops broken ones", () => {
    const stored = [{ name: "Mine", chain: { resize: 1000, convert: "png", metadata: "nope", rename: "{YYYY}" } }, { chain: {} }, 5];
    expect(parsePresets(stored)).toEqual([{ name: "Mine", chain: { resize: 1000, fitMb: null, convert: "png", metadata: "keep", rename: "{YYYY}" } }]);
    expect(parsePresets("x")).toEqual([]);
  });
});
