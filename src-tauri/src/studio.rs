use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, Utc};
use image::ImageFormat;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};
use tauri::Manager;
use uuid::Uuid;

const SCHEMA_VERSION: u32 = 1;
const MAX_IMPORT_BYTES: u64 = 32 * 1024 * 1024;
const CLEANUP_SCRIPT: &str = include_str!("../python/cleanup.py");
const EXPORT_SCRIPT: &str = include_str!("../python/export.py");
const EXTRACT_SCRIPT: &str = include_str!("../python/extract_board.py");
const STAGES: [&str; 7] = [
    "raw",
    "snap",
    "cleanup",
    "normalize",
    "align",
    "preview",
    "export",
];
fn default_true() -> bool {
    true
}

type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub workspace_root: Option<String>,
    pub last_project_id: Option<String>,
    #[serde(default)]
    pub snapper_executable: Option<String>,
    #[serde(default)]
    pub snapper_version: Option<String>,
    #[serde(default)]
    pub python_executable: Option<String>,
    #[serde(default)]
    pub python_environment: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            workspace_root: None,
            last_project_id: None,
            snapper_executable: None,
            snapper_version: None,
            python_executable: None,
            python_environment: None,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    settings: Settings,
    last_project: Option<Project>,
    startup_error: Option<String>,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Preset {
    Generic,
    KangiFight,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSettings {
    pub cell_width: u32,
    pub cell_height: u32,
    pub pivot_x: u32,
    pub pivot_y: u32,
    pub neutral_height_target: Option<u32>,
    #[serde(default = "default_neutral_tolerance")]
    pub neutral_tolerance_px: u8,
    #[serde(default)]
    pub geometry_policy: GeometryPolicy,
    pub source_facings: Vec<String>,
    pub mirrored_facings: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GeometryPolicy {
    #[default]
    Flexible,
    Locked,
}

fn default_neutral_tolerance() -> u8 {
    4
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnchorAssignment {
    pub active_import_id: Option<String>,
    pub import_ids: Vec<String>,
    #[serde(default)]
    pub active_snap_id: Option<String>,
    #[serde(default)]
    pub active_cleanup_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StageState {
    Waiting,
    Processing,
    #[serde(rename = "needs_input")]
    NeedsInput,
    Review,
    Complete,
    Failed,
    Stale,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpscaleMode {
    Automatic,
    ManualHandoff,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnchorMode {
    BottomCenter,
    Custom,
}

fn default_anchor_mode() -> AnchorMode {
    AnchorMode::BottomCenter
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSettings {
    pub k_colors: u16,
    pub chroma: String,
    pub upscale_mode: UpscaleMode,
    pub sheet_columns: u16,
    pub sheet_rows: u16,
    pub manual_polish_enabled: bool,
    pub typical_nudge_range_px: u8,
    pub green_fringe_despeckle: bool,
    #[serde(default = "default_anchor_mode")]
    pub anchor_mode: AnchorMode,
}

impl Default for WorkflowSettings {
    fn default() -> Self {
        Self {
            k_colors: 256,
            chroma: "#00FF00".into(),
            upscale_mode: UpscaleMode::Automatic,
            sheet_columns: 5,
            sheet_rows: 2,
            manual_polish_enabled: true,
            typical_nudge_range_px: 2,
            green_fringe_despeckle: true,
            anchor_mode: AnchorMode::BottomCenter,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRecord {
    pub id: String,
    pub facing: String,
    pub original_name: String,
    pub relative_path: String,
    pub sha256: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub imported_at: DateTime<Utc>,
    pub stages: BTreeMap<String, StageState>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapOutputPaths {
    pub native_path: String,
    pub snapped_path: Option<String>,
    pub upscaled_path: Option<String>,
    pub chroma_path: Option<String>,
    pub mode: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapReview {
    pub id: String,
    pub facing: String,
    pub source_import_id: String,
    pub native_relative_path: String,
    pub native_sha256: String,
    pub native_width: u32,
    pub native_height: u32,
    pub reference_relative_path: String,
    pub reference_sha256: String,
    pub reference_width: u32,
    pub reference_height: u32,
    pub reference_scale: u32,
    #[serde(default)]
    pub outputs: Option<SnapOutputPaths>,
    pub colors: u16,
    pub pixel_size: Option<u32>,
    #[serde(default)]
    pub detected_pixel_size: Option<f64>,
    #[serde(default)]
    pub auto_fit_run_id: Option<String>,
    pub palette: Option<String>,
    pub tool_version: String,
    pub created_at: DateTime<Utc>,
    pub applied_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoFitCandidate {
    pub review_id: String,
    pub pixel_size: f64,
    pub foreground_width: u32,
    pub foreground_height: u32,
    pub foreground_pixels: u32,
    pub removed_speckle_pixels: u32,
    pub fits: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoFitRun {
    pub id: String,
    pub facing: String,
    pub source_import_id: String,
    pub cell_width: u32,
    pub cell_height: u32,
    pub pivot_x: u32,
    pub pivot_y: u32,
    pub neutral_height_target: Option<u32>,
    pub neutral_tolerance_px: u8,
    pub starting_pixel_size: f64,
    pub candidates: Vec<AutoFitCandidate>,
    pub recommended_review_ids: Vec<String>,
    pub selected_review_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReview {
    pub id: String,
    pub facing: String,
    pub source_import_id: String,
    pub source_snap_id: String,
    pub cleaned_relative_path: String,
    pub cleaned_sha256: String,
    pub cleaned_width: u32,
    pub cleaned_height: u32,
    pub normalized_relative_path: String,
    pub normalized_sha256: String,
    pub normalized_width: u32,
    pub normalized_height: u32,
    pub background_hex: String,
    pub tolerance: u8,
    pub min_area: u32,
    pub foreground_pixels: u32,
    pub removed_speckle_pixels: u32,
    pub placement_x: u32,
    pub placement_y: u32,
    pub processor_version: String,
    pub python_environment: String,
    pub created_at: DateTime<Utc>,
    pub applied_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub slug: String,
    pub preset: Preset,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub runtime: RuntimeSettings,
    #[serde(default)]
    pub workflow: WorkflowSettings,
    pub anchors: BTreeMap<String, AnchorAssignment>,
    pub imports: Vec<ImportRecord>,
    #[serde(default)]
    pub snap_reviews: Vec<SnapReview>,
    #[serde(default)]
    pub auto_fit_runs: Vec<AutoFitRun>,
    #[serde(default)]
    pub cleanup_reviews: Vec<CleanupReview>,
    #[serde(default)]
    pub animations: Vec<Animation>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationFrame {
    pub id: String,
    pub original_name: String,
    pub relative_path: String,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    pub imported_at: DateTime<Utc>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardImport {
    pub id: String,
    pub original_name: String,
    pub relative_path: String,
    pub sha256: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub imported_at: DateTime<Utc>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedFrame {
    pub id: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub foreground_pixels: u32,
    pub relative_path: String,
    pub sha256: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionReview {
    pub id: String,
    pub board_import_id: String,
    pub boxes: Vec<DetectedFrame>,
    pub background_hex: String,
    pub tolerance: u8,
    pub min_area: u32,
    pub merge_gap: u8,
    pub created_at: DateTime<Utc>,
    pub applied_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawAnimationFrame {
    pub id: String,
    pub extraction_id: String,
    pub relative_path: String,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchFrameArtifact {
    pub source_frame_id: String,
    pub relative_path: String,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub upscaled_relative_path: Option<String>,
    #[serde(default)]
    pub chroma_relative_path: Option<String>,
    #[serde(default)]
    pub outputs: Option<SnapOutputPaths>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpscaledFrame {
    pub id: String,
    pub snap_review_id: String,
    pub source_frame_id: String,
    pub version: u32,
    pub original_name: String,
    pub relative_path: String,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    pub imported_at: DateTime<Utc>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSnapReview {
    pub id: String,
    pub extraction_id: String,
    pub source_frame_ids: Vec<String>,
    pub frames: Vec<BatchFrameArtifact>,
    pub colors: u16,
    pub pixel_size: Option<u32>,
    #[serde(default)]
    pub detected_pixel_size: Option<f64>,
    #[serde(default)]
    pub auto_fit_run_id: Option<String>,
    pub palette: Option<String>,
    pub tool_version: String,
    pub created_at: DateTime<Utc>,
    pub applied_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchAutoFitFrame {
    pub source_frame_id: String,
    pub foreground_width: u32,
    pub foreground_height: u32,
    pub foreground_pixels: u32,
    pub removed_speckle_pixels: u32,
    pub fits: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchAutoFitCandidate {
    pub review_id: String,
    pub pixel_size: f64,
    pub frames: Vec<BatchAutoFitFrame>,
    pub fits: bool,
    pub worst_target_delta: Option<u32>,
    pub total_foreground_pixels: u64,
    pub total_speckle_loss: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchAutoFitRun {
    pub id: String,
    pub extraction_id: String,
    pub source_frame_ids: Vec<String>,
    pub cell_width: u32,
    pub cell_height: u32,
    pub pivot_x: u32,
    pub pivot_y: u32,
    pub neutral_height_target: Option<u32>,
    pub neutral_tolerance_px: u8,
    pub starting_pixel_size: f64,
    pub candidates: Vec<BatchAutoFitCandidate>,
    pub recommended_review_ids: Vec<String>,
    pub selected_review_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCleanupFrame {
    pub source_frame_id: String,
    pub cleaned_relative_path: String,
    pub cleaned_sha256: String,
    pub normalized_relative_path: String,
    pub normalized_sha256: String,
    pub foreground_pixels: u32,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCleanupReview {
    pub id: String,
    pub snap_review_id: String,
    #[serde(default)]
    pub upscale_frame_ids: Vec<String>,
    pub frames: Vec<BatchCleanupFrame>,
    pub background_hex: String,
    pub tolerance: u8,
    pub min_area: u32,
    #[serde(default = "default_true")]
    pub green_fringe_despeckle: bool,
    pub python_environment: String,
    pub created_at: DateTime<Utc>,
    pub applied_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionOptions {
    pub background: String,
    pub tolerance: u8,
    pub min_area: u32,
    pub merge_gap: u8,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualCrop {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DetectedMetrics {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    foreground_pixels: u32,
    filename: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtractionMetrics {
    background_hex: String,
    boxes: Vec<DetectedMetrics>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameOffset {
    pub frame_id: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlignmentReview {
    pub id: String,
    pub offsets: Vec<FrameOffset>,
    #[serde(default)]
    pub relative_path: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
    pub created_at: DateTime<Utc>,
    pub applied_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationPreviewReview {
    pub id: String,
    pub alignment_id: String,
    pub fps: u8,
    pub looping: bool,
    pub created_at: DateTime<Utc>,
    pub applied_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReview {
    pub id: String,
    pub alignment_id: String,
    pub preview_id: String,
    pub sheet_relative_path: String,
    pub sheet_sha256: String,
    pub sheet_width: u32,
    pub sheet_height: u32,
    pub gif_relative_path: String,
    pub gif_sha256: String,
    pub manifest_relative_path: String,
    pub manifest_sha256: String,
    pub columns: u16,
    pub rows: u16,
    pub padding: u16,
    pub spacing: u16,
    pub gif_duration_ms: u32,
    pub gif_encoded_frames: u16,
    pub created_at: DateTime<Utc>,
    pub applied_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Animation {
    pub id: String,
    pub name: String,
    pub facing: String,
    pub frames: Vec<AnimationFrame>,
    #[serde(default)]
    pub historical_frames: Vec<AnimationFrame>,
    #[serde(default)]
    pub boards: Vec<BoardImport>,
    #[serde(default)]
    pub active_board_id: Option<String>,
    #[serde(default)]
    pub extractions: Vec<ExtractionReview>,
    #[serde(default)]
    pub active_extraction_id: Option<String>,
    #[serde(default)]
    pub raw_frames: Vec<RawAnimationFrame>,
    #[serde(default)]
    pub active_raw_frame_ids: Vec<String>,
    #[serde(default)]
    pub batch_snaps: Vec<BatchSnapReview>,
    #[serde(default)]
    pub batch_auto_fit_runs: Vec<BatchAutoFitRun>,
    #[serde(default)]
    pub active_batch_snap_id: Option<String>,
    #[serde(default)]
    pub upscaled_frames: Vec<UpscaledFrame>,
    #[serde(default)]
    pub active_upscaled_frame_ids: BTreeMap<String, String>,
    #[serde(default)]
    pub upscale_approved: bool,
    #[serde(default)]
    pub batch_cleanups: Vec<BatchCleanupReview>,
    #[serde(default)]
    pub active_batch_cleanup_id: Option<String>,
    pub alignments: Vec<AlignmentReview>,
    pub previews: Vec<AnimationPreviewReview>,
    #[serde(default)]
    pub exports: Vec<ExportReview>,
    pub active_alignment_id: Option<String>,
    pub active_preview_id: Option<String>,
    #[serde(default)]
    pub active_export_id: Option<String>,
    pub stages: BTreeMap<String, StageState>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapOptions {
    pub colors: u16,
    pub pixel_size: Option<u32>,
    pub palette: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupOptions {
    pub background: String,
    pub tolerance: u8,
    pub min_area: u32,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    pub columns: u16,
    pub padding: u16,
    pub spacing: u16,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportArtifactKind {
    Sheet,
    Gif,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportMetrics {
    sheet_width: u32,
    sheet_height: u32,
    rows: u16,
    gif_duration_ms: u32,
    gif_frame_count: usize,
    gif_total_duration_ms: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CleanupArtifactKind {
    Cleaned,
    Normalized,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CleanupMetrics {
    processor_version: String,
    background_hex: String,
    foreground_pixels: u32,
    removed_speckle_pixels: u32,
    bbox_width: u32,
    bbox_height: u32,
    placement_x: u32,
    placement_y: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FitMetrics {
    processor_version: String,
    foreground_width: u32,
    foreground_height: u32,
    foreground_pixels: u32,
    removed_speckle_pixels: u32,
    placement_x: i32,
    placement_y: i32,
    fits: bool,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SnapArtifactKind {
    Native,
    Reference,
}

#[derive(Clone, Default)]
pub struct StudioLock(pub Arc<Mutex<()>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    id: String,
    name: String,
    preset: Preset,
    updated_at: DateTime<Utc>,
    imported_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetPreview {
    data_url: String,
    mime_type: String,
    width: u32,
    height: u32,
}

fn settings_file(app: &tauri::AppHandle) -> Result<PathBuf> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create app settings directory: {e}"))?;
    Ok(dir.join("app-settings.json"))
}

fn read_settings(path: &Path) -> Result<Settings> {
    if !path.exists() {
        return Ok(Settings::default());
    }
    let bytes = fs::read(path).map_err(|e| format!("Cannot read app settings: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("App settings are damaged: {e}"))
}

fn atomic_json<T: Serialize>(target: &Path, value: &T) -> Result<()> {
    let parent = target.parent().ok_or("Invalid metadata path")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = parent.join(format!(".{}.tmp", Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| format!("Cannot prepare metadata: {e}"))?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("Cannot write metadata: {e}"))?;
    }
    if target.exists() {
        let backup = parent.join(format!(".{}.backup", Uuid::new_v4()));
        fs::rename(target, &backup).map_err(|e| format!("Cannot replace metadata: {e}"))?;
        if let Err(error) = fs::rename(&temporary, target) {
            let _ = fs::rename(&backup, target);
            let _ = fs::remove_file(&temporary);
            return Err(format!("Cannot replace metadata: {error}"));
        }
        let _ = fs::remove_file(backup);
    } else {
        fs::rename(&temporary, target).map_err(|e| format!("Cannot finalize metadata: {e}"))?;
    }
    Ok(())
}

fn workspace(settings: &Settings) -> Result<PathBuf> {
    let value = settings
        .workspace_root
        .as_ref()
        .ok_or("Choose a workspace folder first")?;
    let path = fs::canonicalize(value)
        .map_err(|_| "Workspace folder is missing or inaccessible".to_string())?;
    if !path.is_dir() {
        return Err("Workspace is not a folder".into());
    }
    Ok(path)
}

fn projects_dir(settings: &Settings) -> Result<PathBuf> {
    let root = workspace(settings)?;
    let projects = root.join("projects");
    if !projects.is_dir() {
        return Err("Workspace projects folder is missing".into());
    }
    let resolved = fs::canonicalize(&projects).map_err(|e| e.to_string())?;
    if !resolved.starts_with(&root) {
        return Err("Projects folder escapes the workspace".into());
    }
    Ok(resolved)
}

fn project_exports_root(project_dir: &Path) -> Result<PathBuf> {
    let path = project_dir.join("exports");
    fs::create_dir_all(&path).map_err(|e| format!("Cannot create export folder: {e}"))?;
    let resolved =
        fs::canonicalize(&path).map_err(|e| format!("Cannot access export folder: {e}"))?;
    if !resolved.starts_with(project_dir) {
        return Err("Export folder escapes the project".into());
    }
    Ok(resolved)
}

fn project_export_dir(project_dir: &Path, relative: &str) -> Result<PathBuf> {
    let root = project_exports_root(project_dir)?;
    let mut resolved = root.clone();
    for part in safe_relative(relative)?.components() {
        let next = resolved.join(part.as_os_str());
        if !next.exists() {
            fs::create_dir(&next).map_err(|e| format!("Cannot create export folder: {e}"))?;
        }
        resolved =
            fs::canonicalize(&next).map_err(|e| format!("Cannot access export folder: {e}"))?;
        if !resolved.is_dir() || !resolved.starts_with(&root) {
            return Err("Export folder escapes the project".into());
        }
    }
    Ok(resolved)
}

fn open_folder(path: &Path) -> Result<()> {
    let folder = fs::canonicalize(path).map_err(|e| format!("Folder is missing: {e}"))?;
    if !folder.is_dir() {
        return Err("Expected a folder".into());
    }
    let shell_path = explorer_path(&folder);
    Command::new("explorer.exe")
        .arg(shell_path)
        .spawn()
        .map_err(|e| format!("Cannot open folder: {e}"))?;
    Ok(())
}

fn explorer_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else if let Some(local) = value.strip_prefix(r"\\?\") {
        local.to_owned()
    } else {
        value.into_owned()
    }
}

fn deleted_projects_dir(settings: &Settings, create: bool) -> Result<Option<PathBuf>> {
    let root = workspace(settings)?;
    let deleted = root.join("deleted-projects");
    if create {
        fs::create_dir_all(&deleted).map_err(|e| format!("Cannot create recovery folder: {e}"))?;
    } else if !deleted.exists() {
        return Ok(None);
    }
    let resolved =
        fs::canonicalize(&deleted).map_err(|e| format!("Cannot open recovery folder: {e}"))?;
    if !resolved.is_dir() || !resolved.starts_with(&root) {
        return Err("Recovery folder must stay inside the workspace".into());
    }
    Ok(Some(resolved))
}

fn list_deleted_projects_in(settings: &Settings) -> Result<Vec<ProjectSummary>> {
    let Some(root) = deleted_projects_dir(settings, false)? else {
        return Ok(vec![]);
    };
    let mut projects = vec![];
    for entry in fs::read_dir(&root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        let dir = fs::canonicalize(entry.path()).map_err(|e| e.to_string())?;
        if !dir.starts_with(&root) {
            continue;
        }
        if let Ok(project) = read_project(&dir) {
            projects.push(ProjectSummary {
                id: project.id,
                name: project.name,
                preset: project.preset,
                updated_at: project.updated_at,
                imported_count: project.imports.len(),
            });
        }
    }
    projects.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(projects)
}

fn archive_project_in(settings_path: &Path, id: &str) -> Result<Settings> {
    let mut settings = read_settings(settings_path)?;
    let (source, _) = find_project(&settings, id)?;
    let deleted_root =
        deleted_projects_dir(&settings, true)?.ok_or("Recovery folder is missing")?;
    let name = source.file_name().ok_or("Project folder name is invalid")?;
    let target = deleted_root.join(name);
    if target.exists() {
        return Err("Recovery folder already contains this character; restore it first".into());
    }
    fs::rename(&source, &target).map_err(|e| format!("Cannot move character to recovery: {e}"))?;
    if settings.last_project_id.as_deref() == Some(id) {
        settings.last_project_id = None;
        if let Err(error) = atomic_json(settings_path, &settings) {
            let _ = fs::rename(&target, &source);
            return Err(error);
        }
    }
    Ok(settings)
}

fn restore_project_in(settings: &Settings, id: &str) -> Result<Project> {
    if Uuid::parse_str(id).is_err() {
        return Err("Invalid project ID".into());
    }
    let deleted_root =
        deleted_projects_dir(settings, false)?.ok_or("No deleted characters to restore")?;
    let projects_root = projects_dir(settings)?;
    for entry in fs::read_dir(&deleted_root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        let source = fs::canonicalize(entry.path()).map_err(|e| e.to_string())?;
        if !source.starts_with(&deleted_root) {
            continue;
        }
        let Ok(project) = read_project(&source) else {
            continue;
        };
        if project.id != id {
            continue;
        }
        let target =
            projects_root.join(source.file_name().ok_or("Project folder name is invalid")?);
        if target.exists() {
            return Err(
                "A character folder with this name already exists; recovery was not changed".into(),
            );
        }
        fs::rename(&source, &target).map_err(|e| format!("Cannot restore character: {e}"))?;
        return Ok(project);
    }
    Err("Deleted character not found".into())
}

fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let trimmed = slug.trim_end_matches('-');
    if trimmed.is_empty() {
        "character".into()
    } else {
        trimmed.chars().take(48).collect()
    }
}

fn preset_settings(preset: Preset) -> (RuntimeSettings, BTreeMap<String, AnchorAssignment>) {
    let (width, height, pivot_x, pivot_y, target, sources, mirrored) = match preset {
        Preset::Generic => (
            256,
            256,
            128,
            255,
            None,
            vec!["north", "south", "east", "west"],
            BTreeMap::new(),
        ),
        Preset::KangiFight => {
            let mut mirror = BTreeMap::new();
            mirror.insert("left".to_string(), "right".to_string());
            (128, 128, 64, 112, Some(64), vec!["right"], mirror)
        }
    };
    let source_facings: Vec<String> = sources.into_iter().map(str::to_string).collect();
    let anchors = source_facings
        .iter()
        .map(|f| {
            (
                f.clone(),
                AnchorAssignment {
                    active_import_id: None,
                    import_ids: vec![],
                    active_snap_id: None,
                    active_cleanup_id: None,
                },
            )
        })
        .collect();
    (
        RuntimeSettings {
            cell_width: width,
            cell_height: height,
            pivot_x,
            pivot_y,
            neutral_height_target: target,
            neutral_tolerance_px: default_neutral_tolerance(),
            geometry_policy: if preset == Preset::KangiFight {
                GeometryPolicy::Locked
            } else {
                GeometryPolicy::Flexible
            },
            source_facings,
            mirrored_facings: mirrored,
        },
        anchors,
    )
}

fn read_project(dir: &Path) -> Result<Project> {
    let bytes =
        fs::read(dir.join("project.json")).map_err(|e| format!("Cannot read project: {e}"))?;
    let mut project: Project =
        serde_json::from_slice(&bytes).map_err(|e| format!("Project metadata is damaged: {e}"))?;
    if project.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "Unsupported project schema version {}",
            project.schema_version
        ));
    }
    if Uuid::parse_str(&project.id).is_err() {
        return Err("Project ID is invalid".into());
    }
    if project.preset == Preset::KangiFight {
        project.runtime.geometry_policy = GeometryPolicy::Locked;
    }
    if project.preset == Preset::Generic
        && project.workflow.anchor_mode == AnchorMode::BottomCenter
        && (project.runtime.pivot_x != project.runtime.cell_width / 2
            || project.runtime.pivot_y != project.runtime.cell_height.saturating_sub(1))
    {
        project.workflow.anchor_mode = AnchorMode::Custom;
    }
    for animation in &mut project.animations {
        animation.stages.entry("native_review".into()).or_insert(
            if animation.active_raw_frame_ids.is_empty() {
                StageState::Waiting
            } else {
                StageState::Complete
            },
        );
        animation.stages.entry("upscale".into()).or_insert(
            if animation.active_batch_snap_id.is_some() {
                StageState::Complete
            } else {
                StageState::Waiting
            },
        );
    }
    for (facing, anchor) in &project.anchors {
        if !project.runtime.source_facings.contains(facing) {
            return Err("Project facing is invalid".into());
        }
        if let Some(active) = &anchor.active_import_id {
            if !anchor.import_ids.contains(active) {
                return Err("Active anchor reference is invalid".into());
            }
        }
        if let Some(active_snap) = &anchor.active_snap_id {
            if !project.snap_reviews.iter().any(|review| {
                &review.id == active_snap
                    && &review.facing == facing
                    && Some(&review.source_import_id) == anchor.active_import_id.as_ref()
            }) {
                return Err("Active snapped anchor reference is invalid".into());
            }
        }
        if let Some(active_cleanup) = &anchor.active_cleanup_id {
            if !project.cleanup_reviews.iter().any(|review| {
                &review.id == active_cleanup
                    && &review.facing == facing
                    && Some(&review.source_import_id) == anchor.active_import_id.as_ref()
                    && Some(&review.source_snap_id) == anchor.active_snap_id.as_ref()
            }) {
                return Err("Active cleanup reference is invalid".into());
            }
        }
    }
    for record in &project.imports {
        safe_relative(&record.relative_path)?;
    }
    for review in &project.snap_reviews {
        safe_relative(&review.native_relative_path)?;
        safe_relative(&review.reference_relative_path)?;
        if let Some(outputs) = &review.outputs {
            validate_snap_outputs(outputs)?;
        }
        if !project
            .imports
            .iter()
            .any(|record| record.id == review.source_import_id && record.facing == review.facing)
        {
            return Err("Snap review source reference is invalid".into());
        }
    }
    for review in &project.cleanup_reviews {
        safe_relative(&review.cleaned_relative_path)?;
        safe_relative(&review.normalized_relative_path)?;
        if !project.snap_reviews.iter().any(|snap| {
            snap.id == review.source_snap_id
                && snap.source_import_id == review.source_import_id
                && snap.facing == review.facing
        }) {
            return Err("Cleanup review source reference is invalid".into());
        }
    }
    for animation in &project.animations {
        if Uuid::parse_str(&animation.id).is_err()
            || !project.runtime.source_facings.contains(&animation.facing)
        {
            return Err("Animation identity or facing is invalid".into());
        }
        for frame in &animation.frames {
            if Uuid::parse_str(&frame.id).is_err() {
                return Err("Animation frame ID is invalid".into());
            }
            safe_relative(&frame.relative_path)?;
        }
        for frame in &animation.historical_frames {
            safe_relative(&frame.relative_path)?;
        }
        for board in &animation.boards {
            safe_relative(&board.relative_path)?;
        }
        if let Some(id) = &animation.active_board_id {
            if !animation.boards.iter().any(|item| &item.id == id) {
                return Err("Active pose-board reference is invalid".into());
            }
        }
        for extraction in &animation.extractions {
            if !animation
                .boards
                .iter()
                .any(|board| board.id == extraction.board_import_id)
            {
                return Err("Extraction board reference is invalid".into());
            }
            for box_item in &extraction.boxes {
                safe_relative(&box_item.relative_path)?;
            }
        }
        if let Some(id) = &animation.active_extraction_id {
            if !animation.extractions.iter().any(|item| {
                &item.id == id && Some(&item.board_import_id) == animation.active_board_id.as_ref()
            }) {
                return Err("Active extraction reference is invalid".into());
            }
        }
        for frame in &animation.raw_frames {
            safe_relative(&frame.relative_path)?;
        }
        for id in &animation.active_raw_frame_ids {
            if !animation.raw_frames.iter().any(|frame| {
                &frame.id == id
                    && Some(&frame.extraction_id) == animation.active_extraction_id.as_ref()
            }) {
                return Err("Active raw frame reference is invalid".into());
            }
        }
        for review in &animation.batch_snaps {
            if !animation
                .extractions
                .iter()
                .any(|item| item.id == review.extraction_id)
            {
                return Err("Batch snap extraction reference is invalid".into());
            }
            for frame in &review.frames {
                safe_relative(&frame.relative_path)?;
                if let Some(path) = &frame.upscaled_relative_path {
                    safe_relative(path)?;
                }
                if let Some(path) = &frame.chroma_relative_path {
                    safe_relative(path)?;
                }
                if let Some(outputs) = &frame.outputs {
                    validate_snap_outputs(outputs)?;
                }
                if !animation
                    .raw_frames
                    .iter()
                    .any(|item| item.id == frame.source_frame_id)
                {
                    return Err("Batch snap source frame reference is invalid".into());
                }
            }
        }
        if let Some(id) = &animation.active_batch_snap_id {
            if !animation.batch_snaps.iter().any(|item| &item.id == id) {
                return Err("Active batch snap reference is invalid".into());
            }
        }
        for frame in &animation.upscaled_frames {
            safe_relative(&frame.relative_path)?;
            if !animation.batch_snaps.iter().any(|review| {
                review.id == frame.snap_review_id
                    && review
                        .frames
                        .iter()
                        .any(|item| item.source_frame_id == frame.source_frame_id)
            }) {
                return Err("Upscaled frame source reference is invalid".into());
            }
        }
        for (source_id, upscale_id) in &animation.active_upscaled_frame_ids {
            if !animation.upscaled_frames.iter().any(|item| {
                &item.id == upscale_id
                    && &item.source_frame_id == source_id
                    && Some(&item.snap_review_id) == animation.active_batch_snap_id.as_ref()
            }) {
                return Err("Active upscaled frame reference is invalid".into());
            }
        }
        for review in &animation.batch_cleanups {
            if !animation
                .batch_snaps
                .iter()
                .any(|item| item.id == review.snap_review_id)
            {
                return Err("Batch cleanup snap reference is invalid".into());
            }
            for frame in &review.frames {
                safe_relative(&frame.cleaned_relative_path)?;
                safe_relative(&frame.normalized_relative_path)?;
            }
            for id in &review.upscale_frame_ids {
                if !animation
                    .upscaled_frames
                    .iter()
                    .any(|item| &item.id == id && item.snap_review_id == review.snap_review_id)
                {
                    return Err("Cleanup upscaled input reference is invalid".into());
                }
            }
        }
        if let Some(id) = &animation.active_batch_cleanup_id {
            if !animation.batch_cleanups.iter().any(|item| &item.id == id) {
                return Err("Active batch cleanup reference is invalid".into());
            }
        }
        if let Some(id) = &animation.active_alignment_id {
            if !animation.alignments.iter().any(|review| &review.id == id) {
                return Err("Active alignment reference is invalid".into());
            }
        }
        for review in &animation.alignments {
            if let Some(path) = &review.relative_path {
                safe_relative(path)?;
            }
        }
        if let Some(id) = &animation.active_preview_id {
            if !animation.previews.iter().any(|review| {
                &review.id == id
                    && Some(&review.alignment_id) == animation.active_alignment_id.as_ref()
            }) {
                return Err("Active animation preview reference is invalid".into());
            }
        }
        if let Some(id) = &animation.active_export_id {
            if !animation.exports.iter().any(|review| {
                &review.id == id
                    && Some(&review.alignment_id) == animation.active_alignment_id.as_ref()
                    && Some(&review.preview_id) == animation.active_preview_id.as_ref()
            }) {
                return Err("Active export reference is invalid".into());
            }
        }
        for export in &animation.exports {
            safe_relative(&export.sheet_relative_path)?;
            safe_relative(&export.gif_relative_path)?;
            safe_relative(&export.manifest_relative_path)?;
        }
    }
    Ok(project)
}

fn find_project(settings: &Settings, id: &str) -> Result<(PathBuf, Project)> {
    if Uuid::parse_str(id).is_err() {
        return Err("Invalid project ID".into());
    }
    let root = projects_dir(settings)?;
    for entry in fs::read_dir(&root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        let dir = fs::canonicalize(entry.path()).map_err(|e| e.to_string())?;
        if !dir.starts_with(&root) {
            continue;
        }
        if let Ok(project) = read_project(&dir) {
            if project.id == id {
                return Ok((dir, project));
            }
        }
    }
    Err("Project not found".into())
}

fn safe_relative(path: &str) -> Result<PathBuf> {
    let value = Path::new(path);
    if value.as_os_str().is_empty()
        || value
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Asset path is not project-relative".into());
    }
    Ok(value.to_path_buf())
}

fn validate_snap_outputs(outputs: &SnapOutputPaths) -> Result<()> {
    if outputs.mode != "anchor" && outputs.mode != "frame" {
        return Err("Snap output mode is invalid".into());
    }
    safe_relative(&outputs.native_path)?;
    for path in [
        &outputs.snapped_path,
        &outputs.upscaled_path,
        &outputs.chroma_path,
    ]
    .into_iter()
    .flatten()
    {
        safe_relative(path)?;
    }
    Ok(())
}

fn store_workspace(settings_path: &Path, selected: &str) -> Result<Settings> {
    let root =
        fs::canonicalize(selected).map_err(|e| format!("Cannot open selected folder: {e}"))?;
    if !root.is_dir() {
        return Err("Choose a folder, not a file".into());
    }
    let projects = root.join("projects");
    fs::create_dir_all(&projects).map_err(|e| format!("Workspace is not writable: {e}"))?;
    let projects_resolved = fs::canonicalize(&projects).map_err(|e| e.to_string())?;
    if !projects_resolved.starts_with(&root) {
        return Err("Projects folder must stay inside the selected workspace".into());
    }
    let probe = projects.join(format!(".write-test-{}", Uuid::new_v4()));
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|e| format!("Workspace is not writable: {e}"))?;
    fs::remove_file(probe).map_err(|e| format!("Workspace write check failed: {e}"))?;
    let mut settings = read_settings(settings_path)?;
    settings.workspace_root = Some(root.to_string_lossy().into_owned());
    settings.last_project_id = None;
    atomic_json(settings_path, &settings)?;
    Ok(settings)
}

fn create_project_in(settings: &Settings, name: &str, preset: Preset) -> Result<Project> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err("Character name must be 1–80 characters".into());
    }
    let id = Uuid::new_v4().to_string();
    let slug = slugify(name);
    let short_id = &id[..8];
    let dir = projects_dir(settings)?.join(format!("{slug}-{short_id}"));
    fs::create_dir(&dir).map_err(|e| format!("Cannot create project: {e}"))?;
    let (runtime, anchors) = preset_settings(preset);
    for facing in &runtime.source_facings {
        fs::create_dir_all(dir.join("anchors").join(facing).join("raw"))
            .map_err(|e| format!("Cannot create anchor folders: {e}"))?;
    }
    let now = Utc::now();
    let project = Project {
        schema_version: SCHEMA_VERSION,
        id,
        name: name.into(),
        slug,
        preset,
        created_at: now,
        updated_at: now,
        runtime,
        workflow: if preset == Preset::KangiFight {
            WorkflowSettings {
                sheet_columns: 1,
                sheet_rows: 1,
                ..WorkflowSettings::default()
            }
        } else {
            WorkflowSettings::default()
        },
        anchors,
        imports: vec![],
        snap_reviews: vec![],
        auto_fit_runs: vec![],
        cleanup_reviews: vec![],
        animations: vec![],
    };
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn rename_project_in(settings: &Settings, project_id: &str, name: &str) -> Result<Project> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err("Character name must be 1–80 characters".into());
    }
    let (dir, mut project) = find_project(settings, project_id)?;
    if project.name == name {
        return Ok(project);
    }
    // The slug and on-disk directory stay stable so asset references and
    // existing export paths remain valid after a display-name change.
    project.name = name.into();
    project.updated_at = Utc::now();
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn update_workflow_in(
    settings: &Settings,
    project_id: &str,
    workflow: WorkflowSettings,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    if !(2..=256).contains(&workflow.k_colors)
        || !(1..=16).contains(&workflow.sheet_columns)
        || !(1..=16).contains(&workflow.sheet_rows)
        || workflow.typical_nudge_range_px > 5
        || !workflow.chroma.starts_with('#')
        || workflow.chroma.len() != 7
        || !workflow.chroma[1..].chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err("Workflow settings are outside supported ranges".into());
    }
    if project.preset == Preset::KangiFight && workflow.anchor_mode != AnchorMode::BottomCenter {
        return Err("KangiFight anchor mode is fixed by the game contract".into());
    }
    if project.runtime.geometry_policy == GeometryPolicy::Locked
        && workflow.anchor_mode != project.workflow.anchor_mode
    {
        return Err(
            "Runtime geometry is locked; the workflow cannot change its anchor mode".into(),
        );
    }
    let recenter_pivot = project.preset == Preset::Generic
        && workflow.anchor_mode == AnchorMode::BottomCenter
        && (project.runtime.pivot_x != project.runtime.cell_width / 2
            || project.runtime.pivot_y != project.runtime.cell_height.saturating_sub(1));
    if recenter_pivot {
        if project.runtime.geometry_policy == GeometryPolicy::Locked {
            return Err(
                "Runtime geometry is locked; the workflow cannot recenter its pivot".into(),
            );
        }
        if project
            .animations
            .iter()
            .any(|a| !a.frames.is_empty() || !a.historical_frames.is_empty())
            || project
                .anchors
                .values()
                .any(|a| a.active_cleanup_id.is_some())
        {
            return Err("Cannot recenter the pivot after normalized artwork is applied; create a new project for a new runtime contract".into());
        }
        project.runtime.pivot_x = project.runtime.cell_width / 2;
        project.runtime.pivot_y = project.runtime.cell_height.saturating_sub(1);
    }
    let mode_changed = workflow.upscale_mode != project.workflow.upscale_mode;
    let cleaning_changed = workflow.green_fringe_despeckle
        != project.workflow.green_fringe_despeckle
        || recenter_pivot;
    if mode_changed || cleaning_changed {
        for animation in &mut project.animations {
            if animation.active_batch_cleanup_id.is_some() && !animation.frames.is_empty() {
                animation.historical_frames.append(&mut animation.frames);
            }
            if mode_changed {
                animation.upscale_approved = false;
            }
            animation.active_batch_cleanup_id = None;
            animation.active_alignment_id = None;
            animation.active_preview_id = None;
            animation.active_export_id = None;
            if animation.active_batch_snap_id.is_some() {
                if mode_changed {
                    animation.stages.insert(
                        "upscale".into(),
                        if workflow.upscale_mode == UpscaleMode::ManualHandoff {
                            StageState::NeedsInput
                        } else {
                            StageState::Complete
                        },
                    );
                }
                for stage in ["cleanup", "normalize", "align", "preview", "export"] {
                    animation.stages.insert(stage.into(), StageState::Stale);
                }
            }
        }
    }
    project.workflow = workflow;
    project.updated_at = Utc::now();
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn update_runtime_in(
    settings: &Settings,
    project_id: &str,
    cell_width: u32,
    cell_height: u32,
    pivot_x: u32,
    pivot_y: u32,
    anchor_mode: AnchorMode,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    if project.preset == Preset::KangiFight {
        return Err("KangiFight runtime geometry is fixed by the game contract".into());
    }
    if !(16..=1024).contains(&cell_width)
        || !(16..=1024).contains(&cell_height)
        || pivot_x >= cell_width
        || pivot_y >= cell_height
    {
        return Err("Cell must be 16–1024 pixels and pivot must lie inside it".into());
    }
    if anchor_mode == AnchorMode::BottomCenter
        && (pivot_x != cell_width / 2 || pivot_y != cell_height - 1)
    {
        return Err("Bottom-center mode requires a centered X pivot and last-row Y pivot; choose Custom for other coordinates".into());
    }
    if project.runtime.cell_width == cell_width
        && project.runtime.cell_height == cell_height
        && project.runtime.pivot_x == pivot_x
        && project.runtime.pivot_y == pivot_y
        && project.workflow.anchor_mode == anchor_mode
    {
        return Ok(project);
    }
    if project.runtime.geometry_policy == GeometryPolicy::Locked {
        return Err("Runtime geometry is locked for this project; unlock the project policy before changing its cell or pivot".into());
    }
    if project
        .animations
        .iter()
        .any(|a| !a.frames.is_empty() || !a.historical_frames.is_empty())
        || project
            .anchors
            .values()
            .any(|a| a.active_cleanup_id.is_some())
    {
        return Err("Runtime geometry cannot change after normalized artwork is applied; create a new project for a different cell contract".into());
    }
    project.runtime.cell_width = cell_width;
    project.runtime.cell_height = cell_height;
    project.runtime.pivot_x = pivot_x;
    project.runtime.pivot_y = pivot_y;
    project.workflow.anchor_mode = anchor_mode;
    for source in &mut project.imports {
        for stage in ["cleanup", "normalize"] {
            if source.stages.get(stage) != Some(&StageState::Waiting) {
                source.stages.insert(stage.into(), StageState::Stale);
            }
        }
    }
    for animation in &mut project.animations {
        animation.active_batch_cleanup_id = None;
        for stage in ["cleanup", "normalize", "align", "preview", "export"] {
            if animation.stages.get(stage) != Some(&StageState::Waiting) {
                animation.stages.insert(stage.into(), StageState::Stale);
            }
        }
    }
    project.updated_at = Utc::now();
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn update_runtime_policy_in(
    settings: &Settings,
    project_id: &str,
    geometry_policy: GeometryPolicy,
    neutral_height_target: Option<u32>,
    neutral_tolerance_px: u8,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    if neutral_tolerance_px > 32
        || neutral_height_target
            .is_some_and(|target| target == 0 || target > project.runtime.cell_height)
    {
        return Err("Neutral target must fit the cell and tolerance must be 0–32 pixels".into());
    }
    if project.preset == Preset::KangiFight
        && (geometry_policy != GeometryPolicy::Locked
            || neutral_height_target != Some(64)
            || neutral_tolerance_px != 4)
    {
        return Err(
            "This preset's locked runtime and neutral-height contract cannot be changed".into(),
        );
    }
    project.runtime.geometry_policy = geometry_policy;
    project.runtime.neutral_height_target = neutral_height_target;
    project.runtime.neutral_tolerance_px = neutral_tolerance_px;
    project.updated_at = Utc::now();
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn import_into(
    settings: &Settings,
    project_id: &str,
    facing: &str,
    source_path: &str,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    if !project.runtime.source_facings.iter().any(|f| f == facing) {
        return Err("This facing does not accept source imports".into());
    }
    let source =
        fs::canonicalize(source_path).map_err(|e| format!("Source image is missing: {e}"))?;
    let meta = fs::metadata(&source).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > MAX_IMPORT_BYTES {
        return Err("Image must be a file smaller than 32 MB".into());
    }
    let bytes = fs::read(&source).map_err(|e| format!("Cannot read source image: {e}"))?;
    let format = image::guess_format(&bytes)
        .map_err(|_| "Unsupported or damaged image; use PNG or JPEG".to_string())?;
    let (extension, mime) = match format {
        ImageFormat::Png => ("png", "image/png"),
        ImageFormat::Jpeg => ("jpg", "image/jpeg"),
        _ => return Err("Unsupported image format; use PNG or JPEG".into()),
    };
    let (width, height) = image::ImageReader::with_format(Cursor::new(&bytes), format)
        .into_dimensions()
        .map_err(|_| "Image is damaged or incomplete".to_string())?;
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err("Image dimensions must be between 1 and 8192 pixels".into());
    }
    image::load_from_memory_with_format(&bytes, format)
        .map_err(|_| "Image is damaged or incomplete".to_string())?;
    let id = Uuid::new_v4().to_string();
    let relative_path = format!("anchors/{facing}/raw/{id}.{extension}");
    let destination = dir.join(&relative_path);
    let mut dest = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .map_err(|e| format!("Cannot copy source image: {e}"))?;
    if let Err(error) = dest.write_all(&bytes).and_then(|_| dest.sync_all()) {
        drop(dest);
        let _ = fs::remove_file(&destination);
        return Err(format!("Cannot copy source image: {error}"));
    }
    drop(dest);
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let mut stages = BTreeMap::new();
    for stage in STAGES {
        stages.insert(
            stage.to_string(),
            if stage == "raw" {
                StageState::Complete
            } else {
                StageState::Waiting
            },
        );
    }
    let record = ImportRecord {
        id: id.clone(),
        facing: facing.into(),
        original_name: source
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        relative_path,
        sha256: hash,
        mime_type: mime.into(),
        width,
        height,
        imported_at: Utc::now(),
        stages,
    };
    project.imports.push(record);
    let anchor = project
        .anchors
        .get_mut(facing)
        .ok_or("Facing metadata is missing")?;
    anchor.import_ids.push(id.clone());
    anchor.active_import_id = Some(id);
    anchor.active_snap_id = None;
    anchor.active_cleanup_id = None;
    project.updated_at = Utc::now();
    if let Err(error) = atomic_json(&dir.join("project.json"), &project) {
        let _ = fs::remove_file(&destination);
        return Err(error);
    }
    Ok(project)
}

fn preview_in(settings: &Settings, project_id: &str, import_id: &str) -> Result<AssetPreview> {
    let (dir, project) = find_project(settings, project_id)?;
    let record = project
        .imports
        .iter()
        .find(|r| r.id == import_id)
        .ok_or("Import not found")?;
    let (_, bytes) = verified_asset(&dir, &record.relative_path, &record.sha256)?;
    Ok(AssetPreview {
        data_url: format!(
            "data:{};base64,{}",
            record.mime_type,
            STANDARD.encode(bytes)
        ),
        mime_type: record.mime_type.clone(),
        width: record.width,
        height: record.height,
    })
}

fn verified_asset(
    dir: &Path,
    relative_path: &str,
    expected_hash: &str,
) -> Result<(PathBuf, Vec<u8>)> {
    let relative = safe_relative(relative_path)?;
    let path =
        fs::canonicalize(dir.join(relative)).map_err(|_| "Asset file is missing".to_string())?;
    if !path.starts_with(dir) {
        return Err("Asset path escapes the project".into());
    }
    let bytes = fs::read(&path).map_err(|e| format!("Cannot read asset: {e}"))?;
    if format!("{:x}", Sha256::digest(&bytes)) != expected_hash {
        return Err("Asset hash does not match its import record".into());
    }
    Ok((path, bytes))
}

fn inspect_snapper(path: &Path) -> Result<String> {
    let canonical = fs::canonicalize(path).map_err(|e| format!("Snapper CLI is missing: {e}"))?;
    if !canonical.is_file() {
        return Err("Choose a Sprite Fusion CLI executable".into());
    }
    let file_name = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if file_name != "spritefusion-pixel-snapper.exe" && file_name != "spritefusion-pixel-snapper" {
        return Err("Choose spritefusion-pixel-snapper.exe".into());
    }
    let output = Command::new(&canonical)
        .arg("--version")
        .output()
        .map_err(|e| format!("Cannot launch Sprite Fusion CLI: {e}"))?;
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success() || !version.starts_with("spritefusion-pixel-snapper ") {
        return Err("Selected executable did not identify as Sprite Fusion Pixel Snapper".into());
    }
    Ok(version)
}

fn configure_snapper_in(settings_path: &Path, selected: &str) -> Result<Settings> {
    let canonical =
        fs::canonicalize(selected).map_err(|e| format!("Snapper CLI is missing: {e}"))?;
    let version = inspect_snapper(&canonical)?;
    let mut settings = read_settings(settings_path)?;
    settings.snapper_executable = Some(canonical.to_string_lossy().into_owned());
    settings.snapper_version = Some(version);
    atomic_json(settings_path, &settings)?;
    Ok(settings)
}

fn inspect_python(path: &Path) -> Result<String> {
    let canonical =
        fs::canonicalize(path).map_err(|e| format!("Python executable is missing: {e}"))?;
    if !canonical.is_file() {
        return Err("Choose a Python executable".into());
    }
    let output = Command::new(&canonical)
        .args(["-I", "-c", "import PIL, cv2; print('sprite-studio-python|' + PIL.__version__ + '|' + cv2.__version__)"])
        .output()
        .map_err(|e| format!("Cannot launch Python: {e}"))?;
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success() || !value.starts_with("sprite-studio-python|") {
        return Err("This Python environment needs Pillow and opencv-python-headless; install processing-requirements.txt first".into());
    }
    Ok(value)
}

fn configure_python_in(settings_path: &Path, selected: &str) -> Result<Settings> {
    let canonical =
        fs::canonicalize(selected).map_err(|e| format!("Python executable is missing: {e}"))?;
    let environment = inspect_python(&canonical)?;
    let mut settings = read_settings(settings_path)?;
    settings.python_executable = Some(canonical.to_string_lossy().into_owned());
    settings.python_environment = Some(environment);
    atomic_json(settings_path, &settings)?;
    Ok(settings)
}

fn validated_cleanup_options(options: &CleanupOptions) -> Result<String> {
    if options.tolerance > 80 || options.min_area > 64 {
        return Err("Tolerance must be 0–80 and speckle minimum 0–64 pixels".into());
    }
    let background = options.background.trim().trim_start_matches('#');
    if background.eq_ignore_ascii_case("auto") {
        return Ok("auto".into());
    }
    if background.len() != 6 || !background.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Background must be Auto or a six-digit RGB color".into());
    }
    Ok(background.to_ascii_lowercase())
}

fn mark_cleanup_stages(project: &mut Project, source_id: &str, stage: StageState) -> Result<()> {
    let record = project
        .imports
        .iter_mut()
        .find(|record| record.id == source_id)
        .ok_or("Active source import is missing")?;
    record.stages.insert("cleanup".into(), stage.clone());
    record.stages.insert("normalize".into(), stage);
    project.updated_at = Utc::now();
    Ok(())
}

fn checked_png(
    dir: &Path,
    relative: &str,
    expected_width: u32,
    expected_height: u32,
) -> Result<String> {
    let path = dir.join(safe_relative(relative)?);
    let bytes = fs::read(&path).map_err(|e| format!("Cleanup output is missing: {e}"))?;
    if bytes.len() > MAX_IMPORT_BYTES as usize
        || image::guess_format(&bytes).ok() != Some(ImageFormat::Png)
    {
        return Err("Cleanup output must be a PNG smaller than 32 MB".into());
    }
    let image = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
        .map_err(|_| "Cleanup output is damaged".to_string())?
        .to_rgba8();
    if image.width() != expected_width
        || image.height() != expected_height
        || image.pixels().any(|pixel| {
            (pixel.0[3] != 0 && pixel.0[3] != 255) || (pixel.0[3] == 0 && pixel.0[..3] != [0, 0, 0])
        })
    {
        return Err(
            "Processed PNG has wrong dimensions, partial alpha, or hidden fringe colors".into(),
        );
    }
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

fn checked_raw_png(
    dir: &Path,
    relative: &str,
    expected_width: u32,
    expected_height: u32,
) -> Result<String> {
    let path = dir.join(safe_relative(relative)?);
    let bytes = fs::read(&path).map_err(|e| format!("Recovered pose is missing: {e}"))?;
    if bytes.len() > MAX_IMPORT_BYTES as usize
        || image::guess_format(&bytes).ok() != Some(ImageFormat::Png)
    {
        return Err("Recovered pose must be a PNG under 32 MB".into());
    }
    let (width, height) = image::ImageReader::with_format(Cursor::new(&bytes), ImageFormat::Png)
        .into_dimensions()
        .map_err(|_| "Recovered pose is damaged".to_string())?;
    if width != expected_width || height != expected_height {
        return Err("Recovered pose dimensions do not match detection".into());
    }
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

fn validated_snap_options(
    options: &SnapOptions,
    source_width: u32,
    source_height: u32,
) -> Result<Option<String>> {
    if !(2..=256).contains(&options.colors) {
        return Err("Palette color count must be 2–256".into());
    }
    if let Some(size) = options.pixel_size {
        if size == 0 || size > source_width.min(source_height) {
            return Err("Pixel size must fit within the source image".into());
        }
    }
    let palette = options
        .palette
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    if let Some(value) = palette {
        let colors: Vec<&str> = value.split(',').map(str::trim).collect();
        if colors.is_empty()
            || colors.len() > options.colors as usize
            || colors
                .iter()
                .any(|part| part.len() != 6 || !part.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            return Err("Palette must contain comma-separated six-digit RGB hex colors, with no more entries than the color count".into());
        }
        return Ok(Some(colors.join(",").to_ascii_lowercase()));
    }
    Ok(None)
}

fn mark_snap_stage(project: &mut Project, source_import_id: &str, stage: StageState) -> Result<()> {
    let record = project
        .imports
        .iter_mut()
        .find(|record| record.id == source_import_id)
        .ok_or("Active source import is missing")?;
    record.stages.insert("snap".into(), stage);
    project.updated_at = Utc::now();
    Ok(())
}

fn snapper_pixel_size(stdout: &[u8]) -> Option<f64> {
    let text = std::str::from_utf8(stdout).ok()?;
    text.lines().find_map(|line| {
        line.trim()
            .strip_prefix("Pixel size: ")?
            .split_once("px")?
            .0
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|size| size.is_finite() && *size >= 1.0)
    })
}

fn snap_placement(runtime: &RuntimeSettings, width: u32, height: u32) -> (i32, i32, bool) {
    let x = runtime.pivot_x as i32 - (width / 2) as i32;
    let y = runtime.pivot_y as i32 - height as i32 + 1;
    let fits = width > 0
        && height > 0
        && x >= 0
        && y >= 0
        && (x as u32).saturating_add(width) <= runtime.cell_width
        && (y as u32).saturating_add(height) <= runtime.cell_height;
    (x, y, fits)
}

fn rank_auto_fit_candidates(run: &AutoFitRun) -> Vec<&AutoFitCandidate> {
    let mut valid: Vec<_> = run
        .candidates
        .iter()
        .filter(|candidate| candidate.fits)
        .collect();
    valid.sort_by(|a, b| {
        let target_key = |candidate: &AutoFitCandidate| {
            run.neutral_height_target.map_or((0, 0), |target| {
                let delta = candidate.foreground_height.abs_diff(target);
                ((delta > run.neutral_tolerance_px as u32) as u8, delta)
            })
        };
        // A configured neutral target is an art-contract preference; among
        // equally suitable heights, keep the closest recovered grid to source.
        target_key(a)
            .cmp(&target_key(b))
            .then_with(|| {
                (a.pixel_size - run.starting_pixel_size)
                    .abs()
                    .total_cmp(&(b.pixel_size - run.starting_pixel_size).abs())
            })
            .then_with(|| b.foreground_pixels.cmp(&a.foreground_pixels))
            .then_with(|| a.removed_speckle_pixels.cmp(&b.removed_speckle_pixels))
            .then_with(|| a.pixel_size.total_cmp(&b.pixel_size))
    });
    valid
}

fn inspect_snap_fit(
    python: &Path,
    native_path: &Path,
    options: &CleanupOptions,
    background: &str,
    runtime: &RuntimeSettings,
    fringe_cleanup: bool,
) -> Result<FitMetrics> {
    let output = Command::new(python)
        .args(["-I", "-c", CLEANUP_SCRIPT])
        .arg(native_path)
        .arg(native_path.with_extension("inspection-cleaned.png"))
        .arg(native_path.with_extension("inspection-normalized.png"))
        .arg(background)
        .arg(options.tolerance.to_string())
        .arg(options.min_area.to_string())
        .arg(runtime.cell_width.to_string())
        .arg(runtime.cell_height.to_string())
        .arg(runtime.pivot_x.to_string())
        .arg(runtime.pivot_y.to_string())
        .arg(if fringe_cleanup { "1" } else { "0" })
        .arg("--inspect-only")
        .output()
        .map_err(|e| format!("Auto-fit inspection could not run: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Auto-fit inspection failed: {}",
            String::from_utf8_lossy(&output.stderr)
                .trim()
                .chars()
                .take(400)
                .collect::<String>()
        ));
    }
    let metrics: FitMetrics = serde_json::from_slice(&output.stdout)
        .map_err(|_| "Auto-fit inspection returned invalid metrics".to_string())?;
    let (x, y, fits) = snap_placement(runtime, metrics.foreground_width, metrics.foreground_height);
    if metrics.processor_version != "sprite-studio-cleanup-1"
        || metrics.foreground_pixels == 0
        || metrics.foreground_width > 8192
        || metrics.foreground_height > 8192
        || metrics.placement_x != x
        || metrics.placement_y != y
        || metrics.fits != fits
    {
        return Err("Auto-fit inspection disagrees with the fixed-cell pivot placement".into());
    }
    Ok(metrics)
}

fn run_snap_in(
    settings: &Settings,
    project_id: &str,
    facing: &str,
    options: SnapOptions,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let source_id = project
        .anchors
        .get(facing)
        .and_then(|anchor| anchor.active_import_id.clone())
        .ok_or("Import a source anchor for this facing first")?;
    let source = project
        .imports
        .iter()
        .find(|record| record.id == source_id && record.facing == facing)
        .ok_or("Active source import is invalid")?
        .clone();
    let (input_path, _) = verified_asset(&dir, &source.relative_path, &source.sha256)?;
    let palette = validated_snap_options(&options, source.width, source.height)?;
    let executable = settings
        .snapper_executable
        .as_ref()
        .ok_or("Locate the Sprite Fusion CLI first")?;
    let executable = fs::canonicalize(executable)
        .map_err(|_| "Configured Sprite Fusion CLI is missing; locate it again".to_string())?;
    let version = inspect_snapper(&executable)?;
    let review_id = Uuid::new_v4().to_string();
    let relative_dir = format!("anchors/{facing}/snap/{review_id}");
    let review_dir = dir.join(&relative_dir);
    fs::create_dir_all(&review_dir)
        .map_err(|e| format!("Cannot create snap review folder: {e}"))?;
    let native_path = review_dir.join("native.png");
    let reference_path = review_dir.join("reference.png");
    mark_snap_stage(&mut project, &source_id, StageState::Processing)?;
    atomic_json(&dir.join("project.json"), &project)?;

    let result = (|| -> Result<SnapReview> {
        let mut command = Command::new(&executable);
        command
            .arg(&input_path)
            .arg(&native_path)
            .arg(options.colors.to_string());
        if let Some(size) = options.pixel_size {
            command.arg("--pixel-size").arg(size.to_string());
        }
        if let Some(value) = &palette {
            command.arg("--palette").arg(value);
        }
        let output = command
            .output()
            .map_err(|e| format!("Sprite Fusion CLI could not run: {e}"))?;
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Sprite Fusion CLI failed: {}",
                detail.trim().chars().take(500).collect::<String>()
            ));
        }
        let detected_pixel_size = snapper_pixel_size(&output.stdout);
        let native_bytes = fs::read(&native_path)
            .map_err(|e| format!("Sprite Fusion CLI did not produce a native image: {e}"))?;
        if native_bytes.len() > MAX_IMPORT_BYTES as usize
            || image::guess_format(&native_bytes).ok() != Some(ImageFormat::Png)
        {
            return Err("Sprite Fusion CLI produced an unsupported image".into());
        }
        let native = image::load_from_memory_with_format(&native_bytes, ImageFormat::Png)
            .map_err(|_| "Sprite Fusion CLI produced a damaged image".to_string())?;
        let (native_width, native_height) = (native.width(), native.height());
        if native_width == 0 || native_height == 0 || native_width > 8192 || native_height > 8192 {
            return Err("Recovered grid dimensions are invalid".into());
        }
        let scale = (source.width / native_width)
            .min(source.height / native_height)
            .max(1);
        let reference_width = native_width * scale;
        let reference_height = native_height * scale;
        native
            .resize_exact(
                reference_width,
                reference_height,
                image::imageops::FilterType::Nearest,
            )
            .save_with_format(&reference_path, ImageFormat::Png)
            .map_err(|e| format!("Cannot create nearest-neighbour reference: {e}"))?;
        let reference_bytes =
            fs::read(&reference_path).map_err(|e| format!("Cannot read reference image: {e}"))?;
        Ok(SnapReview {
            id: review_id,
            facing: facing.into(),
            source_import_id: source_id.clone(),
            native_relative_path: format!("{relative_dir}/native.png"),
            native_sha256: format!("{:x}", Sha256::digest(&native_bytes)),
            native_width,
            native_height,
            reference_relative_path: format!("{relative_dir}/reference.png"),
            reference_sha256: format!("{:x}", Sha256::digest(&reference_bytes)),
            reference_width,
            reference_height,
            reference_scale: scale,
            outputs: Some(SnapOutputPaths {
                native_path: format!("{relative_dir}/native.png"),
                snapped_path: Some(format!("{relative_dir}/native.png")),
                upscaled_path: Some(format!("{relative_dir}/reference.png")),
                chroma_path: None,
                mode: "anchor".into(),
            }),
            colors: options.colors,
            pixel_size: options.pixel_size,
            detected_pixel_size,
            auto_fit_run_id: None,
            palette,
            tool_version: version,
            created_at: Utc::now(),
            applied_at: None,
        })
    })();
    match result {
        Ok(review) => {
            project.snap_reviews.push(review);
            mark_snap_stage(&mut project, &source_id, StageState::Review)?;
            atomic_json(&dir.join("project.json"), &project)?;
            Ok(project)
        }
        Err(error) => {
            let _ = fs::remove_file(&native_path);
            let _ = fs::remove_file(&reference_path);
            let _ = fs::remove_dir(&review_dir);
            mark_snap_stage(&mut project, &source_id, StageState::Failed)?;
            atomic_json(&dir.join("project.json"), &project)?;
            Err(error)
        }
    }
}

const MAX_AUTO_FIT_ATTEMPTS: usize = 16;
const MAX_AUTO_FIT_CHOICES: usize = 3;

fn run_auto_fit_in(
    settings: &Settings,
    project_id: &str,
    facing: &str,
    cleanup_options: CleanupOptions,
) -> Result<Project> {
    let (dir, project) = find_project(settings, project_id)?;
    let anchor = project
        .anchors
        .get(facing)
        .ok_or("This facing is not part of the project")?;
    let source_id = anchor
        .active_import_id
        .clone()
        .ok_or("Import a source anchor before auto-fit")?;
    let active_id = anchor
        .active_snap_id
        .as_ref()
        .ok_or("Apply a snapped anchor before auto-fit")?;
    let active = project
        .snap_reviews
        .iter()
        .find(|review| &review.id == active_id && review.source_import_id == source_id)
        .ok_or("Active snapped anchor is invalid")?;
    let source = project
        .imports
        .iter()
        .find(|item| item.id == source_id)
        .ok_or("Active source import is missing")?;
    let original_snap_stage = source
        .stages
        .get("snap")
        .cloned()
        .unwrap_or(StageState::Complete);
    let background = validated_cleanup_options(&cleanup_options)?;
    let python = fs::canonicalize(
        settings
            .python_executable
            .as_ref()
            .ok_or("Locate a Python environment with Pillow and OpenCV first")?,
    )
    .map_err(|_| "Configured Python environment is missing; locate it again".to_string())?;
    inspect_python(&python)?;
    let snapper = fs::canonicalize(
        settings
            .snapper_executable
            .as_ref()
            .ok_or("Locate the Sprite Fusion CLI first")?,
    )
    .map_err(|_| "Configured Sprite Fusion CLI is missing; locate it again".to_string())?;
    inspect_snapper(&snapper)?;
    let colors = active.colors;
    let palette = active.palette.clone();
    let initial_override = active.pixel_size;
    let source_limit = source.width.min(source.height);
    let runtime = project.runtime.clone();
    let fringe_cleanup = project.workflow.green_fringe_despeckle;
    let run_id = Uuid::new_v4().to_string();
    let mut run = AutoFitRun {
        id: run_id.clone(),
        facing: facing.into(),
        source_import_id: source_id.clone(),
        cell_width: runtime.cell_width,
        cell_height: runtime.cell_height,
        pivot_x: runtime.pivot_x,
        pivot_y: runtime.pivot_y,
        neutral_height_target: runtime.neutral_height_target,
        neutral_tolerance_px: runtime.neutral_tolerance_px,
        starting_pixel_size: 0.0,
        candidates: vec![],
        recommended_review_ids: vec![],
        selected_review_id: None,
        created_at: Utc::now(),
    };
    for attempt in 0..MAX_AUTO_FIT_ATTEMPTS {
        let pixel_size = if attempt == 0 {
            initial_override
        } else {
            let next = run.starting_pixel_size.floor() as u32 + attempt as u32;
            if next > source_limit {
                break;
            }
            Some(next)
        };
        let updated = match run_snap_in(
            settings,
            project_id,
            facing,
            SnapOptions {
                colors,
                pixel_size,
                palette: palette.clone(),
            },
        ) {
            Ok(project) => project,
            Err(error) => {
                let (dir, mut restored) = find_project(settings, project_id)?;
                mark_snap_stage(&mut restored, &source_id, original_snap_stage)?;
                atomic_json(&dir.join("project.json"), &restored)?;
                return Err(error);
            }
        };
        let mut updated = updated;
        let review = updated
            .snap_reviews
            .last_mut()
            .ok_or("Auto-fit snap review was not saved")?;
        review.auto_fit_run_id = Some(run_id.clone());
        let actual_size = review
            .detected_pixel_size
            .or_else(|| pixel_size.map(f64::from))
            .unwrap_or(source.width as f64 / review.native_width as f64);
        if attempt == 0 {
            run.starting_pixel_size = actual_size;
        }
        let review_id = review.id.clone();
        let (native_path, _) =
            verified_asset(&dir, &review.native_relative_path, &review.native_sha256)?;
        let metrics = match inspect_snap_fit(
            &python,
            &native_path,
            &cleanup_options,
            &background,
            &runtime,
            fringe_cleanup,
        ) {
            Ok(metrics) => metrics,
            Err(error) => {
                mark_snap_stage(&mut updated, &source_id, original_snap_stage)?;
                atomic_json(&dir.join("project.json"), &updated)?;
                return Err(error);
            }
        };
        run.candidates.push(AutoFitCandidate {
            review_id,
            pixel_size: actual_size,
            foreground_width: metrics.foreground_width,
            foreground_height: metrics.foreground_height,
            foreground_pixels: metrics.foreground_pixels,
            removed_speckle_pixels: metrics.removed_speckle_pixels,
            fits: metrics.fits,
        });
        if let Some(existing) = updated
            .auto_fit_runs
            .iter_mut()
            .find(|item| item.id == run_id)
        {
            *existing = run.clone();
        } else {
            updated.auto_fit_runs.push(run.clone());
        }
        mark_snap_stage(&mut updated, &source_id, original_snap_stage.clone())?;
        atomic_json(&dir.join("project.json"), &updated)?;
        let valid_count = run.candidates.iter().filter(|item| item.fits).count();
        let on_target_count = run
            .candidates
            .iter()
            .filter(|item| {
                item.fits
                    && runtime.neutral_height_target.is_some_and(|target| {
                        item.foreground_height.abs_diff(target)
                            <= runtime.neutral_tolerance_px as u32
                    })
            })
            .count();
        if valid_count >= MAX_AUTO_FIT_CHOICES
            && (runtime.neutral_height_target.is_none() || on_target_count >= MAX_AUTO_FIT_CHOICES)
        {
            break;
        }
    }
    let mut finished = find_project(settings, project_id)?.1;
    let ranked = rank_auto_fit_candidates(&run);
    run.recommended_review_ids = ranked
        .iter()
        .take(MAX_AUTO_FIT_CHOICES)
        .map(|candidate| candidate.review_id.clone())
        .collect();
    run.selected_review_id = run.recommended_review_ids.first().cloned();
    let saved = finished
        .auto_fit_runs
        .iter_mut()
        .find(|item| item.id == run_id)
        .ok_or("Auto-fit run was not saved")?;
    let found = run.selected_review_id.is_some();
    *saved = run;
    atomic_json(&dir.join("project.json"), &finished)?;
    if !found {
        return Err(format!(
            "No lossless snap candidate fits the locked {}×{} cell at pivot ({},{}). Try a different source generation or manually choose a coarser pixel size.",
            runtime.cell_width, runtime.cell_height, runtime.pivot_x, runtime.pivot_y
        ));
    }
    Ok(finished)
}

fn apply_snap_in(settings: &Settings, project_id: &str, review_id: &str) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let review = project
        .snap_reviews
        .iter()
        .find(|review| review.id == review_id)
        .ok_or("Snap review not found")?
        .clone();
    let anchor = project
        .anchors
        .get(&review.facing)
        .ok_or("Facing is missing")?;
    if anchor.active_import_id.as_ref() != Some(&review.source_import_id) {
        return Err("This review belongs to an older source import; run snap again".into());
    }
    if let Some(run_id) = &review.auto_fit_run_id {
        let run = project
            .auto_fit_runs
            .iter()
            .find(|item| &item.id == run_id)
            .ok_or("Auto-fit review metadata is missing")?;
        let candidate = run
            .candidates
            .iter()
            .find(|item| item.review_id == review.id)
            .ok_or("Auto-fit candidate metrics are missing")?;
        if !candidate.fits
            || run.cell_width != project.runtime.cell_width
            || run.cell_height != project.runtime.cell_height
            || run.pivot_x != project.runtime.pivot_x
            || run.pivot_y != project.runtime.pivot_y
        {
            return Err(
                "This auto-fit candidate does not fit the current runtime cell and pivot".into(),
            );
        }
    }
    verified_asset(&dir, &review.native_relative_path, &review.native_sha256)?;
    verified_asset(
        &dir,
        &review.reference_relative_path,
        &review.reference_sha256,
    )?;
    let previous_snap = project.anchors[&review.facing].active_snap_id.clone();
    let anchor = project.anchors.get_mut(&review.facing).unwrap();
    anchor.active_snap_id = Some(review.id.clone());
    if previous_snap.as_ref() != Some(&review.id) {
        anchor.active_cleanup_id = None;
    }
    project
        .snap_reviews
        .iter_mut()
        .find(|item| item.id == review_id)
        .unwrap()
        .applied_at = Some(Utc::now());
    mark_snap_stage(&mut project, &review.source_import_id, StageState::Complete)?;
    if previous_snap.as_ref() != Some(&review.id) {
        mark_cleanup_stages(&mut project, &review.source_import_id, StageState::Waiting)?;
    }
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn run_cleanup_in(
    settings: &Settings,
    project_id: &str,
    facing: &str,
    options: CleanupOptions,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let anchor = project
        .anchors
        .get(facing)
        .ok_or("This facing has no source anchor")?;
    let source_id = anchor
        .active_import_id
        .clone()
        .ok_or("Import a source anchor first")?;
    let snap_id = anchor
        .active_snap_id
        .clone()
        .ok_or("Apply a snapped anchor before cleanup")?;
    let snap = project
        .snap_reviews
        .iter()
        .find(|review| review.id == snap_id && review.source_import_id == source_id)
        .ok_or("Active snapped anchor is invalid")?
        .clone();
    if snap.native_width > 2048 || snap.native_height > 2048 {
        return Err(
            "Recovered grid is too large for local cleanup; use a smaller snapped anchor".into(),
        );
    }
    let (source_path, _) = verified_asset(&dir, &snap.native_relative_path, &snap.native_sha256)?;
    let background = validated_cleanup_options(&options)?;
    let executable = settings
        .python_executable
        .as_ref()
        .ok_or("Locate a Python environment with Pillow and OpenCV first")?;
    let executable = fs::canonicalize(executable)
        .map_err(|_| "Configured Python environment is missing; locate it again".to_string())?;
    let environment = inspect_python(&executable)?;
    let id = Uuid::new_v4().to_string();
    let relative_dir = format!("anchors/{facing}/cleanup/{id}");
    let review_dir = dir.join(&relative_dir);
    fs::create_dir_all(&review_dir)
        .map_err(|e| format!("Cannot create cleanup review folder: {e}"))?;
    let cleaned_relative = format!("{relative_dir}/cleaned.png");
    let normalized_relative = format!("{relative_dir}/normalized.png");
    let cleaned_path = dir.join(&cleaned_relative);
    let normalized_path = dir.join(&normalized_relative);
    mark_cleanup_stages(&mut project, &source_id, StageState::Processing)?;
    atomic_json(&dir.join("project.json"), &project)?;

    let result = (|| -> Result<CleanupReview> {
        let runtime = &project.runtime;
        let output = Command::new(&executable)
            .args(["-I", "-c", CLEANUP_SCRIPT])
            .arg(&source_path)
            .arg(&cleaned_path)
            .arg(&normalized_path)
            .arg(&background)
            .arg(options.tolerance.to_string())
            .arg(options.min_area.to_string())
            .arg(runtime.cell_width.to_string())
            .arg(runtime.cell_height.to_string())
            .arg(runtime.pivot_x.to_string())
            .arg(runtime.pivot_y.to_string())
            .arg(if project.workflow.green_fringe_despeckle {
                "1"
            } else {
                "0"
            })
            .output()
            .map_err(|e| format!("Cleanup processor could not run: {e}"))?;
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Cleanup failed: {}",
                detail.trim().chars().take(500).collect::<String>()
            ));
        }
        let metrics: CleanupMetrics = serde_json::from_slice(&output.stdout)
            .map_err(|_| "Cleanup processor returned invalid metrics".to_string())?;
        if metrics.processor_version != "sprite-studio-cleanup-1"
            || metrics.foreground_pixels == 0
            || metrics.bbox_width == 0
            || metrics.bbox_height == 0
            || metrics
                .placement_x
                .checked_add(metrics.bbox_width)
                .is_none_or(|v| v > runtime.cell_width)
            || metrics
                .placement_y
                .checked_add(metrics.bbox_height)
                .is_none_or(|v| v > runtime.cell_height)
        {
            return Err("Cleanup processor returned invalid dimensions".into());
        }
        let cleaned_hash = checked_png(
            &dir,
            &cleaned_relative,
            metrics.bbox_width,
            metrics.bbox_height,
        )?;
        let normalized_hash = checked_png(
            &dir,
            &normalized_relative,
            runtime.cell_width,
            runtime.cell_height,
        )?;
        Ok(CleanupReview {
            id,
            facing: facing.into(),
            source_import_id: source_id.clone(),
            source_snap_id: snap_id,
            cleaned_relative_path: cleaned_relative,
            cleaned_sha256: cleaned_hash,
            cleaned_width: metrics.bbox_width,
            cleaned_height: metrics.bbox_height,
            normalized_relative_path: normalized_relative,
            normalized_sha256: normalized_hash,
            normalized_width: runtime.cell_width,
            normalized_height: runtime.cell_height,
            background_hex: metrics.background_hex,
            tolerance: options.tolerance,
            min_area: options.min_area,
            foreground_pixels: metrics.foreground_pixels,
            removed_speckle_pixels: metrics.removed_speckle_pixels,
            placement_x: metrics.placement_x,
            placement_y: metrics.placement_y,
            processor_version: metrics.processor_version,
            python_environment: environment,
            created_at: Utc::now(),
            applied_at: None,
        })
    })();
    match result {
        Ok(review) => {
            project.cleanup_reviews.push(review);
            mark_cleanup_stages(&mut project, &source_id, StageState::Review)?;
            atomic_json(&dir.join("project.json"), &project)?;
            Ok(project)
        }
        Err(error) => {
            let _ = fs::remove_file(&cleaned_path);
            let _ = fs::remove_file(&normalized_path);
            let _ = fs::remove_dir(&review_dir);
            mark_cleanup_stages(&mut project, &source_id, StageState::Failed)?;
            atomic_json(&dir.join("project.json"), &project)?;
            Err(error)
        }
    }
}

fn apply_cleanup_in(settings: &Settings, project_id: &str, review_id: &str) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let review = project
        .cleanup_reviews
        .iter()
        .find(|review| review.id == review_id)
        .ok_or("Cleanup review not found")?
        .clone();
    let anchor = project
        .anchors
        .get(&review.facing)
        .ok_or("Facing is missing")?;
    if anchor.active_import_id.as_ref() != Some(&review.source_import_id)
        || anchor.active_snap_id.as_ref() != Some(&review.source_snap_id)
    {
        return Err("This cleanup belongs to an older snapped input; run cleanup again".into());
    }
    let source = project
        .imports
        .iter()
        .find(|item| item.id == review.source_import_id)
        .ok_or("Cleanup source import is missing")?;
    if source.stages.get("cleanup") != Some(&StageState::Review)
        || review.normalized_width != project.runtime.cell_width
        || review.normalized_height != project.runtime.cell_height
    {
        return Err("Runtime geometry changed after this cleanup review; run cleanup again".into());
    }
    if project
        .cleanup_reviews
        .iter()
        .rev()
        .find(|item| {
            item.source_import_id == review.source_import_id
                && item.source_snap_id == review.source_snap_id
        })
        .is_none_or(|item| item.id != review.id)
    {
        return Err("A newer cleanup review superseded this one; apply the latest result".into());
    }
    verified_asset(&dir, &review.cleaned_relative_path, &review.cleaned_sha256)?;
    verified_asset(
        &dir,
        &review.normalized_relative_path,
        &review.normalized_sha256,
    )?;
    project
        .anchors
        .get_mut(&review.facing)
        .unwrap()
        .active_cleanup_id = Some(review.id.clone());
    project
        .cleanup_reviews
        .iter_mut()
        .find(|item| item.id == review_id)
        .unwrap()
        .applied_at = Some(Utc::now());
    mark_cleanup_stages(&mut project, &review.source_import_id, StageState::Complete)?;
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn export_copy(source: &[u8], expected_hash: &str, target: &Path) -> Result<()> {
    if target.exists() {
        let metadata = fs::symlink_metadata(target)
            .map_err(|e| format!("Cannot inspect existing export: {e}"))?;
        if !metadata.file_type().is_file() {
            return Err("Existing export is not a regular file".into());
        }
        let existing = fs::read(target).map_err(|e| format!("Cannot read existing export: {e}"))?;
        if format!("{:x}", Sha256::digest(&existing)) != expected_hash {
            return Err("Existing export differs from the applied result; move it aside before exporting again".into());
        }
        return Ok(());
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|e| format!("Cannot create export file: {e}"))?;
    if let Err(error) = file.write_all(source).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(target);
        return Err(format!("Cannot write export file: {error}"));
    }
    Ok(())
}

fn reference_export_folder_in(
    settings: &Settings,
    project_id: &str,
    facing: &str,
) -> Result<Option<PathBuf>> {
    let (dir, project) = find_project(settings, project_id)?;
    let anchor = project
        .anchors
        .get(facing)
        .ok_or("Facing is not part of this project")?;
    let Some(cleanup_id) = &anchor.active_cleanup_id else {
        return Ok(None);
    };
    let review = project
        .cleanup_reviews
        .iter()
        .find(|item| &item.id == cleanup_id)
        .ok_or("Applied cleanup review is missing")?;
    let snap = project
        .snap_reviews
        .iter()
        .find(|item| item.id == review.source_snap_id)
        .ok_or("Applied snap review is missing")?;
    if review.facing != facing
        || anchor.active_import_id.as_ref() != Some(&review.source_import_id)
        || anchor.active_snap_id.as_ref() != Some(&review.source_snap_id)
    {
        return Err("Applied cleanup no longer matches this source".into());
    }
    let relative = format!("references/{facing}/{cleanup_id}");
    let path = dir.join("exports").join(relative);
    if !path.exists() {
        return Ok(None);
    }
    let canonical =
        fs::canonicalize(&path).map_err(|e| format!("Cannot access reference export: {e}"))?;
    let root =
        fs::canonicalize(dir.join("exports")).map_err(|e| format!("Cannot access exports: {e}"))?;
    if !canonical.is_dir() || !canonical.starts_with(&root) || !root.starts_with(&dir) {
        return Err("Reference export escapes the project".into());
    }
    for (name, hash) in [
        ("normalized.png", &review.normalized_sha256),
        ("cutout.png", &review.cleaned_sha256),
        ("snapped-native.png", &snap.native_sha256),
        ("nearest-neighbour-reference.png", &snap.reference_sha256),
    ] {
        let file = canonical.join(name);
        if !fs::symlink_metadata(&file)
            .map_err(|e| format!("Reference export is incomplete: {e}"))?
            .file_type()
            .is_file()
        {
            return Err("Reference export contains a non-file asset".into());
        }
        let bytes = fs::read(file).map_err(|e| format!("Reference export is incomplete: {e}"))?;
        if format!("{:x}", Sha256::digest(&bytes)) != *hash {
            return Err("Reference export hash does not match the applied result".into());
        }
    }
    let manifest_path = canonical.join("manifest.json");
    if !fs::symlink_metadata(&manifest_path)
        .map_err(|e| format!("Reference export manifest is missing: {e}"))?
        .file_type()
        .is_file()
    {
        return Err("Reference export manifest is not a regular file".into());
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(manifest_path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Reference export manifest is damaged: {e}"))?;
    if manifest["cleanupReviewId"] != review.id || manifest["projectId"] != project.id {
        return Err("Reference export manifest does not match the applied result".into());
    }
    Ok(Some(canonical))
}

fn export_reference_in(settings: &Settings, project_id: &str, facing: &str) -> Result<String> {
    let (dir, project) = find_project(settings, project_id)?;
    let anchor = project
        .anchors
        .get(facing)
        .ok_or("Facing is not part of this project")?;
    let cleanup_id = anchor
        .active_cleanup_id
        .as_ref()
        .ok_or("Apply cleanup before exporting this reference")?;
    let cleanup = project
        .cleanup_reviews
        .iter()
        .find(|item| &item.id == cleanup_id)
        .ok_or("Applied cleanup review is missing")?;
    let snap = project
        .snap_reviews
        .iter()
        .find(|item| item.id == cleanup.source_snap_id)
        .ok_or("Applied snap review is missing")?;
    if cleanup.facing != facing
        || snap.facing != facing
        || anchor.active_import_id.as_ref() != Some(&cleanup.source_import_id)
        || anchor.active_snap_id.as_ref() != Some(&cleanup.source_snap_id)
    {
        return Err("Applied reference results no longer match the source".into());
    }
    let files = [
        (
            "normalized.png",
            &cleanup.normalized_relative_path,
            &cleanup.normalized_sha256,
        ),
        (
            "cutout.png",
            &cleanup.cleaned_relative_path,
            &cleanup.cleaned_sha256,
        ),
        (
            "snapped-native.png",
            &snap.native_relative_path,
            &snap.native_sha256,
        ),
        (
            "nearest-neighbour-reference.png",
            &snap.reference_relative_path,
            &snap.reference_sha256,
        ),
    ];
    let source_bytes: Vec<_> = files
        .iter()
        .map(|(_, relative, hash)| verified_asset(&dir, relative, hash).map(|(_, bytes)| bytes))
        .collect::<Result<_>>()?;
    let folder = project_export_dir(&dir, &format!("references/{facing}/{cleanup_id}"))?;
    for ((name, _, hash), bytes) in files.iter().zip(&source_bytes) {
        export_copy(bytes, hash, &folder.join(name))?;
    }
    let manifest = serde_json::json!({
        "schemaVersion": 1, "approvalState": "DRAFT", "kind": "single_reference",
        "projectId": project.id, "projectName": project.name, "facing": facing,
        "sourceImportId": cleanup.source_import_id, "snapReviewId": snap.id,
        "cleanupReviewId": cleanup.id,
        "cell": {"width": project.runtime.cell_width, "height": project.runtime.cell_height},
        "pivot": {"x": project.runtime.pivot_x, "y": project.runtime.pivot_y},
        "files": files.iter().map(|(name, _, hash)| serde_json::json!({"file": name, "sha256": hash})).collect::<Vec<_>>(),
    });
    let manifest_path = folder.join("manifest.json");
    if manifest_path.exists() {
        if !fs::symlink_metadata(&manifest_path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_file()
        {
            return Err("Existing export manifest is not a regular file".into());
        }
        let existing: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).map_err(|e| e.to_string())?)
                .map_err(|e| format!("Existing export manifest is damaged: {e}"))?;
        if existing != manifest {
            return Err("Existing export manifest differs from the applied result".into());
        }
    } else {
        atomic_json(&manifest_path, &manifest)?;
    }
    Ok(folder.to_string_lossy().into_owned())
}

fn cleanup_preview_in(
    settings: &Settings,
    project_id: &str,
    review_id: &str,
    kind: CleanupArtifactKind,
) -> Result<AssetPreview> {
    let (dir, project) = find_project(settings, project_id)?;
    let review = project
        .cleanup_reviews
        .iter()
        .find(|item| item.id == review_id)
        .ok_or("Cleanup review not found")?;
    let (relative, hash, width, height) = match kind {
        CleanupArtifactKind::Cleaned => (
            &review.cleaned_relative_path,
            &review.cleaned_sha256,
            review.cleaned_width,
            review.cleaned_height,
        ),
        CleanupArtifactKind::Normalized => (
            &review.normalized_relative_path,
            &review.normalized_sha256,
            review.normalized_width,
            review.normalized_height,
        ),
    };
    let (_, bytes) = verified_asset(&dir, relative, hash)?;
    Ok(AssetPreview {
        data_url: format!("data:image/png;base64,{}", STANDARD.encode(bytes)),
        mime_type: "image/png".into(),
        width,
        height,
    })
}

fn create_animation_in(
    settings: &Settings,
    project_id: &str,
    name: &str,
    facing: &str,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err("Animation name must be 1–60 characters".into());
    }
    if !project
        .runtime
        .source_facings
        .iter()
        .any(|item| item == facing)
    {
        return Err("Animation facing is invalid for this preset".into());
    }
    if project
        .animations
        .iter()
        .any(|item| item.facing == facing && item.name.eq_ignore_ascii_case(name))
    {
        return Err("An animation with this name and facing already exists".into());
    }
    let id = Uuid::new_v4().to_string();
    fs::create_dir_all(dir.join("animations").join(&id).join("frames_normalized"))
        .map_err(|e| format!("Cannot create animation folder: {e}"))?;
    let now = Utc::now();
    let stages = [
        "board",
        "extract",
        "native_review",
        "snap",
        "upscale",
        "cleanup",
        "normalize",
        "frames",
        "align",
        "preview",
        "export",
    ]
    .into_iter()
    .map(|stage| (stage.into(), StageState::Waiting))
    .collect();
    project.animations.push(Animation {
        id,
        name: name.into(),
        facing: facing.into(),
        frames: vec![],
        historical_frames: vec![],
        boards: vec![],
        active_board_id: None,
        extractions: vec![],
        active_extraction_id: None,
        raw_frames: vec![],
        active_raw_frame_ids: vec![],
        batch_snaps: vec![],
        batch_auto_fit_runs: vec![],
        active_batch_snap_id: None,
        upscaled_frames: vec![],
        active_upscaled_frame_ids: BTreeMap::new(),
        upscale_approved: false,
        batch_cleanups: vec![],
        active_batch_cleanup_id: None,
        alignments: vec![],
        previews: vec![],
        exports: vec![],
        active_alignment_id: None,
        active_preview_id: None,
        active_export_id: None,
        stages,
        created_at: now,
        updated_at: now,
    });
    project.updated_at = now;
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn rename_animation_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    name: &str,
) -> Result<Project> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err("Animation name must be 1–60 characters".into());
    }
    let (dir, mut project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    if animation.name == name {
        return Ok(project);
    }
    let facing = animation.facing.clone();
    if project.animations.iter().any(|item| {
        item.id != animation_id && item.facing == facing && item.name.eq_ignore_ascii_case(name)
    }) {
        return Err("An animation with this name and facing already exists".into());
    }
    let now = Utc::now();
    let animation = project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .unwrap();
    animation.name = name.into();
    animation.updated_at = now;
    project.updated_at = now;
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn import_board_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    source_path: &str,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    if !project
        .animations
        .iter()
        .any(|item| item.id == animation_id)
    {
        return Err("Animation not found".into());
    }
    let source =
        fs::canonicalize(source_path).map_err(|e| format!("Pose board is missing: {e}"))?;
    let meta = fs::metadata(&source).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > MAX_IMPORT_BYTES {
        return Err("Pose board must be an image under 32 MB".into());
    }
    let bytes = fs::read(&source).map_err(|e| format!("Cannot read pose board: {e}"))?;
    let format =
        image::guess_format(&bytes).map_err(|_| "Use a PNG or JPEG pose board".to_string())?;
    let (extension, mime) = match format {
        ImageFormat::Png => ("png", "image/png"),
        ImageFormat::Jpeg => ("jpg", "image/jpeg"),
        _ => return Err("Use a PNG or JPEG pose board".into()),
    };
    let board = image::load_from_memory_with_format(&bytes, format)
        .map_err(|_| "Pose board is damaged".to_string())?;
    if board.width() == 0 || board.height() == 0 || board.width() > 8192 || board.height() > 8192 {
        return Err("Pose board dimensions must be 1–8192 pixels".into());
    }
    let id = Uuid::new_v4().to_string();
    let relative = format!("animations/{animation_id}/board/raw/{id}.{extension}");
    let destination = dir.join(&relative);
    fs::create_dir_all(destination.parent().unwrap())
        .map_err(|e| format!("Cannot create board folder: {e}"))?;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .and_then(|mut file| {
            file.write_all(&bytes)?;
            file.sync_all()
        })
        .map_err(|e| format!("Cannot preserve pose board: {e}"))?;
    let animation = project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .unwrap();
    animation.boards.push(BoardImport {
        id: id.clone(),
        original_name: source
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        relative_path: relative,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        mime_type: mime.into(),
        width: board.width(),
        height: board.height(),
        imported_at: Utc::now(),
    });
    animation.active_board_id = Some(id);
    if animation.active_batch_cleanup_id.is_some() && !animation.frames.is_empty() {
        animation.historical_frames.append(&mut animation.frames);
    }
    animation.active_extraction_id = None;
    animation.active_raw_frame_ids.clear();
    animation.active_batch_snap_id = None;
    animation.active_upscaled_frame_ids.clear();
    animation.upscale_approved = false;
    animation.active_batch_cleanup_id = None;
    animation.active_alignment_id = None;
    animation.active_preview_id = None;
    animation.active_export_id = None;
    animation
        .stages
        .insert("board".into(), StageState::Complete);
    animation
        .stages
        .insert("extract".into(), StageState::Waiting);
    for stage in ["native_review", "snap", "upscale", "cleanup", "normalize"] {
        animation.stages.insert(stage.into(), StageState::Waiting);
    }
    animation.updated_at = Utc::now();
    project.updated_at = animation.updated_at;
    if let Err(error) = atomic_json(&dir.join("project.json"), &project) {
        let _ = fs::remove_file(destination);
        return Err(error);
    }
    Ok(project)
}

fn board_preview_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    board_id: &str,
) -> Result<AssetPreview> {
    let (dir, project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let board = animation
        .boards
        .iter()
        .find(|item| item.id == board_id)
        .ok_or("Pose board not found")?;
    let (_, bytes) = verified_asset(&dir, &board.relative_path, &board.sha256)?;
    Ok(AssetPreview {
        data_url: format!("data:{};base64,{}", board.mime_type, STANDARD.encode(bytes)),
        mime_type: board.mime_type.clone(),
        width: board.width,
        height: board.height,
    })
}

fn validate_extraction_options(options: &ExtractionOptions) -> Result<String> {
    if options.tolerance > 80
        || options.min_area == 0
        || options.min_area > 100_000
        || options.merge_gap > 32
    {
        return Err(
            "Extraction tolerance, minimum area, or merge gap is outside its supported range"
                .into(),
        );
    }
    validated_cleanup_options(&CleanupOptions {
        background: options.background.clone(),
        tolerance: options.tolerance,
        min_area: 0,
    })
}

fn run_extraction_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    options: ExtractionOptions,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let board_id = animation
        .active_board_id
        .clone()
        .ok_or("Import a pose board first")?;
    let board = animation
        .boards
        .iter()
        .find(|item| item.id == board_id)
        .ok_or("Active board is missing")?
        .clone();
    if board.width > 4096 || board.height > 4096 {
        return Err(
            "Pose board is too large for local extraction; use a board up to 4096 pixels per axis"
                .into(),
        );
    }
    let (source_path, _) = verified_asset(&dir, &board.relative_path, &board.sha256)?;
    let background = validate_extraction_options(&options)?;
    let executable = settings
        .python_executable
        .as_ref()
        .ok_or("Locate Python with Pillow and OpenCV first")?;
    let executable = fs::canonicalize(executable)
        .map_err(|_| "Configured Python is missing; locate it again".to_string())?;
    inspect_python(&executable)?;
    let id = Uuid::new_v4().to_string();
    let relative_dir = format!("animations/{animation_id}/board/reviews/{id}");
    let output_dir = dir.join(&relative_dir);
    fs::create_dir_all(&output_dir)
        .map_err(|e| format!("Cannot create extraction review folder: {e}"))?;
    project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .unwrap()
        .stages
        .insert("extract".into(), StageState::Processing);
    project.updated_at = Utc::now();
    atomic_json(&dir.join("project.json"), &project)?;
    let result = (|| -> Result<ExtractionReview> {
        let output = Command::new(&executable)
            .args(["-I", "-c", EXTRACT_SCRIPT])
            .arg(&source_path)
            .arg(&output_dir)
            .arg(&background)
            .arg(options.tolerance.to_string())
            .arg(options.min_area.to_string())
            .arg(options.merge_gap.to_string())
            .output()
            .map_err(|e| format!("Extraction processor could not run: {e}"))?;
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Extraction failed: {}",
                detail.trim().chars().take(500).collect::<String>()
            ));
        }
        let metrics: ExtractionMetrics = serde_json::from_slice(&output.stdout)
            .map_err(|_| "Extraction processor returned invalid boxes".to_string())?;
        if metrics.boxes.is_empty() || metrics.boxes.len() > 64 {
            return Err("Extraction produced an invalid number of poses".into());
        }
        let mut boxes = Vec::with_capacity(metrics.boxes.len());
        for (index, item) in metrics.boxes.into_iter().enumerate() {
            let filename = format!("candidate-{index:02}.png");
            if item.filename != filename
                || item.width == 0
                || item.height == 0
                || item.foreground_pixels == 0
                || item
                    .x
                    .checked_add(item.width)
                    .is_none_or(|v| v > board.width)
                || item
                    .y
                    .checked_add(item.height)
                    .is_none_or(|v| v > board.height)
            {
                return Err("Extraction processor returned an out-of-bounds pose".into());
            }
            let relative = format!("{relative_dir}/{filename}");
            let hash = checked_raw_png(&dir, &relative, item.width, item.height)?;
            boxes.push(DetectedFrame {
                id: Uuid::new_v4().to_string(),
                x: item.x,
                y: item.y,
                width: item.width,
                height: item.height,
                foreground_pixels: item.foreground_pixels,
                relative_path: relative,
                sha256: hash,
            });
        }
        Ok(ExtractionReview {
            id,
            board_import_id: board_id,
            boxes,
            background_hex: metrics.background_hex,
            tolerance: options.tolerance,
            min_area: options.min_area,
            merge_gap: options.merge_gap,
            created_at: Utc::now(),
            applied_at: None,
        })
    })();
    match result {
        Ok(review) => {
            let animation = project
                .animations
                .iter_mut()
                .find(|item| item.id == animation_id)
                .unwrap();
            animation.extractions.push(review);
            animation
                .stages
                .insert("extract".into(), StageState::Review);
            animation.updated_at = Utc::now();
            project.updated_at = animation.updated_at;
            atomic_json(&dir.join("project.json"), &project)?;
            Ok(project)
        }
        Err(error) => {
            if let Ok(entries) = fs::read_dir(&output_dir) {
                for entry in entries.flatten() {
                    let _ = fs::remove_file(entry.path());
                }
            }
            let _ = fs::remove_dir(&output_dir);
            let animation = project
                .animations
                .iter_mut()
                .find(|item| item.id == animation_id)
                .unwrap();
            animation
                .stages
                .insert("extract".into(), StageState::Failed);
            animation.updated_at = Utc::now();
            project.updated_at = animation.updated_at;
            atomic_json(&dir.join("project.json"), &project)?;
            Err(error)
        }
    }
}

fn extraction_candidate_preview_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    review_id: &str,
    box_id: &str,
) -> Result<AssetPreview> {
    let (dir, project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let review = animation
        .extractions
        .iter()
        .find(|item| item.id == review_id)
        .ok_or("Extraction review not found")?;
    let box_item = review
        .boxes
        .iter()
        .find(|item| item.id == box_id)
        .ok_or("Detected pose not found")?;
    let (_, bytes) = verified_asset(&dir, &box_item.relative_path, &box_item.sha256)?;
    Ok(AssetPreview {
        data_url: format!("data:image/png;base64,{}", STANDARD.encode(bytes)),
        mime_type: "image/png".into(),
        width: box_item.width,
        height: box_item.height,
    })
}

fn apply_extraction_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    review_id: &str,
    ordered_box_ids: Vec<String>,
    manual_crops: Vec<ManualCrop>,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let review = animation
        .extractions
        .iter()
        .find(|item| item.id == review_id)
        .ok_or("Extraction review not found")?
        .clone();
    if animation.active_board_id.as_ref() != Some(&review.board_import_id) {
        return Err("This extraction belongs to an older board".into());
    }
    if ordered_box_ids.len() + manual_crops.len() == 0
        || ordered_box_ids.len() + manual_crops.len() > 64
    {
        return Err("Select 1–64 poses in total".into());
    }
    let mut seen = std::collections::HashSet::new();
    for id in &ordered_box_ids {
        if !seen.insert(id) || !review.boxes.iter().any(|item| &item.id == id) {
            return Err("Selected pose list contains duplicates or unknown IDs".into());
        }
    }
    let board = animation
        .boards
        .iter()
        .find(|item| item.id == review.board_import_id)
        .ok_or("Pose board is missing")?;
    for crop in &manual_crops {
        if crop.width == 0
            || crop.height == 0
            || crop
                .x
                .checked_add(crop.width)
                .is_none_or(|v| v > board.width)
            || crop
                .y
                .checked_add(crop.height)
                .is_none_or(|v| v > board.height)
        {
            return Err("Manual crop is outside the pose board".into());
        }
    }
    let manual_image = if manual_crops.is_empty() {
        None
    } else {
        let (_, bytes) = verified_asset(&dir, &board.relative_path, &board.sha256)?;
        Some(image::load_from_memory(&bytes).map_err(|_| "Pose board is damaged".to_string())?)
    };
    let mut outputs = Vec::new();
    let mut written = Vec::new();
    let mut choices = Vec::new();
    for id in &ordered_box_ids {
        let item = review.boxes.iter().find(|item| &item.id == id).unwrap();
        let (_, bytes) = verified_asset(&dir, &item.relative_path, &item.sha256)?;
        choices.push((bytes, item.width, item.height));
    }
    for crop in &manual_crops {
        let mut cursor = Cursor::new(Vec::new());
        manual_image
            .as_ref()
            .unwrap()
            .crop_imm(crop.x, crop.y, crop.width, crop.height)
            .write_to(&mut cursor, ImageFormat::Png)
            .map_err(|e| format!("Cannot save manual crop: {e}"))?;
        choices.push((cursor.into_inner(), crop.width, crop.height));
    }
    for (index, (bytes, width, height)) in choices.into_iter().enumerate() {
        let id = Uuid::new_v4().to_string();
        let relative = format!(
            "animations/{animation_id}/frames_raw/frame_{:03}-{id}.png",
            index + 1
        );
        let destination = dir.join(&relative);
        fs::create_dir_all(destination.parent().unwrap())
            .map_err(|e| format!("Cannot create raw-frame folder: {e}"))?;
        let result = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .and_then(|mut file| {
                file.write_all(&bytes)?;
                file.sync_all()
            });
        if let Err(error) = result {
            for path in written {
                let _ = fs::remove_file(path);
            }
            let _ = fs::remove_file(destination);
            return Err(format!("Cannot preserve extracted frame: {error}"));
        }
        written.push(destination);
        outputs.push(RawAnimationFrame {
            id,
            extraction_id: review.id.clone(),
            relative_path: relative,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            width,
            height,
            created_at: Utc::now(),
        });
    }
    let animation = project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .unwrap();
    animation.active_raw_frame_ids = outputs.iter().map(|item| item.id.clone()).collect();
    if animation.active_batch_cleanup_id.is_some() && !animation.frames.is_empty() {
        animation.historical_frames.append(&mut animation.frames);
    }
    animation.raw_frames.extend(outputs);
    animation.active_extraction_id = Some(review.id.clone());
    animation.active_batch_snap_id = None;
    animation.active_upscaled_frame_ids.clear();
    animation.upscale_approved = false;
    animation.active_batch_cleanup_id = None;
    animation.active_alignment_id = None;
    animation.active_preview_id = None;
    animation.active_export_id = None;
    animation
        .extractions
        .iter_mut()
        .find(|item| item.id == review_id)
        .unwrap()
        .applied_at = Some(Utc::now());
    animation
        .stages
        .insert("extract".into(), StageState::Complete);
    animation
        .stages
        .insert("native_review".into(), StageState::Review);
    animation
        .stages
        .insert("upscale".into(), StageState::Waiting);
    for stage in ["snap", "cleanup", "normalize"] {
        animation.stages.insert(stage.into(), StageState::Waiting);
    }
    animation.updated_at = Utc::now();
    project.updated_at = animation.updated_at;
    if let Err(error) = atomic_json(&dir.join("project.json"), &project) {
        for path in written {
            let _ = fs::remove_file(path);
        }
        return Err(error);
    }
    Ok(project)
}

fn raw_animation_frame_preview_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    frame_id: &str,
) -> Result<AssetPreview> {
    let (dir, project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let frame = animation
        .raw_frames
        .iter()
        .find(|item| item.id == frame_id)
        .ok_or("Raw frame not found")?;
    let (_, bytes) = verified_asset(&dir, &frame.relative_path, &frame.sha256)?;
    Ok(AssetPreview {
        data_url: format!("data:image/png;base64,{}", STANDARD.encode(bytes)),
        mime_type: "image/png".into(),
        width: frame.width,
        height: frame.height,
    })
}

fn run_batch_snap_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    options: SnapOptions,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    if animation.stages.get("native_review") == Some(&StageState::Review) {
        return Err("Confirm Native Review before snapping recovered frames".into());
    }
    let extraction_id = animation
        .active_extraction_id
        .clone()
        .ok_or("Apply pose extraction first")?;
    let sources: Vec<_> = animation
        .active_raw_frame_ids
        .iter()
        .map(|id| {
            animation
                .raw_frames
                .iter()
                .find(|item| &item.id == id)
                .cloned()
                .ok_or("Raw frame is missing".to_string())
        })
        .collect::<Result<_>>()?;
    if sources.is_empty() || sources.len() > 64 {
        return Err("Batch must contain 1–64 raw frames".into());
    }
    let smallest = sources
        .iter()
        .map(|item| item.width.min(item.height))
        .min()
        .unwrap();
    let palette = validated_snap_options(&options, smallest, smallest)?;
    // KangiFight's 64px neutral figure is a grid-recovery target, not a scaling step.
    // One shared override keeps all frames on the same recovered pixel grid.
    let effective_pixel_size = if options.pixel_size.is_none()
        && project.preset == Preset::KangiFight
    {
        let target = project
            .runtime
            .neutral_height_target
            .ok_or("KangiFight neutral height is missing")?;
        let max_height = sources.iter().map(|item| item.height).max().unwrap();
        let size = max_height.div_ceil(target) + 1;
        if size == 0 || size > smallest {
            return Err("Raw poses cannot fit the neutral-height pixel grid; re-extract or use a manual pixel size".into());
        }
        Some(size)
    } else {
        options.pixel_size
    };
    let executable = fs::canonicalize(
        settings
            .snapper_executable
            .as_ref()
            .ok_or("Locate the Sprite Fusion CLI first")?,
    )
    .map_err(|_| "Configured Sprite Fusion CLI is missing".to_string())?;
    let version = inspect_snapper(&executable)?;
    let id = Uuid::new_v4().to_string();
    let relative_dir = format!("animations/{animation_id}/frames_snapped/{id}");
    let review_dir = dir.join(&relative_dir);
    fs::create_dir_all(&review_dir).map_err(|e| format!("Cannot create batch snap review: {e}"))?;
    for source in &sources {
        verified_asset(&dir, &source.relative_path, &source.sha256)?;
    }
    let animation = project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .unwrap();
    animation
        .stages
        .insert("snap".into(), StageState::Processing);
    atomic_json(&dir.join("project.json"), &project)?;
    let result = (|| -> Result<BatchSnapReview> {
        let mut frames = Vec::with_capacity(sources.len());
        let mut detected_pixel_size = None;
        for (index, source) in sources.iter().enumerate() {
            let output_relative = format!("{relative_dir}/frame_{:03}-native.png", index + 1);
            let output_path = dir.join(&output_relative);
            let mut command = Command::new(&executable);
            command
                .arg(dir.join(&source.relative_path))
                .arg(&output_path)
                .arg(options.colors.to_string());
            if let Some(size) = effective_pixel_size {
                command.arg("--pixel-size").arg(size.to_string());
            }
            if let Some(value) = &palette {
                command.arg("--palette").arg(value);
            }
            let output = command
                .output()
                .map_err(|e| format!("Snapper could not run on frame {}: {e}", index + 1))?;
            if !output.status.success() {
                return Err(format!(
                    "Snap failed on frame {}: {}",
                    index + 1,
                    String::from_utf8_lossy(&output.stderr)
                        .trim()
                        .chars()
                        .take(400)
                        .collect::<String>()
                ));
            }
            if detected_pixel_size.is_none() {
                detected_pixel_size = snapper_pixel_size(&output.stdout);
            }
            let bytes = fs::read(&output_path)
                .map_err(|e| format!("Frame {} snap output missing: {e}", index + 1))?;
            if bytes.len() > MAX_IMPORT_BYTES as usize
                || image::guess_format(&bytes).ok() != Some(ImageFormat::Png)
            {
                return Err(format!(
                    "Frame {} snap output is not a valid PNG",
                    index + 1
                ));
            }
            let image = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
                .map_err(|_| format!("Frame {} snap output is damaged", index + 1))?;
            if image.width() == 0
                || image.height() == 0
                || image.width() > 2048
                || image.height() > 2048
            {
                return Err(format!("Frame {} recovered grid is too large", index + 1));
            }
            frames.push(BatchFrameArtifact {
                source_frame_id: source.id.clone(),
                relative_path: output_relative,
                sha256: format!("{:x}", Sha256::digest(&bytes)),
                width: image.width(),
                height: image.height(),
                upscaled_relative_path: None,
                chroma_relative_path: None,
                outputs: Some(SnapOutputPaths {
                    native_path: format!("{relative_dir}/frame_{:03}-native.png", index + 1),
                    snapped_path: Some(format!("{relative_dir}/frame_{:03}-native.png", index + 1)),
                    upscaled_path: None,
                    chroma_path: None,
                    mode: "frame".into(),
                }),
            });
        }
        Ok(BatchSnapReview {
            id,
            extraction_id,
            source_frame_ids: sources.iter().map(|item| item.id.clone()).collect(),
            frames,
            colors: options.colors,
            pixel_size: effective_pixel_size,
            detected_pixel_size,
            auto_fit_run_id: None,
            palette,
            tool_version: version,
            created_at: Utc::now(),
            applied_at: None,
        })
    })();
    let animation = project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .unwrap();
    match result {
        Ok(review) => {
            animation.batch_snaps.push(review);
            animation.stages.insert("snap".into(), StageState::Review);
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&review_dir);
            animation.stages.insert("snap".into(), StageState::Failed);
            atomic_json(&dir.join("project.json"), &project)?;
            return Err(error);
        }
    }
    animation.updated_at = Utc::now();
    project.updated_at = animation.updated_at;
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn rank_batch_auto_fit_candidates(run: &BatchAutoFitRun) -> Vec<&BatchAutoFitCandidate> {
    let mut valid: Vec<_> = run
        .candidates
        .iter()
        .filter(|candidate| candidate.fits)
        .collect();
    valid.sort_by(|a, b| {
        let target_key = |candidate: &BatchAutoFitCandidate| {
            candidate.worst_target_delta.map_or((0, 0), |delta| {
                ((delta > run.neutral_tolerance_px as u32) as u8, delta)
            })
        };
        target_key(a)
            .cmp(&target_key(b))
            .then_with(|| {
                (a.pixel_size - run.starting_pixel_size)
                    .abs()
                    .total_cmp(&(b.pixel_size - run.starting_pixel_size).abs())
            })
            .then_with(|| b.total_foreground_pixels.cmp(&a.total_foreground_pixels))
            .then_with(|| a.total_speckle_loss.cmp(&b.total_speckle_loss))
            .then_with(|| a.pixel_size.total_cmp(&b.pixel_size))
    });
    valid
}

fn run_batch_auto_fit_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    cleanup_options: CleanupOptions,
) -> Result<Project> {
    let (dir, project) = find_project(settings, project_id)?;
    if project.workflow.upscale_mode != UpscaleMode::Automatic {
        return Err("Auto-fit uses native snapped frames; switch from manual upscale or revise the upscaled inputs".into());
    }
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let active_id = animation
        .active_batch_snap_id
        .as_ref()
        .ok_or("Apply a batch snap before auto-fit")?;
    let active = animation
        .batch_snaps
        .iter()
        .find(|review| &review.id == active_id)
        .ok_or("Active batch snap is invalid")?;
    let extraction_id = animation
        .active_extraction_id
        .clone()
        .ok_or("Apply frame recovery before auto-fit")?;
    let source_ids = animation.active_raw_frame_ids.clone();
    let source_limit = source_ids
        .iter()
        .filter_map(|id| animation.raw_frames.iter().find(|frame| &frame.id == id))
        .map(|frame| frame.width.min(frame.height))
        .min()
        .ok_or("No raw frames are active")?;
    let original_stage = animation
        .stages
        .get("snap")
        .cloned()
        .unwrap_or(StageState::Complete);
    let background = validated_cleanup_options(&cleanup_options)?;
    let python = fs::canonicalize(
        settings
            .python_executable
            .as_ref()
            .ok_or("Locate a Python environment with Pillow and OpenCV first")?,
    )
    .map_err(|_| "Configured Python environment is missing; locate it again".to_string())?;
    inspect_python(&python)?;
    let snapper = fs::canonicalize(
        settings
            .snapper_executable
            .as_ref()
            .ok_or("Locate the Sprite Fusion CLI first")?,
    )
    .map_err(|_| "Configured Sprite Fusion CLI is missing; locate it again".to_string())?;
    inspect_snapper(&snapper)?;
    let runtime = project.runtime.clone();
    let fringe_cleanup = project.workflow.green_fringe_despeckle;
    let colors = active.colors;
    let palette = active.palette.clone();
    let initial_override = active.pixel_size;
    let run_id = Uuid::new_v4().to_string();
    let mut run = BatchAutoFitRun {
        id: run_id.clone(),
        extraction_id,
        source_frame_ids: source_ids,
        cell_width: runtime.cell_width,
        cell_height: runtime.cell_height,
        pivot_x: runtime.pivot_x,
        pivot_y: runtime.pivot_y,
        neutral_height_target: runtime.neutral_height_target,
        neutral_tolerance_px: runtime.neutral_tolerance_px,
        starting_pixel_size: 0.0,
        candidates: vec![],
        recommended_review_ids: vec![],
        selected_review_id: None,
        created_at: Utc::now(),
    };
    for attempt in 0..MAX_AUTO_FIT_ATTEMPTS {
        let pixel_size = if attempt == 0 {
            initial_override
        } else {
            let next = run.starting_pixel_size.floor() as u32 + attempt as u32;
            if next > source_limit {
                break;
            }
            Some(next)
        };
        let updated = match run_batch_snap_in(
            settings,
            project_id,
            animation_id,
            SnapOptions {
                colors,
                pixel_size,
                palette: palette.clone(),
            },
        ) {
            Ok(project) => project,
            Err(error) => {
                let (dir, mut restored) = find_project(settings, project_id)?;
                if let Some(animation) = restored
                    .animations
                    .iter_mut()
                    .find(|item| item.id == animation_id)
                {
                    animation
                        .stages
                        .insert("snap".into(), original_stage.clone());
                }
                atomic_json(&dir.join("project.json"), &restored)?;
                return Err(error);
            }
        };
        let mut updated = updated;
        let review = updated
            .animations
            .iter_mut()
            .find(|item| item.id == animation_id)
            .unwrap()
            .batch_snaps
            .last_mut()
            .ok_or("Batch auto-fit snap review was not saved")?;
        review.auto_fit_run_id = Some(run_id.clone());
        let actual_size = review
            .detected_pixel_size
            .or_else(|| pixel_size.map(f64::from))
            .unwrap_or(
                source_limit as f64 / review.frames[0].width.min(review.frames[0].height) as f64,
            );
        if attempt == 0 {
            run.starting_pixel_size = actual_size;
        }
        let review_id = review.id.clone();
        let frame_artifacts = review.frames.clone();
        let mut frames = Vec::with_capacity(frame_artifacts.len());
        for frame in &frame_artifacts {
            let (native_path, _) = verified_asset(&dir, &frame.relative_path, &frame.sha256)?;
            let metrics = match inspect_snap_fit(
                &python,
                &native_path,
                &cleanup_options,
                &background,
                &runtime,
                fringe_cleanup,
            ) {
                Ok(metrics) => metrics,
                Err(error) => {
                    updated
                        .animations
                        .iter_mut()
                        .find(|item| item.id == animation_id)
                        .unwrap()
                        .stages
                        .insert("snap".into(), original_stage.clone());
                    atomic_json(&dir.join("project.json"), &updated)?;
                    return Err(error);
                }
            };
            frames.push(BatchAutoFitFrame {
                source_frame_id: frame.source_frame_id.clone(),
                foreground_width: metrics.foreground_width,
                foreground_height: metrics.foreground_height,
                foreground_pixels: metrics.foreground_pixels,
                removed_speckle_pixels: metrics.removed_speckle_pixels,
                fits: metrics.fits,
            });
        }
        let candidate = BatchAutoFitCandidate {
            review_id,
            pixel_size: actual_size,
            fits: frames.iter().all(|frame| frame.fits),
            worst_target_delta: runtime.neutral_height_target.map(|target| {
                frames
                    .iter()
                    .map(|frame| frame.foreground_height.abs_diff(target))
                    .max()
                    .unwrap_or(0)
            }),
            total_foreground_pixels: frames
                .iter()
                .map(|frame| frame.foreground_pixels as u64)
                .sum(),
            total_speckle_loss: frames
                .iter()
                .map(|frame| frame.removed_speckle_pixels as u64)
                .sum(),
            frames,
        };
        run.candidates.push(candidate);
        let animation = updated
            .animations
            .iter_mut()
            .find(|item| item.id == animation_id)
            .unwrap();
        if let Some(existing) = animation
            .batch_auto_fit_runs
            .iter_mut()
            .find(|item| item.id == run_id)
        {
            *existing = run.clone();
        } else {
            animation.batch_auto_fit_runs.push(run.clone());
        }
        animation
            .stages
            .insert("snap".into(), original_stage.clone());
        atomic_json(&dir.join("project.json"), &updated)?;
        let valid_count = run.candidates.iter().filter(|item| item.fits).count();
        let on_target_count = run
            .candidates
            .iter()
            .filter(|item| {
                item.fits
                    && item
                        .worst_target_delta
                        .is_some_and(|delta| delta <= runtime.neutral_tolerance_px as u32)
            })
            .count();
        if valid_count >= MAX_AUTO_FIT_CHOICES
            && (runtime.neutral_height_target.is_none() || on_target_count >= MAX_AUTO_FIT_CHOICES)
        {
            break;
        }
    }
    let mut finished = find_project(settings, project_id)?.1;
    let ranked = rank_batch_auto_fit_candidates(&run);
    run.recommended_review_ids = ranked
        .iter()
        .take(MAX_AUTO_FIT_CHOICES)
        .map(|item| item.review_id.clone())
        .collect();
    run.selected_review_id = run.recommended_review_ids.first().cloned();
    let found = run.selected_review_id.is_some();
    let animation = finished
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .unwrap();
    let saved = animation
        .batch_auto_fit_runs
        .iter_mut()
        .find(|item| item.id == run_id)
        .ok_or("Batch auto-fit run was not saved")?;
    *saved = run;
    atomic_json(&dir.join("project.json"), &finished)?;
    if !found {
        return Err(format!(
            "No lossless snap candidate fits the locked {}×{} cell at pivot ({},{}). Try a different source generation or manually choose a coarser pixel size.",
            runtime.cell_width, runtime.cell_height, runtime.pivot_x, runtime.pivot_y
        ));
    }
    Ok(finished)
}

fn confirm_native_review_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    accepted_frame_ids: Vec<String>,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter_mut()
        .find(|a| a.id == animation_id)
        .ok_or("Animation not found")?;
    if animation.active_extraction_id.is_none() {
        return Err("Apply Frame Recovery first".into());
    }
    if accepted_frame_ids.is_empty()
        || accepted_frame_ids.len() > animation.active_raw_frame_ids.len()
    {
        return Err("Accept at least one recovered frame".into());
    }
    let mut seen = std::collections::HashSet::new();
    for id in &accepted_frame_ids {
        if !seen.insert(id) || !animation.active_raw_frame_ids.contains(id) {
            return Err("Native review contains an unknown or duplicate frame".into());
        }
        let frame = animation.raw_frames.iter().find(|f| &f.id == id).unwrap();
        verified_asset(&dir, &frame.relative_path, &frame.sha256)?;
    }
    animation.active_raw_frame_ids = accepted_frame_ids;
    if animation.active_batch_cleanup_id.is_some() && !animation.frames.is_empty() {
        animation.historical_frames.append(&mut animation.frames);
    }
    animation.active_batch_snap_id = None;
    animation.active_batch_cleanup_id = None;
    animation.active_upscaled_frame_ids.clear();
    animation.upscale_approved = false;
    animation
        .stages
        .insert("native_review".into(), StageState::Complete);
    for stage in ["snap", "upscale", "cleanup", "normalize"] {
        animation.stages.insert(stage.into(), StageState::Waiting);
    }
    animation.updated_at = Utc::now();
    project.updated_at = animation.updated_at;
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn replace_raw_frame_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    frame_id: &str,
    source_path: Option<&str>,
    crop: Option<ManualCrop>,
) -> Result<Project> {
    if source_path.is_some() == crop.is_some() {
        return Err("Choose either a replacement image or board crop".into());
    }
    let (dir, mut project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|a| a.id == animation_id)
        .ok_or("Animation not found")?;
    let index = animation
        .active_raw_frame_ids
        .iter()
        .position(|id| id == frame_id)
        .ok_or("Frame is not active in Native Review")?;
    let old = animation
        .raw_frames
        .iter()
        .find(|f| f.id == frame_id)
        .ok_or("Recovered frame is missing")?;
    let extraction_id = old.extraction_id.clone();
    let bytes = if let Some(path) = source_path {
        let source =
            fs::canonicalize(path).map_err(|e| format!("Replacement image missing: {e}"))?;
        let data = fs::read(&source).map_err(|e| format!("Cannot read replacement image: {e}"))?;
        if data.len() > MAX_IMPORT_BYTES as usize {
            return Err("Replacement image exceeds 32 MB".into());
        }
        let format = image::guess_format(&data)
            .map_err(|_| "Replacement must be PNG or JPEG".to_string())?;
        if format != ImageFormat::Png && format != ImageFormat::Jpeg {
            return Err("Replacement must be PNG or JPEG".into());
        }
        let image = image::load_from_memory_with_format(&data, format)
            .map_err(|_| "Replacement image is damaged".to_string())?;
        let mut cursor = Cursor::new(Vec::new());
        image
            .write_to(&mut cursor, ImageFormat::Png)
            .map_err(|e| format!("Cannot encode replacement PNG: {e}"))?;
        cursor.into_inner()
    } else {
        let crop = crop.unwrap();
        let board = animation
            .boards
            .iter()
            .find(|b| Some(&b.id) == animation.active_board_id.as_ref())
            .ok_or("Active pose board is missing")?;
        if crop.width == 0
            || crop.height == 0
            || crop
                .x
                .checked_add(crop.width)
                .is_none_or(|x| x > board.width)
            || crop
                .y
                .checked_add(crop.height)
                .is_none_or(|y| y > board.height)
        {
            return Err("Re-crop bounds are outside the pose board".into());
        }
        let (_, data) = verified_asset(&dir, &board.relative_path, &board.sha256)?;
        let image =
            image::load_from_memory(&data).map_err(|_| "Pose board is damaged".to_string())?;
        let mut cursor = Cursor::new(Vec::new());
        image
            .crop_imm(crop.x, crop.y, crop.width, crop.height)
            .write_to(&mut cursor, ImageFormat::Png)
            .map_err(|e| format!("Cannot encode re-crop: {e}"))?;
        cursor.into_inner()
    };
    let image = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
        .map_err(|_| "Recovered image is damaged".to_string())?;
    if image.width() == 0 || image.height() == 0 || image.width() > 4096 || image.height() > 4096 {
        return Err("Recovered frame must be 1–4096 pixels on each axis".into());
    }
    let id = Uuid::new_v4().to_string();
    let relative = format!(
        "animations/{animation_id}/frames_raw/frame_{:03}-{id}.png",
        index + 1
    );
    let destination = dir.join(&relative);
    fs::create_dir_all(destination.parent().unwrap())
        .map_err(|e| format!("Cannot create raw frame folder: {e}"))?;
    if !fs::canonicalize(destination.parent().unwrap())
        .map_err(|e| e.to_string())?
        .starts_with(&dir)
    {
        return Err("Raw frame folder escapes the project".into());
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .and_then(|mut f| {
            f.write_all(&bytes)?;
            f.sync_all()
        })
        .map_err(|e| format!("Cannot preserve corrected frame: {e}"))?;
    let animation = project
        .animations
        .iter_mut()
        .find(|a| a.id == animation_id)
        .unwrap();
    animation.raw_frames.push(RawAnimationFrame {
        id: id.clone(),
        extraction_id,
        relative_path: relative,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        width: image.width(),
        height: image.height(),
        created_at: Utc::now(),
    });
    animation.active_raw_frame_ids[index] = id;
    if animation.active_batch_cleanup_id.is_some() && !animation.frames.is_empty() {
        animation.historical_frames.append(&mut animation.frames);
    }
    animation.active_batch_snap_id = None;
    animation.active_batch_cleanup_id = None;
    animation.active_upscaled_frame_ids.clear();
    animation.upscale_approved = false;
    animation.active_alignment_id = None;
    animation.active_preview_id = None;
    animation.active_export_id = None;
    animation
        .stages
        .insert("native_review".into(), StageState::Review);
    for stage in [
        "snap",
        "upscale",
        "cleanup",
        "normalize",
        "frames",
        "align",
        "preview",
        "export",
    ] {
        animation.stages.insert(stage.into(), StageState::Stale);
    }
    animation.updated_at = Utc::now();
    project.updated_at = animation.updated_at;
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn apply_batch_snap_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    review_id: &str,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let review = animation
        .batch_snaps
        .iter()
        .find(|item| item.id == review_id)
        .ok_or("Batch snap review not found")?
        .clone();
    if animation.active_extraction_id.as_ref() != Some(&review.extraction_id)
        || animation.active_raw_frame_ids != review.source_frame_ids
    {
        return Err("Raw frames have changed; snap again".into());
    }
    if let Some(run_id) = &review.auto_fit_run_id {
        let run = animation
            .batch_auto_fit_runs
            .iter()
            .find(|item| &item.id == run_id)
            .ok_or("Batch auto-fit review metadata is missing")?;
        let candidate = run
            .candidates
            .iter()
            .find(|item| item.review_id == review.id)
            .ok_or("Batch auto-fit candidate metrics are missing")?;
        if !candidate.fits
            || run.cell_width != project.runtime.cell_width
            || run.cell_height != project.runtime.cell_height
            || run.pivot_x != project.runtime.pivot_x
            || run.pivot_y != project.runtime.pivot_y
            || run.source_frame_ids != animation.active_raw_frame_ids
        {
            return Err(
                "This batch auto-fit candidate does not fit the current runtime contract".into(),
            );
        }
    }
    for frame in &review.frames {
        verified_asset(&dir, &frame.relative_path, &frame.sha256)?;
    }
    animation.active_batch_snap_id = Some(review.id.clone());
    if animation.active_batch_cleanup_id.is_some() && !animation.frames.is_empty() {
        animation.historical_frames.append(&mut animation.frames);
    }
    animation.active_upscaled_frame_ids.clear();
    animation.upscale_approved = false;
    animation.active_batch_cleanup_id = None;
    animation.active_alignment_id = None;
    animation.active_preview_id = None;
    animation.active_export_id = None;
    animation
        .batch_snaps
        .iter_mut()
        .find(|item| item.id == review_id)
        .unwrap()
        .applied_at = Some(Utc::now());
    animation.stages.insert("snap".into(), StageState::Complete);
    animation.stages.insert(
        "upscale".into(),
        if project.workflow.upscale_mode == UpscaleMode::ManualHandoff {
            StageState::NeedsInput
        } else {
            StageState::Complete
        },
    );
    animation
        .stages
        .insert("cleanup".into(), StageState::Waiting);
    animation
        .stages
        .insert("normalize".into(), StageState::Waiting);
    for stage in ["frames", "align", "preview", "export"] {
        animation.stages.insert(stage.into(), StageState::Stale);
    }
    animation.updated_at = Utc::now();
    project.updated_at = animation.updated_at;
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn import_upscaled_frame_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    source_frame_id: &str,
    source_path: &str,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    if project.workflow.upscale_mode != UpscaleMode::ManualHandoff {
        return Err("Enable Manual external handoff in project settings first".into());
    }
    let animation = project
        .animations
        .iter()
        .find(|a| a.id == animation_id)
        .ok_or("Animation not found")?;
    let snap_id = animation
        .active_batch_snap_id
        .clone()
        .ok_or("Apply snapped frames first")?;
    let snap = animation
        .batch_snaps
        .iter()
        .find(|r| r.id == snap_id)
        .ok_or("Active snap review is missing")?;
    let index = snap
        .frames
        .iter()
        .position(|f| f.source_frame_id == source_frame_id)
        .ok_or("Source frame does not belong to the active snap")?;
    let source =
        fs::canonicalize(source_path).map_err(|e| format!("Upscaled PNG is missing: {e}"))?;
    let meta = fs::metadata(&source).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > MAX_IMPORT_BYTES {
        return Err("Upscaled frame must be a PNG under 32 MB".into());
    }
    let bytes = fs::read(&source).map_err(|e| format!("Cannot read upscaled frame: {e}"))?;
    if image::guess_format(&bytes).ok() != Some(ImageFormat::Png) {
        return Err("Upscaled frame must be PNG".into());
    }
    let image = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
        .map_err(|_| "Upscaled PNG is damaged".to_string())?;
    if image.width() == 0 || image.height() == 0 || image.width() > 8192 || image.height() > 8192 {
        return Err("Upscaled frame dimensions must be 1–8192 pixels".into());
    }
    let animation = project
        .animations
        .iter_mut()
        .find(|a| a.id == animation_id)
        .unwrap();
    let version = animation
        .upscaled_frames
        .iter()
        .filter(|f| f.snap_review_id == snap_id && f.source_frame_id == source_frame_id)
        .map(|f| f.version)
        .max()
        .unwrap_or(0)
        + 1;
    let relative = format!(
        "animations/{animation_id}/frames_upscaled/frame_{:03}-upscaled-v{version}.png",
        index + 1
    );
    let destination = dir.join(&relative);
    fs::create_dir_all(destination.parent().unwrap())
        .map_err(|e| format!("Cannot create upscale folder: {e}"))?;
    if !fs::canonicalize(destination.parent().unwrap())
        .map_err(|e| e.to_string())?
        .starts_with(&dir)
    {
        return Err("Upscaled frame folder escapes the project".into());
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .and_then(|mut f| {
            f.write_all(&bytes)?;
            f.sync_all()
        })
        .map_err(|e| format!("Cannot preserve upscaled frame: {e}"))?;
    let id = Uuid::new_v4().to_string();
    animation.upscaled_frames.push(UpscaledFrame {
        id: id.clone(),
        snap_review_id: snap_id,
        source_frame_id: source_frame_id.into(),
        version,
        original_name: source
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        relative_path: relative,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        width: image.width(),
        height: image.height(),
        imported_at: Utc::now(),
    });
    animation
        .active_upscaled_frame_ids
        .insert(source_frame_id.into(), id);
    animation.upscale_approved = false;
    animation.active_batch_cleanup_id = None;
    animation.active_alignment_id = None;
    animation.active_preview_id = None;
    animation.active_export_id = None;
    if !animation.frames.is_empty() {
        animation.historical_frames.append(&mut animation.frames);
    }
    animation
        .stages
        .insert("upscale".into(), StageState::NeedsInput);
    for stage in [
        "cleanup",
        "normalize",
        "frames",
        "align",
        "preview",
        "export",
    ] {
        animation.stages.insert(stage.into(), StageState::Stale);
    }
    animation.updated_at = Utc::now();
    project.updated_at = animation.updated_at;
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn approve_upscale_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    if project.workflow.upscale_mode != UpscaleMode::ManualHandoff {
        return Err("Manual external handoff is not enabled".into());
    }
    let animation = project
        .animations
        .iter_mut()
        .find(|a| a.id == animation_id)
        .ok_or("Animation not found")?;
    let snap_id = animation
        .active_batch_snap_id
        .as_ref()
        .ok_or("Apply snapped frames first")?;
    let snap = animation
        .batch_snaps
        .iter()
        .find(|r| &r.id == snap_id)
        .ok_or("Active snap review is missing")?;
    let missing = snap
        .frames
        .iter()
        .filter(|frame| {
            !animation
                .active_upscaled_frame_ids
                .contains_key(&frame.source_frame_id)
        })
        .count();
    if missing > 0 {
        return Err(format!(
            "UPSCALE_MISSING_FRAMES: {missing} of {} upscaled frames are still missing",
            snap.frames.len()
        ));
    }
    for id in animation.active_upscaled_frame_ids.values() {
        let frame = animation
            .upscaled_frames
            .iter()
            .find(|f| &f.id == id)
            .ok_or("Upscaled frame record missing")?;
        verified_asset(&dir, &frame.relative_path, &frame.sha256)?;
    }
    animation.upscale_approved = true;
    animation
        .stages
        .insert("upscale".into(), StageState::Complete);
    animation
        .stages
        .insert("cleanup".into(), StageState::Waiting);
    animation
        .stages
        .insert("normalize".into(), StageState::Waiting);
    animation.updated_at = Utc::now();
    project.updated_at = animation.updated_at;
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn export_snapped_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    destination_path: &str,
    source_frame_ids: Vec<String>,
) -> Result<String> {
    let (dir, project) = find_project(settings, project_id)?;
    let destination_root = fs::canonicalize(destination_path)
        .map_err(|e| format!("Export folder is unavailable: {e}"))?;
    if !destination_root.is_dir() {
        return Err("Choose an export folder".into());
    }
    let animation = project
        .animations
        .iter()
        .find(|a| a.id == animation_id)
        .ok_or("Animation not found")?;
    let snap_id = animation
        .active_batch_snap_id
        .as_ref()
        .ok_or("Apply snapped frames first")?;
    let snap = animation
        .batch_snaps
        .iter()
        .find(|r| &r.id == snap_id)
        .ok_or("Active snap review missing")?;
    let selected: Vec<_> = if source_frame_ids.is_empty() {
        snap.frames.iter().enumerate().collect()
    } else {
        snap.frames
            .iter()
            .enumerate()
            .filter(|(_, f)| source_frame_ids.contains(&f.source_frame_id))
            .collect()
    };
    if selected.is_empty()
        || selected.len()
            != source_frame_ids.len().max(if source_frame_ids.is_empty() {
                snap.frames.len()
            } else {
                0
            })
    {
        return Err("Selected frames are not in the active snap".into());
    }
    let folder = destination_root.join(format!(
        "sprite-studio-{}-{}",
        &animation.id[..8],
        &snap.id[..8]
    ));
    fs::create_dir(&folder).map_err(|e| format!("Cannot create a new handoff folder: {e}"))?;
    for (index, frame) in selected {
        let (_, bytes) = verified_asset(&dir, &frame.relative_path, &frame.sha256)?;
        let path = folder.join(format!("frame_{:03}-native.png", index + 1));
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .and_then(|mut f| f.write_all(&bytes))
            .map_err(|e| format!("Cannot export snapped frame: {e}"))?;
    }
    Ok(folder.to_string_lossy().into_owned())
}

fn upscaled_preview_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    upscale_id: &str,
) -> Result<AssetPreview> {
    let (dir, project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|a| a.id == animation_id)
        .ok_or("Animation not found")?;
    let frame = animation
        .upscaled_frames
        .iter()
        .find(|f| f.id == upscale_id)
        .ok_or("Upscaled frame not found")?;
    let (_, bytes) = verified_asset(&dir, &frame.relative_path, &frame.sha256)?;
    Ok(AssetPreview {
        data_url: format!("data:image/png;base64,{}", STANDARD.encode(bytes)),
        mime_type: "image/png".into(),
        width: frame.width,
        height: frame.height,
    })
}

fn run_batch_cleanup_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    options: CleanupOptions,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let snap_id = animation
        .active_batch_snap_id
        .as_ref()
        .ok_or("Apply batch snap first")?;
    let snap = animation
        .batch_snaps
        .iter()
        .find(|item| &item.id == snap_id)
        .ok_or("Active batch snap is missing")?
        .clone();
    let mut inputs = Vec::with_capacity(snap.frames.len());
    for frame in &snap.frames {
        if project.workflow.upscale_mode == UpscaleMode::ManualHandoff {
            if !animation.upscale_approved {
                return Err(
                    "UPSCALE_REQUIRED: Approve every upscaled frame before background cleaning"
                        .into(),
                );
            }
            let id = animation
                .active_upscaled_frame_ids
                .get(&frame.source_frame_id)
                .ok_or("UPSCALE_MISSING_FRAMES: An upscaled frame is missing")?;
            let upscaled = animation
                .upscaled_frames
                .iter()
                .find(|item| &item.id == id && item.snap_review_id == snap.id)
                .ok_or("Active upscaled frame is stale")?;
            inputs.push((
                frame.source_frame_id.clone(),
                upscaled.relative_path.clone(),
                upscaled.sha256.clone(),
                upscaled.width,
                upscaled.height,
                Some(id.clone()),
            ));
        } else {
            inputs.push((
                frame.source_frame_id.clone(),
                frame.relative_path.clone(),
                frame.sha256.clone(),
                frame.width,
                frame.height,
                None,
            ));
        }
    }
    let background = validated_cleanup_options(&options)?;
    let executable = fs::canonicalize(
        settings
            .python_executable
            .as_ref()
            .ok_or("Locate a Python environment with Pillow and OpenCV first")?,
    )
    .map_err(|_| "Configured Python environment is missing".to_string())?;
    let environment = inspect_python(&executable)?;
    for frame in &inputs {
        verified_asset(&dir, &frame.1, &frame.2)?;
    }
    let id = Uuid::new_v4().to_string();
    let relative_dir = format!("animations/{animation_id}/batch_cleanup/{id}");
    let review_dir = dir.join(&relative_dir);
    let clean_dir = dir.join(format!("animations/{animation_id}/frames_clean/{id}"));
    let normalized_dir = dir.join(format!(
        "animations/{animation_id}/frames_normalized/reviews/{id}"
    ));
    fs::create_dir_all(&review_dir)
        .map_err(|e| format!("Cannot create batch cleanup review: {e}"))?;
    fs::create_dir_all(&clean_dir).map_err(|e| format!("Cannot create clean frame folder: {e}"))?;
    fs::create_dir_all(&normalized_dir)
        .map_err(|e| format!("Cannot create normalized review folder: {e}"))?;
    let runtime = project.runtime.clone();
    let animation = project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .unwrap();
    animation
        .stages
        .insert("cleanup".into(), StageState::Processing);
    animation
        .stages
        .insert("normalize".into(), StageState::Waiting);
    atomic_json(&dir.join("project.json"), &project)?;
    let result = (|| -> Result<BatchCleanupReview> {
        let mut frames = Vec::with_capacity(inputs.len());
        for (index, frame) in inputs.iter().enumerate() {
            let cleaned_relative_path = format!(
                "animations/{animation_id}/frames_clean/{id}/frame_{:03}.png",
                index + 1
            );
            let normalized_relative_path = format!(
                "animations/{animation_id}/frames_normalized/reviews/{id}/frame_{:03}.png",
                index + 1
            );
            let output = Command::new(&executable)
                .args(["-I", "-c", CLEANUP_SCRIPT])
                .arg(dir.join(&frame.1))
                .arg(dir.join(&cleaned_relative_path))
                .arg(dir.join(&normalized_relative_path))
                .arg(&background)
                .arg(options.tolerance.to_string())
                .arg(options.min_area.to_string())
                .arg(runtime.cell_width.to_string())
                .arg(runtime.cell_height.to_string())
                .arg(runtime.pivot_x.to_string())
                .arg(runtime.pivot_y.to_string())
                .arg(if project.workflow.green_fringe_despeckle {
                    "1"
                } else {
                    "0"
                })
                .output()
                .map_err(|e| format!("Cleanup could not run on frame {}: {e}", index + 1))?;
            if !output.status.success() {
                return Err(format!(
                    "Cleanup failed on frame {}: {}",
                    index + 1,
                    String::from_utf8_lossy(&output.stderr)
                        .trim()
                        .chars()
                        .take(400)
                        .collect::<String>()
                ));
            }
            let metrics: CleanupMetrics = serde_json::from_slice(&output.stdout)
                .map_err(|_| format!("Frame {} cleanup metrics invalid", index + 1))?;
            if metrics.processor_version != "sprite-studio-cleanup-1"
                || metrics.foreground_pixels == 0
            {
                return Err(format!("Frame {} cleanup output invalid", index + 1));
            }
            let cleaned_sha256 = checked_png(
                &dir,
                &cleaned_relative_path,
                metrics.bbox_width,
                metrics.bbox_height,
            )?;
            let normalized_sha256 = checked_png(
                &dir,
                &normalized_relative_path,
                runtime.cell_width,
                runtime.cell_height,
            )?;
            frames.push(BatchCleanupFrame {
                source_frame_id: frame.0.clone(),
                cleaned_relative_path,
                cleaned_sha256,
                normalized_relative_path,
                normalized_sha256,
                foreground_pixels: metrics.foreground_pixels,
            });
        }
        Ok(BatchCleanupReview {
            id,
            snap_review_id: snap.id.clone(),
            upscale_frame_ids: inputs.iter().filter_map(|item| item.5.clone()).collect(),
            frames,
            background_hex: background,
            tolerance: options.tolerance,
            min_area: options.min_area,
            green_fringe_despeckle: project.workflow.green_fringe_despeckle,
            python_environment: environment,
            created_at: Utc::now(),
            applied_at: None,
        })
    })();
    let animation = project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .unwrap();
    match result {
        Ok(review) => {
            animation.batch_cleanups.push(review);
            animation
                .stages
                .insert("cleanup".into(), StageState::Review);
            animation
                .stages
                .insert("normalize".into(), StageState::Waiting);
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&review_dir);
            let _ = fs::remove_dir_all(&clean_dir);
            let _ = fs::remove_dir_all(&normalized_dir);
            animation
                .stages
                .insert("cleanup".into(), StageState::Failed);
            animation
                .stages
                .insert("normalize".into(), StageState::Waiting);
            atomic_json(&dir.join("project.json"), &project)?;
            return Err(error);
        }
    }
    animation.updated_at = Utc::now();
    project.updated_at = animation.updated_at;
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn approve_batch_clean_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    review_id: &str,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let review = animation
        .batch_cleanups
        .last()
        .ok_or("Run background clean first")?;
    if review.id != review_id || animation.stages.get("cleanup") != Some(&StageState::Review) {
        return Err("Choose the current cleaned-frame review".into());
    }
    if animation.active_batch_snap_id.as_ref() != Some(&review.snap_review_id) {
        return Err("Snapped frames changed; clean again".into());
    }
    if project.workflow.upscale_mode == UpscaleMode::ManualHandoff {
        let current: Vec<String> = animation
            .active_raw_frame_ids
            .iter()
            .filter_map(|source| animation.active_upscaled_frame_ids.get(source).cloned())
            .collect();
        if !animation.upscale_approved || current != review.upscale_frame_ids {
            return Err("Upscaled inputs changed; clean again".into());
        }
    } else if !review.upscale_frame_ids.is_empty() {
        return Err("Cleanup was made in manual mode; clean again".into());
    }
    if review.green_fringe_despeckle != project.workflow.green_fringe_despeckle {
        return Err("Fringe cleanup setting changed; clean again".into());
    }
    for frame in &review.frames {
        verified_asset(&dir, &frame.cleaned_relative_path, &frame.cleaned_sha256)?;
    }
    let animation = project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .unwrap();
    animation
        .stages
        .insert("cleanup".into(), StageState::Complete);
    animation
        .stages
        .insert("normalize".into(), StageState::Review);
    animation.updated_at = Utc::now();
    project.updated_at = animation.updated_at;
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn apply_batch_cleanup_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    review_id: &str,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let review = animation
        .batch_cleanups
        .iter()
        .find(|item| item.id == review_id)
        .ok_or("Batch cleanup review not found")?
        .clone();
    if animation.active_batch_cleanup_id.as_deref() == Some(review_id) {
        return Ok(project);
    }
    if animation.stages.get("cleanup") != Some(&StageState::Complete)
        || animation.batch_cleanups.last().map(|item| item.id.as_str()) != Some(review_id)
    {
        return Err("Apply the cleaned-frame review before normalizing".into());
    }
    if animation.active_batch_snap_id.as_ref() != Some(&review.snap_review_id) {
        return Err("Snapped frames have changed; clean again".into());
    }
    if project.workflow.upscale_mode == UpscaleMode::ManualHandoff {
        let current: Vec<String> = animation
            .active_raw_frame_ids
            .iter()
            .filter_map(|source| animation.active_upscaled_frame_ids.get(source).cloned())
            .collect();
        if !animation.upscale_approved || current != review.upscale_frame_ids {
            return Err("Upscaled inputs changed; clean again".into());
        }
    } else if !review.upscale_frame_ids.is_empty() {
        return Err("Cleanup was made from manual upscale inputs; clean again".into());
    }
    if review.green_fringe_despeckle != project.workflow.green_fringe_despeckle {
        return Err("Fringe cleanup setting changed; clean again".into());
    }
    let runtime = &project.runtime;
    let mut new_frames = Vec::with_capacity(review.frames.len());
    let mut written = Vec::new();
    for (index, frame) in review.frames.iter().enumerate() {
        verified_asset(&dir, &frame.cleaned_relative_path, &frame.cleaned_sha256)?;
        let (_, bytes) = verified_asset(
            &dir,
            &frame.normalized_relative_path,
            &frame.normalized_sha256,
        )?;
        checked_png(
            &dir,
            &frame.normalized_relative_path,
            runtime.cell_width,
            runtime.cell_height,
        )?;
        let id = Uuid::new_v4().to_string();
        let relative_path = format!("animations/{animation_id}/frames_normalized/{id}.png");
        let destination = dir.join(&relative_path);
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .and_then(|mut file| file.write_all(&bytes))
            .map_err(|e| {
                for path in &written {
                    let _ = fs::remove_file(path);
                }
                format!("Cannot preserve normalized frame: {e}")
            })?;
        written.push(destination);
        new_frames.push(AnimationFrame {
            id,
            original_name: format!("extracted-{:03}.png", index + 1),
            relative_path,
            sha256: frame.normalized_sha256.clone(),
            width: runtime.cell_width,
            height: runtime.cell_height,
            imported_at: Utc::now(),
        });
    }
    let animation = project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .unwrap();
    if !animation.frames.is_empty() {
        animation.historical_frames.append(&mut animation.frames);
    }
    animation.frames.extend(new_frames);
    animation.active_batch_cleanup_id = Some(review.id.clone());
    animation
        .batch_cleanups
        .iter_mut()
        .find(|item| item.id == review_id)
        .unwrap()
        .applied_at = Some(Utc::now());
    for stage in ["cleanup", "normalize", "frames"] {
        animation.stages.insert(stage.into(), StageState::Complete);
    }
    for stage in ["align", "preview", "export"] {
        animation.stages.insert(stage.into(), StageState::Waiting);
    }
    animation.active_alignment_id = None;
    animation.active_preview_id = None;
    animation.active_export_id = None;
    animation.updated_at = Utc::now();
    project.updated_at = animation.updated_at;
    if let Err(error) = atomic_json(&dir.join("project.json"), &project) {
        for path in written {
            let _ = fs::remove_file(path);
        }
        return Err(error);
    }
    Ok(project)
}

fn batch_frame_preview_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    review_id: &str,
    source_frame_id: &str,
    kind: &str,
) -> Result<AssetPreview> {
    let (dir, project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let (relative, hash) = match kind {
        "snapped" => {
            let review = animation
                .batch_snaps
                .iter()
                .find(|item| item.id == review_id)
                .ok_or("Batch snap review not found")?;
            let frame = review
                .frames
                .iter()
                .find(|item| item.source_frame_id == source_frame_id)
                .ok_or("Snapped frame not found")?;
            (&frame.relative_path, &frame.sha256)
        }
        "cleaned" | "normalized" => {
            let review = animation
                .batch_cleanups
                .iter()
                .find(|item| item.id == review_id)
                .ok_or("Batch cleanup review not found")?;
            let frame = review
                .frames
                .iter()
                .find(|item| item.source_frame_id == source_frame_id)
                .ok_or("Cleaned frame not found")?;
            if kind == "cleaned" {
                (&frame.cleaned_relative_path, &frame.cleaned_sha256)
            } else {
                (&frame.normalized_relative_path, &frame.normalized_sha256)
            }
        }
        _ => return Err("Unknown batch preview kind".into()),
    };
    let (_, bytes) = verified_asset(&dir, relative, hash)?;
    let (width, height) = image::ImageReader::with_format(Cursor::new(&bytes), ImageFormat::Png)
        .into_dimensions()
        .map_err(|_| "Batch preview image is damaged".to_string())?;
    Ok(AssetPreview {
        data_url: format!("data:image/png;base64,{}", STANDARD.encode(bytes)),
        mime_type: "image/png".into(),
        width,
        height,
    })
}

fn validate_normalized_frame(bytes: &[u8], width: u32, height: u32) -> Result<()> {
    if image::guess_format(bytes).ok() != Some(ImageFormat::Png) {
        return Err("Animation frames must be PNG images".into());
    }
    let frame = image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .map_err(|_| "Animation frame is damaged".to_string())?
        .to_rgba8();
    if frame.width() != width || frame.height() != height {
        return Err(format!(
            "Animation frame must be exactly {width} × {height} pixels"
        ));
    }
    if frame
        .pixels()
        .any(|pixel| pixel.0[3] != 0 && pixel.0[3] != 255)
    {
        return Err("Animation frame has partial alpha; use binary transparency".into());
    }
    if !frame.pixels().any(|pixel| pixel.0[3] == 255) {
        return Err("Animation frame is empty".into());
    }
    Ok(())
}

fn import_animation_frames_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    paths: Vec<String>,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    if paths.is_empty() || paths.len() > 32 || animation.frames.len() + paths.len() > 64 {
        return Err("Choose 1–32 PNG frames, with at most 64 per animation".into());
    }
    let mut pending = Vec::with_capacity(paths.len());
    for value in &paths {
        let source = fs::canonicalize(value).map_err(|e| format!("Frame file is missing: {e}"))?;
        let meta = fs::metadata(&source).map_err(|e| e.to_string())?;
        if !meta.is_file() || meta.len() > 16 * 1024 * 1024 {
            return Err("Each frame must be a PNG under 16 MB".into());
        }
        let bytes = fs::read(&source).map_err(|e| format!("Cannot read frame: {e}"))?;
        validate_normalized_frame(
            &bytes,
            project.runtime.cell_width,
            project.runtime.cell_height,
        )?;
        let id = Uuid::new_v4().to_string();
        let relative = format!("animations/{animation_id}/frames_normalized/{id}.png");
        pending.push((source, bytes, id, relative));
    }
    let mut written = Vec::new();
    let mut records = Vec::new();
    for (source, bytes, id, relative) in pending {
        let destination = dir.join(&relative);
        let write_result = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .and_then(|mut file| {
                file.write_all(&bytes)?;
                file.sync_all()
            });
        if let Err(error) = write_result {
            for path in written {
                let _ = fs::remove_file(path);
            }
            let _ = fs::remove_file(&destination);
            return Err(format!("Cannot copy animation frame: {error}"));
        }
        written.push(destination);
        records.push(AnimationFrame {
            id,
            original_name: source
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            relative_path: relative,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            width: project.runtime.cell_width,
            height: project.runtime.cell_height,
            imported_at: Utc::now(),
        });
    }
    let animation = project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .unwrap();
    animation.frames.extend(records);
    animation.active_alignment_id = None;
    animation.active_preview_id = None;
    animation.active_export_id = None;
    animation
        .stages
        .insert("frames".into(), StageState::Complete);
    for stage in ["align", "preview", "export"] {
        animation.stages.insert(stage.into(), StageState::Waiting);
    }
    animation.updated_at = Utc::now();
    project.updated_at = animation.updated_at;
    if let Err(error) = atomic_json(&dir.join("project.json"), &project) {
        for path in written {
            let _ = fs::remove_file(path);
        }
        return Err(error);
    }
    Ok(project)
}

fn animation_frame_preview_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    frame_id: &str,
) -> Result<AssetPreview> {
    let (dir, project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let frame = animation
        .frames
        .iter()
        .find(|item| item.id == frame_id)
        .ok_or("Frame not found")?;
    let (_, bytes) = verified_asset(&dir, &frame.relative_path, &frame.sha256)?;
    Ok(AssetPreview {
        data_url: format!("data:image/png;base64,{}", STANDARD.encode(bytes)),
        mime_type: "image/png".into(),
        width: frame.width,
        height: frame.height,
    })
}

fn frame_fits_after_offset(bytes: &[u8], x: i32, y: i32, width: u32, height: u32) -> Result<bool> {
    let image = image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .map_err(|_| "Frame is damaged".to_string())?
        .to_rgba8();
    let mut bounds = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for (px, py, color) in image.enumerate_pixels() {
        if color.0[3] == 255 {
            bounds.0 = bounds.0.min(px as i32);
            bounds.1 = bounds.1.min(py as i32);
            bounds.2 = bounds.2.max(px as i32);
            bounds.3 = bounds.3.max(py as i32);
        }
    }
    Ok(bounds.0 != i32::MAX
        && bounds.0 + x >= 0
        && bounds.1 + y >= 0
        && bounds.2 + x < width as i32
        && bounds.3 + y < height as i32)
}

fn propose_alignment_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    offsets: Vec<FrameOffset>,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    if animation.frames.is_empty() || offsets.len() != animation.frames.len() {
        return Err("Provide one offset for every frame".into());
    }
    for (frame, offset) in animation.frames.iter().zip(&offsets) {
        if frame.id != offset.frame_id || offset.x.abs() > 128 || offset.y.abs() > 128 {
            return Err("Offsets must follow frame order and stay within 128 pixels".into());
        }
        let (_, bytes) = verified_asset(&dir, &frame.relative_path, &frame.sha256)?;
        if !frame_fits_after_offset(&bytes, offset.x, offset.y, frame.width, frame.height)? {
            return Err(format!(
                "Frame {} would clip the fixed cell; reduce its offset",
                frame.original_name
            ));
        }
    }
    let animation = project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .unwrap();
    animation.alignments.push(AlignmentReview {
        id: Uuid::new_v4().to_string(),
        offsets,
        relative_path: None,
        sha256: None,
        created_at: Utc::now(),
        applied_at: None,
    });
    animation.stages.insert("align".into(), StageState::Review);
    animation.updated_at = Utc::now();
    project.updated_at = animation.updated_at;
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn apply_alignment_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    review_id: &str,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let review = animation
        .alignments
        .iter_mut()
        .find(|item| item.id == review_id)
        .ok_or("Alignment review not found")?;
    if review.offsets.len() != animation.frames.len()
        || !review
            .offsets
            .iter()
            .zip(&animation.frames)
            .all(|(offset, frame)| offset.frame_id == frame.id)
    {
        return Err("Alignment review is stale; propose it again".into());
    }
    review.applied_at = Some(Utc::now());
    let relative = format!("animations/{animation_id}/alignments/{review_id}/alignment.json");
    let path = dir.join(&relative);
    fs::create_dir_all(path.parent().unwrap())
        .map_err(|e| format!("Cannot create alignment folder: {e}"))?;
    let payload =
        serde_json::json!({"schemaVersion": 1, "reviewId": review_id, "offsets": review.offsets});
    atomic_json(&path, &payload)?;
    let bytes = fs::read(&path).map_err(|e| format!("Cannot verify alignment file: {e}"))?;
    review.relative_path = Some(relative);
    review.sha256 = Some(format!("{:x}", Sha256::digest(&bytes)));
    animation.active_alignment_id = Some(review_id.into());
    animation.active_preview_id = None;
    animation.active_export_id = None;
    animation
        .stages
        .insert("align".into(), StageState::Complete);
    animation
        .stages
        .insert("preview".into(), StageState::Waiting);
    animation
        .stages
        .insert("export".into(), StageState::Waiting);
    animation.updated_at = Utc::now();
    project.updated_at = animation.updated_at;
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn propose_preview_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    fps: u8,
    looping: bool,
) -> Result<Project> {
    if !(1..=60).contains(&fps) {
        return Err("Preview FPS must be 1–60".into());
    }
    let (dir, mut project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let alignment_id = animation
        .active_alignment_id
        .clone()
        .ok_or("Apply an alignment before preview")?;
    animation.previews.push(AnimationPreviewReview {
        id: Uuid::new_v4().to_string(),
        alignment_id,
        fps,
        looping,
        created_at: Utc::now(),
        applied_at: None,
    });
    animation
        .stages
        .insert("preview".into(), StageState::Review);
    animation.updated_at = Utc::now();
    project.updated_at = animation.updated_at;
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn apply_preview_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    review_id: &str,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let review = animation
        .previews
        .iter_mut()
        .find(|item| item.id == review_id)
        .ok_or("Preview review not found")?;
    if Some(&review.alignment_id) != animation.active_alignment_id.as_ref() {
        return Err("Preview uses an older alignment; propose it again".into());
    }
    review.applied_at = Some(Utc::now());
    animation.active_preview_id = Some(review_id.into());
    animation.active_export_id = None;
    animation
        .stages
        .insert("preview".into(), StageState::Complete);
    animation
        .stages
        .insert("export".into(), StageState::Waiting);
    animation.updated_at = Utc::now();
    project.updated_at = animation.updated_at;
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn run_export_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    options: ExportOptions,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?
        .clone();
    if animation.frames.is_empty() {
        return Err("Import animation frames first".into());
    }
    let alignment_id = animation
        .active_alignment_id
        .clone()
        .ok_or("Apply alignment before export")?;
    let preview_id = animation
        .active_preview_id
        .clone()
        .ok_or("Apply a timed preview before export")?;
    let alignment = animation
        .alignments
        .iter()
        .find(|item| item.id == alignment_id)
        .ok_or("Applied alignment is missing")?;
    let preview = animation
        .previews
        .iter()
        .find(|item| item.id == preview_id && item.alignment_id == alignment_id)
        .ok_or("Applied preview is missing")?;
    let max_columns = if project.preset == Preset::KangiFight {
        64
    } else {
        16
    };
    if options.columns == 0
        || options.columns > max_columns
        || options.padding > 64
        || options.spacing > 32
    {
        return Err("Columns, padding, or spacing are outside the supported range".into());
    }
    if project.preset == Preset::KangiFight
        && (options.columns as usize != animation.frames.len()
            || options.padding != 0
            || options.spacing != 0)
    {
        return Err(
            "KangiFight draft exports must be a horizontal strip with no padding or spacing".into(),
        );
    }
    let width = project.runtime.cell_width;
    let height = project.runtime.cell_height;
    let columns = (options.columns as usize).min(animation.frames.len());
    let rows = animation.frames.len().div_ceil(columns);
    let expected_width = 2 * options.padding as u32
        + columns as u32 * width
        + (columns as u32 - 1) * options.spacing as u32;
    let expected_height = 2 * options.padding as u32
        + rows as u32 * height
        + (rows as u32 - 1) * options.spacing as u32;
    if expected_width > 8192 || expected_height > 8192 {
        return Err("Export sheet exceeds 8192 pixels on one axis".into());
    }
    let executable = settings
        .python_executable
        .as_ref()
        .ok_or("Locate Python with Pillow and OpenCV first")?;
    let executable = fs::canonicalize(executable)
        .map_err(|_| "Configured Python is missing; locate it again".to_string())?;
    inspect_python(&executable)?;
    let mut frame_configs = Vec::with_capacity(animation.frames.len());
    for (frame, offset) in animation.frames.iter().zip(&alignment.offsets) {
        if frame.id != offset.frame_id {
            return Err("Alignment no longer matches frame order".into());
        }
        let (path, bytes) = verified_asset(&dir, &frame.relative_path, &frame.sha256)?;
        if !frame_fits_after_offset(&bytes, offset.x, offset.y, width, height)? {
            return Err("An aligned frame would clip the cell".into());
        }
        frame_configs.push(serde_json::json!({"path": path, "x": offset.x, "y": offset.y}));
    }
    let export_id = Uuid::new_v4().to_string();
    let relative_dir = format!("exports/animations/{animation_id}/{export_id}");
    let export_dir = project_export_dir(&dir, &format!("animations/{animation_id}/{export_id}"))?;
    let sheet_relative = format!("{relative_dir}/spritesheet.png");
    let gif_relative = format!("{relative_dir}/preview.gif");
    let manifest_relative = format!("{relative_dir}/manifest.json");
    let sheet_path = export_dir.join("spritesheet.png");
    let gif_path = export_dir.join("preview.gif");
    let manifest_path = export_dir.join("manifest.json");
    project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .unwrap()
        .stages
        .insert("export".into(), StageState::Processing);
    project.updated_at = Utc::now();
    atomic_json(&dir.join("project.json"), &project)?;

    let result = (|| -> Result<ExportReview> {
        let config = serde_json::json!({
            "cellWidth": width, "cellHeight": height, "columns": columns, "padding": options.padding,
            "spacing": options.spacing, "fps": preview.fps, "looping": preview.looping,
            "frames": frame_configs, "sheetPath": sheet_path, "gifPath": gif_path,
        });
        let mut process = Command::new(&executable)
            .args(["-I", "-c", EXPORT_SCRIPT])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Export processor could not start: {e}"))?;
        process
            .stdin
            .take()
            .ok_or("Cannot send export configuration")?
            .write_all(&serde_json::to_vec(&config).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Cannot send export configuration: {e}"))?;
        let output = process
            .wait_with_output()
            .map_err(|e| format!("Export processor failed: {e}"))?;
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Export failed: {}",
                detail.trim().chars().take(500).collect::<String>()
            ));
        }
        let metrics: ExportMetrics = serde_json::from_slice(&output.stdout)
            .map_err(|_| "Export processor returned invalid metrics".to_string())?;
        if metrics.sheet_width != expected_width
            || metrics.sheet_height != expected_height
            || metrics.rows as usize != rows
            || metrics.gif_frame_count == 0
            || metrics.gif_frame_count > animation.frames.len()
            || metrics.gif_total_duration_ms
                != metrics.gif_duration_ms * animation.frames.len() as u32
            || metrics.gif_duration_ms == 0
        {
            return Err("Export processor returned wrong dimensions or frame count".into());
        }
        let sheet_hash = checked_png(&dir, &sheet_relative, expected_width, expected_height)?;
        let gif_bytes = fs::read(&gif_path).map_err(|e| format!("Export GIF is missing: {e}"))?;
        if gif_bytes.len() > MAX_IMPORT_BYTES as usize
            || !(gif_bytes.starts_with(b"GIF87a") || gif_bytes.starts_with(b"GIF89a"))
        {
            return Err("Export GIF is invalid or too large".into());
        }
        let gif_hash = format!("{:x}", Sha256::digest(&gif_bytes));
        let frames: Vec<_> = animation.frames.iter().zip(&alignment.offsets).enumerate().map(|(index, (frame, offset))| {
            let x = options.padding as u32 + (index % columns) as u32 * (width + options.spacing as u32);
            let y = options.padding as u32 + (index / columns) as u32 * (height + options.spacing as u32);
            serde_json::json!({"index": index, "frameId": frame.id, "sourceSha256": frame.sha256,
                "offset": {"x": offset.x, "y": offset.y}, "rect": {"x": x, "y": y, "width": width, "height": height}})
        }).collect();
        let manifest = serde_json::json!({
            "schemaVersion": 1, "approvalState": "DRAFT", "timingAuthority": "study_only",
            "projectId": project.id, "animationId": animation.id, "animation": animation.name,
            "facing": animation.facing, "preset": project.preset, "frameWidth": width, "frameHeight": height,
            "pivot": {"x": project.runtime.pivot_x, "y": project.runtime.pivot_y, "mode": project.workflow.anchor_mode},
            "frameCount": animation.frames.len(), "fps": preview.fps, "looping": preview.looping,
            "gifDurationMs": metrics.gif_duration_ms, "columns": columns, "rows": rows,
            "gifEncodedFrames": metrics.gif_frame_count,
            "padding": options.padding, "spacing": options.spacing,
            "sheet": {"file": "spritesheet.png", "sha256": sheet_hash},
            "previewGif": {"file": "preview.gif", "sha256": gif_hash}, "frames": frames,
        });
        atomic_json(&manifest_path, &manifest)?;
        let manifest_bytes =
            fs::read(&manifest_path).map_err(|e| format!("Cannot read manifest: {e}"))?;
        Ok(ExportReview {
            id: export_id,
            alignment_id,
            preview_id,
            sheet_relative_path: sheet_relative,
            sheet_sha256: sheet_hash,
            sheet_width: expected_width,
            sheet_height: expected_height,
            gif_relative_path: gif_relative,
            gif_sha256: gif_hash,
            manifest_relative_path: manifest_relative,
            manifest_sha256: format!("{:x}", Sha256::digest(&manifest_bytes)),
            columns: options.columns,
            rows: rows as u16,
            padding: options.padding,
            spacing: options.spacing,
            gif_duration_ms: metrics.gif_duration_ms,
            gif_encoded_frames: metrics.gif_frame_count as u16,
            created_at: Utc::now(),
            applied_at: None,
        })
    })();
    match result {
        Ok(review) => {
            let animation = project
                .animations
                .iter_mut()
                .find(|item| item.id == animation_id)
                .unwrap();
            animation.exports.push(review);
            animation.stages.insert("export".into(), StageState::Review);
            animation.updated_at = Utc::now();
            project.updated_at = animation.updated_at;
            atomic_json(&dir.join("project.json"), &project)?;
            Ok(project)
        }
        Err(error) => {
            for path in [&sheet_path, &gif_path, &manifest_path] {
                let _ = fs::remove_file(path);
            }
            let _ = fs::remove_dir(&export_dir);
            let animation = project
                .animations
                .iter_mut()
                .find(|item| item.id == animation_id)
                .unwrap();
            animation.stages.insert("export".into(), StageState::Failed);
            animation.updated_at = Utc::now();
            project.updated_at = animation.updated_at;
            atomic_json(&dir.join("project.json"), &project)?;
            Err(error)
        }
    }
}

fn apply_export_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    review_id: &str,
) -> Result<Project> {
    let (dir, mut project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter_mut()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let review = animation
        .exports
        .iter_mut()
        .find(|item| item.id == review_id)
        .ok_or("Export review not found")?;
    if animation.active_alignment_id.as_ref() != Some(&review.alignment_id)
        || animation.active_preview_id.as_ref() != Some(&review.preview_id)
    {
        return Err("Export uses older alignment or timing; run export again".into());
    }
    for (path, hash) in [
        (&review.sheet_relative_path, &review.sheet_sha256),
        (&review.gif_relative_path, &review.gif_sha256),
        (&review.manifest_relative_path, &review.manifest_sha256),
    ] {
        verified_asset(&dir, path, hash)?;
    }
    review.applied_at = Some(Utc::now());
    animation.active_export_id = Some(review_id.into());
    animation
        .stages
        .insert("export".into(), StageState::Complete);
    animation.updated_at = Utc::now();
    project.updated_at = animation.updated_at;
    atomic_json(&dir.join("project.json"), &project)?;
    Ok(project)
}

fn export_asset_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    review_id: &str,
    kind: ExportArtifactKind,
) -> Result<AssetPreview> {
    let (dir, project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let review = animation
        .exports
        .iter()
        .find(|item| item.id == review_id)
        .ok_or("Export review not found")?;
    let (path, hash, mime, width, height) = match kind {
        ExportArtifactKind::Sheet => (
            &review.sheet_relative_path,
            &review.sheet_sha256,
            "image/png",
            review.sheet_width,
            review.sheet_height,
        ),
        ExportArtifactKind::Gif => (
            &review.gif_relative_path,
            &review.gif_sha256,
            "image/gif",
            project.runtime.cell_width,
            project.runtime.cell_height,
        ),
    };
    let (_, bytes) = verified_asset(&dir, path, hash)?;
    Ok(AssetPreview {
        data_url: format!("data:{mime};base64,{}", STANDARD.encode(bytes)),
        mime_type: mime.into(),
        width,
        height,
    })
}

fn export_manifest_in(
    settings: &Settings,
    project_id: &str,
    animation_id: &str,
    review_id: &str,
) -> Result<serde_json::Value> {
    let (dir, project) = find_project(settings, project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let review = animation
        .exports
        .iter()
        .find(|item| item.id == review_id)
        .ok_or("Export review not found")?;
    let (_, bytes) = verified_asset(
        &dir,
        &review.manifest_relative_path,
        &review.manifest_sha256,
    )?;
    serde_json::from_slice(&bytes).map_err(|e| format!("Export manifest is damaged: {e}"))
}

fn snap_preview_in(
    settings: &Settings,
    project_id: &str,
    review_id: &str,
    kind: SnapArtifactKind,
) -> Result<AssetPreview> {
    let (dir, project) = find_project(settings, project_id)?;
    let review = project
        .snap_reviews
        .iter()
        .find(|review| review.id == review_id)
        .ok_or("Snap review not found")?;
    let (relative, hash, width, height) = match kind {
        SnapArtifactKind::Native => (
            &review.native_relative_path,
            &review.native_sha256,
            review.native_width,
            review.native_height,
        ),
        SnapArtifactKind::Reference => (
            &review.reference_relative_path,
            &review.reference_sha256,
            review.reference_width,
            review.reference_height,
        ),
    };
    let (_, bytes) = verified_asset(&dir, relative, hash)?;
    Ok(AssetPreview {
        data_url: format!("data:image/png;base64,{}", STANDARD.encode(bytes)),
        mime_type: "image/png".into(),
        width,
        height,
    })
}

fn recover_interrupted(dir: &Path, project: &mut Project) -> Result<()> {
    let mut changed = false;
    for record in &mut project.imports {
        if matches!(record.stages.get("snap"), Some(StageState::Processing)) {
            record.stages.insert("snap".into(), StageState::Failed);
            changed = true;
        }
        if matches!(record.stages.get("cleanup"), Some(StageState::Processing))
            || matches!(record.stages.get("normalize"), Some(StageState::Processing))
        {
            record.stages.insert("cleanup".into(), StageState::Failed);
            record.stages.insert("normalize".into(), StageState::Failed);
            changed = true;
        }
    }
    for animation in &mut project.animations {
        if matches!(
            animation.stages.get("extract"),
            Some(StageState::Processing)
        ) {
            animation
                .stages
                .insert("extract".into(), StageState::Failed);
            changed = true;
        }
        if matches!(animation.stages.get("snap"), Some(StageState::Processing)) {
            animation.stages.insert("snap".into(), StageState::Failed);
            changed = true;
        }
        if matches!(
            animation.stages.get("cleanup"),
            Some(StageState::Processing)
        ) || matches!(
            animation.stages.get("normalize"),
            Some(StageState::Processing)
        ) {
            animation
                .stages
                .insert("cleanup".into(), StageState::Failed);
            animation
                .stages
                .insert("normalize".into(), StageState::Failed);
            changed = true;
        }
        if matches!(animation.stages.get("export"), Some(StageState::Processing)) {
            animation.stages.insert("export".into(), StageState::Failed);
            changed = true;
        }
    }
    if changed {
        project.updated_at = Utc::now();
        atomic_json(&dir.join("project.json"), project)?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_app_state(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
) -> Result<AppState> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    let (last_project, startup_error) = match &settings.last_project_id {
        Some(id) => match find_project(&settings, id) {
            Ok((dir, mut project)) => {
                recover_interrupted(&dir, &mut project)?;
                (Some(project), None)
            }
            Err(e) => (None, Some(e)),
        },
        None => (None, None),
    };
    Ok(AppState {
        settings,
        last_project,
        startup_error,
    })
}

#[tauri::command]
pub fn set_workspace(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    path: String,
) -> Result<Settings> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    store_workspace(&settings_file(&app)?, &path)
}

#[tauri::command]
pub fn list_projects(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
) -> Result<Vec<ProjectSummary>> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    let mut projects = vec![];
    for entry in fs::read_dir(projects_dir(&settings)?).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        if let Ok(project) = read_project(&entry.path()) {
            projects.push(ProjectSummary {
                id: project.id,
                name: project.name,
                preset: project.preset,
                updated_at: project.updated_at,
                imported_count: project.imports.len(),
            });
        }
    }
    projects.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(projects)
}

#[tauri::command]
pub fn list_deleted_projects(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
) -> Result<Vec<ProjectSummary>> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    list_deleted_projects_in(&settings)
}

#[tauri::command]
pub fn delete_project(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
) -> Result<Settings> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    archive_project_in(&settings_file(&app)?, &project_id)
}

#[tauri::command]
pub fn restore_project(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    restore_project_in(&settings, &project_id)
}

#[tauri::command]
pub fn create_project(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    name: String,
    preset: Preset,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let path = settings_file(&app)?;
    let mut settings = read_settings(&path)?;
    let project = create_project_in(&settings, &name, preset)?;
    settings.last_project_id = Some(project.id.clone());
    atomic_json(&path, &settings)?;
    Ok(project)
}

#[tauri::command]
pub fn rename_project(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    name: String,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    rename_project_in(&settings, &project_id, &name)
}

#[tauri::command]
pub fn open_project(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let path = settings_file(&app)?;
    let mut settings = read_settings(&path)?;
    let (dir, mut project) = find_project(&settings, &project_id)?;
    recover_interrupted(&dir, &mut project)?;
    settings.last_project_id = Some(project.id.clone());
    atomic_json(&path, &settings)?;
    Ok(project)
}

#[tauri::command]
pub fn import_anchor(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    facing: String,
    source_path: String,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    import_into(&settings, &project_id, &facing, &source_path)
}

#[tauri::command]
pub fn configure_snapper(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    path: String,
) -> Result<Settings> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    configure_snapper_in(&settings_file(&app)?, &path)
}

#[tauri::command]
pub async fn run_snap(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    facing: String,
    options: SnapOptions,
) -> Result<Project> {
    let lock = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock
            .lock()
            .map_err(|_| "Project operation lock is unavailable")?;
        let settings = read_settings(&settings_file(&app)?)?;
        run_snap_in(&settings, &project_id, &facing, options)
    })
    .await
    .map_err(|e| format!("Snap task did not complete: {e}"))?
}

#[tauri::command]
pub async fn run_auto_fit(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    facing: String,
    cleanup_options: CleanupOptions,
) -> Result<Project> {
    let lock = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock
            .lock()
            .map_err(|_| "Project operation lock is unavailable")?;
        let settings = read_settings(&settings_file(&app)?)?;
        run_auto_fit_in(&settings, &project_id, &facing, cleanup_options)
    })
    .await
    .map_err(|e| format!("Auto-fit task did not complete: {e}"))?
}

#[tauri::command]
pub fn apply_snap(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    review_id: String,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    apply_snap_in(&settings, &project_id, &review_id)
}

#[tauri::command]
pub fn read_snap_preview(
    app: tauri::AppHandle,
    project_id: String,
    review_id: String,
    kind: SnapArtifactKind,
) -> Result<AssetPreview> {
    let settings = read_settings(&settings_file(&app)?)?;
    snap_preview_in(&settings, &project_id, &review_id, kind)
}

#[tauri::command]
pub fn configure_python(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    path: String,
) -> Result<Settings> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    configure_python_in(&settings_file(&app)?, &path)
}

#[tauri::command]
pub async fn run_cleanup(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    facing: String,
    options: CleanupOptions,
) -> Result<Project> {
    let lock = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock
            .lock()
            .map_err(|_| "Project operation lock is unavailable")?;
        let settings = read_settings(&settings_file(&app)?)?;
        run_cleanup_in(&settings, &project_id, &facing, options)
    })
    .await
    .map_err(|e| format!("Cleanup task did not complete: {e}"))?
}

#[tauri::command]
pub fn apply_cleanup(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    review_id: String,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    apply_cleanup_in(&settings, &project_id, &review_id)
}

#[tauri::command]
pub fn read_cleanup_preview(
    app: tauri::AppHandle,
    project_id: String,
    review_id: String,
    kind: CleanupArtifactKind,
) -> Result<AssetPreview> {
    let settings = read_settings(&settings_file(&app)?)?;
    cleanup_preview_in(&settings, &project_id, &review_id, kind)
}

#[tauri::command]
pub fn create_animation(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    name: String,
    facing: String,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    create_animation_in(&settings, &project_id, &name, &facing)
}

#[tauri::command]
pub fn rename_animation(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    name: String,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    rename_animation_in(&settings, &project_id, &animation_id, &name)
}

#[tauri::command]
pub fn import_animation_frames(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    paths: Vec<String>,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    import_animation_frames_in(&settings, &project_id, &animation_id, paths)
}

#[tauri::command]
pub fn read_animation_frame_preview(
    app: tauri::AppHandle,
    project_id: String,
    animation_id: String,
    frame_id: String,
) -> Result<AssetPreview> {
    let settings = read_settings(&settings_file(&app)?)?;
    animation_frame_preview_in(&settings, &project_id, &animation_id, &frame_id)
}

#[tauri::command]
pub fn propose_alignment(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    offsets: Vec<FrameOffset>,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    propose_alignment_in(&settings, &project_id, &animation_id, offsets)
}

#[tauri::command]
pub fn apply_alignment(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    review_id: String,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    apply_alignment_in(&settings, &project_id, &animation_id, &review_id)
}

#[tauri::command]
pub fn propose_animation_preview(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    fps: u8,
    looping: bool,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    propose_preview_in(&settings, &project_id, &animation_id, fps, looping)
}

#[tauri::command]
pub fn apply_animation_preview(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    review_id: String,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    apply_preview_in(&settings, &project_id, &animation_id, &review_id)
}

#[tauri::command]
pub async fn run_export(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    options: ExportOptions,
) -> Result<Project> {
    let lock = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock
            .lock()
            .map_err(|_| "Project operation lock is unavailable")?;
        let settings = read_settings(&settings_file(&app)?)?;
        run_export_in(&settings, &project_id, &animation_id, options)
    })
    .await
    .map_err(|e| format!("Export task did not complete: {e}"))?
}

#[tauri::command]
pub fn apply_export(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    review_id: String,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    apply_export_in(&settings, &project_id, &animation_id, &review_id)
}

#[tauri::command]
pub fn read_export_asset(
    app: tauri::AppHandle,
    project_id: String,
    animation_id: String,
    review_id: String,
    kind: ExportArtifactKind,
) -> Result<AssetPreview> {
    let settings = read_settings(&settings_file(&app)?)?;
    export_asset_in(&settings, &project_id, &animation_id, &review_id, kind)
}

#[tauri::command]
pub fn read_export_manifest(
    app: tauri::AppHandle,
    project_id: String,
    animation_id: String,
    review_id: String,
) -> Result<serde_json::Value> {
    let settings = read_settings(&settings_file(&app)?)?;
    export_manifest_in(&settings, &project_id, &animation_id, &review_id)
}

#[tauri::command]
pub fn import_pose_board(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    source_path: String,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    import_board_in(&settings, &project_id, &animation_id, &source_path)
}

#[tauri::command]
pub fn read_pose_board_preview(
    app: tauri::AppHandle,
    project_id: String,
    animation_id: String,
    board_id: String,
) -> Result<AssetPreview> {
    let settings = read_settings(&settings_file(&app)?)?;
    board_preview_in(&settings, &project_id, &animation_id, &board_id)
}

#[tauri::command]
pub async fn run_pose_extraction(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    options: ExtractionOptions,
) -> Result<Project> {
    let lock = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock
            .lock()
            .map_err(|_| "Project operation lock is unavailable")?;
        let settings = read_settings(&settings_file(&app)?)?;
        run_extraction_in(&settings, &project_id, &animation_id, options)
    })
    .await
    .map_err(|e| format!("Extraction task did not complete: {e}"))?
}

#[tauri::command]
pub fn read_extraction_candidate(
    app: tauri::AppHandle,
    project_id: String,
    animation_id: String,
    review_id: String,
    box_id: String,
) -> Result<AssetPreview> {
    let settings = read_settings(&settings_file(&app)?)?;
    extraction_candidate_preview_in(&settings, &project_id, &animation_id, &review_id, &box_id)
}

#[tauri::command]
pub fn apply_pose_extraction(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    review_id: String,
    ordered_box_ids: Vec<String>,
    manual_crops: Vec<ManualCrop>,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    apply_extraction_in(
        &settings,
        &project_id,
        &animation_id,
        &review_id,
        ordered_box_ids,
        manual_crops,
    )
}

#[tauri::command]
pub fn read_raw_animation_frame(
    app: tauri::AppHandle,
    project_id: String,
    animation_id: String,
    frame_id: String,
) -> Result<AssetPreview> {
    let settings = read_settings(&settings_file(&app)?)?;
    raw_animation_frame_preview_in(&settings, &project_id, &animation_id, &frame_id)
}

#[tauri::command]
pub async fn run_batch_snap(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    options: SnapOptions,
) -> Result<Project> {
    let lock = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock
            .lock()
            .map_err(|_| "Project operation lock is unavailable")?;
        let settings = read_settings(&settings_file(&app)?)?;
        run_batch_snap_in(&settings, &project_id, &animation_id, options)
    })
    .await
    .map_err(|e| format!("Batch snap task did not complete: {e}"))?
}

