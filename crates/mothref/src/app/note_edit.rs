// SPDX-License-Identifier: GPL-3.0-or-later
//! List editing works on the note source and keeps egui's selection and undo history.
use egui::text::{CCursor, CCursorRange};
use egui::{Event, Key, Modifiers, TextEdit};
use moth_scene::NoteLine;
use std::ops::Range;

use super::Editing;

fn byte(text: &str, cursor: CCursor) -> usize {
    text.char_indices()
        .nth(cursor.index.into())
        .map_or(text.len(), |(offset, _)| offset)
}

fn parse_line(line: &str, markup: bool) -> (&str, NoteLine, &str) {
    if markup {
        NoteLine::parse(line)
    } else {
        ("", NoteLine::Plain, line)
    }
}

fn caret(text: &str, offset: usize) -> CCursor {
    CCursor::new(text[..offset].chars().count())
}

fn lines(text: &str, range: Range<usize>) -> Range<usize> {
    let start = text[..range.start].rfind('\n').map_or(0, |at| at + 1);
    let end = if range.start < range.end && text[..range.end].ends_with('\n') {
        range.end - 1
    } else {
        text[range.end..]
            .find('\n')
            .map_or(text.len(), |at| at + range.end)
    };
    start..end
}

fn apply_changes(
    text: &mut String,
    selection: CCursorRange,
    changes: Vec<(Range<usize>, String)>,
) -> CCursorRange {
    let mut ends = [
        byte(text, selection.primary),
        byte(text, selection.secondary),
    ];
    for (span, replacement) in changes.into_iter().rev() {
        for end in &mut ends {
            if *end >= span.start {
                *end = if *end >= span.end {
                    *end - span.len() + replacement.len()
                } else {
                    span.start + replacement.len()
                };
            }
        }
        text.replace_range(span, &replacement);
    }
    CCursorRange {
        primary: caret(text, ends[0]),
        secondary: caret(text, ends[1]),
        h_pos: None,
    }
}

fn record(
    ctx: &egui::Context,
    id: egui::Id,
    mut state: egui::text_edit::TextEditState,
    before: (CCursorRange, String),
    after: (CCursorRange, String),
) {
    let mut undoer = state.undoer();
    undoer.add_undo(&before);
    // Feeding a new state clears redo history, as it does for a normal edit.
    undoer.feed_state(ctx.input(|i| i.time), &after);
    undoer.add_undo(&after);
    state.set_undoer(undoer);
    state.cursor.set_char_range(Some(after.0));
    TextEdit::store_state(ctx, id, state);
}

/// Applies a toolbar style to the current paragraph or every selected paragraph.
pub(super) fn format(ctx: &egui::Context, id: egui::Id, editing: &mut Editing, style: NoteLine) {
    let state = TextEdit::load_state(ctx, id).unwrap_or_default();
    let selection = state.cursor.char_range().unwrap_or_default();
    let before = (selection, editing.draft.clone());
    let left = byte(&before.1, selection.primary);
    let right = byte(&before.1, selection.secondary);
    let span = lines(&before.1, left.min(right)..left.max(right));
    let selected = &before.1[span.clone()];
    let markup = editing.markup;
    let same = |left, right| std::mem::discriminant(&left) == std::mem::discriminant(&right);
    let remove = selected
        .split('\n')
        .all(|line| same(parse_line(line, markup).1, style));
    let mut at = span.start;
    let mut number = 1u32;
    let mut changes = Vec::new();
    for line in selected.split('\n') {
        let (indent, old, body) = parse_line(line, markup);
        let start = at + indent.len();
        let end = at + line.len() - body.len();
        if !remove && old.is_list() && same(old, style) && !matches!(style, NoteLine::Numbered(_)) {
            at += line.len() + 1;
            continue;
        }
        let prefix = match style {
            _ if remove => String::new(),
            NoteLine::Numbered(_) => {
                let prefix = NoteLine::Numbered(number).prefix().into_owned();
                number += 1;
                prefix
            }
            _ => style.prefix().into_owned(),
        };
        changes.push((start..end, prefix));
        at += line.len() + 1;
    }
    let mut draft = before.1.clone();
    let selection = apply_changes(&mut draft, selection, changes);
    if draft.len() <= moth_scene::MAX_NOTE_BYTES {
        editing.draft = draft;
        editing.markup = true;
        record(ctx, id, state, before, (selection, editing.draft.clone()));
    }
    editing.focus = true;
}

