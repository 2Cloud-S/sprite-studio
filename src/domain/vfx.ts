import type { VfxProcessing, VfxProfile } from "../vfxTypes";
// JSON object member order is not identity. Rust's serde_json::Value sorts
// keys, while edited JavaScript objects retain insertion order. Array order
// (frames, holds, colors, coordinates) remains significant.
export function vfxValueKey(value: unknown): string {
  return JSON.stringify(value, (_key, v) =>
    v && typeof v === "object" && !Array.isArray(v)
      ? Object.fromEntries(
          Object.entries(v).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)),
        )
      : v,
  );
}
export const lightStudy = (): VfxProfile => ({
  cell: [24, 24],
  anchor: [12, 12],
  sourceOrientation: "east",
  mirrorPolicy: "horizontal_about_anchor",
  looping: false,
  tickHz: 60,
  hitStopPolicy: "pause",
  completion: "hide",
  allowEmptyTerminal: false,
  palette: {
    name: "Light Contact · proposed subset",
    authorityId: null,
    packageReference: null,
    colors: ["#FFF1D2", "#C9C2B8", "#38344B"],
    enforce: true,
  },
});
export const defaultProcessing = (): VfxProcessing => ({
  offset: [0, 0],
  pixelPitch: 1,
  alphaThreshold: null,
  chroma: null,
  tolerance: 0,
  remapPalette: false,
});
export function frameAtTick(
  tick: number,
  holds: number[],
  looping: boolean,
  completion = "hide",
) {
  const total = holds.reduce((a, b) => a + b, 0);
  if (!holds.length || holds.some((t) => !Number.isInteger(t) || t <= 0))
    return { index: null, completed: false, total: 0 };
  if (!looping && tick >= total)
    return {
      index: completion === "empty_terminal" ? holds.length - 1 : null,
      completed: true,
      total,
    };
  let age = looping ? tick % total : Math.max(0, tick);
  for (let i = 0; i < holds.length; i++) {
    if (age < holds[i]) return { index: i, completed: false, total };
    age -= holds[i];
  }
  return { index: null, completed: true, total };
}
export function advanceVfxClock(
  tick: number,
  delta: number,
  hitStop: boolean,
  paused: boolean,
  policy: "pause" | "continue",
) {
  return paused || (hitStop && policy === "pause")
    ? tick
    : tick + Math.max(0, Math.floor(delta));
}
// Anchor coordinates are integer canvas boundaries; mirror pixel centers about that fixed point.
export const mirroredPixelX = (x: number, anchor: number) => 2 * anchor - 1 - x;
