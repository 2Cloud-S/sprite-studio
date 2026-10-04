import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AppState,
  AssetPreview,
  Preset,
  Project,
  ProjectSummary,
  Settings,
  SnapOptions,
  CleanupOptions,
  ExportOptions,
  ExtractionOptions,
  ManualCrop,
  WorkflowSettings,
} from "../types";

export const desktop = {
  appState: () => invoke<AppState>("get_app_state"),
  listProjects: () => invoke<ProjectSummary[]>("list_projects"),
  listDeletedProjects: () => invoke<ProjectSummary[]>("list_deleted_projects"),
  deleteProject: (projectId: string) => invoke<Settings>("delete_project", { projectId }),
  restoreProject: (projectId: string) => invoke<Project>("restore_project", { projectId }),
  setWorkspace: (path: string) => invoke<Settings>("set_workspace", { path }),
  createProject: (name: string, preset: Preset) =>
    invoke<Project>("create_project", { name, preset }),
  renameProject: (projectId: string, name: string) =>
    invoke<Project>("rename_project", { projectId, name }),
  openProject: (projectId: string) =>
    invoke<Project>("open_project", { projectId }),
  updateWorkflow: (projectId: string, workflow: WorkflowSettings) => invoke<Project>("update_workflow", { projectId, workflow }),
  updateRuntime: (projectId: string, cellWidth: number, cellHeight: number, pivotX: number, pivotY: number, anchorMode: WorkflowSettings["anchorMode"]) => invoke<Project>("update_runtime", { projectId, cellWidth, cellHeight, pivotX, pivotY, anchorMode }),
  updateRuntimePolicy: (projectId: string, geometryPolicy: Project["runtime"]["geometryPolicy"], neutralHeightTarget: number | null, neutralTolerancePx: number) => invoke<Project>("update_runtime_policy", { projectId, geometryPolicy, neutralHeightTarget, neutralTolerancePx }),
  confirmNativeReview: (projectId: string, animationId: string, acceptedFrameIds: string[]) => invoke<Project>("confirm_native_review", { projectId, animationId, acceptedFrameIds }),
  replaceRawFrame: (projectId: string, animationId: string, frameId: string, sourcePath: string | null, crop: ManualCrop | null) => invoke<Project>("replace_raw_frame", { projectId, animationId, frameId, sourcePath, crop }),
  importUpscaledFrame: (projectId: string, animationId: string, sourceFrameId: string, sourcePath: string) => invoke<Project>("import_upscaled_frame", { projectId, animationId, sourceFrameId, sourcePath }),
  approveUpscale: (projectId: string, animationId: string) => invoke<Project>("approve_upscale", { projectId, animationId }),
  exportSnapped: (projectId: string, animationId: string, destinationPath: string, sourceFrameIds: string[]) => invoke<string>("export_snapped", { projectId, animationId, destinationPath, sourceFrameIds }),
  openSnapFolder: (projectId: string, animationId: string) => invoke<void>("open_snap_folder", { projectId, animationId }),
  openWorkspaceFolder: () => invoke<string>("open_workspace_folder"),
  openProjectExportsFolder: (projectId: string) => invoke<string>("open_project_exports_folder", { projectId }),
  exportReference: (projectId: string, facing: string) => invoke<string>("export_reference", { projectId, facing }),
  referenceExportFolder: (projectId: string, facing: string) => invoke<string | null>("reference_export_folder", { projectId, facing }),
  openReferenceExportFolder: (projectId: string, facing: string) => invoke<string>("open_reference_export_folder", { projectId, facing }),
  openAnimationExportFolder: (projectId: string, animationId: string, reviewId: string) => invoke<string>("open_animation_export_folder", { projectId, animationId, reviewId }),
  upscaledPreview: (projectId: string, animationId: string, upscaleId: string) => invoke<AssetPreview>("read_upscaled_preview", { projectId, animationId, upscaleId }),
  chooseUpscaledFrame: () => open({ directory: false, multiple: false, title: "Import upscaled PNG", filters: [{ name: "PNG image", extensions: ["png"] }] }),
  chooseUpscaledFrames: () => open({ directory: false, multiple: true, title: "Import upscaled PNGs", filters: [{ name: "PNG image", extensions: ["png"] }] }),
  chooseExportFolder: () => open({ directory: true, multiple: false, title: "Export snapped frames for external upscale" }),
  importAnchor: (projectId: string, facing: string, sourcePath: string) =>
    invoke<Project>("import_anchor", { projectId, facing, sourcePath }),
  preview: (projectId: string, importId: string) =>
    invoke<AssetPreview>("read_asset_preview", { projectId, importId }),
  configureSnapper: (path: string) => invoke<Settings>("configure_snapper", { path }),
  runSnap: (projectId: string, facing: string, options: SnapOptions) =>
    invoke<Project>("run_snap", { projectId, facing, options }),
  runAutoFit: (projectId: string, facing: string, cleanupOptions: CleanupOptions) =>
    invoke<Project>("run_auto_fit", { projectId, facing, cleanupOptions }),
  applySnap: (projectId: string, reviewId: string) =>
    invoke<Project>("apply_snap", { projectId, reviewId }),
  snapPreview: (projectId: string, reviewId: string, kind: "native" | "reference") =>
    invoke<AssetPreview>("read_snap_preview", { projectId, reviewId, kind }),
  configurePython: (path: string) => invoke<Settings>("configure_python", { path }),
  runCleanup: (projectId: string, facing: string, options: CleanupOptions) =>
    invoke<Project>("run_cleanup", { projectId, facing, options }),
  applyCleanup: (projectId: string, reviewId: string) =>
    invoke<Project>("apply_cleanup", { projectId, reviewId }),
  cleanupPreview: (projectId: string, reviewId: string, kind: "cleaned" | "normalized") =>
    invoke<AssetPreview>("read_cleanup_preview", { projectId, reviewId, kind }),
  createAnimation: (projectId: string, name: string, facing: string) =>
    invoke<Project>("create_animation", { projectId, name, facing }),
  renameAnimation: (projectId: string, animationId: string, name: string) =>
    invoke<Project>("rename_animation", { projectId, animationId, name }),
  importAnimationFrames: (projectId: string, animationId: string, paths: string[]) =>
    invoke<Project>("import_animation_frames", { projectId, animationId, paths }),
  animationFramePreview: (projectId: string, animationId: string, frameId: string) =>
    invoke<AssetPreview>("read_animation_frame_preview", { projectId, animationId, frameId }),
  proposeAlignment: (projectId: string, animationId: string, offsets: import("../types").FrameOffset[]) =>
    invoke<Project>("propose_alignment", { projectId, animationId, offsets }),
  applyAlignment: (projectId: string, animationId: string, reviewId: string) =>
    invoke<Project>("apply_alignment", { projectId, animationId, reviewId }),
  proposeAnimationPreview: (projectId: string, animationId: string, fps: number, looping: boolean) =>
    invoke<Project>("propose_animation_preview", { projectId, animationId, fps, looping }),
  applyAnimationPreview: (projectId: string, animationId: string, reviewId: string) =>
    invoke<Project>("apply_animation_preview", { projectId, animationId, reviewId }),
  runExport: (projectId: string, animationId: string, options: ExportOptions) =>
    invoke<Project>("run_export", { projectId, animationId, options }),
  applyExport: (projectId: string, animationId: string, reviewId: string) =>
    invoke<Project>("apply_export", { projectId, animationId, reviewId }),
  exportAsset: (projectId: string, animationId: string, reviewId: string, kind: "sheet" | "gif") =>
    invoke<AssetPreview>("read_export_asset", { projectId, animationId, reviewId, kind }),
  exportManifest: (projectId: string, animationId: string, reviewId: string) =>
    invoke<Record<string, unknown>>("read_export_manifest", { projectId, animationId, reviewId }),
  importPoseBoard: (projectId: string, animationId: string, sourcePath: string) =>
    invoke<Project>("import_pose_board", { projectId, animationId, sourcePath }),
  poseBoardPreview: (projectId: string, animationId: string, boardId: string) =>
    invoke<AssetPreview>("read_pose_board_preview", { projectId, animationId, boardId }),
  runPoseExtraction: (projectId: string, animationId: string, options: ExtractionOptions) =>
    invoke<Project>("run_pose_extraction", { projectId, animationId, options }),
  extractionCandidate: (projectId: string, animationId: string, reviewId: string, boxId: string) =>
    invoke<AssetPreview>("read_extraction_candidate", { projectId, animationId, reviewId, boxId }),
  applyPoseExtraction: (projectId: string, animationId: string, reviewId: string, orderedBoxIds: string[], manualCrops: ManualCrop[]) =>
    invoke<Project>("apply_pose_extraction", { projectId, animationId, reviewId, orderedBoxIds, manualCrops }),
  rawAnimationFrame: (projectId: string, animationId: string, frameId: string) =>
    invoke<AssetPreview>("read_raw_animation_frame", { projectId, animationId, frameId }),
  runBatchSnap: (projectId: string, animationId: string, options: SnapOptions) =>
    invoke<Project>("run_batch_snap", { projectId, animationId, options }),
  runBatchAutoFit: (projectId: string, animationId: string, cleanupOptions: CleanupOptions) =>
    invoke<Project>("run_batch_auto_fit", { projectId, animationId, cleanupOptions }),
  applyBatchSnap: (projectId: string, animationId: string, reviewId: string) =>
    invoke<Project>("apply_batch_snap", { projectId, animationId, reviewId }),
  runBatchCleanup: (projectId: string, animationId: string, options: CleanupOptions) =>
    invoke<Project>("run_batch_cleanup", { projectId, animationId, options }),
  applyBatchCleanup: (projectId: string, animationId: string, reviewId: string) =>
    invoke<Project>("apply_batch_cleanup", { projectId, animationId, reviewId }),
  approveBatchClean: (projectId: string, animationId: string, reviewId: string) => invoke<Project>("approve_batch_clean", { projectId, animationId, reviewId }),
  batchFramePreview: (projectId: string, animationId: string, reviewId: string, sourceFrameId: string, kind: "snapped" | "cleaned" | "normalized") =>
    invoke<AssetPreview>("read_batch_frame_preview", { projectId, animationId, reviewId, sourceFrameId, kind }),
  choosePoseBoard: () => open({ directory: false, multiple: false, title: "Import animation pose board",
    filters: [{ name: "PNG or JPEG image", extensions: ["png", "jpg", "jpeg"] }] }),
  chooseAnimationFrames: () => open({
    directory: false, multiple: true, title: "Import normalized animation frames",
    filters: [{ name: "PNG image", extensions: ["png"] }],
  }),
  choosePython: () => open({
    directory: false, multiple: false, title: "Choose Python with Pillow and OpenCV",
    filters: [{ name: "Python executable", extensions: ["exe"] }],
  }),
  chooseSnapper: () =>
    open({
      directory: false,
      multiple: false,
      title: "Locate Sprite Fusion Pixel Snapper",
      filters: [{ name: "Windows executable", extensions: ["exe"] }],
    }),
  chooseWorkspace: () =>
    open({
      directory: true,
      multiple: false,
      title: "Choose Sprite Studio workspace",
    }),
  chooseImage: () =>
    open({
      directory: false,
      multiple: false,
      title: "Import anchor image",
      filters: [
        { name: "PNG or JPEG image", extensions: ["png", "jpg", "jpeg"] },
      ],
    }),
};
