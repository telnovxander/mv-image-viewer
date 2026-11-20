// IV-19: the chain of a batch as the window edits it, and presets.
import type { BatchStep } from "./backend";

export interface Chain {
  resize: number | null;
  fitMb: number | null;
  convert: "keep" | "jpeg" | "png";
  metadata: "keep" | "strip_gps" | "strip_all";
  rename: string | null;
}

export interface Preset {
  name: string;
  chain: Chain;
}

export const EMPTY_CHAIN: Chain = { resize: null, fitMb: null, convert: "keep", metadata: "keep", rename: null };

/** Built-in presets; their names are locale keys (`batch.preset_*`). */
export const BUILT_IN: readonly Preset[] = [
  { name: "messenger", chain: { resize: 2048, fitMb: 2, convert: "jpeg", metadata: "strip_gps", rename: null } },
  { name: "publish", chain: { resize: 3000, fitMb: null, convert: "jpeg", metadata: "strip_all", rename: null } },
];

export function stepsOf(chain: Chain): BatchStep[] {
  const steps: BatchStep[] = [];
  if (chain.resize) steps.push({ step: "resize", long_side: chain.resize });
  if (chain.fitMb) steps.push({ step: "fit_size", max_bytes: Math.round(chain.fitMb * 1024 * 1024) });
  if (chain.convert !== "keep") steps.push({ step: "convert", to: chain.convert });
  if (chain.metadata !== "keep") steps.push({ step: chain.metadata });
  if (chain.rename) steps.push({ step: "rename_by_date", template: chain.rename });
  return steps;
}

/** Stored presets from the settings file; broken entries are dropped. */
export function parsePresets(value: unknown): Preset[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((p): Preset[] => {
    if (typeof p !== "object" || p === null || typeof (p as Preset).name !== "string") return [];
    const c = (p as Preset).chain ?? {};
    const num = (v: unknown) => (typeof v === "number" && v > 0 ? v : null);
    return [
      {
        name: (p as Preset).name,
        chain: {
          resize: num(c.resize),
          fitMb: num(c.fitMb),
          convert: ["keep", "jpeg", "png"].includes(c.convert) ? c.convert : "keep",
          metadata: ["keep", "strip_gps", "strip_all"].includes(c.metadata) ? c.metadata : "keep",
          rename: typeof c.rename === "string" && c.rename !== "" ? c.rename : null,
        },
      },
    ];
  });
}
