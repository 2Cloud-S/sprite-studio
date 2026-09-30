import { create } from "zustand";
import { isTauri } from "@tauri-apps/api/core";
import { desktop } from "../services/desktop";
import { latestCleanupReview, latestSnapReview } from "../domain/snap";
import type {
  AssetPreview,
  Facing,
  Preset,
  Project,
  ProjectSummary,
  Settings,
  SnapOptions,
  CleanupOptions,
  FrameOffset,
  ExportOptions,
  ExtractionOptions,
  ManualCrop,
  WorkflowSettings,
} from "../types";

interface StudioStore {
  settings: Settings | null;
  projects: ProjectSummary[];
  deletedProjects: ProjectSummary[];
  project: Project | null;
  facing: Facing | null;
  preview: AssetPreview | null;
  snapNative: AssetPreview | null;
  snapReference: AssetPreview | null;
  activeSnapPreview: AssetPreview | null;
  cleanedPreview: AssetPreview | null;
  normalizedPreview: AssetPreview | null;
  referenceExportFolder: string | null;
  selectedAnimationId: string | null;
  framePreviews: Record<string, AssetPreview>;
  exportSheet: AssetPreview | null;
  exportGif: AssetPreview | null;
  exportManifest: Record<string, unknown> | null;
  poseBoardPreview: AssetPreview | null;
  extractionCandidates: Record<string, AssetPreview>;
  rawFramePreviews: Record<string, AssetPreview>;
  batchSnapPreviews: Record<string, AssetPreview>;
  batchNormalizedPreviews: Record<string, AssetPreview>;
  batchCleanedPreviews: Record<string, AssetPreview>;
  upscaledPreviews: Record<string, AssetPreview>;
  handoffFolder: string | null;
  handoffDropArmed: boolean;
  pendingHandoffPaths: string[];
  busy: boolean;
  snapBusy: boolean;
  cleanupBusy: boolean;
  error: string | null;
  boot: () => Promise<void>;
  chooseWorkspace: () => Promise<void>;
  openWorkspaceFolder: () => Promise<void>;
  openProjectExportsFolder: () => Promise<void>;
  createProject: (name: string, preset: Preset) => Promise<boolean>;
  deleteProject: (id: string) => Promise<boolean>;
  restoreProject: (id: string) => Promise<void>;
  openProject: (id: string) => Promise<void>;
  selectFacing: (facing: Facing) => Promise<void>;
  importPath: (path: string, facing: Facing) => Promise<void>;
  importFromPicker: (facing: Facing) => Promise<void>;
  chooseSnapper: () => Promise<void>;
  runSnap: (options: SnapOptions) => Promise<void>;
  applySnap: (reviewId: string) => Promise<void>;
  choosePython: () => Promise<void>;
  runCleanup: (options: CleanupOptions) => Promise<void>;
  applyCleanup: (reviewId: string) => Promise<void>;
  exportReference: () => Promise<void>;
  openReferenceExportFolder: () => Promise<void>;
  createAnimation: (name: string, facing: Facing) => Promise<void>;
  selectAnimation: (id: string) => Promise<void>;
  importAnimationFrames: () => Promise<void>;
  proposeAlignment: (offsets: FrameOffset[]) => Promise<void>;
  applyAlignment: (reviewId: string) => Promise<void>;
  proposeAnimationPreview: (fps: number, looping: boolean) => Promise<void>;
  applyAnimationPreview: (reviewId: string) => Promise<void>;
  runExport: (options: ExportOptions) => Promise<void>;
  applyExport: (reviewId: string) => Promise<void>;
  openAnimationExportFolder: (reviewId: string) => Promise<void>;
  importPoseBoard: () => Promise<void>;
  importPoseBoardPath: (path: string) => Promise<void>;
  runPoseExtraction: (options: ExtractionOptions) => Promise<void>;
  applyPoseExtraction: (reviewId: string, orderedBoxIds: string[], manualCrops: ManualCrop[]) => Promise<void>;
  confirmNativeReview: (acceptedFrameIds: string[]) => Promise<void>;
  replaceRawFrame: (frameId: string, crop?: ManualCrop) => Promise<void>;
  runBatchSnap: (options: SnapOptions) => Promise<void>;
  applyBatchSnap: (reviewId: string) => Promise<void>;
  runBatchCleanup: (options: CleanupOptions) => Promise<void>;
  applyBatchCleanup: (reviewId: string) => Promise<void>;
  approveBatchClean: (reviewId: string) => Promise<void>;
  updateWorkflow: (workflow: WorkflowSettings) => Promise<void>;
  updateRuntime: (cellWidth: number, cellHeight: number, pivotX: number, pivotY: number) => Promise<void>;
  importUpscaledFrame: (sourceFrameId: string, path?: string) => Promise<boolean>;
  approveUpscale: () => Promise<void>;
  exportSnapped: (sourceFrameIds: string[]) => Promise<void>;
  openSnapFolder: () => Promise<void>;
  clearMessage: () => void;
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function selectedPreviews(project: Project, facing: Facing) {
  const id = project.anchors[facing]?.activeImportId;
  const review = latestSnapReview(project, facing);
  const activeSnapId = project.anchors[facing]?.activeSnapId;
  const cleanup = latestCleanupReview(project, facing);
  const [preview, snapNative, snapReference, activeSnapPreview, cleanedPreview, normalizedPreview, referenceExportFolder] = await Promise.all([
    id ? desktop.preview(project.id, id) : null,
    review ? desktop.snapPreview(project.id, review.id, "native") : null,
    review ? desktop.snapPreview(project.id, review.id, "reference") : null,
    activeSnapId ? desktop.snapPreview(project.id, activeSnapId, "native") : null,
    cleanup ? desktop.cleanupPreview(project.id, cleanup.id, "cleaned") : null,
    cleanup ? desktop.cleanupPreview(project.id, cleanup.id, "normalized") : null,
    project.anchors[facing]?.activeCleanupId ? desktop.referenceExportFolder(project.id, facing).catch(() => null) : null,
  ]);
  return { preview, snapNative, snapReference, activeSnapPreview, cleanedPreview, normalizedPreview, referenceExportFolder };
}

async function animationPreviews(project: Project, animationId: string): Promise<Record<string, AssetPreview>> {
  const animation = project.animations.find(item => item.id === animationId);
  if (!animation) return {};
  const entries = await Promise.all(animation.frames.map(async frame => [
    frame.id, await desktop.animationFramePreview(project.id, animationId, frame.id),
  ] as const));
  return Object.fromEntries(entries);
}

async function exportPreviews(project: Project, animationId: string) {
  const animation = project.animations.find(item => item.id === animationId);
  const review = animation?.exports.slice().reverse().find(item => item.alignmentId === animation.activeAlignmentId && item.previewId === animation.activePreviewId);
  if (!review) return { exportSheet: null, exportGif: null, exportManifest: null };
  const [exportSheet, exportGif, exportManifest] = await Promise.all([
    desktop.exportAsset(project.id, animationId, review.id, "sheet"),
    desktop.exportAsset(project.id, animationId, review.id, "gif"),
    desktop.exportManifest(project.id, animationId, review.id),
  ]);
  return { exportSheet, exportGif, exportManifest };
}

async function poseBoardPreviews(project: Project, animationId: string) {
  const animation = project.animations.find(item => item.id === animationId);
  const boardId = animation?.activeBoardId;
  const review = animation?.extractions.slice().reverse().find(item => item.boardImportId === boardId);
  const [poseBoardPreview, candidates, raw] = await Promise.all([
    boardId ? desktop.poseBoardPreview(project.id, animationId, boardId) : null,
    review ? Promise.all(review.boxes.map(async box => [box.id, await desktop.extractionCandidate(project.id, animationId, review.id, box.id)] as const)) : [],
    animation ? Promise.all(animation.activeRawFrameIds.map(async id => [id, await desktop.rawAnimationFrame(project.id, animationId, id)] as const)) : [],
  ]);
  return { poseBoardPreview, extractionCandidates: Object.fromEntries(candidates), rawFramePreviews: Object.fromEntries(raw) };
}

async function batchPreviews(project: Project, animationId: string) {
  const animation = project.animations.find(item => item.id === animationId);
  const snap = animation?.batchSnaps.slice().reverse().find(item => item.extractionId === animation.activeExtractionId && item.sourceFrameIds.join() === animation.activeRawFrameIds.join());
  const cleanup = animation?.batchCleanups.slice().reverse().find(item => item.snapReviewId === animation.activeBatchSnapId);
  const [snapped, cleaned, normalized, upscaled] = await Promise.all([
    snap ? Promise.all(snap.frames.map(async item => [item.sourceFrameId, await desktop.batchFramePreview(project.id, animationId, snap.id, item.sourceFrameId, "snapped")] as const)) : [],
    cleanup ? Promise.all(cleanup.frames.map(async item => [item.sourceFrameId, await desktop.batchFramePreview(project.id, animationId, cleanup.id, item.sourceFrameId, "cleaned")] as const)) : [],
    cleanup ? Promise.all(cleanup.frames.map(async item => [item.sourceFrameId, await desktop.batchFramePreview(project.id, animationId, cleanup.id, item.sourceFrameId, "normalized")] as const)) : [],
    animation ? Promise.all(Object.entries(animation.activeUpscaledFrameIds).map(async ([sourceId, upscaleId]) => [sourceId, await desktop.upscaledPreview(project.id, animationId, upscaleId)] as const)) : [],
  ]);
  return { batchSnapPreviews: Object.fromEntries(snapped), batchCleanedPreviews: Object.fromEntries(cleaned), batchNormalizedPreviews: Object.fromEntries(normalized), upscaledPreviews: Object.fromEntries(upscaled) };
}

export const useStudio = create<StudioStore>((set, get) => ({
  settings: null,
  projects: [],
  deletedProjects: [],
  project: null,
  facing: null,
  preview: null,
  snapNative: null,
  snapReference: null,
  activeSnapPreview: null,
  referenceExportFolder: null,
  cleanedPreview: null,
  normalizedPreview: null,
  selectedAnimationId: null,
  framePreviews: {},
  exportSheet: null,
  exportGif: null,
  exportManifest: null,
  poseBoardPreview: null,
  extractionCandidates: {},
  rawFramePreviews: {},
  batchSnapPreviews: {},
  batchNormalizedPreviews: {},
  batchCleanedPreviews: {},
  upscaledPreviews: {},
  handoffFolder: null,
  handoffDropArmed: false,
  pendingHandoffPaths: [],
  busy: true,
  snapBusy: false,
  cleanupBusy: false,
  error: null,
  boot: async () => {
    if (!isTauri()) {
      set({ busy: false });
      return;
    }
    set({ busy: true, error: null });
    try {
      const state = await desktop.appState();
      const projects = state.settings.workspaceRoot
        ? await desktop.listProjects()
        : [];
      const deletedProjects = state.settings.workspaceRoot
        ? await desktop.listDeletedProjects()
        : [];
      const project = state.lastProject;
      const facing = project?.runtime.sourceFacings[0] ?? null;
      const selectedAnimationId = project?.animations[0]?.id ?? null;
      set({
        settings: state.settings,
        projects,
        deletedProjects,
        project,
        facing,
        selectedAnimationId,
        framePreviews: {},
        exportSheet: null, exportGif: null, exportManifest: null,
        poseBoardPreview: null, extractionCandidates: {}, rawFramePreviews: {}, batchSnapPreviews: {}, batchNormalizedPreviews: {},
        busy: false,
        error: state.startupError,
      });
      if (project && facing) {
        try {
          set(await selectedPreviews(project, facing));
        } catch (error) {
          set({ error: message(error) });
        }
      }
      if (project && selectedAnimationId) {
        try { set({ framePreviews: await animationPreviews(project, selectedAnimationId) }); }
        catch (error) { set({ error: message(error) }); }
        try { set(await exportPreviews(project, selectedAnimationId)); }
        catch (error) { set({ error: message(error) }); }
        try { set(await poseBoardPreviews(project, selectedAnimationId)); }
        catch (error) { set({ error: message(error) }); }
        try { set(await batchPreviews(project, selectedAnimationId)); }
        catch (error) { set({ error: message(error) }); }
      }
    } catch (error) {
      set({ busy: false, error: message(error) });
    }
  },
  chooseWorkspace: async () => {
    try {
      const path = await desktop.chooseWorkspace();
      if (typeof path !== "string") return;
      set({ busy: true, error: null });
      const settings = await desktop.setWorkspace(path);
      set({
        settings,
        projects: [],
        deletedProjects: [],
        project: null,
        facing: null,
        preview: null,
        snapNative: null,
        snapReference: null,
        activeSnapPreview: null,
        cleanedPreview: null,
        normalizedPreview: null,
        referenceExportFolder: null,
        selectedAnimationId: null,
        framePreviews: {},
        exportSheet: null, exportGif: null, exportManifest: null,
        poseBoardPreview: null, extractionCandidates: {}, rawFramePreviews: {}, batchSnapPreviews: {}, batchNormalizedPreviews: {},
        busy: false,
      });
    } catch (error) {
      set({ busy: false, error: message(error) });
    }
  },
  openWorkspaceFolder: async () => {
    try { await desktop.openWorkspaceFolder(); }
    catch (error) { set({ error: message(error) }); }
  },
  openProjectExportsFolder: async () => {
    const project = get().project;
    if (!project) return;
    try { await desktop.openProjectExportsFolder(project.id); }
    catch (error) { set({ error: message(error) }); }
  },
  createProject: async (name, preset) => {
    set({ busy: true, error: null });
    try {
      const project = await desktop.createProject(name, preset);
      const projects = await desktop.listProjects();
      set({
        project,
        projects,
        facing: project.runtime.sourceFacings[0],
        preview: null,
        snapNative: null,
        snapReference: null,
        activeSnapPreview: null,
        cleanedPreview: null,
        normalizedPreview: null,
        referenceExportFolder: null,
        selectedAnimationId: null,
        framePreviews: {},
        exportSheet: null, exportGif: null, exportManifest: null,
        poseBoardPreview: null, extractionCandidates: {}, rawFramePreviews: {}, batchSnapPreviews: {}, batchNormalizedPreviews: {},
        busy: false,
      });
      return true;
    } catch (error) {
      set({ busy: false, error: message(error) });
      return false;
    }
  },
  deleteProject: async (id) => {
    if (get().busy || !get().projects.some(item => item.id === id)) return false;
    set({ busy: true, error: null });
    try {
      const settings = await desktop.deleteProject(id);
      const [projects, deletedProjects] = await Promise.all([desktop.listProjects(), desktop.listDeletedProjects()]);
      const wasSelected = get().project?.id === id;
      set({
        settings, projects, deletedProjects, busy: false,
        ...(wasSelected ? { project: null, facing: null, selectedAnimationId: null, preview: null, snapNative: null, snapReference: null, activeSnapPreview: null, cleanedPreview: null, normalizedPreview: null, referenceExportFolder: null, framePreviews: {}, exportSheet: null, exportGif: null, exportManifest: null, poseBoardPreview: null, extractionCandidates: {}, rawFramePreviews: {}, batchSnapPreviews: {}, batchNormalizedPreviews: {} } : {}),
      });
      return true;
    } catch (error) { set({ busy: false, error: message(error) }); return false; }
  },
  restoreProject: async (id) => {
    if (get().busy || !get().deletedProjects.some(item => item.id === id)) return;
    set({ busy: true, error: null });
    try {
      await desktop.restoreProject(id);
      const [projects, deletedProjects] = await Promise.all([desktop.listProjects(), desktop.listDeletedProjects()]);
      set({ projects, deletedProjects, busy: false });
      await get().openProject(id);
    } catch (error) { set({ busy: false, error: message(error) }); }
  },
  openProject: async (id) => {
    set({ busy: true, error: null, preview: null, snapNative: null, snapReference: null, activeSnapPreview: null, cleanedPreview: null, normalizedPreview: null, referenceExportFolder: null, selectedAnimationId: null, framePreviews: {}, exportSheet: null, exportGif: null, exportManifest: null, poseBoardPreview: null, extractionCandidates: {}, rawFramePreviews: {}, batchSnapPreviews: {}, batchNormalizedPreviews: {} });
    try {
      const project = await desktop.openProject(id);
      const facing = project.runtime.sourceFacings[0];
      const selectedAnimationId = project.animations[0]?.id ?? null;
      set({ project, facing, selectedAnimationId, busy: false });
      if (facing) {
        const previews = await selectedPreviews(project, facing);
        if (get().project?.id === project.id && get().facing === facing)
          set(previews);
      }
      if (selectedAnimationId) {
        const frames = await animationPreviews(project, selectedAnimationId);
        if (get().project?.id === project.id && get().selectedAnimationId === selectedAnimationId) set({ framePreviews: frames });
        const exports = await exportPreviews(project, selectedAnimationId);
        if (get().project?.id === project.id && get().selectedAnimationId === selectedAnimationId) set(exports);
        const board = await poseBoardPreviews(project, selectedAnimationId);
        if (get().project?.id === project.id && get().selectedAnimationId === selectedAnimationId) set(board);
        const batch = await batchPreviews(project, selectedAnimationId);
        if (get().project?.id === project.id && get().selectedAnimationId === selectedAnimationId) set(batch);
      }
    } catch (error) {
      set({ busy: false, error: message(error) });
    }
  },
  selectFacing: async (facing) => {
    const project = get().project;
    if (!project) return;
    set({ facing, preview: null, snapNative: null, snapReference: null, activeSnapPreview: null, cleanedPreview: null, normalizedPreview: null, referenceExportFolder: null, error: null });
    try {
      const previews = await selectedPreviews(project, facing);
      if (get().project?.id === project.id && get().facing === facing)
        set(previews);
    } catch (error) {
      set({ error: message(error) });
    }
  },
  importPath: async (path, facing) => {
    const project = get().project;
    if (!project || get().busy) return;
    set({ busy: true, error: null });
    try {
      const updated = await desktop.importAnchor(project.id, facing, path);
      const projects = await desktop.listProjects();
      set({
        project: updated,
        projects,
        facing,
        preview: null,
        snapNative: null,
        snapReference: null,
        activeSnapPreview: null,
        cleanedPreview: null,
        normalizedPreview: null,
        referenceExportFolder: null,
        busy: false,
      });
      try {
        const previews = await selectedPreviews(updated, facing);
        if (get().project?.id === updated.id && get().facing === facing)
          set(previews);
      } catch (error) {
        set({ error: message(error) });
      }
    } catch (error) {
      set({ busy: false, error: message(error) });
    }
  },
  importFromPicker: async (facing) => {
    try {
      const path = await desktop.chooseImage();
      if (typeof path === "string") await get().importPath(path, facing);
    } catch (error) {
      set({ error: message(error) });
    }
  },
  chooseSnapper: async () => {
    try {
      const path = await desktop.chooseSnapper();
      if (typeof path !== "string") return;
      set({ busy: true, error: null });
      const settings = await desktop.configureSnapper(path);
      set({ settings, busy: false });
    } catch (error) {
      set({ busy: false, error: message(error) });
    }
  },
  runSnap: async (options) => {
    const project = get().project;
    const facing = get().facing;
    if (!project || !facing || get().busy) return;
    set({ busy: true, snapBusy: true, error: null });
    try {
      const updated = await desktop.runSnap(project.id, facing, options);
      set({ project: updated, busy: false, snapBusy: false });
      const previews = await selectedPreviews(updated, facing);
      if (get().project?.id === updated.id && get().facing === facing) set(previews);
    } catch (error) {
      try {
        const refreshed = await desktop.openProject(project.id);
        if (get().project?.id === project.id) set({ project: refreshed });
      } catch { /* Keep the last known project while showing the original failure. */ }
      set({ busy: false, snapBusy: false, error: message(error) });
    }
  },
  applySnap: async (reviewId) => {
    const project = get().project;
    if (!project || get().busy) return;
    set({ busy: true, error: null });
    try {
      const updated = await desktop.applySnap(project.id, reviewId);
      set({ project: updated, busy: false, cleanedPreview: null, normalizedPreview: null });
      const facing = get().facing;
      if (facing) {
        const previews = await selectedPreviews(updated, facing);
        if (get().project?.id === updated.id && get().facing === facing) set(previews);
      }
    } catch (error) {
      set({ busy: false, error: message(error) });
    }
  },
  choosePython: async () => {
    try {
      const path = await desktop.choosePython();
      if (typeof path !== "string") return;
      set({ busy: true, error: null });
      const settings = await desktop.configurePython(path);
      set({ settings, busy: false });
    } catch (error) {
      set({ busy: false, error: message(error) });
    }
  },
  runCleanup: async (options) => {
    const project = get().project;
    const facing = get().facing;
    if (!project || !facing || get().busy) return;
    set({ busy: true, cleanupBusy: true, error: null });
    try {
      const updated = await desktop.runCleanup(project.id, facing, options);
      set({ project: updated, busy: false, cleanupBusy: false });
      const previews = await selectedPreviews(updated, facing);
      if (get().project?.id === updated.id && get().facing === facing) set(previews);
    } catch (error) {
      try {
        const refreshed = await desktop.openProject(project.id);
        if (get().project?.id === project.id) set({ project: refreshed });
      } catch { /* Preserve last known project while showing original failure. */ }
      set({ busy: false, cleanupBusy: false, error: message(error) });
    }
  },
  applyCleanup: async (reviewId) => {
    const project = get().project;
    if (!project || get().busy) return;
    set({ busy: true, error: null });
    try {
      const updated = await desktop.applyCleanup(project.id, reviewId);
      const facing = get().facing;
      const referenceExportFolder = facing ? await desktop.referenceExportFolder(project.id, facing).catch(() => null) : null;
      set({ project: updated, referenceExportFolder, busy: false });
    } catch (error) {
      set({ busy: false, error: message(error) });
    }
  },
  exportReference: async () => {
    const project = get().project;
    const facing = get().facing;
    if (!project || !facing || get().busy) return;
    set({ busy: true, error: null });
    try {
      const referenceExportFolder = await desktop.exportReference(project.id, facing);
      if (get().project?.id === project.id && get().facing === facing) set({ referenceExportFolder });
      set({ busy: false });
    } catch (error) { set({ busy: false, error: message(error) }); }
  },
  openReferenceExportFolder: async () => {
    const project = get().project;
    const facing = get().facing;
    if (!project || !facing) return;
    try { await desktop.openReferenceExportFolder(project.id, facing); }
    catch (error) { set({ error: message(error) }); }
  },
  createAnimation: async (name, facing) => {
    const project = get().project;
    if (!project || get().busy) return;
    set({ busy: true, error: null });
    try {
      const updated = await desktop.createAnimation(project.id, name, facing);
      const id = updated.animations[updated.animations.length - 1]?.id ?? null;
      set({ project: updated, selectedAnimationId: id, framePreviews: {}, exportSheet: null, exportGif: null, exportManifest: null, poseBoardPreview: null, extractionCandidates: {}, rawFramePreviews: {}, batchSnapPreviews: {}, batchNormalizedPreviews: {}, busy: false });
    } catch (error) { set({ busy: false, error: message(error) }); }
  },
  selectAnimation: async (id) => {
    const project = get().project;
    if (!project || !project.animations.some(item => item.id === id)) return;
    set({ selectedAnimationId: id, framePreviews: {}, exportSheet: null, exportGif: null, exportManifest: null, poseBoardPreview: null, extractionCandidates: {}, rawFramePreviews: {}, batchSnapPreviews: {}, batchNormalizedPreviews: {}, error: null });
    try {
      const frames = await animationPreviews(project, id);
      if (get().project?.id === project.id && get().selectedAnimationId === id) set({ framePreviews: frames });
      const exports = await exportPreviews(project, id);
      if (get().project?.id === project.id && get().selectedAnimationId === id) set(exports);
      const board = await poseBoardPreviews(project, id);
      if (get().project?.id === project.id && get().selectedAnimationId === id) set(board);
      const batch = await batchPreviews(project, id);
      if (get().project?.id === project.id && get().selectedAnimationId === id) set(batch);
    } catch (error) { set({ error: message(error) }); }
  },
  importAnimationFrames: async () => {
    const project = get().project;
    const id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    try {
      const paths = await desktop.chooseAnimationFrames();
      if (!Array.isArray(paths) || paths.length === 0) return;
      set({ busy: true, error: null });
      const updated = await desktop.importAnimationFrames(project.id, id, paths);
      set({ project: updated, framePreviews: {}, exportSheet: null, exportGif: null, exportManifest: null, busy: false });
      const frames = await animationPreviews(updated, id);
      if (get().project?.id === project.id && get().selectedAnimationId === id) set({ framePreviews: frames });
    } catch (error) { set({ busy: false, error: message(error) }); }
  },
  proposeAlignment: async (offsets) => {
    const project = get().project;
    const id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    set({ busy: true, error: null });
    try { set({ project: await desktop.proposeAlignment(project.id, id, offsets), busy: false }); }
    catch (error) { set({ busy: false, error: message(error) }); }
  },
  applyAlignment: async (reviewId) => {
    const project = get().project;
    const id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    set({ busy: true, error: null });
    try { set({ project: await desktop.applyAlignment(project.id, id, reviewId), exportSheet: null, exportGif: null, exportManifest: null, busy: false }); }
    catch (error) { set({ busy: false, error: message(error) }); }
  },
  proposeAnimationPreview: async (fps, looping) => {
    const project = get().project;
    const id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    set({ busy: true, error: null });
    try { set({ project: await desktop.proposeAnimationPreview(project.id, id, fps, looping), busy: false }); }
    catch (error) { set({ busy: false, error: message(error) }); }
  },
  applyAnimationPreview: async (reviewId) => {
    const project = get().project;
    const id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    set({ busy: true, error: null });
    try { set({ project: await desktop.applyAnimationPreview(project.id, id, reviewId), exportSheet: null, exportGif: null, exportManifest: null, busy: false }); }
    catch (error) { set({ busy: false, error: message(error) }); }
  },
  runExport: async (options) => {
    const project = get().project;
    const id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    set({ busy: true, error: null });
    try {
      const updated = await desktop.runExport(project.id, id, options);
      set({ project: updated, busy: false, exportSheet: null, exportGif: null, exportManifest: null });
      const previews = await exportPreviews(updated, id);
      if (get().project?.id === project.id && get().selectedAnimationId === id) set(previews);
    } catch (error) {
      try { const refreshed = await desktop.openProject(project.id); if (get().project?.id === project.id) set({ project: refreshed }); }
      catch { /* Preserve the original export error. */ }
      set({ busy: false, error: message(error) });
    }
  },
  applyExport: async (reviewId) => {
    const project = get().project;
    const id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    set({ busy: true, error: null });
    try { set({ project: await desktop.applyExport(project.id, id, reviewId), busy: false }); }
    catch (error) { set({ busy: false, error: message(error) }); }
  },
  openAnimationExportFolder: async (reviewId) => {
    const project = get().project;
    const animationId = get().selectedAnimationId;
    if (!project || !animationId) return;
    try { await desktop.openAnimationExportFolder(project.id, animationId, reviewId); }
    catch (error) { set({ error: message(error) }); }
  },
  importPoseBoard: async () => {
    const project = get().project, id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    try {
      const path = await desktop.choosePoseBoard();
      if (typeof path !== "string") return;
      await get().importPoseBoardPath(path);
    } catch (error) { set({ busy: false, error: message(error) }); }
  },
  importPoseBoardPath: async (path) => {
    const project = get().project, id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    set({ busy: true, error: null });
    try {
      const updated = await desktop.importPoseBoard(project.id, id, path);
      set({ project: updated, poseBoardPreview: null, extractionCandidates: {}, rawFramePreviews: {}, batchSnapPreviews: {}, batchNormalizedPreviews: {}, busy: false });
      const previews = await poseBoardPreviews(updated, id);
      if (get().project?.id === project.id && get().selectedAnimationId === id) set(previews);
    } catch (error) { set({ busy: false, error: message(error) }); }
  },
  runPoseExtraction: async (options) => {
    const project = get().project, id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    set({ busy: true, error: null });
    try {
      const updated = await desktop.runPoseExtraction(project.id, id, options);
      set({ project: updated, extractionCandidates: {}, busy: false });
      const previews = await poseBoardPreviews(updated, id);
      if (get().project?.id === project.id && get().selectedAnimationId === id) set(previews);
    } catch (error) {
      try { const refreshed = await desktop.openProject(project.id); if (get().project?.id === project.id) set({ project: refreshed }); }
      catch { /* Show the original extraction error. */ }
      set({ busy: false, error: message(error) });
    }
  },
  applyPoseExtraction: async (reviewId, orderedBoxIds, manualCrops) => {
    const project = get().project, id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    set({ busy: true, error: null });
    try {
      const updated = await desktop.applyPoseExtraction(project.id, id, reviewId, orderedBoxIds, manualCrops);
      set({ project: updated, batchSnapPreviews: {}, batchNormalizedPreviews: {}, busy: false });
      const previews = await poseBoardPreviews(updated, id);
      if (get().project?.id === project.id && get().selectedAnimationId === id) set(previews);
    } catch (error) { set({ busy: false, error: message(error) }); }
  },
  confirmNativeReview: async (acceptedFrameIds) => {
    const project = get().project, id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    set({ busy: true, error: null });
    try { set({ project: await desktop.confirmNativeReview(project.id, id, acceptedFrameIds), busy: false }); }
    catch (error) { set({ busy: false, error: message(error) }); }
  },
  replaceRawFrame: async (frameId, crop) => {
    const project = get().project, id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    try {
      const path = crop ? null : await desktop.chooseImage();
      if (!crop && typeof path !== "string") return;
      set({ busy: true, error: null });
      const updated = await desktop.replaceRawFrame(project.id, id, frameId, typeof path === "string" ? path : null, crop ?? null);
      set({ project: updated, batchSnapPreviews: {}, batchCleanedPreviews: {}, batchNormalizedPreviews: {}, upscaledPreviews: {}, framePreviews: {}, exportSheet: null, exportGif: null, exportManifest: null, busy: false });
      const previews = await poseBoardPreviews(updated, id);
      if (get().project?.id === project.id && get().selectedAnimationId === id) set(previews);
    } catch (error) { set({ busy: false, error: message(error) }); }
  },
  runBatchSnap: async (options) => {
    const project = get().project, id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    set({ busy: true, error: null });
    try {
      const updated = await desktop.runBatchSnap(project.id, id, options);
      set({ project: updated, batchSnapPreviews: {}, busy: false });
      const previews = await batchPreviews(updated, id);
      if (get().project?.id === project.id && get().selectedAnimationId === id) set(previews);
    } catch (error) {
      try { const refreshed = await desktop.openProject(project.id); if (get().project?.id === project.id) set({ project: refreshed }); } catch { /* Keep original error. */ }
      set({ busy: false, error: message(error) });
    }
  },
  applyBatchSnap: async (reviewId) => {
    const project = get().project, id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    set({ busy: true, error: null });
    try { set({ project: await desktop.applyBatchSnap(project.id, id, reviewId), batchNormalizedPreviews: {}, busy: false }); }
    catch (error) { set({ busy: false, error: message(error) }); }
  },
  runBatchCleanup: async (options) => {
    const project = get().project, id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    set({ busy: true, error: null });
    try {
      const updated = await desktop.runBatchCleanup(project.id, id, options);
      set({ project: updated, batchNormalizedPreviews: {}, busy: false });
      const previews = await batchPreviews(updated, id);
      if (get().project?.id === project.id && get().selectedAnimationId === id) set(previews);
    } catch (error) {
      try { const refreshed = await desktop.openProject(project.id); if (get().project?.id === project.id) set({ project: refreshed }); } catch { /* Keep original error. */ }
      set({ busy: false, error: message(error) });
    }
  },
  applyBatchCleanup: async (reviewId) => {
    const project = get().project, id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    set({ busy: true, error: null });
    try {
      const updated = await desktop.applyBatchCleanup(project.id, id, reviewId);
      set({ project: updated, exportSheet: null, exportGif: null, exportManifest: null, busy: false });
      const frames = await animationPreviews(updated, id);
      if (get().project?.id === project.id && get().selectedAnimationId === id) set({ framePreviews: frames });
    } catch (error) { set({ busy: false, error: message(error) }); }
  },
  approveBatchClean: async (reviewId) => {
    const project = get().project, id = get().selectedAnimationId;
    if (!project || !id || get().busy) return;
    set({ busy: true, error: null });
    try { set({ project: await desktop.approveBatchClean(project.id, id, reviewId), busy: false }); }
    catch (error) { set({ busy: false, error: message(error) }); }
  },
  updateWorkflow: async (workflow) => {
    const project = get().project;
    if (!project || get().busy) return;
    set({ busy: true, error: null });
    try { set({ project: await desktop.updateWorkflow(project.id, workflow), busy: false }); }
    catch (error) { set({ busy: false, error: message(error) }); }
  },
  updateRuntime: async (cellWidth, cellHeight, pivotX, pivotY) => {
    const project = get().project;
    if (!project || get().busy) return;
    set({ busy: true, error: null });
    try { set({ project: await desktop.updateRuntime(project.id, cellWidth, cellHeight, pivotX, pivotY), busy: false }); }
    catch (error) { set({ busy: false, error: message(error) }); }
  },
  importUpscaledFrame: async (sourceFrameId, path) => {
    const project = get().project, animationId = get().selectedAnimationId;
    if (!project || !animationId || get().busy) return false;
    try {
      const source = path ?? await desktop.chooseUpscaledFrame();
      if (typeof source !== "string") return false;
      set({ busy: true, error: null });
      const updated = await desktop.importUpscaledFrame(project.id, animationId, sourceFrameId, source);
      set({ project: updated, busy: false, batchNormalizedPreviews: {}, framePreviews: {}, exportSheet: null, exportGif: null, exportManifest: null });
      const previews = await batchPreviews(updated, animationId);
      if (get().project?.id === project.id && get().selectedAnimationId === animationId) set(previews);
      return true;
    } catch (error) { set({ busy: false, error: message(error) }); return false; }
  },
  approveUpscale: async () => {
    const project = get().project, animationId = get().selectedAnimationId;
    if (!project || !animationId || get().busy) return;
    set({ busy: true, error: null });
    try { set({ project: await desktop.approveUpscale(project.id, animationId), busy: false }); }
    catch (error) { set({ busy: false, error: message(error) }); }
  },
  exportSnapped: async (sourceFrameIds) => {
    const project = get().project, animationId = get().selectedAnimationId;
    if (!project || !animationId || get().busy) return;
    try {
      const destination = await desktop.chooseExportFolder();
      if (typeof destination !== "string") return;
      set({ busy: true, error: null });
      const handoffFolder = await desktop.exportSnapped(project.id, animationId, destination, sourceFrameIds);
      set({ handoffFolder, busy: false });
    } catch (error) { set({ busy: false, error: message(error) }); }
  },
  openSnapFolder: async () => {
    const project = get().project, animationId = get().selectedAnimationId;
    if (!project || !animationId) return;
    try { await desktop.openSnapFolder(project.id, animationId); }
    catch (error) { set({ error: message(error) }); }
  },
  clearMessage: () => set({ error: null }),
}));
