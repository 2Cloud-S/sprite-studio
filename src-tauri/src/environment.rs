//! Full-canvas static layers. No calls into character cleanup, stage or export code.
use crate::asset_io::{hash, color, rgb, distance, png, decode, secure_parent, write_bound};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{imageops, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

type Result<T> = std::result::Result<T, String>;
const W: u32 = 480;
const H: u32 = 270;
const BIOME: &str = "biome.sunscorched.v1";
const PALETTE: [&str; 10] = [
    "#F3B56B", "#D9704C", "#A44950", "#5A303B", "#814032", "#A95438", "#D37A45", "#966644",
    "#C68B52", "#47766A",
];
const IDS: [&str; 6] = [
    "sunscorched_sky_void",
    "sunscorched_far_silhouette",
    "sunscorched_midground_landmark",
    "sunscorched_fight_plane_back",
    "sunscorched_fight_plane_surface",
    "sunscorched_restrained_foreground",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Parameters {
    pub resize_to_canvas: bool,
    pub pixel_pitch: u32,
    pub remove_background: Option<String>,
    pub background_tolerance: u8,
    pub binary_alpha: bool,
    pub remap_palette: bool,
    #[serde(default)]
    pub canvas_mode: CanvasMode,
    #[serde(default)]
    pub offset: [i32; 2],
}
impl Default for Parameters {
    fn default() -> Self {
        Self {
            resize_to_canvas: false,
            pixel_pitch: 1,
            remove_background: None,
            background_tolerance: 0,
            binary_alpha: false,
            remap_palette: false,
            canvas_mode: CanvasMode::Preserve,
            offset: [0, 0],
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CanvasMode {
    #[default]
    Preserve,
    Stretch,
    Fit,
    Fill,
    Pad,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AssetType {
    #[default]
    BackgroundLayer,
    Prop,
    Tile,
    Overlay,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaletteProfile {
    pub id: String,
    pub name: String,
    pub colors: Vec<String>,
    pub slots: Vec<String>,
    pub enforce: bool,
}
impl Default for PaletteProfile {
    fn default() -> Self {
        Self {
            id: BIOME.into(),
            name: "Sunscorched Outback · approved ten colors".into(),
            colors: PALETTE.iter().map(|s| s.to_string()).collect(),
            slots: vec![],
            enforce: true,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rules {
    pub alpha: String,
    pub ground_y: Option<u32>,
    pub seamless_required: bool,
}
impl Default for Rules {
    fn default() -> Self {
        Self {
            alpha: "binary".into(),
            ground_y: None,
            seamless_required: false,
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Guides {
    pub ground_y: Option<u32>,
    pub hud_y: Option<u32>,
    pub markers_x: Vec<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct AssetContract {
    canvas: [u32; 2],
    palette: PaletteProfile,
    rules: Rules,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceImage {
    path: String,
    sha256: String,
    original_name: String,
    width: u32,
    height: u32,
}
fn legacy_package_id() -> String {
    BIOME.into()
}
fn legacy_display_name() -> String {
    "Sunscorched Outback".into()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    width: u32,
    height: u32,
    origin: [u32; 2],
    color_count: usize,
    approved_colors: Vec<String>,
    unexpected_colors: Vec<String>,
    unexpected_color_count: usize,
    opaque_pixels: usize,
    transparent_pixels: usize,
    partial_alpha_pixels: usize,
    palette_pass: bool,
    alpha_pass: bool,
    canvas_pass: bool,
    ground_pass: bool,
    detected_pixel_pitch: Option<u32>,
    warnings: Vec<String>,
    errors: Vec<String>,
    valid: bool,
    #[serde(default)]
    color_usage: Vec<(String, usize)>,
    #[serde(default)]
    edge_mismatches: [usize; 2],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Source {
    id: String,
    original_name: String,
    path: String,
    sha256: String,
    report: Report,
    #[serde(default)]
    imported_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Revision {
    id: String,
    source_id: String,
    source_hash: String,
    path: String,
    sha256: String,
    parameters: Parameters,
    report: Report,
    reviewer: Option<String>,
    approved_at: Option<String>,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    contract: Option<AssetContract>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Layer {
    asset_id: String,
    order: usize,
    parameters: Parameters,
    current_source_id: Option<String>,
    current_revision_id: Option<String>,
    sources: Vec<Source>,
    revisions: Vec<Revision>,
    exports: Vec<String>,
    #[serde(default)]
    exported_revision_ids: Vec<String>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    asset_type: AssetType,
    #[serde(default)]
    removed: bool,
    #[serde(default)]
    rules: Rules,
    #[serde(default)]
    native_size: Option<[u32; 2]>,
    #[serde(default)]
    pivot: Option<[i32; 2]>,
    #[serde(default)]
    position: [i32; 2],
    #[serde(default)]
    modified_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportRecord {
    path: String,
    kind: String,
    exported_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    schema_version: u32,
    processor_version: String,
    project_id: String,
    biome_id: String,
    arena_id: String,
    palette_version: String,
    canvas: [u32; 2],
    origin: [u32; 2],
    layers: Vec<Layer>,
    #[serde(default = "legacy_package_id")]
    package_id: String,
    #[serde(default = "legacy_display_name")]
    display_name: String,
    #[serde(default)]
    template_id: Option<String>,
    #[serde(default)]
    palette: PaletteProfile,
    #[serde(default)]
    guides: Guides,
    #[serde(default)]
    master_reference: Option<ReferenceImage>,
    #[serde(default)]
    package_exports: Vec<String>,
    #[serde(default)]
    composite_exports: Vec<String>,
    #[serde(default)]
    export_history: Vec<ExportRecord>,
    #[serde(default)]
    revision: u64,
    #[serde(skip)]
    recovery_notice: Option<String>,
}
impl Environment {
    fn new(id: &str) -> Self {
        Self {
            schema_version: 2,
            processor_version: "static.environment.v2".into(),
            project_id: id.into(),
            biome_id: BIOME.into(),
            arena_id: "arena.outback".into(),
            palette_version: BIOME.into(),
            canvas: [W, H],
            origin: [0, 0],
            recovery_notice: None,
            package_id: BIOME.into(),
            display_name: legacy_display_name(),
            template_id: Some(BIOME.into()),
            palette: PaletteProfile::default(),
            guides: Guides {
                ground_y: Some(224),
                hud_y: None,
                markers_x: vec![156, 324],
            },
            master_reference: None,
            package_exports: vec![],
            composite_exports: vec![],
            export_history: vec![],
            revision: 0,
            layers: IDS
                .iter()
                .enumerate()
                .map(|(order, id)| Layer {
                    asset_id: id.to_string(),
                    order,
                    parameters: Parameters::default(),
                    current_source_id: None,
                    current_revision_id: None,
                    sources: vec![],
                    revisions: vec![],
                    exports: vec![],
                    exported_revision_ids: vec![],
                    name: id.replace("sunscorched_", "").replace('_', " "),
                    asset_type: AssetType::BackgroundLayer,
                    removed: false,
                    rules: Rules {
                        alpha: if order == 0 { "opaque" } else { "binary" }.into(),
                        ground_y: if order == 4 { Some(224) } else { None },
                        seamless_required: false,
                    },
                    native_size: None,
                    pivot: None,
                    position: [0, 0],
                    modified_at: String::new(),
                })
                .collect(),
        }
    }
}
// Conservative detection: only claim a pitch when every origin-aligned block is constant.
fn pitch(image: &RgbaImage) -> Option<u32> {
    if image.pixels().all(|p| p == image.get_pixel(0, 0)) {
        return None;
    }
    (2..=16).rev().find(|n| {
        image.width() % n == 0
            && image.height() % n == 0
            && image
                .enumerate_pixels()
                .all(|(x, y, p)| p == image.get_pixel(x / n * n, y / n * n))
    })
}
#[cfg(test)]
fn inspect(image: &RgbaImage, order: usize) -> Report {
    let s = Environment::new("validation");
    inspect_asset(image, &contract(&s, &s.layers[order]))
}
fn contract(s: &Environment, l: &Layer) -> AssetContract {
    AssetContract {
        canvas: l.native_size.unwrap_or(s.canvas),
        palette: s.palette.clone(),
        rules: l.rules.clone(),
    }
}
fn inspect_asset(image: &RgbaImage, contract: &AssetContract) -> Report {
    let colors: BTreeSet<[u8; 3]> = image.pixels().filter(|p| p[3] != 0).map(rgb).collect();
    let allowed: Vec<_> = contract
        .palette
        .colors
        .iter()
        .filter_map(|s| color(s).ok())
        .collect();
    let outside: Vec<String> = colors
        .iter()
        .filter(|c| !allowed.contains(c))
        .map(|c| format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]))
        .collect();
    let partial = image.pixels().filter(|p| p[3] != 0 && p[3] != 255).count();
    let transparent = image.pixels().filter(|p| p[3] == 0).count();
    let opaque = image.pixels().filter(|p| p[3] == 255).count();
    let canvas = image.dimensions() == (contract.canvas[0], contract.canvas[1]);
    let alpha = match contract.rules.alpha.as_str() {
        "opaque" => transparent == 0 && partial == 0,
        "any" => true,
        _ => partial == 0,
    };
    let ground = contract.rules.ground_y.is_none()
        || (canvas
            && image.enumerate_pixels().all(|(_, y, p)| {
                p[3] == if y < contract.rules.ground_y.unwrap() {
                    0
                } else {
                    255
                }
            }));
    let mut errors = vec![];
    if !canvas {
        errors.push(format!(
            "Canvas must be {}×{}; transformation requires explicit consent",
            contract.canvas[0], contract.canvas[1]
        ));
    }
    if contract.palette.enforce && !outside.is_empty() {
        errors.push(format!(
            "{} colors outside {}",
            outside.len(),
            contract.palette.name
        ));
    }
    if !alpha {
        errors.push(
            if contract.rules.alpha == "opaque" {
                "This asset must be fully opaque"
            } else {
                "Alpha must be binary (0/255)"
            }
            .into(),
        );
    }
    if !ground {
        errors.push(format!(
            "Ground rule requires transparent rows above Y={} and opaque rows below",
            contract.rules.ground_y.unwrap()
        ));
    }
    let edge_mismatches = [
        (0..image.height())
            .filter(|&y| image.get_pixel(0, y) != image.get_pixel(image.width() - 1, y))
            .count(),
        (0..image.width())
            .filter(|&x| image.get_pixel(x, 0) != image.get_pixel(x, image.height() - 1))
            .count(),
    ];
    if contract.rules.seamless_required && edge_mismatches != [0, 0] {
        errors.push("Opposite tile edges differ; seamless edge rule fails".into());
    }
    let mut usage = std::collections::BTreeMap::<String, usize>::new();
    for p in image.pixels().filter(|p| p[3] != 0) {
        *usage
            .entry(format!("#{:02X}{:02X}{:02X}", p[0], p[1], p[2]))
            .or_default() += 1;
    }
    let mut warnings=vec!["Pixel-pitch detection is conservative, not an artistic quality certificate. Review at 1× and 4×; baked checkerboards and inconsistent pixel clusters require manual review.".into()];
    if partial > 0 {
        warnings.push(
            "Partial alpha may indicate antialiasing; inspect original before thresholding".into(),
        );
    }
    if outside.len() > 32 {
        warnings.push("Many colors may indicate gradients/blending. Remapping is a proposal, not an art approval.".into());
    }
    Report {
        width: image.width(),
        height: image.height(),
        origin: [0, 0],
        color_count: colors.len(),
        approved_colors: contract.palette.colors.clone(),
        unexpected_color_count: outside.len(),
        unexpected_colors: outside.into_iter().take(128).collect(),
        opaque_pixels: opaque,
        transparent_pixels: transparent,
        partial_alpha_pixels: partial,
        palette_pass: errors.iter().all(|e| !e.contains("colors outside")),
        alpha_pass: alpha,
        canvas_pass: canvas,
        ground_pass: ground,
        detected_pixel_pitch: pitch(image),
        valid: errors.is_empty(),
        warnings,
        errors,
        color_usage: usage.into_iter().take(128).collect(),
        edge_mismatches,
    }
}
#[cfg(test)]
fn process(source: &RgbaImage, p: &Parameters, order: usize) -> Result<RgbaImage> {
    let s = Environment::new("processing");
    process_asset(source, p, &contract(&s, &s.layers[order]))
}
fn process_asset(
    source: &RgbaImage,
    p: &Parameters,
    contract: &AssetContract,
) -> Result<RgbaImage> {
    if !(1..=16).contains(&p.pixel_pitch) {
        return Err("Manual pixel pitch must be 1–16".into());
    }
    if contract.rules.alpha == "opaque" && p.remove_background.is_some() {
        return Err("Background removal is disabled for the opaque sky".into());
    }
    let key = p.remove_background.as_deref().map(color).transpose()?;
    let [w, h] = contract.canvas;
    let mode = if p.resize_to_canvas {
        &CanvasMode::Stretch
    } else {
        &p.canvas_mode
    };
    let mut out = match mode {
        CanvasMode::Preserve => {
            if p.offset != [0, 0] {
                return Err("Choose an explicit canvas operation before positioning".into());
            }
            source.clone()
        }
        CanvasMode::Stretch => imageops::resize(source, w, h, imageops::FilterType::Nearest),
        CanvasMode::Fit | CanvasMode::Fill | CanvasMode::Pad => {
            let image = if *mode == CanvasMode::Pad {
                source.clone()
            } else {
                let sx = f64::from(w) / f64::from(source.width());
                let sy = f64::from(h) / f64::from(source.height());
                let ratio = if *mode == CanvasMode::Fit {
                    sx.min(sy)
                } else {
                    sx.max(sy)
                };
                let rw = (f64::from(source.width()) * ratio).round().max(1.0) as u32;
                let rh = (f64::from(source.height()) * ratio).round().max(1.0) as u32;
                if rw > 8192 || rh > 8192 {
                    return Err("Aspect fit/fill intermediate is too large".into());
                }
                imageops::resize(source, rw, rh, imageops::FilterType::Nearest)
            };
            let (x, y) = if *mode == CanvasMode::Pad {
                (i64::from(p.offset[0]), i64::from(p.offset[1]))
            } else {
                (
                    (i64::from(w) - i64::from(image.width())) / 2 + i64::from(p.offset[0]),
                    (i64::from(h) - i64::from(image.height())) / 2 + i64::from(p.offset[1]),
                )
            };
            // Padding must not silently discard source pixels. Cropping is explicit Fill only.
            if *mode != CanvasMode::Fill
                && (x < 0
                    || y < 0
                    || x + i64::from(image.width()) > i64::from(w)
                    || y + i64::from(image.height()) > i64::from(h))
            {
                return Err(
                    "Placement would crop pixels. Expand canvas or explicitly choose Fill / crop"
                        .into(),
                );
            }
            let mut canvas = RgbaImage::new(w, h);
            imageops::overlay(&mut canvas, &image, x, y);
            canvas
        }
    };
    // Grid is always anchored to (0,0). This never crops, trims or recenters content.
    if p.pixel_pitch > 1 {
        let original = out.clone();
        for (x, y, pixel) in out.enumerate_pixels_mut() {
            *pixel = *original.get_pixel(
                x / p.pixel_pitch * p.pixel_pitch,
                y / p.pixel_pitch * p.pixel_pitch,
            );
        }
    }
    let allowed: Vec<_> = contract
        .palette
        .colors
        .iter()
        .map(|s| color(s))
        .collect::<Result<_>>()?;
    if p.remap_palette && allowed.is_empty() {
        return Err("Choose a nonempty palette before remapping".into());
    }
    for pixel in out.pixels_mut() {
        if let Some(key) = key {
            if distance(rgb(pixel), key) <= u32::from(p.background_tolerance).pow(2) {
                *pixel = Rgba([0, 0, 0, 0]);
            }
        }
        if p.binary_alpha {
            pixel[3] = if pixel[3] >= 128 { 255 } else { 0 };
        }
        if p.remap_palette && pixel[3] != 0 {
            let nearest = allowed
                .iter()
                .min_by_key(|c| distance(rgb(pixel), **c))
                .unwrap();
            pixel[0] = nearest[0];
            pixel[1] = nearest[1];
            pixel[2] = nearest[2];
        }
    }
    Ok(out)
}
#[cfg(test)]
fn folder(root: &Path) -> PathBuf {
    root.join("environments").join(BIOME)
}
fn package_folder(root: &Path, id: &str) -> Result<PathBuf> {
    safe_id(id)?;
    Ok(root.join("environments").join(id))
}
fn safe_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 100
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        || id == "."
        || id == ".."
    {
        return Err("Invalid environment identity".into());
    }
    Ok(())
}
fn upgrade(mut s: Environment) -> Environment {
    if s.schema_version == 1 {
        s.schema_version = 2;
        s.template_id = Some(BIOME.into());
        s.guides = Guides {
            ground_y: Some(224),
            hud_y: None,
            markers_x: vec![156, 324],
        };
        for l in &mut s.layers {
            l.name = l.asset_id.replace("sunscorched_", "").replace('_', " ");
            l.rules = Rules {
                alpha: if l.asset_id == IDS[0] {
                    "opaque"
                } else {
                    "binary"
                }
                .into(),
                ground_y: if l.asset_id == IDS[4] {
                    Some(224)
                } else {
                    None
                },
                seamless_required: false,
            };
            let c = AssetContract {
                canvas: s.canvas,
                palette: s.palette.clone(),
                rules: l.rules.clone(),
            };
            for r in &mut l.revisions {
                r.contract = Some(c.clone());
            }
        }
    }
    s
}
fn legacy_is_valid(s: &Environment) -> bool {
    s.schema_version != 1
        || (s.package_id == BIOME
            && s.biome_id == BIOME
            && s.palette_version == BIOME
            && s.canvas == [W, H]
            && s.origin == [0, 0]
            && s.layers.len() == 6
            && s.layers
                .iter()
                .enumerate()
                .all(|(i, l)| l.order == i && l.asset_id == IDS[i]))
}
fn relative_file(root: &Path, value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
        || !value.starts_with("environments/")
    {
        return Err("Invalid static asset relative path".into());
    }
    let resolved = fs::canonicalize(root.join(path)).map_err(|e| e.to_string())?;
    let base = fs::canonicalize(root).map_err(|e| e.to_string())?;
    if !resolved.starts_with(base) {
        return Err("Static asset escapes project".into());
    }
    Ok(resolved)
}
fn checked_bytes(root: &Path, path: &str, expected: &str) -> Result<Vec<u8>> {
    let bytes = fs::read(relative_file(root, path)?).map_err(|e| e.to_string())?;
    if hash(&bytes) != expected {
        return Err(format!(
            "Hash mismatch: {path}; preserved source/revision was changed externally"
        ));
    }
    Ok(bytes)
}
fn validate_state(s: &Environment, id: &str) -> Result<()> {
    if s.schema_version != 2 || s.project_id != id || s.origin != [0, 0] || s.layers.len() > 256 {
        return Err(
            "Unsupported or inconsistent environment metadata; no migration performed".into(),
        );
    }
    safe_id(&s.package_id)?;
    validate_dimensions(s.canvas)?;
    validate_palette(&s.palette)?;
    if s.guides.ground_y.is_some_and(|y| y > s.canvas[1])
        || s.guides.hud_y.is_some_and(|y| y > s.canvas[1])
        || s.guides.markers_x.iter().any(|x| *x > s.canvas[0])
    {
        return Err("Preview guides must stay within the package canvas".into());
    }
    if s.display_name.trim().is_empty() || s.display_name.len() > 160 {
        return Err("Package name must be 1–160 characters".into());
    }
    if s.template_id.as_deref() == Some(BIOME)
        && (s.canvas != [W, H] || s.palette != PaletteProfile::default())
    {
        return Err("Outback template requires its exact 480×270 canvas and approved ten-color palette; duplicate as Custom to use another contract".into());
    }
    let mut ids = BTreeSet::new();
    for (i, l) in s.layers.iter().enumerate() {
        safe_id(&l.asset_id)?;
        if l.name.trim().is_empty() || l.name.len() > 160 {
            return Err("Asset name must be 1–160 characters".into());
        }
        if l.asset_type == AssetType::BackgroundLayer && l.native_size.is_some() {
            return Err("Full-canvas background layers use the shared package canvas".into());
        }
        if l.order != i || !ids.insert(&l.asset_id) {
            return Err("Environment layer identity/order is invalid".into());
        }
        if let Some(size) = l.native_size {
            validate_dimensions(size)?;
        }
        if !["binary", "opaque", "any"].contains(&l.rules.alpha.as_str())
            || l.rules
                .ground_y
                .is_some_and(|y| y >= l.native_size.unwrap_or(s.canvas)[1])
        {
            return Err("Invalid per-asset validation rule".into());
        }
        if s.template_id.as_deref() == Some(BIOME) && IDS.contains(&l.asset_id.as_str()) {
            let expected = if l.asset_id == IDS[0] {
                "opaque"
            } else {
                "binary"
            };
            if l.rules.alpha != expected
                || l.rules.ground_y
                    != if l.asset_id == IDS[4] {
                        Some(224)
                    } else {
                        None
                    }
                || l.native_size.is_some()
                || l.position != [0, 0]
                || l.asset_type != AssetType::BackgroundLayer
            {
                return Err("Outback built-in layer contract cannot be changed".into());
            }
        }
        if let Some(id) = &l.current_source_id {
            if !l.sources.iter().any(|s| &s.id == id) {
                return Err("Missing current source record".into());
            }
        }
        if let Some(id) = &l.current_revision_id {
            if !l.revisions.iter().any(|r| {
                &r.id == id
                    && Some(&r.source_id) == l.current_source_id.as_ref()
                    && r.parameters == l.parameters
                    && r.contract.as_ref() == Some(&contract(s, l))
            }) {
                return Err("Missing or stale current revision".into());
            }
        }
    }
    Ok(())
}
fn validate_dimensions(size: [u32; 2]) -> Result<()> {
    if size.iter().any(|&v| v == 0 || v > 4096) {
        Err("Canvas dimensions must be 1–4096 per axis".into())
    } else {
        Ok(())
    }
}
fn validate_palette(p: &PaletteProfile) -> Result<()> {
    if p.colors.len() > 256
        || (p.enforce && p.colors.is_empty())
        || p.colors.iter().any(|s| color(s).is_err())
        || (!p.slots.is_empty() && p.slots.len() != p.colors.len())
    {
        return Err(
            "Palette needs 1–256 #RRGGBB colors when enforced; slot names must match color count"
                .into(),
        );
    }
    Ok(())
}
#[cfg(test)]
fn load(root: &Path, id: &str) -> Result<Environment> {
    load_package(root, id, BIOME)
}
fn load_package(root: &Path, id: &str, package: &str) -> Result<Environment> {
    let dir = package_folder(root, package)?;
    let primary = dir.join("index.json");
    if let Ok(bytes) = fs::read(&primary) {
        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            if value["schemaVersion"].as_u64().unwrap_or(1) > 2 {
                return Err("Newer environment schema: update Sprite Studio; no downgrade or migration performed".into());
            }
        }
    }
    if !primary.exists()
        && !dir.join("index.previous.json").exists()
        && !dir.join("index.pending.json").exists()
        && !dir.join("history").exists()
    {
        if package == BIOME {
            return Ok(Environment::new(id));
        }
        return Err("Package does not exist".into());
    }
    for name in ["index.json", "index.previous.json", "index.pending.json"] {
        if let Ok(bytes) = fs::read(dir.join(name)) {
            if let Ok(s) = serde_json::from_slice::<Environment>(&bytes) {
                if s.schema_version > 2 {
                    return Err("Newer environment schema: no downgrade performed".into());
                }
                if !legacy_is_valid(&s) {
                    continue;
                }
                let mut s = upgrade(s);
                if s.package_id == package && validate_state(&s, id).is_ok() {
                    if name != "index.json" {
                        s.recovery_notice=Some(format!("Recovered from {name}; primary retained for diagnosis. Review recovered layer state before exporting."));
                    }
                    return Ok(s);
                }
            }
        }
    }
    let mut history = fs::read_dir(dir.join("history"))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .collect::<Vec<_>>();
    history.sort_by_key(|e| std::cmp::Reverse(e.metadata().and_then(|m| m.modified()).ok()));
    for entry in history {
        if let Ok(bytes) = fs::read(entry.path()) {
            if let Ok(s) = serde_json::from_slice::<Environment>(&bytes) {
                if s.schema_version > 2 {
                    continue;
                }
                if !legacy_is_valid(&s) {
                    continue;
                }
                let mut s = upgrade(s);
                if s.package_id == package && validate_state(&s, id).is_ok() {
                    s.recovery_notice=Some("Recovered a validated history snapshot after an interrupted save; old files retained".into());
                    return Ok(s);
                }
            }
        }
    }
    Err("Environment metadata damaged; history retained. Restore a validated history snapshot; do not reset workspace.".into())
}
fn save(root: &Path, s: &Environment) -> Result<()> {
    validate_state(s, &s.project_id)?;
    let dir = package_folder(root, &s.package_id)?;
    secure_parent(root, &dir.join("history/snapshot.json"))?;
    let bytes = serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?;
    let primary = dir.join("index.json");
    if let Ok(old) = fs::read(&primary) {
        if serde_json::from_slice::<serde_json::Value>(&old)
            .ok()
            .and_then(|v| v["schemaVersion"].as_u64())
            == Some(1)
        {
            write_bound(
                root,
                &dir.join(format!("index.v1-backup-{}.json", Uuid::new_v4())),
                &old,
            )?;
        }
    }
    crate::asset_io::save_metadata(root, &dir, &bytes)
}

fn layer_mut(s: &mut Environment, order: usize) -> Result<&mut Layer> {
    s.layers
        .get_mut(order)
        .filter(|l| !l.removed)
        .ok_or("Asset is unavailable or removed from this package".into())
}
fn import(root: &Path, s: &mut Environment, order: usize, path: &Path) -> Result<()> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 32 * 1024 * 1024 {
        return Err("Import a PNG of at most 32 MiB".into());
    }
    let bytes = fs::read(path).map_err(|e| format!("Cannot read PNG: {e}"))?;
    let image = decode(&bytes)?;
    let id = Uuid::new_v4().to_string();
    let c = contract(s, s.layers.get(order).ok_or("Asset not found")?);
    let relative = format!(
        "environments/{}/layers/{}/{id}/original.png",
        s.package_id,
        s.layers.get(order).ok_or("Asset not found")?.asset_id
    );
    write_bound(root, &root.join(&relative), &bytes)?;
    let l = layer_mut(s, order)?;
    l.sources.push(Source {
        id: id.clone(),
        original_name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        path: relative,
        sha256: hash(&bytes),
        report: inspect_asset(&image, &c),
        imported_at: chrono::Utc::now().to_rfc3339(),
    });
    l.current_source_id = Some(id);
    l.current_revision_id = None;
    l.modified_at = chrono::Utc::now().to_rfc3339();
    save(root, s)
}
fn set_parameters(root: &Path, s: &mut Environment, order: usize, p: Parameters) -> Result<()> {
    // Validate even before there is a source. Never invalidate another layer.
    let c = contract(s, s.layers.get(order).ok_or("Asset not found")?);
    let l = s.layers.get(order).ok_or("Asset not found")?;
    let input = if let Some(src) = l
        .sources
        .iter()
        .find(|src| Some(&src.id) == l.current_source_id.as_ref())
    {
        decode(&checked_bytes(root, &src.path, &src.sha256)?)?
    } else {
        RgbaImage::new(1, 1)
    };
    process_asset(&input, &p, &c)?;
    let l = layer_mut(s, order)?;
    if l.parameters != p {
        l.parameters = p;
        l.current_revision_id = None;
        l.modified_at = chrono::Utc::now().to_rfc3339();
    }
    save(root, s)
}
fn propose(root: &Path, s: &mut Environment, order: usize) -> Result<()> {
    let c = contract(s, s.layers.get(order).ok_or("Asset not found")?);
    let package = s.package_id.clone();
    let l = layer_mut(s, order)?;
    let source = l
        .sources
        .iter()
        .find(|src| Some(&src.id) == l.current_source_id.as_ref())
        .ok_or("Import this layer first")?
        .clone();
    let out = process_asset(
        &decode(&checked_bytes(root, &source.path, &source.sha256)?)?,
        &l.parameters,
        &c,
    )?;
    let bytes = png(&out)?;
    let id = Uuid::new_v4().to_string();
    let path = format!(
        "environments/{package}/layers/{}/revisions/{id}/processed.png",
        l.asset_id
    );
    write_bound(root, &root.join(&path), &bytes)?;
    l.revisions.push(Revision {
        id: id.clone(),
        source_id: source.id,
        source_hash: source.sha256,
        path,
        sha256: hash(&bytes),
        parameters: l.parameters.clone(),
        report: inspect_asset(&out, &c),
        created_at: chrono::Utc::now().to_rfc3339(),
        contract: Some(c),
        reviewer: None,
        approved_at: None,
    });
    l.current_revision_id = Some(id);
    l.modified_at = chrono::Utc::now().to_rfc3339();
    save(root, s)
}
fn current(l: &Layer) -> Result<&Revision> {
    l.revisions
        .iter()
        .find(|r| {
            Some(&r.id) == l.current_revision_id.as_ref()
                && Some(&r.source_id) == l.current_source_id.as_ref()
                && r.parameters == l.parameters
        })
        .ok_or(
            "Process/review current layer settings; previous revisions remain recoverable".into(),
        )
}
fn validate_current(root: &Path, l: &Layer) -> Result<Report> {
    let r = current(l)?;
    let src = l
        .sources
        .iter()
        .find(|s| s.id == r.source_id)
        .ok_or("Missing source")?;
    checked_bytes(root, &src.path, &r.source_hash)?;
    let image = decode(&checked_bytes(root, &r.path, &r.sha256)?)?;
    Ok(inspect_asset(
        &image,
        r.contract.as_ref().ok_or("Revision contract is missing")?,
    ))
}
fn approve(root: &Path, s: &mut Environment, order: usize, reviewer: &str) -> Result<()> {
    if reviewer.trim().is_empty() {
        return Err("Record a reviewer after checking native/4× composition, palette remapping and hidden background artifacts".into());
    }
    let l = layer_mut(s, order)?;
    let report = validate_current(root, l)?;
    if !report.valid {
        return Err(report.errors.join("; "));
    }
    let r = l
        .revisions
        .iter_mut()
        .find(|r| Some(&r.id) == l.current_revision_id.as_ref())
        .unwrap();
    r.reviewer = Some(reviewer.trim().into());
    r.approved_at = Some(chrono::Utc::now().to_rfc3339());
    save(root, s)
}
fn export_layer(root: &Path, s: &mut Environment, order: usize) -> Result<String> {
    let l = layer_mut(s, order)?.clone();
    let r = current(&l)?.clone();
    if r.approved_at.is_none() {
        return Err("Explicit visual approval required before export".into());
    }
    let report = validate_current(root, &l)?;
    if !report.valid {
        return Err(report.errors.join("; "));
    }
    let bytes = checked_bytes(root, &r.path, &r.sha256)?;
    let revision = l.exports.len() + 1;
    let relative = format!(
        "environments/{}/exports/{}/{revision}-{}",
        s.package_id,
        l.asset_id,
        Uuid::new_v4()
    );
    let dir = root.join(&relative);
    write_bound(root, &dir.join(format!("{}.png", l.asset_id)), &bytes)?;
    let manifest = asset_manifest(s, &l, &r, &report, revision);
    write_bound(
        root,
        &dir.join("manifest.json"),
        &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )?;
    s.export_history.push(ExportRecord {
        path: relative.clone(),
        kind: "asset".into(),
        exported_at: chrono::Utc::now().to_rfc3339(),
    });
    s.layers[order].exports.push(relative);
    s.layers[order].exported_revision_ids.push(r.id.clone());
    save(root, s)?;
    Ok(dir.to_string_lossy().into())
}
fn asset_manifest(
    s: &Environment,
    l: &Layer,
    r: &Revision,
    report: &Report,
    revision: usize,
) -> serde_json::Value {
    let mut m = serde_json::json!({"schemaVersion":2,"assetType":if l.asset_type == AssetType::BackgroundLayer { "static_environment_layer" } else { match l.asset_type { AssetType::Prop => "environment_prop", AssetType::Tile => "environment_tile", _ => "environment_overlay" } },"projectId":s.project_id,
        "packageId":s.package_id,"assetId":l.asset_id,"displayName":l.name,"biomeId":s.biome_id,"arenaId":s.arena_id,"layerOrder":l.order,"sourceId":r.source_id,
        "sourceHash":r.source_hash,"exportHash":r.sha256,"canvas":{"width":report.width,"height":report.height},"origin":[0,0],
        "processorVersion":s.processor_version,"parameters":r.parameters,"paletteVersion":s.palette.id,"paletteProfile":s.palette,"validation":report,
        "exportRevision":revision,"processingRevision":r.id,"status":"approved","reviewer":r.reviewer,"approvedAt":r.approved_at,"placement":l.position,"dependencies":[r.source_id]});
    if let Some(pivot) = l.pivot {
        m["pivot"] = serde_json::json!(pivot);
    }
    m
}
fn export_package(root: &Path, s: &mut Environment, orders: &[usize]) -> Result<String> {
    if orders.is_empty() || orders.iter().collect::<BTreeSet<_>>().len() != orders.len() {
        return Err("Choose at least one unique approved asset".into());
    }
    // Preflight everything before writing, to avoid partial packages that look complete.
    let mut records = vec![];
    for order in orders {
        let l = s
            .layers
            .get(*order)
            .filter(|l| !l.removed)
            .ok_or("Asset unavailable")?;
        let r = current(l)?;
        if r.contract.as_ref() != Some(&contract(s, l)) || r.approved_at.is_none() {
            return Err(format!("{} needs current visual approval", l.name));
        }
        let report = validate_current(root, l)?;
        if !report.valid {
            return Err(format!("{}: {}", l.name, report.errors.join("; ")));
        }
        records.push((
            l.clone(),
            r.clone(),
            report,
            checked_bytes(root, &r.path, &r.sha256)?,
        ));
    }
    records.sort_by_key(|(l, _, _, _)| l.order);
    let relative = format!(
        "environments/{}/package-exports/{}",
        s.package_id,
        Uuid::new_v4()
    );
    let dir = root.join(&relative);
    let mut assets = vec![];
    for (l, r, report, bytes) in records {
        let image_path = format!("assets/{}.png", l.asset_id);
        let manifest_path = format!("assets/{}.manifest.json", l.asset_id);
        write_bound(root, &dir.join(&image_path), &bytes)?;
        write_bound(
            root,
            &dir.join(&manifest_path),
            &serde_json::to_vec_pretty(&asset_manifest(s, &l, &r, &report, 1))
                .map_err(|e| e.to_string())?,
        )?;
        assets.push(serde_json::json!({"assetId":l.asset_id,"path":image_path,"manifest":manifest_path,"layerOrder":l.order,"revision":r.id,"sha256":r.sha256,"validation":report}));
        s.layers[l.order].exported_revision_ids.push(r.id.clone());
    }
    write_bound(root,&dir.join("package.manifest.json"),&serde_json::to_vec_pretty(&serde_json::json!({"schemaVersion":2,"packageId":s.package_id,"projectId":s.project_id,"displayName":s.display_name,"canvas":s.canvas,"paletteProfile":s.palette,"origin":[0,0],"status":"approved","assets":assets,"exportedAt":chrono::Utc::now().to_rfc3339(),"guidesIncluded":false})).map_err(|e|e.to_string())?)?;
    s.export_history.push(ExportRecord {
        path: relative.clone(),
        kind: "package".into(),
        exported_at: chrono::Utc::now().to_rfc3339(),
    });
    s.package_exports.push(relative);
    save(root, s)?;
    Ok(dir.to_string_lossy().into_owned())
}
fn composite(root: &Path, s: &Environment, visible: &[usize]) -> Result<RgbaImage> {
    if visible.iter().any(|i| *i >= s.layers.len())
        || visible.iter().collect::<BTreeSet<_>>().len() != visible.len()
    {
        return Err("Visible orders must identify unique assets".into());
    }
    let mut out = RgbaImage::new(s.canvas[0], s.canvas[1]);
    for l in &s.layers {
        if l.removed || !visible.contains(&l.order) {
            continue;
        }
        let r = current(l)?;
        let image = decode(&checked_bytes(root, &r.path, &r.sha256)?)?;
        if l.asset_type == AssetType::BackgroundLayer
            && image.dimensions() != (s.canvas[0], s.canvas[1])
        {
            return Err(format!(
                "{} cannot composite: dimensions differ",
                l.asset_id
            ));
        }
        imageops::overlay(
            &mut out,
            &image,
            i64::from(l.position[0]),
            i64::from(l.position[1]),
        );
    }
    Ok(out)
}
fn preview(image: &RgbaImage) -> Result<serde_json::Value> {
    Ok(
        serde_json::json!({"dataUrl":format!("data:image/png;base64,{}",STANDARD.encode(png(image)?)),"width":image.width(),"height":image.height(),"mimeType":"image/png"}),
    )
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Load,
    PackageHistory,
    Create {
        name: String,
        template: bool,
        canvas: [u32; 2],
        palette: PaletteProfile,
        layer_count: usize,
    },
    Configure {
        name: String,
        biome: String,
        arena: String,
        canvas: [u32; 2],
        palette: PaletteProfile,
        guides: Guides,
    },
    Duplicate {
        name: String,
    },
    AddAsset {
        name: String,
        asset_type: AssetType,
        native_size: Option<[u32; 2]>,
        rules: Rules,
    },
    AssetSettings {
        order: usize,
        name: String,
        rules: Rules,
        native_size: Option<[u32; 2]>,
        pivot: Option<[i32; 2]>,
        position: [i32; 2],
    },
    RemoveAsset {
        order: usize,
    },
    RestoreAsset {
        order: usize,
    },
    Reorder {
        asset_ids: Vec<String>,
    },
    ExportPackage {
        orders: Vec<usize>,
    },
    AttachReference {
        path: String,
    },
    ReferencePreview,
    RevisionPreview {
        order: usize,
        revision: String,
    },
    OpenExport {
        path: String,
    },
    ReviewComposite {
        layers: Vec<CompositeLayer>,
    },
    Import {
        order: usize,
        path: String,
    },
    Settings {
        order: usize,
        parameters: Parameters,
    },
    Process {
        order: usize,
    },
    Approve {
        order: usize,
        reviewer: String,
    },
    Export {
        order: usize,
    },
    Preview {
        order: usize,
        original: bool,
    },
    Composite {
        visible: Vec<usize>,
        export: bool,
    },
    Restore {
        order: usize,
        revision: String,
    },
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompositeLayer {
    order: usize,
    revision: String,
    opacity: f32,
}
#[cfg(test)]
fn dispatch(root: &Path, id: &str, request: Request) -> Result<serde_json::Value> {
    dispatch_package(root, id, BIOME, request)
}
fn list_packages(root: &Path, id: &str) -> serde_json::Value {
    let mut packages = vec![];
    let mut failures = vec![];
    if let Ok(entries) = fs::read_dir(root.join("environments")) {
        for entry in entries.flatten().filter(|e| e.path().is_dir()) {
            let key = entry.file_name().to_string_lossy().into_owned();
            match load_package(root,id,&key) {
                Ok(s) => packages.push(serde_json::json!({"id":s.package_id,"name":s.display_name,"canvas":s.canvas,"assets":s.layers.iter().filter(|l| !l.removed).count(),"revision":s.revision})),
                Err(e) => failures.push(format!("{key}: {e}")),
            }
        }
    }
    packages.sort_by_key(|p| p["name"].as_str().unwrap_or_default().to_lowercase());
    serde_json::json!({"packages":packages,"failures":failures})
}
fn empty_layer(
    name: String,
    order: usize,
    asset_type: AssetType,
    native_size: Option<[u32; 2]>,
    rules: Rules,
) -> Layer {
    Layer {
        asset_id: format!("asset-{}", Uuid::new_v4()),
        order,
        parameters: Parameters::default(),
        current_source_id: None,
        current_revision_id: None,
        sources: vec![],
        revisions: vec![],
        exports: vec![],
        exported_revision_ids: vec![],
        name,
        asset_type,
        removed: false,
        rules,
        native_size,
        pivot: None,
        position: [0, 0],
        modified_at: chrono::Utc::now().to_rfc3339(),
    }
}
fn create_package(
    root: &Path,
    id: &str,
    name: String,
    template: bool,
    canvas: [u32; 2],
    palette: PaletteProfile,
    layer_count: usize,
) -> Result<Environment> {
    if layer_count > 128 {
        return Err("Start with at most 128 slots".into());
    }
    let mut s = Environment::new(id);
    s.package_id = format!("package-{}", Uuid::new_v4());
    s.display_name = name;
    if !template {
        s.template_id = None;
        s.biome_id = String::new();
        s.arena_id = String::new();
        s.canvas = canvas;
        s.palette = palette;
        s.palette_version = s.palette.id.clone();
        s.guides = Guides::default();
        s.layers = (0..layer_count)
            .map(|i| {
                empty_layer(
                    format!("Layer {}", i + 1),
                    i,
                    AssetType::BackgroundLayer,
                    None,
                    Rules::default(),
                )
            })
            .collect();
    }
    save(root, &s)?;
    Ok(s)
}
fn dispatch_package(
    root: &Path,
    id: &str,
    package: &str,
    request: Request,
) -> Result<serde_json::Value> {
    let mut s = if let Request::Create {
        name,
        template,
        canvas,
        palette,
        layer_count,
    } = &request
    {
        create_package(
            root,
            id,
            name.clone(),
            *template,
            *canvas,
            palette.clone(),
            *layer_count,
        )?
    } else {
        load_package(root, id, package)?
    };
    let mut result = serde_json::Value::Null;
    let mut validation = None;
    let mut metadata_history = serde_json::Value::Null;
    match request {
        Request::Load | Request::Create { .. } => {}
        Request::PackageHistory => {
            let dir = package_folder(root, &s.package_id)?.join("history");
            let mut entries = fs::read_dir(dir)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .collect::<Vec<_>>();
            entries
                .sort_by_key(|e| std::cmp::Reverse(e.metadata().and_then(|m| m.modified()).ok()));
            let records=entries.into_iter().take(100).filter_map(|entry|{
                let bytes=fs::read(entry.path()).ok()?;
                let old:Environment=serde_json::from_slice(&bytes).ok()?;
                if old.schema_version>2 || !legacy_is_valid(&old) {return None;}
                let old=upgrade(old);
                if old.package_id!=s.package_id || validate_state(&old,id).is_err(){return None;}
                Some(serde_json::json!({"snapshot":entry.file_name().to_string_lossy(),"name":old.display_name,"canvas":old.canvas,"palette":old.palette,"guides":old.guides,"biome":old.biome_id,"arena":old.arena_id,"revision":old.revision,"assets":old.layers.len()}))
            }).collect::<Vec<_>>();
            metadata_history = serde_json::json!(records);
        }
        Request::Configure {
            name,
            biome,
            arena,
            canvas,
            palette,
            guides,
        } => {
            validate_dimensions(canvas)?;
            validate_palette(&palette)?;
            s.display_name = name;
            s.biome_id = biome;
            s.arena_id = arena;
            s.guides = guides;
            if s.canvas != canvas || s.palette != palette {
                let old_contracts = s.layers.iter().map(|l| contract(&s, l)).collect::<Vec<_>>();
                s.canvas = canvas;
                s.palette = palette;
                s.palette_version = s.palette.id.clone();
                for (l, old) in s.layers.iter_mut().zip(old_contracts) {
                    let new = AssetContract {
                        canvas: l.native_size.unwrap_or(s.canvas),
                        palette: s.palette.clone(),
                        rules: l.rules.clone(),
                    };
                    if new != old {
                        l.current_revision_id = None;
                    }
                }
            }
            s.revision += 1;
            save(root, &s)?;
        }
        Request::Duplicate { name } => {
            s.package_id = format!("package-{}", Uuid::new_v4());
            s.display_name = name;
            s.template_id = None;
            s.master_reference = None;
            s.package_exports.clear();
            s.composite_exports.clear();
            s.export_history.clear();
            s.revision = 0;
            for l in &mut s.layers {
                l.sources.clear();
                l.revisions.clear();
                l.exports.clear();
                l.exported_revision_ids.clear();
                l.current_source_id = None;
                l.current_revision_id = None;
            }
            save(root, &s)?;
        }
        Request::AddAsset {
            name,
            asset_type,
            native_size,
            rules,
        } => {
            s.layers.push(empty_layer(
                name,
                s.layers.len(),
                asset_type,
                native_size,
                rules,
            ));
            s.revision += 1;
            save(root, &s)?;
        }
        Request::AssetSettings {
            order,
            name,
            rules,
            native_size,
            pivot,
            position,
        } => {
            let l = layer_mut(&mut s, order)?;
            if l.rules != rules || l.native_size != native_size {
                l.current_revision_id = None;
            }
            l.name = name;
            l.rules = rules;
            l.native_size = native_size;
            l.pivot = pivot;
            l.position = position;
            l.modified_at = chrono::Utc::now().to_rfc3339();
            s.revision += 1;
            save(root, &s)?;
        }
        Request::RemoveAsset { order } => {
            layer_mut(&mut s, order)?.removed = true;
            s.revision += 1;
            save(root, &s)?;
        }
        Request::RestoreAsset { order } => {
            s.layers.get_mut(order).ok_or("Asset not found")?.removed = false;
            s.revision += 1;
            save(root, &s)?;
        }
        Request::Reorder { asset_ids } => {
            if asset_ids.len() != s.layers.len()
                || asset_ids.iter().collect::<BTreeSet<_>>().len() != asset_ids.len()
            {
                return Err("Reordering must include each asset exactly once".into());
            }
            let layers = asset_ids
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    let mut l = s
                        .layers
                        .iter()
                        .find(|l| &l.asset_id == id)
                        .ok_or("Unknown asset")?
                        .clone();
                    l.order = i;
                    Ok(l)
                })
                .collect::<Result<Vec<_>>>()?;
            s.layers = layers;
            s.revision += 1;
            save(root, &s)?;
        }
        Request::ExportPackage { orders } => {
            result = export_package(root, &mut s, &orders)?.into();
        }
        Request::AttachReference { path } => {
            let bytes = fs::read(&path).map_err(|e| e.to_string())?;
            let image = decode(&bytes)?;
            let relative = format!(
                "environments/{}/references/{}/original.png",
                s.package_id,
                Uuid::new_v4()
            );
            write_bound(root, &root.join(&relative), &bytes)?;
            s.master_reference = Some(ReferenceImage {
                path: relative,
                sha256: hash(&bytes),
                original_name: Path::new(&path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                width: image.width(),
                height: image.height(),
            });
            save(root, &s)?;
        }
        Request::ReferencePreview => {
            let r = s
                .master_reference
                .as_ref()
                .ok_or("Attach a master reference first")?;
            result = preview(&decode(&checked_bytes(root, &r.path, &r.sha256)?)?)?;
        }
        Request::RevisionPreview { order, revision } => {
            let l = layer_mut(&mut s, order)?;
            let r = l
                .revisions
                .iter()
                .find(|r| r.id == revision)
                .ok_or("Revision not found")?;
            result = preview(&decode(&checked_bytes(root, &r.path, &r.sha256)?)?)?;
        }
        Request::OpenExport { path } => {
            // Only previously recorded export folders can be opened, never arbitrary client paths.
            let permitted = s
                .package_exports
                .iter()
                .chain(s.composite_exports.iter())
                .chain(s.layers.iter().flat_map(|l| &l.exports))
                .any(|p| p == &path);
            if !permitted {
                return Err("Export directory is not registered in this package".into());
            }
            let dir = relative_file(root, &path)?;
            crate::studio::open_folder(&dir)?;
            result = dir.to_string_lossy().into_owned().into();
        }
        Request::ReviewComposite { layers } => {
            if layers.is_empty()
                || layers
                    .iter()
                    .map(|l| l.order)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != layers.len()
            {
                return Err("Choose unique compositor layers".into());
            }
            let mut out = RgbaImage::new(s.canvas[0], s.canvas[1]);
            let mut sorted = layers.iter().collect::<Vec<_>>();
            sorted.sort_by_key(|l| l.order);
            for item in sorted {
                if !item.opacity.is_finite() || !(0.0..=1.0).contains(&item.opacity) {
                    return Err("Review opacity must be 0–1".into());
                }
                let l = s
                    .layers
                    .get(item.order)
                    .filter(|l| !l.removed)
                    .ok_or("Asset unavailable")?;
                let r = l
                    .revisions
                    .iter()
                    .find(|r| r.id == item.revision)
                    .ok_or("Revision unavailable")?;
                let mut image = decode(&checked_bytes(root, &r.path, &r.sha256)?)?;
                if l.asset_type == AssetType::BackgroundLayer
                    && image.dimensions() != (s.canvas[0], s.canvas[1])
                {
                    return Err("Background revision dimensions differ from package canvas".into());
                }
                for pixel in image.pixels_mut() {
                    pixel[3] = (f32::from(pixel[3]) * item.opacity).round() as u8;
                }
                imageops::overlay(
                    &mut out,
                    &image,
                    i64::from(l.position[0]),
                    i64::from(l.position[1]),
                );
            }
            let relative = format!(
                "environments/{}/review-composites/{}",
                s.package_id,
                Uuid::new_v4()
            );
            let dir = root.join(&relative);
            let bytes = png(&out)?;
            write_bound(root, &dir.join("composite.review-only.png"), &bytes)?;
            write_bound(root,&dir.join("manifest.review-only.json"),&serde_json::to_vec_pretty(&serde_json::json!({"schemaVersion":2,"status":"review_only","layers":layers,"canvas":s.canvas,"origin":[0,0],"guidesIncluded":false,"masterReferenceIncluded":false,"sha256":hash(&bytes)})).map_err(|e|e.to_string())?)?;
            s.export_history.push(ExportRecord {
                path: relative.clone(),
                kind: "review_only".into(),
                exported_at: chrono::Utc::now().to_rfc3339(),
            });
            s.composite_exports.push(relative);
            save(root, &s)?;
            result = dir.to_string_lossy().into_owned().into();
        }
        Request::Import { order, path } => import(root, &mut s, order, Path::new(&path))?,
        Request::Settings { order, parameters } => set_parameters(root, &mut s, order, parameters)?,
        Request::Process { order } => propose(root, &mut s, order)?,
        Request::Approve { order, reviewer } => approve(root, &mut s, order, &reviewer)?,
        Request::Export { order } => result = export_layer(root, &mut s, order)?.into(),
        Request::Restore { order, revision } => {
            let l = layer_mut(&mut s, order)?;
            let r = l
                .revisions
                .iter()
                .find(|r| r.id == revision)
                .ok_or("Revision not found")?
                .clone();
            checked_bytes(root, &r.path, &r.sha256)?;
            if r.contract.as_ref() != Some(&contract(&s, &s.layers[order])) {
                return Err("This revision used a different package palette/canvas/rule contract. Restore that configuration explicitly before selecting it.".into());
            }
            let l = layer_mut(&mut s, order)?;
            l.current_source_id = Some(r.source_id);
            l.parameters = r.parameters;
            l.current_revision_id = Some(r.id);
            save(root, &s)?;
        }
        Request::Preview { order, original } => {
            let c = contract(&s, s.layers.get(order).ok_or("Asset not found")?);
            let l = layer_mut(&mut s, order)?;
            let (path, hash) = if original {
                let src = l
                    .sources
                    .iter()
                    .find(|src| Some(&src.id) == l.current_source_id.as_ref())
                    .ok_or("No original source")?;
                (&src.path, &src.sha256)
            } else {
                let r = current(l)?;
                (&r.path, &r.sha256)
            };
            let image = decode(&checked_bytes(root, path, hash)?)?;
            validation = Some(inspect_asset(&image, &c));
            result = preview(&image)?;
        }
        Request::Composite { visible, export } => {
            let image = composite(root, &s, &visible)?;
            result = if export {
                let dir = package_folder(root, &s.package_id)?
                    .join("review-composites")
                    .join(Uuid::new_v4().to_string());
                write_bound(root, &dir.join("composite.review-only.png"), &png(&image)?)?;
                write_bound(root,&dir.join("manifest.review-only.json"),&serde_json::to_vec_pretty(&serde_json::json!({"status":"review_only","visibleLayerOrders":visible,"processingRevisions":s.layers.iter().filter(|l|visible.contains(&l.order)).map(|l|(&l.asset_id,&l.current_revision_id)).collect::<Vec<_>>(),"origin":[0,0],"canvas":s.canvas,"guidesIncluded":false})).map_err(|e|e.to_string())?)?;
                let relative = dir
                    .strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                s.export_history.push(ExportRecord {
                    path: relative.clone(),
                    kind: "review_only".into(),
                    exported_at: chrono::Utc::now().to_rfc3339(),
                });
                s.composite_exports.push(relative);
                save(root, &s)?;
                dir.to_string_lossy().into_owned().into()
            } else {
                preview(&image)?
            };
        }
    }
    Ok(
        serde_json::json!({"environment":s,"result":result,"recoveryNotice":s.recovery_notice,"catalog":list_packages(root,id),"validation":validation,"metadataHistory":metadata_history}),
    )
}
fn owner_root(workspace: &Path, app: &tauri::AppHandle, id: &str) -> Result<PathBuf> {
    if Uuid::parse_str(id).is_err() {
        return Err("Invalid environment project ID".into());
    }
    let dir = workspace.join("environment-projects").join(id);
    if dir.join("owner.json").exists() {
        let resolved = fs::canonicalize(&dir).map_err(|e| e.to_string())?;
        if !resolved.starts_with(workspace) {
            return Err("Environment project escapes workspace".into());
        }
        return Ok(resolved);
    }
    crate::studio::environment_project_dir(app, id)
}
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum LibraryRequest {
    Load,
    CreateProject { name: String },
}
fn library_in(workspace: &Path, request: LibraryRequest) -> Result<serde_json::Value> {
    let mut created = None;
    if let LibraryRequest::CreateProject { name } = request {
        if name.trim().is_empty() || name.len() > 160 {
            return Err("Environment project name must be 1–160 characters".into());
        }
        let id = Uuid::new_v4().to_string();
        let dir = workspace.join("environment-projects").join(&id);
        write_bound(workspace,&dir.join("owner.json"),&serde_json::to_vec_pretty(&serde_json::json!({"id":id,"name":name.trim(),"kind":"environment","schemaVersion":1})).map_err(|e|e.to_string())?)?;
        created = Some(id);
    }
    let mut owners = vec![];
    let mut failures = vec![];
    for (directory, file, kind) in [
        ("projects", "project.json", "existing_project"),
        ("environment-projects", "owner.json", "environment"),
    ] {
        if let Ok(entries) = fs::read_dir(workspace.join(directory)) {
            for entry in entries.flatten().filter(|e| e.path().is_dir()) {
                match fs::read(entry.path().join(file))
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                {
                    Some(v) if v["id"].as_str().is_some() && v["name"].as_str().is_some() => {
                        let resolved = fs::canonicalize(entry.path()).map_err(|e| e.to_string())?;
                        if !resolved.starts_with(workspace) {
                            failures.push("Skipped a project outside the workspace".to_string());
                            continue;
                        }
                        let catalog = list_packages(&resolved, v["id"].as_str().unwrap());
                        owners.push(serde_json::json!({"id":v["id"],"name":v["name"],"kind":kind,"catalog":catalog}));
                    }
                    _ => {
                        failures.push(format!("Cannot read {}", entry.path().join(file).display()))
                    }
                }
            }
        }
    }
    owners.sort_by_key(|o| o["name"].as_str().unwrap_or_default().to_lowercase());
    Ok(serde_json::json!({"owners":owners,"createdId":created,"failures":failures}))
}
#[tauri::command]
pub async fn environment_library(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::studio::StudioLock>,
    request: LibraryRequest,
) -> Result<serde_json::Value> {
    let lock = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock
            .0
            .lock()
            .map_err(|_| "Project operation lock unavailable")?;
        library_in(&crate::studio::environment_workspace_root(&app)?, request)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn environment_command(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::studio::StudioLock>,
    project_id: String,
    package_id: Option<String>,
    request: Request,
) -> Result<serde_json::Value> {
    let lock = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock
            .0
            .lock()
            .map_err(|_| "Project operation lock is unavailable")?;
        let workspace = crate::studio::environment_workspace_root(&app)?;
        let root = owner_root(&workspace, &app, &project_id)?;
        dispatch_package(
            &root,
            &project_id,
            package_id.as_deref().unwrap_or(BIOME),
            request,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    fn fixture(order: usize) -> RgbaImage {
        let mut image = RgbaImage::new(W, H);
        if order == 0 {
            for p in image.pixels_mut() {
                *p = Rgba([243, 181, 107, 255]);
            }
        } else if order == 4 {
            for (x, y, p) in image.enumerate_pixels_mut() {
                if y >= 224 {
                    *p = Rgba([169, 84, 56, 255]);
                } else if x == 0 {
                    *p = Rgba([0, 0, 0, 0]);
                }
            }
        } else if order != 5 {
            image.put_pixel(156, 100, Rgba([71, 118, 106, 255]));
        }
        image
    }
    fn add(root: &Path, s: &mut Environment, order: usize) {
        let source = root.join(format!("fixture-{order}.png"));
        fixture(order).save(&source).unwrap();
        import(root, s, order, &source).unwrap();
        propose(root, s, order).unwrap();
    }
    #[test]
    fn import_preserves_original_bytes_and_records_dimensions() {
        let tmp = tempdir().unwrap();
        let mut s = Environment::new("test");
        add(tmp.path(), &mut s, 0);
        let src = &s.layers[0].sources[0];
        assert_eq!(
            fs::read(tmp.path().join(&src.path)).unwrap(),
            fs::read(tmp.path().join("fixture-0.png")).unwrap()
        );
        assert_eq!((src.report.width, src.report.height), (W, H));
        assert_eq!(
            src.sha256,
            hash(&fs::read(tmp.path().join(&src.path)).unwrap())
        );
    }
    #[test]
    fn sky_is_opaque_and_cleanup_is_rejected() {
        let image = fixture(0);
        assert!(inspect(&image, 0).valid);
        let mut p = Parameters::default();
        p.remove_background = Some("#F3B56B".into());
        assert!(process(&image, &p, 0).unwrap_err().contains("disabled"));
        assert!(!inspect(&RgbaImage::new(W, H), 0).alpha_pass);
    }
    #[test]
    fn transparency_and_empty_foreground_are_valid() {
        assert!(inspect(&fixture(1), 1).valid);
        assert!(inspect(&fixture(5), 5).valid);
        let mut image = fixture(1);
        image.put_pixel(0, 0, Rgba([71, 118, 106, 127]));
        assert!(!inspect(&image, 1).alpha_pass);
    }
    #[test]
    fn palette_validates_pixels_not_selected_settings() {
        let mut image = fixture(0);
        image.put_pixel(0, 0, Rgba([1, 2, 3, 255]));
        let r = inspect(&image, 0);
        assert!(!r.valid);
        assert_eq!(r.unexpected_colors, vec!["#010203"]);
        let p = Parameters {
            remap_palette: true,
            ..Default::default()
        };
        let out = process(&image, &p, 0).unwrap();
        assert!(inspect(&out, 0).palette_pass);
        assert_ne!(out, image);
    }
    #[test]
    fn only_surface_requires_ground_alpha_rows() {
        assert!(inspect(&fixture(4), 4).valid);
        let mut image = fixture(4);
        image.put_pixel(200, 223, Rgba([169, 84, 56, 255]));
        assert!(!inspect(&image, 4).ground_pass);
        assert!(inspect(&image, 3).ground_pass);
        image = fixture(4);
        image.put_pixel(200, 224, Rgba([0, 0, 0, 0]));
        assert!(!inspect(&image, 4).ground_pass);
    }
    #[test]
    fn normalization_is_explicit_nearest_and_never_trims() {
        let mut source = RgbaImage::new(960, 540);
        source.put_pixel(300, 200, Rgba([71, 118, 106, 255]));
        let unchanged = process(&source, &Parameters::default(), 1).unwrap();
        assert_eq!(unchanged.dimensions(), (960, 540));
        assert!(!inspect(&unchanged, 1).canvas_pass);
        let p = Parameters {
            resize_to_canvas: true,
            ..Default::default()
        };
        let out = process(&source, &p, 1).unwrap();
        assert_eq!(out.dimensions(), (W, H));
        assert_eq!(
            out,
            imageops::resize(&source, W, H, imageops::FilterType::Nearest)
        );
        assert_eq!(out.get_pixel(0, 0)[3], 0);
    }
    #[test]
    fn full_canvas_origin_and_sparse_content_are_preserved() {
        let mut source = RgbaImage::new(W, H);
        source.put_pixel(479, 269, Rgba([71, 118, 106, 255]));
        let out = process(&source, &Parameters::default(), 1).unwrap();
        assert_eq!(source, out);
        assert_eq!(inspect(&out, 1).origin, [0, 0]);
    }
    #[test]
    fn processing_and_png_export_are_deterministic() {
        let mut source = fixture(1);
        source.put_pixel(0, 0, Rgba([0, 255, 0, 255]));
        source.put_pixel(1, 0, Rgba([71, 118, 106, 100]));
        let p = Parameters {
            pixel_pitch: 2,
            remove_background: Some("#00FF00".into()),
            binary_alpha: true,
            remap_palette: true,
            ..Default::default()
        };
        assert_eq!(
            png(&process(&source, &p, 1).unwrap()).unwrap(),
            png(&process(&source, &p, 1).unwrap()).unwrap()
        );
        assert_eq!(process(&source, &p, 1).unwrap().get_pixel(0, 0)[3], 0);
    }
    #[test]
    fn conservative_pitch_detection_does_not_claim_flat_sky() {
        assert_eq!(pitch(&fixture(0)), None);
        let mut source = RgbaImage::new(12, 12);
        for (x, y, p) in source.enumerate_pixels_mut() {
            *p = Rgba([if (x / 3 + y / 3) % 2 == 0 { 100 } else { 200 }, 0, 0, 255]);
        }
        assert_eq!(pitch(&source), Some(3));
        source.put_pixel(1, 1, Rgba([1, 0, 0, 255]));
        assert_eq!(pitch(&source), None);
    }
    #[test]
    fn composite_order_is_fixed_even_when_visibility_order_is_reversed() {
        let tmp = tempdir().unwrap();
        let mut s = Environment::new("test");
        for i in 0..6 {
            add(tmp.path(), &mut s, i);
        }
        // Layer 2 intentionally replaces layer 1's pixel; visibility input order cannot reorder paint.
        let r = current(&s.layers[2]).unwrap().clone();
        let mut image = fixture(2);
        image.put_pixel(156, 100, Rgba([129, 64, 50, 255]));
        let bytes = png(&image).unwrap();
        fs::write(tmp.path().join(&r.path), &bytes).unwrap();
        let revision = s.layers[2].revisions.last_mut().unwrap();
        revision.sha256 = hash(&bytes);
        let normal = composite(tmp.path(), &s, &[0, 1, 2, 3, 4, 5]).unwrap();
        let reversed = composite(tmp.path(), &s, &[5, 4, 3, 2, 1, 0]).unwrap();
        assert_eq!(normal, reversed);
        assert_eq!(
            *composite(tmp.path(), &s, &[2, 1])
                .unwrap()
                .get_pixel(156, 100),
            Rgba([129, 64, 50, 255])
        );
        assert!(composite(tmp.path(), &s, &[0, 0]).is_err());
    }
    #[test]
    fn layer_invalidation_preserves_other_layers_and_character_files() {
        let tmp = tempdir().unwrap();
        let mut s = Environment::new("test");
        fs::write(
            tmp.path().join("project.json"),
            b"character metadata sentinel",
        )
        .unwrap();
        fs::create_dir(tmp.path().join("animations")).unwrap();
        fs::write(
            tmp.path().join("animations/recovered.png"),
            b"manually recovered",
        )
        .unwrap();
        add(tmp.path(), &mut s, 0);
        add(tmp.path(), &mut s, 1);
        approve(tmp.path(), &mut s, 0, "test reviewer").unwrap();
        let out = export_layer(tmp.path(), &mut s, 0).unwrap();
        let other = serde_json::to_value(&s.layers[1]).unwrap();
        let prior = current(&s.layers[0]).unwrap().id.clone();
        set_parameters(
            tmp.path(),
            &mut s,
            0,
            Parameters {
                remap_palette: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(s.layers[0].current_revision_id.is_none());
        assert_eq!(other, serde_json::to_value(&s.layers[1]).unwrap());
        assert!(Path::new(&out).exists());
        assert!(s.layers[0]
            .revisions
            .iter()
            .any(|r| r.id == prior && r.approved_at.is_some()));
        assert_eq!(
            fs::read(tmp.path().join("project.json")).unwrap(),
            b"character metadata sentinel"
        );
        assert_eq!(
            fs::read(tmp.path().join("animations/recovered.png")).unwrap(),
            b"manually recovered"
        );
        assert_eq!(
            load(tmp.path(), "test").unwrap().layers[0]
                .parameters
                .remap_palette,
            true
        );
    }
    #[test]
    fn old_revision_can_be_restored_without_deleting_new_outputs() {
        let tmp = tempdir().unwrap();
        let mut s = Environment::new("test");
        add(tmp.path(), &mut s, 0);
        let old = current(&s.layers[0]).unwrap().id.clone();
        set_parameters(
            tmp.path(),
            &mut s,
            0,
            Parameters {
                remap_palette: true,
                ..Default::default()
            },
        )
        .unwrap();
        propose(tmp.path(), &mut s, 0).unwrap();
        let reply = dispatch(
            tmp.path(),
            "test",
            Request::Restore {
                order: 0,
                revision: old.clone(),
            },
        )
        .unwrap();
        assert_eq!(reply["environment"]["layers"][0]["currentRevisionId"], old);
        assert_eq!(
            reply["environment"]["layers"][0]["revisions"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }
    #[test]
    fn export_requires_review_and_rechecks_source_and_revision_hashes() {
        let tmp = tempdir().unwrap();
        let mut s = Environment::new("test");
        add(tmp.path(), &mut s, 0);
        assert!(export_layer(tmp.path(), &mut s, 0).is_err());
        assert!(approve(tmp.path(), &mut s, 0, "").is_err());
        approve(tmp.path(), &mut s, 0, "QA").unwrap();
        let source = s.layers[0].sources[0].path.clone();
        fs::write(tmp.path().join(&source), b"corrupt").unwrap();
        assert!(export_layer(tmp.path(), &mut s, 0)
            .unwrap_err()
            .contains("Hash mismatch"));
    }
    #[test]
    fn corrupt_processed_pixels_cannot_export() {
        let tmp = tempdir().unwrap();
        let mut s = Environment::new("test");
        add(tmp.path(), &mut s, 0);
        approve(tmp.path(), &mut s, 0, "QA").unwrap();
        fs::write(
            tmp.path().join(&current(&s.layers[0]).unwrap().path),
            b"corrupt",
        )
        .unwrap();
        assert!(export_layer(tmp.path(), &mut s, 0)
            .unwrap_err()
            .contains("Hash mismatch"));
    }
    #[test]
    fn interrupted_rotation_recovers_without_resetting_workspace() {
        let tmp = tempdir().unwrap();
        let mut s = Environment::new("test");
        add(tmp.path(), &mut s, 0);
        let before = serde_json::to_value(&s).unwrap();
        let dir = folder(tmp.path());
        fs::rename(dir.join("index.json"), dir.join("index.pending.json")).unwrap();
        // An older backup is preferred over a pending commit, never a blank reset.
        let recovered = load(tmp.path(), "test").unwrap();
        assert!(recovered.recovery_notice.is_some());
        assert!(!recovered.layers[0].sources.is_empty());
        fs::remove_file(dir.join("index.previous.json")).unwrap();
        assert_eq!(
            serde_json::to_value(load(tmp.path(), "test").unwrap()).unwrap(),
            before
        );
        fs::write(dir.join("index.pending.json"), b"interrupted partial JSON").unwrap();
        assert!(load(tmp.path(), "test").unwrap().recovery_notice.is_some());
        assert!(dir.join("index.pending.json").exists());
    }
    #[test]
    fn future_schema_is_never_downgraded_to_older_backup() {
        let tmp = tempdir().unwrap();
        let mut s = Environment::new("test");
        add(tmp.path(), &mut s, 0);
        fs::write(
            folder(tmp.path()).join("index.json"),
            br#"{"schemaVersion":3}"#,
        )
        .unwrap();
        assert!(load(tmp.path(), "test")
            .unwrap_err()
            .contains("Newer environment schema"));
    }
    #[test]
    fn malformed_request_paths_and_parameters_fail_clearly() {
        let tmp = tempdir().unwrap();
        assert!(relative_file(tmp.path(), "../project.json").is_err());
        assert!(relative_file(tmp.path(), "animations/test.png").is_err());
        assert!(process(
            &fixture(1),
            &Parameters {
                pixel_pitch: 0,
                ..Default::default()
            },
            1
        )
        .is_err());
        assert!(serde_json::from_value::<Request>(
            serde_json::json!({"operation":"process","order":0,"pivot":112})
        )
        .is_err());
        let mut s = Environment::new("test");
        assert!(import(tmp.path(), &mut s, 9, Path::new("missing.png")).is_err());
        assert!(decode(b"not a PNG").is_err());
        assert!(color("#ééé").is_err());
    }
    #[test]
    fn complete_import_process_approve_export_reopen_cycle() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let mut s = Environment::new("test");
        for order in 0..6 {
            add(root, &mut s, order);
            approve(root, &mut s, order, "Synthetic QA fixture (not scenery)").unwrap();
            export_layer(root, &mut s, order).unwrap();
        }
        let mut reopened = load(root, "test").unwrap();
        assert_eq!(
            serde_json::to_value(&s).unwrap(),
            serde_json::to_value(&reopened).unwrap()
        );
        let old_dir = root.join(&reopened.layers[0].exports[0]);
        let next = export_layer(root, &mut reopened, 0).unwrap();
        assert_eq!(
            fs::read(old_dir.join(format!("{}.png", IDS[0]))).unwrap(),
            fs::read(Path::new(&next).join(format!("{}.png", IDS[0]))).unwrap()
        );
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(Path::new(&next).join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(manifest["assetType"], "static_environment_layer");
        assert_eq!(manifest["origin"], serde_json::json!([0, 0]));
        assert_eq!(manifest["validation"]["valid"], true);
        for absent in ["pivot", "fps", "tickHolds"] {
            assert!(manifest.get(absent).is_none());
        }
        let reply = dispatch(
            root,
            "test",
            Request::Composite {
                visible: vec![0, 1, 2, 3, 4, 5],
                export: true,
            },
        )
        .unwrap();
        let dir = Path::new(reply["result"].as_str().unwrap());
        assert_eq!(
            decode(&fs::read(dir.join("composite.review-only.png")).unwrap()).unwrap(),
            composite(root, &reopened, &[0, 1, 2, 3, 4, 5]).unwrap()
        );
        let m: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("manifest.review-only.json")).unwrap())
                .unwrap();
        assert_eq!(m["guidesIncluded"], false);
    }
    fn custom(root: &Path, id: &str) -> Environment {
        create_package(
            root,
            id,
            "Independent world".into(),
            false,
            [8, 6],
            PaletteProfile {
                id: "original".into(),
                name: "Original".into(),
                colors: vec![],
                slots: vec![],
                enforce: false,
            },
            2,
        )
        .unwrap()
    }
    fn custom_add(root: &Path, s: &mut Environment, order: usize, size: [u32; 2]) {
        let mut image = RgbaImage::new(size[0], size[1]);
        image.put_pixel(size[0] - 1, size[1] - 1, Rgba([10, 20, 30, 255]));
        let path = root.join(format!("custom-{order}.png"));
        image.save(&path).unwrap();
        import(root, s, order, &path).unwrap();
        propose(root, s, order).unwrap();
    }
    #[test]
    fn legacy_sidecar_loads_without_writing_and_first_save_retains_v1_backup() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let mut s = Environment::new("test");
        add(root, &mut s, 4);
        let mut v = serde_json::to_value(&s).unwrap();
        v["schemaVersion"] = 1.into();
        for key in [
            "packageId",
            "displayName",
            "templateId",
            "palette",
            "guides",
            "masterReference",
            "packageExports",
            "compositeExports",
            "exportHistory",
            "revision",
        ] {
            v.as_object_mut().unwrap().remove(key);
        }
        for l in v["layers"].as_array_mut().unwrap() {
            for key in [
                "name",
                "assetType",
                "removed",
                "rules",
                "nativeSize",
                "pivot",
                "position",
                "modifiedAt",
            ] {
                l.as_object_mut().unwrap().remove(key);
            }
            for r in l["revisions"].as_array_mut().unwrap() {
                r.as_object_mut().unwrap().remove("contract");
            }
        }
        let path = folder(root).join("index.json");
        let old = serde_json::to_vec_pretty(&v).unwrap();
        fs::write(&path, &old).unwrap();
        let reopened = load(root, "test").unwrap();
        assert_eq!(fs::read(&path).unwrap(), old);
        assert_eq!(reopened.layers[4].rules.ground_y, Some(224));
        assert!(validate_current(root, &reopened.layers[4]).unwrap().valid);
        save(root, &reopened).unwrap();
        let backups = fs::read_dir(folder(root))
            .unwrap()
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("index.v1-backup-")
            })
            .collect::<Vec<_>>();
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read(backups[0].path()).unwrap(), old);
        assert_eq!(
            load(root, "test").unwrap().layers[4].sources[0].sha256,
            s.layers[4].sources[0].sha256
        );
    }
    #[test]
    fn custom_package_and_removed_assets_reopen_without_deleting_history() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let mut s = custom(root, "a");
        custom_add(root, &mut s, 0, [8, 6]);
        let src = s.layers[0].sources[0].clone();
        let old = current(&s.layers[0]).unwrap().clone();
        let package = s.package_id.clone();
        dispatch_package(root, "a", &package, Request::RemoveAsset { order: 0 }).unwrap();
        let removed = load_package(root, "a", &package).unwrap();
        assert!(removed.layers[0].removed);
        assert_eq!(
            checked_bytes(root, &src.path, &src.sha256).unwrap(),
            fs::read(root.join("custom-0.png")).unwrap()
        );
        assert!(root.join(&old.path).exists());
        dispatch_package(root, "a", &package, Request::RestoreAsset { order: 0 }).unwrap();
        assert!(!load_package(root, "a", &package).unwrap().layers[0].removed);
        dispatch_package(
            root,
            "a",
            &package,
            Request::AddAsset {
                name: "New rock".into(),
                asset_type: AssetType::Prop,
                native_size: Some([4, 4]),
                rules: Rules::default(),
            },
        )
        .unwrap();
        assert_eq!(load_package(root, "a", &package).unwrap().layers.len(), 3);
    }
    #[test]
    fn independent_prop_and_tile_use_native_dimensions_and_pivot_metadata() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let mut s = custom(root, "a");
        s.layers[0].asset_type = AssetType::Prop;
        s.layers[0].native_size = Some([3, 4]);
        s.layers[0].pivot = Some([1, 3]);
        s.layers[0].position = [2, 1];
        custom_add(root, &mut s, 0, [3, 4]);
        assert!(current(&s.layers[0]).unwrap().report.valid);
        approve(root, &mut s, 0, "QA").unwrap();
        let out = export_layer(root, &mut s, 0).unwrap();
        let m: serde_json::Value =
            serde_json::from_slice(&fs::read(Path::new(&out).join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(m["pivot"], serde_json::json!([1, 3]));
        assert_eq!(m["canvas"]["width"], 3);
        assert_eq!(
            *composite(root, &s, &[0]).unwrap().get_pixel(4, 4),
            Rgba([10, 20, 30, 255])
        );
        let c = contract(&s, &s.layers[0]);
        let tile = RgbaImage::from_pixel(3, 4, Rgba([10, 20, 30, 255]));
        assert_eq!(inspect_asset(&tile, &c).edge_mismatches, [0, 0]);
        let mut broken = tile.clone();
        broken.put_pixel(0, 1, Rgba([99, 20, 30, 255]));
        let mut tile_contract = c;
        tile_contract.rules.seamless_required = true;
        assert!(!inspect_asset(&broken, &tile_contract).valid);
    }
    #[test]
    fn fit_pad_fill_are_explicit_and_full_canvas_origin_is_preserved_by_default() {
        let tmp = tempdir().unwrap();
        let s = custom(tmp.path(), "a");
        let c = contract(&s, &s.layers[0]);
        let mut source = RgbaImage::new(4, 2);
        source.put_pixel(3, 1, Rgba([10, 20, 30, 255]));
        assert_eq!(
            process_asset(&source, &Parameters::default(), &c).unwrap(),
            source
        );
        let pad = Parameters {
            canvas_mode: CanvasMode::Pad,
            offset: [2, 3],
            ..Default::default()
        };
        let out = process_asset(&source, &pad, &c).unwrap();
        assert_eq!(out.dimensions(), (8, 6));
        assert_eq!(*out.get_pixel(5, 4), Rgba([10, 20, 30, 255]));
        let invalid = Parameters {
            offset: [6, 5],
            ..pad.clone()
        };
        assert!(process_asset(&source, &invalid, &c)
            .unwrap_err()
            .contains("crop"));
        let fit = Parameters {
            canvas_mode: CanvasMode::Fit,
            ..Default::default()
        };
        let out = process_asset(&source, &fit, &c).unwrap();
        assert_eq!(out.dimensions(), (8, 6));
        assert_eq!(out.get_pixel(7, 5)[3], 0);
        assert_eq!(out.get_pixel(7, 4)[3], 255);
        let fill = Parameters {
            canvas_mode: CanvasMode::Fill,
            ..Default::default()
        };
        assert_eq!(
            process_asset(&source, &fill, &c).unwrap().dimensions(),
            (8, 6)
        );
    }
    #[test]
    fn palette_switch_stales_only_that_package_and_remap_is_explicit() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let mut a = custom(root, "a");
        custom_add(root, &mut a, 0, [8, 6]);
        let b = custom(root, "a");
        let prior = fs::read(
            package_folder(root, &b.package_id)
                .unwrap()
                .join("index.json"),
        )
        .unwrap();
        let p = PaletteProfile {
            id: "custom".into(),
            name: "Two".into(),
            colors: vec!["#000000".into(), "#FFFFFF".into()],
            slots: vec!["dark".into(), "light".into()],
            enforce: true,
        };
        dispatch_package(
            root,
            "a",
            &a.package_id,
            Request::Configure {
                name: a.display_name.clone(),
                biome: "".into(),
                arena: "".into(),
                canvas: a.canvas,
                palette: p,
                guides: Guides::default(),
            },
        )
        .unwrap();
        let mut a = load_package(root, "a", &a.package_id).unwrap();
        assert!(a.layers[0].current_revision_id.is_none());
        assert_eq!(a.layers[0].revisions.len(), 1);
        propose(root, &mut a, 0).unwrap();
        assert!(!current(&a.layers[0]).unwrap().report.palette_pass);
        set_parameters(
            root,
            &mut a,
            0,
            Parameters {
                remap_palette: true,
                ..Default::default()
            },
        )
        .unwrap();
        propose(root, &mut a, 0).unwrap();
        assert!(current(&a.layers[0]).unwrap().report.valid);
        assert_eq!(
            fs::read(
                package_folder(root, &b.package_id)
                    .unwrap()
                    .join("index.json")
            )
            .unwrap(),
            prior
        );
    }
    #[test]
    fn custom_package_export_rechecks_all_assets_and_preserves_individual_bytes() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let mut s = custom(root, "a");
        for i in 0..2 {
            custom_add(root, &mut s, i, [8, 6]);
        }
        assert!(export_package(root, &mut s, &[0, 1]).is_err());
        for i in 0..2 {
            approve(root, &mut s, i, "QA").unwrap();
        }
        let out = export_package(root, &mut s, &[1, 0]).unwrap();
        let m: serde_json::Value = serde_json::from_slice(
            &fs::read(Path::new(&out).join("package.manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(m["assets"][0]["layerOrder"], 0);
        assert_eq!(m["guidesIncluded"], false);
        for l in &s.layers {
            let r = current(l).unwrap();
            assert_eq!(
                hash(
                    &fs::read(Path::new(&out).join(format!("assets/{}.png", l.asset_id))).unwrap()
                ),
                r.sha256
            );
        }
        let source = s.layers[1].sources[0].clone();
        fs::write(root.join(&source.path), b"bad").unwrap();
        assert!(export_package(root, &mut s, &[0, 1])
            .unwrap_err()
            .contains("Hash mismatch"));
        assert!(Path::new(&out).exists());
    }
    #[test]
    fn reorder_is_persistent_without_invalidating_asset_revisions() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let mut s = custom(root, "a");
        for i in 0..2 {
            custom_add(root, &mut s, i, [8, 6]);
        }
        let ids = s
            .layers
            .iter()
            .rev()
            .map(|l| l.asset_id.clone())
            .collect::<Vec<_>>();
        let old = current(&s.layers[1]).unwrap().id.clone();
        dispatch_package(
            root,
            "a",
            &s.package_id,
            Request::Reorder {
                asset_ids: ids.clone(),
            },
        )
        .unwrap();
        let reopened = load_package(root, "a", &s.package_id).unwrap();
        assert_eq!(reopened.layers[0].asset_id, ids[0]);
        assert_eq!(current(&reopened.layers[0]).unwrap().id, old);
    }
    #[test]
    fn export_timeline_preserves_cross_kind_order_after_reopen() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let mut s = custom(root, "a");
        custom_add(root, &mut s, 0, [8, 6]);
        approve(root, &mut s, 0, "tester").unwrap();
        export_layer(root, &mut s, 0).unwrap();
        export_package(root, &mut s, &[0]).unwrap();
        let reopened = load_package(root, "a", &s.package_id).unwrap();
        assert_eq!(reopened.export_history.len(), 2);
        assert_eq!(reopened.export_history[0].kind, "asset");
        assert_eq!(reopened.export_history[1].kind, "package");
        assert_eq!(
            reopened.export_history[0].path,
            reopened.layers[0].exports[0]
        );
        assert_eq!(reopened.export_history[1].path, reopened.package_exports[0]);
        for event in &reopened.export_history {
            assert!(chrono::DateTime::parse_from_rfc3339(&event.exported_at).is_ok());
            assert!(relative_file(root, &event.path).unwrap().is_dir());
        }
    }
    #[test]
    fn master_reference_and_review_opacity_never_change_production_pixels() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let mut s = custom(root, "a");
        custom_add(root, &mut s, 0, [8, 6]);
        let source = s.layers[0].sources[0].clone();
        let r = current(&s.layers[0]).unwrap().clone();
        let reply = dispatch_package(
            root,
            "a",
            &s.package_id,
            Request::AttachReference {
                path: root.join("custom-0.png").to_string_lossy().into(),
            },
        )
        .unwrap();
        assert!(reply["environment"]["masterReference"]["sha256"]
            .as_str()
            .is_some());
        let reply = dispatch_package(
            root,
            "a",
            &s.package_id,
            Request::ReviewComposite {
                layers: vec![CompositeLayer {
                    order: 0,
                    revision: r.id.clone(),
                    opacity: 0.5,
                }],
            },
        )
        .unwrap();
        let dir = Path::new(reply["result"].as_str().unwrap());
        let out = decode(&fs::read(dir.join("composite.review-only.png")).unwrap()).unwrap();
        assert_eq!(out.get_pixel(7, 5)[3], 128);
        assert_eq!(
            checked_bytes(root, &source.path, &source.sha256).unwrap(),
            fs::read(root.join("custom-0.png")).unwrap()
        );
        assert_eq!(hash(&fs::read(root.join(&r.path)).unwrap()), r.sha256);
        let m: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("manifest.review-only.json")).unwrap())
                .unwrap();
        assert_eq!(m["layers"][0]["revision"], r.id);
        assert_eq!(m["masterReferenceIncluded"], false);
    }
    #[test]
    fn independent_environment_owner_creation_never_creates_character_project() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir(root.join("projects")).unwrap();
        fs::write(root.join("projects/sentinel.txt"), b"unchanged").unwrap();
        let reply = library_in(
            root,
            LibraryRequest::CreateProject {
                name: "World project".into(),
            },
        )
        .unwrap();
        let id = reply["createdId"].as_str().unwrap();
        assert!(root
            .join("environment-projects")
            .join(id)
            .join("owner.json")
            .exists());
        assert!(!root
            .join("environment-projects")
            .join(id)
            .join("project.json")
            .exists());
        assert_eq!(
            fs::read(root.join("projects/sentinel.txt")).unwrap(),
            b"unchanged"
        );
    }
    #[test]
    #[ignore = "Read-only audit of the existing environment project given by SPRITE_STUDIO_ENV_AUDIT_PROJECT"]
    fn audit_existing_environment_read_only() {
        let root = PathBuf::from(std::env::var("SPRITE_STUDIO_ENV_AUDIT_PROJECT").unwrap());
        let primary = folder(&root).join("index.json");
        let bytes = fs::read(&primary).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let id = v["projectId"].as_str().unwrap();
        let s = load(&root, id).unwrap();
        let mut count = 0;
        for l in &s.layers {
            for src in &l.sources {
                checked_bytes(&root, &src.path, &src.sha256).unwrap();
                count += 1;
            }
            for r in &l.revisions {
                checked_bytes(&root, &r.path, &r.sha256).unwrap();
                count += 1;
            }
        }
        assert_eq!(fs::read(&primary).unwrap(), bytes);
        println!("Read-only legacy audit: {} layers / {count} verified source and revision hashes; metadata unchanged",s.layers.len());
    }
    #[test]
    fn edits_are_isolated_across_projects_and_canvas_change_preserves_independent_props() {
        let tmp = tempdir().unwrap();
        let a_root = tmp.path().join("a");
        let b_root = tmp.path().join("b");
        fs::create_dir_all(&a_root).unwrap();
        fs::create_dir_all(&b_root).unwrap();
        let mut a = custom(&a_root, "a");
        let b = custom(&b_root, "b");
        let b_path = package_folder(&b_root, &b.package_id)
            .unwrap()
            .join("index.json");
        let b_before = fs::read(&b_path).unwrap();
        a.layers[1].asset_type = AssetType::Prop;
        a.layers[1].native_size = Some([3, 4]);
        custom_add(&a_root, &mut a, 0, [8, 6]);
        custom_add(&a_root, &mut a, 1, [3, 4]);
        let prop = current(&a.layers[1]).unwrap().id.clone();
        dispatch_package(
            &a_root,
            "a",
            &a.package_id,
            Request::Configure {
                name: a.display_name.clone(),
                canvas: [16, 12],
                biome: "".into(),
                arena: "".into(),
                palette: a.palette.clone(),
                guides: Guides::default(),
            },
        )
        .unwrap();
        let changed = load_package(&a_root, "a", &a.package_id).unwrap();
        assert!(changed.layers[0].current_revision_id.is_none());
        assert_eq!(current(&changed.layers[1]).unwrap().id, prop);
        assert_eq!(fs::read(&b_path).unwrap(), b_before);
        let history =
            dispatch_package(&a_root, "a", &a.package_id, Request::PackageHistory).unwrap();
        assert!(!history["metadataHistory"].as_array().unwrap().is_empty());
    }
    #[test]
    #[ignore = "Writes a synthetic demo to the explicitly supplied new SPRITE_STUDIO_STATIC_DEMO_DIR"]
    fn write_demo() {
        let root = PathBuf::from(
            std::env::var("SPRITE_STUDIO_STATIC_DEMO_DIR").expect("Set a NEW demo workspace path"),
        );
        fs::create_dir(&root)
            .expect("Demo folder must not already exist; will not overwrite workspace");
        let (dir, id) = crate::studio::create_static_demo_project(&root).unwrap();
        let mut s = Environment::new(&id);
        for order in 0..6 {
            add(&dir, &mut s, order);
            approve(
                &dir,
                &mut s,
                order,
                "Synthetic test approval only; not production artwork",
            )
            .unwrap();
            export_layer(&dir, &mut s, order).unwrap();
        }
        dispatch(
            &dir,
            &id,
            Request::Composite {
                visible: vec![0, 1, 2, 3, 4, 5],
                export: true,
            },
        )
        .unwrap();
        let reopened = load(&dir, &id).unwrap();
        assert_eq!(
            serde_json::to_value(s).unwrap(),
            serde_json::to_value(reopened).unwrap()
        );
        crate::studio::read_project(&dir).unwrap();
        println!("DEMO WORKSPACE: {}", root.display());
        println!("DEMO PROJECT: {}", dir.display());
    }
}
