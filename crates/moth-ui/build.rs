// SPDX-License-Identifier: GPL-3.0-or-later
use std::{collections::BTreeMap, env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=locales");
    let mut catalogs = BTreeMap::new();
    for entry in fs::read_dir("locales").expect("locales directory must exist") {
        let path = entry
            .expect("catalog directory entry must be readable")
            .path();
        if path.extension().and_then(|s| s.to_str()) != Some("txt") {
            continue;
        }
        let code = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("UTF-8 locale code")
            .to_owned();
        assert!(
            !code.is_empty() && code.bytes().all(|c| c.is_ascii_lowercase() || c == b'-'),
            "invalid locale code: {code}"
        );
        let source = fs::read_to_string(&path).expect("catalog must be UTF-8");
        let mut entries = BTreeMap::new();
        for (line, value) in source.lines().enumerate() {
            if value.trim().is_empty() || value.starts_with('#') {
                continue;
            }
            let (key, value) = value
                .split_once(" = ")
                .unwrap_or_else(|| panic!("{}:{}: expected key = value", path.display(), line + 1));
            assert!(!value.trim().is_empty(), "{}: empty {key}", path.display());
            assert!(
                entries.insert(key.to_owned(), value.to_owned()).is_none(),
                "{}: duplicate {key}",
                path.display()
            );
        }
        let name = entries.remove("name").expect("catalog needs native name");
        catalogs.insert(code, (name, entries));
    }
    let english = &catalogs.get("en").expect("English fallback must exist").1;
    let mut output = String::from("const CATALOGS: &[Catalog] = &[\n");
    for (code, (name, entries)) in &catalogs {
        for key in entries.keys() {
            assert!(english.contains_key(key), "{code}: unknown key {key}");
        }
        output.push_str(&format!(
            "Catalog {{ code: {code:?}, name: {name:?}, strings: &[\n"
        ));
        for (key, fallback) in english {
            output.push_str(&format!("{:?},\n", entries.get(key).unwrap_or(fallback)));
        }
        output.push_str("] },\n");
    }
    output.push_str("];\nimpl Locale {\n/// Every catalog appears in the language menu automatically.\npub const ALL: [Self; ");
    output.push_str(&format!("{}] = [", catalogs.len()));
    for i in 0..catalogs.len() {
        output.push_str(&format!("Self({i}),"));
    }
    output.push_str("];\n");
    for (i, code) in catalogs.keys().enumerate() {
        output.push_str(&format!(
            "/// Built-in language `{code}`.\npub const {}: Self = Self({i});\n",
            code.replace('-', "_").to_ascii_uppercase()
        ));
    }
    output.push_str("}\nfn index(key: Text) -> usize { match key {\n");
    for (i, key) in english.keys().enumerate() {
        output.push_str(&format!("Text::{key} => {i},\n"));
    }
    output.push_str("} }\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR")).join("catalogs.rs"),
        output,
    )
    .expect("generated catalogs must be writable");
}
