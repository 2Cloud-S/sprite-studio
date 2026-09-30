import { useState } from "react";
import { Check, FolderOpen, RotateCw, Scissors, ShieldAlert } from "lucide-react";
import { ReviewImage } from "./SnapStage";
import { latestCleanupReview } from "./domain/snap";
import { useStudio } from "./store/studio";
import { WorkflowTip } from "./WorkflowTip";
import type { CleanupOptions, Facing, Project } from "./types";

export function CleanupStage({ project, facing }: { project: Project; facing: Facing }) {
  const { settings, activeSnapPreview, cleanedPreview, normalizedPreview, busy, cleanupBusy, choosePython, runCleanup, applyCleanup } = useStudio();
  const [background, setBackground] = useState("auto");
  const [tolerance, setTolerance] = useState("18");
  const [minArea, setMinArea] = useState("2");
  const activeSnapId = project.anchors[facing]?.activeSnapId;
  const review = latestCleanupReview(project, facing);
  const applied = !!review && project.anchors[facing]?.activeCleanupId === review.id;
  const stage = project.imports.find(record => record.id === project.anchors[facing]?.activeImportId)?.stages.cleanup ?? "waiting";
  const toleranceNumber = Number(tolerance);
  const minAreaNumber = Number(minArea);
  const colorValid = /^#?[0-9a-fA-F]{6}$/.test(background.trim()) || background.trim().toLowerCase() === "auto";
  const valid = colorValid && Number.isInteger(toleranceNumber) && toleranceNumber >= 0 && toleranceNumber <= 80 && Number.isInteger(minAreaNumber) && minAreaNumber >= 0 && minAreaNumber <= 64;
  const options: CleanupOptions = { background: background.trim(), tolerance: toleranceNumber, minArea: minAreaNumber };
  const heightDelta = review && project.runtime.neutralHeightTarget !== null ? review.cleanedHeight - project.runtime.neutralHeightTarget : null;

  return <section className="snap-stage cleanup-stage" aria-labelledby="cleanup-stage-title">
    <div className="snap-stage-heading"><div><span className="eyebrow">STAGE 03 · LOCAL PROCESSING</span><h2 id="cleanup-stage-title">Clean and place the silhouette</h2><p>Key the chroma, remove tiny fragments, then place whole pixels in the fixed runtime cell.</p></div><span className={`snap-state ${stage}`}>{cleanupBusy ? "Processing" : stage}</span></div>
    <div className="snap-tool-row"><div className="snap-tool-icon"><Scissors size={19} /></div><div className="snap-tool-copy"><strong>Python · Pillow + OpenCV</strong><span title={settings?.pythonExecutable ?? undefined}>{settings?.pythonEnvironment ?? "No compatible local Python selected"}{settings?.pythonExecutable ? ` · ${settings.pythonExecutable.split(/[\\/]/).slice(-2).join("/")}` : ""}</span></div><button className="button secondary" onClick={choosePython} disabled={busy}><FolderOpen size={16} /> {settings?.pythonExecutable ? "Change Python" : "Locate Python"}</button></div>
    <WorkflowTip label="Output size · fixed">{project.runtime.cellWidth} × {project.runtime.cellHeight} runtime cell, pivot ({project.runtime.pivotX}, {project.runtime.pivotY}){project.runtime.neutralHeightTarget ? `, ${project.runtime.neutralHeightTarget}px neutral-height target` : ""}. Auto chroma, tolerance 18, and 2-pixel speckle removal are editable starting values. Confirm binary alpha and intact edges. A pose that does not fit must be revised, never shrunk to fit.</WorkflowTip>
    <div className="snap-form cleanup-form"><div className="snap-field"><label htmlFor="cleanup-background">Chroma color</label><input id="cleanup-background" className="text-input" value={background} onChange={event => setBackground(event.target.value)} aria-invalid={!colorValid} placeholder="Auto or #17dc40" /></div><div className="snap-field"><label htmlFor="cleanup-tolerance">Tolerance</label><input id="cleanup-tolerance" className="text-input" type="number" min="0" max="80" step="1" value={tolerance} onChange={event => setTolerance(event.target.value)} aria-invalid={!Number.isInteger(toleranceNumber) || toleranceNumber < 0 || toleranceNumber > 80} /></div><div className="snap-field"><label htmlFor="cleanup-area">Remove clusters smaller than</label><input id="cleanup-area" className="text-input" type="number" min="0" max="64" step="1" value={minArea} onChange={event => setMinArea(event.target.value)} aria-invalid={!Number.isInteger(minAreaNumber) || minAreaNumber < 0 || minAreaNumber > 64} /></div></div>
    <div className="snap-run-row"><p>{activeSnapId ? "No smoothing or scaling. Change the chroma settings if a contour pixel disappears." : "Apply a pixel-snap review first. Cleanup always uses that applied native grid."}</p><button className="button secondary" onClick={() => void runCleanup(options)} disabled={!activeSnapId || !settings?.pythonExecutable || !valid || busy}><RotateCw size={16} /> {cleanupBusy ? "Cleaning…" : review ? "Run again" : "Run cleanup"}</button></div>
    {review && <div className="snap-review"><div className="snap-review-heading"><div><span className="eyebrow">{applied ? "ACTIVE CLEANED INPUT" : "REVIEW CANDIDATE"}</span><h3>{applied ? "Applied silhouette" : "Inspect the cutout and placement"}</h3></div><span className="snap-review-meta">{review.cleanedWidth} × {review.cleanedHeight} cutout · {review.normalizedWidth} × {review.normalizedHeight} cell</span></div><div className="cleanup-compare"><ReviewImage image={activeSnapPreview} label="Applied native grid" zoom={2} /><ReviewImage image={cleanedPreview} label="Binary-alpha cutout" zoom={2} /><ReviewImage image={normalizedPreview} label="Fixed-cell placement" zoom={2} /></div><div className="cleanup-metrics"><span>Chroma <strong>#{review.backgroundHex}</strong></span><span>Foreground <strong>{review.foregroundPixels} px</strong></span><span>Speckles removed <strong>{review.removedSpecklePixels} px</strong></span><span>Top-left <strong>{review.placementX}, {review.placementY}</strong></span>{heightDelta !== null && <span>Neutral-height delta <strong>{heightDelta > 0 ? "+" : ""}{heightDelta} px</strong></span>}</div><div className="snap-review-actions"><p><ShieldAlert size={16} /> {heightDelta !== null && heightDelta !== 0 ? "Height differs from the KangiFight target; review the silhouette. No scaling was applied." : "Check for lost details and chroma fringes; this does not approve production art."}</p>{applied ? <span className="snap-applied"><Check size={16} /> Applied for alignment</span> : <button className="button primary" onClick={() => void applyCleanup(review.id)} disabled={busy || !cleanedPreview || !normalizedPreview}><Check size={16} /> Apply cleanup</button>}</div></div>}
  </section>;
}