#[tauri::command]
pub async fn run_batch_auto_fit(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    cleanup_options: CleanupOptions,
) -> Result<Project> {
    let lock = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock
            .lock()
            .map_err(|_| "Project operation lock is unavailable")?;
        let settings = read_settings(&settings_file(&app)?)?;
        run_batch_auto_fit_in(&settings, &project_id, &animation_id, cleanup_options)
    })
    .await
    .map_err(|e| format!("Batch auto-fit task did not complete: {e}"))?
}

#[tauri::command]
pub fn apply_batch_snap(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    review_id: String,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    apply_batch_snap_in(&settings, &project_id, &animation_id, &review_id)
}

#[tauri::command]
pub async fn run_batch_cleanup(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    options: CleanupOptions,
) -> Result<Project> {
    let lock = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock
            .lock()
            .map_err(|_| "Project operation lock is unavailable")?;
        let settings = read_settings(&settings_file(&app)?)?;
        run_batch_cleanup_in(&settings, &project_id, &animation_id, options)
    })
    .await
    .map_err(|e| format!("Batch cleanup task did not complete: {e}"))?
}

#[tauri::command]
pub fn apply_batch_cleanup(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    review_id: String,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    apply_batch_cleanup_in(&settings, &project_id, &animation_id, &review_id)
}

