import { useEffect, useState } from "react";
import { ArrowDown, ArrowUp, Check, ImagePlus, Scissors, X } from "lucide-react";
import { useStudio } from "./store/studio";
import { WorkflowTip } from "./WorkflowTip";
import type { Animation, ExtractionOptions, ManualCrop } from "./types";

const initialOptions: ExtractionOptions = { background: "auto", tolerance: 36, minArea: 64, mergeGap: 4 };

export function PoseBoardPanel({ animation }: { animation: Animation }) {
  const { busy, poseBoardPreview, extractionCandidates, rawFramePreviews, importPoseBoard, runPoseExtraction, applyPoseExtraction, confirmNativeReview, replaceRawFrame } = useStudio();
  const [options, setOptions] = useState(initialOptions);
  const [ordered, setOrdered] = useState<string[]>([]);
  const [manual, setManual] = useState<ManualCrop[]>([]);
  const [accepted, setAccepted] = useState<string[]>([]);
  const [reviewCropFrameId, setReviewCropFrameId] = useState<string | null>(null);
  const [reviewCrop, setReviewCrop] = useState<ManualCrop>({ x: 0, y: 0, width: 128, height: 128 });
  const [crop, setCrop] = useState<ManualCrop>({ x: 0, y: 0, width: 128, height: 128 });
  const board = animation.boards.find(item => item.id === animation.activeBoardId);
  const review = animation.extractions.slice().reverse().find(item => item.boardImportId === animation.activeBoardId);
  const applied = !!review && animation.activeExtractionId === review.id;
  const rawFrames = animation.activeRawFrameIds.map(id => animation.rawFrames.find(item => item.id === id)).filter(item => item !== undefined);
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
  const reviewCropValid = !!board && Number.isInteger(reviewCrop.x) && Number.isInteger(reviewCrop.y) && Number.isInteger(reviewCrop.width) && Number.isInteger(reviewCrop.height) && reviewCrop.x >= 0 && reviewCrop.y >= 0 && reviewCrop.width > 0 && reviewCrop.height > 0 && reviewCrop.x + reviewCrop.width <= board.width && reviewCrop.y + reviewCrop.height <= board.height;
  const optionsValid = (options.background === "auto" || /^#[0-9a-fA-F]{6}$/.test(options.background)) && Number.isInteger(options.tolerance) && options.tolerance >= 0 && options.tolerance <= 80 && Number.isInteger(options.minArea) && options.minArea >= 1 && options.minArea <= 100_000 && Number.isInteger(options.mergeGap) && options.mergeGap >= 0 && options.mergeGap <= 32;

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
        <p className="animation-hint">Inspect complete bodies at recovered pixel size against the shared baseline. Accept, exclude, re-crop from the board, or replace an individual PNG/JPEG. Earlier files stay preserved.</p>
        <div className="pose-raw-strip native-review-strip">{rawFrames.map((frame, index) => <div key={frame.id} className="pose-raw-item native-review-item">
          <span>{String(index + 1).padStart(2, "0")}</span>
          <div className="native-review-image" style={{ height: Math.max(...rawFrames.map(item => item.height)) }}>{rawFramePreviews[frame.id] && <img src={rawFramePreviews[frame.id].dataUrl} width={frame.width} height={frame.height} alt={`Raw frame ${index + 1}`} />}</div>
          <small>{frame.width}×{frame.height}</small>
          <label><input type="checkbox" checked={accepted.includes(frame.id)} onChange={event => setAccepted(current => event.target.checked ? [...current, frame.id] : current.filter(id => id !== frame.id))} /> {accepted.includes(frame.id) ? "Accept" : "Exclude"}</label>
          <div className="animation-actions"><button className="button secondary" disabled={busy} onClick={() => { setReviewCrop({ x: 0, y: 0, width: Math.min(board?.width ?? 128, frame.width), height: Math.min(board?.height ?? 128, frame.height) }); setReviewCropFrameId(frame.id); }}>Re-crop</button><button className="button secondary" disabled={busy} onClick={() => void replaceRawFrame(frame.id)}>Replace</button></div>
        </div>)}</div>
        {reviewCropFrameId && <div className="pose-manual"><span className="eyebrow">RE-CROP FRAME FROM BOARD PIXELS</span><div className="pose-crop-inputs">{(["x", "y", "width", "height"] as const).map(key => <label key={key}>{key}<input className="text-input" type="number" min={key === "width" || key === "height" ? 1 : 0} value={reviewCrop[key]} onChange={event => setReviewCrop({ ...reviewCrop, [key]: Number(event.target.value) })} /></label>)}<button className="button secondary" disabled={busy || !reviewCropValid} onClick={() => { void replaceRawFrame(reviewCropFrameId, reviewCrop); setReviewCropFrameId(null); }}>Apply re-crop</button><button className="button ghost" onClick={() => setReviewCropFrameId(null)}>Cancel</button></div></div>}
        <div className="animation-actions"><button className="button primary" disabled={busy || accepted.length === 0 || animation.stages.native_review === "complete"} onClick={() => void confirmNativeReview(accepted)}><Check size={15} /> Confirm native review</button>{animation.stages.native_review === "complete" && <span className="snap-applied"><Check size={15} /> Confirmed</span>}</div>
      </div>}
    </>}
  </section>;
}
