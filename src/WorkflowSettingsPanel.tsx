import { useEffect, useState } from "react";
import { Check } from "lucide-react";
import { useStudio } from "./store/studio";
import type { Project, WorkflowSettings } from "./types";

export function WorkflowSettingsPanel({ project }: { project: Project }) {
  const { busy, updateWorkflow, updateRuntime, updateRuntimePolicy } = useStudio();
  const [workflow, setWorkflow] = useState<WorkflowSettings>(project.workflow);
  const [cellWidth, setCellWidth] = useState(project.runtime.cellWidth);
  const [cellHeight, setCellHeight] = useState(project.runtime.cellHeight);
  const [pivotX, setPivotX] = useState(project.runtime.pivotX);
  const [pivotY, setPivotY] = useState(project.runtime.pivotY);
  const [geometryPolicy, setGeometryPolicy] = useState(project.runtime.geometryPolicy);
  const [neutralTarget, setNeutralTarget] = useState(project.runtime.neutralHeightTarget?.toString() ?? "");
  const [neutralTolerance, setNeutralTolerance] = useState(project.runtime.neutralTolerancePx);

  useEffect(() => setWorkflow(project.workflow), [project.id, project.workflow]);
  useEffect(() => {
    setCellWidth(project.runtime.cellWidth);
    setCellHeight(project.runtime.cellHeight);
    setPivotX(project.runtime.pivotX);
    setPivotY(project.runtime.pivotY);
    setGeometryPolicy(project.runtime.geometryPolicy);
    setNeutralTarget(project.runtime.neutralHeightTarget?.toString() ?? "");
    setNeutralTolerance(project.runtime.neutralTolerancePx);
  }, [project.id, project.runtime]);

  const fixedLegacyGeometry = project.preset === "kangi-fight";
  const geometryLocked = project.animations.some(animation => animation.frames.length > 0 || animation.historicalFrames.length > 0)
    || Object.values(project.anchors).some(anchor => !!anchor.activeCleanupId);
  const geometryDisabled = fixedLegacyGeometry || geometryLocked || project.runtime.geometryPolicy === "locked";
  const targetNumber = neutralTarget.trim() === "" ? null : Number(neutralTarget);
  const policyValid = (targetNumber === null || (Number.isInteger(targetNumber) && targetNumber >= 1 && targetNumber <= project.runtime.cellHeight))
    && Number.isInteger(neutralTolerance) && neutralTolerance >= 0 && neutralTolerance <= 32;
  const workflowValid = Number.isInteger(workflow.kColors) && workflow.kColors >= 2 && workflow.kColors <= 256
    && /^#[\da-fA-F]{6}$/.test(workflow.chroma)
    && Number.isInteger(workflow.sheetColumns) && workflow.sheetColumns >= 1 && workflow.sheetColumns <= 16
    && Number.isInteger(workflow.sheetRows) && workflow.sheetRows >= 1 && workflow.sheetRows <= 16
    && Number.isInteger(workflow.typicalNudgeRangePx) && workflow.typicalNudgeRangePx >= 0 && workflow.typicalNudgeRangePx <= 5;
  const runtimeValid = [cellWidth, cellHeight, pivotX, pivotY].every(Number.isInteger)
    && cellWidth >= 16 && cellWidth <= 1024 && cellHeight >= 16 && cellHeight <= 1024
    && pivotX >= 0 && pivotX < cellWidth && pivotY >= 0 && pivotY < cellHeight
    && (workflow.anchorMode !== "bottom-center" || (pivotX === Math.floor(cellWidth / 2) && pivotY === cellHeight - 1));

  const editPivot = (axis: "x" | "y", value: number) => {
    if (axis === "x") setPivotX(value);
    else setPivotY(value);
    setWorkflow(current => ({ ...current, anchorMode: "custom" }));
  };
  const changeAnchorMode = (mode: WorkflowSettings["anchorMode"]) => {
    setWorkflow(current => ({ ...current, anchorMode: mode }));
    if (mode === "bottom-center") {
      setPivotX(Math.floor(cellWidth / 2));
      setPivotY(cellHeight - 1);
    }
  };

  return <details className="pose-panel workflow-settings-panel">
    <summary>Project workflow settings <small>{project.runtime.cellWidth * project.workflow.sheetColumns}×{project.runtime.cellHeight * project.workflow.sheetRows} draft sheet · {project.workflow.upscaleMode === "manual-handoff" ? "manual upscale" : "automatic"}</small></summary>
    <p className="animation-hint">These settings belong to the character project and apply to both single references and pose-board animations. Snap colors and chroma are starting values; each processing stage still has its own review and Apply step.</p>

    <div className="batch-controls">
      <label>Snap colors<input className="text-input" type="number" min="2" max="256" value={workflow.kColors} onChange={event => setWorkflow({ ...workflow, kColors: Number(event.target.value) })} /></label>
      <label>Chroma / key color<input className="text-input" value={workflow.chroma} onChange={event => setWorkflow({ ...workflow, chroma: event.target.value })} /></label>
      <label>Upscale mode<select className="text-input" value={workflow.upscaleMode} onChange={event => setWorkflow({ ...workflow, upscaleMode: event.target.value as WorkflowSettings["upscaleMode"] })}><option value="automatic">Automatic inside snapper</option><option value="manual-handoff">Manual external handoff</option></select></label>
    </div>
    <div className="batch-controls">
      <label>Sheet columns<input className="text-input" type="number" min="1" max="16" disabled={fixedLegacyGeometry} value={workflow.sheetColumns} onChange={event => setWorkflow({ ...workflow, sheetColumns: Number(event.target.value) })} /></label>
      <label>Sheet rows<input className="text-input" type="number" min="1" max="16" disabled={fixedLegacyGeometry} value={workflow.sheetRows} onChange={event => setWorkflow({ ...workflow, sheetRows: Number(event.target.value) })} /></label>
      <label>Typical nudge (px)<input className="text-input" type="number" min="0" max="5" value={workflow.typicalNudgeRangePx} onChange={event => setWorkflow({ ...workflow, typicalNudgeRangePx: Number(event.target.value) })} /></label>
    </div>
    <div className="animation-actions">
      <label><input type="checkbox" checked={workflow.manualPolishEnabled} onChange={event => setWorkflow({ ...workflow, manualPolishEnabled: event.target.checked })} /> Manual aligner polish</label>
      <label><input type="checkbox" checked={workflow.greenFringeDespeckle} onChange={event => setWorkflow({ ...workflow, greenFringeDespeckle: event.target.checked })} /> Green fringe cleanup</label>
      <button className="button secondary" disabled={busy || !workflowValid} onClick={() => void updateWorkflow({ ...workflow, anchorMode: project.workflow.anchorMode })}><Check size={15} /> Save workflow</button>
    </div>

    <div className="runtime-policy">
      <strong>Runtime geometry policy</strong>
      <p className="animation-hint">Locked keeps the configured cell and pivot fixed. Auto-fit can try a coarser recovered pixel grid, but every candidate still needs review and Apply.</p>
      <div className="runtime-policy-choices" role="group" aria-label="Runtime geometry policy">
        <label><input type="radio" name={`geometry-policy-${project.id}`} value="flexible" checked={geometryPolicy === "flexible"} disabled={fixedLegacyGeometry} onChange={() => setGeometryPolicy("flexible")} /> Flexible</label>
        <label><input type="radio" name={`geometry-policy-${project.id}`} value="locked" checked={geometryPolicy === "locked"} disabled={fixedLegacyGeometry} onChange={() => setGeometryPolicy("locked")} /> Locked</label>
      </div>
      <p className="animation-hint">Cell: {project.runtime.cellWidth} × {project.runtime.cellHeight} · Pivot: ({project.runtime.pivotX},{project.runtime.pivotY})</p>
      <div className="batch-controls">
        <label>Neutral target height (px)<input className="text-input" type="number" min="1" max={project.runtime.cellHeight} placeholder="Optional" value={neutralTarget} disabled={fixedLegacyGeometry} onChange={event => setNeutralTarget(event.target.value)} /></label>
        <label>Neutral tolerance (± px)<input className="text-input" type="number" min="0" max="32" value={neutralTolerance} disabled={fixedLegacyGeometry} onChange={event => setNeutralTolerance(Number(event.target.value))} /></label>
      </div>
      {!fixedLegacyGeometry && <button className="button secondary" disabled={busy || !policyValid} onClick={() => void updateRuntimePolicy(geometryPolicy, targetNumber, neutralTolerance)}><Check size={15} /> Save runtime policy</button>}
      {fixedLegacyGeometry && <small>This preset stays locked at a 64 px neutral target with ±4 px tolerance.</small>}
    </div>

    <p className="animation-hint">Runtime cell and pivot use whole-pixel coordinates. Editing Pivot X or Pivot Y selects Custom pivot automatically. Save runtime geometry to apply those values; source images are not resized.</p>
    <div className="batch-controls">
      <label>Cell width<input className="text-input" type="number" min="16" max="1024" disabled={geometryDisabled} value={cellWidth} onChange={event => { const width = Number(event.target.value); setCellWidth(width); if (workflow.anchorMode === "bottom-center") setPivotX(Math.floor(width / 2)); }} /></label>
      <label>Cell height<input className="text-input" type="number" min="16" max="1024" disabled={geometryDisabled} value={cellHeight} onChange={event => { const height = Number(event.target.value); setCellHeight(height); if (workflow.anchorMode === "bottom-center") setPivotY(height - 1); }} /></label>
      <label>Pivot X<input className="text-input" type="number" min="0" max={Math.max(0, cellWidth - 1)} disabled={geometryDisabled} value={pivotX} onChange={event => editPivot("x", Number(event.target.value))} /></label>
      <label>Pivot Y<input className="text-input" type="number" min="0" max={Math.max(0, cellHeight - 1)} disabled={geometryDisabled} value={pivotY} onChange={event => editPivot("y", Number(event.target.value))} /></label>
      <label>Anchor mode<select className="text-input" disabled={geometryDisabled} value={workflow.anchorMode} onChange={event => changeAnchorMode(event.target.value as WorkflowSettings["anchorMode"])}><option value="bottom-center">Bottom-center</option><option value="custom">Custom pivot</option></select></label>
    </div>
    {!fixedLegacyGeometry && <div className="animation-actions">
      <button className="button secondary" disabled={busy || geometryDisabled || !runtimeValid} onClick={() => void updateRuntime(cellWidth, cellHeight, pivotX, pivotY, workflow.anchorMode)}><Check size={15} /> Save runtime geometry</button>
      <small>{geometryLocked ? "Geometry is locked after normalized artwork is applied." : project.runtime.geometryPolicy === "locked" ? "Switch to Flexible and save the policy before editing geometry." : "Pivot changes save with geometry, not with Save workflow."}</small>
    </div>}
    {fixedLegacyGeometry && <p className="animation-hint">This existing project's runtime geometry is fixed. New projects can edit their cell and pivot before normalized artwork is applied.</p>}
  </details>;
}
