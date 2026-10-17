// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! Shared theme and localized M0 controls.
use egui::{Color32, Context};
use std::sync::atomic::{AtomicU8, Ordering};
/// Centralized strings keep widgets independent of the selected locale.
#[derive(Clone, Copy, Debug)]
pub enum Text {
    /// Localized persistence control.
    StorageBusy,
    /// Localized persistence control.
    OpenEmpty,
    /// Localized persistence control.
    Recovered,
    /// Localized persistence control.
    Opened,
    /// Localized persistence control.
    Saved,
    /// Localized persistence control.
    Path,
    /// Localized persistence control.
    Save,
    /// Localized persistence control.
    Open,
    /// Localized persistence control.
    Undo,
    /// Localized persistence control.
    Redo,
    /// Localized persistence control.
    Recovery,
    /// Localized persistence control.
    SaveClose,

    /// Explains bounded clipboard worker backpressure.
    ClipboardBusy,
    /// Explains bounded import backlog backpressure.
    ImportBusy,
    /// Explains empty and unsupported clipboard payloads without exposing backend errors.
    ClipboardEmpty,
    /// Shared localization key for Title.
    Title,
    /// Shared localization key for Paste.
    Paste,
    /// Shared localization key for Fit.
    Fit,
    /// Shared localization key for Pan.
    Pan,
    /// Shared localization key for Select.
    Select,
    /// Shared localization key for Empty.
    Empty,
    /// Shared localization key for Volatile.
    Volatile,
    /// Shared localization key for Close.
    Close,
    /// Shared localization key for Keep.
    Keep,
    /// Shared localization key for CloseWarning.
    CloseWarning,
    /// Shared localization key for BoardFull.
    BoardFull,
    /// Localized AllBoards control.
    AllBoards,
    /// Localized Pinned control.
    Pinned,
    /// Localized Stacks control.
    Stacks,
    /// Localized ListView control.
    ListView,
    /// Localized GridView control.
    GridView,
    /// Localized Name control.
    Name,
    /// Localized LastOpened control.
    LastOpened,
    /// Localized Boards control.
    Boards,
    /// Localized LibraryEmpty control.
    LibraryEmpty,
    /// Localized NoMatches control.
    NoMatches,
    /// Localized Search control.
    Search,
    /// Localized ClickThrough control.
    ClickThrough,
    /// Localized Opacity control.
    Opacity,
    /// Localized ExitOverlay control.
    ExitOverlay,
    /// Localized OverlayUnsupported control.
    OverlayUnsupported,
    /// Localized TabLimit control.
    TabLimit,
    /// Localized PathDialog control.
    PathDialog,
    /// Localized Cancel control.
    Cancel,
    /// Localized Arrange action.
    Arrange,
    /// Localized prompt asking how to arrange a burst of newly added images.
    ArrangePrompt,
    /// Localized Horizontal action.
    Horizontal,
    /// Localized Vertical action.
    Vertical,
    /// Localized Grid action.
    Grid,
    /// Localized Optimal action.
    Optimal,
    /// Localized Normalize action.
    Normalize,
    /// Localized NormalizeWidth action.
    NormalizeWidth,
    /// Localized NormalizeHeight action.
    NormalizeHeight,
    /// Localized NormalizeSize action.
    NormalizeSize,
    /// Localized NormalizeArea action.
    NormalizeArea,
    /// Localized RotateCw action.
    RotateCw,
    /// Localized RotateCcw action.
    RotateCcw,
    /// Localized FlipHorizontal action.
    FlipHorizontal,
    /// Localized FlipVertical action.
    FlipVertical,
    /// Localized ResetTransform action.
    ResetTransform,
    /// Localized Delete action.
    Delete,
    /// Localized FitSelection action.
    FitSelection,
    /// Localized Tags control.
    Tags,
    /// Localized EditTags control.
    EditTags,
    /// Localized TagsHint control.
    TagsHint,
    /// Localized Apply control.
    Apply,
    /// Localized All control.
    All,
    /// Localized SmartCollections control.
    SmartCollections,
    /// Catalog navigation.
    Library,
    /// Open another tab.
    NewBoard,
    /// Safe tab closure.
    CloseTab,
    /// Native window closure.
    CloseWindow,
    /// Native maximize.
    Maximize,
    /// Native minimize.
    Minimize,
    /// Transparent reference mode.
    Overlay,
    /// Native topmost state.
    OnTop,
    /// Import clipboard content.
    PasteImage,
    /// Unsaved board breadcrumb.
    NotSaved,
    /// Names a board before its first save.
    Untitled,
    /// Labels the compact board status.
    Items,
    /// Labels the compact selection status.
    Selected,
    /// Explains the close confirmation.
    SaveQuestion,
    /// Shared localization key for SourceChanged.
    SourceChanged,
    /// Confirms that a BeeRef board was imported.
    Imported,
    /// Explains that some BeeRef items could not be represented.
    ImportSkipped,
}
/// Interface languages. English is the fallback; Russian ships with the 0.3 release.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Locale {
    /// English, the default and the fallback when detection fails.
    En,
    /// Russian.
    Ru,
}

