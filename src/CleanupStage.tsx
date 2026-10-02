import { useEffect, useState } from "react";
import { Check, FolderOpen, RotateCw, Scissors, ShieldAlert } from "lucide-react";
import { ReviewImage } from "./SnapStage";
import { latestAutoFitRun, latestCleanupReview } from "./domain/snap";
import { useStudio } from "./store/studio";
import { WorkflowTip } from "./WorkflowTip";
import type { CleanupOptions, Facing, Project } from "./types";

export function CleanupStage({ project, facing }: { project: Project; facing: Facing }) {
  const { settings, activeSnapPreview, cleanedPreview, normalizedPreview, autoFitPreviews, busy, cleanupBusy, autoFitBusy, cleanupFit, choosePython, runCleanup, applyCleanup, updateRuntime, runAutoFit, applySnap } = useStudio();
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
  const fit = cleanupFit?.projectId === project.id && cleanupFit.facing === facing ? cleanupFit : null;
  const geometryLocked = project.animations.some(animation => animation.frames.length > 0 || animation.historicalFrames.length > 0)
    || Object.values(project.anchors).some(anchor => !!anchor.activeCleanupId);
  const locked = project.runtime.geometryPolicy === "locked" || project.preset === "kangi-fight" || geometryLocked;
  const canExpand = !!fit?.suggested && project.runtime.geometryPolicy === "flexible" && project.preset === "generic" && !geometryLocked;
  const autoFit = latestAutoFitRun(project, facing);
  const [selectedCandidateId, setSelectedCandidateId] = useState<string | null>(null);
  useEffect(() => setSelectedCandidateId(autoFit?.recommendedReviewIds.includes(activeSnapId ?? "") ? activeSnapId ?? null : autoFit?.selectedReviewId ?? null), [autoFit?.id, autoFit?.selectedReviewId, activeSnapId]);
  const selectedCandidate = autoFit?.candidates.find(candidate => candidate.reviewId === selectedCandidateId);
  const expandAndRetry = async () => {
    if (!fit?.suggested || !canExpand) return;
    const { cellWidth, cellHeight, pivotX, pivotY } = fit.suggested;
    if (await updateRuntime(cellWidth, cellHeight, pivotX, pivotY, "custom")) await runCleanup(options);
  };

  return <section className="snap-stage cleanup-stage" aria-labelledby="cleanup-stage-title">
    <div className="snap-stage-heading"><div><span className="eyebrow">STAGE 03 · LOCAL PROCESSING</span><h2 id="cleanup-stage-title">Clean and place the silhouette</h2><p>Key the chroma, remove tiny fragments, then place whole pixels in the fixed runtime cell.</p></div><span className={`snap-state ${stage}`}>{cleanupBusy ? "Processing" : stage}</span></div>
    <div className="snap-tool-row"><div className="snap-tool-icon"><Scissors size={19} /></div><div className="snap-tool-copy"><strong>Python · Pillow + OpenCV</strong><span title={settings?.pythonExecutable ?? undefined}>{settings?.pythonEnvironment ?? "No compatible local Python selected"}{settings?.pythonExecutable ? ` · ${settings.pythonExecutable.split(/[\\/]/).slice(-2).join("/")}` : ""}</span></div><button className="button secondary" onClick={choosePython} disabled={busy}><FolderOpen size={16} /> {settings?.pythonExecutable ? "Change Python" : "Locate Python"}</button></div>
    <WorkflowTip label="Output size · fixed">{project.runtime.cellWidth} × {project.runtime.cellHeight} runtime cell, pivot ({project.runtime.pivotX}, {project.runtime.pivotY}){project.runtime.neutralHeightTarget ? `, ${project.runtime.neutralHeightTarget}px neutral-height target` : ""}. Auto chroma, tolerance 18, and 2-pixel speckle removal are editable starting values. Confirm binary alpha and intact edges. A pose that does not fit must be revised, never shrunk to fit.</WorkflowTip>
    <div className="snap-form cleanup-form"><div className="snap-field"><label htmlFor="cleanup-background">Chroma color</label><input id="cleanup-background" className="text-input" value={background} onChange={event => setBackground(event.target.value)} aria-invalid={!colorValid} placeholder="Auto or #17dc40" /></div><div className="snap-field"><label htmlFor="cleanup-tolerance">Tolerance</label><input id="cleanup-tolerance" className="text-input" type="number" min="0" max="80" step="1" value={tolerance} onChange={event => setTolerance(event.target.value)} aria-invalid={!Number.isInteger(toleranceNumber) || toleranceNumber < 0 || toleranceNumber > 80} /></div><div className="snap-field"><label htmlFor="cleanup-area">Remove clusters smaller than</label><input id="cleanup-area" className="text-input" type="number" min="0" max="64" step="1" value={minArea} onChange={event => setMinArea(event.target.value)} aria-invalid={!Number.isInteger(minAreaNumber) || minAreaNumber < 0 || minAreaNumber > 64} /></div></div>
    <p className="animation-hint">Green fringe cleanup is {project.workflow.greenFringeDespeckle ? "on" : "off"}. When on, it keys dark green residues close to a green backdrop; turn it off in Project workflow settings if the character has intentional green edge details.</p>
    <div className="snap-run-row"><p>{activeSnapId ? "No smoothing or scaling. Change the chroma settings if a contour pixel disappears." : "Apply a pixel-snap review first. Cleanup always uses that applied native grid."}</p><button className="button secondary" onClick={() => void runCleanup(options)} disabled={!activeSnapId || !settings?.pythonExecutable || !valid || busy}><RotateCw size={16} /> {cleanupBusy ? "Cleaning…" : review ? "Run again" : "Run cleanup"}</button></div>
    {fit && <div className="cleanup-fit-alert" role="status">
      <strong>{locked ? "Sprite exceeds locked runtime geometry" : "Foreground does not fit the runtime cell"}</strong>
      <p>The cleaned silhouette measures {fit.foregroundWidth} × {fit.foregroundHeight} px. It cannot be placed intact in {fit.cellWidth} × {fit.cellHeight} at pivot ({fit.pivotX}, {fit.pivotY}). Nothing was cropped or scaled.</p>
      {project.runtime.neutralHeightTarget !== null && <p>Neutral target: {project.runtime.neutralHeightTarget} px (±{project.runtime.neutralTolerancePx} px).</p>}
      <p>{locked ? "Auto-fit tries progressively coarser snap grids against this exact cell and pivot. Each result remains a review candidate." : "You can expand this editable project cell, or keep it and try auto-fit."}</p>
      <div className="cleanup-fit-actions">
        {!locked && canExpand && fit.suggested && <button className="button primary" disabled={busy} onClick={() => void expandAndRetry()}>Use suggested {fit.suggested.cellWidth} × {fit.suggested.cellHeight} cell and retry</button>}
        <button className={locked ? "button primary" : "button secondary"} disabled={busy || !settings?.snapperExecutable || !settings?.pythonExecutable || !activeSnapId} onClick={() => void runAutoFit(options)}>{autoFitBusy ? "Finding a fitting grid…" : locked ? "Auto-fit to locked cell" : "Auto-fit to current cell"}</button>
      </div>
      {locked && fit.suggested && <details className="animation-hint"><summary>Diagnostic: suggested larger cell</summary><small>{fit.suggested.cellWidth} × {fit.suggested.cellHeight} at pivot ({fit.suggested.pivotX},{fit.suggested.pivotY}) could hold this silhouette, but using it would violate the locked runtime contract. No geometry change is offered here.</small></details>}
      {geometryLocked && <small>Normalized artwork already applied elsewhere also prevents geometry changes.</small>}
    </div>}
    {autoFit && <div className="cleanup-fit-results">
      <div className="snap-review-heading"><div><span className="eyebrow">AUTO-FIT REVIEW{activeSnapId === selectedCandidateId ? " · APPLIED" : " · NOT APPLIED"}</span><h3>{autoFit.selectedReviewId ? "Review a fitting snap" : "No fitting candidate in this run"}</h3></div><span className="snap-review-meta">{autoFit.candidates.length} bounded attempts</span></div>
      {autoFit.selectedReviewId ? <>
        <p className="animation-hint">The candidates below fit {autoFit.cellWidth} × {autoFit.cellHeight} at pivot ({autoFit.pivotX},{autoFit.pivotY}). Compare silhouette quality before Apply; cleanup must run again afterward.</p>
        <div className="auto-fit-options" role="group" aria-label="Fitting snap candidates">
          {autoFit.recommendedReviewIds.map(id => { const candidate = autoFit.candidates.find(item => item.reviewId === id); if (!candidate) return null; const delta = autoFit.neutralHeightTarget === null ? null : Math.abs(candidate.foregroundHeight - autoFit.neutralHeightTarget); return <button key={id} type="button" className={selectedCandidateId === id ? "selected" : ""} onClick={() => setSelectedCandidateId(id)} aria-pressed={selectedCandidateId === id}><strong>Pixel size {candidate.pixelSize.toFixed(1)}</strong><span>Foreground {candidate.foregroundWidth} × {candidate.foregroundHeight} ✓</span><small>{delta === null ? "Fits pivot" : delta <= autoFit.neutralTolerancePx ? `Neutral target ✓ (${delta}px away)` : `${delta}px from neutral target`}</small></button>; })}
        </div>
        {selectedCandidate && <ReviewImage image={autoFitPreviews[selectedCandidate.reviewId] ?? null} label="Selected auto-fit native grid" zoom={2} />}
        <div className="animation-actions">{activeSnapId === selectedCandidateId ? <span className="snap-applied"><Check size={15} /> Applied</span> : <button className="button primary" disabled={busy || !selectedCandidate?.fits || !autoFitPreviews[selectedCandidate.reviewId]} onClick={() => selectedCandidateId && void applySnap(selectedCandidateId)}><Check size={15} /> Apply selected snap</button>}<small>Only Apply changes the active snap. The original import and all earlier reviews stay intact.</small></div>
      </> : <p className="animation-hint">No candidate met the fixed cell and pivot. Try a different source generation or manually choose a coarser pixel size.</p>}
    </div>}
    {review && <div className="snap-review"><div className="snap-review-heading"><div><span className="eyebrow">{applied ? "ACTIVE CLEANED INPUT" : "REVIEW CANDIDATE"}</span><h3>{applied ? "Applied silhouette" : "Inspect the cutout and placement"}</h3></div><span className="snap-review-meta">{review.cleanedWidth} × {review.cleanedHeight} cutout · {review.normalizedWidth} × {review.normalizedHeight} cell</span></div><div className="cleanup-compare"><ReviewImage image={activeSnapPreview} label="Applied native grid" zoom={2} /><ReviewImage image={cleanedPreview} label="Binary-alpha cutout" zoom={2} /><ReviewImage image={normalizedPreview} label="Fixed-cell placement" zoom={2} /></div><div className="cleanup-metrics"><span>Chroma <strong>#{review.backgroundHex}</strong></span><span>Foreground <strong>{review.foregroundPixels} px</strong></span><span>Speckles removed <strong>{review.removedSpecklePixels} px</strong></span><span>Top-left <strong>{review.placementX}, {review.placementY}</strong></span>{heightDelta !== null && <span>Neutral-height delta <strong>{heightDelta > 0 ? "+" : ""}{heightDelta} px</strong></span>}</div><div className="snap-review-actions"><p><ShieldAlert size={16} /> {heightDelta !== null && heightDelta !== 0 ? "Height differs from the KangiFight target; review the silhouette. No scaling was applied." : "Check for lost details and chroma fringes; this does not approve production art."}</p>{applied ? <span className="snap-applied"><Check size={16} /> Applied for alignment</span> : <button className="button primary" onClick={() => void applyCleanup(review.id)} disabled={busy || !cleanedPreview || !normalizedPreview}><Check size={16} /> Apply cleanup</button>}</div></div>}
  </section>;
}
