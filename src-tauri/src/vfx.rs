//! VFX domain on Sprite Studio's existing PNG/sidecar/locking primitives.
//! No character cleanup, component pruning, opaque-bounds centering or engine adapter.
use crate::asset_io::{
    checked_bytes, color, decode, distance, hash, metadata_candidates, png, rgb, save_metadata,
    secure_parent, write_bound,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{imageops, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;
type Result<T> = std::result::Result<T, String>;
const PROCESSOR: &str = "vfx.fixed-origin.v1";
fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
fn id() -> String {
    Uuid::new_v4().to_string()
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Palette {
    pub name: String,
    pub authority_id: Option<String>,
    pub package_reference: Option<String>,
    pub colors: Vec<String>,
    pub enforce: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub cell: [u32; 2],
    pub anchor: [i32; 2],
    pub source_orientation: String,
    pub mirror_policy: String,
    pub looping: bool,
    pub tick_hz: u32,
    pub hit_stop_policy: String,
    pub completion: String,
    pub allow_empty_terminal: bool,
    pub palette: Palette,
}
impl Profile {
    fn custom() -> Self {
        let mut p = Self::study();
        p.cell = [32, 32];
        p.anchor = [16, 16];
        p.source_orientation = "none".into();
        p.mirror_policy = "none".into();
        p.palette = Palette {
            name: "Unrestricted · pending review".into(),
            authority_id: None,
            package_reference: None,
            colors: vec![],
            enforce: false,
        };
        p
    }
    fn study() -> Self {
        Self {
            cell: [24, 24],
            anchor: [12, 12],
            source_orientation: "east".into(),
            mirror_policy: "horizontal_about_anchor".into(),
            looping: false,
            tick_hz: 60,
            hit_stop_policy: "pause".into(),
            completion: "hide".into(),
            allow_empty_terminal: false,
            palette: Palette {
                name: "Light Contact · proposed subset".into(),
                authority_id: None,
                package_reference: None,
                colors: vec!["#FFF1D2".into(), "#C9C2B8".into(), "#38344B".into()],
                enforce: true,
            },
        }
    }
}
fn validate_profile(p: &Profile) -> Result<()> {
    if p.cell.iter().any(|n| *n == 0 || *n > 2048)
        || p.anchor[0] < 0
        || p.anchor[1] < 0
        || p.anchor[0] >= p.cell[0] as i32
        || p.anchor[1] >= p.cell[1] as i32
    {
        return Err("Choose a 1–2048 fixed cell and an integer anchor inside it".into());
    }
    if p.tick_hz == 0
        || p.tick_hz > 1000
        || !["east", "west", "north", "south", "none"].contains(&p.source_orientation.as_str())
        || !["none", "horizontal_about_anchor"].contains(&p.mirror_policy.as_str())
        || !["pause", "continue"].contains(&p.hit_stop_policy.as_str())
        || !["hide", "empty_terminal"].contains(&p.completion.as_str())
    {
        return Err("Invalid timing/orientation/clock/completion profile".into());
    }
    if p.completion == "empty_terminal" && (!p.allow_empty_terminal || p.looping) {
        return Err("Empty-terminal completion requires a permitted terminal frame and non-looping playback".into());
    }
    if p.palette.colors.len() > 256
        || (p.palette.enforce && p.palette.colors.is_empty())
        || p.palette.name.len() > 160
    {
        return Err("Enforced palettes need 1–256 exact RGB colors".into());
    }
    for c in &p.palette.colors {
        color(c)?;
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Source {
    id: String,
    path: String,
    sha256: String,
    original_name: String,
    size: [u32; 2],
    imported_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Region {
    pub source_id: String,
    pub rect: [u32; 4],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Import {
    id: String,
    regions: Vec<Region>,
    approved_by: Option<String>,
    approved_at: Option<String>,
    created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Processing {
    pub offset: [i32; 2],
    pub pixel_pitch: u32,
    pub alpha_threshold: Option<u8>,
    pub chroma: Option<String>,
    pub tolerance: u8,
    pub remap_palette: bool,
}
impl Default for Processing {
    fn default() -> Self {
        Self {
            offset: [0, 0],
            pixel_pitch: 1,
            alpha_threshold: None,
            chroma: None,
            tolerance: 0,
            remap_palette: false,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Report {
    size: [u32; 2],
    opaque_pixels: usize,
    partial_alpha_pixels: usize,
    transparent_rgb_pixels: usize,
    unexpected_colors: Vec<String>,
    clipped_pixels: usize,
    edge_pixels: usize,
    possible_green_fringe_pixels: usize,
    bounds: Option<[u32; 4]>,
    errors: Vec<String>,
    warnings: Vec<String>,
}
fn inspect(im: &RgbaImage, profile: &Profile, terminal: bool, clipped: usize) -> Report {
    let mut r = Report {
        size: [im.width(), im.height()],
        opaque_pixels: 0,
        partial_alpha_pixels: 0,
        transparent_rgb_pixels: 0,
        unexpected_colors: vec![],
        clipped_pixels: clipped,
        edge_pixels: 0,
        possible_green_fringe_pixels: 0,
        bounds: None,
        errors: vec![],
        warnings: vec![],
    };
    let mut colors = BTreeSet::new();
    let mut bounds = [im.width(), im.height(), 0, 0];
    for (x, y, p) in im.enumerate_pixels() {
        if p[3] == 0 {
            if rgb(p) != [0, 0, 0] {
                r.transparent_rgb_pixels += 1;
            }
            continue;
        }
        r.opaque_pixels += 1;
        if p[3] != 255 {
            r.partial_alpha_pixels += 1;
        }
        colors.insert(format!("#{:02X}{:02X}{:02X}", p[0], p[1], p[2]));
        bounds[0] = bounds[0].min(x);
        bounds[1] = bounds[1].min(y);
        bounds[2] = bounds[2].max(x);
        bounds[3] = bounds[3].max(y);
        if x == 0 || y == 0 || x + 1 == im.width() || y + 1 == im.height() {
            r.edge_pixels += 1;
        }
        if p[1] as i32 > p[0] as i32 + 40 && p[1] as i32 > p[2] as i32 + 40 {
            r.possible_green_fringe_pixels += 1;
        }
    }
    if r.opaque_pixels > 0 {
        r.bounds = Some([
            bounds[0],
            bounds[1],
            bounds[2] - bounds[0] + 1,
            bounds[3] - bounds[1] + 1,
        ]);
    }
    r.unexpected_colors = colors
        .into_iter()
        .filter(|c| {
            profile.palette.enforce
                && !profile
                    .palette
                    .colors
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(c))
        })
        .collect();
    if r.size != profile.cell {
        r.errors.push("Fixed-cell dimensions differ".into());
    }
    if r.partial_alpha_pixels > 0 {
        r.errors
            .push("Non-binary alpha; review explicit thresholding".into());
    }
    if r.transparent_rgb_pixels > 0 {
        r.errors.push("Transparent RGB is not normalized".into());
    }
    if !r.unexpected_colors.is_empty() {
        r.errors
            .push("Unexpected RGB colors; choose an explicit remap or revise palette".into());
    }
    if clipped > 0 {
        r.errors.push(format!(
            "{clipped} nontransparent pixels would clip; correct placement/cell/pitch"
        ));
    }
    if r.opaque_pixels == 0 && !(terminal && profile.allow_empty_terminal) {
        r.errors
            .push("Empty frame requires explicit terminal-frame permission".into());
    }
    if r.edge_pixels > 0 {
        r.warnings
            .push("Visible pixels touch cell edges: review safety padding".into());
    }
    if r.possible_green_fringe_pixels > 0 {
        r.warnings.push(
            "Possible green fringe or intentional green: inspect without automatic deletion".into(),
        );
    }
    r
}
fn process(source: &RgbaImage, p: &Processing, profile: &Profile) -> Result<(RgbaImage, usize)> {
    if p.pixel_pitch == 0
        || p.pixel_pitch > 64
        || p.offset.iter().any(|v| v.unsigned_abs() > 4096)
        || p.alpha_threshold == Some(0)
    {
        return Err(
            "Integer pitch 1–64, bounded offsets and alpha threshold 1–255 required".into(),
        );
    }
    if source.width() % p.pixel_pitch != 0 || source.height() % p.pixel_pitch != 0 {
        return Err(
            "Pixel pitch must divide the complete source rectangle; no implicit edge crop".into(),
        );
    }
    let key = p.chroma.as_deref().map(color).transpose()?;
    let palette = profile
        .palette
        .colors
        .iter()
        .map(|c| color(c))
        .collect::<Result<Vec<_>>>()?;
    if p.remap_palette && palette.is_empty() {
        return Err("Choose a palette before explicit remapping".into());
    }
    // Explicit integer nearest sampling retains origin; no connected-component cleanup exists here.
    let scaled = imageops::resize(
        source,
        source.width() / p.pixel_pitch,
        source.height() / p.pixel_pitch,
        imageops::FilterType::Nearest,
    );
    let mut out = RgbaImage::new(profile.cell[0], profile.cell[1]);
    let mut clipped = 0;
    for (x, y, pixel) in scaled.enumerate_pixels() {
        let mut v = *pixel;
        if key.is_some_and(|k| distance(rgb(&v), k) <= u32::from(p.tolerance).pow(2)) {
            v = Rgba([0, 0, 0, 0]);
        }
        if let Some(t) = p.alpha_threshold {
            v[3] = if v[3] > 0 && v[3] >= t { 255 } else { 0 };
        }
        if v[3] == 0 {
            v = Rgba([0, 0, 0, 0]);
        } else if p.remap_palette {
            let c = palette
                .iter()
                .min_by_key(|c| distance(rgb(&v), **c))
                .unwrap();
            v[0] = c[0];
            v[1] = c[1];
            v[2] = c[2];
        }
        let tx = x as i32 + p.offset[0];
        let ty = y as i32 + p.offset[1];
        if tx < 0 || ty < 0 || tx >= profile.cell[0] as i32 || ty >= profile.cell[1] as i32 {
            if v[3] > 0 {
                clipped += 1;
            }
            continue;
        }
        out.put_pixel(tx as u32, ty as u32, v);
    }
    Ok((out, clipped))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Frame {
    id: String,
    source_id: String,
    rect: [u32; 4],
    path: String,
    sha256: String,
    processing: Processing,
    hold_ticks: u32,
    report: Report,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checks {
    pub artwork: bool,
    pub regions_and_anchor: bool,
    pub palette: bool,
    pub timing: bool,
    pub hit_stop_and_completion: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Approval {
    reviewer: String,
    approved_at: String,
    revision_hash: String,
    checks: Checks,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Revision {
    id: String,
    import_id: String,
    profile: Profile,
    frames: Vec<Frame>,
    approval: Option<Approval>,
    created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Export {
    id: String,
    path: String,
    png_hash: String,
    manifest_hash: String,
    source_manifest_hash: String,
    revision_id: String,
    exported_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Effect {
    schema_version: u32,
    id: String,
    project_id: String,
    effect_id: String,
    name: String,
    package: String,
    draft_profile: Profile,
    sources: Vec<Source>,
    imports: Vec<Import>,
    pending_import_id: Option<String>,
    active_import_id: Option<String>,
    revisions: Vec<Revision>,
    active_revision_id: Option<String>,
    exports: Vec<Export>,
    generation: u64,
    #[serde(default)]
    draft: Option<Draft>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Draft {
    pub profile: Profile,
    pub settings: Vec<Processing>,
    pub holds: Vec<u32>,
    pub base_revision_id: Option<String>,
    pub import_id: Option<String>,
}
fn draft_matches(d: &Draft, r: &Revision) -> bool {
    d.profile == r.profile
        && d.settings
            == r.frames
                .iter()
                .map(|f| f.processing.clone())
                .collect::<Vec<_>>()
        && d.holds == r.frames.iter().map(|f| f.hold_ticks).collect::<Vec<_>>()
}
fn valid_id(s: &str) -> Result<()> {
    Uuid::parse_str(s)
        .map(|_| ())
        .map_err(|_| "Invalid VFX storage ID".into())
}
fn effect_dir(root: &Path, project: &str, effect: &str) -> Result<PathBuf> {
    valid_id(project)?;
    valid_id(effect)?;
    Ok(root
        .join("vfx-projects")
        .join(project)
        .join("effects")
        .join(effect))
}
fn validate(e: &Effect, p: &str, id: &str) -> Result<()> {
    if e.schema_version != 1
        || e.id != id
        || e.project_id != p
        || e.sources.len() > 512
        || e.imports.len() > 512
        || e.revisions.len() > 512
    {
        return Err("Invalid or unsupported VFX metadata".into());
    }
    validate_profile(&e.draft_profile)?;
    if let Some(d) = &e.draft {
        validate_profile(&d.profile)?;
        let count = e
            .imports
            .iter()
            .find(|i| Some(&i.id) == e.active_import_id.as_ref())
            .map_or(0, |i| i.regions.len());
        if d.base_revision_id != e.active_revision_id
            || d.import_id != e.active_import_id
            || d.settings.len() != count
            || d.holds.len() != count
            || d.holds.iter().any(|t| *t == 0 || *t > 36000)
        {
            return Err("Invalid saved draft chain/holds".into());
        }
    }
    for r in &e.revisions {
        validate_profile(&r.profile)?;
        if r.frames.is_empty()
            || r.frames.len() > 128
            || r.frames
                .iter()
                .any(|f| f.hold_ticks == 0 || f.hold_ticks > 36000)
        {
            return Err("Invalid frame tick holds".into());
        }
    }
    Ok(())
}
fn save(root: &Path, e: &mut Effect) -> Result<()> {
    validate(e, &e.project_id, &e.id)?;
    e.generation += 1;
    save_metadata(
        root,
        &effect_dir(root, &e.project_id, &e.id)?,
        &serde_json::to_vec_pretty(e).map_err(|e| e.to_string())?,
    )
}
fn load(root: &Path, p: &str, id: &str) -> Result<(Effect, Option<String>)> {
    let dir = effect_dir(root, p, id)?;
    if !dir.exists() {
        return Err("VFX effect not found".into());
    }
    let candidates = metadata_candidates(root, &dir)?;
    let mut recovered: Option<(Effect, String)> = None;
    for (name, bytes) in candidates {
        if let Ok(v) = serde_json::from_slice::<Value>(&bytes) {
            if v["schemaVersion"].as_u64().unwrap_or(1) > 1 {
                return Err("Newer VFX schema: no downgrade/migration performed".into());
            }
        }
        if let Ok(e) = serde_json::from_slice::<Effect>(&bytes) {
            if validate(&e, p, id).is_ok() {
                if name == "index.json" {
                    return Ok((e, None));
                }
                if recovered
                    .as_ref()
                    .is_none_or(|(prior, _)| e.generation > prior.generation)
                {
                    recovered = Some((e, name));
                }
            }
        }
    }
    if let Some((e, name)) = recovered {
        return Ok((e,Some(format!("Recovered latest complete generation from {name}; damaged/interrupted metadata retained. Reapprove before export."))));
    }
    Err("VFX metadata damaged. History retained; do not reset the workspace".into())
}
fn relative(root: &Path, p: &Path) -> Result<String> {
    Ok(p.strip_prefix(root)
        .map_err(|_| "Output outside workspace")?
        .to_string_lossy()
        .replace('\\', "/"))
}
fn preview(im: &RgbaImage) -> Result<Value> {
    Ok(
        json!({"dataUrl":format!("data:image/png;base64,{}",STANDARD.encode(png(im)?)),"width":im.width(),"height":im.height(),"mimeType":"image/png"}),
    )
}
fn source<'a>(e: &'a Effect, id: &str) -> Result<&'a Source> {
    e.sources
        .iter()
        .find(|s| s.id == id)
        .ok_or("Source not found".into())
}
fn region_image(root: &Path, e: &Effect, r: &Region) -> Result<RgbaImage> {
    let s = source(e, &r.source_id)?;
    let im = decode(&checked_bytes(root, &s.path, &s.sha256)?)?;
    let [x, y, w, h] = r.rect;
    if w == 0
        || h == 0
        || x.checked_add(w).is_none_or(|v| v > im.width())
        || y.checked_add(h).is_none_or(|v| v > im.height())
    {
        return Err("Frame rectangle exceeds original PNG".into());
    }
    Ok(imageops::crop_imm(&im, x, y, w, h).to_image())
}
fn revision_hash(r: &Revision) -> Result<String> {
    let mut copy = r.clone();
    copy.approval = None;
    Ok(hash(&serde_json::to_vec(&copy).map_err(|e| e.to_string())?))
}
fn verify_revision(root: &Path, e: &Effect, r: &Revision) -> Result<Vec<RgbaImage>> {
    let mut images = vec![];
    for (i, f) in r.frames.iter().enumerate() {
        let raw = region_image(
            root,
            e,
            &Region {
                source_id: f.source_id.clone(),
                rect: f.rect,
            },
        )?;
        let (expected, clip) = process(&raw, &f.processing, &r.profile)?;
        let im = decode(&checked_bytes(root, &f.path, &f.sha256)?)?;
        let report = inspect(&im, &r.profile, i + 1 == r.frames.len(), clip);
        if im != expected || report != f.report || !report.errors.is_empty() {
            return Err(format!(
                "Frame {} validation failed: {:?}",
                i + 1,
                report.errors
            ));
        }
        images.push(im);
    }
    if r.profile.completion == "empty_terminal"
        && r.frames.last().unwrap().report.opaque_pixels != 0
    {
        return Err("Empty-terminal policy needs an actually transparent last frame".into());
    }
    Ok(images)
}
fn checked_approval(root: &Path, e: &Effect, r: &Revision) -> Result<Vec<RgbaImage>> {
    let a = r
        .approval
        .as_ref()
        .ok_or("Owner approval required for artwork, anchor, palette, holds and clock")?;
    if a.revision_hash != revision_hash(r)? {
        return Err("Approval hash changed".into());
    }
    reviewer(&a.reviewer)?;
    if !(a.checks.artwork
        && a.checks.regions_and_anchor
        && a.checks.palette
        && a.checks.timing
        && a.checks.hit_stop_and_completion)
    {
        return Err("Incomplete saved approval".into());
    }
    verify_revision(root, e, r)
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Library,
    CreateProject {
        name: String,
    },
    CreateEffect {
        effect_id: String,
        name: String,
        package: String,
        profile: Option<Profile>,
    },
    Load,
    SaveDraft {
        draft: Draft,
    },
    ImportFrames {
        paths: Vec<String>,
        board_count: Option<u32>,
        rectangles: Option<Vec<[u32; 4]>>,
    },
    SetRegions {
        regions: Vec<Region>,
    },
    ApproveImport {
        import_id: String,
        reviewer: String,
    },
    Process {
        profile: Profile,
        settings: Vec<Processing>,
        holds: Vec<u32>,
    },
    Approve {
        revision_id: String,
        reviewer: String,
        checks: Checks,
    },
    Restore {
        revision_id: String,
    },
    Export {
        revision_id: String,
        gutter: u32,
    },
    VerifyExport {
        export_id: String,
    },
    OpenExport {
        export_id: String,
    },
}
fn library(root: &Path) -> Result<Value> {
    let mut projects = vec![];
    let mut failures = vec![];
    for entry in fs::read_dir(root.join("vfx-projects"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let dir = entry.path();
        let Some(project) = dir.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if valid_id(project).is_err() {
            continue;
        }
        let resolved = fs::canonicalize(&dir).map_err(|e| e.to_string())?;
        if !resolved.starts_with(fs::canonicalize(root).map_err(|e| e.to_string())?) {
            failures.push("Skipped linked VFX project".to_string());
            continue;
        }
        let Some(owner) = fs::read(dir.join("owner.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        else {
            failures.push(format!("Damaged VFX project {project}"));
            continue;
        };
        let mut effects = vec![];
        for entry in fs::read_dir(dir.join("effects"))
            .into_iter()
            .flatten()
            .flatten()
        {
            let id = entry.file_name().to_string_lossy().into_owned();
            match load(root, project, &id) {
                Ok((e, notice)) => {
                    let r = e
                        .revisions
                        .iter()
                        .find(|r| Some(&r.id) == e.active_revision_id.as_ref());
                    effects.push(json!({"id":e.id,"effectId":e.effect_id,"name":e.name,"package":e.package,"status":if r.is_some_and(|r|r.approval.is_some()&&e.draft.as_ref().is_none_or(|d|draft_matches(d,r))){"approved"}else{"draft"},"recoveryNotice":notice}));
                }
                Err(err) => failures.push(format!("{id}: {err}")),
            }
        }
        effects.sort_by_key(|e| e["name"].as_str().unwrap_or("").to_lowercase());
        projects.push(json!({"id":project,"name":owner["name"],"effects":effects}));
    }
    projects.sort_by_key(|e| e["name"].as_str().unwrap_or("").to_lowercase());
    Ok(json!({"projects":projects,"failures":failures}))
}
fn reviewer(s: &str) -> Result<()> {
    if s.trim().is_empty() || s.len() > 160 {
        return Err("Enter the owner/reviewer name".into());
    }
    Ok(())
}
fn reply(root: &Path, e: &Effect, notice: Option<String>, result: Value) -> Result<Value> {
    let pending = e
        .imports
        .iter()
        .find(|r| Some(&r.id) == e.pending_import_id.as_ref());
    let active = e
        .imports
        .iter()
        .find(|r| Some(&r.id) == e.active_import_id.as_ref());
    let import = pending.or(active);
    let mut crops = vec![];
    let mut originals = vec![];
    if let Some(import) = import {
        for region in &import.regions {
            crops.push(preview(&region_image(root, e, region)?)?);
        }
        for source_id in import
            .regions
            .iter()
            .map(|r| &r.source_id)
            .collect::<BTreeSet<_>>()
        {
            let s = source(e, source_id)?;
            originals.push(json!({"sourceId":source_id,"image":preview(&decode(&checked_bytes(root,&s.path,&s.sha256)?)?)?}));
        }
    }
    let mut frames = vec![];
    if let Some(r) = e
        .revisions
        .iter()
        .find(|r| Some(&r.id) == e.active_revision_id.as_ref())
    {
        for f in &r.frames {
            frames.push(preview(&decode(&checked_bytes(
                root, &f.path, &f.sha256,
            )?)?)?);
        }
    }
    Ok(
        json!({"effect":e,"recoveryNotice":notice,"crops":crops,"originals":originals,"frames":frames,"result":result}),
    )
}
fn export(root: &Path, e: &mut Effect, r: &Revision, gutter: u32) -> Result<Value> {
    if gutter > 64 {
        return Err("Gutter must be 0–64".into());
    }
    let images = checked_approval(root, e, r)?;
    let width = r.profile.cell[0] * images.len() as u32 + gutter * (images.len() as u32 - 1);
    if width > 4096 {
        return Err("Sheet exceeds 4096 pixels; reduce cells/count".into());
    }
    let mut sheet = RgbaImage::new(width, r.profile.cell[1]);
    let mut frame_records = vec![];
    let export_id = id();
    let dir = effect_dir(root, &e.project_id, &e.id)?
        .join("exports")
        .join(&export_id);
    secure_parent(root, &dir.join("spritesheet.png"))?;
    for (i, im) in images.iter().enumerate() {
        let x = i as u32 * (r.profile.cell[0] + gutter);
        imageops::replace(&mut sheet, im, x as i64, 0);
        let f = &r.frames[i];
        write_bound(
            root,
            &dir.join(format!("review-frames/{i:03}.png")),
            &png(im)?,
        )?;
        frame_records.push(json!({"id":f.id,"sourceId":f.source_id,"sourceRect":f.rect,"rect":[x,0,r.profile.cell[0],r.profile.cell[1]],"offset":[0,0],"processingOffset":f.processing.offset,"placementBakedIntoCell":true,"hold_ticks":f.hold_ticks,"sha256":f.sha256,"validation":f.report,"processing":f.processing}));
    }
    let bytes = png(&sheet)?;
    let sheet_hash = hash(&bytes);
    write_bound(root, &dir.join("spritesheet.png"), &bytes)?;
    let mut raw = vec![];
    for source_id in r
        .frames
        .iter()
        .map(|f| &f.source_id)
        .collect::<BTreeSet<_>>()
    {
        let s = source(e, source_id)?;
        let name = format!("raw/{}.png", s.id);
        write_bound(
            root,
            &dir.join(&name),
            &checked_bytes(root, &s.path, &s.sha256)?,
        )?;
        raw.push(json!({"id":s.id,"file":name,"sha256":s.sha256,"originalName":s.original_name,"size":s.size,"importedAt":s.imported_at}));
    }
    let raw_manifest = json!({"schema":"sprite-studio.vfx.sources.v1","originalBytesPreserved":true,"sources":raw});
    let source_bytes = serde_json::to_vec_pretty(&raw_manifest).map_err(|e| e.to_string())?;
    write_bound(root, &dir.join("source.json"), &source_bytes)?;
    let manifest = json!({"schema":"sprite-studio.vfx.v1","effectId":e.effect_id,"displayName":e.name,"projectId":e.project_id,"package":e.package,"revision":r.id,"processorVersion":PROCESSOR,"revisionHash":revision_hash(r)?,"sourceHashes":raw,"sourceManifestSha256":hash(&source_bytes),"pngSha256":sheet_hash,"sheetSize":[width,r.profile.cell[1]],"cellSize":r.profile.cell,"contactAnchor":r.profile.anchor,"frameOrder":r.frames.iter().map(|f|&f.id).collect::<Vec<_>>(),"frames":frame_records,"gutter":gutter,"totalDurationTicks":r.frames.iter().map(|f|f.hold_ticks as u64).sum::<u64>(),"simulationHz":r.profile.tick_hz,"timingAuthority":"explicit_hold_ticks","loop":r.profile.looping,"hitStopPolicy":r.profile.hit_stop_policy,"completion":r.profile.completion,"sourceOrientation":r.profile.source_orientation,"mirroringPolicy":r.profile.mirror_policy,"palette":r.profile.palette,"paletteAuthorityStatus":if r.profile.palette.authority_id.is_some(){"USER_SUPPLIED"}else{"UNASSIGNED"},"alpha":"binary; transparent RGB zero","approval":r.approval,"status":"OWNER_APPROVED_ASSET","engineAdapter":"NOT_IMPLEMENTED","reviewFrames":"review_only; PNG frames; no guides"});
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    write_bound(root, &dir.join("manifest.json"), &manifest_bytes)?;
    e.exports.push(Export {
        id: export_id,
        path: relative(root, &dir)?,
        png_hash: sheet_hash.clone(),
        manifest_hash: hash(&manifest_bytes),
        source_manifest_hash: hash(&source_bytes),
        revision_id: r.id.clone(),
        exported_at: now(),
    });
    Ok(json!({"directory":dir,"pngSha256":sheet_hash,"manifestSha256":hash(&manifest_bytes)}))
}
fn verify_export(root: &Path, e: &Effect, export_id: &str) -> Result<Value> {
    let x = e
        .exports
        .iter()
        .find(|x| x.id == export_id)
        .ok_or("Export not found")?;
    let sheet = decode(&checked_bytes(
        root,
        &format!("{}/spritesheet.png", x.path),
        &x.png_hash,
    )?)?;
    let manifest: Value = serde_json::from_slice(&checked_bytes(
        root,
        &format!("{}/manifest.json", x.path),
        &x.manifest_hash,
    )?)
    .map_err(|e| e.to_string())?;
    let raw: Value = serde_json::from_slice(&checked_bytes(
        root,
        &format!("{}/source.json", x.path),
        &x.source_manifest_hash,
    )?)
    .map_err(|e| e.to_string())?;
    let r = e
        .revisions
        .iter()
        .find(|r| r.id == x.revision_id)
        .ok_or("Export revision missing")?;
    let frames = checked_approval(root, e, r)?;
    if manifest["revisionHash"] != revision_hash(r)?
        || manifest["pngSha256"] != x.png_hash
        || manifest["sourceManifestSha256"] != x.source_manifest_hash
        || manifest["frames"]
            .as_array()
            .is_none_or(|f| f.len() != frames.len())
    {
        return Err("Export contract mismatch".into());
    }
    for (i, im) in frames.iter().enumerate() {
        let rect = &manifest["frames"][i]["rect"];
        let sx = rect[0].as_u64().ok_or("Invalid frame rectangle")? as u32;
        if sx.checked_add(im.width()).is_none_or(|w| w > sheet.width())
            || im.height() != sheet.height()
            || imageops::crop_imm(&sheet, sx, 0, im.width(), im.height()).to_image() != *im
        {
            return Err("Export pixels differ from approved frame".into());
        }
    }
    for s in raw["sources"].as_array().ok_or("Missing raw provenance")? {
        checked_bytes(
            root,
            &format!(
                "{}/{}",
                x.path,
                s["file"].as_str().ok_or("Invalid raw path")?
            ),
            s["sha256"].as_str().ok_or("Missing raw hash")?,
        )?;
    }
    Ok(json!({"valid":true,"pngSha256":x.png_hash,"directory":root.join(&x.path)}))
}

pub(crate) fn dispatch(
    root: &Path,
    project: Option<&str>,
    effect: Option<&str>,
    request: Request,
) -> Result<Value> {
    match request {
        Request::Library => return library(root),
        Request::CreateProject { name } => {
            reviewer(&name)?;
            let project = id();
            let dir = root.join("vfx-projects").join(&project);
            write_bound(
                root,
                &dir.join("owner.json"),
                &serde_json::to_vec_pretty(
                    &json!({"schemaVersion":1,"id":project,"name":name.trim()}),
                )
                .map_err(|e| e.to_string())?,
            )?;
            return Ok(json!({"library":library(root)?,"createdProjectId":project}));
        }
        _ => {}
    }
    let project = project.ok_or("Select a VFX project")?;
    valid_id(project)?;
    let owner = root.join("vfx-projects").join(project).join("owner.json");
    let owner_path = fs::canonicalize(owner).map_err(|_| "VFX project not found")?;
    if !owner_path.starts_with(fs::canonicalize(root).map_err(|e| e.to_string())?) {
        return Err("Project escapes workspace".into());
    }
    if let Request::CreateEffect {
        effect_id,
        name,
        package,
        profile,
    } = request
    {
        reviewer(&name)?;
        if effect_id.trim().is_empty() || effect_id.len() > 160 || package.len() > 160 {
            return Err("Effect ID/package must be bounded text".into());
        }
        let mut e = Effect {
            schema_version: 1,
            id: id(),
            project_id: project.into(),
            effect_id,
            name,
            package,
            draft_profile: profile.unwrap_or_else(Profile::custom),
            sources: vec![],
            imports: vec![],
            pending_import_id: None,
            active_import_id: None,
            revisions: vec![],
            active_revision_id: None,
            exports: vec![],
            generation: 0,
            draft: None,
        };
        save(root, &mut e)?;
        return reply(root, &e, None, Value::Null);
    }
    let effect = effect.ok_or("Select an effect")?;
    let (mut e, notice) = load(root, project, effect)?;
    let mut result = Value::Null;
    let mut changed = true;
    match request {
        Request::Load => changed = false,
        Request::SaveDraft { draft } => {
            validate_profile(&draft.profile)?;
            if draft.base_revision_id != e.active_revision_id
                || draft.import_id != e.active_import_id
            {
                return Err(
                    "Draft is based on an older import/revision; reopen before editing".into(),
                );
            }
            let count = e
                .imports
                .iter()
                .find(|i| Some(&i.id) == e.active_import_id.as_ref())
                .map_or(0, |i| i.regions.len());
            if draft.holds.len() != count
                || draft.settings.len() != count
                || draft.holds.iter().any(|t| *t == 0 || *t > 36000)
            {
                return Err("Draft holds/settings must match approved frame order".into());
            }
            for p in &draft.settings {
                if p.pixel_pitch == 0
                    || p.pixel_pitch > 64
                    || p.offset.iter().any(|n| n.unsigned_abs() > 4096)
                    || p.alpha_threshold == Some(0)
                {
                    return Err("Invalid draft pixel settings".into());
                }
                if let Some(c) = &p.chroma {
                    color(c)?;
                }
            }
            changed = e.draft.as_ref() != Some(&draft);
            e.draft_profile = draft.profile.clone();
            e.draft = Some(draft);
        }
        Request::ImportFrames {
            paths,
            board_count,
            rectangles,
        } => {
            if paths.is_empty()
                || paths.len() > 128
                || (paths.len() != 1 && (board_count.is_some() || rectangles.is_some()))
            {
                return Err("Import 1–128 ordered PNGs; board rectangles use one source".into());
            }
            let mut regions = vec![];
            let mut total = 0u64;
            for file in paths {
                let size = fs::metadata(&file).map_err(|e| e.to_string())?.len();
                total += size;
                if size > 32 * 1024 * 1024 || total > 256 * 1024 * 1024 {
                    return Err("Import exceeds PNG/batch byte limits".into());
                }
                let bytes = fs::read(&file).map_err(|e| e.to_string())?;
                let im = decode(&bytes)?;
                let source_id = id();
                let dest = effect_dir(root, project, effect)?
                    .join("raw")
                    .join(format!("{source_id}.png"));
                write_bound(root, &dest, &bytes)?;
                e.sources.push(Source {
                    id: source_id.clone(),
                    path: relative(root, &dest)?,
                    sha256: hash(&bytes),
                    original_name: Path::new(&file)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into(),
                    size: [im.width(), im.height()],
                    imported_at: now(),
                });
                let rects = if let Some(ref rects) = rectangles {
                    rects.clone()
                } else if let Some(count) = board_count {
                    if count == 0 || count > 128 || im.width() % count != 0 {
                        return Err("Board count must divide its full width; otherwise enter explicit rectangles".into());
                    }
                    (0..count)
                        .map(|i| [i * (im.width() / count), 0, im.width() / count, im.height()])
                        .collect()
                } else {
                    vec![[0, 0, im.width(), im.height()]]
                };
                for rect in rects {
                    let r = Region {
                        source_id: source_id.clone(),
                        rect,
                    };
                    region_image(root, &e, &r)?;
                    regions.push(r);
                }
            }
            if regions.is_empty() || regions.len() > 128 {
                return Err("Choose 1–128 frame regions".into());
            }
            let review_id = id();
            e.imports.push(Import {
                id: review_id.clone(),
                regions,
                approved_by: None,
                approved_at: None,
                created_at: now(),
            });
            e.pending_import_id = Some(review_id);
        }
        Request::SetRegions { regions } => {
            if regions.is_empty() || regions.len() > 128 {
                return Err("Choose 1–128 rectangles".into());
            }
            for r in &regions {
                region_image(root, &e, r)?;
            }
            let review_id = id();
            e.imports.push(Import {
                id: review_id.clone(),
                regions,
                approved_by: None,
                approved_at: None,
                created_at: now(),
            });
            e.pending_import_id = Some(review_id);
        }
        Request::ApproveImport {
            import_id,
            reviewer: owner,
        } => {
            reviewer(&owner)?;
            if e.pending_import_id.as_ref() != Some(&import_id) {
                return Err("Approve the exact pending segmentation review".into());
            }
            let imp = e.imports.iter().find(|i| i.id == import_id).unwrap();
            for r in &imp.regions {
                region_image(root, &e, r)?;
            }
            let imp = e.imports.iter_mut().find(|i| i.id == import_id).unwrap();
            imp.approved_by = Some(owner);
            imp.approved_at = Some(now());
            e.active_import_id = Some(import_id);
            e.pending_import_id = None;
            e.active_revision_id = None;
            e.draft = None;
        }
        Request::Process {
            profile,
            settings,
            holds,
        } => {
            validate_profile(&profile)?;
            if e.pending_import_id.is_some() {
                return Err("Approve detected/manual frame regions first".into());
            }
            let imp = e
                .imports
                .iter()
                .find(|i| Some(&i.id) == e.active_import_id.as_ref())
                .ok_or("Approve imported frame regions first")?
                .clone();
            if holds.len() != imp.regions.len()
                || settings.len() != holds.len()
                || holds.iter().any(|t| *t == 0 || *t > 36000)
            {
                return Err(
                    "Each ordered frame needs a positive integer hold_ticks (1–36000) and settings"
                        .into(),
                );
            }
            let revision_id = id();
            let mut frames = vec![];
            for (i, r) in imp.regions.iter().enumerate() {
                let raw = region_image(root, &e, r)?;
                let (im, clipped) = process(&raw, &settings[i], &profile)?;
                let bytes = png(&im)?;
                let dest = effect_dir(root, project, effect)?
                    .join("revisions")
                    .join(&revision_id)
                    .join(format!("{i:03}.png"));
                write_bound(root, &dest, &bytes)?;
                frames.push(Frame {
                    id: id(),
                    source_id: r.source_id.clone(),
                    rect: r.rect,
                    path: relative(root, &dest)?,
                    sha256: hash(&bytes),
                    processing: settings[i].clone(),
                    hold_ticks: holds[i],
                    report: inspect(&im, &profile, i + 1 == holds.len(), clipped),
                });
            }
            e.draft_profile = profile.clone();
            e.revisions.push(Revision {
                id: revision_id.clone(),
                import_id: imp.id,
                profile,
                frames,
                approval: None,
                created_at: now(),
            });
            e.active_revision_id = Some(revision_id);
            e.draft = None;
        }
        Request::Approve {
            revision_id,
            reviewer: owner,
            checks,
        } => {
            reviewer(&owner)?;
            if e.draft.as_ref().is_some_and(|d| {
                e.revisions
                    .iter()
                    .find(|r| Some(&r.id) == e.active_revision_id.as_ref())
                    .is_none_or(|r| !draft_matches(d, r))
            }) {
                return Err("Process and review saved draft edits before approval".into());
            }
            if !(checks.artwork
                && checks.regions_and_anchor
                && checks.palette
                && checks.timing
                && checks.hit_stop_and_completion)
            {
                return Err("Explicit owner approval of artwork, regions/anchor, palette, holds and clock is required".into());
            }
            if e.active_revision_id.as_ref() != Some(&revision_id) || e.pending_import_id.is_some()
            {
                return Err("Approve the exact current revision after segmentation".into());
            }
            let r = e.revisions.iter().find(|r| r.id == revision_id).unwrap();
            verify_revision(root, &e, r)?;
            let a = Approval {
                reviewer: owner,
                approved_at: now(),
                revision_hash: revision_hash(r)?,
                checks,
            };
            e.revisions
                .iter_mut()
                .find(|r| r.id == revision_id)
                .unwrap()
                .approval = Some(a);
        }
        Request::Restore { revision_id } => {
            let r = e
                .revisions
                .iter()
                .find(|r| r.id == revision_id)
                .ok_or("Revision not found")?;
            verify_revision(root, &e, r)?;
            e.active_import_id = Some(r.import_id.clone());
            e.pending_import_id = None;
            e.draft_profile = r.profile.clone();
            e.active_revision_id = Some(revision_id);
            e.draft = None;
        }
        Request::Export {
            revision_id,
            gutter,
        } => {
            if notice.is_some() {
                return Err(
                    "Recovered metadata requires exact owner reapproval before export".into(),
                );
            }
            if e.draft.as_ref().is_some_and(|d| {
                e.revisions
                    .iter()
                    .find(|r| Some(&r.id) == e.active_revision_id.as_ref())
                    .is_none_or(|r| !draft_matches(d, r))
            }) {
                return Err("Saved draft edits require a new processed/approved revision".into());
            }
            if e.active_revision_id.as_ref() != Some(&revision_id) || e.pending_import_id.is_some()
            {
                return Err("Export the exact active approved revision".into());
            }
            let r = e
                .revisions
                .iter()
                .find(|r| r.id == revision_id)
                .unwrap()
                .clone();
            result = export(root, &mut e, &r, gutter)?;
        }
        Request::VerifyExport { export_id } => {
            result = verify_export(root, &e, &export_id)?;
            changed = false;
        }
        Request::OpenExport { export_id } => {
            result = verify_export(root, &e, &export_id)?;
            let directory = result["directory"]
                .as_str()
                .ok_or("Missing export directory")?;
            crate::studio::open_folder(Path::new(directory))?;
            changed = false;
        }
        _ => return Err("Invalid VFX operation".into()),
    }
    if changed {
        save(root, &mut e)?;
    }
    reply(root, &e, if changed { None } else { notice }, result)
}
#[tauri::command]
pub async fn vfx_command(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::studio::StudioLock>,
    project_id: Option<String>,
    effect_id: Option<String>,
    request: Request,
) -> Result<Value> {
    let lock = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock
            .0
            .lock()
            .map_err(|_| "Workspace operation lock unavailable")?;
        dispatch(
            &crate::studio::environment_workspace_root(&app)?,
            project_id.as_deref(),
            effect_id.as_deref(),
            request,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
#[cfg(test)]
#[path = "vfx_tests.rs"]
mod tests;
