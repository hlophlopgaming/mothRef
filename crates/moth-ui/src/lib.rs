// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! Shared theme and localized M0 controls.
use egui::{Color32, Context};
/// Centralized strings keep widgets independent of the selected locale.
#[derive(Clone, Copy, Debug)]
pub enum Text {
    /// Disable decorative motion and inertial movement.
    ReducedMotion,
    /// Meaning of the preference difference marker.
    ChangedSetting,
    /// Stable arrangement order.
    ByLayer,
    /// Localized preference.
    ScaleWithWindow,
    /// Localized preference.
    WheelZoom,
    /// Localized preference.
    WheelPan,
    /// Localized preference.
    WheelOff,

    /// Bounded in-memory undo depth.
    UndoLimit,
    /// Localized Duplicate control.
    Duplicate,
    /// Localized NudgeLeft control.
    NudgeLeft,
    /// Localized NudgeRight control.
    NudgeRight,
    /// Localized NudgeUp control.
    NudgeUp,
    /// Localized NudgeDown control.
    NudgeDown,
    /// Localized AlignLeft control.
    AlignLeft,
    /// Localized AlignRight control.
    AlignRight,
    /// Localized AlignTop control.
    AlignTop,
    /// Localized AlignBottom control.
    AlignBottom,
    /// Localized DistributeHorizontal control.
    DistributeHorizontal,
    /// Localized DistributeVertical control.
    DistributeVertical,
    /// Localized Stack control.
    Stack,
    /// Localized BringFront control.
    BringFront,
    /// Localized SendBack control.
    SendBack,
    /// Localized BringForward control.
    BringForward,
    /// Localized SendBackward control.
    SendBackward,
    /// Localized LockCanvas control.
    LockCanvas,
    /// Localized DarkTheme control.
    DarkTheme,
    /// Localized UiScale control.
    UiScale,
    /// Localized Settings control.
    Settings,
    /// Localized Defaults control.
    Defaults,
    /// Localized ByTime control.
    ByTime,
    /// Localized Random control.
    Random,
    /// Localized ArrangeThreshold control.
    ArrangeThreshold,
    /// Localized AutoArrange control.
    AutoArrange,
    /// Localized ResetRotation control.
    ResetRotation,
    /// Localized ResetFlip control.
    ResetFlip,
    /// Localized ResetCrop control.
    ResetCrop,
    /// Localized ResetScale control.
    ResetScale,
    /// Localized NormalizeScale control.
    NormalizeScale,
    /// Localized ResetCamera control.
    ResetCamera,
    /// Localized NextImage control.
    NextImage,
    /// Localized PreviousImage control.
    PreviousImage,
    /// Localized CanvasGrayscale control.
    CanvasGrayscale,
    /// Localized PackFit control.
    PackFit,
    /// Localized CoordinatePicker control.
    CoordinatePicker,
    /// Localized ExportZip control.
    ExportZip,

    /// Durable checkpoint status.
    SaveState,
    /// Durable edits awaiting a checkpoint.
    Journaled,
    /// Recovery protection is enabled.
    RecoveryOn,
    /// Camera controls.
    ZoomIn,
    /// Camera controls.
    ZoomOut,
    /// Editable camera percentage.
    Zoom,
    /// Localized Tools control.
    Tools,
    /// Localized Frame control.
    Frame,
    /// Localized Rotation control.
    Rotation,
    /// Localized Content control.
    Content,
    /// Localized ContentScale control.
    ContentScale,
    /// Localized OffsetX control.
    OffsetX,
    /// Localized OffsetY control.
    OffsetY,
    /// Localized Grayscale control.
    Grayscale,
    /// Localized Gap control.
    Gap,
    /// Localized ByFilename control.
    ByFilename,
    /// Localized ScaleSelection control.
    ScaleSelection,
    /// Localized Export control.
    Export,
    /// Localized Exported control.
    Exported,
    /// Localized ExportScene control.
    ExportScene,
    /// Localized ExportSelection control.
    ExportSelection,
    /// Localized ExportOriginal control.
    ExportOriginal,
    /// Localized ExportOriginals control.
    ExportOriginals,
    /// Localized CopyImage control.
    CopyImage,
    /// Localized SampleColor control.
    SampleColor,
    /// Localized ExportSide control.
    ExportSide,
    /// Localized ExportLimit control.
    ExportLimit,
    /// Localized CropRegion control.
    CropRegion,
    /// Localized Width control.
    Width,
    /// Localized Height control.
    Height,
    /// Localized PositionX control.
    PositionX,
    /// Localized PositionY control.
    PositionY,
    /// Localized EditSelection control.
    EditSelection,

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
    /// Creates a text note at the cursor.
    AddNote,
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
    /// Confirms that a foreign board was imported.
    Imported,
    /// Explains that some imported items could not be represented.
    ImportSkipped,
}
mod localization;
pub use localization::{Locale, locale, set_locale, text};

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

