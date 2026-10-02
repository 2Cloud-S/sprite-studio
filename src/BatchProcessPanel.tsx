import { useEffect, useState } from "react";
import { Check, FolderOpen, Maximize2, RotateCw } from "lucide-react";
import { useStudio } from "./store/studio";
import { WorkflowTip } from "./WorkflowTip";
import { ImageReviewDialog, type ReviewFrame } from "./ImageReviewDialog";
import { UpscaleHandoffPanel } from "./UpscaleHandoffPanel";
import { latestBatchAutoFitRun, latestBatchSnapReview } from "./domain/snap";
import type { Animation, CleanupOptions, Project, SnapOptions } from "./types";

export function BatchProcessPanel({ animation, project }: { animation: Animation; project: Project }) {
  const { settings, busy, batchSnapPreviews, batchAutoFitPreviews, batchAutoFitBusy, batchCleanupFit, batchCleanedPreviews, batchNormalizedPreviews, chooseSnapper, choosePython, runBatchSnap, runBatchAutoFit, loadBatchAutoFitPreview, applyBatchSnap, runBatchCleanup, approveBatchClean, applyBatchCleanup, updateWorkflow, updateRuntime } = useStudio();
  const [colors, setColors] = useState(project.workflow.kColors);
  const [pixelSize, setPixelSize] = useState("");
  const [palette, setPalette] = useState("");
  const [background, setBackground] = useState(project.workflow.chroma);
  const [tolerance, setTolerance] = useState(24);
  const [minArea, setMinArea] = useState(2);
  const [selectedSnapFrameId, setSelectedSnapFrameId] = useState<string | null>(null);
  const [selectedAutoFitId, setSelectedAutoFitId] = useState<string | null>(null);
  const [selectedAutoFitFrameId, setSelectedAutoFitFrameId] = useState<string | null>(null);
  const extraction = animation.extractions.find(item => item.id === animation.activeExtractionId);
  useEffect(() => setColors(project.workflow.kColors), [project.id, project.workflow.kColors]);
  useEffect(() => { setBackground(extraction ? `#${extraction.backgroundHex}` : project.workflow.chroma); }, [extraction?.id, project.workflow.chroma]);
  const snap = latestBatchSnapReview(animation);
  const autoFit = latestBatchAutoFitRun(project, animation);
  useEffect(() => setSelectedAutoFitId(autoFit?.recommendedReviewIds.includes(animation.activeBatchSnapId ?? "") ? animation.activeBatchSnapId ?? null : autoFit?.selectedReviewId ?? null), [autoFit?.id, autoFit?.selectedReviewId, animation.activeBatchSnapId]);
  useEffect(() => setSelectedSnapFrameId(null), [snap?.id]);
  const activeUpscaleIds = animation.activeRawFrameIds.map(id => animation.activeUpscaledFrameIds[id]).filter(Boolean);
  const cleanup = animation.batchCleanups.slice().reverse().find(item => item.snapReviewId === animation.activeBatchSnapId && (project.workflow.upscaleMode === "automatic" ? item.upscaleFrameIds.length === 0 : item.upscaleFrameIds.join() === activeUpscaleIds.join()));
  const snappedApplied = !!snap && animation.activeBatchSnapId === snap.id;
  const cleanupApplied = !!cleanup && animation.activeBatchCleanupId === cleanup.id;
  const target = project.runtime.neutralHeightTarget;
  const maxRawHeight = Math.max(...animation.activeRawFrameIds.map(id => animation.rawFrames.find(item => item.id === id)?.height ?? 0));
  const suggestedPixelSize = target ? Math.ceil(maxRawHeight / target) + 1 : null;
  const fit = batchCleanupFit?.projectId === project.id && batchCleanupFit.animationId === animation.id ? batchCleanupFit : null;
  const geometryLocked = project.runtime.geometryPolicy === "locked" || project.preset === "kangi-fight";
  const canExpand = project.runtime.geometryPolicy === "flexible" && project.preset === "generic" && !!fit?.suggested;
  const smallest = animation.activeRawFrameIds.map(id => animation.rawFrames.find(item => item.id === id)).filter(item => item !== undefined).reduce((value, item) => Math.min(value, item.width, item.height), Infinity);
  const parsedSize = pixelSize.trim() ? Number(pixelSize) : null;
  const parsedPalette = palette.trim() || null;
  const snapValid = Number.isInteger(colors) && colors >= 2 && colors <= 256 && (parsedSize === null || (Number.isInteger(parsedSize) && parsedSize >= 1 && parsedSize <= smallest)) && (!parsedPalette || (parsedPalette.split(",").length <= colors && parsedPalette.split(",").every(value => /^[0-9a-fA-F]{6}$/.test(value.trim()))));
  const cleanupValid = (background === "auto" || /^#?[0-9a-fA-F]{6}$/.test(background)) && Number.isInteger(tolerance) && tolerance >= 0 && tolerance <= 80 && Number.isInteger(minArea) && minArea >= 0 && minArea <= 64;
  const snapOptions: SnapOptions = { colors, pixelSize: parsedSize, palette: parsedPalette };
  const cleanupOptions: CleanupOptions = { background, tolerance, minArea };
  const snappedReviewFrames: ReviewFrame[] = snap?.frames.flatMap((frame, index) => {
    const preview = batchSnapPreviews[frame.sourceFrameId];
    return preview ? [{ id: frame.sourceFrameId, label: `Snapped frame ${index + 1}`, dataUrl: preview.dataUrl, width: frame.width, height: frame.height }] : [];
  }) ?? [];
  const autoFitReview = animation.batchSnaps.find(item => item.id === selectedAutoFitId);
  const autoFitCandidate = autoFit?.candidates.find(item => item.reviewId === selectedAutoFitId);
  const autoFitImages = selectedAutoFitId ? batchAutoFitPreviews[selectedAutoFitId] : undefined;
  const autoFitReviewFrames: ReviewFrame[] = autoFitReview?.frames.flatMap((frame, index) => {
    const preview = autoFitImages?.[frame.sourceFrameId];
    return preview ? [{ id: frame.sourceFrameId, label: `Auto-fit frame ${index + 1}`, dataUrl: preview.dataUrl, width: frame.width, height: frame.height }] : [];
  }) ?? [];
  const selectAutoFit = (id: string) => { setSelectedAutoFitId(id); void loadBatchAutoFitPreview(id); };
  const expandAndRetry = async () => {
    if (!fit?.suggested || !canExpand) return;
    const { cellWidth, cellHeight, pivotX, pivotY } = fit.suggested;
    if (await updateRuntime(cellWidth, cellHeight, pivotX, pivotY, "custom")) await runBatchCleanup(cleanupOptions);
  };

  if (animation.activeRawFrameIds.length === 0) return null;
  return <section className="pose-panel batch-panel" aria-label="Batch pixel processing">
    <div className="animation-panel-heading"><div><span className="eyebrow">BATCH PROCESSING</span><h4>Raw poses to fixed cells</h4></div><span>{animation.activeRawFrameIds.length} frames</span></div>
    <p className="animation-hint">Correct order: recover frame → confirm native review → snap each frame once → {project.workflow.upscaleMode === "manual-handoff" ? "manual upscale handoff → " : ""}background clean → runtime normalize → frame align → export. Original crops and review outputs are never overwritten.</p>
    {target && <p className="animation-hint">Auto uses one shared {suggestedPixelSize}px pixel grid, estimated from the tallest raw pose to approach the {target}px neutral-height target. This changes pixel-grid recovery, not the size of a normalized frame.</p>}
    <div className="batch-stage"><div className="animation-panel-heading"><h4>1 · Pixel snap all</h4><span className={`snap-state ${animation.stages.snap}`}>{animation.stages.snap}</span></div>
      <WorkflowTip label="Per-frame snap · starting values">{project.workflow.kColors} colors and Auto pixel size. 256 is the generous repo starting point: it preserves subtle palette while flattening sub-pixel noise. {target ? `One shared grid approaches the ${target}px neutral-height target.` : "Override pixel size only when the recovered grid is wrong."} Snap each recovered crop once, never the entire board or an upscaled result. The local CLI currently provides a native PNG; optional output slots remain empty when unavailable.</WorkflowTip>
      <div className="batch-controls"><label>Colors<input className="text-input" type="number" min="2" max="256" value={colors} onChange={event => setColors(Number(event.target.value))} /></label><label>Pixel size<input className="text-input" type="number" min="1" value={pixelSize} placeholder="Auto" onChange={event => setPixelSize(event.target.value)} /></label><label>Palette<input className="text-input" value={palette} placeholder="Optional hex colors" onChange={event => setPalette(event.target.value)} /></label></div>
      <div className="animation-actions">{!settings?.snapperExecutable && <button className="button secondary" disabled={busy} onClick={() => void chooseSnapper()}><FolderOpen size={15} /> Locate CLI</button>}<button className="button secondary" disabled={busy || animation.stages.native_review === "review" || !settings?.snapperExecutable || !snapValid} onClick={() => void runBatchSnap(snapOptions)}><RotateCw size={15} /> {snap ? "Snap again" : "Snap all"}</button>{snap && !snappedApplied && <button className="button primary" disabled={busy || snap.frames.length !== animation.activeRawFrameIds.length} onClick={() => void applyBatchSnap(snap.id)}><Check size={15} /> Apply snapped frames</button>}{snappedApplied && <span className="snap-applied"><Check size={15} /> Applied</span>}</div>
      {snap && <><p className="animation-hint">Select any snapped frame to inspect its native pixels and zoom in before applying the batch.</p><div className="pose-raw-strip" aria-label="Snapped frame previews">{snap.frames.map((frame, index) => <button type="button" className="pose-raw-item snap-frame-open" key={frame.sourceFrameId} disabled={!batchSnapPreviews[frame.sourceFrameId]} onClick={() => setSelectedSnapFrameId(frame.sourceFrameId)} aria-haspopup="dialog" aria-label={`Open snapped frame ${index + 1} preview, ${frame.width} by ${frame.height} pixels`}><span className="snap-frame-heading"><span>{String(index + 1).padStart(2, "0")}</span><Maximize2 size={14} aria-hidden="true" /></span>{batchSnapPreviews[frame.sourceFrameId] && <img src={batchSnapPreviews[frame.sourceFrameId].dataUrl} alt="" />}<small>{frame.width}×{frame.height}</small></button>)}</div></>}
    </div>
    {project.workflow.upscaleMode === "manual-handoff" && snappedApplied && <UpscaleHandoffPanel animation={animation} project={project} />}
    <div className="batch-stage"><div className="animation-panel-heading"><h4>{project.workflow.upscaleMode === "manual-handoff" ? "3" : "2"} · Background clean</h4><span className={`snap-state ${animation.stages.cleanup}`}>{animation.stages.cleanup}</span></div>
      <WorkflowTip label="Fixed-cell output · required">Each frame becomes {project.runtime.cellWidth} × {project.runtime.cellHeight} at pivot ({project.runtime.pivotX}, {project.runtime.pivotY}) with binary alpha and whole-pixel placement. Start with the detected board chroma, tolerance 24, and 2-pixel speckle area; adjust if edges vanish or chroma remains. No scaling is used. If a pose overflows, revise or re-snap its source.</WorkflowTip>
      <div className="batch-controls"><label>Background<input className="text-input" value={background} onChange={event => setBackground(event.target.value)} /></label><label>Tolerance<input className="text-input" type="number" min="0" max="80" value={tolerance} onChange={event => setTolerance(Number(event.target.value))} /></label><label>Speckle area<input className="text-input" type="number" min="0" max="64" value={minArea} onChange={event => setMinArea(Number(event.target.value))} /></label></div>
      <label className="animation-hint"><input type="checkbox" checked={project.workflow.greenFringeDespeckle} disabled={busy} onChange={event => void updateWorkflow({ ...project.workflow, greenFringeDespeckle: event.target.checked })} /> Green fringe cleanup near keyed background (speckle area above controls isolated fragments)</label>
      <div className="animation-actions">{!settings?.pythonExecutable && <button className="button secondary" disabled={busy} onClick={() => void choosePython()}><FolderOpen size={15} /> Locate Python</button>}<button className="button secondary" disabled={busy || !snappedApplied || (project.workflow.upscaleMode === "manual-handoff" && !animation.upscaleApproved) || !settings?.pythonExecutable || !cleanupValid} onClick={() => void runBatchCleanup(cleanupOptions)}><RotateCw size={15} /> {cleanup ? "Clean again" : "Clean all"}</button>{cleanup && animation.stages.cleanup === "review" && <button className="button primary" disabled={busy || cleanup.frames.length !== snap?.frames.length} onClick={() => void approveBatchClean(cleanup.id)}><Check size={15} /> Apply cleaned frames</button>}{cleanup && animation.stages.cleanup === "complete" && <span className="snap-applied"><Check size={15} /> Clean applied</span>}</div>
      {fit && <div className="cleanup-fit-alert" role="status">
        <strong>{geometryLocked ? "Sprite exceeds locked runtime geometry" : "Foreground does not fit the runtime cell"}</strong>
        <p>Recovered foreground: {fit.foregroundWidth}×{fit.foregroundHeight}. Runtime cell: {fit.cellWidth}×{fit.cellHeight}; pivot ({fit.pivotX},{fit.pivotY}). No foreground was cropped or scaled.</p>
        {target !== null && <p>Neutral target: {target}px (±{project.runtime.neutralTolerancePx}px).</p>}
        <div className="cleanup-fit-actions">
          {canExpand && fit.suggested && <button className="button primary" disabled={busy} onClick={() => void expandAndRetry()}>Use suggested {fit.suggested.cellWidth}×{fit.suggested.cellHeight} cell</button>}
          <button className={geometryLocked ? "button primary" : "button secondary"} disabled={busy || !settings?.snapperExecutable || !settings?.pythonExecutable || !cleanupValid || project.workflow.upscaleMode !== "automatic"} onClick={() => void runBatchAutoFit(cleanupOptions)}>{batchAutoFitBusy ? "Finding fitting grids…" : geometryLocked ? "Auto-fit to locked cell" : "Auto-fit to current cell"}</button>
        </div>
        {project.workflow.upscaleMode !== "automatic" && <small>Auto-fit is available for automatic snap/cleanup; manual upscale handoff cannot be resnapped automatically.</small>}
        {geometryLocked && fit.suggested && <details className="animation-hint"><summary>Diagnostic: suggested larger cell</summary><small>{fit.suggested.cellWidth}×{fit.suggested.cellHeight} at pivot ({fit.suggested.pivotX},{fit.suggested.pivotY}) would violate this locked runtime contract. No geometry change is offered here.</small></details>}
      </div>}
      {autoFit && <div className="cleanup-fit-results"><div className="snap-review-heading"><div><span className="eyebrow">AUTO-FIT REVIEW</span><h3>{autoFit.selectedReviewId ? "Review fitting snap batches" : "No fitting candidate in this run"}</h3></div><span className="snap-review-meta">{autoFit.candidates.length} bounded attempts · active snap untouched</span></div>{autoFit.selectedReviewId ? <><div className="auto-fit-options" role="group" aria-label="Fitting batch candidates">{autoFit.recommendedReviewIds.map(id => { const candidate = autoFit.candidates.find(item => item.reviewId === id); if (!candidate) return null; const maxHeight = Math.max(...candidate.frames.map(frame => frame.foregroundHeight)); return <button type="button" key={id} className={selectedAutoFitId === id ? "selected" : ""} onClick={() => selectAutoFit(id)} aria-pressed={selectedAutoFitId === id}><strong>Pixel size {candidate.pixelSize.toFixed(1)}</strong><span>{candidate.frames.length} frames fit ✓</span><small>{target === null ? "Fits pivots" : `Tallest ${maxHeight}px · target ${target}px`}</small></button>; })}</div>{autoFitReview && <div className="pose-raw-strip" aria-label="Auto-fit candidate frames">{autoFitReview.frames.map((frame, index) => <button type="button" className="pose-raw-item snap-frame-open" key={frame.sourceFrameId} disabled={!autoFitImages?.[frame.sourceFrameId]} onClick={() => setSelectedAutoFitFrameId(frame.sourceFrameId)}><span className="snap-frame-heading">{String(index + 1).padStart(2, "0")} <Maximize2 size={14} /></span>{autoFitImages?.[frame.sourceFrameId] && <img src={autoFitImages[frame.sourceFrameId].dataUrl} alt="" />}<small>{frame.width}×{frame.height}</small></button>)}</div>}<div className="animation-actions">{animation.activeBatchSnapId === selectedAutoFitId ? <span className="snap-applied"><Check size={15} /> Applied</span> : <button className="button primary" disabled={busy || !autoFitCandidate?.fits || !autoFitImages || Object.keys(autoFitImages).length !== animation.activeRawFrameIds.length} onClick={() => selectedAutoFitId && void applyBatchSnap(selectedAutoFitId)}><Check size={15} /> Apply selected snap batch</button>}<small>Each retry is an immutable review; no candidate is auto-applied.</small></div></> : <p className="animation-hint">No lossless candidate fits this cell and pivot. Try another source generation or manually select a coarser pixel size.</p>}</div>}
      {cleanup && <><p className="animation-hint">Review transparent cleaned crops and their fixed-cell normalized outputs before Apply. Cleaned PNGs remain separate from snapped or upscaled inputs.</p><div className="pose-raw-strip" aria-label="Cleaned and normalized frame previews">{cleanup.frames.map((frame, index) => <div className="pose-raw-item cleanup-comparison" key={frame.sourceFrameId}><span>{String(index + 1).padStart(2, "0")}</span><div><figure>{batchCleanedPreviews[frame.sourceFrameId] && <img src={batchCleanedPreviews[frame.sourceFrameId].dataUrl} alt={`Cleaned frame ${index + 1}`} />}<figcaption>Clean</figcaption></figure><figure>{batchNormalizedPreviews[frame.sourceFrameId] && <img src={batchNormalizedPreviews[frame.sourceFrameId].dataUrl} alt={`Normalized frame ${index + 1}`} />}<figcaption>Cell</figcaption></figure></div><small>{frame.foregroundPixels} pixels</small></div>)}</div></>}
    </div>
    <div className="batch-stage"><div className="animation-panel-heading"><h4>{project.workflow.upscaleMode === "manual-handoff" ? "4" : "3"} · Runtime normalize</h4><span className={`snap-state ${animation.stages.normalize}`}>{animation.stages.normalize}</span></div>
      <p className="animation-hint">Review the fixed {project.runtime.cellWidth}×{project.runtime.cellHeight} cells at pivot ({project.runtime.pivotX},{project.runtime.pivotY}). No scaling or cropping is applied. Cleaned crops remain separate from normalized cells.</p>
      <div className="animation-actions">{cleanup && animation.stages.cleanup === "complete" && !cleanupApplied && <button className="button primary" disabled={busy || animation.stages.normalize !== "review"} onClick={() => void applyBatchCleanup(cleanup.id)}><Check size={15} /> Apply normalized frames</button>}{cleanupApplied && <span className="snap-applied"><Check size={15} /> Normalized frames applied</span>}</div>
    </div>
    <ImageReviewDialog open={selectedSnapFrameId !== null && snappedReviewFrames.length > 0} frames={snappedReviewFrames} selectedId={selectedSnapFrameId} onSelect={setSelectedSnapFrameId} onClose={() => setSelectedSnapFrameId(null)} />
    <ImageReviewDialog open={selectedAutoFitFrameId !== null && autoFitReviewFrames.length > 0} frames={autoFitReviewFrames} selectedId={selectedAutoFitFrameId} onSelect={setSelectedAutoFitFrameId} onClose={() => setSelectedAutoFitFrameId(null)} />
  </section>;
}