#[tauri::command]
pub fn approve_batch_clean(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    review_id: String,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    approve_batch_clean_in(&settings, &project_id, &animation_id, &review_id)
}

#[tauri::command]
pub fn read_batch_frame_preview(
    app: tauri::AppHandle,
    project_id: String,
    animation_id: String,
    review_id: String,
    source_frame_id: String,
    kind: String,
) -> Result<AssetPreview> {
    let settings = read_settings(&settings_file(&app)?)?;
    batch_frame_preview_in(
        &settings,
        &project_id,
        &animation_id,
        &review_id,
        &source_frame_id,
        &kind,
    )
}

#[tauri::command]
pub fn update_workflow(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    workflow: WorkflowSettings,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    update_workflow_in(&settings, &project_id, workflow)
}

#[tauri::command]
pub fn update_runtime(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    cell_width: u32,
    cell_height: u32,
    pivot_x: u32,
    pivot_y: u32,
    anchor_mode: AnchorMode,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    update_runtime_in(
        &settings,
        &project_id,
        cell_width,
        cell_height,
        pivot_x,
        pivot_y,
        anchor_mode,
    )
}

#[tauri::command]
pub fn update_runtime_policy(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    geometry_policy: GeometryPolicy,
    neutral_height_target: Option<u32>,
    neutral_tolerance_px: u8,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    update_runtime_policy_in(
        &settings,
        &project_id,
        geometry_policy,
        neutral_height_target,
        neutral_tolerance_px,
    )
}

