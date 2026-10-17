import type { AssetPreview } from "./types";

export interface EnvironmentParameters {
  resizeToCanvas: boolean;
  pixelPitch: number;
  removeBackground: string | null;
  backgroundTolerance: number;
  binaryAlpha: boolean;
  remapPalette: boolean;
  canvasMode: "preserve" | "stretch" | "fit" | "fill" | "pad";
  offset: [number, number];
}
export type EnvironmentAssetType =
  "background_layer" | "prop" | "tile" | "overlay";
export interface PaletteProfile {
  id: string;
  name: string;
  colors: string[];
  slots: string[];
  enforce: boolean;
}
export interface EnvironmentRules {
  alpha: "opaque" | "binary" | "any";
  groundY: number | null;
  seamlessRequired: boolean;
}
export interface EnvironmentGuides {
  groundY: number | null;
  hudY: number | null;
  markersX: number[];
}
export interface EnvironmentReport {
  width: number;
  height: number;
  origin: [number, number];
  colorCount: number;
  approvedColors: string[];
  unexpectedColors: string[];
  unexpectedColorCount: number;
  opaquePixels: number;
  transparentPixels: number;
  partialAlphaPixels: number;
  palettePass: boolean;
  alphaPass: boolean;
  canvasPass: boolean;
  groundPass: boolean;
  detectedPixelPitch: number | null;
  warnings: string[];
  errors: string[];
  valid: boolean;
  colorUsage: [string, number][];
  edgeMismatches: [number, number];
}
export interface EnvironmentSource {
  id: string;
  originalName: string;
  path: string;
  sha256: string;
  report: EnvironmentReport;
  importedAt: string;
}
export interface EnvironmentRevision {
  id: string;
  sourceId: string;
  sourceHash: string;
  path: string;
  sha256: string;
  parameters: EnvironmentParameters;
  report: EnvironmentReport;
  reviewer: string | null;
  approvedAt: string | null;
  createdAt: string;
}
export interface EnvironmentLayer {
  assetId: string;
  order: number;
  parameters: EnvironmentParameters;
  currentSourceId: string | null;
  currentRevisionId: string | null;
  sources: EnvironmentSource[];
  revisions: EnvironmentRevision[];
  exports: string[];
  exportedRevisionIds: string[];
  name: string;
  assetType: EnvironmentAssetType;
  removed: boolean;
  rules: EnvironmentRules;
  nativeSize: [number, number] | null;
  pivot: [number, number] | null;
  position: [number, number];
  modifiedAt: string;
}
export interface EnvironmentState {
  schemaVersion: number;
  processorVersion: string;
  projectId: string;
  biomeId: string;
  arenaId: string;
  paletteVersion: string;
  canvas: [number, number];
  origin: [number, number];
  layers: EnvironmentLayer[];
  packageId: string;
  displayName: string;
  templateId: string | null;
  palette: PaletteProfile;
  guides: EnvironmentGuides;
  masterReference: {
    path: string;
    sha256: string;
    originalName: string;
    width: number;
    height: number;
  } | null;
  packageExports: string[];
  compositeExports: string[];
  exportHistory: {
    path: string;
    kind: "asset" | "package" | "review_only";
    exportedAt: string;
  }[];
  revision: number;
}
export interface EnvironmentCatalog {
  packages: {
    id: string;
    name: string;
    canvas: [number, number];
    assets: number;
    revision: number;
  }[];
  failures: string[];
}
export interface EnvironmentOwner {
  id: string;
  name: string;
  kind: "environment" | "existing_project";
  catalog: EnvironmentCatalog;
}
export interface EnvironmentLibrary {
  owners: EnvironmentOwner[];
  createdId: string | null;
  failures: string[];
}
export type EnvironmentRequest =
  | { operation: "load" }
  | { operation: "package_history" }
  | {
      operation: "create";
      name: string;
      template: boolean;
      canvas: [number, number];
      palette: PaletteProfile;
      layer_count: number;
    }
  | {
      operation: "configure";
      name: string;
      biome: string;
      arena: string;
      canvas: [number, number];
      palette: PaletteProfile;
      guides: EnvironmentGuides;
    }
  | { operation: "duplicate"; name: string }
  | {
      operation: "add_asset";
      name: string;
      asset_type: EnvironmentAssetType;
      native_size: [number, number] | null;
      rules: EnvironmentRules;
    }
  | {
      operation: "asset_settings";
      order: number;
      name: string;
      rules: EnvironmentRules;
      native_size: [number, number] | null;
      pivot: [number, number] | null;
      position: [number, number];
    }
  | { operation: "remove_asset" | "restore_asset"; order: number }
  | { operation: "reorder"; asset_ids: string[] }
  | { operation: "export_package"; orders: number[] }
  | { operation: "attach_reference"; path: string }
  | { operation: "reference_preview" }
  | { operation: "revision_preview"; order: number; revision: string }
  | { operation: "open_export"; path: string }
  | {
      operation: "review_composite";
      layers: { order: number; revision: string; opacity: number }[];
    }
  | { operation: "import"; order: number; path: string }
  | { operation: "settings"; order: number; parameters: EnvironmentParameters }
  | { operation: "process"; order: number }
  | { operation: "approve"; order: number; reviewer: string }
  | { operation: "export"; order: number }
  | { operation: "preview"; order: number; original: boolean }
  | { operation: "composite"; visible: number[]; export: boolean }
  | { operation: "restore"; order: number; revision: string };
export interface EnvironmentReply {
  environment: EnvironmentState;
  result: string | AssetPreview | null;
  recoveryNotice: string | null;
  catalog: EnvironmentCatalog;
  validation?: EnvironmentReport | null;
  metadataHistory?:
    | {
        snapshot: string;
        name: string;
        canvas: [number, number];
        palette: PaletteProfile;
        guides: EnvironmentGuides;
        biome: string;
        arena: string;
        revision: number;
        assets: number;
      }[]
    | null;
}