impl Locale {
    /// Every selectable language, listed under its own native name.
    pub const ALL: [Locale; 2] = [Locale::En, Locale::Ru];
    /// Native name keeps the language menu readable without translation.
    pub const fn name(self) -> &'static str {
        match self {
            Locale::En => "English",
            Locale::Ru => "Русский",
        }
    }
    /// Two-letter code persisted in the settings file.
    pub const fn code(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Ru => "ru",
        }
    }
    /// Maps a persisted or detected language code back to a locale.
    pub fn from_code(code: &str) -> Option<Self> {
        match code.trim().to_ascii_lowercase().as_str() {
            "ru" => Some(Locale::Ru),
            "en" => Some(Locale::En),
            _ => None,
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(Locale::En as u8);

/// Returns the active interface language.
pub fn locale() -> Locale {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Locale::Ru,
        _ => Locale::En,
    }
}

/// Switches the interface language; widgets repaint with the next frame.
pub fn set_locale(language: Locale) {
    CURRENT.store(language as u8, Ordering::Relaxed);
}

fn en(key: Text) -> &'static str {
    match key {
        Text::AllBoards => "All boards",
        Text::Pinned => "Pinned",
        Text::Stacks => "STACKS",
        Text::ListView => "List view",
        Text::GridView => "Grid view",
        Text::Name => "Name",
        Text::LastOpened => "Last opened",
        Text::Boards => "boards",
        Text::LibraryEmpty => "Your boards will appear here. Open a board or create a new one.",
        Text::NoMatches => "No boards match your search.",
        Text::Search => "Search…",
        Text::ClickThrough => "Click-through",
        Text::Opacity => "Opacity",
        Text::ExitOverlay => "Exit overlay",
        Text::OverlayUnsupported => "Transparent surfaces are unavailable on this adapter.",
        Text::TabLimit => "Four boards are already open. Close one before opening another.",
        Text::PathDialog => "Open or save a board",
        Text::Cancel => "Cancel",
        Text::Arrange => "Arrange",
        Text::ArrangePrompt => "How should the added images be arranged?",
        Text::Horizontal => "Horizontal",
        Text::Vertical => "Vertical",
        Text::Grid => "Grid",
        Text::Optimal => "Optimal",
        Text::Normalize => "Normalize",
        Text::NormalizeWidth => "Width",
        Text::NormalizeHeight => "Height",
        Text::NormalizeSize => "Size",
        Text::NormalizeArea => "Area",
        Text::RotateCw => "Rotate 90° clockwise",
        Text::RotateCcw => "Rotate 90° counter-clockwise",
        Text::FlipHorizontal => "Flip horizontally",
        Text::FlipVertical => "Flip vertically",
        Text::ResetTransform => "Reset rotation and crop",
        Text::Delete => "Delete",
        Text::FitSelection => "Fit selection",
        Text::Tags => "Tags",
        Text::EditTags => "Edit tags",
        Text::TagsHint => "armor, lighting, wip",
        Text::Apply => "Apply",
        Text::All => "All",
        Text::SmartCollections => "SMART COLLECTIONS",
        Text::Library => "Library",
        Text::NewBoard => "New board",
        Text::CloseTab => "Close board",
        Text::CloseWindow => "Close window",
        Text::Maximize => "Maximize",
        Text::Minimize => "Minimize",
        Text::Overlay => "Overlay mode",
        Text::OnTop => "On top",
        Text::PasteImage => "Paste image",
        Text::NotSaved => "Untitled board · not saved yet",
        Text::Untitled => "Untitled",
        Text::Items => "items",
        Text::Selected => "selected",
        Text::SaveQuestion => "Save this board?",
        Text::StorageBusy => "Storage is busy; wait for pending operations.",
        Text::OpenEmpty => "Save and restart before opening another board.",
        Text::Recovered => "Recovered the last valid checkpoint; torn bytes were ignored.",
        Text::Opened => "Board opened. Changes are journaled automatically.",
        Text::Saved => "Board saved.",
        Text::Path => "Board path (.moth)",
        Text::Save => "Save",
        Text::Open => "Open",
        Text::Undo => "Undo",
        Text::Redo => "Redo",
        Text::Recovery => "Recover a board",
        Text::SaveClose => "Save and close",

        Text::SourceChanged => "The source file changed. Its previous preview cannot be reloaded.",
        Text::Imported => "Imported a BeeRef board. Save it as a .moth file.",
        Text::ImportSkipped => {
            "Imported images from a BeeRef board; notes and unsupported transforms were skipped."
        }
        Text::BoardFull => "Board metadata budget full (1,000,000 images)",
        Text::ClipboardEmpty => {
            "Clipboard has no supported image, files, HTTP(S) image URL or HTML image."
        }
        Text::ImportBusy => "Import queue is full; wait for pending images and try again.",
        Text::Title => "MothRef",
        Text::Paste => "Paste",
        Text::Fit => "Fit board",
        Text::Pan => "Pan",
        Text::Select => "Select",
        Text::ClipboardBusy => "Clipboard is busy; try again shortly.",
        Text::Empty => "Drop images here, or paste an image to begin.",
        Text::Volatile => "Journaled recovery • PNG / JPEG • previews up to 2048 px",
        Text::Close => "Discard and close",
        Text::Keep => "Keep working",
        Text::CloseWarning => {
            "Save this board before closing? Discard deletes this session’s recovery files."
        }
    }
}
// shortcut: Russian plural forms (Items/Boards) are simplified to one genitive form;
// revisit with proper plural rules when a second slavic language is added.
fn ru(key: Text) -> &'static str {
    match key {
        Text::StorageBusy => "Хранилище занято; дождитесь завершения операций.",
        Text::OpenEmpty => "Сохраните и перезапустите, прежде чем открыть другую доску.",
        Text::Recovered => {
            "Восстановлена последняя корректная контрольная точка; повреждённые данные пропущены."
        }
        Text::Opened => "Доска открыта. Изменения журналируются автоматически.",
        Text::Saved => "Доска сохранена.",
        Text::Path => "Путь доски (.moth)",
        Text::Save => "Сохранить",
        Text::Open => "Открыть",
        Text::Undo => "Отменить",
        Text::Redo => "Вернуть",
        Text::Recovery => "Восстановить доску",
        Text::SaveClose => "Сохранить и закрыть",
        Text::ClipboardBusy => "Буфер обмена занят; попробуйте ещё раз.",
        Text::ImportBusy => "Очередь импорта переполнена; дождитесь и попробуйте снова.",
        Text::ClipboardEmpty => {
            "В буфере обмена нет поддерживаемого изображения, файлов, URL изображения или HTML-изображения."
        }
        Text::Title => "MothRef",
        Text::Paste => "Вставить",
        Text::Fit => "Уместить доску",
        Text::Pan => "Перемещение",
        Text::Select => "Выбор",
        Text::Empty => "Перетащите изображения сюда или вставьте изображение.",
        Text::Volatile => "Журналируемое восстановление • PNG / JPEG • превью до 2048 px",
        Text::Close => "Отменить и закрыть",
        Text::Keep => "Продолжить работу",
        Text::CloseWarning => {
            "Сохранить доску перед закрытием? При отмене удаляются файлы восстановления этой сессии."
        }
        Text::BoardFull => "Бюджет метаданных доски исчерпан (1 000 000 изображений)",
        Text::AllBoards => "Все доски",
        Text::Pinned => "Закреплённые",
        Text::Stacks => "СТОПКИ",
        Text::ListView => "Список",
        Text::GridView => "Сетка",
        Text::Name => "Имя",
        Text::LastOpened => "Открыта",
        Text::Boards => "доски",
        Text::LibraryEmpty => "Здесь появятся ваши доски. Откройте или создайте новую.",
        Text::NoMatches => "Нет досок, соответствующих поиску.",
        Text::Search => "Поиск…",
        Text::ClickThrough => "Сквозной клик",
        Text::Opacity => "Непрозрачность",
        Text::ExitOverlay => "Выйти из режима наложения",
        Text::OverlayUnsupported => "Прозрачные поверхности недоступны на этом адаптере.",
        Text::TabLimit => "Уже открыто четыре доски. Закройте одну, прежде чем открыть другую.",
        Text::PathDialog => "Открыть или сохранить доску",
        Text::Cancel => "Отмена",
        Text::Arrange => "Расставить",
        Text::ArrangePrompt => "Как расположить добавленные изображения?",
        Text::Horizontal => "Горизонтально",
        Text::Vertical => "Вертикально",
        Text::Grid => "Сетка",
        Text::Optimal => "Оптимально",
        Text::Normalize => "Нормализовать",
        Text::NormalizeWidth => "Ширина",
        Text::NormalizeHeight => "Высота",
        Text::NormalizeSize => "Размер",
        Text::NormalizeArea => "Площадь",
        Text::RotateCw => "Повернуть на 90° по часовой",
        Text::RotateCcw => "Повернуть на 90° против часовой",
        Text::FlipHorizontal => "Отразить по горизонтали",
        Text::FlipVertical => "Отразить по вертикали",
        Text::ResetTransform => "Сбросить поворот и кадрирование",
        Text::Delete => "Удалить",
        Text::FitSelection => "Уместить выделенное",
        Text::Tags => "Теги",
        Text::EditTags => "Изменить теги",
        Text::TagsHint => "броня, освещение, в работе",
        Text::Apply => "Применить",
        Text::All => "Все",
        Text::SmartCollections => "УМНЫЕ КОЛЛЕКЦИИ",
        Text::Library => "Библиотека",
        Text::NewBoard => "Новая доска",
        Text::CloseTab => "Закрыть доску",
        Text::CloseWindow => "Закрыть окно",
        Text::Maximize => "Развернуть",
        Text::Minimize => "Свернуть",
        Text::Overlay => "Режим наложения",
        Text::OnTop => "Поверх окон",
        Text::PasteImage => "Вставить изображение",
        Text::NotSaved => "Безымянная доска · ещё не сохранена",
        Text::Untitled => "Безымянная",
        Text::Items => "элементов",
        Text::Selected => "выбрано",
        Text::SaveQuestion => "Сохранить эту доску?",
        Text::SourceChanged => {
            "Исходный файл изменился. Предыдущее превью невозможно перезагрузить."
        }
        Text::Imported => "Импортирована доска BeeRef. Сохраните её как файл .moth.",
        Text::ImportSkipped => {
            "Импортированы изображения из доски BeeRef; заметки и неподдерживаемые преобразования пропущены."
        }
    }
}

