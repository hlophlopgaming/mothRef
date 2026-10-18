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
        Ok(file) => {
            file.take(64 * 1024 + 1).read_to_string(&mut text)?;
            if text.len() > 64 * 1024 {
                return Err(io::Error::other("Settings exceed budget"));
            }
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
    save_all(
        directory,
        &BTreeMap::from([(key.to_owned(), value.to_owned())]),
    )
}
/// Saves a whole preference change together, preserving unrelated keys.
pub fn save_all(directory: &Path, values: &BTreeMap<String, String>) -> io::Result<()> {
    if values.len() > 128 {
        return Err(io::Error::other("Too many preferences"));
    }
    for (key, value) in values {
        if key.is_empty()
            || key.len() > 128
            || value.len() > 1024
            || key.contains(['\n', '\r', '='])
            || value.contains(['\n', '\r'])
        {
            return Err(io::Error::other("Invalid preference"));
        }
    }
    std::fs::create_dir_all(directory)?;
    let path = directory.join("settings.txt");
    static WRITER: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _writer = WRITER
        .lock()
        .map_err(|_| io::Error::other("Preference writer unavailable"))?;
    let mut settings = load(directory)?;
    settings.extend(values.iter().map(|(k, v)| (k.clone(), v.clone())));
    let text = settings
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("\n");
    if text.len() > 64 * 1024 {
        return Err(io::Error::other("Settings exceed budget"));
    }
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_reject_injection_and_oversized_files() {
        let root =
            std::env::temp_dir().join(format!("moth-settings-budget-{}", uuid::Uuid::now_v7()));
        assert!(save(&root, "theme\nlanguage", "dark").is_err());
        assert!(save(&root, "theme", "dark\nlanguage=ru").is_err());
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("settings.txt"), vec![b'x'; 65537]).unwrap();
        assert!(load(&root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
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