#[tauri::command]
pub fn confirm_native_review(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    accepted_frame_ids: Vec<String>,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    confirm_native_review_in(&settings, &project_id, &animation_id, accepted_frame_ids)
}

#[tauri::command]
pub fn replace_raw_frame(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    frame_id: String,
    source_path: Option<String>,
    crop: Option<ManualCrop>,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    replace_raw_frame_in(
        &settings,
        &project_id,
        &animation_id,
        &frame_id,
        source_path.as_deref(),
        crop,
    )
}

#[tauri::command]
pub fn import_upscaled_frame(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
    source_frame_id: String,
    source_path: String,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    import_upscaled_frame_in(
        &settings,
        &project_id,
        &animation_id,
        &source_frame_id,
        &source_path,
    )
}

#[tauri::command]
pub fn approve_upscale(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    animation_id: String,
) -> Result<Project> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    approve_upscale_in(&settings, &project_id, &animation_id)
}

#[tauri::command]
pub fn export_snapped(
    app: tauri::AppHandle,
    project_id: String,
    animation_id: String,
    destination_path: String,
    source_frame_ids: Vec<String>,
) -> Result<String> {
    let settings = read_settings(&settings_file(&app)?)?;
    export_snapped_in(
        &settings,
        &project_id,
        &animation_id,
        &destination_path,
        source_frame_ids,
    )
}