/// Uses the active platform modifier and only advertises implemented bindings.
pub fn tooltip(key: Text) -> String {
    let shortcut = match key {
        Text::NewBoard => "Ctrl+N",
        Text::Open => "Ctrl+O",
        Text::Save => "Ctrl+S",
        Text::Undo => "Ctrl+Z",
        Text::Redo => "Ctrl+Y",
        Text::Paste | Text::PasteImage => "Ctrl+V",
        Text::Library => "Ctrl+L",
        Text::CloseTab => "Ctrl+W",
        Text::Overlay => "Ctrl+T",
        Text::Select => "V",
        Text::Pan => "H",
        Text::Fit => "Home",
        _ => "",
    };
    if shortcut.is_empty() {
        text(key).into()
    } else {
        let shortcut = if egui::os::OperatingSystem::default() == egui::os::OperatingSystem::Mac {
            shortcut.replace("Ctrl", "⌘")
        } else {
            shortcut.into()
        };
        format!("{} ({shortcut})", text(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_text_key_has_a_non_empty_translation_and_switches_locale() {
        let keys = [
            Text::Tools,
            Text::Frame,
            Text::Rotation,
            Text::Content,
            Text::ContentScale,
            Text::OffsetX,
            Text::OffsetY,
            Text::Grayscale,
            Text::Gap,
            Text::ByFilename,
            Text::ScaleSelection,
            Text::Export,
            Text::ScaleWithWindow,
            Text::WheelZoom,
            Text::WheelPan,
            Text::WheelOff,
            Text::UndoLimit,
            Text::Duplicate,
            Text::NudgeLeft,
            Text::NudgeRight,
            Text::NudgeUp,
            Text::NudgeDown,
            Text::AlignLeft,
            Text::AlignRight,
            Text::AlignTop,
            Text::AlignBottom,
            Text::DistributeHorizontal,
            Text::DistributeVertical,
            Text::Stack,
            Text::BringFront,
            Text::SendBack,
            Text::BringForward,
            Text::SendBackward,
            Text::LockCanvas,
            Text::DarkTheme,
            Text::UiScale,
            Text::Settings,
            Text::Defaults,
            Text::ByTime,
            Text::Random,
            Text::ArrangeThreshold,
            Text::AutoArrange,
            Text::ResetRotation,
            Text::ResetFlip,
            Text::ResetCrop,
            Text::ResetScale,
            Text::NormalizeScale,
            Text::ResetCamera,
            Text::NextImage,
            Text::PreviousImage,
            Text::CanvasGrayscale,
            Text::PackFit,
            Text::CoordinatePicker,
            Text::ExportZip,
            Text::ChangedSetting,
            Text::ByLayer,
            Text::Exported,
            Text::ExportScene,
            Text::ExportSelection,
            Text::ExportOriginal,
            Text::ExportOriginals,
            Text::CopyImage,
            Text::SampleColor,
            Text::ExportSide,
            Text::ExportLimit,
            Text::CropRegion,
            Text::Width,
            Text::Height,
            Text::PositionX,
            Text::PositionY,
            Text::EditSelection,
            Text::StorageBusy,
            Text::OpenEmpty,
            Text::Recovered,
            Text::Opened,
            Text::SaveState,
            Text::Journaled,
            Text::RecoveryOn,
            Text::ZoomIn,
            Text::ZoomOut,
            Text::Zoom,
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
            Text::AddNote,
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
        set_locale(Locale::EN);
        assert_eq!(Locale::from_code("ru"), Some(Locale::RU));
        assert_eq!(Locale::from_code("EN"), Some(Locale::EN));
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
    fn persistence_status_distinguishes_saved_journaled_and_untitled() {
        for (path, dirty, expected) in [
            ("board.moth", false, ["saved", "сохранено"]),
            ("board.moth", true, ["journaled", "в журнале"]),
            ("", false, ["recovery on", "восстановление включено"]),
        ] {
            let ctx = Context::default();
            let mut output = ctx.run_ui(Default::default(), |ui| {
                let mut state = status(false);
                state.path = path;
                state.dirty = dirty;
                toolbar(ui, state, &mut false, &[]);
            });
            output.textures_delta.clear();
            assert!(output.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(t) if expected.contains(&t.galley.text()))
            }));
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
        for size in [
            egui::vec2(1440., 900.),
            egui::vec2(390., 844.),
            egui::vec2(320., 640.),
            egui::vec2(844., 390.),
        ] {
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
                                assert!(
                                    ui.max_rect().height() > size.y / 2.,
                                    "{size:?}: {:?}",
                                    ui.max_rect()
                                );
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

/// Time-based visual transitions and repaint policy.
pub mod motion;
