//! Shared existing PNG, hash, confined writes and durable sidecar primitives.
use image::{ImageFormat, Rgba, RgbaImage};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Write},
    path::{Component, Path},
};
use uuid::Uuid;
type Result<T> = std::result::Result<T, String>;
pub(crate) fn checked_bytes(root: &Path, relative: &str, expected: &str) -> Result<Vec<u8>> {
    let p = Path::new(relative);
    if p.as_os_str().is_empty() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err("Unsafe asset path".into());
    }
    let resolved = fs::canonicalize(root.join(p)).map_err(|e| e.to_string())?;
    if !resolved.starts_with(fs::canonicalize(root).map_err(|e| e.to_string())?) {
        return Err("Asset escapes workspace".into());
    }
    let bytes = fs::read(resolved).map_err(|e| e.to_string())?;
    if hash(&bytes) != expected {
        return Err(format!("Hash mismatch: {relative}"));
    }
    Ok(bytes)
}
// Same history/pending/previous rotation as the environment workspace. No generation is deleted.
pub(crate) fn save_metadata(root: &Path, dir: &Path, bytes: &[u8]) -> Result<()> {
    serde_json::from_slice::<serde_json::Value>(bytes).map_err(|e| e.to_string())?;
    write_bound(
        root,
        &dir.join("history").join(format!("{}.json", Uuid::new_v4())),
        bytes,
    )?;
    let previous = dir.join("index.previous.json");
    let pending = dir.join("index.pending.json");
    let primary = dir.join("index.json");
    for p in [&pending, &previous] {
        if p.exists() {
            fs::rename(
                p,
                dir.join("history").join(format!("{}.json", Uuid::new_v4())),
            )
            .map_err(|e| e.to_string())?;
        }
    }
    write_new(&pending, bytes)?;
    if fs::read(&pending).map_err(|e| e.to_string())? != bytes {
        return Err("Metadata verification failed".into());
    }
    if primary.exists() {
        fs::rename(&primary, &previous).map_err(|e| e.to_string())?;
    }
    fs::rename(pending, primary).map_err(|e| e.to_string())?;
    Ok(())
}
pub(crate) fn metadata_candidates(root: &Path, dir: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    secure_parent(root, &dir.join("index.json"))?;
    let mut files = vec![
        dir.join("index.json"),
        dir.join("index.previous.json"),
        dir.join("index.pending.json"),
    ];
    let mut history: Vec<_> = fs::read_dir(dir.join("history"))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .collect();
    history.sort_by_key(|e| std::cmp::Reverse(e.metadata().and_then(|m| m.modified()).ok()));
    files.extend(history.into_iter().take(128).map(|e| e.path()));
    let mut result = vec![];
    for f in files {
        if !f.exists() {
            continue;
        }
        let resolved = fs::canonicalize(&f).map_err(|e| e.to_string())?;
        if !resolved.starts_with(fs::canonicalize(root).map_err(|e| e.to_string())?) {
            return Err("Metadata escapes workspace".into());
        }
        if fs::metadata(&resolved).map_err(|e| e.to_string())?.len() > 16 * 1024 * 1024 {
            continue;
        }
        if let Ok(bytes) = fs::read(&resolved) {
            result.push((f.file_name().unwrap().to_string_lossy().into_owned(), bytes));
        }
    }
    Ok(result)
}
pub(crate) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) fn color(s: &str) -> Result<[u8; 3]> {
    if s.len() != 7 || !s.starts_with('#') || !s.is_ascii() {
        return Err("Use a #RRGGBB background color".into());
    }
    let mut c = [0; 3];
    for (i, item) in c.iter_mut().enumerate() {
        *item =
            u8::from_str_radix(&s[1 + i * 2..3 + i * 2], 16).map_err(|_| "Invalid RGB color")?;
    }
    Ok(c)
}
pub(crate) fn rgb(p: &Rgba<u8>) -> [u8; 3] {
    [p[0], p[1], p[2]]
}
pub(crate) fn distance(a: [u8; 3], b: [u8; 3]) -> u32 {
    (0..3)
        .map(|i| (i32::from(a[i]) - i32::from(b[i])).pow(2) as u32)
        .sum()
}
pub(crate) fn png(image: &RgbaImage) -> Result<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    image
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}
pub(crate) fn decode(bytes: &[u8]) -> Result<RgbaImage> {
    if bytes.len() > 32 * 1024 * 1024
        || image::guess_format(bytes).map_err(|e| e.to_string())? != ImageFormat::Png
    {
        return Err("Import a PNG of at most 32 MiB".into());
    }
    let reader = image::ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png);
    let (w, h) = reader.into_dimensions().map_err(|e| e.to_string())?;
    if w == 0 || h == 0 || w > 4096 || h > 4096 {
        return Err("PNG dimensions must be between 1 and 4096 per axis".into());
    }
    Ok(image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .map_err(|e| e.to_string())?
        .into_rgba8())
}
pub(crate) fn secure_parent(root: &Path, target: &Path) -> Result<()> {
    let base = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let parent = target.parent().ok_or("Missing parent")?;
    let existing = parent
        .ancestors()
        .find(|p| p.exists())
        .ok_or("Missing project root")?;
    if !fs::canonicalize(existing)
        .map_err(|e| e.to_string())?
        .starts_with(&base)
    {
        return Err("Environment output escapes project".into());
    }
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    if !fs::canonicalize(parent)
        .map_err(|e| e.to_string())?
        .starts_with(base)
    {
        return Err("Environment output escapes project".into());
    }
    Ok(())
}
pub(crate) fn write_bound(root: &Path, path: &Path, bytes: &[u8]) -> Result<()> {
    secure_parent(root, path)?;
    write_new(path, bytes)
}
pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or("Missing parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())
}
