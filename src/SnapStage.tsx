import { useEffect, useState } from "react";
import { Check, FolderOpen, Maximize2, RotateCw, ScanLine, ShieldAlert } from "lucide-react";
import { latestSnapReview } from "./domain/snap";
import { useStudio } from "./store/studio";
import { WorkflowTip } from "./WorkflowTip";
import { ImageReviewDialog } from "./ImageReviewDialog";
import type { AssetPreview, Facing, Project, SnapOptions } from "./types";

export function ReviewImage({ image, label, zoom = 1, source = false }: { image: AssetPreview | null; label: string; zoom?: number; source?: boolean }) {
  return <div className="snap-image-pane">
    <div className="snap-pane-heading"><strong>{label}</strong><span>{image ? `${image.width} × ${image.height}` : "Waiting"}</span></div>
    <div className="snap-checker checkerboard" role="region" aria-label={`${label} image viewport`} tabIndex={0}>
      {image ? <img
        src={image.dataUrl}
        alt={label}
        draggable={false}
        style={{ width: image.width * zoom, height: image.height * zoom, imageRendering: source ? "auto" : "pixelated" }}
      /> : <span className="snap-pane-empty">Preview unavailable</span>}
    </div>
  </div>
}

export function SnapStage({ project, facing }: { project: Project; facing: Facing }) {
  const { settings, preview, snapNative, snapReference, busy, snapBusy, chooseSnapper, runSnap, applySnap } = useStudio();
  const [colors, setColors] = useState("16");
  const [pixelSize, setPixelSize] = useState("");
  const [palette, setPalette] = useState("");
  const [zoom, setZoom] = useState(2);
  const [previewOpen, setPreviewOpen] = useState(false);
  useEffect(() => setZoom(2), [project.id, facing]);
  const source = project.imports.find(record => record.id === project.anchors[facing]?.activeImportId);
  const review = latestSnapReview(project, facing);
  useEffect(() => setPreviewOpen(false), [review?.id]);
  const applied = !!review && project.anchors[facing]?.activeSnapId === review.id;
  const colorCount = Number(colors);
  const forcedSize = pixelSize.trim() ? Number(pixelSize) : null;
  const normalizedPalette = palette.trim() || null;
  const paletteValid = !normalizedPalette || (normalizedPalette.split(",").length <= colorCount && normalizedPalette.split(",").every(value => /^[0-9a-fA-F]{6}$/.test(value.trim())));
  const optionsValid = Number.isInteger(colorCount) && colorCount >= 2 && colorCount <= 256 &&
    (forcedSize === null || (Number.isInteger(forcedSize) && forcedSize > 0 && !!source && forcedSize <= Math.min(source.width, source.height))) && paletteValid;
  const options: SnapOptions = { colors: colorCount, pixelSize: forcedSize, palette: normalizedPalette };
  const stage = source?.stages.snap ?? "waiting";

  return <section className="snap-stage" aria-labelledby="snap-stage-title">
    <div className="snap-stage-heading"><div><span className="eyebrow">STAGE 02 · LOCAL PROCESSING</span><h2 id="snap-stage-title">Recover the pixel grid</h2><p>Run Sprite Fusion on this source, then inspect its native pixels before applying.</p></div><span className={`snap-state ${stage}`}>{snapBusy ? "Processing" : stage}</span></div>
    <div className="snap-tool-row"><div className="snap-tool-icon"><ScanLine size={19} /></div><div className="snap-tool-copy"><strong>Sprite Fusion Pixel Snapper</strong><span title={settings?.snapperExecutable ?? undefined}>{settings?.snapperVersion ?? "No local CLI selected"}{settings?.snapperExecutable ? ` · ${settings.snapperExecutable.split(/[\\/]/).slice(-2).join("/")}` : ""}</span></div><button className="button secondary" onClick={chooseSnapper} disabled={busy}><FolderOpen size={16} /> {settings?.snapperExecutable ? "Change CLI" : "Locate CLI"}</button></div>
    <WorkflowTip label="Snap settings · starting point">16 colors and Auto pixel size are a first pass, not a palette approval. Override pixel size only if the recovered grid is wrong; for KangiFight, use approved palette entries before production review. Keep the small native PNG and use only its nearest-neighbour upscale as a later generation reference.</WorkflowTip>
    <div className="snap-form"><div className="snap-field"><label htmlFor="snap-colors">Palette colors</label><input id="snap-colors" className="text-input" type="number" min="2" max="256" step="1" value={colors} onChange={event => setColors(event.target.value)} aria-invalid={!Number.isInteger(colorCount) || colorCount < 2 || colorCount > 256} /></div><div className="snap-field"><label htmlFor="snap-pixel-size">Pixel size override</label><input id="snap-pixel-size" className="text-input" type="number" min="1" max={source ? Math.min(source.width, source.height) : undefined} step="1" value={pixelSize} onChange={event => setPixelSize(event.target.value)} placeholder="Auto detect" /></div><div className="snap-field snap-palette-field"><label htmlFor="snap-palette">Approved palette <span>optional for studies</span></label><input id="snap-palette" className="text-input" value={palette} onChange={event => setPalette(event.target.value)} aria-invalid={!paletteValid} aria-describedby="snap-palette-help" placeholder="0d2b45,203c56,544e68" /><small id="snap-palette-help">Comma-separated six-digit RGB values, without #.</small></div></div>
    <div className="snap-run-row"><p>{project.preset === "kangi-fight" ? "This remains a DRAFT study; check identity, silhouette, and approved colors before applying." : "Reject noisy clusters or a broken silhouette; simplify the source if needed."}</p><button className="button secondary" onClick={() => void runSnap(options)} disabled={!source || !settings?.snapperExecutable || !optionsValid || busy}><RotateCw size={16} /> {snapBusy ? "Snapping…" : review ? "Run again" : "Run pixel snap"}</button></div>
    {review && <div className="snap-review"><div className="snap-review-heading"><div><span className="eyebrow">{applied ? "ACTIVE SNAPPED INPUT" : "REVIEW CANDIDATE"}</span><h3>{applied ? "Applied grid" : "Compare before applying"}</h3></div><span className="snap-review-meta">{review.nativeWidth} × {review.nativeHeight} native · {review.referenceScale}× reference</span></div><div className="snap-compare"><ReviewImage image={preview} label="Original source" source zoom={Math.min(1, preview ? 240 / Math.max(preview.width, preview.height) : 1)} /><div className="snap-native-column"><div className="snap-zoom" role="group" aria-label="Native preview zoom">{[1, 2, 4].map(value => <button key={value} type="button" className={zoom === value ? "active" : ""} onClick={() => setZoom(value)} aria-pressed={zoom === value}>{value}×</button>)}<button type="button" className="snap-open-preview" onClick={() => setPreviewOpen(true)} disabled={!snapNative} aria-haspopup="dialog"><Maximize2 size={13} /> Large view</button></div><ReviewImage image={snapNative} label="Recovered native grid" zoom={zoom} /></div></div><div className="snap-reference"><ReviewImage image={snapReference} label="Nearest-neighbour reference" zoom={Math.min(1, snapReference ? 150 / Math.max(snapReference.width, snapReference.height) : 1)} /><div><strong>Both outputs are preserved</strong><p>The native PNG proves the recovered grid. The reference is an integer-scale nearest-neighbour copy for later image-guided work.</p><span>Run {review.id.slice(0, 8)} · {review.toolVersion} · SHA-256 recorded</span></div></div><div className="snap-review-actions"><p><ShieldAlert size={16} /> Snap does not clean chroma, fix anatomy, or approve production art.</p>{applied ? <span className="snap-applied"><Check size={16} /> Applied for the next stage</span> : <button className="button primary" onClick={() => void applySnap(review.id)} disabled={busy || !snapNative || !snapReference}><Check size={16} /> Apply snapped anchor</button>}</div></div>}
    <ImageReviewDialog open={previewOpen && !!snapNative} frames={snapNative && review ? [{ id: review.id, label: `${facing} native snapped grid`, dataUrl: snapNative.dataUrl, width: snapNative.width, height: snapNative.height }] : []} selectedId={review?.id ?? null} onSelect={() => {}} onClose={() => setPreviewOpen(false)} />
  </section>;
}