#[tauri::command]
pub fn open_snap_folder(
    app: tauri::AppHandle,
    project_id: String,
    animation_id: String,
) -> Result<()> {
    let settings = read_settings(&settings_file(&app)?)?;
    let (dir, project) = find_project(&settings, &project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|a| a.id == animation_id)
        .ok_or("Animation not found")?;
    let id = animation
        .active_batch_snap_id
        .as_ref()
        .ok_or("Apply snapped frames first")?;
    let review = animation
        .batch_snaps
        .iter()
        .find(|r| &r.id == id)
        .ok_or("Active snap review missing")?;
    let relative = review
        .frames
        .first()
        .ok_or("Snap review has no frames")?
        .relative_path
        .clone();
    let path = verified_asset(&dir, &relative, &review.frames[0].sha256)?.0;
    let folder = path.parent().ok_or("Snap folder missing")?;
    open_folder(folder)
}

#[tauri::command]
pub fn open_workspace_folder(app: tauri::AppHandle) -> Result<String> {
    let settings = read_settings(&settings_file(&app)?)?;
    let folder = workspace(&settings)?;
    open_folder(&folder)?;
    Ok(folder.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn open_project_exports_folder(app: tauri::AppHandle, project_id: String) -> Result<String> {
    let settings = read_settings(&settings_file(&app)?)?;
    let (dir, _) = find_project(&settings, &project_id)?;
    let folder = project_exports_root(&dir)?;
    open_folder(&folder)?;
    Ok(folder.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn export_reference(
    app: tauri::AppHandle,
    state: tauri::State<'_, StudioLock>,
    project_id: String,
    facing: String,
) -> Result<String> {
    let _guard = state
        .0
        .lock()
        .map_err(|_| "Project operation lock is unavailable")?;
    let settings = read_settings(&settings_file(&app)?)?;
    export_reference_in(&settings, &project_id, &facing)
}

#[tauri::command]
pub fn reference_export_folder(
    app: tauri::AppHandle,
    project_id: String,
    facing: String,
) -> Result<Option<String>> {
    let settings = read_settings(&settings_file(&app)?)?;
    Ok(reference_export_folder_in(&settings, &project_id, &facing)?
        .map(|path| path.to_string_lossy().into_owned()))
}

#[tauri::command]
pub fn open_reference_export_folder(
    app: tauri::AppHandle,
    project_id: String,
    facing: String,
) -> Result<String> {
    let settings = read_settings(&settings_file(&app)?)?;
    let folder = reference_export_folder_in(&settings, &project_id, &facing)?
        .ok_or("Export this reference first")?;
    open_folder(&folder)?;
    Ok(folder.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn open_animation_export_folder(
    app: tauri::AppHandle,
    project_id: String,
    animation_id: String,
    review_id: String,
) -> Result<String> {
    let settings = read_settings(&settings_file(&app)?)?;
    let (dir, project) = find_project(&settings, &project_id)?;
    let animation = project
        .animations
        .iter()
        .find(|item| item.id == animation_id)
        .ok_or("Animation not found")?;
    let review = animation
        .exports
        .iter()
        .find(|item| item.id == review_id)
        .ok_or("Export review not found")?;
    let assets = [
        (&review.sheet_relative_path, &review.sheet_sha256),
        (&review.gif_relative_path, &review.gif_sha256),
        (&review.manifest_relative_path, &review.manifest_sha256),
    ];
    let mut folder: Option<PathBuf> = None;
    for (relative, hash) in assets {
        let (path, _) = verified_asset(&dir, relative, hash)?;
        let parent = path.parent().ok_or("Export folder is missing")?;
        if folder.as_deref().is_some_and(|existing| existing != parent) {
            return Err("Export files are not in one folder".into());
        }
        folder = Some(parent.to_path_buf());
    }
    let folder = folder.ok_or("Export folder is missing")?;
    open_folder(&folder)?;
    Ok(folder.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn read_upscaled_preview(
    app: tauri::AppHandle,
    project_id: String,
    animation_id: String,
    upscale_id: String,
) -> Result<AssetPreview> {
    let settings = read_settings(&settings_file(&app)?)?;
    upscaled_preview_in(&settings, &project_id, &animation_id, &upscale_id)
}

#[tauri::command]
pub fn read_asset_preview(
    app: tauri::AppHandle,
    project_id: String,
    import_id: String,
) -> Result<AssetPreview> {
    let settings = read_settings(&settings_file(&app)?)?;
    preview_in(&settings, &project_id, &import_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_settings(root: &Path) -> Settings {
        fs::create_dir_all(root.join("projects")).unwrap();
        Settings {
            workspace_root: Some(root.to_string_lossy().into_owned()),
            last_project_id: None,
            snapper_executable: None,
            snapper_version: None,
            python_executable: None,
            python_environment: None,
        }
    }

    fn sample_png(path: &Path) {
        let image = image::RgbaImage::from_pixel(4, 4, image::Rgba([250, 120, 30, 255]));
        image.save(path).unwrap();
    }

    fn snap_sample_png(path: &Path) {
        let mut image = image::RgbImage::from_pixel(256, 256, image::Rgb([23, 220, 64]));
        for y in 48..216 {
            for x in 76..180 {
                let color = if y < 100 {
                    [59, 40, 35]
                } else if x < 128 {
                    [228, 159, 53]
                } else {
                    [171, 84, 42]
                };
                image.put_pixel(x, y, image::Rgb(color));
            }
        }
        image.save(path).unwrap();
    }

    #[test]
    fn snap_fit_rejects_pivot_and_one_pixel_overflow() {
        let (mut runtime, _) = preset_settings(Preset::Generic);
        runtime.cell_width = 128;
        runtime.cell_height = 128;
        runtime.pivot_x = 64;
        runtime.pivot_y = 112;
        assert_eq!(snap_placement(&runtime, 100, 126), (14, -13, false));
        assert!(!snap_placement(&runtime, 129, 70).2);
        assert!(!snap_placement(&runtime, 80, 114).2);
        assert!(snap_placement(&runtime, 80, 113).2);
    }

    #[test]
    fn installed_auto_fit_preserves_active_snap_until_apply_and_is_deterministic() {
        let (Ok(snapper), Ok(python)) = (
            std::env::var("SPRITE_STUDIO_TEST_SNAPPER"),
            std::env::var("SPRITE_STUDIO_TEST_PYTHON"),
        ) else {
            return;
        };
        let tmp = tempdir().unwrap();
        let base = test_settings(tmp.path());
        let settings_path = tmp.path().join("app-settings.json");
        atomic_json(&settings_path, &base).unwrap();
        configure_snapper_in(&settings_path, &snapper).unwrap();
        let settings = configure_python_in(&settings_path, &python).unwrap();
        let project = create_project_in(&settings, "Auto-fit Fox", Preset::Generic).unwrap();
        update_runtime_in(
            &settings,
            &project.id,
            128,
            128,
            64,
            112,
            AnchorMode::Custom,
        )
        .unwrap();
        update_runtime_policy_in(&settings, &project.id, GeometryPolicy::Locked, Some(64), 4)
            .unwrap();
        let source = tmp.path().join("source.png");
        snap_sample_png(&source);
        let imported =
            import_into(&settings, &project.id, "east", source.to_str().unwrap()).unwrap();
        let original_hash = imported.imports[0].sha256.clone();
        let first = run_snap_in(
            &settings,
            &project.id,
            "east",
            SnapOptions {
                colors: 16,
                pixel_size: Some(1),
                palette: None,
            },
        )
        .unwrap();
        let active_id = first.snap_reviews.last().unwrap().id.clone();
        apply_snap_in(&settings, &project.id, &active_id).unwrap();
        let cleanup_options = CleanupOptions {
            background: "auto".into(),
            tolerance: 18,
            min_area: 2,
        };
        assert!(run_cleanup_in(&settings, &project.id, "east", cleanup_options.clone()).is_err());
        let fitted =
            run_auto_fit_in(&settings, &project.id, "east", cleanup_options.clone()).unwrap();
        let run = fitted.auto_fit_runs.last().unwrap();
        assert!(!run.candidates[0].fits);
        assert!(run.candidates[1].fits, "the first coarser grid should fit");
        let selected_id = run.selected_review_id.as_ref().unwrap();
        let chosen = run
            .candidates
            .iter()
            .find(|item| &item.review_id == selected_id)
            .unwrap();
        assert!(chosen.fits);
        assert!(chosen.pixel_size > run.starting_pixel_size);
        assert!(
            snap_placement(
                &fitted.runtime,
                chosen.foreground_width,
                chosen.foreground_height
            )
            .2
        );
        assert_eq!(
            fitted.anchors["east"].active_snap_id.as_deref(),
            Some(active_id.as_str())
        );
        assert_eq!(fitted.imports[0].sha256, original_hash);
        assert!(fitted
            .snap_reviews
            .iter()
            .all(|item| item.id == active_id || item.applied_at.is_none()));
        let selected_review = fitted
            .snap_reviews
            .iter()
            .find(|item| &item.id == selected_id)
            .unwrap();
        let selected_hash = selected_review.native_sha256.clone();
        assert!(selected_review.native_width < first.snap_reviews[0].native_width);
        let second =
            run_auto_fit_in(&settings, &project.id, "east", cleanup_options.clone()).unwrap();
        let second_id = second
            .auto_fit_runs
            .last()
            .unwrap()
            .selected_review_id
            .as_ref()
            .unwrap();
        let second_review = second
            .snap_reviews
            .iter()
            .find(|item| &item.id == second_id)
            .unwrap();
        assert_eq!(second_review.native_sha256, selected_hash);
        assert_eq!(
            second
                .auto_fit_runs
                .last()
                .unwrap()
                .candidates
                .iter()
                .find(|item| &item.review_id == second_id)
                .unwrap()
                .pixel_size,
            chosen.pixel_size
        );
        assert_eq!(
            second.anchors["east"].active_snap_id.as_deref(),
            Some(active_id.as_str())
        );
        let applied = apply_snap_in(&settings, &project.id, selected_id).unwrap();
        assert_eq!(
            applied.anchors["east"].active_snap_id.as_deref(),
            Some(selected_id.as_str())
        );
        assert_eq!(applied.imports[0].sha256, original_hash);
        assert_eq!(
            (
                applied.runtime.cell_width,
                applied.runtime.cell_height,
                applied.runtime.pivot_x,
                applied.runtime.pivot_y
            ),
            (128, 128, 64, 112)
        );
        let cleaned = run_cleanup_in(&settings, &project.id, "east", cleanup_options).unwrap();
        let review = cleaned.cleanup_reviews.last().unwrap();
        assert_eq!(
            (review.cleaned_width, review.cleaned_height),
            (chosen.foreground_width, chosen.foreground_height)
        );
        assert_eq!(
            (review.normalized_width, review.normalized_height),
            (128, 128)
        );
    }

    #[test]
    fn installed_auto_fit_no_candidate_keeps_locked_contract_and_prior_review() {
        let (Ok(snapper), Ok(python)) = (
            std::env::var("SPRITE_STUDIO_TEST_SNAPPER"),
            std::env::var("SPRITE_STUDIO_TEST_PYTHON"),
        ) else {
            return;
        };
        let tmp = tempdir().unwrap();
        let base = test_settings(tmp.path());
        let settings_path = tmp.path().join("app-settings.json");
        atomic_json(&settings_path, &base).unwrap();
        configure_snapper_in(&settings_path, &snapper).unwrap();
        let settings = configure_python_in(&settings_path, &python).unwrap();
        let project = create_project_in(&settings, "Impossible Fox", Preset::Generic).unwrap();
        update_runtime_in(&settings, &project.id, 32, 32, 16, 0, AnchorMode::Custom).unwrap();
        update_runtime_policy_in(&settings, &project.id, GeometryPolicy::Locked, None, 4).unwrap();
        let source = tmp.path().join("source.png");
        snap_sample_png(&source);
        let imported =
            import_into(&settings, &project.id, "east", source.to_str().unwrap()).unwrap();
        let first = run_snap_in(
            &settings,
            &project.id,
            "east",
            SnapOptions {
                colors: 16,
                pixel_size: Some(1),
                palette: None,
            },
        )
        .unwrap();
        let active_id = first.snap_reviews.last().unwrap().id.clone();
        apply_snap_in(&settings, &project.id, &active_id).unwrap();
        let result = run_auto_fit_in(
            &settings,
            &project.id,
            "east",
            CleanupOptions {
                background: "auto".into(),
                tolerance: 18,
                min_area: 2,
            },
        );
        let error = match result {
            Ok(_) => panic!("Expected no fitting candidate"),
            Err(error) => error,
        };
        assert!(
            error.contains("No lossless snap candidate fits the locked 32×32 cell at pivot (16,0)")
        );
        let persisted = find_project(&settings, &project.id).unwrap().1;
        assert_eq!(
            persisted.anchors["east"].active_snap_id.as_deref(),
            Some(active_id.as_str())
        );
        assert_eq!(persisted.imports[0].sha256, imported.imports[0].sha256);
        assert_eq!(
            (
                persisted.runtime.cell_width,
                persisted.runtime.cell_height,
                persisted.runtime.pivot_x,
                persisted.runtime.pivot_y
            ),
            (32, 32, 16, 0)
        );
        assert_eq!(persisted.runtime.geometry_policy, GeometryPolicy::Locked);
        assert!(persisted
            .auto_fit_runs
            .last()
            .unwrap()
            .selected_review_id
            .is_none());
        assert!(persisted
            .auto_fit_runs
            .last()
            .unwrap()
            .candidates
            .iter()
            .all(|item| !item.fits));
        assert!(persisted.snap_reviews.len() > 1);
    }

    #[test]
    fn auto_fit_ranking_respects_neutral_target_then_source_detail() {
        let mut run = AutoFitRun {
            id: "test".into(),
            facing: "east".into(),
            source_import_id: "source".into(),
            cell_width: 128,
            cell_height: 128,
            pivot_x: 64,
            pivot_y: 112,
            neutral_height_target: Some(64),
            neutral_tolerance_px: 4,
            starting_pixel_size: 8.0,
            candidates: vec![],
            recommended_review_ids: vec![],
            selected_review_id: None,
            created_at: Utc::now(),
        };
        let candidate =
            |id: &str, size: f64, height: u32, detail: u32, loss: u32| AutoFitCandidate {
                review_id: id.into(),
                pixel_size: size,
                foreground_width: 60,
                foreground_height: height,
                foreground_pixels: detail,
                removed_speckle_pixels: loss,
                fits: true,
            };
        run.candidates = vec![
            candidate("near-source", 9.0, 90, 700, 0),
            candidate("near-target", 13.0, 65, 600, 1),
            candidate("exact-target", 14.0, 64, 590, 1),
            candidate("less-detail", 14.0, 64, 580, 2),
        ];
        assert_eq!(rank_auto_fit_candidates(&run)[0].review_id, "exact-target");
        run.neutral_height_target = None;
        assert_eq!(rank_auto_fit_candidates(&run)[0].review_id, "near-source");
    }

    #[test]
    fn locked_geometry_rejects_mutation_but_flexible_geometry_allows_it() {
        let tmp = tempdir().unwrap();
        let settings = test_settings(tmp.path());
        let project = create_project_in(&settings, "Policy Fox", Preset::Generic).unwrap();
        let locked =
            update_runtime_policy_in(&settings, &project.id, GeometryPolicy::Locked, Some(64), 4)
                .unwrap();
        assert_eq!(locked.runtime.geometry_policy, GeometryPolicy::Locked);
        let mut altered_workflow = locked.workflow.clone();
        altered_workflow.anchor_mode = AnchorMode::Custom;
        assert!(update_workflow_in(&settings, &project.id, altered_workflow).is_err());
        assert!(update_runtime_in(
            &settings,
            &project.id,
            272,
            272,
            136,
            271,
            AnchorMode::BottomCenter,
        )
        .is_err());
        assert_eq!(
            find_project(&settings, &project.id)
                .unwrap()
                .1
                .runtime
                .cell_width,
            256
        );
        update_runtime_policy_in(
            &settings,
            &project.id,
            GeometryPolicy::Flexible,
            Some(64),
            4,
        )
        .unwrap();
        let resized = update_runtime_in(
            &settings,
            &project.id,
            272,
            272,
            136,
            271,
            AnchorMode::BottomCenter,
        )
        .unwrap();
        assert_eq!(resized.runtime.cell_width, 272);
    }

    #[test]
    fn presets_and_reimports_preserve_bytes() {
        let tmp = tempdir().unwrap();
        let settings = test_settings(tmp.path());
        let generic = create_project_in(&settings, "Test Fox", Preset::Generic).unwrap();
        let fighter = create_project_in(&settings, "Kangaroo", Preset::KangiFight).unwrap();
        assert_eq!(generic.runtime.cell_width, 256);
        assert_eq!(generic.runtime.pivot_y, 255);
        assert_eq!(fighter.runtime.cell_width, 128);
        assert_eq!(fighter.runtime.neutral_height_target, Some(64));
        assert_eq!(
            fighter.runtime.mirrored_facings.get("left").unwrap(),
            "right"
        );
        let source = tmp.path().join("source.png");
        sample_png(&source);
        let once = import_into(&settings, &generic.id, "north", source.to_str().unwrap()).unwrap();
        let twice = import_into(&settings, &generic.id, "north", source.to_str().unwrap()).unwrap();
        assert_eq!(twice.imports.len(), 2);
        assert_ne!(
            twice.imports[0].relative_path,
            twice.imports[1].relative_path
        );
        assert_eq!(twice.imports[0].sha256, twice.imports[1].sha256);
        assert_eq!(
            twice.anchors["north"].active_import_id,
            Some(twice.imports[1].id.clone())
        );
        assert_eq!(
            once.imports[0].sha256,
            format!("{:x}", Sha256::digest(fs::read(&source).unwrap()))
        );
        let (dir, _) = find_project(&settings, &generic.id).unwrap();
        assert_eq!(
            fs::read(dir.join(&once.imports[0].relative_path)).unwrap(),
            fs::read(&source).unwrap()
        );
        let preview = preview_in(&settings, &generic.id, &once.imports[0].id).unwrap();
        assert_eq!(preview.width, 4);
        assert!(preview.data_url.starts_with("data:image/png;base64,"));
        assert_eq!(
            find_project(&settings, &generic.id)
                .unwrap()
                .1
                .imports
                .len(),
            2
        );
    }

    #[test]
    fn project_rename_preserves_folder_slug_and_imported_assets() {
        let tmp = tempdir().unwrap();
        let settings = test_settings(tmp.path());
        let project = create_project_in(&settings, "First Fox", Preset::Generic).unwrap();
        let source = tmp.path().join("source.png");
        sample_png(&source);
        let imported =
            import_into(&settings, &project.id, "east", source.to_str().unwrap()).unwrap();
        let (original_dir, _) = find_project(&settings, &project.id).unwrap();
        let original_import = imported.imports[0].clone();

        let renamed = rename_project_in(&settings, &project.id, "  Comet Cat  ").unwrap();
        assert_eq!(renamed.name, "Comet Cat");
        assert_eq!(renamed.id, project.id);
        assert_eq!(renamed.slug, project.slug);
        assert_eq!(renamed.imports[0].id, original_import.id);
        assert_eq!(renamed.imports[0].sha256, original_import.sha256);
        assert_eq!(
            renamed.anchors["east"].active_import_id,
            imported.anchors["east"].active_import_id
        );
        assert_eq!(
            find_project(&settings, &project.id).unwrap().0,
            original_dir
        );
        assert_eq!(
            fs::read(original_dir.join(&original_import.relative_path)).unwrap(),
            fs::read(&source).unwrap()
        );
        assert_eq!(read_project(&original_dir).unwrap().name, "Comet Cat");

        assert!(rename_project_in(&settings, &project.id, "  ").is_err());
        assert!(rename_project_in(&settings, &project.id, &"x".repeat(81)).is_err());
        assert_eq!(read_project(&original_dir).unwrap().name, "Comet Cat");
    }

    #[test]
    fn animation_rename_preserves_identity_and_blocks_same_facing_duplicates() {
        let tmp = tempdir().unwrap();
        let settings = test_settings(tmp.path());
        let project = create_project_in(&settings, "Animation Fox", Preset::Generic).unwrap();
        let project = create_animation_in(&settings, &project.id, "idle", "east").unwrap();
        let east_id = project.animations[0].id.clone();
        create_animation_in(&settings, &project.id, "attack", "east").unwrap();
        let west = create_animation_in(&settings, &project.id, "idle", "west").unwrap();
        let west_id = west.animations[2].id.clone();
        let source = tmp.path().join("board.png");
        sample_png(&source);
        let imported =
            import_board_in(&settings, &project.id, &east_id, source.to_str().unwrap()).unwrap();
        let board = imported.animations[0].boards[0].clone();

        assert!(rename_animation_in(&settings, &project.id, &east_id, " ATTACK ").is_err());
        assert!(rename_animation_in(&settings, &project.id, &east_id, "  ").is_err());
        assert!(rename_animation_in(&settings, &project.id, &east_id, &"x".repeat(61)).is_err());
        let renamed = rename_animation_in(&settings, &project.id, &east_id, "  walk  ").unwrap();
        assert_eq!(renamed.animations[0].name, "walk");
        assert_eq!(renamed.animations[0].id, east_id);
        assert_eq!(renamed.animations[0].boards[0].id, board.id);
        assert_eq!(renamed.animations[0].boards[0].sha256, board.sha256);
        assert_eq!(
            renamed.animations[0].active_board_id,
            imported.animations[0].active_board_id
        );
        assert_eq!(renamed.animations[1].name, "attack");
        assert_eq!(renamed.animations[2].name, "idle");
        let (dir, persisted) = find_project(&settings, &project.id).unwrap();
        assert_eq!(persisted.animations[0].name, "walk");
        assert_eq!(
            fs::read(dir.join(&board.relative_path)).unwrap(),
            fs::read(&source).unwrap()
        );
        let cross_facing = rename_animation_in(&settings, &project.id, &west_id, "attack").unwrap();
        assert_eq!(cross_facing.animations[1].name, "attack");
        assert_eq!(cross_facing.animations[2].name, "attack");
        assert!(rename_animation_in(&settings, &project.id, "missing", "walk").is_err());
    }

    #[test]
    fn runtime_pivot_and_anchor_mode_save_together() {
        let tmp = tempdir().unwrap();
        let settings = test_settings(tmp.path());
        let project = create_project_in(&settings, "Pivot Fox", Preset::Generic).unwrap();

        let custom = update_runtime_in(
            &settings,
            &project.id,
            256,
            256,
            120,
            230,
            AnchorMode::Custom,
        )
        .unwrap();
        assert_eq!((custom.runtime.pivot_x, custom.runtime.pivot_y), (120, 230));
        assert_eq!(custom.workflow.anchor_mode, AnchorMode::Custom);
        assert_eq!(
            find_project(&settings, &project.id)
                .unwrap()
                .1
                .workflow
                .anchor_mode,
            AnchorMode::Custom
        );

        assert!(update_runtime_in(
            &settings,
            &project.id,
            256,
            256,
            120,
            230,
            AnchorMode::BottomCenter,
        )
        .is_err());
        let unchanged = find_project(&settings, &project.id).unwrap().1;
        assert_eq!(
            (unchanged.runtime.pivot_x, unchanged.runtime.pivot_y),
            (120, 230)
        );
        assert_eq!(unchanged.workflow.anchor_mode, AnchorMode::Custom);

        let centered = update_runtime_in(
            &settings,
            &project.id,
            256,
            256,
            128,
            255,
            AnchorMode::BottomCenter,
        )
        .unwrap();
        assert_eq!(centered.workflow.anchor_mode, AnchorMode::BottomCenter);
        assert_eq!(
            (centered.runtime.pivot_x, centered.runtime.pivot_y),
            (128, 255)
        );

        let source = tmp.path().join("pivot-source.png");
        sample_png(&source);
        import_into(&settings, &project.id, "north", source.to_str().unwrap()).unwrap();
        let (dir, mut pending) = find_project(&settings, &project.id).unwrap();
        pending.imports[0]
            .stages
            .insert("cleanup".into(), StageState::Review);
        pending.imports[0]
            .stages
            .insert("normalize".into(), StageState::Review);
        atomic_json(&dir.join("project.json"), &pending).unwrap();
        let resized = update_runtime_in(
            &settings,
            &project.id,
            272,
            272,
            136,
            271,
            AnchorMode::BottomCenter,
        )
        .unwrap();
        assert_eq!(resized.imports[0].stages["cleanup"], StageState::Stale);
        assert_eq!(resized.imports[0].stages["normalize"], StageState::Stale);
    }

    #[test]
    fn export_folders_resolve_inside_the_selected_workspace() {
        let tmp = tempdir().unwrap();
        let settings = test_settings(tmp.path());
        let first = create_project_in(&settings, "First", Preset::Generic).unwrap();
        let second = create_project_in(&settings, "Second", Preset::Generic).unwrap();
        let first_dir = find_project(&settings, &first.id).unwrap().0;
        let second_dir = find_project(&settings, &second.id).unwrap().0;
        let export_dir = project_export_dir(&first_dir, "references/east/test-run").unwrap();
        assert!(export_dir.starts_with(project_exports_root(&first_dir).unwrap()));
        assert!(export_dir.starts_with(workspace(&settings).unwrap()));
        assert!(!export_dir.starts_with(second_dir));
        assert!(project_export_dir(&first_dir, "../escape").is_err());
    }

    #[test]
    fn explorer_receives_a_normal_windows_folder_path() {
        assert_eq!(
            explorer_path(Path::new(r"\\?\C:\Users\hp\sprites")),
            r"C:\Users\hp\sprites"
        );
        assert_eq!(
            explorer_path(Path::new(r"\\?\UNC\server\share\sprites")),
            r"\\server\share\sprites"
        );
    }

    #[test]
    fn applied_reference_export_is_reopenable_and_preserves_source_bytes() {
        let tmp = tempdir().unwrap();
        let settings = test_settings(tmp.path());
        let project = create_project_in(&settings, "Reference Fox", Preset::Generic).unwrap();
        let source = tmp.path().join("source.png");
        sample_png(&source);
        let mut project =
            import_into(&settings, &project.id, "east", source.to_str().unwrap()).unwrap();
        let (dir, _) = find_project(&settings, &project.id).unwrap();
        let artifact_dir = dir.join("test-artifacts");
        fs::create_dir(&artifact_dir).unwrap();
        let hash = format!("{:x}", Sha256::digest(fs::read(&source).unwrap()));
        for name in [
            "native.png",
            "reference.png",
            "cutout.png",
            "normalized.png",
        ] {
            fs::copy(&source, artifact_dir.join(name)).unwrap();
        }
        let source_id = project.anchors["east"].active_import_id.clone().unwrap();
        let snap_id = Uuid::new_v4().to_string();
        let cleanup_id = Uuid::new_v4().to_string();
        project.snap_reviews.push(SnapReview {
            id: snap_id.clone(),
            facing: "east".into(),
            source_import_id: source_id.clone(),
            native_relative_path: "test-artifacts/native.png".into(),
            native_sha256: hash.clone(),
            native_width: 4,
            native_height: 4,
            reference_relative_path: "test-artifacts/reference.png".into(),
            reference_sha256: hash.clone(),
            reference_width: 4,
            reference_height: 4,
            reference_scale: 1,
            outputs: None,
            colors: 16,
            pixel_size: None,
            detected_pixel_size: None,
            auto_fit_run_id: None,
            palette: None,
            tool_version: "test".into(),
            created_at: Utc::now(),
            applied_at: Some(Utc::now()),
        });
        project.cleanup_reviews.push(CleanupReview {
            id: cleanup_id.clone(),
            facing: "east".into(),
            source_import_id: source_id,
            source_snap_id: snap_id.clone(),
            cleaned_relative_path: "test-artifacts/cutout.png".into(),
            cleaned_sha256: hash.clone(),
            cleaned_width: 4,
            cleaned_height: 4,
            normalized_relative_path: "test-artifacts/normalized.png".into(),
            normalized_sha256: hash.clone(),
            normalized_width: 4,
            normalized_height: 4,
            background_hex: "00ff00".into(),
            tolerance: 18,
            min_area: 2,
            foreground_pixels: 16,
            removed_speckle_pixels: 0,
            placement_x: 0,
            placement_y: 0,
            processor_version: "test".into(),
            python_environment: "test".into(),
            created_at: Utc::now(),
            applied_at: Some(Utc::now()),
        });
        project.anchors.get_mut("east").unwrap().active_snap_id = Some(snap_id);
        project.anchors.get_mut("east").unwrap().active_cleanup_id = Some(cleanup_id);
        atomic_json(&dir.join("project.json"), &project).unwrap();

        assert!(reference_export_folder_in(&settings, &project.id, "east")
            .unwrap()
            .is_none());
        let folder = export_reference_in(&settings, &project.id, "east").unwrap();
        let folder = PathBuf::from(folder);
        assert!(folder.starts_with(dir.join("exports")));
        assert_eq!(
            fs::read(folder.join("normalized.png")).unwrap(),
            fs::read(&source).unwrap()
        );
        assert_eq!(
            reference_export_folder_in(&settings, &project.id, "east").unwrap(),
            Some(folder.clone())
        );
        assert_eq!(
            PathBuf::from(export_reference_in(&settings, &project.id, "east").unwrap()),
            folder
        );
        fs::write(folder.join("normalized.png"), b"changed").unwrap();
        assert!(reference_export_folder_in(&settings, &project.id, "east").is_err());
        assert!(export_reference_in(&settings, &project.id, "east").is_err());
    }

    #[test]
    fn deleted_character_can_be_restored_without_losing_assets() {
        let tmp = tempdir().unwrap();
        let mut settings = test_settings(tmp.path());
        let settings_path = tmp.path().join("app-settings.json");
        let project = create_project_in(&settings, "Recoverable Fox", Preset::Generic).unwrap();
        let source = tmp.path().join("source.png");
        sample_png(&source);
        let imported =
            import_into(&settings, &project.id, "north", source.to_str().unwrap()).unwrap();
        settings.last_project_id = Some(project.id.clone());
        atomic_json(&settings_path, &settings).unwrap();
        let archived = archive_project_in(&settings_path, &project.id).unwrap();
        assert!(archived.last_project_id.is_none());
        assert!(find_project(&archived, &project.id).is_err());
        assert_eq!(list_deleted_projects_in(&archived).unwrap().len(), 1);
        assert!(archive_project_in(&settings_path, &project.id).is_err());
        let restored = restore_project_in(&archived, &project.id).unwrap();
        assert_eq!(restored.imports[0].sha256, imported.imports[0].sha256);
        let (dir, _) = find_project(&archived, &project.id).unwrap();
        assert_eq!(
            fs::read(dir.join(&restored.imports[0].relative_path)).unwrap(),
            fs::read(source).unwrap()
        );
        assert!(list_deleted_projects_in(&archived).unwrap().is_empty());
    }

    #[test]
    fn invalid_images_and_paths_are_rejected() {
        let tmp = tempdir().unwrap();
        let settings = test_settings(tmp.path());
        let project = create_project_in(&settings, "Guard", Preset::KangiFight).unwrap();
        let broken = tmp.path().join("broken.png");
        fs::write(&broken, b"not a png").unwrap();
        assert!(import_into(&settings, &project.id, "right", broken.to_str().unwrap()).is_err());
        assert!(import_into(&settings, &project.id, "left", broken.to_str().unwrap()).is_err());
        assert!(safe_relative("../outside.png").is_err());
        assert!(safe_relative("C:\\outside.png").is_err());
    }

    #[test]
    fn jpeg_import_uses_right_facing_source() {
        let tmp = tempdir().unwrap();
        let settings = test_settings(tmp.path());
        let project = create_project_in(&settings, "Fighter", Preset::KangiFight).unwrap();
        let source = tmp.path().join("source.jpg");
        image::RgbImage::from_pixel(6, 8, image::Rgb([42, 100, 180]))
            .save(&source)
            .unwrap();
        let imported =
            import_into(&settings, &project.id, "right", source.to_str().unwrap()).unwrap();
        assert_eq!(imported.imports[0].mime_type, "image/jpeg");
        assert_eq!(imported.imports[0].height, 8);
        assert!(preview_in(&settings, &project.id, &imported.imports[0].id)
            .unwrap()
            .data_url
            .starts_with("data:image/jpeg;base64,"));
    }

    #[test]
    fn workspace_and_last_project_survive_reload_and_missing_asset_is_reported() {
        let tmp = tempdir().unwrap();
        let workspace_root = tmp.path().join("workspace");
        fs::create_dir(&workspace_root).unwrap();
        let settings_path = tmp.path().join("app-settings.json");
        let mut settings =
            store_workspace(&settings_path, workspace_root.to_str().unwrap()).unwrap();
        let project = create_project_in(&settings, "Fox", Preset::Generic).unwrap();
        settings.last_project_id = Some(project.id.clone());
        atomic_json(&settings_path, &settings).unwrap();
        let reloaded = read_settings(&settings_path).unwrap();
        assert_eq!(reloaded.last_project_id, Some(project.id.clone()));
        assert_eq!(find_project(&reloaded, &project.id).unwrap().1.name, "Fox");
        let source = tmp.path().join("source.png");
        sample_png(&source);
        let imported =
            import_into(&reloaded, &project.id, "west", source.to_str().unwrap()).unwrap();
        let (dir, _) = find_project(&reloaded, &project.id).unwrap();
        fs::remove_file(dir.join(&imported.imports[0].relative_path)).unwrap();
        assert!(preview_in(&reloaded, &project.id, &imported.imports[0].id)
            .unwrap_err()
            .contains("missing"));
    }

    #[test]
    fn old_project_metadata_and_snap_options_are_safe() {
        let tmp = tempdir().unwrap();
        let settings = test_settings(tmp.path());
        let project = create_project_in(&settings, "Legacy", Preset::Generic).unwrap();
        let (dir, _) = find_project(&settings, &project.id).unwrap();
        let path = dir.join("project.json");
        let mut old: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        old.as_object_mut().unwrap().remove("snapReviews");
        old.as_object_mut().unwrap().remove("cleanupReviews");
        old.as_object_mut().unwrap().remove("animations");
        for anchor in old["anchors"].as_object_mut().unwrap().values_mut() {
            anchor.as_object_mut().unwrap().remove("activeSnapId");
            anchor.as_object_mut().unwrap().remove("activeCleanupId");
        }
        atomic_json(&path, &old).unwrap();
        let loaded = read_project(&dir).unwrap();
        assert!(loaded.snap_reviews.is_empty());
        assert!(loaded.anchors["north"].active_snap_id.is_none());
        assert!(loaded.cleanup_reviews.is_empty());
        assert!(loaded.animations.is_empty());
        assert!(loaded.anchors["north"].active_cleanup_id.is_none());
        assert!(validated_snap_options(
            &SnapOptions {
                colors: 1,
                pixel_size: None,
                palette: None
            },
            256,
            256
        )
        .is_err());
        assert!(validated_cleanup_options(&CleanupOptions {
            background: "green".into(),
            tolerance: 18,
            min_area: 2
        })
        .is_err());
        assert!(validated_cleanup_options(&CleanupOptions {
            background: "auto".into(),
            tolerance: 81,
            min_area: 2
        })
        .is_err());
        assert!(validated_snap_options(
            &SnapOptions {
                colors: 16,
                pixel_size: Some(257),
                palette: None
            },
            256,
            256
        )
        .is_err());
        assert!(validated_snap_options(
            &SnapOptions {
                colors: 16,
                pixel_size: None,
                palette: Some("not-hex".into())
            },
            256,
            256
        )
        .is_err());
    }

    #[test]
    fn interrupted_snap_becomes_failed_on_reopen() {
        let tmp = tempdir().unwrap();
        let settings = test_settings(tmp.path());
        let project = create_project_in(&settings, "Interrupted", Preset::Generic).unwrap();
        let source = tmp.path().join("source.png");
        sample_png(&source);
        let imported =
            import_into(&settings, &project.id, "north", source.to_str().unwrap()).unwrap();
        let (dir, mut loaded) = find_project(&settings, &project.id).unwrap();
        mark_snap_stage(&mut loaded, &imported.imports[0].id, StageState::Processing).unwrap();
        mark_cleanup_stages(&mut loaded, &imported.imports[0].id, StageState::Processing).unwrap();
        atomic_json(&dir.join("project.json"), &loaded).unwrap();
        recover_interrupted(&dir, &mut loaded).unwrap();
        assert_eq!(
            read_project(&dir).unwrap().imports[0].stages["snap"],
            StageState::Failed
        );
        assert_eq!(
            read_project(&dir).unwrap().imports[0].stages["cleanup"],
            StageState::Failed
        );
        assert_eq!(
            read_project(&dir).unwrap().imports[0].stages["normalize"],
            StageState::Failed
        );
    }

    #[test]
    fn installed_snapper_review_apply_and_reimport() {
        let Ok(executable) = std::env::var("SPRITE_STUDIO_TEST_SNAPPER") else {
            return;
        };
        let tmp = tempdir().unwrap();
        let base_settings = test_settings(tmp.path());
        let settings_path = tmp.path().join("app-settings.json");
        atomic_json(&settings_path, &base_settings).unwrap();
        let settings = configure_snapper_in(&settings_path, &executable).unwrap();
        assert!(settings
            .snapper_version
            .as_deref()
            .unwrap()
            .starts_with("spritefusion-pixel-snapper "));
        let project = create_project_in(&settings, "Snapping Fox", Preset::Generic).unwrap();
        let source = tmp.path().join("source.png");
        snap_sample_png(&source);
        let imported =
            import_into(&settings, &project.id, "east", source.to_str().unwrap()).unwrap();
        let review_project = run_snap_in(
            &settings,
            &project.id,
            "east",
            SnapOptions {
                colors: 16,
                pixel_size: Some(4),
                palette: None,
            },
        )
        .unwrap();
        assert_eq!(review_project.imports[0].stages["snap"], StageState::Review);
        assert!(review_project.anchors["east"].active_snap_id.is_none());
        let review = &review_project.snap_reviews[0];
        assert!(review.native_width > 0 && review.native_width <= imported.imports[0].width);
        assert!(review.reference_width >= review.native_width);
        assert!(review.reference_scale >= 1);
        assert!(
            snap_preview_in(&settings, &project.id, &review.id, SnapArtifactKind::Native)
                .unwrap()
                .data_url
                .starts_with("data:image/png;base64,")
        );
        assert!(snap_preview_in(
            &settings,
            &project.id,
            &review.id,
            SnapArtifactKind::Reference
        )
        .unwrap()
        .data_url
        .starts_with("data:image/png;base64,"));
        let applied = apply_snap_in(&settings, &project.id, &review.id).unwrap();
        assert_eq!(applied.imports[0].stages["snap"], StageState::Complete);
        assert_eq!(
            applied.anchors["east"].active_snap_id.as_ref(),
            Some(&review.id)
        );
        assert!(
            read_project(&find_project(&settings, &project.id).unwrap().0)
                .unwrap()
                .snap_reviews[0]
                .applied_at
                .is_some()
        );
        if let Ok(python) = std::env::var("SPRITE_STUDIO_TEST_PYTHON") {
            let settings = configure_python_in(&settings_path, &python).unwrap();
            let candidate = run_cleanup_in(
                &settings,
                &project.id,
                "east",
                CleanupOptions {
                    background: "auto".into(),
                    tolerance: 18,
                    min_area: 2,
                },
            )
            .unwrap();
            assert_eq!(candidate.imports[0].stages["cleanup"], StageState::Review);
            assert_eq!(candidate.imports[0].stages["normalize"], StageState::Review);
            assert!(candidate.anchors["east"].active_cleanup_id.is_none());
            let cleanup = &candidate.cleanup_reviews[0];
            assert!(cleanup.cleaned_width < candidate.runtime.cell_width);
            assert_eq!(cleanup.normalized_width, candidate.runtime.cell_width);
            assert!(cleanup_preview_in(
                &settings,
                &project.id,
                &cleanup.id,
                CleanupArtifactKind::Cleaned
            )
            .is_ok());
            assert!(cleanup_preview_in(
                &settings,
                &project.id,
                &cleanup.id,
                CleanupArtifactKind::Normalized
            )
            .is_ok());
            let applied_cleanup = apply_cleanup_in(&settings, &project.id, &cleanup.id).unwrap();
            assert_eq!(
                applied_cleanup.imports[0].stages["cleanup"],
                StageState::Complete
            );
            assert_eq!(
                applied_cleanup.imports[0].stages["normalize"],
                StageState::Complete
            );
            assert_eq!(
                applied_cleanup.anchors["east"].active_cleanup_id.as_ref(),
                Some(&cleanup.id)
            );
            assert!(reference_export_folder_in(&settings, &project.id, "east")
                .unwrap()
                .is_none());
            let folder = export_reference_in(&settings, &project.id, "east").unwrap();
            let folder_path = Path::new(&folder);
            assert!(folder_path.starts_with(
                find_project(&settings, &project.id)
                    .unwrap()
                    .0
                    .join("exports")
            ));
            assert_eq!(
                fs::read(folder_path.join("normalized.png")).unwrap(),
                fs::read(
                    find_project(&settings, &project.id)
                        .unwrap()
                        .0
                        .join(&cleanup.normalized_relative_path)
                )
                .unwrap()
            );
            assert_eq!(
                reference_export_folder_in(&settings, &project.id, "east").unwrap(),
                Some(folder_path.to_path_buf())
            );
            assert_eq!(
                export_reference_in(&settings, &project.id, "east").unwrap(),
                folder
            );
            let second = run_cleanup_in(
                &settings,
                &project.id,
                "east",
                CleanupOptions {
                    background: "auto".into(),
                    tolerance: 0,
                    min_area: 0,
                },
            )
            .unwrap();
            assert_eq!(second.cleanup_reviews.len(), 2);
            assert_ne!(
                second.cleanup_reviews[0].cleaned_relative_path,
                second.cleanup_reviews[1].cleaned_relative_path
            );
            assert_eq!(
                second.anchors["east"].active_cleanup_id.as_ref(),
                Some(&cleanup.id)
            );
            let next_snap = run_snap_in(
                &settings,
                &project.id,
                "east",
                SnapOptions {
                    colors: 12,
                    pixel_size: Some(4),
                    palette: None,
                },
            )
            .unwrap();
            let next_snap_id = next_snap.snap_reviews.last().unwrap().id.clone();
            let after_snap = apply_snap_in(&settings, &project.id, &next_snap_id).unwrap();
            assert!(after_snap.anchors["east"].active_cleanup_id.is_none());
            assert!(reference_export_folder_in(&settings, &project.id, "east")
                .unwrap()
                .is_none());
            assert!(export_reference_in(&settings, &project.id, "east").is_err());
            assert_eq!(after_snap.imports[0].stages["cleanup"], StageState::Waiting);
            assert!(apply_cleanup_in(&settings, &project.id, &cleanup.id).is_err());
        }
        let new_import =
            import_into(&settings, &project.id, "east", source.to_str().unwrap()).unwrap();
        assert!(new_import.anchors["east"].active_snap_id.is_none());
        assert!(new_import.anchors["east"].active_cleanup_id.is_none());
        assert_eq!(
            new_import.snap_reviews.len(),
            if std::env::var("SPRITE_STUDIO_TEST_PYTHON").is_ok() {
                2
            } else {
                1
            }
        );
        assert!(apply_snap_in(&settings, &project.id, &review.id).is_err());
    }

    #[test]
    fn animation_alignment_and_preview_are_reviewed_and_invalidate_on_import() {
        let tmp = tempdir().unwrap();
        let settings = test_settings(tmp.path());
        let project = create_project_in(&settings, "Kangaroo", Preset::KangiFight).unwrap();
        let project = create_animation_in(&settings, &project.id, "idle", "right").unwrap();
        let animation_id = project.animations[0].id.clone();
        let source1 = tmp.path().join("one.png");
        let source2 = tmp.path().join("two.png");
        for (path, shift) in [(&source1, 0), (&source2, 3)] {
            let mut image = image::RgbaImage::new(128, 128);
            for y in 65..112 {
                for x in (51 + shift)..(77 + shift) {
                    image.put_pixel(x, y, image::Rgba([210, 120, 45, 255]));
                }
            }
            image.save(path).unwrap();
        }
        let imported = import_animation_frames_in(
            &settings,
            &project.id,
            &animation_id,
            vec![
                source1.to_string_lossy().into_owned(),
                source2.to_string_lossy().into_owned(),
            ],
        )
        .unwrap();
        let frames = &imported.animations[0].frames;
        assert_eq!(frames.len(), 2);
        assert_ne!(frames[0].relative_path, frames[1].relative_path);
        assert_eq!(
            imported.animations[0].stages["frames"],
            StageState::Complete
        );
        assert!(
            animation_frame_preview_in(&settings, &project.id, &animation_id, &frames[0].id)
                .is_ok()
        );
        let proposed = propose_alignment_in(
            &settings,
            &project.id,
            &animation_id,
            vec![
                FrameOffset {
                    frame_id: frames[0].id.clone(),
                    x: 0,
                    y: 0,
                },
                FrameOffset {
                    frame_id: frames[1].id.clone(),
                    x: -3,
                    y: 0,
                },
            ],
        )
        .unwrap();
        assert_eq!(proposed.animations[0].stages["align"], StageState::Review);
        assert!(proposed.animations[0].active_alignment_id.is_none());
        let alignment_id = proposed.animations[0].alignments[0].id.clone();
        let applied =
            apply_alignment_in(&settings, &project.id, &animation_id, &alignment_id).unwrap();
        assert_eq!(applied.animations[0].stages["align"], StageState::Complete);
        assert_eq!(
            applied.animations[0].active_alignment_id.as_deref(),
            Some(alignment_id.as_str())
        );
        let candidate = propose_preview_in(&settings, &project.id, &animation_id, 8, true).unwrap();
        assert_eq!(
            candidate.animations[0].stages["preview"],
            StageState::Review
        );
        let preview_id = candidate.animations[0].previews[0].id.clone();
        let approved =
            apply_preview_in(&settings, &project.id, &animation_id, &preview_id).unwrap();
        assert_eq!(
            approved.animations[0].stages["preview"],
            StageState::Complete
        );
        assert_eq!(
            approved.animations[0].active_preview_id.as_deref(),
            Some(preview_id.as_str())
        );
        if let Ok(python) = std::env::var("SPRITE_STUDIO_TEST_PYTHON") {
            let settings_path = tmp.path().join("app-settings.json");
            atomic_json(&settings_path, &settings).unwrap();
            let python_settings = configure_python_in(&settings_path, &python).unwrap();
            let candidate = run_export_in(
                &python_settings,
                &project.id,
                &animation_id,
                ExportOptions {
                    columns: 2,
                    padding: 0,
                    spacing: 0,
                },
            )
            .unwrap();
            assert_eq!(candidate.animations[0].stages["export"], StageState::Review);
            let review = &candidate.animations[0].exports[0];
            assert!(review
                .sheet_relative_path
                .starts_with("exports/animations/"));
            assert_eq!(review.sheet_width, 256);
            assert_eq!(review.sheet_height, 128);
            let manifest =
                export_manifest_in(&python_settings, &project.id, &animation_id, &review.id)
                    .unwrap();
            assert_eq!(manifest["approvalState"], "DRAFT");
            assert_eq!(manifest["frameCount"], 2);
            assert!(export_asset_in(
                &python_settings,
                &project.id,
                &animation_id,
                &review.id,
                ExportArtifactKind::Sheet
            )
            .is_ok());
            assert!(export_asset_in(
                &python_settings,
                &project.id,
                &animation_id,
                &review.id,
                ExportArtifactKind::Gif
            )
            .is_ok());
            let applied_export =
                apply_export_in(&python_settings, &project.id, &animation_id, &review.id).unwrap();
            assert_eq!(
                applied_export.animations[0].stages["export"],
                StageState::Complete
            );
            let repeated = run_export_in(
                &python_settings,
                &project.id,
                &animation_id,
                ExportOptions {
                    columns: 2,
                    padding: 0,
                    spacing: 0,
                },
            )
            .unwrap();
            assert_eq!(repeated.animations[0].exports.len(), 2);
            assert_eq!(
                repeated.animations[0].exports[0].sheet_sha256,
                repeated.animations[0].exports[1].sheet_sha256
            );
            assert_eq!(
                repeated.animations[0].exports[0].gif_sha256,
                repeated.animations[0].exports[1].gif_sha256
            );
            assert_eq!(
                repeated.animations[0].exports[0].manifest_sha256,
                repeated.animations[0].exports[1].manifest_sha256
            );
            assert_ne!(
                repeated.animations[0].exports[0].sheet_relative_path,
                repeated.animations[0].exports[1].sheet_relative_path
            );
        }
        let extended = import_animation_frames_in(
            &settings,
            &project.id,
            &animation_id,
            vec![source1.to_string_lossy().into_owned()],
        )
        .unwrap();
        assert_eq!(extended.animations[0].frames.len(), 3);
        assert!(extended.animations[0].active_alignment_id.is_none());
        assert!(extended.animations[0].active_preview_id.is_none());
        assert!(extended.animations[0].active_export_id.is_none());
        assert_eq!(extended.animations[0].stages["align"], StageState::Waiting);
        assert!(apply_alignment_in(&settings, &project.id, &animation_id, &alignment_id).is_err());
        assert!(propose_preview_in(&settings, &project.id, &animation_id, 8, true).is_err());
    }

    #[test]
    fn pose_board_extraction_preserves_source_and_allows_curation() {
        let Ok(python) = std::env::var("SPRITE_STUDIO_TEST_PYTHON") else {
            return;
        };
        let tmp = tempdir().unwrap();
        let base = test_settings(tmp.path());
        let settings_path = tmp.path().join("app-settings.json");
        atomic_json(&settings_path, &base).unwrap();
        let settings = configure_python_in(&settings_path, &python).unwrap();
        let project = create_project_in(&settings, "Board", Preset::Generic).unwrap();
        let project = create_animation_in(&settings, &project.id, "attack", "south").unwrap();
        let animation_id = project.animations[0].id.clone();
        let source = tmp.path().join("board.png");
        let mut image = image::RgbImage::from_pixel(512, 256, image::Rgb([0, 255, 0]));
        for y in 60..185 {
            for x in 55..150 {
                image.put_pixel(x, y, image::Rgb([170, 68, 31]));
            }
        }
        for y in 45..185 {
            for x in 310..420 {
                image.put_pixel(x, y, image::Rgb([102, 57, 38]));
            }
        }
        image.save(&source).unwrap();
        let imported = import_board_in(
            &settings,
            &project.id,
            &animation_id,
            source.to_str().unwrap(),
        )
        .unwrap();
        let board = &imported.animations[0].boards[0];
        assert_eq!(
            board.sha256,
            format!("{:x}", Sha256::digest(fs::read(&source).unwrap()))
        );
        assert!(board_preview_in(&settings, &project.id, &animation_id, &board.id).is_ok());
        let candidate = run_extraction_in(
            &settings,
            &project.id,
            &animation_id,
            ExtractionOptions {
                background: "auto".into(),
                tolerance: 18,
                min_area: 100,
                merge_gap: 2,
            },
        )
        .unwrap();
        assert_eq!(
            candidate.animations[0].stages["extract"],
            StageState::Review
        );
        let review = &candidate.animations[0].extractions[0];
        assert_eq!(review.boxes.len(), 2);
        assert!(candidate.animations[0].active_extraction_id.is_none());
        assert!(extraction_candidate_preview_in(
            &settings,
            &project.id,
            &animation_id,
            &review.id,
            &review.boxes[0].id
        )
        .is_ok());
        let curated = apply_extraction_in(
            &settings,
            &project.id,
            &animation_id,
            &review.id,
            vec![review.boxes[1].id.clone(), review.boxes[0].id.clone()],
            vec![ManualCrop {
                x: 55,
                y: 60,
                width: 95,
                height: 125,
            }],
        )
        .unwrap();
        assert_eq!(
            curated.animations[0].stages["extract"],
            StageState::Complete
        );
        assert_eq!(curated.animations[0].active_raw_frame_ids.len(), 3);
        assert_eq!(
            curated.animations[0].raw_frames[0].width,
            review.boxes[1].width
        );
        assert!(raw_animation_frame_preview_in(
            &settings,
            &project.id,
            &animation_id,
            &curated.animations[0].active_raw_frame_ids[2]
        )
        .is_ok());
        assert!(fs::read(source).is_ok());
        if let Ok(snapper) = std::env::var("SPRITE_STUDIO_TEST_SNAPPER") {
            confirm_native_review_in(
                &settings,
                &project.id,
                &animation_id,
                curated.animations[0].active_raw_frame_ids.clone(),
            )
            .unwrap();
            let settings = configure_snapper_in(&settings_path, &snapper).unwrap();
            let snapped = run_batch_snap_in(
                &settings,
                &project.id,
                &animation_id,
                SnapOptions {
                    colors: 8,
                    pixel_size: Some(4),
                    palette: None,
                },
            )
            .unwrap();
            assert_eq!(snapped.animations[0].stages["snap"], StageState::Review);
            let snap = &snapped.animations[0].batch_snaps[0];
            assert_eq!(snap.frames.len(), 3);
            assert!(batch_frame_preview_in(
                &settings,
                &project.id,
                &animation_id,
                &snap.id,
                &snap.frames[0].source_frame_id,
                "snapped"
            )
            .is_ok());
            let applied =
                apply_batch_snap_in(&settings, &project.id, &animation_id, &snap.id).unwrap();
            assert_eq!(applied.animations[0].stages["snap"], StageState::Complete);
            let cleaned = run_batch_cleanup_in(
                &settings,
                &project.id,
                &animation_id,
                CleanupOptions {
                    background: "#00ff00".into(),
                    tolerance: 24,
                    min_area: 2,
                },
            )
            .unwrap();
            assert_eq!(cleaned.animations[0].stages["cleanup"], StageState::Review);
            let review = &cleaned.animations[0].batch_cleanups[0];
            assert_eq!(review.frames.len(), 3);
            assert!(batch_frame_preview_in(
                &settings,
                &project.id,
                &animation_id,
                &review.id,
                &review.frames[0].source_frame_id,
                "normalized"
            )
            .is_ok());
            approve_batch_clean_in(&settings, &project.id, &animation_id, &review.id).unwrap();
            let applied =
                apply_batch_cleanup_in(&settings, &project.id, &animation_id, &review.id).unwrap();
            assert_eq!(applied.animations[0].frames.len(), 3);
            assert_eq!(applied.animations[0].stages["frames"], StageState::Complete);
            assert!(read_project(&find_project(&settings, &project.id).unwrap().0).is_ok());
        }
    }

    #[test]
    fn kangifight_batch_auto_grid_fits_without_normalization_scaling() {
        let (Ok(python), Ok(snapper)) = (
            std::env::var("SPRITE_STUDIO_TEST_PYTHON"),
            std::env::var("SPRITE_STUDIO_TEST_SNAPPER"),
        ) else {
            return;
        };
        let tmp = tempdir().unwrap();
        let base = test_settings(tmp.path());
        let settings_path = tmp.path().join("app-settings.json");
        atomic_json(&settings_path, &base).unwrap();
        configure_python_in(&settings_path, &python).unwrap();
        let settings = configure_snapper_in(&settings_path, &snapper).unwrap();
        let project = create_project_in(&settings, "Tall Fighter", Preset::KangiFight).unwrap();
        let project = create_animation_in(&settings, &project.id, "idle", "right").unwrap();
        let animation_id = project.animations[0].id.clone();
        let source = tmp.path().join("board.png");
        let mut image = image::RgbImage::from_pixel(512, 1024, image::Rgb([0, 255, 0]));
        for y in 48..978 {
            for x in 106..406 {
                image.put_pixel(x, y, image::Rgb([210, 106, 53]));
            }
        }
        image.save(&source).unwrap();
        import_board_in(
            &settings,
            &project.id,
            &animation_id,
            source.to_str().unwrap(),
        )
        .unwrap();
        let detected = run_extraction_in(
            &settings,
            &project.id,
            &animation_id,
            ExtractionOptions {
                background: "auto".into(),
                tolerance: 18,
                min_area: 100,
                merge_gap: 2,
            },
        )
        .unwrap();
        let review = &detected.animations[0].extractions[0];
        let curated = apply_extraction_in(
            &settings,
            &project.id,
            &animation_id,
            &review.id,
            vec![review.boxes[0].id.clone()],
            vec![],
        )
        .unwrap();
        confirm_native_review_in(
            &settings,
            &project.id,
            &animation_id,
            curated.animations[0].active_raw_frame_ids.clone(),
        )
        .unwrap();
        let snapped = run_batch_snap_in(
            &settings,
            &project.id,
            &animation_id,
            SnapOptions {
                colors: 16,
                pixel_size: None,
                palette: None,
            },
        )
        .unwrap();
        let review = &snapped.animations[0].batch_snaps[0];
        assert_eq!(review.pixel_size, Some(16));
        assert!(review.frames[0].height <= 68);
        apply_batch_snap_in(&settings, &project.id, &animation_id, &review.id).unwrap();
        let cleaned = run_batch_cleanup_in(
            &settings,
            &project.id,
            &animation_id,
            CleanupOptions {
                background: "#00ff00".into(),
                tolerance: 24,
                min_area: 2,
            },
        )
        .unwrap();
        let review = &cleaned.animations[0].batch_cleanups[0];
        approve_batch_clean_in(&settings, &project.id, &animation_id, &review.id).unwrap();
        let applied =
            apply_batch_cleanup_in(&settings, &project.id, &animation_id, &review.id).unwrap();
        assert_eq!(applied.animations[0].frames[0].width, 128);
        assert_eq!(applied.animations[0].frames[0].height, 128);
        assert_eq!(
            applied.animations[0].stages["normalize"],
            StageState::Complete
        );
    }

    #[test]
    fn installed_batch_auto_fit_preserves_applied_frames_until_review_apply() {
        let (Ok(python), Ok(snapper)) = (
            std::env::var("SPRITE_STUDIO_TEST_PYTHON"),
            std::env::var("SPRITE_STUDIO_TEST_SNAPPER"),
        ) else {
            return;
        };
        let tmp = tempdir().unwrap();
        let base = test_settings(tmp.path());
        let settings_path = tmp.path().join("app-settings.json");
        atomic_json(&settings_path, &base).unwrap();
        configure_python_in(&settings_path, &python).unwrap();
        let settings = configure_snapper_in(&settings_path, &snapper).unwrap();
        let project = create_project_in(&settings, "Batch Fit", Preset::Generic).unwrap();
        update_runtime_in(
            &settings,
            &project.id,
            128,
            128,
            64,
            112,
            AnchorMode::Custom,
        )
        .unwrap();
        update_runtime_policy_in(&settings, &project.id, GeometryPolicy::Locked, None, 4).unwrap();
        let project = create_animation_in(&settings, &project.id, "idle", "east").unwrap();
        let animation_id = project.animations[0].id.clone();
        let source = tmp.path().join("board.png");
        let mut image = image::RgbImage::from_pixel(256, 256, image::Rgb([0, 255, 0]));
        for y in 20..220 {
            for x in 40..220 {
                image.put_pixel(x, y, image::Rgb([210, 106, 53]));
            }
        }
        image.save(&source).unwrap();
        let original_board_bytes = fs::read(&source).unwrap();
        import_board_in(
            &settings,
            &project.id,
            &animation_id,
            source.to_str().unwrap(),
        )
        .unwrap();
        let detected = run_extraction_in(
            &settings,
            &project.id,
            &animation_id,
            ExtractionOptions {
                background: "auto".into(),
                tolerance: 18,
                min_area: 100,
                merge_gap: 2,
            },
        )
        .unwrap();
        let extraction = &detected.animations[0].extractions[0];
        let curated = apply_extraction_in(
            &settings,
            &project.id,
            &animation_id,
            &extraction.id,
            vec![extraction.boxes[0].id.clone()],
            vec![],
        )
        .unwrap();
        confirm_native_review_in(
            &settings,
            &project.id,
            &animation_id,
            curated.animations[0].active_raw_frame_ids.clone(),
        )
        .unwrap();
        let first = run_batch_snap_in(
            &settings,
            &project.id,
            &animation_id,
            SnapOptions {
                colors: 16,
                pixel_size: Some(1),
                palette: None,
            },
        )
        .unwrap();
        let active_id = first.animations[0].batch_snaps.last().unwrap().id.clone();
        apply_batch_snap_in(&settings, &project.id, &animation_id, &active_id).unwrap();
        let options = CleanupOptions {
            background: "#00ff00".into(),
            tolerance: 24,
            min_area: 2,
        };
        assert!(
            run_batch_cleanup_in(&settings, &project.id, &animation_id, options.clone()).is_err()
        );
        let fitted =
            run_batch_auto_fit_in(&settings, &project.id, &animation_id, options.clone()).unwrap();
        let animation = &fitted.animations[0];
        let run = animation.batch_auto_fit_runs.last().unwrap();
        let selected_id = run.selected_review_id.as_ref().unwrap();
        assert!(!run.candidates[0].fits);
        assert!(run.candidates[1].fits);
        assert_eq!(
            animation.active_batch_snap_id.as_deref(),
            Some(active_id.as_str())
        );
        assert!(run
            .candidates
            .iter()
            .find(|item| &item.review_id == selected_id)
            .unwrap()
            .frames
            .iter()
            .all(|frame| frame.fits));
        let applied =
            apply_batch_snap_in(&settings, &project.id, &animation_id, selected_id).unwrap();
        assert_eq!(
            applied.animations[0].active_batch_snap_id.as_deref(),
            Some(selected_id.as_str())
        );
        assert_eq!(
            (
                applied.runtime.cell_width,
                applied.runtime.cell_height,
                applied.runtime.pivot_x,
                applied.runtime.pivot_y
            ),
            (128, 128, 64, 112)
        );
        let cleaned = run_batch_cleanup_in(&settings, &project.id, &animation_id, options).unwrap();
        assert_eq!(
            cleaned.animations[0]
                .batch_cleanups
                .last()
                .unwrap()
                .frames
                .len(),
            1
        );
        assert_eq!(fs::read(&source).unwrap(), original_board_bytes);
    }

    #[test]
    fn manual_upscale_handoff_versions_inputs_and_resumes_after_restart() {
        let tmp = tempdir().unwrap();
        let settings = test_settings(tmp.path());
        let mut project = create_project_in(&settings, "Handoff", Preset::Generic).unwrap();
        project = create_animation_in(&settings, &project.id, "idle", "south").unwrap();
        let animation_id = project.animations[0].id.clone();
        let (dir, _) = find_project(&settings, &project.id).unwrap();
        let source = tmp.path().join("orange.png");
        sample_png(&source);
        let bytes = fs::read(&source).unwrap();
        let hash = format!("{:x}", Sha256::digest(&bytes));
        let now = Utc::now();
        let board_id = Uuid::new_v4().to_string();
        let extraction_id = Uuid::new_v4().to_string();
        let snap_id = Uuid::new_v4().to_string();
        let board_path = format!("animations/{animation_id}/board/raw/{board_id}.png");
        fs::create_dir_all(dir.join(format!("animations/{animation_id}/board/raw"))).unwrap();
        fs::write(dir.join(&board_path), &bytes).unwrap();
        let animation = &mut project.animations[0];
        animation.boards.push(BoardImport {
            id: board_id.clone(),
            original_name: "orange.png".into(),
            relative_path: board_path,
            sha256: hash.clone(),
            mime_type: "image/png".into(),
            width: 4,
            height: 4,
            imported_at: now,
        });
        animation.active_board_id = Some(board_id.clone());
        animation.extractions.push(ExtractionReview {
            id: extraction_id.clone(),
            board_import_id: board_id,
            boxes: vec![],
            background_hex: "00ff00".into(),
            tolerance: 24,
            min_area: 1,
            merge_gap: 0,
            created_at: now,
            applied_at: Some(now),
        });
        animation.active_extraction_id = Some(extraction_id.clone());
        let mut snapped = Vec::new();
        for index in 0..2 {
            let frame_id = Uuid::new_v4().to_string();
            let raw_path = format!("animations/{animation_id}/frames_raw/frame_{index:03}.png");
            let snap_path =
                format!("animations/{animation_id}/batch_snap/{snap_id}/{index:03}.png");
            for path in [&raw_path, &snap_path] {
                fs::create_dir_all(dir.join(path).parent().unwrap()).unwrap();
                fs::write(dir.join(path), &bytes).unwrap();
            }
            animation.raw_frames.push(RawAnimationFrame {
                id: frame_id.clone(),
                extraction_id: extraction_id.clone(),
                relative_path: raw_path,
                sha256: hash.clone(),
                width: 4,
                height: 4,
                created_at: now,
            });
            animation.active_raw_frame_ids.push(frame_id.clone());
            snapped.push(BatchFrameArtifact {
                source_frame_id: frame_id,
                relative_path: snap_path,
                sha256: hash.clone(),
                width: 4,
                height: 4,
                upscaled_relative_path: None,
                chroma_relative_path: None,
                outputs: None,
            });
        }
        animation.batch_snaps.push(BatchSnapReview {
            id: snap_id.clone(),
            extraction_id,
            source_frame_ids: animation.active_raw_frame_ids.clone(),
            frames: snapped,
            colors: 256,
            pixel_size: Some(1),
            detected_pixel_size: Some(1.0),
            auto_fit_run_id: None,
            palette: None,
            tool_version: "fixture".into(),
            created_at: now,
            applied_at: Some(now),
        });
        animation.active_batch_snap_id = Some(snap_id);
        animation.stages.insert("snap".into(), StageState::Complete);
        atomic_json(&dir.join("project.json"), &project).unwrap();
        let handoff_root = tmp.path().join("handoff-export");
        fs::create_dir(&handoff_root).unwrap();
        let exported = export_snapped_in(
            &settings,
            &project.id,
            &animation_id,
            handoff_root.to_str().unwrap(),
            vec![],
        )
        .unwrap();
        assert_eq!(
            fs::read(Path::new(&exported).join("frame_001-native.png")).unwrap(),
            bytes
        );
        assert_eq!(
            fs::read(Path::new(&exported).join("frame_002-native.png")).unwrap(),
            bytes
        );
        assert!(export_snapped_in(
            &settings,
            &project.id,
            &animation_id,
            handoff_root.to_str().unwrap(),
            vec![]
        )
        .is_err());
        if let Ok(python) = std::env::var("SPRITE_STUDIO_TEST_PYTHON") {
            let mut with_python = settings.clone();
            with_python.python_executable = Some(python);
            let automatic = run_batch_cleanup_in(
                &with_python,
                &project.id,
                &animation_id,
                CleanupOptions {
                    background: "#00ff00".into(),
                    tolerance: 24,
                    min_area: 2,
                },
            )
            .unwrap();
            assert!(automatic.animations[0]
                .batch_cleanups
                .last()
                .unwrap()
                .upscale_frame_ids
                .is_empty());
        }
        let mut workflow = project.workflow.clone();
        workflow.upscale_mode = UpscaleMode::ManualHandoff;
        let project = update_workflow_in(&settings, &project.id, workflow).unwrap();
        assert_eq!(
            project.animations[0].stages["upscale"],
            StageState::NeedsInput
        );
        let ids = project.animations[0].active_raw_frame_ids.clone();
        assert!(approve_upscale_in(&settings, &project.id, &animation_id).is_err());
        let damaged = tmp.path().join("damaged.png");
        fs::write(&damaged, b"not a png").unwrap();
        assert!(import_upscaled_frame_in(
            &settings,
            &project.id,
            &animation_id,
            &ids[0],
            damaged.to_str().unwrap()
        )
        .is_err());
        let first = import_upscaled_frame_in(
            &settings,
            &project.id,
            &animation_id,
            &ids[0],
            source.to_str().unwrap(),
        )
        .unwrap();
        assert_eq!(first.animations[0].upscaled_frames[0].version, 1);
        assert!(approve_upscale_in(&settings, &project.id, &animation_id).is_err());
        import_upscaled_frame_in(
            &settings,
            &project.id,
            &animation_id,
            &ids[1],
            source.to_str().unwrap(),
        )
        .unwrap();
        let approved = approve_upscale_in(&settings, &project.id, &animation_id).unwrap();
        assert!(approved.animations[0].upscale_approved);
        let reloaded = read_project(&dir).unwrap();
        assert_eq!(
            reloaded.animations[0].stages["upscale"],
            StageState::Complete
        );
        if let Ok(python) = std::env::var("SPRITE_STUDIO_TEST_PYTHON") {
            let mut with_python = settings.clone();
            with_python.python_executable = Some(python);
            let cleaned = run_batch_cleanup_in(
                &with_python,
                &project.id,
                &animation_id,
                CleanupOptions {
                    background: "#00ff00".into(),
                    tolerance: 24,
                    min_area: 2,
                },
            )
            .unwrap();
            let cleanup = cleaned.animations[0].batch_cleanups.last().unwrap();
            assert_eq!(cleanup.upscale_frame_ids.len(), 2);
            approve_batch_clean_in(&with_python, &project.id, &animation_id, &cleanup.id).unwrap();
            let applied =
                apply_batch_cleanup_in(&with_python, &project.id, &animation_id, &cleanup.id)
                    .unwrap();
            assert_eq!(applied.animations[0].frames.len(), 2);
        }
        let versioned = import_upscaled_frame_in(
            &settings,
            &project.id,
            &animation_id,
            &ids[0],
            source.to_str().unwrap(),
        )
        .unwrap();
        assert_eq!(
            versioned.animations[0]
                .upscaled_frames
                .last()
                .unwrap()
                .version,
            2
        );
        assert!(!versioned.animations[0].upscale_approved);
        assert_eq!(versioned.animations[0].stages["cleanup"], StageState::Stale);
        if std::env::var("SPRITE_STUDIO_TEST_PYTHON").is_ok() {
            assert_eq!(versioned.animations[0].historical_frames.len(), 2);
            assert!(versioned.animations[0].frames.is_empty());
        }
        let earlier = &versioned.animations[0].upscaled_frames[0];
        let latest = versioned.animations[0].upscaled_frames.last().unwrap();
        assert_ne!(earlier.relative_path, latest.relative_path);
        for frame in [earlier, latest] {
            assert_eq!(fs::read(dir.join(&frame.relative_path)).unwrap(), bytes);
        }
        let old_raw_path = versioned.animations[0].raw_frames[0].relative_path.clone();
        let recropped = replace_raw_frame_in(
            &settings,
            &project.id,
            &animation_id,
            &ids[0],
            None,
            Some(ManualCrop {
                x: 0,
                y: 0,
                width: 4,
                height: 4,
            }),
        )
        .unwrap();
        let new_raw_id = recropped.animations[0].active_raw_frame_ids[0].clone();
        assert_ne!(new_raw_id, ids[0]);
        assert_eq!(
            recropped.animations[0].stages["native_review"],
            StageState::Review
        );
        let replaced = replace_raw_frame_in(
            &settings,
            &project.id,
            &animation_id,
            &new_raw_id,
            Some(source.to_str().unwrap()),
            None,
        )
        .unwrap();
        assert_ne!(replaced.animations[0].active_raw_frame_ids[0], new_raw_id);
        assert_eq!(fs::read(dir.join(old_raw_path)).unwrap(), bytes);
        assert!(read_project(&dir).is_ok());
    }

    #[test]
    fn older_projects_receive_workflow_and_stage_defaults_on_open() {
        let tmp = tempdir().unwrap();
        let settings = test_settings(tmp.path());
        let project = create_project_in(&settings, "Legacy", Preset::Generic).unwrap();
        let project = create_animation_in(&settings, &project.id, "idle", "south").unwrap();
        let (dir, _) = find_project(&settings, &project.id).unwrap();
        let mut metadata: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("project.json")).unwrap()).unwrap();
        metadata.as_object_mut().unwrap().remove("workflow");
        metadata["runtime"]["pivotY"] = serde_json::json!(245);
        let stages = metadata["animations"][0]["stages"].as_object_mut().unwrap();
        stages.remove("native_review");
        stages.remove("upscale");
        atomic_json(&dir.join("project.json"), &metadata).unwrap();
        let reopened = read_project(&dir).unwrap();
        assert_eq!(reopened.workflow.k_colors, 256);
        assert_eq!(reopened.workflow.anchor_mode, AnchorMode::Custom);
        assert_eq!(reopened.runtime.pivot_y, 245);
        assert_eq!(
            reopened.animations[0].stages["native_review"],
            StageState::Waiting
        );
        assert_eq!(
            reopened.animations[0].stages["upscale"],
            StageState::Waiting
        );
    }
}
