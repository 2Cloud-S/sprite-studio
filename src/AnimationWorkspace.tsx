import { useEffect, useRef, useState } from "react";
import { Check, ChevronLeft, ChevronRight, ImagePlus, Layers2, Pause, Pencil, Play, Plus, RotateCcw } from "lucide-react";
import { useStudio } from "./store/studio";
import { ExportPanel } from "./ExportPanel";
import { PoseBoardPanel } from "./PoseBoardPanel";
import { BatchProcessPanel } from "./BatchProcessPanel";
import { WorkflowSettingsPanel } from "./WorkflowSettingsPanel";
import { WorkflowTip } from "./WorkflowTip";
import type { Animation, AssetPreview, Facing, FrameOffset, Project } from "./types";

function CellView({ project, animation, images, index, offsets, zoom, onion, grid, guides, onPointerNudge, nearest = true }: {
  project: Project; animation: Animation; images: Record<string, AssetPreview>; index: number;
  offsets: FrameOffset[]; zoom: number; onion: boolean; grid: boolean; guides: boolean;
  onPointerNudge?: (x: number, y: number) => void; nearest?: boolean;
}) {
  const drag = useRef<{ x: number; y: number; ox: number; oy: number } | null>(null);
  const frame = animation.frames[index];
  const priorIndex = (index - 1 + animation.frames.length) % animation.frames.length;
  const prior = animation.frames[priorIndex];
  const priorOffset = offsets[priorIndex] ?? { x: 0, y: 0 };
  const offset = offsets[index] ?? { x: 0, y: 0 };
  const width = project.runtime.cellWidth * zoom;
  const height = project.runtime.cellHeight * zoom;
  const pixel = (item: typeof frame, x: number, y: number, className: string) => item && images[item.id] ?
    <img className={className} src={images[item.id].dataUrl} alt="" draggable={false}
      style={{ width, height, left: x * zoom, top: y * zoom, imageRendering: nearest ? "pixelated" : "auto" }} /> : null;
  return <div className="animation-cell checkerboard" style={{ width, height, backgroundSize: `${16 * zoom}px ${16 * zoom}px` }}
    onPointerDown={onPointerNudge ? event => {
      drag.current = { x: event.clientX, y: event.clientY, ox: offset.x, oy: offset.y };
      event.currentTarget.setPointerCapture(event.pointerId);
    } : undefined}
    onPointerMove={onPointerNudge ? event => {
      if (!drag.current) return;
      onPointerNudge(drag.current.ox + Math.round((event.clientX - drag.current.x) / zoom), drag.current.oy + Math.round((event.clientY - drag.current.y) / zoom));
    } : undefined}
    onPointerUp={() => { drag.current = null; }} onPointerCancel={() => { drag.current = null; }}>
    {onion && animation.frames.length > 1 && pixel(prior, priorOffset.x, priorOffset.y, "animation-pixel onion")}
    {pixel(frame, offset.x, offset.y, "animation-pixel")}
    {grid && <div className="animation-pixel-grid" style={{ backgroundSize: `${zoom}px ${zoom}px` }} />}
    {guides && <><div className="animation-guide vertical" style={{ left: project.runtime.pivotX * zoom }} /><div className="animation-guide horizontal" style={{ top: project.runtime.pivotY * zoom }} /><div className="animation-pivot" style={{ left: project.runtime.pivotX * zoom, top: project.runtime.pivotY * zoom }} /></>}
  </div>;
}

