export type Preset = "generic" | "kangi-fight";
export type StageId =
  "raw" | "snap" | "cleanup" | "normalize" | "align" | "preview" | "export";
export type StageState =
  "waiting" | "processing" | "needs_input" | "review" | "complete" | "failed" | "stale";
export type UpscaleMode = "automatic" | "manual-handoff";
export interface WorkflowSettings { kColors: number; chroma: string; upscaleMode: UpscaleMode; sheetColumns: number; sheetRows: number; manualPolishEnabled: boolean; typicalNudgeRangePx: number; greenFringeDespeckle: boolean; anchorMode: "bottom-center" | "custom"; }
export type Facing = "north" | "south" | "east" | "west" | "right";

export interface Settings {
  workspaceRoot: string | null;
  lastProjectId: string | null;
  snapperExecutable: string | null;
  snapperVersion: string | null;
  pythonExecutable: string | null;
  pythonEnvironment: string | null;
}

export interface RuntimeSettings {
  cellWidth: number;
  cellHeight: number;
  pivotX: number;
  pivotY: number;
  neutralHeightTarget: number | null;
  sourceFacings: Facing[];
  mirroredFacings: Record<string, string>;
}

export interface AnchorAssignment {
  activeImportId: string | null;
  importIds: string[];
  activeSnapId: string | null;
  activeCleanupId: string | null;
}

export interface SnapOptions {
  colors: number;
  pixelSize: number | null;
  palette: string | null;
}
export interface SnapOutputPaths { nativePath: string; snappedPath: string | null; upscaledPath: string | null; chromaPath: string | null; mode: "anchor" | "frame"; }

export interface SnapReview {
  id: string;
  facing: Facing;
  sourceImportId: string;
  nativeRelativePath: string;
  nativeSha256: string;
  nativeWidth: number;
  nativeHeight: number;
  referenceRelativePath: string;
  referenceSha256: string;
  referenceWidth: number;
  referenceHeight: number;
  referenceScale: number;
  outputs: SnapOutputPaths | null;
  colors: number;
  pixelSize: number | null;
  palette: string | null;
  toolVersion: string;
  createdAt: string;
  appliedAt: string | null;
}

export interface CleanupOptions {
  background: string;
  tolerance: number;
  minArea: number;
}

export interface CleanupReview {
  id: string;
  facing: Facing;
  sourceImportId: string;
  sourceSnapId: string;
  cleanedRelativePath: string;
  cleanedSha256: string;
  cleanedWidth: number;
  cleanedHeight: number;
  normalizedRelativePath: string;
  normalizedSha256: string;
  normalizedWidth: number;
  normalizedHeight: number;
  backgroundHex: string;
  tolerance: number;
  minArea: number;
  foregroundPixels: number;
  removedSpecklePixels: number;
  placementX: number;
  placementY: number;
  processorVersion: string;
  pythonEnvironment: string;
  createdAt: string;
  appliedAt: string | null;
}

export interface ImportRecord {
  id: string;
  facing: Facing;
  originalName: string;
  relativePath: string;
  sha256: string;
  mimeType: string;
  width: number;
  height: number;
  importedAt: string;
  stages: Record<StageId, StageState>;
}

export interface Project {
  schemaVersion: number;
  id: string;
  name: string;
  slug: string;
  preset: Preset;
  createdAt: string;
  updatedAt: string;
  runtime: RuntimeSettings;
  workflow: WorkflowSettings;
  anchors: Record<string, AnchorAssignment>;
  imports: ImportRecord[];
  snapReviews: SnapReview[];
  cleanupReviews: CleanupReview[];
  animations: Animation[];
}

export interface AnimationFrame {
  id: string;
  originalName: string;
  relativePath: string;
  sha256: string;
  width: number;
  height: number;
  importedAt: string;
}

export interface BoardImport {
  id: string;
  originalName: string;
  relativePath: string;
  sha256: string;
  mimeType: string;
  width: number;
  height: number;
  importedAt: string;
}

export interface DetectedFrame {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  foregroundPixels: number;
  relativePath: string;
  sha256: string;
}

export interface ExtractionReview {
  id: string;
  boardImportId: string;
  boxes: DetectedFrame[];
  backgroundHex: string;
  tolerance: number;
  minArea: number;
  mergeGap: number;
  createdAt: string;
  appliedAt: string | null;
}

