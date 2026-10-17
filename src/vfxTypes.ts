import type { AssetPreview } from "./types";
export interface VfxPalette {
  name: string;
  authorityId: string | null;
  packageReference: string | null;
  colors: string[];
  enforce: boolean;
}
export interface VfxProfile {
  cell: [number, number];
  anchor: [number, number];
  sourceOrientation: string;
  mirrorPolicy: string;
  looping: boolean;
  tickHz: number;
  hitStopPolicy: "pause" | "continue";
  completion: "hide" | "empty_terminal";
  allowEmptyTerminal: boolean;
  palette: VfxPalette;
}
export interface VfxProcessing {
  offset: [number, number];
  pixelPitch: number;
  alphaThreshold: number | null;
  chroma: string | null;
  tolerance: number;
  remapPalette: boolean;
}
export interface VfxDraft {
  profile: VfxProfile;
  settings: VfxProcessing[];
  holds: number[];
  baseRevisionId: string | null;
  importId: string | null;
}
export interface VfxReport {
  size: [number, number];
  opaquePixels: number;
  partialAlphaPixels: number;
  transparentRgbPixels: number;
  unexpectedColors: string[];
  clippedPixels: number;
  edgePixels: number;
  possibleGreenFringePixels: number;
  bounds: [number, number, number, number] | null;
  errors: string[];
  warnings: string[];
}
export interface VfxSource {
  id: string;
  path: string;
  sha256: string;
  originalName: string;
  size: [number, number];
  importedAt: string;
}
export interface VfxRegion {
  sourceId: string;
  rect: [number, number, number, number];
}
export interface VfxImport {
  id: string;
  regions: VfxRegion[];
  approvedBy: string | null;
  approvedAt: string | null;
  createdAt: string;
}
export interface VfxFrame {
  id: string;
  sourceId: string;
  rect: [number, number, number, number];
  path: string;
  sha256: string;
  processing: VfxProcessing;
  holdTicks: number;
  report: VfxReport;
}
export interface VfxChecks {
  artwork: boolean;
  regionsAndAnchor: boolean;
  palette: boolean;
  timing: boolean;
  hitStopAndCompletion: boolean;
}
export interface VfxRevision {
  id: string;
  importId: string;
  profile: VfxProfile;
  frames: VfxFrame[];
  approval: {
    reviewer: string;
    approvedAt: string;
    revisionHash: string;
    checks: VfxChecks;
  } | null;
  createdAt: string;
}
export interface VfxExport {
  id: string;
  path: string;
  pngHash: string;
  manifestHash: string;
  sourceManifestHash: string;
  revisionId: string;
  exportedAt: string;
}
export interface VfxEffect {
  schemaVersion: 1;
  id: string;
  projectId: string;
  effectId: string;
  name: string;
  package: string;
  draftProfile: VfxProfile;
  sources: VfxSource[];
  imports: VfxImport[];
  pendingImportId: string | null;
  activeImportId: string | null;
  revisions: VfxRevision[];
  activeRevisionId: string | null;
  exports: VfxExport[];
  generation: number;
  draft?: VfxDraft | null;
}
export interface VfxLibrary {
  projects: {
    id: string;
    name: string;
    effects: {
      id: string;
      name: string;
      effectId: string;
      package: string;
      status: string;
      recoveryNotice: string | null;
    }[];
  }[];
  failures: string[];
}
export interface VfxReply {
  effect: VfxEffect;
  recoveryNotice: string | null;
  crops: AssetPreview[];
  originals: { sourceId: string; image: AssetPreview }[];
  frames: AssetPreview[];
  result: unknown;
}
export type VfxRequest =
  | { operation: "save_draft"; draft: VfxDraft }
  | { operation: "library" }
  | { operation: "create_project"; name: string }
  | {
      operation: "create_effect";
      effect_id: string;
      name: string;
      package: string;
      profile: VfxProfile | null;
    }
  | { operation: "load" }
  | {
      operation: "import_frames";
      paths: string[];
      board_count: number | null;
      rectangles: [number, number, number, number][] | null;
    }
  | { operation: "set_regions"; regions: VfxRegion[] }
  | { operation: "approve_import"; import_id: string; reviewer: string }
  | {
      operation: "process";
      profile: VfxProfile;
      settings: VfxProcessing[];
      holds: number[];
    }
  | {
      operation: "approve";
      revision_id: string;
      reviewer: string;
      checks: VfxChecks;
    }
  | { operation: "restore"; revision_id: string }
  | { operation: "export"; revision_id: string; gutter: number }
  | { operation: "verify_export" | "open_export"; export_id: string };
