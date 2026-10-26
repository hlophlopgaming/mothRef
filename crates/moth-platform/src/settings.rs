// SPDX-License-Identifier: GPL-3.0-or-later
//! Tiny plain-text app settings, kept next to the recent-file catalog.
//! This is a disposable preference store, never board data.
use std::{
    collections::BTreeMap,
    io::{self, Read},
    path::Path,
};
/// Persistent accessibility preference; absent means motion is enabled.
pub const REDUCED_MOTION: &str = "reduced_motion";
/// All preferences and registered shortcuts fit one validated update; the file still has a 64 KiB limit.
pub const MAX_ENTRIES: usize = 256;

/// Stable opaque identity for a board's local presentation preference, independent of Rust's hasher.
pub fn board_window_key(path: &str) -> Option<String> {
    (!path.is_empty()).then(|| format!("board_window_{}", blake3::hash(path.as_bytes()).to_hex()))
}

/// Keep presentation history for the 64 most recently used boards within the preference budget.
pub fn trim_board_windows(values: &mut BTreeMap<String, String>) {
    let mut keys: Vec<_> = values
        .iter()
        .filter(|(key, _)| key.starts_with("board_window_"))
        .map(|(key, value)| {
            (
                value
                    .split_once('|')
                    .and_then(|(time, _)| time.parse::<u128>().ok())
                    .unwrap_or(0),
                key.clone(),
            )
        })
        .collect();
    keys.sort_unstable();
    let excess = keys.len().saturating_sub(64);
    for (_, key) in keys.into_iter().take(excess) {
        values.remove(&key);
    }
}

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
    if values.len() > MAX_ENTRIES {
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
    trim_board_windows(&mut settings);
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
    fn board_window_history_evicts_the_least_recent_without_touching_settings() {
        let mut values = BTreeMap::from([("theme".into(), "dark".into())]);
        for index in 0..70 {
            values.insert(
                format!("board_window_{index}"),
                format!("{index}|appearance"),
            );
        }
        trim_board_windows(&mut values);
        assert_eq!(values.len(), 65);
        assert_eq!(values.get("theme").map(String::as_str), Some("dark"));
        assert!(!values.contains_key("board_window_5"));
        assert!(values.contains_key("board_window_6"));
        assert!(values.contains_key("board_window_69"));
        assert!(board_window_key("").is_none());
        assert_eq!(board_window_key("a"), board_window_key("a"));
        assert_ne!(board_window_key("a"), board_window_key("b"));
    }

    #[test]
    fn entry_budget_accepts_the_full_registry_and_still_rejects_oversized_updates() {
        let root =
            std::env::temp_dir().join(format!("moth-settings-entries-{}", uuid::Uuid::now_v7()));
        let mut values: BTreeMap<_, _> = (0..MAX_ENTRIES)
            .map(|index| (format!("binding_{index}"), String::new()))
            .collect();
        save_all(&root, &values).unwrap();
        assert_eq!(load(&root).unwrap(), values);
        values.insert("one-too-many".into(), String::new());
        assert!(save_all(&root, &values).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

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
