// SPDX-License-Identifier: GPL-3.0-or-later
use crate::Text;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A bundled language; catalogs determine the available choices at build time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Locale(usize);

struct Catalog {
    code: &'static str,
    name: &'static str,
    strings: &'static [&'static str],
}

include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));

impl Locale {
    /// Native names remain readable when another language is active.
    pub fn name(self) -> &'static str {
        CATALOGS[self.0].name
    }
    /// Stable language tag used by persisted preferences.
    pub fn code(self) -> &'static str {
        CATALOGS[self.0].code
    }
    /// Accepts language tags and OS locale names, preferring an exact match.
    pub fn from_code(code: &str) -> Option<Self> {
        let code = code
            .trim()
            .split(['.', '@'])
            .next()?
            .replace('_', "-")
            .to_ascii_lowercase();
        Self::ALL
            .into_iter()
            .find(|l| l.code() == code)
            .or_else(|| {
                let base = code.split('-').next()?;
                Self::ALL.into_iter().find(|l| l.code() == base)
            })
    }
}

static CURRENT: AtomicUsize = AtomicUsize::new(Locale::EN.0);

/// Returns the active interface language.
pub fn locale() -> Locale {
    Locale(CURRENT.load(Ordering::Relaxed))
}
/// Switches translations for the next frame without runtime I/O.
pub fn set_locale(language: Locale) {
    CURRENT.store(language.0, Ordering::Relaxed);
}
/// Missing translations fall back to English at build time.
pub fn text(key: Text) -> &'static str {
    CATALOGS[locale().0].strings[index(key)]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogs_and_language_tags() {
        assert_eq!(Locale::from_code(" RU_ru.UTF-8 "), Some(Locale::RU));
        assert_eq!(Locale::from_code("en-GB"), Some(Locale::EN));
        assert_eq!(Locale::from_code("unknown"), None);
        let english = &CATALOGS[Locale::EN.0];
        for language in Locale::ALL {
            let catalog = &CATALOGS[language.0];
            assert!(!catalog.name.is_empty());
            assert_eq!(catalog.strings.len(), english.strings.len());
            assert!(catalog.strings.iter().all(|s| !s.trim().is_empty()));
        }
        assert_eq!(english.strings[index(Text::Title)], "MothRef");
    }
}
