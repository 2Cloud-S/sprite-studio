//! Conservative, metadata-only reconciliation of an interrupted animation workflow.
//! This module deliberately edits JSON values, not typed project structs, so fields
//! added by a newer Sprite Studio version survive a repair unchanged.
use chrono::{DateTime, Utc};
use image::ImageFormat;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

type Result<T> = std::result::Result<T, String>;

#[derive(Debug)]
pub struct ReconcileEntry {
    pub animation: String,
    pub result: String,
}

#[derive(Debug)]
pub struct ReconcileReport {
    pub recovered_complete: usize,
    pub recovered_review: usize,
    pub still_stale: usize,
    pub missing_or_corrupt: usize,
    pub unchanged: usize,
    pub backup_path: Option<PathBuf>,
    pub entries: Vec<ReconcileEntry>,
}

struct RecoveryPlan {
    cleanup_id: String,
    alignment_id: String,
    preview_id: String,
    export_id: String,
    export_applied: bool,
    frame_ids: Vec<String>,
    cleanup_settings: Value,
}

fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Missing {key}"))
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("Missing {key}"))
}

fn timestamp(value: &Value, key: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(string(value, key)?)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| format!("Invalid {key}"))
}

fn verified_file(root: &Path, relative: &str, expected_hash: &str) -> Result<Vec<u8>> {
    let rel = Path::new(relative);
    if rel.as_os_str().is_empty()
        || rel
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!("Unsafe asset path: {relative}"));
    }
    let path = root.join(rel);
    let resolved = fs::canonicalize(&path).map_err(|_| format!("Missing asset: {relative}"))?;
    if !resolved.starts_with(root) || !resolved.is_file() {
        return Err(format!("Asset escapes project: {relative}"));
    }
    let bytes = fs::read(resolved).map_err(|_| format!("Cannot read asset: {relative}"))?;
    if format!("{:x}", Sha256::digest(&bytes)) != expected_hash.to_ascii_lowercase() {
        return Err(format!("Hash mismatch: {relative}"));
    }
    Ok(bytes)
}

fn verified_png(root: &Path, relative: &str, hash: &str, width: u32, height: u32) -> Result<()> {
    let bytes = verified_file(root, relative, hash)?;
    let image = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
        .map_err(|_| format!("Damaged PNG: {relative}"))?;
    if image.width() != width || image.height() != height {
        return Err(format!("Wrong PNG dimensions: {relative}"));
    }
    Ok(())
}

fn review<'a>(animation: &'a Value, collection: &str, id: &str) -> Result<&'a Value> {
    array(animation, collection)?
        .iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
        .ok_or_else(|| format!("Missing {collection} review {id}"))
}