export function AnimationWorkspace({ project, facing }: { project: Project; facing: Facing }) {
  const { selectedAnimationId, framePreviews, busy, createAnimation, renameAnimation, selectAnimation, importAnimationFrames,
    proposeAlignment, applyAlignment, proposeAnimationPreview, applyAnimationPreview } = useStudio();
  const [newName, setNewName] = useState("");
  const [renaming, setRenaming] = useState(false);
  const [renameName, setRenameName] = useState("");
  const [newFacing, setNewFacing] = useState<Facing>(facing);
  const animation = project.animations.find(item => item.id === selectedAnimationId) ?? null;
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [playbackIndex, setPlaybackIndex] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [fps, setFps] = useState(8);
  const [looping, setLooping] = useState(true);
  const [onion, setOnion] = useState(true);
  const [grid, setGrid] = useState(false);
  const [guides, setGuides] = useState(true);
  const [nearest, setNearest] = useState(true);
  const [offsets, setOffsets] = useState<FrameOffset[]>([]);
  const [dirty, setDirty] = useState(false);
  const alignReview = animation?.alignments[animation.alignments.length - 1] ?? null;
  const previewReview = animation?.previews.slice().reverse().find(item => item.alignmentId === animation.activeAlignmentId) ?? null;
  const alignApplied = !!alignReview && animation?.activeAlignmentId === alignReview.id;
  const previewSettingsMatch = !!previewReview && previewReview.fps === fps && previewReview.looping === looping;
  const previewApplied = !!previewReview && animation?.activePreviewId === previewReview.id && previewSettingsMatch;
  const zoom = Math.min(2, 256 / Math.max(project.runtime.cellWidth, project.runtime.cellHeight));

  useEffect(() => {
    const latest = animation?.alignments[animation.alignments.length - 1];
    setOffsets(animation?.frames.map(frame => latest?.offsets.find(item => item.frameId === frame.id) ?? { frameId: frame.id, x: 0, y: 0 }) ?? []);
    setDirty(false);
  }, [animation?.id, animation?.frames.length, animation?.alignments.length]);
  useEffect(() => {
    setSelectedIndex(0); setPlaybackIndex(0); setPlaying(false);
    const preview = animation?.previews[animation.previews.length - 1];
    setFps(preview?.fps ?? 8); setLooping(preview?.looping ?? true);
  }, [animation?.id]);
  useEffect(() => setNewFacing(facing), [facing]);
  useEffect(() => { setRenaming(false); setRenameName(animation?.name ?? ""); }, [animation?.id]);
  useEffect(() => {
    if (!playing || !animation?.frames.length || !Number.isInteger(fps) || fps < 1 || fps > 60) return;
    const timer = window.setInterval(() => setPlaybackIndex(index => {
      if (index + 1 < animation.frames.length) return index + 1;
      if (looping) return 0;
      setPlaying(false);
      return index;
    }), 1000 / fps);
    return () => window.clearInterval(timer);
  }, [playing, fps, looping, animation?.id, animation?.frames.length]);

  const nudge = (x: number, y: number) => {
    if (!animation?.frames[selectedIndex] || !project.workflow.manualPolishEnabled) return;
    setOffsets(current => current.map((item, index) => index === selectedIndex ? { ...item, x, y } : item));
    setDirty(true);
  };
  const current = offsets[selectedIndex] ?? { x: 0, y: 0 };
  const keyNudge = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (!project.workflow.manualPolishEnabled) return;
    const delta = event.shiftKey ? 5 : 1;
    const steps: Record<string, [number, number]> = { ArrowLeft: [-delta, 0], ArrowRight: [delta, 0], ArrowUp: [0, -delta], ArrowDown: [0, delta] };
    const step = steps[event.key];
    if (!step) return;
    event.preventDefault();
    nudge(current.x + step[0], current.y + step[1]);
  };
  const fpsValid = Number.isInteger(fps) && fps >= 1 && fps <= 60;
  const frameCount = animation?.frames.length ?? 0;
  return <section id="animation-workspace" className="animation-workspace" aria-labelledby="animation-title">
    <div className="animation-heading"><div><span className="eyebrow">POSE-BOARD WORKFLOW</span><h2 id="animation-title">Build an animation from poses</h2><p>Start with a multi-pose board or already-normalized frames. A single-image reference is optional, not a prerequisite.</p></div><span className="animation-count">{project.animations.length} animations</span></div>
    <ol className="animation-steps" aria-label="Pose-board workflow steps"><li><strong>1</strong> Create or select an animation</li><li><strong>2</strong> Import a pose board</li><li><strong>3</strong> Detect, review, and apply frames</li><li><strong>4</strong> Snap, clean, align, export</li></ol>
    <WorkflowTip label="Board source · suggested">For AI-generated boards, start around 2048 × 1536: an implied 4 × 3 layout gives each pose about 512 × 512 source pixels. A 1536 × 1152 board gives 384 × 384 areas. Use flat, non-conflicting chroma; an alternating-pixel guide is optional, but its grid lines must not appear in the art. These are generation guides, not crop boxes or runtime cells. Keep each action and facing on its own board; extraction supports up to 4096 pixels per axis. For walks, curate one complete cycle outside the app—video import is not yet supported.</WorkflowTip>
    <div className="animation-create"><label htmlFor="new-animation-name">Animation name<input id="new-animation-name" className="text-input" value={newName} onChange={event => setNewName(event.target.value)} placeholder="e.g. idle" maxLength={60} /></label><label htmlFor="new-animation-facing">Facing<select id="new-animation-facing" className="text-input" value={newFacing} onChange={event => setNewFacing(event.target.value as Facing)}>{project.runtime.sourceFacings.map(item => <option key={item} value={item}>{item}</option>)}</select></label><button className="button secondary" disabled={busy || !newName.trim()} onClick={() => { void createAnimation(newName, newFacing); setNewName(""); }}><Plus size={16} /> Create animation</button></div>
    {project.animations.length > 0 && <div className="animation-tabs" role="tablist" aria-label="Animations">{project.animations.map(item => <button key={item.id} role="tab" aria-selected={item.id === selectedAnimationId} className={item.id === selectedAnimationId ? "selected" : ""} onClick={() => void selectAnimation(item.id)}>{item.name} <small>{item.facing} · {item.frames.length}</small></button>)}</div>}
    {animation && <div className="animation-rename-row">{renaming ? <form onSubmit={async event => {
      event.preventDefault();
      if (!renameName.trim()) return;
      if (renameName.trim() === animation.name || await renameAnimation(animation.id, renameName)) setRenaming(false);
    }}><label htmlFor="rename-animation-name">Rename {animation.facing} animation<input id="rename-animation-name" className="text-input" autoFocus maxLength={60} value={renameName} onChange={event => setRenameName(event.target.value)} aria-invalid={!renameName.trim()} /></label><button className="button primary" type="submit" disabled={busy || !renameName.trim()}><Check size={15} /> Save name</button><button className="button secondary" type="button" disabled={busy} onClick={() => setRenaming(false)}>Cancel</button></form> : <><span>Selected: <strong>{animation.name}</strong> · {animation.facing}</span><button className="button secondary" disabled={busy} onClick={() => { setRenameName(animation.name); setRenaming(true); }}><Pencil size={15} /> Rename animation</button></>}</div>}
    {animation ? <div className="animation-editor"><WorkflowSettingsPanel project={project} /><PoseBoardPanel animation={animation} /><BatchProcessPanel animation={animation} project={project} /><div className="animation-subheading"><div><span className="eyebrow">NORMALIZED FRAMES</span><h3>{animation.name} <small>{animation.facing}</small></h3></div><button className="button secondary" disabled={busy} onClick={() => void importAnimationFrames()}><ImagePlus size={16} /> Import PNG frames</button></div>
      {frameCount ? <><div className="frame-strip" role="group" aria-label="Animation frames">{animation.frames.map((frame, index) => <button key={frame.id} type="button" className={`frame-thumb ${index === selectedIndex ? "selected" : ""}`} onClick={() => { setSelectedIndex(index); setPlaybackIndex(index); }} title={frame.originalName} aria-label={`Frame ${index + 1}: ${frame.originalName}`}><span>{String(index + 1).padStart(2, "0")}</span>{framePreviews[frame.id] && <img src={framePreviews[frame.id].dataUrl} alt="" draggable={false} />}<small>{frame.originalName}</small></button>)}</div>
      <div className="animation-panels"><div className="animation-panel"><div className="animation-panel-heading"><div><span className="eyebrow">ALIGNMENT EDITOR</span><h4>Frame {selectedIndex + 1} of {frameCount}</h4></div><span className={`snap-state ${animation.stages.align}`}>{animation.stages.align}</span></div><WorkflowTip label="Alignment · whole pixels">Keep the shared ({project.runtime.pivotX}, {project.runtime.pivotY}) pivot. Use onion skin to correct only visible one- or two-pixel drift; never resize an individual frame.</WorkflowTip><div className="animation-canvas-wrap" tabIndex={0} role="group" aria-label="Alignment canvas. Arrow keys move one pixel; Shift and arrows move five pixels" onKeyDown={keyNudge}><CellView project={project} animation={animation} images={framePreviews} index={selectedIndex} offsets={offsets} zoom={zoom} onion={onion} grid={grid} guides={guides} onPointerNudge={nudge} /></div><div className="animation-offset"><span>X <strong>{current.x}</strong></span><span>Y <strong>{current.y}</strong></span><button className="button ghost" onClick={() => nudge(0, 0)} disabled={busy}><RotateCcw size={14} /> Reset frame</button></div><div className="animation-options"><label><input type="checkbox" checked={onion} onChange={event => setOnion(event.target.checked)} /> Onion skin</label><label><input type="checkbox" checked={grid} onChange={event => setGrid(event.target.checked)} /> Pixel grid</label><label><input type="checkbox" checked={guides} onChange={event => setGuides(event.target.checked)} /> Pivot / baseline</label></div><p className="animation-hint">Drag the frame or focus the canvas and use arrow keys. Shift moves five pixels. No source pixels are changed.</p><div className="animation-actions"><button className="button secondary" onClick={() => void proposeAlignment(offsets)} disabled={busy || offsets.length !== frameCount}>Review offsets</button>{alignReview && !alignApplied && <button className="button primary" onClick={() => void applyAlignment(alignReview.id)} disabled={busy || dirty}><Check size={15} /> Apply alignment</button>}{alignApplied && !dirty && <span className="snap-applied"><Check size={15} /> Applied</span>}</div></div>
      <div className="animation-panel"><div className="animation-panel-heading"><div><span className="eyebrow">TIMED PREVIEW</span><h4>Motion at native pixels</h4></div><span className={`snap-state ${animation.stages.preview}`}>{animation.stages.preview}</span></div><WorkflowTip label="Playback · review">8 FPS is a study default, not a KangiFight timing rule. Check the first/last loop seam, native pixels, nearest-neighbour enlargement, and any slide or flicker. KangiFight gameplay exposure comes from its approved hold-tick register.</WorkflowTip><div className="animation-canvas-wrap playback" tabIndex={0} role="region" aria-label="Timed animation preview viewport"><CellView project={project} animation={animation} images={framePreviews} index={playbackIndex} offsets={offsets} zoom={zoom} onion={false} grid={grid} guides={guides} nearest={nearest} /></div><div className="animation-player"><button aria-label="Previous frame" onClick={() => { setPlaying(false); setPlaybackIndex(index => (index - 1 + frameCount) % frameCount); }}><ChevronLeft size={18} /></button><button aria-label={playing ? "Pause" : "Play"} onClick={() => setPlaying(value => !value)}>{playing ? <Pause size={18} /> : <Play size={18} />}</button><button aria-label="Next frame" onClick={() => { setPlaying(false); setPlaybackIndex(index => (index + 1) % frameCount); }}><ChevronRight size={18} /></button><span>{playbackIndex + 1} / {frameCount}</span></div><div className="animation-settings"><label>FPS <input className="text-input" type="number" min="1" max="60" step="1" value={fps} onChange={event => setFps(Number(event.target.value))} aria-invalid={!fpsValid} /></label><label><input type="checkbox" checked={looping} onChange={event => setLooping(event.target.checked)} /> Loop</label><label><input type="checkbox" checked={nearest} onChange={event => setNearest(event.target.checked)} /> Nearest</label></div><p className="animation-hint">Check the loop seam, jitter, silhouette, and contact readability before applying the preview settings.</p><div className="animation-actions"><button className="button secondary" disabled={busy || !animation.activeAlignmentId || !fpsValid || dirty} onClick={() => void proposeAnimationPreview(fps, looping)}><Layers2 size={15} /> Review playback</button>{previewReview && !previewApplied && <button className="button primary" disabled={busy || !previewSettingsMatch} onClick={() => void applyAnimationPreview(previewReview.id)}><Check size={15} /> Apply preview</button>}{previewApplied && <span className="snap-applied"><Check size={15} /> Applied</span>}</div></div></div><ExportPanel project={project} animation={animation} /></> : <div className="animation-empty">No normalized frames yet. Apply a pose-board extraction, then process its raw frames, or import fixed-cell binary-alpha PNGs.</div>}</div> : <div className="animation-empty">Start here: name the animation, choose its facing, and select Create animation. The pose-board import appears next.</div>}
  </section>;
}
