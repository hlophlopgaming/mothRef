// SPDX-License-Identifier: GPL-3.0-or-later
fn main() -> anyhow::Result<()> {
    let mut directories = vec![std::path::PathBuf::from("images")];
    let mut accepted = 0;
    let mut failed = 0;
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory)? {
            let path = entry?.path();
            if path.is_dir() {
                directories.push(path);
                continue;
            }
            if !matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp")
            ) {
                continue;
            }
            if path.metadata()?.len() > moth_io::MAX_INPUT_BYTES {
                continue;
            }
            match moth_io::decode(&std::fs::read(&path)?, 256) {
                Ok(image) => {
                    assert!(!image.mips.is_empty());
                    accepted += 1;
                }
                Err(error) => {
                    println!("{}: {error}", path.display());
                    failed += 1;
                }
            }
        }
    }
    println!("{accepted} accepted, {failed} rejected");
    Ok(())
}