fn plan_for(root: &Path, project: &Value, animation: &Value) -> Result<RecoveryPlan> {
    let animation_id = string(animation, "id")?;
    let current_snap_id = string(animation, "activeBatchSnapId")?;
    let active_raw: Vec<&str> = array(animation, "activeRawFrameIds")?
        .iter()
        .map(|value| value.as_str().ok_or("Invalid raw frame ID".into()))
        .collect::<Result<_>>()?;
    let snap = review(animation, "batchSnaps", current_snap_id)?;
    if snap.get("appliedAt").and_then(Value::as_str).is_none()
        || string(snap, "extractionId")? != string(animation, "activeExtractionId")?
        || array(snap, "sourceFrameIds")?
            .iter()
            .map(Value::as_str)
            .collect::<Vec<_>>()
            != active_raw.iter().map(|id| Some(*id)).collect::<Vec<_>>()
    {
        return Err("Current snap/accepted raw-frame chain does not match".into());
    }
    for frame in array(snap, "frames")? {
        verified_file(
            root,
            string(frame, "relativePath")?,
            string(frame, "sha256")?,
        )?;
    }
    for id in &active_raw {
        let frame = review(animation, "rawFrames", id)?;
        verified_file(
            root,
            string(frame, "relativePath")?,
            string(frame, "sha256")?,
        )?;
    }

    let export = array(animation, "exports")?
        .last()
        .ok_or("No export review exists")?;
    let export_applied = export.get("appliedAt").and_then(Value::as_str).is_some();
    let export_time = timestamp(export, "createdAt")?;
    if array(animation, "batchCleanups")?
        .iter()
        .any(|item| timestamp(item, "createdAt").is_ok_and(|time| time > export_time))
    {
        return Err("A newer cleanup review exists; keep the current work in progress".into());
    }
    if array(animation, "batchSnaps")?
        .iter()
        .any(|item| timestamp(item, "createdAt").is_ok_and(|time| time > export_time))
    {
        return Err("A newer snap review exists; keep the current work in progress".into());
    }
    let alignment_id = string(export, "alignmentId")?;
    let preview_id = string(export, "previewId")?;
    let alignment = review(animation, "alignments", alignment_id)?;
    let preview = review(animation, "previews", preview_id)?;
    if alignment.get("appliedAt").and_then(Value::as_str).is_none()
        || preview.get("appliedAt").and_then(Value::as_str).is_none()
        || string(preview, "alignmentId")? != alignment_id
    {
        return Err("Alignment or timed preview was not applied".into());
    }
    let sidecar: Value = serde_json::from_slice(&verified_file(
        root,
        string(alignment, "relativePath")?,
        string(alignment, "sha256")?,
    )?)
    .map_err(|_| "Damaged alignment sidecar")?;
    if sidecar.get("offsets") != alignment.get("offsets") {
        return Err("Alignment offsets do not match saved sidecar".into());
    }

    let manifest: Value = serde_json::from_slice(&verified_file(
        root,
        string(export, "manifestRelativePath")?,
        string(export, "manifestSha256")?,
    )?)
    .map_err(|_| "Damaged export manifest")?;
    if string(&manifest, "projectId")? != string(project, "id")?
        || string(&manifest, "animationId")? != animation_id
        || manifest.get("fps") != preview.get("fps")
        || manifest.get("looping") != preview.get("looping")
        || manifest.pointer("/sheet/sha256") != export.get("sheetSha256")
        || manifest.pointer("/previewGif/sha256") != export.get("gifSha256")
    {
        return Err("Export manifest dependency metadata does not match".into());
    }
    let runtime = project.get("runtime").ok_or("Missing runtime geometry")?;
    let width = runtime
        .get("cellWidth")
        .and_then(Value::as_u64)
        .ok_or("Missing cell width")? as u32;
    let height = runtime
        .get("cellHeight")
        .and_then(Value::as_u64)
        .ok_or("Missing cell height")? as u32;
    if manifest.get("frameWidth").and_then(Value::as_u64) != Some(width as u64)
        || manifest.get("frameHeight").and_then(Value::as_u64) != Some(height as u64)
        || manifest.pointer("/pivot/x") != runtime.get("pivotX")
        || manifest.pointer("/pivot/y") != runtime.get("pivotY")
    {
        return Err("Export geometry differs from current runtime".into());
    }
    verified_png(
        root,
        string(export, "sheetRelativePath")?,
        string(export, "sheetSha256")?,
        export
            .get("sheetWidth")
            .and_then(Value::as_u64)
            .ok_or("Missing sheet width")? as u32,
        export
            .get("sheetHeight")
            .and_then(Value::as_u64)
            .ok_or("Missing sheet height")? as u32,
    )?;
    verified_file(
        root,
        string(export, "gifRelativePath")?,
        string(export, "gifSha256")?,
    )?;

    let manifest_frames = array(&manifest, "frames")?;
    let offsets = array(alignment, "offsets")?;
    if manifest_frames.is_empty()
        || manifest_frames.len() != offsets.len()
        || manifest.get("frameCount").and_then(Value::as_u64) != Some(manifest_frames.len() as u64)
    {
        return Err("Export frame count does not match alignment".into());
    }
    let all_frames = array(animation, "historicalFrames")?
        .iter()
        .chain(array(animation, "frames")?.iter());
    let mut frame_ids = Vec::new();
    let mut frame_hashes = Vec::new();
    for (index, (manifest_frame, offset)) in manifest_frames.iter().zip(offsets).enumerate() {
        let id = string(manifest_frame, "frameId")?;
        if manifest_frame.get("index").and_then(Value::as_u64) != Some(index as u64)
            || string(offset, "frameId")? != id
            || manifest_frame.pointer("/offset/x") != offset.get("x")
            || manifest_frame.pointer("/offset/y") != offset.get("y")
        {
            return Err("Manifest frame order or alignment offset differs".into());
        }
        let record = all_frames
            .clone()
            .find(|frame| frame.get("id").and_then(Value::as_str) == Some(id))
            .ok_or_else(|| format!("Normalized frame record missing: {id}"))?;
        let hash = string(record, "sha256")?;
        if hash != string(manifest_frame, "sourceSha256")?
            || record.get("width").and_then(Value::as_u64) != Some(width as u64)
            || record.get("height").and_then(Value::as_u64) != Some(height as u64)
        {
            return Err(format!("Normalized frame differs from manifest: {id}"));
        }
        verified_png(root, string(record, "relativePath")?, hash, width, height)?;
        frame_ids.push(id.to_owned());
        frame_hashes.push(hash.to_owned());
    }
    if frame_ids.iter().collect::<BTreeSet<_>>().len() != frame_ids.len() {
        return Err("Duplicate normalized frame IDs".into());
    }
    let cleanup = array(animation, "batchCleanups")?
        .iter()
        .rev()
        .find(|item| {
            item.get("appliedAt").and_then(Value::as_str).is_some()
                && item.get("snapReviewId").and_then(Value::as_str) == Some(current_snap_id)
                && item
                    .get("frames")
                    .and_then(Value::as_array)
                    .is_some_and(|frames| {
                        frames.len() == frame_hashes.len()
                            && frames.iter().zip(&frame_hashes).all(|(frame, hash)| {
                                frame.get("normalizedSha256").and_then(Value::as_str) == Some(hash)
                            })
                    })
        })
        .ok_or("No applied cleanup matches the exported frame hashes and current snap")?;
    if let Some(saved) = animation
        .get("cleanupSettings")
        .filter(|value| !value.is_null())
    {
        if saved
            .get("background")
            .and_then(Value::as_str)
            .map(|value| value.trim_start_matches('#').to_ascii_lowercase())
            != cleanup
                .get("backgroundHex")
                .and_then(Value::as_str)
                .map(str::to_ascii_lowercase)
            || saved.get("tolerance") != cleanup.get("tolerance")
            || saved.get("minArea") != cleanup.get("minArea")
            || saved.get("greenFringeDespeckle") != cleanup.get("greenFringeDespeckle")
        {
            return Err("Current animation cleanup settings differ from exported cleanup".into());
        }
    }
    for (index, frame) in array(cleanup, "frames")?.iter().enumerate() {
        if string(frame, "sourceFrameId")? != active_raw[index] {
            return Err("Cleanup source order differs from active raw frames".into());
        }
        verified_file(
            root,
            string(frame, "cleanedRelativePath")?,
            string(frame, "cleanedSha256")?,
        )?;
        verified_png(
            root,
            string(frame, "normalizedRelativePath")?,
            string(frame, "normalizedSha256")?,
            width,
            height,
        )?;
    }
    Ok(RecoveryPlan {
        cleanup_id: string(cleanup, "id")?.into(),
        alignment_id: alignment_id.into(),
        preview_id: preview_id.into(),
        export_id: string(export, "id")?.into(),
        export_applied,
        frame_ids,
        cleanup_settings: json!({
            "background": string(cleanup, "backgroundHex")?,
            "tolerance": cleanup.get("tolerance"), "minArea": cleanup.get("minArea"),
            "greenFringeDespeckle": cleanup.get("greenFringeDespeckle"),
        }),
    })
}