export interface RawAnimationFrame {
  id: string;
  extractionId: string;
  relativePath: string;
  sha256: string;
  width: number;
  height: number;
  createdAt: string;
}

export interface BatchFrameArtifact { sourceFrameId: string; relativePath: string; sha256: string; width: number; height: number; upscaledRelativePath: string | null; chromaRelativePath: string | null; outputs: SnapOutputPaths | null; }
export interface UpscaledFrame { id: string; snapReviewId: string; sourceFrameId: string; version: number; originalName: string; relativePath: string; sha256: string; width: number; height: number; importedAt: string; }
export interface BatchSnapReview { id: string; extractionId: string; sourceFrameIds: string[]; frames: BatchFrameArtifact[]; colors: number; pixelSize: number | null; palette: string | null; toolVersion: string; createdAt: string; appliedAt: string | null; }
export interface BatchCleanupFrame { sourceFrameId: string; cleanedRelativePath: string; cleanedSha256: string; normalizedRelativePath: string; normalizedSha256: string; foregroundPixels: number; }
export interface BatchCleanupReview { id: string; snapReviewId: string; upscaleFrameIds: string[]; frames: BatchCleanupFrame[]; backgroundHex: string; tolerance: number; minArea: number; greenFringeDespeckle: boolean; pythonEnvironment: string; createdAt: string; appliedAt: string | null; }

export interface ExtractionOptions {
  background: string;
  tolerance: number;
  minArea: number;
  mergeGap: number;
}

export interface ManualCrop { x: number; y: number; width: number; height: number; }

export interface FrameOffset {
  frameId: string;
  x: number;
  y: number;
}

export interface AlignmentReview {
  id: string;
  offsets: FrameOffset[];
  relativePath: string | null;
  sha256: string | null;
  createdAt: string;
  appliedAt: string | null;
}

export interface AnimationPreviewReview {
  id: string;
  alignmentId: string;
  fps: number;
  looping: boolean;
  createdAt: string;
  appliedAt: string | null;
}

export interface ExportOptions {
  columns: number;
  padding: number;
  spacing: number;
}

export interface ExportReview {
  id: string;
  alignmentId: string;
  previewId: string;
  sheetRelativePath: string;
  sheetSha256: string;
  sheetWidth: number;
  sheetHeight: number;
  gifRelativePath: string;
  gifSha256: string;
  manifestRelativePath: string;
  manifestSha256: string;
  columns: number;
  rows: number;
  padding: number;
  spacing: number;
  gifDurationMs: number;
  gifEncodedFrames: number;
  createdAt: string;
  appliedAt: string | null;
}

export interface Animation {
  id: string;
  name: string;
  facing: Facing;
  frames: AnimationFrame[];
  historicalFrames: AnimationFrame[];
  boards: BoardImport[];
  activeBoardId: string | null;
  extractions: ExtractionReview[];
  activeExtractionId: string | null;
  rawFrames: RawAnimationFrame[];
  activeRawFrameIds: string[];
  batchSnaps: BatchSnapReview[];
  activeBatchSnapId: string | null;
  upscaledFrames: UpscaledFrame[];
  activeUpscaledFrameIds: Record<string, string>;
  upscaleApproved: boolean;
  batchCleanups: BatchCleanupReview[];
  activeBatchCleanupId: string | null;
  alignments: AlignmentReview[];
  previews: AnimationPreviewReview[];
  exports: ExportReview[];
  activeAlignmentId: string | null;
  activePreviewId: string | null;
  activeExportId: string | null;
  stages: Record<"board" | "extract" | "native_review" | "snap" | "upscale" | "cleanup" | "normalize" | "frames" | "align" | "preview" | "export", StageState>;
  createdAt: string;
  updatedAt: string;
}

export interface ProjectSummary {
  id: string;
  name: string;
  preset: Preset;
  updatedAt: string;
  importedCount: number;
}

export interface AppState {
  settings: Settings;
  lastProject: Project | null;
  startupError: string | null;
}

export interface AssetPreview {
  dataUrl: string;
  mimeType: string;
  width: number;
  height: number;
}
