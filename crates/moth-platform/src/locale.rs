// SPDX-License-Identifier: GPL-3.0-or-later
//! Best-effort detection of the user's interface language, used only as the
//! first-launch default before a choice is persisted in the settings file.
/// Returns the two-letter language code the UI should default to: `"en"` or `"ru"`.
pub fn detect() -> &'static str {
    language_code(&raw_language())
}

fn language_code(raw: &str) -> &'static str {
    match raw.split(['_', '-', '.', '@']).next() {
        Some("ru") => "ru",
        _ => "en",
    }
}

#[cfg(not(target_os = "windows"))]
fn raw_language() -> String {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(var)
            && !value.is_empty()
            && value != "C"
            && value != "POSIX"
        {
            return value.to_ascii_lowercase();
        }
    }
    String::new()
}

#[cfg(target_os = "windows")]
fn raw_language() -> String {
    // SAFETY: GetUserDefaultUILanguage always succeeds and returns a LANGID.
    let langid = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
    // The primary language is the low ten bits of the LANGID.
    match langid & 0x3FF {
        0x19 => "ru".to_owned(), // LANG_RUSSIAN
        other => format!("{other}"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn language_code_maps_known_locale_codes_to_a_supported_language() {
        for (input, expected) in [
            ("ru_RU.UTF-8", "ru"),
            ("ru", "ru"),
            ("en_US.UTF-8", "en"),
            ("C", "en"),
            ("", "en"),
            ("de_DE.UTF-8", "en"),
        ] {
            assert_eq!(super::language_code(input), expected, "input: {input}");
        }
    }
}