pub(super) fn display_list_markers(text: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| {
            let (indent, style, body) = NoteLine::parse(line);
            match style {
                NoteLine::Bullet | NoteLine::Check(_) => {
                    format!("{indent}{}{body}", style.prefix())
                }
                _ => line.to_owned(),
            }
        })
        .collect()
}

/// Turns a typed dash/space or checklist prefix into its visible editor marker
/// after egui processes the text event, so the caret and IME stay in its control.
pub(super) fn show_typed_marker(
    ctx: &egui::Context,
    editing: &mut Editing,
    output: &egui::text_edit::TextEditOutput,
) {
    if !editing.markup || !output.response.changed() {
        return;
    }
    let Some(selection) = output
        .cursor_range
        .filter(|range| range.primary == range.secondary)
    else {
        return;
    };
    let offset = byte(&editing.draft, selection.primary);
    let start = editing.draft[..offset].rfind('\n').map_or(0, |at| at + 1);
    let raw = &editing.draft[start..offset];
    let (indent, style, body) = NoteLine::parse(raw);
    if !body.is_empty()
        || !matches!(style, NoteLine::Bullet | NoteLine::Check(_))
        || raw[indent.len()..] == style.prefix()
    {
        return;
    }
    let range = start + indent.len()..offset;
    let before = (selection, editing.draft.clone());
    let selection = apply_changes(
        &mut editing.draft,
        selection,
        vec![(range, style.prefix().into_owned())],
    );
    record(
        ctx,
        output.response.id,
        output.state.clone(),
        before,
        (selection, editing.draft.clone()),
    );
    ctx.request_repaint();
}

const INDENT: &str = "  ";

fn enter(text: &mut String, selection: Range<usize>, soft: bool) -> Option<usize> {
    let line_start = text[..selection.start].rfind('\n').map_or(0, |at| at + 1);
    let line_end = text[selection.start..]
        .find('\n')
        .map_or(text.len(), |at| at + selection.start);
    let line = &text[line_start..line_end];
    let (indent, style, body) = NoteLine::parse(line);
    let item_start = line_start + line.len() - body.len();
    let (indent, style, body, item_start) = if style.is_list() {
        (indent, style, body, item_start)
    } else {
        // A soft return leaves an indented continuation of the preceding item.
        let (indent, style, _) = text[..line_start]
            .split_terminator('\n')
            .rev()
            .map(NoteLine::parse)
            .find(|(_, style, body)| style.is_list() || !body.starts_with(INDENT))?;
        if !style.is_list() || !line.starts_with(&format!("{indent}{INDENT}")) {
            return None;
        }
        (indent, style, line, line_start)
    };
    if selection.start < item_start {
        return None;
    }
    if !soft && body.trim().is_empty() && selection.is_empty() {
        text.replace_range(line_start..line_end, "");
        return Some(line_start);
    }
    let next = match style {
        NoteLine::Numbered(number) => NoteLine::Numbered(number.saturating_add(1)),
        NoteLine::Check(_) => NoteLine::Check(false),
        _ => style,
    };
    let prefix = if soft {
        format!("\n{indent}{INDENT}")
    } else {
        format!("\n{indent}{}", next.prefix())
    };
    let offset = selection.start + prefix.len();
    let depth = indent.len();
    text.replace_range(selection, &prefix);
    if !soft && let NoteLine::Numbered(number) = next {
        renumber_following(text, offset, number, depth);
    }
    Some(offset)
}

fn renumber_following(text: &mut String, offset: usize, mut number: u32, depth: usize) {
    let mut changes = Vec::new();
    let mut start = offset;
    for line in text[offset..].split('\n').skip(1) {
        start = text[start..]
            .find('\n')
            .map_or(text.len(), |at| start + at + 1);
        let (indent, style, body) = NoteLine::parse(line);
        if indent.len() > depth || (!style.is_list() && line.starts_with(INDENT)) {
            continue;
        }
        if indent.len() != depth || !matches!(style, NoteLine::Numbered(_)) {
            break;
        }
        number = number.saturating_add(1);
        changes.push((
            start + depth..start + line.len() - body.len(),
            NoteLine::Numbered(number).prefix(),
        ));
    }
    for (span, prefix) in changes.into_iter().rev() {
        text.replace_range(span, &prefix);
    }
}

