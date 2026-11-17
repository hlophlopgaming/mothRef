// SPDX-License-Identifier: GPL-3.0-or-later
//! Static native preview bundles with explicit pixel/board coordinate transforms.
use super::*;
use std::{fs::OpenOptions, path::Path};

fn write_new(path: &Path, value: &Value) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}
fn read_manifest(path: &Path) -> Result<Value> {
    let mut bytes = Vec::new();
    std::fs::File::open(path.join("manifest.json"))?
        .take(MAX_REQUEST + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_REQUEST,
        "preview manifest exceeds budget"
    );
    let mut value: Value = serde_json::from_slice(&bytes)?;
    ensure!(
        value["protocol_version"] == "preview-bundle/v1"
            && value["software"] == "mothref"
            && value["status"] == "ok",
        "not a completed MothRef preview"
    );
    value["_bundle_dir"] = json!(path);
    value["_manifest_path"] = json!(path.join("manifest.json"));
    value["_summary_path"] = json!(path.join("summary.json"));
    Ok(value)
}
pub(super) fn capture(
    session: &mut Session,
    root: PathBuf,
    side: u32,
    dry_run: bool,
) -> Result<Value> {
    ensure!(!dry_run, "preview capture is disabled in dry-run");
    let bounds = session.scene.bounds().context("board is empty")?;
    let root = std::path::absolute(root)?;
    let id = moth_scene::Uuid::now_v7().to_string();
    let path = root.join(&id);
    std::fs::create_dir_all(&root)?;
    std::fs::create_dir(&path)?;
    std::fs::create_dir(path.join("artifacts"))?;
    let image = path.join("artifacts/board.png");
    session.request(Request::Export(moth_platform::export::Target::Scene {
        path: image.clone(),
        ids: Vec::new(),
        region: Some(bounds),
        side,
    }))?;
    let mut header = [0u8; 24];
    std::fs::File::open(&image)?.read_exact(&mut header)?;
    ensure!(
        &header[..8] == b"\x89PNG\r\n\x1a\n" && &header[12..16] == b"IHDR",
        "native exporter did not produce PNG"
    );
    let width = u32::from_be_bytes(header[16..20].try_into()?);
    let height = u32::from_be_bytes(header[20..24].try_into()?);
    let mapping = coordinates(bounds, width, height);
    let mut summary = session.info(0, 1000);
    summary["preview"] = mapping.clone();
    write_new(&path.join("summary.json"), &summary)?;
    let fingerprint = format!(
        "blake3:{}",
        moth_io::content_hash(&moth_format::board::snapshot(&session.scene)?)
    );
    let created_at = utc(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs());
    let manifest = json!({"protocol_version":"preview-bundle/v1","bundle_id":id,"bundle_kind":"capture","software":"mothref","recipe":"board","status":"ok","created_at":created_at,
        "generator":{"name":"mothref","version":env!("CARGO_PKG_VERSION")},"source":{"project_path":session.path,"fingerprint":fingerprint},"summary_path":"summary.json",
        "warnings":["Scene export uses native bounded previews, not full-resolution originals."],
        "artifacts":[{"artifact_id":"board","role":"hero","kind":"image","label":"Native board preview","media_type":"image/png","path":"artifacts/board.png","bytes":std::fs::metadata(image)?.len(),"coordinates":mapping}]});
    // The manifest is published last; readers ignore interrupted captures.
    write_new(&path.join("manifest.json"), &manifest)?;
    read_manifest(&path)
}
pub(super) fn latest(root: PathBuf) -> Result<Value> {
    let root = std::path::absolute(root)?;
    if root.join("manifest.json").is_file() {
        return read_manifest(&root);
    }
    let mut paths = Vec::new();
    for (index, entry) in std::fs::read_dir(root)?.take(4097).enumerate() {
        ensure!(
            index < 4096,
            "preview root exceeds 4096-entry inspection budget"
        );
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            paths.push(entry.path());
        }
    }
    paths.sort();
    for path in paths.into_iter().rev() {
        if let Ok(value) = read_manifest(&path) {
            return Ok(value);
        }
    }
    bail!("no completed preview; run mothref preview BOARD DIRECTORY")
}
fn coordinates(bounds: Rect, width: u32, height: u32) -> Value {
    let [x, y, w, h] = rect_json(bounds).map(f64::from);
    let scale = (f64::from(width) / w).min(f64::from(height) / h);
    let offset = [
        (f64::from(width) - w * scale) / 2. - x * scale,
        (f64::from(height) - h * scale) / 2. - y * scale,
    ];
    json!({"pixel_size":[width,height],"board_bounds":rect_json(bounds),"board_to_pixel":{"scale":scale,"offset":offset},"pixel_to_board":{"scale":1./scale,"offset":offset.map(|v|-v/scale)}})
}
fn utc(seconds: u64) -> String {
    let mut days = seconds / 86400;
    let mut year = 1970;
    let leap = |year: u64| {
        year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
    };
    while days >= 365 + u64::from(leap(year)) {
        days -= 365 + u64::from(leap(year));
        year += 1;
    }
    let months = [
        31,
        28 + u64::from(leap(year)),
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1;
    for length in months {
        if days < length {
            break;
        }
        days -= length;
        month += 1;
    }
    format!(
        "{year:04}-{month:02}-{:02}T{:02}:{:02}:{:02}Z",
        days + 1,
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestamps_keep_leap_days_and_utc_boundaries() {
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(1709164800), "2024-02-29T00:00:00Z");
        assert_eq!(utc(1709251199), "2024-02-29T23:59:59Z");
        assert_eq!(utc(1709251200), "2024-03-01T00:00:00Z");
    }
    #[test]
    fn pixel_mapping_includes_fractional_padding_and_negative_board_origin() {
        let mapping = coordinates(
            Rect {
                x: -100.,
                y: 50.,
                width: 200.,
                height: 100.,
            },
            400,
            201,
        );
        assert_eq!(
            mapping["board_to_pixel"],
            json!({"scale":2.,"offset":[200.,-99.5]})
        );
        assert_eq!(
            mapping["pixel_to_board"],
            json!({"scale":0.5,"offset":[-100.,49.75]})
        );
    }
}