fn apply_plan(animation: &mut Value, plan: &RecoveryPlan) -> Result<()> {
    let historical = animation
        .get_mut("historicalFrames")
        .and_then(Value::as_array_mut)
        .ok_or("Missing historical frames")?;
    let mut selected = Vec::new();
    for id in &plan.frame_ids {
        let index = historical
            .iter()
            .position(|frame| frame.get("id").and_then(Value::as_str) == Some(id))
            .ok_or("Recovered frame was not historical")?;
        selected.push(historical.remove(index));
    }
    animation["frames"] = Value::Array(selected);
    animation["activeBatchCleanupId"] = json!(plan.cleanup_id);
    animation["activeAlignmentId"] = json!(plan.alignment_id);
    animation["activePreviewId"] = json!(plan.preview_id);
    animation["activeExportId"] = if plan.export_applied {
        json!(plan.export_id)
    } else {
        Value::Null
    };
    if animation.get("cleanupSettings").is_none_or(Value::is_null) {
        animation["cleanupSettings"] = plan.cleanup_settings.clone();
    }
    for stage in [
        "cleanup",
        "normalize",
        "frames",
        "align",
        "preview",
        "export",
    ] {
        animation["stages"][stage] = json!(if stage == "export" && !plan.export_applied {
            "review"
        } else {
            "complete"
        });
    }
    animation["updatedAt"] = json!(Utc::now().to_rfc3339());
    Ok(())
}

