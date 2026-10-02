import { useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { isTauri } from "@tauri-apps/api/core";
import {
  ArrowDown,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  Check,
  CircleAlert,
  FolderOpen,
  ImagePlus,
  Layers3,
  Maximize2,
  Plus,
  RotateCcw,
  ShieldCheck,
  Sparkles,
  Trash2,
  X,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import { useStudio } from "./store/studio";
import { SnapStage } from "./SnapStage";
import { CleanupStage } from "./CleanupStage";
import { ReferenceExportPanel } from "./ReferenceExportPanel";
import { AnimationWorkspace } from "./AnimationWorkspace";
import { WorkflowSettingsPanel } from "./WorkflowSettingsPanel";
import { WorkflowTip } from "./WorkflowTip";
import { readablePath } from "./domain/paths";
import type { Facing, Preset, Project } from "./types";
import "./styles.css";

const facingIcons = {
  north: ArrowUp,
  south: ArrowDown,
  east: ArrowRight,
  west: ArrowLeft,
  right: ArrowRight,
};
const stageLabels = [
  ["raw", "Source import"],
  ["snap", "Pixel snap"],
  ["cleanup", "Chroma cleanup"],
  ["normalize", "Normalize"],
  ["align", "Frame alignment"],
  ["preview", "Animation review"],
  ["export", "Export"],
] as const;
const boardStageLabels = [
  ["board", "Pose board"], ["extract", "Frame recovery"], ["native_review", "Native review"], ["snap", "Per-frame pixel snap"], ["upscale", "Upscale handoff"],
  ["cleanup", "Remove chroma"], ["normalize", "Fixed-cell frames"],
  ["align", "Align frames"], ["preview", "Preview motion"], ["export", "Export draft"],
] as const;

function projectLabel(preset: Preset) {
  return preset === "generic" ? "Generic" : "KangiFight";
}
function StudioMark() {
  return (
    <div className="studio-mark" aria-hidden="true">
      <span />
      <span />
      <span />
      <span />
    </div>
  );
}

function CreateProjectDialog({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [name, setName] = useState("");
  const [touched, setTouched] = useState(false);
  const createProject = useStudio((s) => s.createProject);
  const busy = useStudio((s) => s.busy);
  useEffect(() => {
    const node = dialog.current;
    if (open && node && !node.open) node.showModal();
    if (!open && node?.open) node.close();
  }, [open]);
  const invalid = touched && !name.trim();
  return (
    <dialog
      ref={dialog}
      className="create-dialog"
      onClose={onClose}
      onCancel={onClose}
    >
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          setTouched(true);
          if (!name.trim()) return;
          if (await createProject(name, "generic")) {
            setName("");
            setTouched(false);
            onClose();
          }
        }}
      >
        <div className="dialog-heading">
          <div>
            <span className="eyebrow">NEW CHARACTER</span>
            <h2>Create project</h2>
          </div>
          <button
            className="icon-button"
            type="button"
            onClick={onClose}
            aria-label="Close"
          >
            <X size={18} />
          </button>
        </div>
        <label className="field-label" htmlFor="character-name">
          Character name
        </label>
        <input
          id="character-name"
          autoFocus
          className="text-input"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onBlur={() => setTouched(true)}
          aria-invalid={invalid}
          aria-describedby="name-help"
          placeholder="e.g. Ember Fox"
          maxLength={80}
        />
        <div
          id="name-help"
          className={`field-help ${invalid ? "field-error" : ""}`}
        >
          {invalid
            ? "Enter a name to create this character."
            : "A safe folder name will be created automatically."}
        </div>
        <div className="project-defaults">
          <Layers3 size={20} aria-hidden="true" />
          <span>
            <strong>Flexible character workspace</strong>
            <small>Four directions · 256 × 256 cells. You can adjust the runtime cell later.</small>
          </span>
        </div>
        <div className="dialog-actions">
          <button type="button" className="button ghost" onClick={onClose}>
            Cancel
          </button>
          <button className="button primary" disabled={busy} type="submit">
            {busy ? "Creating…" : "Create character"}
          </button>
        </div>
      </form>
    </dialog>
  );
}

function Sidebar({ onNew }: { onNew: () => void }) {
  const { projects, deletedProjects, project, openProject, deleteProject, restoreProject, busy, settings, chooseWorkspace, openWorkspaceFolder, openProjectExportsFolder } =
    useStudio();
  const workspacePath = settings?.workspaceRoot ? readablePath(settings.workspaceRoot) : null;
  const [recentlyDeleted, setRecentlyDeleted] = useState<{ id: string; name: string } | null>(null);
  useEffect(() => {
    if (!recentlyDeleted) return;
    const timer = window.setTimeout(() => setRecentlyDeleted(null), 10000);
    return () => window.clearTimeout(timer);
  }, [recentlyDeleted]);
  return (
    <aside className="sidebar" aria-label="Project library">
      <div className="brand">
        <StudioMark />
        <div>
          <strong>Sprite Studio</strong>
          <span>CHARACTER WORKSPACE</span>
        </div>
      </div>
      <div className="sidebar-section-label">
        LIBRARY <span>{projects.length}</span>
      </div>
      <button className="new-project" onClick={onNew}>
        <Plus size={18} /> New character
      </button>
      <nav className="project-list" aria-label="Character projects">
        {projects.map((item) => (
          <div className={`project-row ${project?.id === item.id ? "active" : ""}`} key={item.id}>
            <button
              className={`project-item ${project?.id === item.id ? "active" : ""}`}
              onClick={() => void openProject(item.id)}
              aria-current={project?.id === item.id ? "page" : undefined}
              disabled={busy}
            >
              <span className="project-avatar">{item.name.slice(0, 1).toUpperCase()}</span>
              <span className="project-copy"><strong>{item.name}</strong><small>{projectLabel(item.preset)} · {item.importedCount} imports</small></span>
            </button>
            <button className="project-delete" disabled={busy} aria-label={`Delete ${item.name}`} title={`Delete ${item.name} (recoverable)`} onClick={async () => {
              if (await deleteProject(item.id)) setRecentlyDeleted({ id: item.id, name: item.name });
            }}><Trash2 size={16} /></button>
          </div>
        ))}
        {projects.length === 0 && (
          <p className="sidebar-empty">
            Your character projects will appear here.
          </p>
        )}
      </nav>
      {recentlyDeleted && <div className="sidebar-undo" role="status"><span>{recentlyDeleted.name} moved to Recently deleted.</span><button disabled={busy} onClick={() => { void restoreProject(recentlyDeleted.id); setRecentlyDeleted(null); }}>Undo</button></div>}
      {deletedProjects.length > 0 && <details className="deleted-projects"><summary>Recently deleted <span>{deletedProjects.length}</span></summary><div>{deletedProjects.map(item => <div className="deleted-project" key={item.id}><span title={item.name}>{item.name}</span><button disabled={busy} onClick={() => { void restoreProject(item.id); setRecentlyDeleted(null); }} aria-label={`Restore ${item.name}`}><RotateCcw size={14} /> Restore</button></div>)}</div><p>Kept in this workspace’s deleted-projects folder.</p></details>}
      <div className="sidebar-bottom">
        <div className="workspace-caption">WORKSPACE</div>
        <button
          className="workspace-button"
          onClick={() => void openWorkspaceFolder()}
          aria-label="Open selected workspace folder"
          title={workspacePath ?? "Workspace is not selected"}
        >
          <FolderOpen size={18} />
          <span>
            {workspacePath
              ?.split(/[\\/]/)
              .filter(Boolean)
              .slice(-1)[0] ?? "No workspace"}
          </span>
        </button>
        <small className="workspace-path" title={workspacePath ?? undefined}>{workspacePath}</small>
        <button className="workspace-change" onClick={() => void chooseWorkspace()}>Change workspace…</button>
        <div className="workspace-caption exports-caption">PROJECT EXPORTS</div>
        <button className="workspace-button" onClick={() => void openProjectExportsFolder()} disabled={!project} title={project ? `Open exports for ${project.name}` : "Select a character project first"}>
          <FolderOpen size={18} /> <span>{project ? `${project.name} exports` : "Select a character"}</span>
        </button>
      </div>
    </aside>
  );
}

function AnchorCard({
  facing,
  project,
  selected,
  onSelect,
  onImport,
}: {
  facing: Facing;
  project: Project;
  selected: boolean;
  onSelect: () => void;
  onImport: () => void;
}) {
  const Icon = facingIcons[facing];
  const active = project.imports.find(
    (record) => record.id === project.anchors[facing]?.activeImportId,
  );
  return (
    <div className={`anchor-card ${selected ? "selected" : ""}`}>
      <button
        className="anchor-select"
        onClick={onSelect}
        aria-pressed={selected}
      >
        <span className="anchor-icon">
          <Icon size={17} />
        </span>
        <span>
          <strong>{facing[0].toUpperCase() + facing.slice(1)}</strong>
          <small>
            {active
              ? `${active.width} × ${active.height} reference`
              : "No reference image"}
          </small>
        </span>
        <span className={`anchor-dot ${active ? "ready" : ""}`} />
      </button>
      <button
        className="anchor-add"
        onClick={onImport}
        aria-label={`Import ${facing} reference image`}
        title={`Import ${facing} reference image`}
      >
        <Plus size={17} />
      </button>
    </div>
  );
}

function Preview({ project, facing }: { project: Project; facing: Facing }) {
  const { preview, importFromPicker } = useStudio();
  const [zoom, setZoom] = useState(1);
  useEffect(() => setZoom(1), [project.id, facing]);
  const active = project.imports.find(
    (record) => record.id === project.anchors[facing]?.activeImportId,
  );
  return (
    <section className="preview-section">
      <div className="preview-header">
        <div>
          <span className="eyebrow">SINGLE-IMAGE REFERENCE</span>
          <h2>{facing[0].toUpperCase() + facing.slice(1)} reference</h2>
        </div>
        <div className="preview-tools">
          <span className="preview-size">
            {preview
              ? `${preview.width} × ${preview.height}`
              : `${project.runtime.cellWidth} × ${project.runtime.cellHeight} cell`}
          </span>
          <span className="tool-divider" />
          <button
            className="icon-button"
            onClick={() => setZoom((v) => Math.max(0.25, v / 2))}
            disabled={!preview}
            aria-label="Zoom out"
          >
            <ZoomOut size={18} />
          </button>
          <span className="zoom-value">{Math.round(zoom * 100)}%</span>
          <button
            className="icon-button"
            onClick={() => setZoom((v) => Math.min(8, v * 2))}
            disabled={!preview}
            aria-label="Zoom in"
          >
            <ZoomIn size={18} />
          </button>
          <button
            className="icon-button"
            onClick={() => setZoom(1)}
            disabled={!preview}
            aria-label="Actual size"
          >
            <Maximize2 size={17} />
          </button>
        </div>
      </div>
      <div className="checkerboard preview-canvas">
        {preview ? (
          <div className="preview-scroll">
            <img
              src={preview.dataUrl}
              alt={`${facing} reference for ${project.name}`}
              style={{
                width: preview.width * zoom,
                height: preview.height * zoom,
              }}
              draggable={false}
            />
          </div>
        ) : (
          <div className="preview-empty">
            <div className="empty-art">
              <ImagePlus size={36} strokeWidth={1.5} />
            </div>
            <h3>No {facing} reference yet</h3>
            <p>Optional: import one neutral PNG or JPEG. Multi-pose boards belong in the animation workflow.</p>
            <button
              className="button primary"
              onClick={() => importFromPicker(facing)}
            >
              <ImagePlus size={17} /> Import reference image
            </button>
          </div>
        )}
      </div>
      <div className="preview-footer">
        <span>
          <span className={`footer-dot ${active ? "ready" : ""}`} />
          {active ? "Original source · preserved" : "Waiting for source image"}
        </span>
        <span>Nearest-neighbour display</span>
      </div>
    </section>
  );
}

function Inspector({ project, facing }: { project: Project; facing: Facing }) {
  const active = project.imports.find(
    (record) => record.id === project.anchors[facing]?.activeImportId,
  );
  const history = project.imports
    .filter((record) => record.facing === facing)
    .slice()
    .reverse();
  return (
    <aside className="inspector" aria-label="Asset inspector" tabIndex={0}>
      <div className="inspector-heading">
        <span className="eyebrow">ASSET INSPECTOR</span>
        <h2>Properties</h2>
      </div>
      <div className="inspector-section">
        <div className="inspector-title">Runtime contract</div>
        <div className="detail-row">
          <span>Preset</span>
          <strong>{projectLabel(project.preset)}</strong>
        </div>
        <div className="detail-row">
          <span>Cell</span>
          <strong>
            {project.runtime.cellWidth} × {project.runtime.cellHeight}
          </strong>
        </div>
        <div className="detail-row">
          <span>Pivot</span>
          <strong>
            {project.runtime.pivotX}, {project.runtime.pivotY}
          </strong>
        </div>
        {project.runtime.neutralHeightTarget && (
          <div className="detail-row">
            <span>Neutral height</span>
            <strong>{project.runtime.neutralHeightTarget} px</strong>
          </div>
        )}
        {project.preset === "kangi-fight" && (
          <div className="contract-note">
            <ShieldCheck size={16} />
            <span>
              Left presentation mirrors the right source. Production approval
              remains separate.
            </span>
          </div>
        )}
      </div>
      <div className="inspector-section">
        <div className="inspector-title">Active source</div>
        {active ? (
          <>
            <div className="detail-row">
              <span>Filename</span>
              <strong className="truncate" title={active.originalName}>
                {active.originalName}
              </strong>
            </div>
            <div className="detail-row">
              <span>Dimensions</span>
              <strong>
                {active.width} × {active.height}
              </strong>
            </div>
            <div className="detail-row">
              <span>Imported</span>
              <strong>
                {new Date(active.importedAt).toLocaleDateString()}
              </strong>
            </div>
            <div className="hash-box">
              <span>SHA-256</span>
              <code title={active.sha256}>{active.sha256}</code>
            </div>
          </>
        ) : (
          <p className="inspector-muted">No source assigned to this facing.</p>
        )}
      </div>
      <div className="inspector-section workflow-section">
        <div className="inspector-title">Workflow</div>
        <p className="inspector-muted">
          Pixel snap, cleanup, normalization, alignment, preview, and export
          run locally. Each result needs review before it becomes active.
        </p>
        <div className="stage-list">
          {stageLabels.map(([id, label], index) => {
            const state = active?.stages[id] ?? "waiting";
            return (
              <div className="stage-row" key={id}>
                <span className={`stage-indicator ${state}`}>
                  {state === "complete" ? (
                    <Check size={13} />
                  ) : (
                    String(index + 1).padStart(2, "0")
                  )}
                </span>
                <span>{label}</span>
                <small>{state}</small>
              </div>
            );
          })}
        </div>
      </div>
      {history.length > 0 && (
        <div className="inspector-section history-section">
          <div className="inspector-title">
            Import history <span>{history.length}</span>
          </div>
          {history.map((record, index) => (
            <div key={record.id} className="history-item">
              <span
                className={`history-bullet ${index === 0 ? "current" : ""}`}
              />
              <span title={record.originalName}>{record.originalName}</span>
              <small>{index === 0 ? "Active" : "Preserved"}</small>
            </div>
          ))}
        </div>
      )}
    </aside>
  );
}

function BoardInspector({ project, route }: { project: Project; route: "board" | null }) {
  const selectedAnimationId = useStudio(s => s.selectedAnimationId);
  const animation = project.animations.find(item => item.id === selectedAnimationId);
  const board = animation?.boards.find(item => item.id === animation.activeBoardId);
  const next = route === null ? "Choose Pose board or Single reference in the main area." : !animation ? "Create an animation to name this pose sequence." : !board ? "Import a pose board for the selected animation." : !animation.activeExtractionId ? "Detect poses, choose their order, then Apply." : !animation.activeBatchSnapId ? "Snap the raw frames, review, then Apply." : !animation.activeBatchCleanupId ? "Clean and normalize the snapped frames." : "Align, preview, and export the draft animation.";
  return <aside className="inspector" aria-label="Pose-board inspector" tabIndex={0}>
    <div className="inspector-heading"><span className="eyebrow">POSE-BOARD INSPECTOR</span><h2>Where you are</h2></div>
    <div className="inspector-section"><div className="inspector-title">Next action</div><p className="inspector-muted">{next}</p></div>
    <div className="inspector-section"><div className="inspector-title">Selected animation</div>{animation ? <><div className="detail-row"><span>Name</span><strong>{animation.name}</strong></div><div className="detail-row"><span>Facing</span><strong>{animation.facing}</strong></div><div className="detail-row"><span>Raw poses</span><strong>{animation.activeRawFrameIds.length}</strong></div><div className="detail-row"><span>Fixed-cell frames</span><strong>{animation.frames.length}</strong></div></> : <p className="inspector-muted">No animation selected yet.</p>}</div>
    <div className="inspector-section"><div className="inspector-title">Pose-board source</div>{board ? <><div className="detail-row"><span>Filename</span><strong className="truncate" title={board.originalName}>{board.originalName}</strong></div><div className="detail-row"><span>Dimensions</span><strong>{board.width} × {board.height}</strong></div><div className="hash-box"><span>SHA-256</span><code title={board.sha256}>{board.sha256}</code></div></> : <p className="inspector-muted">Import a board after creating or selecting an animation.</p>}</div>
    <div className="inspector-section workflow-section"><div className="inspector-title">Animation workflow</div><div className="stage-list">{boardStageLabels.map(([id, label], index) => { const state = animation?.stages[id] ?? "waiting"; return <div className="stage-row" key={id}><span className={`stage-indicator ${state}`}>{state === "complete" ? <Check size={13} /> : String(index + 1).padStart(2, "0")}</span><span>{label}</span><small>{state}</small></div>; })}</div></div>
    <div className="inspector-section"><div className="inspector-title">Runtime contract</div><div className="detail-row"><span>Preset</span><strong>{projectLabel(project.preset)}</strong></div><div className="detail-row"><span>Cell</span><strong>{project.runtime.cellWidth} × {project.runtime.cellHeight}</strong></div><div className="detail-row"><span>Pivot</span><strong>{project.runtime.pivotX}, {project.runtime.pivotY}</strong></div>{project.preset === "kangi-fight" && <p className="inspector-muted">Draft output stays in Sprite Studio. Production approval is separate.</p>}</div>
  </aside>;
}

function WorkspaceSetup() {
  const chooseWorkspace = useStudio((s) => s.chooseWorkspace);
  return (
    <div className="setup-view">
      <div className="setup-visual">
        <StudioMark />
        <div className="setup-grid" />
      </div>
      <span className="eyebrow">LOCAL-FIRST CHARACTER WORKSPACE</span>
      <h1>Make room for your sprites.</h1>
      <p>
        Choose a folder where Sprite Studio will keep character projects and
        untouched source images. Nothing is uploaded.
      </p>
      <button className="button primary large" onClick={chooseWorkspace}>
        <FolderOpen size={18} /> Choose workspace folder
      </button>
      <div className="setup-fineprint">
        <ShieldCheck size={16} /> Your files stay on this computer.
      </div>
    </div>
  );
}
function EmptyWorkspace({ onNew }: { onNew: () => void }) {
  return (
    <div className="empty-workspace">
      <div className="empty-workspace-icon">
        <Sparkles size={28} />
      </div>
      <span className="eyebrow">READY TO BEGIN</span>
      <h1>Your next character starts here.</h1>
      <p>
        Create a project to organize its pose boards or reference images and preserve every
        imported original.
      </p>
      <button className="button primary large" onClick={onNew}>
        <Plus size={18} /> New character
      </button>
    </div>
  );
}

function App() {
  const {
    boot,
    settings,
    project,
    facing,
    error,
    clearMessage,
    busy,
    handoffDropArmed,
    importFromPicker,
    selectFacing,
  } = useStudio();
  const [createOpen, setCreateOpen] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [workspaceRoute, setWorkspaceRoute] = useState<"board" | "reference" | null>(null);
  const routeRef = useRef<"board" | "reference" | null>(null);
  useEffect(() => {
    const route = project?.animations.length ? "board" : project?.imports.length ? "reference" : null;
    setWorkspaceRoute(route);
    routeRef.current = route;
  }, [project?.id]);
  const navigateRoute = (route: "board" | "reference") => {
    routeRef.current = route;
    setWorkspaceRoute(route);
    window.requestAnimationFrame(() => {
      document.getElementById(route === "board" ? "animation-workspace" : "reference-workspace")?.scrollIntoView({ behavior: "smooth", block: "start" });
    });
  };
  useEffect(() => {
    void boot();
  }, [boot]);
  useEffect(() => {
    if (!isTauri()) return;
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "enter" || event.payload.type === "over")
          setDragging(true);
        if (event.payload.type === "leave") setDragging(false);
        if (event.payload.type === "drop") {
          setDragging(false);
          const path = event.payload.paths[0];
          const state = useStudio.getState();
          if (!path) return;
          if (state.handoffDropArmed) {
            useStudio.setState({ handoffDropArmed: false, pendingHandoffPaths: event.payload.paths });
            return;
          }
          if (routeRef.current === "board") {
            if (state.selectedAnimationId) void state.importPoseBoardPath(path);
            else useStudio.setState({ error: "Create an animation first, then drop its pose board here." });
          } else if (routeRef.current === "reference" && state.facing) {
            void state.importPath(path, state.facing);
          } else {
            useStudio.setState({ error: "Choose Pose board or Single reference before dropping an image." });
          }
        }
      })
      .then((dispose) => {
        unlisten = dispose;
      })
      .catch((error) => useStudio.setState({ error: String(error) }));
    return () => {
      unlisten?.();
    };
  }, []);
  const activeFacing = facing ?? project?.runtime.sourceFacings[0] ?? null;
  return (
    <div className="app-shell">
      {settings?.workspaceRoot ? (
        <>
          <Sidebar onNew={() => setCreateOpen(true)} />
          {project && activeFacing ? (
            <>
              <main className="main-workspace">
                <header className="topbar">
                  <div className="breadcrumbs">
                    <span>Characters</span>
                    <span className="breadcrumb-separator">/</span>
                    <strong>{project.name}</strong>
                    <span className="preset-badge">
                      {projectLabel(project.preset)}
                    </span>
                  </div>
                  <div className="topbar-status">
                    <span className="live-dot" /> Workspace saved locally
                  </div>
                </header>
                <div className="workspace-content">
                  <div className="page-heading">
                    <div>
                      <span className="eyebrow">CHARACTER PROJECT</span>
                      <h1>{project.name}</h1>
                      <p>
                        Choose what you have. A pose board and a single reference image are different starting points.
                      </p>
                    </div>
                  </div>
                  <section className="start-routes" aria-label="Choose a starting point">
                    <button className={`start-route ${workspaceRoute === "board" ? "active" : ""}`} aria-pressed={workspaceRoute === "board"} onClick={() => navigateRoute("board")}>
                      <span className="start-route-icon"><Layers3 size={21} /></span><span><strong>Pose board → animation</strong><small>One image with several poses. Create or select an animation, import the board, then recover its frames. No anchor is required.</small><em>Open pose-board workflow <ArrowRight size={15} /></em></span>
                    </button>
                    <button className={`start-route ${workspaceRoute === "reference" ? "active" : ""}`} aria-pressed={workspaceRoute === "reference"} onClick={() => navigateRoute("reference")}>
                      <span className="start-route-icon"><ImagePlus size={21} /></span><span><strong>Single reference image</strong><small>One neutral image for a facing. Use it to establish character identity—not as a multi-frame pose board.</small><em>Open reference workflow <ArrowRight size={15} /></em></span>
                    </button>
                  </section>
                  {workspaceRoute === "reference" && <section id="reference-workspace" className="reference-workspace" aria-label="Single-image reference workflow">
                    <div className="anchor-heading"><div><h2>Single-image references</h2><p>Optional neutral images by facing. Import a pose board under “Pose board → animation” instead.</p></div><span>{project.runtime.sourceFacings.filter(f => project.anchors[f]?.activeImportId).length} of {project.runtime.sourceFacings.length} references</span></div>
                    <WorkflowSettingsPanel project={project} />
                    <WorkflowTip label="Source image · suggested">Start with one neutral, full-body image on flat #00FF00 chroma, or magenta when green conflicts with the character. A 1024 × 1024 square gives grid recovery room to work; it is a generation suggestion, not the runtime size. Keep generous margins, a simple silhouette, no smoothing, and no action-only effects. For another facing, use the applied snapped reference as the identity input, then snap that new facing too.</WorkflowTip>
                    <div className="anchor-grid">
                      {project.runtime.sourceFacings.map((item) => <AnchorCard key={item} facing={item} project={project} selected={activeFacing === item} onSelect={() => selectFacing(item)} onImport={() => importFromPicker(item)} />)}
                      {project.preset === "kangi-fight" && <div className="mirror-card"><span className="anchor-icon"><ArrowLeft size={17} /></span><span><strong>Left</strong><small>Shown by mirroring right; no second image needed</small></span><span className="mirror-label">MIRROR</span></div>}
                    </div>
                    <Preview project={project} facing={activeFacing} />
                    <SnapStage project={project} facing={activeFacing} />
                    <CleanupStage project={project} facing={activeFacing} />
                    <ReferenceExportPanel project={project} facing={activeFacing} />
                  </section>}
                  {workspaceRoute === "board" && <AnimationWorkspace project={project} facing={activeFacing} />}
                </div>
              </main>
              {workspaceRoute === "reference" ? <Inspector project={project} facing={activeFacing} /> : <BoardInspector project={project} route={workspaceRoute} />}
            </>
          ) : (
            <main className="main-workspace">
              <header className="topbar">
                <div className="breadcrumbs">
                  <strong>Characters</strong>
                </div>
                <div className="topbar-status">
                  <span className="live-dot" /> Workspace saved locally
                </div>
              </header>
              <EmptyWorkspace onNew={() => setCreateOpen(true)} />
            </main>
          )}
        </>
      ) : (
        <WorkspaceSetup />
      )}
      <footer className="status-bar">
        <span>
          <span className="status-square" /> SPRITE STUDIO{" "}
          <span className="status-divider">/</span> LOCAL WORKFLOW
        </span>
        <span className="status-credit">Created by Afnan · YouTube: 2Clouds</span>
        <span>
          {busy
            ? "Working…"
            : project
              ? `${project.runtime.cellWidth} × ${project.runtime.cellHeight} CELL · ${project.preset === "kangi-fight" ? "RIGHT SOURCE" : "4 SOURCE DIRECTIONS"}`
              : "LOCAL PROJECTS"}
        </span>
      </footer>
      {error && (
        <div className="message-banner error" role="alert">
          <CircleAlert size={17} />
          <span>{error}</span>
          <button
            className="icon-button"
            onClick={clearMessage}
            aria-label="Dismiss message"
          >
            <X size={16} />
          </button>
        </div>
      )}
      {dragging && project && (
        <div className="drop-overlay" aria-label={handoffDropArmed ? "Drop upscaled PNGs" : "Drop image to import"}>
          <div>
            <ImagePlus size={42} />
            <h2>{handoffDropArmed ? "Drop upscaled PNGs" : workspaceRoute === "board" ? "Drop pose board" : workspaceRoute === "reference" ? `Drop ${activeFacing} reference` : "Choose a starting point first"}</h2>
            <p>{handoffDropArmed ? "You’ll confirm which imported file belongs to each snapped frame." : workspaceRoute === "board" ? "The board will be copied into the selected animation." : workspaceRoute === "reference" ? "The reference will be copied into this project." : "Choose Pose board or Single reference above."}</p>
          </div>
        </div>
      )}
      <CreateProjectDialog
        open={createOpen}
        onClose={() => setCreateOpen(false)}
      />
    </div>
  );
}
export default App;