/// Translation lookup has one entry point; widgets stay independent of the locale.
pub fn text(key: Text) -> &'static str {
    match locale() {
        Locale::En => en(key),
        Locale::Ru => ru(key),
    }
}
/// Color tokens are shared by the canvas overlays and the egui theme.
pub const BACKGROUND: Color32 = Color32::from_rgb(17, 19, 23);
/// Selection color remains visible over both dark and bright images.
pub const ACCENT: Color32 = Color32::from_rgb(181, 206, 143);
/// Applies one house theme without embedding colors in individual controls.
pub fn theme(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::empty();
    fonts.font_data.insert(
        "Hack".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../fonts/Hack-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "Noto Sans".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../fonts/NotoSans-Regular.ttf"
        ))),
    );
    fonts
        .families
        .insert(egui::FontFamily::Proportional, vec!["Noto Sans".into()]);
    fonts
        .families
        .insert(egui::FontFamily::Monospace, vec!["Hack".into()]);
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = Color32::from_rgb(31, 34, 40);
    style.visuals.window_fill = Color32::from_rgb(31, 34, 40);
    style.visuals.window_stroke = egui::Stroke::new(1., Color32::from_rgb(48, 52, 60));
    style.visuals.window_corner_radius = egui::CornerRadius::same(14);
    style.visuals.override_text_color = Some(Color32::from_rgb(224, 226, 231));
    style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(37, 41, 48);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(49, 55, 61);
    style.visuals.selection.bg_fill = Color32::from_rgb(61, 76, 45);
    style.visuals.selection.stroke = egui::Stroke::new(1., ACCENT);
    style.spacing.button_padding = egui::vec2(12., 8.);
    style.spacing.item_spacing = egui::vec2(8., 8.);
    style.spacing.interact_size = egui::vec2(36., 36.);
    style.visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1., Color32::from_rgb(47, 51, 59));
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.corner_radius = egui::CornerRadius::same(8);
    }
    style.animation_time = 0.12;
    ctx.set_style_of(egui::Theme::Dark, style);
    ctx.set_theme(egui::Theme::Dark);
}
/// Canvas overlays and navigation widgets.
pub mod canvas;
mod chrome;
/// Reusable vector controls.
pub mod icons;
/// Responsive board catalog.
pub mod library;
pub use chrome::{Actions, Status, Tab, toolbar};
/// Raised surfaces use the same neutral tone throughout the app.
pub const PANEL: Color32 = Color32::from_rgb(31, 34, 40);
/// Secondary metadata recedes without disappearing.
pub const MUTED: Color32 = Color32::from_rgb(120, 125, 136);

