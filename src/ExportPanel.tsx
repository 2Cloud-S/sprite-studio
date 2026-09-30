import { useEffect, useState } from "react";
import { Check, Download, FolderOpen, ShieldAlert } from "lucide-react";
import { useStudio } from "./store/studio";
import { WorkflowTip } from "./WorkflowTip";
import type { Animation, ExportOptions, Project } from "./types";

export function ExportPanel({ project, animation }: { project: Project; animation: Animation }) {
  const { settings, exportSheet, exportGif, exportManifest, busy, runExport, applyExport, openAnimationExportFolder } = useStudio();
  const fixed = project.preset === "kangi-fight";
  const [columns, setColumns] = useState(() => Math.min(project.workflow.sheetColumns, Math.max(1, animation.frames.length)));
  const [padding, setPadding] = useState(0);
  const [spacing, setSpacing] = useState(0);
  useEffect(() => {
    setColumns(fixed ? animation.frames.length : Math.min(project.workflow.sheetColumns, Math.max(1, animation.frames.length)));
    setPadding(0);
    setSpacing(0);
  }, [animation.id, animation.frames.length, fixed, project.workflow.sheetColumns]);

  const options: ExportOptions = { columns: fixed ? animation.frames.length : columns, padding: fixed ? 0 : padding, spacing: fixed ? 0 : spacing };
  const valid = Number.isInteger(options.columns) && options.columns >= 1 && options.columns <= Math.min(fixed ? 64 : 16, animation.frames.length) &&
    Number.isInteger(options.padding) && options.padding >= 0 && options.padding <= 64 &&
    Number.isInteger(options.spacing) && options.spacing >= 0 && options.spacing <= 32;
  const review = animation.exports.slice().reverse().find(item => item.alignmentId === animation.activeAlignmentId && item.previewId === animation.activePreviewId) ?? null;
  const applied = !!review && animation.activeExportId === review.id;
  const exportFolder = review?.sheetRelativePath.slice(0, review.sheetRelativePath.lastIndexOf("/")) ?? null;

  return <section className="export-panel" aria-labelledby="export-title">
    <div className="animation-subheading">
      <div><span className="eyebrow">STAGE 05 · LOCAL EXPORT</span><h3 id="export-title">Sheet, GIF, and manifest</h3><p>Generated from applied alignment and playback settings. The output is a draft, not a Godot production asset.</p></div>
      <span className={`snap-state ${animation.stages.export}`}>{animation.stages.export}</span>
    </div>
    <WorkflowTip label="Sheet settings">{fixed
      ? `KangiFight requires one horizontal row: ${animation.frames.length} × ${project.runtime.cellWidth} = ${animation.frames.length * project.runtime.cellWidth}px wide, ${project.runtime.cellHeight}px high; padding 0, spacing 0.`
      : `Generic starts at up to ${project.workflow.sheetColumns} columns and ${project.workflow.sheetRows} rows (${project.runtime.cellWidth * project.workflow.sheetColumns}×${project.runtime.cellHeight * project.workflow.sheetRows} for a full sheet), padding 0, spacing 0; rows auto-fit the frame count. Every cell stays ${project.runtime.cellWidth} × ${project.runtime.cellHeight}.`} Review the sheet, GIF, and manifest together before Apply.</WorkflowTip>
    <div className="export-controls">
      <label>Columns<input className="text-input" type="number" min="1" max={Math.min(fixed ? 64 : 16, animation.frames.length)} value={options.columns} disabled={fixed} onChange={event => setColumns(Number(event.target.value))} /></label>
      <label>Padding<input className="text-input" type="number" min="0" max="64" value={options.padding} disabled={fixed} onChange={event => setPadding(Number(event.target.value))} /></label>
      <label>Spacing<input className="text-input" type="number" min="0" max="32" value={options.spacing} disabled={fixed} onChange={event => setSpacing(Number(event.target.value))} /></label>
      <button className="button secondary" disabled={busy || !valid || !settings?.pythonExecutable || !animation.activePreviewId} onClick={() => void runExport(options)}><Download size={16} /> {review ? "Export again" : "Build export"}</button>
    </div>
    {fixed && <p className="export-rule">KangiFight draft: horizontal 128 × 128 strip, zero padding and spacing. Gameplay timing still comes from the hold-tick register.</p>}
    {!animation.activePreviewId && <p className="export-rule">Apply a timed preview before exporting.</p>}
    {review && <div className="export-review">
      <div className="snap-review-heading"><div><span className="eyebrow">{applied ? "ACTIVE EXPORT" : "REVIEW CANDIDATE"}</span><h4>{review.columns} columns · {review.rows} rows · {review.sheetWidth} × {review.sheetHeight}</h4></div><span className="snap-review-meta">{review.gifDurationMs} ms GIF frames</span></div>
      <div className="export-artifacts">
        <div><strong>Spritesheet PNG</strong><div className="export-sheet checkerboard" role="region" aria-label="Spritesheet viewport" tabIndex={0}>{exportSheet ? <img src={exportSheet.dataUrl} alt={`Spritesheet for ${animation.name}`} draggable={false} /> : <span>Loading sheet…</span>}</div><small title={review.sheetSha256}>SHA-256 {review.sheetSha256.slice(0, 16)}…</small></div>
        <div><strong>Preview GIF</strong><div className="export-gif checkerboard" role="region" aria-label="Animated GIF viewport" tabIndex={0}>{exportGif ? <img src={exportGif.dataUrl} alt={`Animated preview for ${animation.name}`} draggable={false} /> : <span>Loading GIF…</span>}</div><small>{animation.frames.length} frames · study timing</small></div>
      </div>
      <details className="export-manifest"><summary>Inspect manifest · DRAFT</summary><pre>{exportManifest ? JSON.stringify(exportManifest, null, 2) : "Loading manifest…"}</pre></details>
      {exportFolder && <div className="export-location"><span>Saved in project</span><code title={exportFolder}>{exportFolder}</code><button className="button secondary" disabled={busy} onClick={() => void openAnimationExportFolder(review.id)}><FolderOpen size={16} /> Open export folder</button></div>}
      <div className="snap-review-actions"><p><ShieldAlert size={16} /> Review palette, rights, anatomy, timing, and device import before any production use.</p>{applied ? <span className="snap-applied"><Check size={16} /> Applied export</span> : <button className="button primary" disabled={busy || !exportSheet || !exportGif || !exportManifest} onClick={() => void applyExport(review.id)}><Check size={16} /> Apply export</button>}</div>
    </div>}
  </section>;
}
