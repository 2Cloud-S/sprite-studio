import { useEffect, useRef, useState } from "react";
import { Check, Download, FolderOpen, ImagePlus } from "lucide-react";
import { desktop } from "./services/desktop";
import { useStudio } from "./store/studio";
import type { Animation, Project } from "./types";

export function UpscaleHandoffPanel({ animation, project }: { animation: Animation; project: Project }) {
  const { settings, busy, batchSnapPreviews, upscaledPreviews, handoffFolder, handoffDropArmed, pendingHandoffPaths, importUpscaledFrame, approveUpscale, exportSnapped, openSnapFolder } = useStudio();
  const snap = animation.batchSnaps.find(item => item.id === animation.activeBatchSnapId);
  const [selected, setSelected] = useState<string | null>(null);
  const [pending, setPending] = useState<string[]>([]);
  const [mapping, setMapping] = useState<Record<string, string>>({});
  const [notice, setNotice] = useState<"required" | "missing" | "mapping" | null>(null);
  const noticeDialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { if (snap) setSelected(snap.frames[0]?.sourceFrameId ?? null); }, [snap?.id]);
  useEffect(() => { if (snap && animation.stages.upscale === "needs_input") setNotice("required"); }, [snap?.id]);
  useEffect(() => { const node = noticeDialog.current; if (notice && node && !node.open) node.showModal(); else if (!notice && node?.open) node.close(); }, [notice]);
  useEffect(() => { if (pendingHandoffPaths.length) { queueFiles(pendingHandoffPaths); useStudio.setState({ pendingHandoffPaths: [] }); } }, [pendingHandoffPaths]);
  if (!snap) return null;
  const missing = snap.frames.filter(frame => !animation.activeUpscaledFrameIds[frame.sourceFrameId]).length;
  const root = settings?.workspaceRoot ? `${settings.workspaceRoot}/projects/${project.slug}-${project.id.slice(0, 8)}` : "";
  function queueFiles(paths: string[]) {
    const next: Record<string, string> = {};
    for (const path of paths) {
      const match = /frame[_ -]?(\d+)/i.exec(path.split(/[\\/]/).pop() ?? "");
      const index = match ? Number(match[1]) - 1 : -1;
      if (index >= 0 && index < snap!.frames.length) next[path] = snap!.frames[index].sourceFrameId;
    }
    setMapping(next); setPending(paths); setNotice("mapping");
  }
  async function chooseAll() {
    try { const paths = await desktop.chooseUpscaledFrames(); if (Array.isArray(paths) && paths.length) queueFiles(paths); }
    catch (error) { useStudio.setState({ error: String(error) }); }
  }
  async function importMapped() {
    const targets = pending.map(path => mapping[path]);
    if (targets.some(value => !value) || new Set(targets).size !== targets.length) { useStudio.setState({ error: "UPSCALE_IMPORT_MAPPING_REQUIRED: Choose a unique frame for every imported file." }); return; }
    for (const path of pending) { if (!await importUpscaledFrame(mapping[path], path)) return; }
    setPending([]); setMapping({}); setNotice(null);
  }
  return <div className="batch-stage handoff-stage" aria-label="Upscale Handoff">
    <div className="animation-panel-heading"><div><span className="eyebrow">OPTIONAL STAGE · MANUAL EXTERNAL UPSCALE</span><h4>Upscale required</h4></div><span className={`snap-state ${animation.stages.upscale}`}>{animation.stages.upscale}</span></div>
    <p className="animation-hint">The per-frame snap is complete. Export or locate the native snapped PNGs, upscale them externally, then import one result per frame. The app pauses here; it does not assume the upscale happened internally. No second snap pass.</p>
    <div className="animation-actions"><button className="button secondary" disabled={busy} onClick={() => void openSnapFolder()}><FolderOpen size={15} /> Open Folder</button><button className="button secondary" disabled={busy || !selected} onClick={() => void exportSnapped(selected ? [selected] : [])}><Download size={15} /> Export Selected</button><button className="button secondary" disabled={busy} onClick={() => void exportSnapped([])}><Download size={15} /> Export All</button><button className="button secondary" disabled={busy} onClick={() => void chooseAll()}><ImagePlus size={15} /> Import All Upscaled Frames</button><button className="button secondary" disabled={busy} onClick={() => useStudio.setState({ handoffDropArmed: !handoffDropArmed })}>{handoffDropArmed ? "Drop PNGs anywhere now · Cancel" : "Import by drop"}</button></div>
    {handoffFolder && <p className="animation-hint">Exported to <code>{handoffFolder}</code></p>}
    <p className="animation-hint">{missing ? `UPSCALE_MISSING_FRAMES: ${missing} of ${snap.frames.length} upscaled frames are still missing.` : animation.upscaleApproved ? "All upscaled inputs approved. Background clean can continue." : "All inputs supplied. Review them and choose Continue."} {missing > 0 && <button className="button ghost" onClick={() => setNotice("missing")}>Why paused?</button>}</p>
    <div className="handoff-grid">{snap.frames.map((frame, index) => { const upscaleId = animation.activeUpscaledFrameIds[frame.sourceFrameId]; const upscale = animation.upscaledFrames.find(item => item.id === upscaleId); return <div className={`handoff-frame ${selected === frame.sourceFrameId ? "selected" : ""}`} key={frame.sourceFrameId}>
      <label><input type="radio" name="handoff-frame" checked={selected === frame.sourceFrameId} onChange={() => setSelected(frame.sourceFrameId)} /> Frame {String(index + 1).padStart(3, "0")} · {upscale ? animation.upscaleApproved ? "approved" : "supplied" : "waiting_for_upscale"}</label>
      <div className="handoff-images"><div className="checkerboard">{batchSnapPreviews[frame.sourceFrameId] && <img src={batchSnapPreviews[frame.sourceFrameId].dataUrl} alt={`Native snapped frame ${index + 1}`} />}</div><div className="checkerboard">{upscaledPreviews[frame.sourceFrameId] && <img src={upscaledPreviews[frame.sourceFrameId].dataUrl} alt={`Imported upscaled frame ${index + 1}`} />}</div></div>
      <small>Native {frame.width}×{frame.height} {upscale && `→ upscaled ${upscale.width}×${upscale.height} · v${upscale.version}`}</small><code title={`${root}/${frame.relativePath}`}>{root}/{frame.relativePath}</code><button className="button secondary" disabled={busy} onClick={() => void importUpscaledFrame(frame.sourceFrameId)}><ImagePlus size={14} /> {upscale ? "Import new version" : "Import Upscaled Frame"}</button>
    </div>; })}</div>
    <dialog ref={noticeDialog} className="handoff-dialog" onClose={() => { setNotice(null); if (notice === "mapping") setPending([]); }}>
      {notice === "mapping" ? <div className="handoff-mapping" aria-label="Confirm upscale import mapping"><h4>Confirm file-to-frame mapping</h4><p>UPSCALE_IMPORT_MAPPING_REQUIRED: Choose the source frame for each file. Matching filenames were preselected.</p>{pending.map(path => <label key={path}><code>{path.split(/[\\/]/).pop()}</code><select className="text-input" value={mapping[path] ?? ""} onChange={event => setMapping(current => ({ ...current, [path]: event.target.value }))}><option value="">Choose frame</option>{snap.frames.map((frame, index) => <option key={frame.sourceFrameId} value={frame.sourceFrameId}>Frame {String(index + 1).padStart(3, "0")}</option>)}</select></label>)}<div className="animation-actions"><button className="button secondary" onClick={() => { setPending([]); setNotice(null); }}>Cancel</button><button className="button primary" disabled={busy || pending.some(path => !mapping[path]) || new Set(pending.map(path => mapping[path])).size !== pending.length} onClick={() => void importMapped()}>Import mapped files</button></div></div> : <><h4>{notice === "required" ? "UPSCALE_REQUIRED" : "UPSCALE_MISSING_FRAMES"}</h4><p>{notice === "required" ? "Upscaled frame inputs are required before background cleaning can continue." : `${missing} of ${snap.frames.length} upscaled frames are still missing.`}</p><button className="button primary" onClick={() => setNotice(null)}>Understood</button></>}
    </dialog>
    <div className="animation-actions"><button className="button primary" disabled={busy || missing > 0 || animation.upscaleApproved} onClick={() => void approveUpscale()}><Check size={15} /> Continue to background clean</button></div>
  </div>;
}
