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
    translated(locale(), key)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plural_counts_and_messages_follow_the_render_language() {
        assert_eq!(
            Message::Count(Text::AddedImages, 6).render(Locale::EN),
            "Added images: 6"
        );
        assert_eq!(
            Message::Count(Text::AddedImages, 6).render(Locale::RU),
            "Добавлено изображений: 6"
        );
        assert_eq!(item_counts(Locale::EN, 1, 2), "1 image · 2 notes");
        assert_eq!(item_counts(Locale::EN, 1, 0), "1 image");
        assert_eq!(item_counts(Locale::RU, 0, 1), "1 заметка");
        assert_eq!(item_counts(Locale::EN, 0, 0), "0 images");
        for (count, noun) in [
            (0, "изображений"),
            (1, "изображение"),
            (2, "изображения"),
            (5, "изображений"),
            (11, "изображений"),
            (12, "изображений"),
            (21, "изображение"),
            (24, "изображения"),
            (111, "изображений"),
        ] {
            assert_eq!(item_counts(Locale::RU, count, 0), format!("{count} {noun}"));
            assert_eq!(
                item_counts(Locale::RU, count, 1),
                if count == 0 {
                    "1 заметка".to_owned()
                } else {
                    format!("{count} {noun} · 1 заметка")
                }
            );
        }
        let message = Message::Count(Text::ImportSkippedCount, 3);
        assert_eq!(message.render(Locale::EN), "Imported; skipped 3 items");
        assert_eq!(
            message.render(Locale::RU),
            "Импортировано; пропущено элементов: 3"
        );
        let saved = Message::from(Text::Saved);
        assert_ne!(saved.render(Locale::EN), saved.render(Locale::RU));
    }
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

/// Resolves a key without changing the application language.
pub fn translated(language: Locale, key: Text) -> &'static str {
    CATALOGS[language.0].strings[index(key)]
}
/// Inflects board counts using the selected catalog's plural rules.
pub fn item_counts(language: Locale, images: usize, notes: usize) -> String {
    let noun = |count: usize, forms: [Text; 3]| {
        let form = if language == Locale::RU {
            if count % 10 == 1 && count % 100 != 11 {
                0
            } else if (2..=4).contains(&(count % 10)) && !(12..=14).contains(&(count % 100)) {
                1
            } else {
                2
            }
        } else if count == 1 {
            0
        } else {
            2
        };
        format!("{count} {}", translated(language, forms[form]))
    };
    let image_count = noun(images, [Text::ImageOne, Text::ImageFew, Text::ImageMany]);
    let note_count = noun(notes, [Text::NoteOne, Text::NoteFew, Text::NoteMany]);
    match (images, notes) {
        (_, 0) => image_count,
        (0, _) => note_count,
        _ => format!("{image_count} · {note_count}"),
    }
}

/// Worker feedback keeps translation keys until the next paint.
#[derive(Clone)]
pub enum Message {
    /// A bundled message without arguments.
    Text(Text),
    /// A bundled count-bearing message.
    Count(Text, usize),
    /// External errors or user data, already supplied by a worker.
    Detail(String),
}
impl Default for Message {
    fn default() -> Self {
        Self::Detail(String::new())
    }
}
impl From<Text> for Message {
    fn from(key: Text) -> Self {
        Self::Text(key)
    }
}
impl From<String> for Message {
    fn from(detail: String) -> Self {
        Self::Detail(detail)
    }
}
impl Message {
    /// Resolve the current language without changing the saved message.
    pub fn render(&self, language: Locale) -> String {
        match self {
            Self::Text(key) => translated(language, *key).to_owned(),
            Self::Count(key, count) => {
                translated(language, *key).replace("{count}", &count.to_string())
            }
            Self::Detail(detail) => detail.clone(),
        }
    }
}