fn indent(text: &mut String, selection: CCursorRange, outdent: bool) -> Option<CCursorRange> {
    let start = byte(text, selection.primary);
    let end = byte(text, selection.secondary);
    let span = lines(text, start.min(end)..start.max(end));
    let mut at = span.start;
    let mut changes = Vec::new();
    let selected = &text[span];
    if !selected
        .split('\n')
        .any(|line| NoteLine::parse(line).1.is_list())
    {
        return None;
    }
    for line in selected.split('\n') {
        if !line.is_empty() {
            if outdent {
                let remove = if line.starts_with('\t') {
                    1
                } else {
                    line.bytes()
                        .take(INDENT.len())
                        .take_while(|c| *c == b' ')
                        .count()
                };
                changes.push((at..at + remove, String::new()));
            } else {
                changes.push((at..at, INDENT.to_owned()));
            }
        }
        at += line.len() + 1;
    }
    Some(apply_changes(text, selection, changes))
}

/// Handles list-specific keys only while the editor owns focus. Regular editing
/// and IME input stay with the text widget.
pub(super) fn list_keys(ui: &egui::Ui, id: egui::Id, editing: &mut Editing) {
    if !editing.markup || !ui.memory(|m| m.has_focus(id)) {
        return;
    }
    let Some(state) = TextEdit::load_state(ui.ctx(), id) else {
        return;
    };
    let Some(selection) = state.cursor.char_range() else {
        return;
    };
    let key = ui.input(|input| {
        if input
            .events
            .iter()
            .any(|event| matches!(event, Event::Ime(_)))
        {
            return None;
        }
        input
            .events
            .iter()
            .enumerate()
            .find_map(|(index, event)| match event {
                Event::Key {
                    key: key @ (Key::Enter | Key::Tab),
                    pressed: true,
                    modifiers,
                    ..
                } if matches!(*modifiers, Modifiers::NONE | Modifiers::SHIFT) => {
                    Some((index, *key, modifiers.shift))
                }
                _ => None,
            })
    });
    let Some((index, key, shift)) = key else {
        return;
    };
    let before = (selection, editing.draft.clone());
    let mut draft = before.1.clone();
    let next = if key == Key::Enter {
        let left = byte(&draft, selection.primary);
        let right = byte(&draft, selection.secondary);
        enter(&mut draft, left.min(right)..left.max(right), shift)
            .map(|at| CCursorRange::one(caret(&draft, at)))
    } else {
        indent(&mut draft, selection, shift)
    };
    let Some(next) = next else { return };
    if draft.len() > moth_scene::MAX_NOTE_BYTES {
        return;
    }
    ui.input_mut(|input| {
        input.events.remove(index);
    });
    editing.draft = draft;
    record(ui.ctx(), id, state, before, (next, editing.draft.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enter_continues_and_exits_nested_checklists() {
        let mut text = "  - [x] Готово".to_owned();
        let at = text.len();
        let tail = enter(&mut text, at..at, false).unwrap();
        assert_eq!(text, "  - [x] Готово\n  □ ");
        enter(&mut text, tail..tail, false).unwrap();
        assert_eq!(text, "  - [x] Готово\n");
    }

    #[test]
    fn a_soft_break_preserves_the_number_and_splitting_renumbers_following_items() {
        let mut text = "1. Первый\n2. Следующий".to_owned();
        let at = "1. Первый".len();
        let continuation = enter(&mut text, at..at, true).unwrap();
        assert_eq!(text, "1. Первый\n  \n2. Следующий");
        text.insert_str(continuation, "деталь");
        let at = continuation + "деталь".len();
        enter(&mut text, at..at, false).unwrap();
        assert_eq!(text, "1. Первый\n  деталь\n2. \n3. Следующий");
    }

    #[test]
    fn indent_and_outdent_preserve_a_multiline_unicode_selection() {
        let mut text = "- ёж\n- лиса\nостальной текст".to_owned();
        let end = caret(&text, "- ёж\n- лиса\n".len());
        let selected =
            indent(&mut text, CCursorRange::two(CCursor::default(), end), false).unwrap();
        assert_eq!(text, "  - ёж\n  - лиса\nостальной текст");
        indent(&mut text, selected, true).unwrap();
        assert_eq!(text, "- ёж\n- лиса\nостальной текст");
    }
}
