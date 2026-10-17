import { useEffect, useState } from "react";
import { ArrowDown, ArrowUp, Check, ImagePlus, Scissors, X } from "lucide-react";
import { useStudio } from "./store/studio";
import { WorkflowTip } from "./WorkflowTip";
import type { Animation, ExtractionOptions, ManualCrop } from "./types";

const initialOptions: ExtractionOptions = { background: "auto", tolerance: 36, minArea: 64, mergeGap: 4 };
const initialRecoveryOptions: ExtractionOptions = { background: "auto", tolerance: 24, minArea: 64, mergeGap: 4 };

export function PoseBoardPanel({ animation }: { animation: Animation }) {
  const { busy, poseBoardPreview, extractionCandidates, rawFramePreviews, frameRecoveryPreviews, frameSourcePreviews, frameRecoverySourcePreviews, importPoseBoard, runPoseExtraction, applyPoseExtraction, confirmNativeReview, replaceRawFrame, applyFrameRecovery } = useStudio();
  const [options, setOptions] = useState(initialOptions);
  const [recoveryOptions, setRecoveryOptions] = useState(initialRecoveryOptions);
  const [ordered, setOrdered] = useState<string[]>([]);
  const [manual, setManual] = useState<ManualCrop[]>([]);
  const [accepted, setAccepted] = useState<string[]>([]);
  const [reviewCropFrameId, setReviewCropFrameId] = useState<string | null>(null);
  const [reviewCropSourceReviewId, setReviewCropSourceReviewId] = useState<string | null>(null);
  const [reviewCrop, setReviewCrop] = useState<ManualCrop>({ x: 0, y: 0, width: 1, height: 1 });
  const [crop, setCrop] = useState<ManualCrop>({ x: 0, y: 0, width: 128, height: 128 });
  const board = animation.boards.find(item => item.id === animation.activeBoardId);
  const review = animation.extractions.slice().reverse().find(item => item.boardImportId === animation.activeBoardId);
  const applied = !!review && animation.activeExtractionId === review.id;
  const rawFrames = animation.activeRawFrameIds.map(id => animation.rawFrames.find(item => item.id === id)).filter(item => item !== undefined);
  const reviewFrame = rawFrames.find(item => item.id === reviewCropFrameId);
  const reviewSource = animation.frameRecoveries.find(item => item.id === reviewCropSourceReviewId && item.targetFrameId === reviewCropFrameId);
  const pendingRecoveries = [...new Map(animation.frameRecoveries.filter(item => !item.appliedAt && rawFrames.some(frame => frame.id === item.targetFrameId)).map(item => [item.targetFrameId, item])).values()];
  useEffect(() => setAccepted(animation.activeRawFrameIds), [animation.activeRawFrameIds.join()]);

  useEffect(() => {
    setOrdered(review?.boxes.map(item => item.id) ?? []);
    setManual([]);
  }, [review?.id]);
  useEffect(() => {
    setCrop({ x: 0, y: 0, width: Math.min(128, board?.width ?? 128), height: Math.min(128, board?.height ?? 128) });
  }, [board?.id, board?.width, board?.height]);

  const move = (id: string, direction: number) => setOrdered(current => {
    const index = current.indexOf(id);
    const target = index + direction;
    if (index < 0 || target < 0 || target >= current.length) return current;
    const next = [...current];
    [next[index], next[target]] = [next[target], next[index]];
    return next;
  });
  const addCrop = () => {
    if (!board || !Number.isInteger(crop.x) || !Number.isInteger(crop.y) || !Number.isInteger(crop.width) || !Number.isInteger(crop.height) || crop.x < 0 || crop.y < 0 || crop.width < 1 || crop.height < 1 || crop.x + crop.width > board.width || crop.y + crop.height > board.height) return;
    setManual(current => [...current, crop]);
  };
  const cropValid = !!board && Number.isInteger(crop.x) && Number.isInteger(crop.y) && Number.isInteger(crop.width) && Number.isInteger(crop.height) && crop.x >= 0 && crop.y >= 0 && crop.width > 0 && crop.height > 0 && crop.x + crop.width <= board.width && crop.y + crop.height <= board.height;
  const reviewSourceWidth = reviewSource?.sourceWidth ?? reviewFrame?.sourceWidth ?? board?.width ?? 0;
  const reviewSourceHeight = reviewSource?.sourceHeight ?? reviewFrame?.sourceHeight ?? board?.height ?? 0;
  const reviewCropValid = !!reviewFrame && Number.isInteger(reviewCrop.x) && Number.isInteger(reviewCrop.y) && Number.isInteger(reviewCrop.width) && Number.isInteger(reviewCrop.height) && reviewCrop.x >= 0 && reviewCrop.y >= 0 && reviewCrop.width > 0 && reviewCrop.height > 0 && reviewCrop.x + reviewCrop.width <= reviewSourceWidth && reviewCrop.y + reviewCrop.height <= reviewSourceHeight;
  const optionsValid = (options.background === "auto" || /^#[0-9a-fA-F]{6}$/.test(options.background)) && Number.isInteger(options.tolerance) && options.tolerance >= 0 && options.tolerance <= 80 && Number.isInteger(options.minArea) && options.minArea >= 1 && options.minArea <= 100_000 && Number.isInteger(options.mergeGap) && options.mergeGap >= 0 && options.mergeGap <= 32;
  const recoveryOptionsValid = (recoveryOptions.background === "auto" || /^#[0-9a-fA-F]{6}$/.test(recoveryOptions.background)) && Number.isInteger(recoveryOptions.tolerance) && recoveryOptions.tolerance >= 0 && recoveryOptions.tolerance <= 80 && Number.isInteger(recoveryOptions.minArea) && recoveryOptions.minArea >= 1 && recoveryOptions.minArea <= 100_000 && Number.isInteger(recoveryOptions.mergeGap) && recoveryOptions.mergeGap >= 0 && recoveryOptions.mergeGap <= 32;

  return <section id="pose-board-panel" className="pose-panel" aria-labelledby="pose-title">
    <div className="animation-panel-heading"><div><span className="eyebrow">STEP 2 · POSE BOARD</span><h4 id="pose-title">{board ? "Review imported poses" : "Import a pose board"}</h4></div><span className={`snap-state ${animation.stages.extract}`}>{animation.stages.extract}</span></div>
    <p className="animation-hint">A pose board is one PNG or JPEG containing several poses for this animation. It is not a single-image reference and does not require an anchor.</p>
    <WorkflowTip label="Recovery · starting values">Auto chroma, tolerance 36, minimum area 64, merge gap 4. The app finds foreground components; it does not slice a rigid 4 × 3 grid. Review each recovered pose for complete limbs, stable identity, and deliberate frame order before Apply. Counts such as idle 10, attack 8, hurt/jump 6, death 10 are source-study examples—not required gameplay frame counts.</WorkflowTip>
    {!board && <p className="pose-start-note">Start here: choose the board image for <strong>{animation.name}</strong>. Next you’ll detect its separate poses and choose their order.</p>}
    <div className="animation-actions"><button id="pose-board-import" className={board ? "button secondary" : "button primary"} disabled={busy} onClick={() => void importPoseBoard()}><ImagePlus size={15} /> {board ? "Import another board" : "Import pose board"}</button>{board && <span className="pose-board-name">{board.originalName} · {board.width}×{board.height}</span>}</div>
    {board && <>
      <div className="pose-board-layout">
        <div className="pose-board-preview checkerboard" aria-label="Pose board preview">
          {poseBoardPreview && <img src={poseBoardPreview.dataUrl} alt={`Pose board ${board.originalName}`} />}
        </div>
        <div className="pose-settings">
          <label>Background <input className="text-input" value={options.background} onChange={event => setOptions({ ...options, background: event.target.value })} placeholder="auto or #00ff00" /></label>
          <label>Tolerance <input className="text-input" type="number" min="0" max="80" value={options.tolerance} onChange={event => setOptions({ ...options, tolerance: Number(event.target.value) })} /></label>
          <label>Minimum area <input className="text-input" type="number" min="1" max="100000" value={options.minArea} onChange={event => setOptions({ ...options, minArea: Number(event.target.value) })} /></label>
          <label>Merge gap <input className="text-input" type="number" min="0" max="32" value={options.mergeGap} onChange={event => setOptions({ ...options, mergeGap: Number(event.target.value) })} /></label>
          <button className="button secondary" disabled={busy || !optionsValid} onClick={() => void runPoseExtraction(options)}><Scissors size={15} /> Detect frames</button>
          <small>Auto samples corner pixels. Increase merge gap when a limb separates; reduce it if neighbouring poses join. Valid ranges: tolerance 0–80, minimum area 1–100,000, merge gap 0–32.</small>
        </div>
      </div>
      {review && <div className="pose-review">
        <div className="animation-subheading"><div><span className="eyebrow">EXTRACTION REVIEW</span><h3>{review.boxes.length} candidates</h3></div><span className="animation-hint">Uncheck rejects. Arrows change playback order.</span></div>
        <div className="pose-candidates">{review.boxes.map(box => {
          const index = ordered.indexOf(box.id);
          return <div className={`pose-candidate ${index < 0 ? "excluded" : ""}`} key={box.id}>
            <div className="pose-candidate-image checkerboard">{extractionCandidates[box.id] && <img src={extractionCandidates[box.id].dataUrl} alt={`Detected pose ${box.id}`} />}</div>
            <div className="pose-candidate-meta"><label><input type="checkbox" checked={index >= 0} onChange={event => setOrdered(current => event.target.checked ? [...current, box.id] : current.filter(id => id !== box.id))} /> {index >= 0 ? `#${index + 1}` : "Exclude"}</label><small>{box.x},{box.y} · {box.width}×{box.height}</small></div>
            <div className="pose-order"><button aria-label={`Move pose ${box.id} earlier`} disabled={index <= 0} onClick={() => move(box.id, -1)}><ArrowUp size={15} /></button><button aria-label={`Move pose ${box.id} later`} disabled={index < 0 || index >= ordered.length - 1} onClick={() => move(box.id, 1)}><ArrowDown size={15} /></button></div>
          </div>;
        })}</div>
        <div className="pose-manual"><span className="eyebrow">MANUAL CROP · SOURCE PIXELS</span><div className="pose-crop-inputs">{(["x", "y", "width", "height"] as const).map(key => <label key={key}>{key}<input className="text-input" type="number" min={key === "width" || key === "height" ? 1 : 0} value={crop[key]} onChange={event => setCrop({ ...crop, [key]: Number(event.target.value) })} /></label>)}<button className="button secondary" disabled={!cropValid} onClick={addCrop}><Scissors size={14} /> Add crop</button></div>{manual.length > 0 && <div className="pose-manual-list">{manual.map((item, index) => <span key={index}>#{index + 1}: {item.x},{item.y} · {item.width}×{item.height}<button aria-label={`Remove manual crop ${index + 1}`} onClick={() => setManual(current => current.filter((_, at) => at !== index))}><X size={13} /></button></span>)}</div>}</div>
        <div className="animation-actions"><button className="button primary" disabled={busy || ordered.length + manual.length === 0} onClick={() => void applyPoseExtraction(review.id, ordered, manual)}><Check size={15} /> {applied ? "Re-apply recovered frames" : `Apply ${ordered.length + manual.length} raw frames`}</button>{applied && <span className="snap-applied"><Check size={15} /> Recovery applied</span>}</div>
      </div>}
      {rawFrames.length > 0 && <div className="pose-raw">
        <span className="eyebrow">NATIVE REVIEW · {rawFrames.length} RECOVERED FRAMES</span>
        <p className="animation-hint">Inspect complete bodies at natural source size. Replace automatically detects a pose from a standalone PNG/JPEG; review and Apply its crop before snapping. Re-crop always uses the frame’s current source, never the 128×128 runtime cell.</p>
        <div className="pose-raw-strip native-review-strip">{rawFrames.map((frame, index) => <div key={frame.id} className="pose-raw-item native-review-item">
          <span>{String(index + 1).padStart(2, "0")}</span>
          <div className="native-review-image" style={{ height: Math.max(...rawFrames.map(item => item.height)) }}>{rawFramePreviews[frame.id] && <img src={rawFramePreviews[frame.id].dataUrl} width={frame.width} height={frame.height} alt={`Raw frame ${index + 1}`} />}</div>
          <small>{frame.width}×{frame.height}</small>
          <label><input type="checkbox" checked={accepted.includes(frame.id)} onChange={event => setAccepted(current => event.target.checked ? [...current, frame.id] : current.filter(id => id !== frame.id))} /> {accepted.includes(frame.id) ? "Accept" : "Exclude"}</label>
          <div className="animation-actions"><button className="button secondary" disabled={busy} onClick={() => { setReviewCropSourceReviewId(null); setReviewCrop(frame.sourceCrop ?? { x: 0, y: 0, width: frame.sourceWidth ?? board?.width ?? frame.width, height: frame.sourceHeight ?? board?.height ?? frame.height }); setReviewCropFrameId(frame.id); }}>Re-crop</button><button className="button secondary" disabled={busy || !recoveryOptionsValid} onClick={() => void replaceRawFrame(frame.id, "pick", recoveryOptions)}>Replace</button></div>
        </div>)}</div>
        {reviewCropFrameId && reviewFrame && <div className="pose-board-preview checkerboard" aria-label="Active source image for re-crop"><img src={reviewSource ? frameRecoverySourcePreviews[reviewSource.id]?.dataUrl : reviewFrame.sourceKind === "standaloneReplacement" ? frameSourcePreviews[reviewFrame.id]?.dataUrl : poseBoardPreview?.dataUrl} alt="Active full-size source for frame re-crop" /></div>}
        {reviewCropFrameId && reviewFrame && <div className="pose-manual"><span className="eyebrow">RE-CROP FRAME FROM {(reviewSource?.sourceKind ?? reviewFrame.sourceKind) === "standaloneReplacement" ? "REPLACEMENT IMAGE" : "POSE BOARD"}</span><p className="animation-hint">Source: {(reviewSource?.sourceRelativePath ?? reviewFrame.sourceRelativePath)?.split("/").slice(-1)[0] ?? board?.originalName} · {reviewSourceWidth} × {reviewSourceHeight} source pixels. Raw crop may be any size; the runtime cell is applied only during normalization.</p><div className="pose-crop-inputs">{(["background", "tolerance", "minArea", "mergeGap"] as const).map(key => <label key={key}>{key === "minArea" ? "Minimum area" : key === "mergeGap" ? "Merge gap" : key}<input className="text-input" type={key === "background" ? "text" : "number"} value={recoveryOptions[key]} onChange={event => setRecoveryOptions({ ...recoveryOptions, [key]: key === "background" ? event.target.value : Number(event.target.value) })} /></label>)}</div><div className="animation-actions"><button className="button secondary" disabled={busy || !recoveryOptionsValid} onClick={() => void replaceRawFrame(reviewCropFrameId, "auto", recoveryOptions, reviewCropSourceReviewId ?? undefined)}>Detect foreground / Auto recover pose</button></div><span className="eyebrow">MANUAL CROP · ACTIVE SOURCE PIXELS</span><div className="pose-crop-inputs">{(["x", "y", "width", "height"] as const).map(key => <label key={key}>{key}<input className="text-input" type="number" min={key === "width" || key === "height" ? 1 : 0} value={reviewCrop[key]} onChange={event => setReviewCrop({ ...reviewCrop, [key]: Number(event.target.value) })} /></label>)}<button className="button secondary" disabled={busy || !reviewCropValid || !recoveryOptionsValid} onClick={() => { void replaceRawFrame(reviewCropFrameId, reviewCrop, recoveryOptions, reviewCropSourceReviewId ?? undefined); setReviewCropFrameId(null); setReviewCropSourceReviewId(null); }}>Review manual crop</button><button className="button ghost" onClick={() => { setReviewCropFrameId(null); setReviewCropSourceReviewId(null); }}>Cancel</button></div></div>}
        {pendingRecoveries.map(recovery => <div key={recovery.id} className="pose-review frame-recovery-review"><span className="eyebrow">FRAME {String(rawFrames.findIndex(frame => frame.id === recovery.targetFrameId) + 1).padStart(2, "0")} — {recovery.sourceKind === "standaloneReplacement" ? "STANDALONE REPLACEMENT" : "BOARD RE-CROP"}</span><h3>Review recovered pose</h3><div className="pose-candidate-image checkerboard">{frameRecoveryPreviews[recovery.id] && <img src={frameRecoveryPreviews[recovery.id].dataUrl} alt="Recovered crop awaiting Apply" />}</div><p className="animation-hint">Source {recovery.sourceWidth} × {recovery.sourceHeight} · Foreground {recovery.foregroundWidth} × {recovery.foregroundHeight} · Raw crop {recovery.crop.width} × {recovery.crop.height} at ({recovery.crop.x}, {recovery.crop.y}) · Background #{recovery.backgroundHex.toUpperCase()}</p><p className="animation-hint">✓ Pose recovered for review. The active frame and previous snap remain unchanged until Apply.</p><div className="animation-actions"><button className="button primary" disabled={busy} onClick={() => void applyFrameRecovery(recovery.id)}><Check size={15} /> Apply recovered crop</button><button className="button secondary" disabled={busy || !recoveryOptionsValid} onClick={() => void replaceRawFrame(recovery.targetFrameId, "auto", recoveryOptions, recovery.id)}>Auto recover again</button><button className="button secondary" disabled={busy} onClick={() => { setReviewCropSourceReviewId(recovery.id); setReviewCropFrameId(recovery.targetFrameId); setReviewCrop(recovery.crop); }}>Manual re-crop</button></div></div>)}
        <div className="animation-actions"><button className="button primary" disabled={busy || accepted.length === 0 || animation.stages.native_review === "complete"} onClick={() => void confirmNativeReview(accepted)}><Check size={15} /> Confirm native review</button>{animation.stages.native_review === "complete" && <span className="snap-applied"><Check size={15} /> Confirmed</span>}</div>
      </div>}
    </>}
  </section>;
}
