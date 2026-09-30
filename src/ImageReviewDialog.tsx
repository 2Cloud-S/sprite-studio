import { useEffect, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, X } from "lucide-react";

export interface ReviewFrame {
  id: string;
  label: string;
  dataUrl: string;
  width: number;
  height: number;
}

interface ImageReviewDialogProps {
  open: boolean;
  frames: ReviewFrame[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onClose: () => void;
}

export function ImageReviewDialog({ open, frames, selectedId, onSelect, onClose }: ImageReviewDialogProps) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [zoom, setZoom] = useState(2);
  const selectedIndex = Math.max(0, frames.findIndex(frame => frame.id === selectedId));
  const frame = frames[selectedIndex];

  useEffect(() => {
    const element = dialog.current;
    if (open && element && !element.open) element.showModal();
    if (!open && element?.open) element.close();
  }, [open]);
  useEffect(() => { if (open) setZoom(2); }, [open]);

  const selectAdjacent = (step: number) => {
    if (frames.length < 2) return;
    onSelect(frames[(selectedIndex + step + frames.length) % frames.length].id);
  };

  return <dialog ref={dialog} className="image-review-dialog" aria-labelledby="image-review-title" onClose={onClose}
    onKeyDown={event => {
      if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
        event.preventDefault();
        selectAdjacent(event.key === "ArrowLeft" ? -1 : 1);
      }
    }}>
    <div className="image-review-header">
      <div><span className="eyebrow">PIXEL-SNAP REVIEW</span><h2 id="image-review-title">{frame?.label ?? "Snapped image"}</h2><p>{frame ? `${frame.width} × ${frame.height} native pixels · ${selectedIndex + 1} of ${frames.length}` : "Preview unavailable"}</p></div>
      <button type="button" className="icon-button" onClick={onClose} aria-label="Close image preview"><X size={18} /></button>
    </div>
    <div className="image-review-controls">
      <div className="image-review-navigation"><button type="button" className="button secondary" onClick={() => selectAdjacent(-1)} disabled={frames.length < 2} aria-label="Previous snapped frame"><ChevronLeft size={16} /> Previous</button><button type="button" className="button secondary" onClick={() => selectAdjacent(1)} disabled={frames.length < 2} aria-label="Next snapped frame">Next <ChevronRight size={16} /></button></div>
      <div className="image-review-zoom" role="group" aria-label="Preview zoom">{[1, 2, 4, 8].map(value => <button type="button" key={value} className={zoom === value ? "active" : ""} aria-pressed={zoom === value} onClick={() => setZoom(value)}>{value}×</button>)}</div>
    </div>
    <div className="image-review-canvas checkerboard" role="region" aria-label="Snapped image at selected zoom" tabIndex={0}>
      {frame && <img src={frame.dataUrl} alt={`${frame.label} at ${zoom}× zoom`} draggable={false} style={{ width: frame.width * zoom, height: frame.height * zoom }} />}
    </div>
    <p className="image-review-footnote">1× shows native pixels. Larger views use nearest-neighbour display only; the saved image is unchanged. Use ←/→ for frames and Esc to close. Inspect the silhouette and pixel clusters before Apply.</p>
  </dialog>;
}
