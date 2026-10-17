import type { Animation, BatchSnapReview } from "./types";

function regionText(region: { width: number; height: number; area: number } | null): string {
  return region ? `${region.width} × ${region.height} · ${region.area} px` : "Not detected";
}

export function ScaleConsistencyReview({ animation, review }: { animation: Animation; review: BatchSnapReview }) {
  if (review.scaleMetrics.length === 0) return null;
  return <section className="scale-consistency-review" aria-label="Scale consistency review">
    <div className="snap-review-heading"><div><span className="eyebrow">POST-SNAP REVIEW</span><h3>Scale consistency</h3></div><span className="snap-review-meta">{review.scaleMode === "sharedAnimationScale" ? `Shared ${review.pixelSize ?? "—"} px` : "Per-frame Auto"}</span></div>
    {review.scaleInconsistency && <div className="scale-warning" role="alert"><strong>⚠ Scale inconsistency detected</strong><p>One or more frames used a different logical pixel pitch. Use Shared animation scale or inspect those frames manually.</p></div>}
    <p className="animation-hint">Compare logical pitch and the approximate upper/head and central body-core regions. Those region measurements are visual aids, not anatomical recognition; crouch, jump, and stretch extents are never treated as failures by themselves.</p>
    <div className="scale-table-wrap"><table className="scale-table"><thead><tr><th>Frame</th><th>Foreground</th><th>Auto estimate</th><th>Used pitch</th><th>Upper/head region*</th><th>Body core*</th><th>Area</th></tr></thead><tbody>{review.scaleMetrics.map((metric, index) => {
      const currentIndex = animation.activeRawFrameIds.indexOf(metric.sourceFrameId);
      const label = currentIndex >= 0 ? currentIndex + 1 : index + 1;
      return <tr key={metric.sourceFrameId} className={metric.pitchDiffers ? "scale-row-warning" : ""}><th scope="row">{String(label).padStart(2, "0")}{metric.pitchDiffers ? " ⚠" : ""}</th><td>{metric.foregroundWidth} × {metric.foregroundHeight}</td><td>{metric.detectedPixelSize?.toFixed(1) ?? "—"} px</td><td>{metric.effectivePixelSize?.toFixed(1) ?? "—"} px</td><td>{regionText(metric.headRegion)}</td><td>{regionText(metric.bodyCoreRegion)}</td><td>{metric.foregroundArea} px</td></tr>;
    })}</tbody></table></div>
    <small>*Heuristic occupied regions; verify actual head and pelvis alignment visually in the snapped previews.</small>
  </section>;
}
