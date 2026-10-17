// SPDX-License-Identifier: GPL-3.0-or-later
//! Tiny plain-text app settings, kept next to the recent-file catalog.
//! This is a disposable preference store, never board data.
use std::{
    collections::BTreeMap,
    io::{self, Read},
    path::Path,
};
/// Reads `settings.txt` into a `key=value` map; missing files read as empty.
pub fn load(directory: &Path) -> io::Result<BTreeMap<String, String>> {
    std::fs::create_dir_all(directory)?;
    let path = directory.join("settings.txt");
    let mut text = String::new();
    match std::fs::File::open(&path) {
        Ok(mut file) => {
            file.read_to_string(&mut text)?;
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(e),
    }
    Ok(text
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect())
}
/// Writes one `key=value` pair, preserving the others.
pub fn save(directory: &Path, key: &str, value: &str) -> io::Result<()> {
    std::fs::create_dir_all(directory)?;
    let path = directory.join("settings.txt");
    let mut settings = load(directory)?;
    settings.insert(key.to_owned(), value.to_owned());
    let text = settings
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn save_preserves_other_keys_and_load_round_trips() {
        let root = std::env::temp_dir().join(format!("mothref-settings-{}", uuid::Uuid::now_v7()));
        save(&root, "language", "ru").unwrap();
        save(&root, "theme", "dark").unwrap();
        let settings = load(&root).unwrap();
        assert_eq!(settings.get("language").map(String::as_str), Some("ru"));
        assert_eq!(settings.get("theme").map(String::as_str), Some("dark"));
        assert_eq!(load(&root.join("missing")).unwrap().len(), 0);
        std::fs::remove_dir_all(root).unwrap();
    }
}