pub fn reconcile_project_file(project_file: &Path, apply: bool) -> Result<ReconcileReport> {
    let root = fs::canonicalize(project_file.parent().ok_or("Missing project directory")?)
        .map_err(|error| error.to_string())?;
    let original =
        fs::read(project_file).map_err(|error| format!("Cannot read project: {error}"))?;
    let original_hash = Sha256::digest(&original);
    let mut project: Value = serde_json::from_slice(&original)
        .map_err(|error| format!("Project JSON is damaged: {error}"))?;
    let snapshot = project.clone();
    let mut report = ReconcileReport {
        recovered_complete: 0,
        recovered_review: 0,
        still_stale: 0,
        missing_or_corrupt: 0,
        unchanged: 0,
        backup_path: None,
        entries: Vec::new(),
    };
    let animations = array(&snapshot, "animations")?;
    for (index, animation) in animations.iter().enumerate() {
        let name = string(animation, "name")?.to_owned();
        let stale = animation.pointer("/stages/cleanup").and_then(Value::as_str) == Some("stale");
        if !stale
            || animation
                .get("activeBatchCleanupId")
                .is_some_and(|value| !value.is_null())
            || !array(animation, "frames")?.is_empty()
        {
            report.unchanged += 1;
            report.entries.push(ReconcileEntry {
                animation: name,
                result: "unchanged (current work or no stale cleanup)".into(),
            });
            continue;
        }
        match plan_for(&root, &snapshot, animation) {
            Ok(plan) => {
                if apply {
                    apply_plan(&mut project["animations"][index], &plan)?;
                }
                if plan.export_applied {
                    report.recovered_complete += 1;
                } else {
                    report.recovered_review += 1;
                }
                report.entries.push(ReconcileEntry {
                    animation: name,
                    result: if plan.export_applied {
                        "recoverable through applied export"
                    } else {
                        "recoverable through export review"
                    }
                    .into(),
                });
            }
            Err(reason) => {
                if reason.starts_with("Missing asset")
                    || reason.starts_with("Hash mismatch")
                    || reason.starts_with("Damaged")
                    || reason.starts_with("Wrong PNG")
                {
                    report.missing_or_corrupt += 1;
                } else {
                    report.still_stale += 1;
                }
                report.entries.push(ReconcileEntry {
                    animation: name,
                    result: reason,
                });
            }
        }
    }
    if apply && report.recovered_complete + report.recovered_review > 0 {
        project["updatedAt"] = json!(Utc::now().to_rfc3339());
        if Sha256::digest(fs::read(project_file).map_err(|error| error.to_string())?)
            != original_hash
        {
            return Err("Project changed during reconciliation; no repair was written".into());
        }
        let backup = root.join(format!("project.json.pre-reconcile-{}", Uuid::new_v4()));
        fs::copy(project_file, &backup)
            .map_err(|error| format!("Cannot preserve pre-reconcile metadata: {error}"))?;
        let temp = root.join(format!(".project-reconcile-{}.tmp", Uuid::new_v4()));
        let payload = serde_json::to_vec_pretty(&project).map_err(|error| error.to_string())?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| format!("Cannot prepare reconciliation: {error}"))?;
        file.write_all(&payload)
            .and_then(|_| file.sync_all())
            .map_err(|error| format!("Cannot write reconciliation: {error}"))?;
        let swap = root.join(format!(".project-before-swap-{}.json", Uuid::new_v4()));
        fs::rename(project_file, &swap)
            .map_err(|error| format!("Cannot stage old project: {error}"))?;
        if let Err(error) = fs::rename(&temp, project_file) {
            let _ = fs::rename(&swap, project_file);
            return Err(format!(
                "Cannot replace project: {error}; backup retained at {}",
                backup.display()
            ));
        }
        // Retain both backups; recovery must never delete the previous metadata.
        report.backup_path = Some(backup);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn fixture(root: &Path, export_applied: bool) -> PathBuf {
        let image = image::RgbaImage::from_pixel(4, 4, image::Rgba([180, 70, 30, 255]));
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut png, ImageFormat::Png)
            .unwrap();
        let png = png.into_inner();
        fs::write(root.join("frame.png"), &png).unwrap();
        let png_hash = format!("{:x}", Sha256::digest(&png));
        let gif = b"GIF89a test fixture";
        fs::write(root.join("preview.gif"), gif).unwrap();
        let gif_hash = format!("{:x}", Sha256::digest(gif));
        let offsets = json!([{"frameId":"frame-1","x":0,"y":0}]);
        let sidecar = json!({"schemaVersion":1,"reviewId":"align-1","offsets":offsets});
        let sidecar_bytes = serde_json::to_vec(&sidecar).unwrap();
        fs::write(root.join("alignment.json"), &sidecar_bytes).unwrap();
        let sidecar_hash = format!("{:x}", Sha256::digest(&sidecar_bytes));
        let manifest = json!({
            "projectId":"project-1", "animationId":"animation-1", "fps":8,
            "looping":true, "frameCount":1, "frameWidth":4, "frameHeight":4,
            "pivot":{"x":2,"y":3}, "sheet":{"sha256":png_hash},
            "previewGif":{"sha256":gif_hash},
            "frames":[{"index":0,"frameId":"frame-1","sourceSha256":png_hash,"offset":{"x":0,"y":0}}]
        });
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
        fs::write(root.join("manifest.json"), &manifest_bytes).unwrap();
        let manifest_hash = format!("{:x}", Sha256::digest(&manifest_bytes));
        let project = json!({
            "id":"project-1", "runtime":{"cellWidth":4,"cellHeight":4,"pivotX":2,"pivotY":3},
            "updatedAt":"2026-01-01T00:00:05Z", "futureProjectField":{"keep":true},
            "animations":[{
                "id":"animation-1", "name":"idle", "futureAnimationField":{"keep":"yes"},
                "activeExtractionId":"extract-1", "activeRawFrameIds":["raw-1"],
                "activeBatchSnapId":"snap-1", "activeBatchCleanupId":null,
                "activeAlignmentId":null,"activePreviewId":null,"activeExportId":null,
                "stages":{"cleanup":"stale","normalize":"stale","frames":"complete","align":"stale","preview":"stale","export":"stale","snap":"complete","native_review":"complete"},
                "frames":[],
                "historicalFrames":[{"id":"frame-1","relativePath":"frame.png","sha256":png_hash,"width":4,"height":4}],
                "rawFrames":[{"id":"raw-1","relativePath":"frame.png","sha256":png_hash}],
                "batchSnaps":[{"id":"snap-1","extractionId":"extract-1","sourceFrameIds":["raw-1"],"frames":[{"relativePath":"frame.png","sha256":png_hash}],"createdAt":"2026-01-01T00:00:00Z","appliedAt":"2026-01-01T00:00:01Z"}],
                "batchCleanups":[{"id":"clean-1","snapReviewId":"snap-1","frames":[{"sourceFrameId":"raw-1","cleanedRelativePath":"frame.png","cleanedSha256":png_hash,"normalizedRelativePath":"frame.png","normalizedSha256":png_hash}],"backgroundHex":"00ff00","tolerance":24,"minArea":2,"greenFringeDespeckle":true,"createdAt":"2026-01-01T00:00:02Z","appliedAt":"2026-01-01T00:00:03Z"}],
                "alignments":[{"id":"align-1","relativePath":"alignment.json","sha256":sidecar_hash,"offsets":offsets,"appliedAt":"2026-01-01T00:00:04Z"}],
                "previews":[{"id":"preview-1","alignmentId":"align-1","fps":8,"looping":true,"appliedAt":"2026-01-01T00:00:05Z"}],
                "exports":[{"id":"export-1","alignmentId":"align-1","previewId":"preview-1","sheetRelativePath":"frame.png","sheetSha256":png_hash,"sheetWidth":4,"sheetHeight":4,"gifRelativePath":"preview.gif","gifSha256":gif_hash,"manifestRelativePath":"manifest.json","manifestSha256":manifest_hash,"createdAt":"2026-01-01T00:00:06Z","appliedAt":if export_applied { json!("2026-01-01T00:00:07Z") } else { Value::Null }}]
            },{"id":"animation-2","name":"other","stages":{"cleanup":"complete"},"frames":[],"futureAnimationField":{"unchanged":true}}]
        });
        let file = root.join("project.json");
        fs::write(&file, serde_json::to_vec_pretty(&project).unwrap()).unwrap();
        file
    }

    #[test]
    fn reconciliation_recovers_only_verified_animation_and_preserves_unknown_fields() {
        let tmp = tempdir().unwrap();
        let file = fixture(tmp.path(), true);
        let before = fs::read(&file).unwrap();
        let dry = reconcile_project_file(&file, false).unwrap();
        assert_eq!(dry.recovered_complete, 1);
        assert_eq!(dry.unchanged, 1);
        assert_eq!(fs::read(&file).unwrap(), before);
        let applied = reconcile_project_file(&file, true).unwrap();
        assert_eq!(applied.recovered_complete, 1);
        assert!(applied.backup_path.unwrap().is_file());
        let project: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        assert_eq!(project["animations"][0]["stages"]["export"], "complete");
        assert_eq!(
            project["animations"][0]["frames"].as_array().unwrap().len(),
            1
        );
        assert_eq!(project["animations"][0]["activeBatchCleanupId"], "clean-1");
        assert_eq!(project["futureProjectField"]["keep"], true);
        assert_eq!(
            project["animations"][0]["futureAnimationField"]["keep"],
            "yes"
        );
        assert_eq!(
            project["animations"][1]["futureAnimationField"]["unchanged"],
            true
        );
        assert_eq!(
            reconcile_project_file(&file, false)
                .unwrap()
                .recovered_complete,
            0
        );
    }

    #[test]
    fn reconciliation_preserves_export_review_and_rejects_newer_or_damaged_inputs() {
        let tmp = tempdir().unwrap();
        let file = fixture(tmp.path(), false);
        let report = reconcile_project_file(&file, true).unwrap();
        assert_eq!(report.recovered_review, 1);
        let value: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        assert_eq!(value["animations"][0]["stages"]["export"], "review");
        assert!(value["animations"][0]["activeExportId"].is_null());

        let second = tempdir().unwrap();
        let file = fixture(second.path(), true);
        let mut value: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        value["animations"][0]["batchCleanups"]
            .as_array_mut()
            .unwrap()
            .push(json!({"createdAt":"2026-01-01T00:00:08Z"}));
        fs::write(&file, serde_json::to_vec(&value).unwrap()).unwrap();
        let report = reconcile_project_file(&file, false).unwrap();
        assert_eq!(report.recovered_complete, 0);
        assert_eq!(report.still_stale, 1);
        value["animations"][0]["batchCleanups"]
            .as_array_mut()
            .unwrap()
            .pop();
        fs::write(&file, serde_json::to_vec(&value).unwrap()).unwrap();
        fs::write(second.path().join("frame.png"), b"damaged").unwrap();
        let report = reconcile_project_file(&file, false).unwrap();
        assert_eq!(report.missing_or_corrupt, 1);
        assert_eq!(report.recovered_complete, 0);
    }
    #[test]
    fn unsafe_paths_are_rejected_without_reading_outside_project() {
        let root = std::env::temp_dir();
        assert!(verified_file(&root, "../other.png", "0").is_err());
        assert!(verified_file(&root, "C:/other.png", "0").is_err());
    }
}
