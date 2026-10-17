import type { EnvironmentLayer, EnvironmentState } from "../environmentTypes";

export const activeRevision = (l: EnvironmentLayer) =>
  l.revisions.find((r) => r.id === l.currentRevisionId);
export function assetStatus(
  l: EnvironmentLayer,
): "empty" | "stale" | "failed" | "review" | "approved" {
  if (!l.currentSourceId) return "empty";
  const r = activeRevision(l);
  if (!r) return "stale";
  if (!r.report.valid) return "failed";
  return r.approvedAt ? "approved" : "review";
}
export function environmentCounts(s: EnvironmentState) {
  const layers = s.layers.filter((l) => !l.removed);
  return {
    imported: layers.filter((l) => l.currentSourceId).length,
    processed: layers.filter((l) => l.currentRevisionId).length,
    approved: layers.filter((l) => assetStatus(l) === "approved").length,
    review: layers.filter(
      (l) => assetStatus(l) === "review" || assetStatus(l) === "stale",
    ).length,
    failed: layers.filter((l) => assetStatus(l) === "failed").length,
    revisions: s.layers.reduce((n, l) => n + l.revisions.length, 0),
    exports:
      s.packageExports.length +
      s.layers.reduce((n, l) => n + l.exports.length, 0),
  };
}
export function matchesEnvironmentAsset(
  l: EnvironmentLayer,
  search: string,
  type: string,
  status: string,
) {
  return (
    !l.removed &&
    `${l.name} ${l.assetId}`.toLowerCase().includes(search.toLowerCase()) &&
    (!type || l.assetType === type) &&
    (!status || assetStatus(l) === status)
  );
}
