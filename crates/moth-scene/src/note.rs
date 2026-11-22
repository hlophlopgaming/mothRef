// SPDX-License-Identifier: GPL-3.0-or-later
//! Line formatting shared by the canvas and vector export.
use std::borrow::Cow;

/// Presentation and editor entry point of a text item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NoteKind {
    /// A note on a backed card.
    #[default]
    Card,
    /// Text over the board with no card background.
    Label,
    /// A card started as a list.
    List,
}

impl NoteKind {
    /// Name used by the native interface.
    pub fn code(self) -> &'static str {
        match self {
            Self::Card => "note",
            Self::Label => "label",
            Self::List => "list",
        }
    }

    /// Native name, shared by command options and callbacks.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "note" => Some(Self::Card),
            "label" => Some(Self::Label),
            "list" => Some(Self::List),
            _ => None,
        }
    }
}

/// A formatting prefix recognized at the start of a note line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteLine {
    /// Ordinary text, including escaped formatting prefixes.
    Plain,
    /// A larger heading.
    Heading,
    /// An unordered list entry.
    Bullet,
    /// A numbered list entry.
    Numbered(u32),
    /// A checklist entry with its current state.
    Check(bool),
}

impl NoteLine {
    /// Interprets a line while preserving its raw text for editing.
    pub fn parse(line: &str) -> (&str, Self, &str) {
        let content = line.trim_start_matches([' ', '\t']);
        let indent = &line[..line.len() - content.len()];
        let (style, body) = Self::split_prefix(content);
        if style.is_list() || (indent.is_empty() && style == Self::Heading) {
            return (indent, style, body);
        }
        if let Some(body) = line.strip_prefix('\\')
            && Self::split_prefix(body.trim_start_matches([' ', '\t'])).0 != Self::Plain
        {
            return ("", Self::Plain, body);
        }
        ("", Self::Plain, line)
    }

    fn split_prefix(line: &str) -> (Self, &str) {
        for (prefix, style) in [
            ("# ", Self::Heading),
            ("- [ ] ", Self::Check(false)),
            ("- [x] ", Self::Check(true)),
            ("- [X] ", Self::Check(true)),
            ("• [ ] ", Self::Check(false)),
            ("• [x] ", Self::Check(true)),
            ("• [X] ", Self::Check(true)),
            ("□ ", Self::Check(false)),
            ("▣ ", Self::Check(true)),
            ("- ", Self::Bullet),
            ("• ", Self::Bullet),
        ] {
            if let Some(body) = line.strip_prefix(prefix) {
                return (style, body);
            }
        }
        let digits = line.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0
            && let Some(body) = line[digits..].strip_prefix(". ")
            && let Ok(number) = line[..digits].parse()
        {
            return (Self::Numbered(number), body);
        }
        (Self::Plain, line)
    }

    /// Whether this prefix participates in a list.
    pub fn is_list(self) -> bool {
        matches!(self, Self::Bullet | Self::Numbered(_) | Self::Check(_))
    }

    /// The marker prepended to the visible body.
    pub fn marker(self) -> Cow<'static, str> {
        Cow::Borrowed(match self {
            Self::Bullet => "• ",
            Self::Check(false) => "□ ",
            Self::Check(true) => "▣ ",
            Self::Numbered(number) => return Cow::Owned(format!("{number}. ")),
            _ => "",
        })
    }

    /// Relative heading size in the note's coordinate space.
    pub fn scale(self) -> f32 {
        if self == Self::Heading { 1.35 } else { 1. }
    }

    /// The editable text prefix for this style. List markers stay visible while typing.
    pub fn prefix(self) -> Cow<'static, str> {
        Cow::Borrowed(match self {
            Self::Plain => "",
            Self::Heading => "# ",
            Self::Bullet => "• ",
            Self::Check(false) => "□ ",
            Self::Check(true) => "▣ ",
            Self::Numbered(number) => return Cow::Owned(format!("{number}. ")),
        })
    }
}

/// Interior space in board units between a note's content and its frame.
pub fn note_padding(font_size: f32) -> f32 {
    font_size / 4.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_prefixes_keep_text_and_escape_literals() {
        assert_eq!(
            NoteLine::parse("# Heading"),
            ("", NoteLine::Heading, "Heading")
        );
        assert_eq!(
            NoteLine::parse("- [X] task"),
            ("", NoteLine::Check(true), "task")
        );
        assert_eq!(NoteLine::parse("- item"), ("", NoteLine::Bullet, "item"));
        assert_eq!(
            NoteLine::parse("  12. Item"),
            ("  ", NoteLine::Numbered(12), "Item")
        );
        assert_eq!(
            NoteLine::parse(r"\- literal"),
            ("", NoteLine::Plain, "- literal")
        );
    }
}
