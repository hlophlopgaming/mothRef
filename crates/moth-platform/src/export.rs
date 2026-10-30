// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded, worker-only exports. Destinations are created exclusively, never overwritten.
//! shortcut: scene/clipboard export and sampling use previews; use tiled decoding for original detail.
use crate::DesktopStorage;
use base64::{Engine, engine::general_purpose::STANDARD};
use moth_format::{Handle, Storage};
use moth_scene::{BlobId, Item, Rect, Scene, Uuid};
use std::{
    collections::HashMap,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

/// Original-backed resets derive geometry on the storage worker, never in a frame.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Geometry {
    /// Restore a frame to original pixel dimensions.
    Scale,
    /// Match the reference frame's pixels-per-source-pixel ratio.
    Normalize,
    /// Restore original aspect and remove content cropping at the current width.
    Crop,
    /// Restore original dimensions and all geometric transforms, retaining appearance.
    All,
}
/// Clipboard representation selected independently from the sampled image appearance.
#[derive(Clone, Copy, Default)]
pub enum ColorFormat {
    /// Standard hexadecimal RGB triplet.
    #[default]
    Hex,
    /// RGB channel values in the range 0–255.
    Rgb,
    /// Hue in degrees, saturation and value in percent.
    Hsv,
}
/// Export choices share the storage worker's authoritative scene and original handles.
pub enum Target {
    /// Composited scene or selected items; an optional world rectangle crops the output.
    Scene {
        /// Destination filename, created exclusively.
        path: PathBuf,
        /// Empty means the whole scene; otherwise export these stable item identities.
        ids: Vec<Uuid>,
        /// Optional world-space crop, independent of item selection.
        region: Option<Rect>,
        /// Requested longest side, clamped to the raster budget.
        side: u32,
    },
    /// Preserve an image's original encoded bytes.
    Original {
        /// Destination filename, created exclusively.
        path: PathBuf,
        /// Image whose original bytes should be preserved.
        id: Uuid,
    },
    /// Open a bounded, verified copy in the system image viewer.
    Viewer(Uuid),
    /// Copy an image as RGBA through the native clipboard backend.
    Clipboard(Uuid),
    /// Write deduplicated original images into a new subdirectory.
    Originals(PathBuf),
    /// A streaming ZIP of original images; empty identities means all images.
    Archive {
        /// Destination created exclusively.
        path: PathBuf,
        /// Stable selected identities, or empty for the board.
        ids: Vec<Uuid>,
    },
    /// Original-pixel coordinates and metadata at a world point.
    Info {
        /// Image underneath the chosen point.
        id: Uuid,
        /// World position mapped back through the content transform.
        point: [f32; 2],
    },
    /// Sample a world point through the image transform, copying its HEX value.
    Sample {
        /// Clipboard representation of the sampled value.
        format: ColorFormat,
        /// Image underneath the chosen point.
        id: Uuid,
        /// World position before the frame/content transforms are undone.
        point: [f32; 2],
    },
}
/// A single slot bounds viewer copies to one original per board (MAX_RECORD bytes).
fn viewer_image(
    storage: &DesktopStorage,
    sources: &HashMap<BlobId, Handle>,
    item: &Item,
) -> io::Result<PathBuf> {
    let hash = item.blob().ok_or_else(|| error("Select an image"))?;
    let handle = sources
        .get(&hash)
        .ok_or_else(|| error("Image source unavailable"))?;
    let mut header = [0u8; 16];
    let count = storage.open(*handle)?.read(&mut header)?;
    let extension = extension(&header[..count]);
    if extension == "mothrgba" {
        return Err(error(
            "This original cannot be opened in a system viewer; export it as PNG",
        ));
    }
    let directory = storage.recovery_directory()?.join("viewer");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!("{hash}.{extension}"));
    // Recreate the slot so a viewer's edits can never masquerade as the board original.
    for entry in std::fs::read_dir(&directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            std::fs::remove_file(entry.path())?;
        }
    }
    if let Err(error) = copy_original(storage, sources, item, &path) {
        let _ = std::fs::remove_file(&path);
        return Err(error);
    }
    Ok(path)
}
fn color(p: [u8; 3]) -> (String, String, String) {
    let hex = format!("#{:02X}{:02X}{:02X}", p[0], p[1], p[2]);

    let rgb = [p[0] as f32 / 255., p[1] as f32 / 255., p[2] as f32 / 255.];
    let max = rgb.into_iter().fold(0f32, f32::max);
    let min = rgb.into_iter().fold(1f32, f32::min);
    let d = max - min;
    let hue = if d == 0. {
        0.
    } else if max == rgb[0] {
        60. * ((rgb[1] - rgb[2]) / d).rem_euclid(6.)
    } else if max == rgb[1] {
        60. * ((rgb[2] - rgb[0]) / d + 2.)
    } else {
        60. * ((rgb[0] - rgb[1]) / d + 4.)
    };
    let rgb = format!("RGB {}, {}, {}", p[0], p[1], p[2]);
    let hsv = format!(
        "HSV {:.0}°, {:.0}%, {:.0}%",
        hue,
        if max == 0. { 0. } else { 100. * d / max },
        100. * max
    );
    (hex, rgb, hsv)
}
fn error(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}
fn original(
    storage: &DesktopStorage,
    sources: &HashMap<BlobId, Handle>,
    item: &Item,
) -> io::Result<Vec<u8>> {
    let hash = item.blob().ok_or_else(|| error("Select an image"))?;
    let handle = sources
        .get(&hash)
        .ok_or_else(|| error("Image source unavailable"))?;
    let mut bytes = Vec::new();
    storage
        .open(*handle)?
        .take(moth_io::MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > moth_io::MAX_INPUT_BYTES || blake3::hash(&bytes) != hash {
        return Err(error("Image exceeds export budget or source changed"));
    }
    Ok(bytes)
}
fn copy_original(
    storage: &DesktopStorage,
    sources: &HashMap<BlobId, Handle>,
    item: &Item,
    path: &Path,
) -> io::Result<()> {
    let hash = item.blob().ok_or_else(|| error("Select an image"))?;
    let handle = sources
        .get(&hash)
        .ok_or_else(|| error("Image source unavailable"))?;
    let mut source = storage.open(*handle)?;
    if source.seek(SeekFrom::End(0))? > moth_format::board::MAX_RECORD {
        return Err(error("Original exceeds export budget"));
    }
    source.seek(SeekFrom::Start(0))?;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let mut buffer = [0u8; 64 * 1024];
    let mut hash_check = blake3::Hasher::new();
    let mut total = 0u64;
    loop {
        let count = source.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > moth_format::board::MAX_RECORD {
            return Err(error("Original exceeds export budget"));
        }
        hash_check.update(&buffer[..count]);
        output.write_all(&buffer[..count])?;
    }
    if hash_check.finalize() != hash {
        return Err(error("Image source changed"));
    }
    output.sync_all()
}
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn item(scene: &Scene, id: Uuid) -> io::Result<&Item> {
    scene.get(id).ok_or_else(|| error("Item no longer exists"))
}
pub(super) fn image_geometry(
    storage: &DesktopStorage,
    scene: &Scene,
    sources: &HashMap<BlobId, Handle>,
    ids: &[Uuid],
    mode: Geometry,
) -> io::Result<Vec<moth_scene::Command>> {
    let mut scale = None;
    let mut commands = Vec::new();
    for id in ids {
        let item = item(scene, *id)?;
        if let Some(note) = item.note() {
            if mode == Geometry::All {
                let size = moth_scene::note_size(note);
                let rect = Rect {
                    width: size[0],
                    height: size[1],
                    ..item.rect
                };
                if !rect.valid() {
                    return Err(error("Note dimensions exceed scene limits"));
                }
                commands.push(moth_scene::Command::SetRect { id: *id, rect });
                commands.push(moth_scene::Command::SetTransform {
                    id: *id,
                    transform: moth_scene::Transform {
                        note_font_size: moth_scene::NOTE_FONT_SIZE,
                        note_wrap: false,
                        ..item.transform.reset_geometry()
                    },
                });
            }
            continue;
        }
        let bytes = original(storage, sources, item)?;
        let dimensions = std::panic::catch_unwind(|| moth_io::decode(&bytes, 1))
            .map_err(|_| error("Image decoder panicked"))?
            .map_err(error)?
            .original;
        let factor = match mode {
            Geometry::Normalize => *scale.get_or_insert(item.rect.width / dimensions[0] as f32),
            Geometry::Crop => item.rect.width / dimensions[0] as f32,
            _ => 1.,
        };
        let rect = Rect {
            width: dimensions[0] as f32 * factor,
            height: dimensions[1] as f32 * factor,
            ..item.rect
        };
        if !rect.valid() {
            return Err(error("Original dimensions exceed scene limits"));
        }
        commands.push(moth_scene::Command::SetRect { id: *id, rect });
        if matches!(mode, Geometry::Crop | Geometry::All) {
            let mut transform = item.transform;
            transform.content_scale = 1.;
            transform.content_offset = [0.; 2];
            transform.content_rotation = 0.;
            if mode == Geometry::All {
                transform = transform.reset_geometry();
            }
            commands.push(moth_scene::Command::SetTransform { id: *id, transform });
        }
    }
    Ok(commands)
}
/// Runs inside a panic boundary on the serial storage worker, with bounded raster output.
pub(super) fn run(
    storage: &DesktopStorage,
    scene: &Scene,
    sources: &HashMap<BlobId, Handle>,
    target: Target,
    clipboard: &mut Option<arboard::Clipboard>,
) -> io::Result<String> {
    match target {
        Target::Viewer(id) => {
            use crate::Platform;
            let item = scene
                .get(id)
                .ok_or_else(|| error("Image no longer exists"))?;
            let path = viewer_image(storage, sources, item)?;
            crate::Desktop.open_image(&path)?;
            return Ok("Image opened in system viewer".into());
        }
        Target::Archive { path, ids } => {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            let mut zip = zip::ZipWriter::new(file);
            let mut seen = std::collections::HashSet::new();
            for item in scene
                .items()
                .filter(|i| ids.is_empty() || ids.contains(&i.id))
            {
                let Some(hash) = item.blob() else {
                    continue;
                };
                if !seen.insert(hash) {
                    continue;
                }
                let handle = sources
                    .get(&hash)
                    .ok_or_else(|| error("Image source unavailable"))?;
                let mut source = storage.open(*handle)?;
                let length = source.seek(SeekFrom::End(0))?;
                if length > moth_format::board::MAX_RECORD {
                    return Err(error("Original exceeds export budget"));
                }
                source.seek(SeekFrom::Start(0))?;
                let mut header = [0u8; 16];
                let count = source.read(&mut header)?;
                source.seek(SeekFrom::Start(0))?;
                zip.start_file(
                    format!("{hash}.{}", extension(&header[..count])),
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored),
                )
                .map_err(error)?;
                let mut hasher = blake3::Hasher::new();
                let mut buffer = [0u8; 64 * 1024];
                let mut total = 0;
                loop {
                    let n = source.read(&mut buffer)?;
                    if n == 0 {
                        break;
                    }
                    total += n as u64;
                    if total > moth_format::board::MAX_RECORD {
                        return Err(error("Original exceeds export budget"));
                    }
                    hasher.update(&buffer[..n]);
                    zip.write_all(&buffer[..n])?;
                }
                if hasher.finalize() != hash {
                    return Err(error("Image source changed"));
                }
            }
            zip.finish().map_err(error)?.sync_all()?;
        }
        Target::Info { id, point } => {
            let item = item(scene, id)?;
            let bytes = original(storage, sources, item)?;
            let decoded = moth_io::decode(&bytes, 1).map_err(error)?;
            let uv = item
                .source_uv(point, decoded.original)
                .ok_or_else(|| error("No image pixel at this position"))?;
            return Ok(format!(
                "{} · X {} Y {} · {} × {}",
                item.name,
                (uv[0] * decoded.original[0] as f32)
                    .floor()
                    .min((decoded.original[0] - 1) as f32),
                (uv[1] * decoded.original[1] as f32)
                    .floor()
                    .min((decoded.original[1] - 1) as f32),
                decoded.original[0],
                decoded.original[1]
            ));
        }
        Target::Original { path, id } => {
            copy_original(storage, sources, item(scene, id)?, &path)?;
        }
        Target::Originals(directory) => {
            // A fresh directory makes collisions and partial exports visible without replacing files.
            std::fs::create_dir(&directory)?;
            let mut seen = std::collections::HashSet::new();
            for item in scene.items() {
                if let Some(hash) = item.blob()
                    && seen.insert(hash)
                {
                    let handle = sources
                        .get(&hash)
                        .ok_or_else(|| error("Image source unavailable"))?;
                    let mut header = [0u8; 16];
                    let count = storage.open(*handle)?.read(&mut header)?;
                    let extension = extension(&header[..count]);
                    copy_original(
                        storage,
                        sources,
                        item,
                        &directory.join(format!("{hash}.{extension}")),
                    )?;
                }
            }
        }
        Target::Clipboard(id) | Target::Sample { id, .. } => {
            let bytes = original(storage, sources, item(scene, id)?)?;
            let decoded = moth_io::decode(&bytes, moth_io::MAX_SIDE).map_err(error)?;
            let mip = decoded
                .mips
                .first()
                .ok_or_else(|| error("No image pixels"))?;
            if clipboard.is_none() {
                *clipboard = Some(arboard::Clipboard::new().map_err(error)?);
            }
            let clipboard = clipboard
                .as_mut()
                .ok_or_else(|| error("Clipboard unavailable"))?;
            if let Target::Sample { point, format, .. } = target {
                let uv = item(scene, id)?
                    .source_uv(point, decoded.original)
                    .ok_or_else(|| error("No image pixel at this position"))?;
                if !uv.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)) {
                    return Err(error("Invalid sample position"));
                }
                let x = (uv[0] * mip.width as f32) as u32;
                let y = (uv[1] * mip.height as f32) as u32;
                let offset =
                    ((y.min(mip.height - 1) * mip.width + x.min(mip.width - 1)) * 4) as usize;
                let p = &mip.bytes[offset..offset + 3];
                let (hex, rgb, hsv) = color([p[0], p[1], p[2]]);
                clipboard
                    .set_text(match format {
                        ColorFormat::Hex => hex.clone(),
                        ColorFormat::Rgb => rgb.clone(),
                        ColorFormat::Hsv => hsv.clone(),
                    })
                    .map_err(error)?;
                return Ok(format!("{hex} · {rgb} · {hsv}"));
            }
            clipboard
                .set_image(arboard::ImageData {
                    width: mip.width as usize,
                    height: mip.height as usize,
                    bytes: std::borrow::Cow::Borrowed(&mip.bytes),
                })
                .map_err(error)?;
        }
        Target::Scene {
            path,
            ids,
            region,
            side,
        } => {
            let mut items: Vec<_> = if ids.is_empty() {
                scene.items().collect()
            } else {
                ids.iter()
                    .map(|id| item(scene, *id))
                    .collect::<io::Result<_>>()?
            };
            items.sort_unstable_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
            let bounds = bounds(&items, region)?;
            let svg = svg(storage, sources, &items, bounds, side)?;
            if path
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("svg"))
            {
                write_new(&path, svg.as_bytes())?;
            } else if path
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("png") || s.eq_ignore_ascii_case("pdf"))
            {
                let mut options = resvg::usvg::Options::default();
                options.fontdb_mut().load_font_data(
                    include_bytes!("../../moth-ui/fonts/NotoSans-Regular.ttf").to_vec(),
                );
                let tree = resvg::usvg::Tree::from_str(&svg, &options).map_err(error)?;
                let size = tree.size().to_int_size();
                let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
                    .ok_or_else(|| error("Export allocation failed"))?;
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::identity(),
                    &mut pixmap.as_mut(),
                );
                if path
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("pdf"))
                {
                    pdf(&path, &pixmap)?;
                } else {
                    write_new(&path, &pixmap.encode_png().map_err(error)?)?;
                }
            } else {
                return Err(error("Choose a .png, .svg or .pdf filename"));
            }
        }
    }
    Ok(String::new())
}
// A single raster page shares the PNG/SVG bounds and raster budget.
fn pdf(path: &Path, pixmap: &resvg::tiny_skia::Pixmap) -> io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n")?;
    let (w, h) = (pixmap.width(), pixmap.height());
    let draw = format!("q {w} 0 0 {h} 0 0 cm /Im0 Do Q\n");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] /Resources << /XObject << /Im0 5 0 R >> >> /Contents 4 0 R >>"
        ),
        format!("<< /Length {} >>\nstream\n{draw}endstream", draw.len()),
    ];
    let mut offsets = vec![0];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(file.stream_position()?);
        writeln!(file, "{} 0 obj\n{object}\nendobj", index + 1)?;
    }
    offsets.push(file.stream_position()?);
    writeln!(
        file,
        "5 0 obj\n<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length {} >>\nstream",
        u64::from(w) * u64::from(h) * 3
    )?;
    let mut row = Vec::with_capacity(w as usize * 3);
    for pixels in pixmap.data().chunks_exact(w as usize * 4) {
        row.clear();
        for p in pixels.as_chunks::<4>().0 {
            for channel in &p[..3] {
                row.push(channel.saturating_add(255 - p[3]));
            }
        }
        file.write_all(&row)?;
    }
    file.write_all(b"\nendstream\nendobj\n")?;
    let xref = file.stream_position()?;
    writeln!(file, "xref\n0 6\n0000000000 65535 f ")?;
    for offset in &offsets[1..] {
        writeln!(file, "{offset:010} 00000 n ")?;
    }
    write!(
        file,
        "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    )?;
    file.sync_all()
}
fn extension(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG") {
        "png"
    } else if bytes.starts_with(b"\xff\xd8") {
        "jpg"
    } else if bytes.starts_with(b"GIF8") {
        "gif"
    } else if bytes.starts_with(b"BM") {
        "bmp"
    } else if bytes.starts_with(b"RIFF") {
        "webp"
    } else {
        "mothrgba"
    }
}
fn bounds(items: &[&Item], region: Option<Rect>) -> io::Result<Rect> {
    if let Some(rect) = region {
        return rect
            .valid()
            .then_some(rect)
            .ok_or_else(|| error("Invalid export region"));
    }
    let first = items.first().ok_or_else(|| error("Board is empty"))?;
    let mut min = first
        .rect
        .rotated_envelope(first.transform.rotation)
        .lower();
    let mut max = first
        .rect
        .rotated_envelope(first.transform.rotation)
        .upper();
    for item in items {
        let b = item.rect.rotated_envelope(item.transform.rotation);
        for i in 0..2 {
            min[i] = min[i].min(b.lower()[i]);
            max[i] = max[i].max(b.upper()[i]);
        }
    }
    let rect = Rect {
        x: min[0],
        y: min[1],
        width: max[0] - min[0],
        height: max[1] - min[1],
    };
    rect.valid()
        .then_some(rect)
        .ok_or_else(|| error("Export bounds exceed budget"))
}
fn note_export_lines(
    text: &str,
    size: f32,
    width: f32,
    options: &resvg::usvg::Options,
) -> io::Result<Vec<String>> {
    let measure = |line: &str| -> io::Result<f32> {
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><text xml:space="preserve" y="{size}" font-family="Noto Sans" font-size="{size}">{}</text></svg>"#,
            escape(line)
        );
        let tree = resvg::usvg::Tree::from_str(&source, options).map_err(error)?;
        Ok(tree.root().abs_bounding_box().width())
    };
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let ends: Vec<_> = paragraph
            .char_indices()
            .map(|(i, c)| i + c.len_utf8())
            .collect();
        let mut start = 0;
        while start < ends.len() {
            let begin = if start == 0 { 0 } else { ends[start - 1] };
            // Exponential probing bounds shaping work even for a 32 KiB line in a narrow frame.
            let mut fit = start + 1;
            let mut high = fit;
            while high < ends.len() && measure(&paragraph[begin..ends[high - 1]])? <= width {
                fit = high;
                high = (start + (high - start) * 2).min(ends.len());
            }
            if measure(&paragraph[begin..ends[high - 1]])? <= width {
                fit = high;
            } else {
                while high > fit + 1 {
                    let middle = fit + (high - fit) / 2;
                    if measure(&paragraph[begin..ends[middle - 1]])? <= width {
                        fit = middle;
                    } else {
                        high = middle;
                    }
                }
            }
            if fit < ends.len()
                && let Some(boundary) = (start..fit).rev().find(|&i| {
                    paragraph[..ends[i]]
                        .chars()
                        .next_back()
                        .is_some_and(char::is_whitespace)
                })
            {
                fit = boundary + 1;
            }
            lines.push(paragraph[begin..ends[fit - 1]].to_owned());
            start = fit;
        }
    }
    Ok(lines)
}
/// Group backdrop colors by palette code, the dark theme's swatches.
const GROUP_SWATCHES: [&str; moth_scene::GROUP_COLORS as usize] = [
    "#9a9f94", "#a6c47a", "#e0b46c", "#8fb8de", "#e58fae", "#b4a0d8",
];
fn svg(
    storage: &DesktopStorage,
    sources: &HashMap<BlobId, Handle>,
    items: &[&Item],
    bounds: Rect,
    side: u32,
) -> io::Result<String> {
    let scale = side.clamp(1, 4096) as f32 / bounds.width.max(bounds.height);
    let width = (bounds.width * scale).ceil().max(1.) as u32;
    let height = (bounds.height * scale).ceil().max(1.) as u32;
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{width}" height="{height}" viewBox="{} {} {} {}"><defs><filter id="gray" color-interpolation-filters="linearRGB"><feColorMatrix type="saturate" values="0"/></filter></defs>"#,
        bounds.x, bounds.y, bounds.width, bounds.height
    );
    let mut note_options = resvg::usvg::Options::default();
    if items
        .iter()
        .any(|item| item.note().is_some() && item.transform.note_wrap)
    {
        note_options
            .fontdb_mut()
            .load_font_data(include_bytes!("../../moth-ui/fonts/NotoSans-Regular.ttf").to_vec());
    }
    // Backdrops go first, outer groups before inner ones, so no tint covers an image.
    let mut groups: Vec<_> = items.iter().filter(|item| item.is_group()).collect();
    groups
        .sort_by(|a, b| (b.rect.width * b.rect.height).total_cmp(&(a.rect.width * a.rect.height)));
    for item in groups {
        let (Some(style), r) = (item.group_style(), item.rect) else {
            continue;
        };
        let swatch = GROUP_SWATCHES[style.color as usize % GROUP_SWATCHES.len()];
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="14" fill="{swatch}" fill-opacity="0.1" stroke="{swatch}" stroke-opacity="0.4"/>"##,
            r.x, r.y, r.width, r.height
        ));
        if !style.title.is_empty() {
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{swatch}" font-family="Noto Sans" font-size="16">{}</text>"##,
                r.x + 24.,
                r.y + 24.,
                escape(&style.title)
            ));
        }
    }
    for (index, item) in items.iter().filter(|item| !item.is_group()).enumerate() {
        let r = item.rect;
        let t = item.transform;
        let (cx, cy) = (r.x + r.width / 2., r.y + r.height / 2.);
        svg.push_str(&format!(r#"<g opacity="{}" transform="translate({cx} {cy}) rotate({})"><defs><clipPath id="c{index}"><rect x="{}" y="{}" width="{}" height="{}"/></clipPath></defs><g clip-path="url(#c{index})">"#,t.opacity,t.rotation.to_degrees(),-r.width/2.,-r.height/2.,r.width,r.height));
        if item.blob().is_some() {
            let bytes = original(storage, sources, item)?;
            let decoded = moth_io::decode(&bytes, moth_io::MAX_SIDE).map_err(error)?;
            let mip = decoded
                .mips
                .first()
                .ok_or_else(|| error("No image pixels"))?;
            let mut png = Vec::new();
            moth_io::write_png(&mut png, mip.width, mip.height, &mip.bytes).map_err(error)?;
            let fit = (r.width / decoded.original[0] as f32)
                .max(r.height / decoded.original[1] as f32)
                * t.content_scale;
            let (w, h) = (
                decoded.original[0] as f32 * fit,
                decoded.original[1] as f32 * fit,
            );
            svg.push_str(&format!(r#"<g transform="rotate({}) translate({} {}) scale({} {})"><image x="{}" y="{}" width="{w}" height="{h}" image-rendering="{}" {} xlink:href="data:image/png;base64,{}"/></g>"#,t.content_rotation.to_degrees(),t.content_offset[0]*r.width,t.content_offset[1]*r.height,if t.flip[0]{-1}else{1},if t.flip[1]{-1}else{1},-w/2.,-h/2.,if t.sampling.nearest(moth_scene::Sampling::Smooth, fit * scale){"optimizeSpeed"}else{"optimizeQuality"},if t.grayscale{r#"filter="url(#gray)""#}else{""},STANDARD.encode(png)));
        } else if let Some(text) = item.note() {
            let size = item.note_font_size();
            let lines: Vec<String> = if t.note_wrap {
                note_export_lines(text, size, r.width, &note_options)?
            } else {
                text.split('\n').map(str::to_owned).collect()
            };
            svg.push_str(&format!(r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#272a32" fill-opacity="0.9215686" stroke="#30343c"/><text fill="#e0e2e7" font-family="Noto Sans" font-size="{size}">"##,-r.width/2.,-r.height/2.,r.width,r.height));
            for (row, line) in lines.iter().enumerate() {
                svg.push_str(&format!(
                    r#"<tspan x="{}" y="{}">{}</tspan>"#,
                    -r.width / 2.,
                    -r.height / 2. + size * (1. + row as f32 * 1.3),
                    escape(line)
                ));
            }
            svg.push_str("</text>");
        }
        svg.push_str("</g></g>");
        if svg.len() > 64 * 1024 * 1024 {
            return Err(error("SVG exceeds 64 MiB export budget"));
        }
    }
    svg.push_str("</svg>");
    Ok(svg)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn viewer_copy_preserves_original_replaces_edited_copy_and_bounds_slot() {
        let root = std::env::temp_dir().join(format!("moth-viewer-{}", Uuid::now_v7()));
        let storage = DesktopStorage::in_directory(root.clone());
        let handle = storage.stage_rgba(2, 1, &[255; 8]).unwrap();
        let mut bytes = Vec::new();
        storage
            .open(handle)
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        let hash = blake3::hash(&bytes);
        let item = Item::image(
            hash,
            Rect {
                x: 0.,
                y: 0.,
                width: 2.,
                height: 1.,
            },
            None,
        );
        let sources = HashMap::from([(hash, handle)]);
        let path = viewer_image(&storage, &sources, &item).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::write(&path, b"viewer edits").unwrap();
        assert_eq!(viewer_image(&storage, &sources, &item).unwrap(), path);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let mut original = Vec::new();
        storage
            .open(handle)
            .unwrap()
            .read_to_end(&mut original)
            .unwrap();
        assert_eq!(original, bytes);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
        let wrong = Item::image(blake3::hash(b"wrong"), item.rect, None);
        assert!(
            viewer_image(
                &storage,
                &HashMap::from([(wrong.blob().unwrap(), handle)]),
                &wrong
            )
            .is_err()
        );
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            0
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn wrapped_note_export_uses_fixed_font_and_preserves_unicode_text() {
        let text = "one two three four пять шесть\nlast line";
        let mut options = resvg::usvg::Options::default();
        options
            .fontdb_mut()
            .load_font_data(include_bytes!("../../moth-ui/fonts/NotoSans-Regular.ttf").to_vec());
        let lines = note_export_lines(text, 24., 100., &options).unwrap();
        assert!(lines.len() > 2);
        assert_eq!(lines.concat(), text.replace('\n', ""));
        let narrow = note_export_lines("Русский", 24., 0.001, &options).unwrap();
        assert_eq!(narrow.len(), 7);
        assert_eq!(narrow.concat(), "Русский");
        let empty = note_export_lines("\n", 24., 100., &options).unwrap();
        assert_eq!(empty, ["", ""]);
    }
    #[test]
    fn color_formats_cover_hue_gray_and_black() {
        assert_eq!(
            color([255, 0, 0]),
            (
                "#FF0000".into(),
                "RGB 255, 0, 0".into(),
                "HSV 0°, 100%, 100%".into()
            )
        );
        assert_eq!(color([0, 255, 0]).2, "HSV 120°, 100%, 100%");
        assert_eq!(color([0, 0, 255]).2, "HSV 240°, 100%, 100%");
        assert_eq!(color([128; 3]).2, "HSV 0°, 0%, 50%");
        assert_eq!(color([0; 3]).2, "HSV 0°, 0%, 0%");
    }
    #[test]
    fn crop_and_all_resets_restore_original_geometry_and_preserve_appearance() {
        let root = std::env::temp_dir().join(format!("moth-reset-original-{}", Uuid::now_v7()));
        let storage = DesktopStorage::in_directory(root.clone());
        let (handle, image) = storage.stage_preview(2, 1, &[255; 8]).unwrap();
        let mut item = Item::image(
            image.hash,
            Rect {
                x: 10.,
                y: 20.,
                width: 20.,
                height: 20.,
            },
            None,
        );
        item.transform = moth_scene::Transform {
            rotation: 1.,
            flip: [true, false],
            content_scale: 2.,
            content_offset: [0.2, 0.3],
            content_rotation: 0.5,
            opacity: 0.4,
            grayscale: true,
            ..Default::default()
        };
        let id = item.id;
        let mut scene = Scene::default();
        scene.apply(moth_scene::Command::Insert(item)).unwrap();
        let sources = HashMap::from([(image.hash, handle)]);
        for mode in [Geometry::Crop, Geometry::All] {
            let commands = image_geometry(&storage, &scene, &sources, &[id], mode).unwrap();
            let mut undo = Vec::new();
            for c in commands {
                undo.push(scene.apply(c).unwrap());
            }
            let item = scene.get(id).unwrap();
            assert_eq!(item.rect.width / item.rect.height, 2.);
            assert_eq!(item.transform.content_scale, 1.);
            assert_eq!(item.transform.content_offset, [0.; 2]);
            assert_eq!(item.transform.opacity, 0.4);
            assert!(item.transform.grayscale);
            if mode == Geometry::All {
                assert_eq!(item.rect.width, 2.);
                assert_eq!(item.transform.rotation, 0.);
                assert_eq!(item.transform.flip, [false; 2]);
            } else {
                assert_eq!(item.rect.width, 20.);
                assert_eq!(item.transform.rotation, 1.);
            }
            for c in undo.into_iter().rev() {
                scene.apply(c).unwrap();
            }
            assert_eq!(scene.get(id).unwrap().rect.height, 20.);
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn pdf_offsets_archive_crc_and_original_scale() {
        let root = std::env::temp_dir().join(format!("moth-export-variants-{}", Uuid::now_v7()));
        std::fs::create_dir_all(&root).unwrap();
        let storage = DesktopStorage::in_directory(root.clone());
        let (handle, image) = storage
            .stage_preview(2, 1, &[255, 0, 0, 255, 0, 255, 0, 255])
            .unwrap();
        let item = Item::image(
            image.hash,
            Rect {
                x: 0.,
                y: 0.,
                width: 20.,
                height: 10.,
            },
            None,
        );
        let id = item.id;
        let mut scene = Scene::default();
        scene.apply(moth_scene::Command::Insert(item)).unwrap();
        let sources = HashMap::from([(image.hash, handle)]);
        let path = root.join("board.pdf");
        run(
            &storage,
            &scene,
            &sources,
            Target::Scene {
                path: path.clone(),
                ids: vec![id],
                region: None,
                side: 2,
            },
            &mut None,
        )
        .unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("/MediaBox [0 0 2 1]"));
        let xref = text
            .rsplit_once("startxref\n")
            .unwrap()
            .1
            .lines()
            .next()
            .unwrap()
            .parse::<usize>()
            .unwrap();
        assert!(bytes[xref..].starts_with(b"xref\n0 6\n"));
        let offsets = std::str::from_utf8(&bytes[xref..])
            .unwrap()
            .lines()
            .skip(3)
            .take(5);
        for (i, line) in offsets.enumerate() {
            let offset = line[..10].parse::<usize>().unwrap();
            assert!(bytes[offset..].starts_with(format!("{} 0 obj", i + 1).as_bytes()));
        }
        assert!(bytes.windows(6).any(|w| w == [255, 0, 0, 0, 255, 0]));
        assert!(
            run(
                &storage,
                &scene,
                &sources,
                Target::Scene {
                    path: path.clone(),
                    ids: vec![],
                    region: None,
                    side: 2
                },
                &mut None
            )
            .is_err()
        );
        let path = root.join("originals.zip");
        run(
            &storage,
            &scene,
            &sources,
            Target::Archive {
                path: path.clone(),
                ids: vec![id],
            },
            &mut None,
        )
        .unwrap();
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        assert_eq!(archive.len(), 1);
        let mut original_bytes = Vec::new();
        archive
            .by_index(0)
            .unwrap()
            .read_to_end(&mut original_bytes)
            .unwrap();
        assert_eq!(blake3::hash(&original_bytes), image.hash);
        let commands = image_geometry(&storage, &scene, &sources, &[id], Geometry::Scale).unwrap();
        assert!(
            matches!(commands[0],moth_scene::Command::SetRect{rect,..} if rect.width==2.&&rect.height==1.)
        );
        let info = run(
            &storage,
            &scene,
            &sources,
            Target::Info {
                id,
                point: [15., 5.],
            },
            &mut None,
        )
        .unwrap();
        assert!(info.contains("X 1 Y 0"));
        assert!(info.contains("2 × 1"));
        drop(archive);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn scene_region_svg_and_original_exports_preserve_content() {
        let root = std::env::temp_dir().join(format!("moth-export-scene-{}", Uuid::now_v7()));
        std::fs::create_dir(&root).unwrap();
        let storage = DesktopStorage::in_directory(root.clone());
        let (handle, decoded) = storage
            .stage_preview(2, 1, &[255, 0, 0, 255, 0, 255, 0, 255])
            .unwrap();
        let image = Item::image(
            decoded.hash,
            Rect {
                x: 0.,
                y: 0.,
                width: 2.,
                height: 1.,
            },
            None,
        );
        let id = image.id;
        let mut scene = Scene::default();
        scene.apply(moth_scene::Command::Insert(image)).unwrap();
        let sources = HashMap::from([(decoded.hash, handle)]);
        let mut clipboard = None;
        let png = root.join("scene.png");
        run(
            &storage,
            &scene,
            &sources,
            Target::Scene {
                path: png.clone(),
                ids: vec![id],
                region: None,
                side: 2,
            },
            &mut clipboard,
        )
        .unwrap();
        let rendered = moth_io::decode(&std::fs::read(&png).unwrap(), 2).unwrap();
        assert_eq!(rendered.original, [2, 1]);
        assert_eq!(rendered.mips[0].bytes, [255, 0, 0, 255, 0, 255, 0, 255]);
        let crop = root.join("crop.png");
        run(
            &storage,
            &scene,
            &sources,
            Target::Scene {
                path: crop.clone(),
                ids: vec![],
                region: Some(Rect {
                    x: 1.,
                    y: 0.,
                    width: 1.,
                    height: 1.,
                }),
                side: 1,
            },
            &mut clipboard,
        )
        .unwrap();
        let rendered = moth_io::decode(&std::fs::read(crop).unwrap(), 1).unwrap();
        assert_eq!(rendered.mips[0].bytes, [0, 255, 0, 255]);
        let svg_path = root.join("scene.svg");
        run(
            &storage,
            &scene,
            &sources,
            Target::Scene {
                path: svg_path.clone(),
                ids: vec![],
                region: None,
                side: 2,
            },
            &mut clipboard,
        )
        .unwrap();
        let svg = std::fs::read_to_string(svg_path).unwrap();
        assert!(svg.contains("data:image/png;base64,"));
        assert!(svg.contains("clip-path"));
        let original_path = root.join("original.png");
        run(
            &storage,
            &scene,
            &sources,
            Target::Original {
                path: original_path.clone(),
                id,
            },
            &mut clipboard,
        )
        .unwrap();
        assert_eq!(
            std::fs::read(original_path).unwrap(),
            original(&storage, &sources, scene.get(id).unwrap()).unwrap()
        );
        let directory = root.join("originals");
        run(
            &storage,
            &scene,
            &sources,
            Target::Originals(directory.clone()),
            &mut clipboard,
        )
        .unwrap();
        assert_eq!(std::fs::read_dir(directory).unwrap().count(), 1);
        let mut notes = Scene::default();
        notes
            .apply(moth_scene::Command::Insert(Item::text(
                "<& Привет",
                Rect {
                    x: 0.,
                    y: 0.,
                    width: 160.,
                    height: 31.2,
                },
                None,
            )))
            .unwrap();
        let note_svg = root.join("notes.svg");
        run(
            &storage,
            &notes,
            &HashMap::new(),
            Target::Scene {
                path: note_svg.clone(),
                ids: vec![],
                region: None,
                side: 160,
            },
            &mut clipboard,
        )
        .unwrap();
        assert!(
            std::fs::read_to_string(note_svg)
                .unwrap()
                .contains("&lt;&amp; Привет")
        );
        let note_png = root.join("notes.png");
        run(
            &storage,
            &notes,
            &HashMap::new(),
            Target::Scene {
                path: note_png.clone(),
                ids: vec![],
                region: None,
                side: 160,
            },
            &mut clipboard,
        )
        .unwrap();
        let rendered = moth_io::decode(&std::fs::read(note_png).unwrap(), 160).unwrap();
        assert!(
            rendered.mips[0]
                .bytes
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] > 0)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn escaped_notes_and_exclusive_writes() {
        assert_eq!(escape("<&\""), "&lt;&amp;&quot;");
        let path = std::env::temp_dir().join(format!("moth-export-{}", Uuid::now_v7()));
        write_new(&path, b"original").unwrap();
        assert!(write_new(&path, b"replacement").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        std::fs::remove_file(path).unwrap();
    }
}