/// Centers the import entry point while keeping it usable on narrow windows.
pub fn empty_board(ui: &mut egui::Ui) -> Actions {
    let bounds = ui.max_rect();
    let size = egui::vec2(
        500f32.min(bounds.width() - 24.).max(0.),
        300f32.min(bounds.height()).max(0.),
    );
    let rect = egui::Rect::from_center_size(bounds.center(), size);
    ui.painter().rect_stroke(
        rect,
        14.,
        egui::Stroke::new(1., Color32::from_rgb(59, 63, 72)),
        egui::StrokeKind::Inside,
    );
    let mut actions = Actions::default();
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(16.)), |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space((size.y * 0.17).max(0.));
            let (icon, _) = ui.allocate_exact_size(egui::vec2(38., 32.), egui::Sense::hover());
            let stroke = egui::Stroke::new(1.5, Color32::from_rgb(113, 119, 132));
            ui.painter()
                .rect_stroke(icon, 3., stroke, egui::StrokeKind::Inside);
            ui.painter()
                .circle_stroke(icon.min + egui::vec2(11., 10.), 3., stroke);
            ui.painter().line_segment(
                [
                    icon.left_bottom() + egui::vec2(3., -3.),
                    icon.center() + egui::vec2(5., 0.),
                ],
                stroke,
            );
            ui.painter().line_segment(
                [
                    icon.center() + egui::vec2(5., 0.),
                    icon.right_bottom() - egui::vec2(3., 5.),
                ],
                stroke,
            );
            ui.add_space(20.);
            ui.label(text(Text::Empty));
            ui.add_space(4.);
            ui.small(text(Text::Volatile));
            ui.add_space(20.);
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - 230.) / 2.).max(0.));
                actions.paste = ui
                    .add_sized([105., 36.], egui::Button::new(text(Text::Paste)))
                    .clicked();
                actions.open = ui
                    .add_sized([105., 36.], egui::Button::new(text(Text::Open)))
                    .clicked();
            });
        });
    });
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_text_key_has_a_non_empty_translation_and_switches_locale() {
        let keys = [
            Text::StorageBusy,
            Text::OpenEmpty,
            Text::Recovered,
            Text::Opened,
            Text::Saved,
            Text::Path,
            Text::Save,
            Text::Open,
            Text::Undo,
            Text::Redo,
            Text::Recovery,
            Text::SaveClose,
            Text::ClipboardBusy,
            Text::ImportBusy,
            Text::ClipboardEmpty,
            Text::Title,
            Text::Paste,
            Text::Fit,
            Text::Pan,
            Text::Select,
            Text::Empty,
            Text::Volatile,
            Text::Close,
            Text::Keep,
            Text::CloseWarning,
            Text::BoardFull,
            Text::AllBoards,
            Text::Pinned,
            Text::Stacks,
            Text::ListView,
            Text::GridView,
            Text::Name,
            Text::LastOpened,
            Text::Boards,
            Text::LibraryEmpty,
            Text::NoMatches,
            Text::Search,
            Text::ClickThrough,
            Text::Opacity,
            Text::ExitOverlay,
            Text::OverlayUnsupported,
            Text::TabLimit,
            Text::PathDialog,
            Text::Cancel,
            Text::Arrange,
            Text::ArrangePrompt,
            Text::Horizontal,
            Text::Vertical,
            Text::Grid,
            Text::Optimal,
            Text::Normalize,
            Text::NormalizeWidth,
            Text::NormalizeHeight,
            Text::NormalizeSize,
            Text::NormalizeArea,
            Text::RotateCw,
            Text::RotateCcw,
            Text::FlipHorizontal,
            Text::FlipVertical,
            Text::ResetTransform,
            Text::Delete,
            Text::FitSelection,
            Text::Tags,
            Text::EditTags,
            Text::TagsHint,
            Text::Apply,
            Text::All,
            Text::SmartCollections,
            Text::Library,
            Text::NewBoard,
            Text::CloseTab,
            Text::CloseWindow,
            Text::Maximize,
            Text::Minimize,
            Text::Overlay,
            Text::OnTop,
            Text::PasteImage,
            Text::NotSaved,
            Text::Untitled,
            Text::Items,
            Text::Selected,
            Text::SaveQuestion,
            Text::SourceChanged,
            Text::Imported,
            Text::ImportSkipped,
        ];
        for &key in &keys {
            for language in Locale::ALL {
                set_locale(language);
                assert!(!text(key).is_empty(), "{language:?}: {key:?}");
                assert_eq!(locale(), language);
            }
        }
        set_locale(Locale::En);
        assert_eq!(Locale::from_code("ru"), Some(Locale::Ru));
        assert_eq!(Locale::from_code("EN"), Some(Locale::En));
        assert_eq!(Locale::from_code("de"), None);
    }
    fn status(library: bool) -> Status<'static> {
        Status {
            path: "",
            adapter: "Test adapter",
            count: 3,
            selected: 0,
            zoom: 1.,
            texture_bytes: 0,
            message: "",
            idle: true,
            dirty: false,
            library,
            pinned: false,
            can_pin: false,
        }
    }
    #[test]
    fn toolbar_does_not_flash_while_journaling() {
        let draw = |idle| {
            let ctx = Context::default();
            theme(&ctx);
            let mut output = ctx.run_ui(Default::default(), |ui| {
                let mut state = status(false);
                state.idle = idle;
                toolbar(ui, state, &mut false, &[]);
            });
            output.textures_delta.clear();
            format!("{:?}", output.shapes)
        };
        assert_eq!(draw(true), draw(false));
    }
    #[test]
    fn chrome_and_library_keep_a_usable_canvas_on_small_screens() {
        for size in [egui::vec2(1440., 900.), egui::vec2(390., 844.)] {
            for library in [false, true] {
                let ctx = Context::default();
                theme(&ctx);
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ui| {
                        toolbar(ui, status(library), &mut false, &[]);
                        if library {
                            library::Library::default().show(ui, &[]);
                        } else {
                            egui::CentralPanel::default().show(ui, |ui| {
                                assert!(ui.max_rect().height() > size.y / 2.);
                                let actions = empty_board(ui);
                                assert!(!actions.paste && !actions.open);
                            });
                        }
                    },
                );
                output.textures_delta.clear();
                assert!(
                    !ctx.tessellate(output.shapes, output.pixels_per_point)
                        .is_empty()
                );
            }
        }
    }
    #[test]
    fn idle_canvas_furniture_does_not_request_continuous_frames() {
        let ctx = Context::default();
        theme(&ctx);
        let mut delay = std::time::Duration::ZERO;
        for frame in 0..30 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1440., 900.),
                    )),
                    time: Some(frame as f64 / 60.),
                    ..Default::default()
                },
                |ui| {
                    toolbar(ui, status(false), &mut false, &[]);
                    egui::CentralPanel::default().show(ui, |ui| {
                        let viewport = ui.max_rect();
                        canvas::tools(ui.ctx(), viewport, &mut false);
                        canvas::minimap(
                            ui.ctx(),
                            viewport,
                            &[],
                            egui::Rect::from_min_max(egui::pos2(0.1, 0.1), egui::pos2(0.9, 0.9)),
                        );
                    });
                },
            );
            delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
            output.textures_delta.clear();
        }
        assert!(
            delay >= std::time::Duration::from_secs(1),
            "idle UI requested another immediate frame: {delay:?}"
        );
    }
}
