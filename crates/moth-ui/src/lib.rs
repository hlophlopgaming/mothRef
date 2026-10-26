// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! Shared theme and localized M0 controls.
use egui::Context;
/// Centralized strings keep widgets independent of the selected locale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Text {
    /// Toggle board presentation chrome.
    ToggleView,
    /// View appearance popover.
    ViewAppearance,
    /// Image presentation opacity.
    ImageOpacity,
    /// Canvas presentation opacity.
    BoardOpacity,
    /// Whole window presentation opacity.
    WindowOpacity,
    /// Opaque canvas background.
    BackgroundSolid,
    /// Translucent canvas background.
    BackgroundTranslucent,
    /// Native blurred canvas background.
    BackgroundGlass,
    /// Blur unavailable on this backend.
    GlassFallback,
    /// Hide non-image content without editing it.
    ImagesOnly,
    /// Export destination, distinct from the native board save path.
    ExportPath,
    /// Short heading for color and coordinate tools.
    ImagePickers,
    /// Explains the armed color picker.
    PickColorHint,
    /// Explains the armed coordinate picker.
    PickCoordinatesHint,
    /// Native image picker entry point on an empty board.
    ChooseFiles,
    /// Import feedback count; translated when drawn.
    AddedImages,
    /// Recovery timestamp.
    ChangedNow,
    /// Recovery timestamp.
    ChangedMinutes,
    /// Recovery timestamp.
    ChangedHours,
    /// Recovery timestamp.
    ChangedDays,
    /// Localized UI control.
    FileMenu,
    /// Localized UI control.
    AlignGroup,
    /// Localized UI control.
    OrderGroup,
    /// Localized UI control.
    InsertImages,
    /// Localized UI control.
    OpenFile,
    /// Localized UI control.
    RestoreRecovery,
    /// Localized UI control.
    DiscardRecovery,
    /// Localized UI control.
    ImageOne,
    /// Localized UI control.
    ImageFew,
    /// Localized UI control.
    ImageMany,
    /// Localized UI control.
    NoteOne,
    /// Localized UI control.
    NoteFew,
    /// Localized UI control.
    NoteMany,
    /// Localized UI control.
    NewImageLayout,
    /// Localized UI control.
    ImageGap,
    /// Localized UI control.
    ManyImages,
    /// Localized UI control.
    PackingChoice,
    /// Localized UI control.
    PackingSplit,
    /// Localized UI control.
    ImportSkippedCount,

    /// Settings section for bindings and input.
    KeysSettings,
    /// Localized UI control.
    UnsavedChip,
    /// Localized UI control.
    EditNote,

    /// Localized UI control.
    GeneralSettings,
    /// Localized UI control.
    AppearanceSettings,
    /// Localized UI control.
    CanvasSettings,
    /// Localized UI control.
    AddingSettings,
    /// Localized UI control.
    WindowSettings,
    /// Localized UI control.
    PerformanceSettings,
    /// Localized UI control.
    IntegrationsSettings,
    /// Localized UI control.
    DeveloperSettings,
    /// Localized UI control.
    ResetSection,
    /// Localized UI control.
    Insert,
    /// Localized UI control.
    More,
    /// Localized UI control.
    EditItem,
    /// Localized UI control.
    ArrangeOptions,
    /// Localized UI control.
    ScaleApply,
    /// Localized UI control.
    ApplyItem,
    /// Localized UI control.
    CancelItem,
    /// Localized UI control.
    Images,
    /// Localized UI control.
    Language,
    /// Localized control.
    CheatSheet,
    /// Localized control.
    Help,
    /// Localized control.
    ShortcutFeedback,
    /// Localized control.
    ToastHold,
    /// Localized control.
    ToastDismiss,
    /// Localized control.
    ToastHint,
    /// Localized control.
    LayoutRows,
    /// Localized control.
    LayoutColumns,
    /// Localized control.
    VerticalHint,
    /// Localized control.
    HorizontalHint,
    /// Localized control.
    GridHint,
    /// Explains the gap exception for optimal packing.
    GapHint,
    /// Explains square packing without resizing.
    OptimalHint,
    /// Localized control.
    CategoryBoard,
    /// Localized control.
    CategoryView,
    /// Localized control.
    CategoryEdit,
    /// Localized control.
    CategoryPointer,

    /// Shortcut editor column.
    BindingAction,
    /// Shortcut editor column.
    BindingProposed,
    /// Shortcut editor column.
    BindingCurrent,
    /// Localized shortcut control.
    CommandPalette,
    /// Localized shortcut control.
    Shortcuts,
    /// Localized shortcut control.
    NextTab,
    /// Localized shortcut control.
    ZoomReset,
    /// Keyboard canvas navigation.
    /// Keyboard camera navigation preference.
    KeyboardPanMode,
    /// Canvas navigator visibility preference.
    ShowMinimap,
    /// Canvas navigator maximum side length preference.
    MinimapSize,
    /// Canvas navigator opacity preference.
    MinimapOpacity,
    /// Canvas navigator distance from center preference.
    MinimapDistance,
    /// Canvas navigator placement.
    MinimapPosition,
    /// Canvas navigator placement.
    PositionTopLeft,
    /// Canvas navigator placement.
    PositionTopCenter,
    /// Canvas navigator placement.
    PositionTopRight,
    /// Canvas navigator placement.
    PositionMiddleLeft,
    /// Canvas navigator placement.
    PositionCenter,
    /// Canvas navigator placement.
    PositionMiddleRight,
    /// Canvas navigator placement.
    PositionBottomLeft,
    /// Canvas navigator placement.
    PositionBottomCenter,
    /// Canvas navigator placement.
    PositionBottomRight,

    /// Keyboard camera navigation preference.
    PanStepped,
    /// Keyboard camera navigation preference.
    PanFlying,
    /// Keyboard camera navigation preference.
    KeyboardPanStep,
    /// Keyboard camera navigation preference.
    KeyboardPanSpeed,
    /// Keyboard camera navigation preference.
    Deselect,
    PanLeft,
    /// Keyboard canvas navigation.
    PanRight,
    /// Keyboard canvas navigation.
    PanUp,
    /// Keyboard canvas navigation.
    PanDown,

    /// Localized shortcut control.
    NudgeLeftFast,
    /// Localized shortcut control.
    NudgeRightFast,
    /// Localized shortcut control.
    NudgeUpFast,
    /// Localized shortcut control.
    NudgeDownFast,
    /// Localized shortcut control.
    ArrangeName,
    /// Localized shortcut control.
    ArrangeTime,
    /// Localized shortcut control.
    MoveDrag,
    /// Localized shortcut control.
    ExtendDrag,
    ToggleDrag,
    /// Localized shortcut control.
    DuplicateDrag,
    /// Localized shortcut control.
    PanDrag,
    /// Localized shortcut control.
    PanSpaceDrag,
    /// Localized shortcut control.
    PanAltDrag,
    /// Localized shortcut control.
    BindingHelp,
    /// Localized shortcut control.
    PresetHelp,
    /// Localized shortcut control.
    SearchActions,
    /// Localized shortcut control.
    NoActions,
    /// Localized shortcut control.
    InvalidBinding,
    /// Localized shortcut control.
    BindingConflict,
    /// Localized shortcut control.
    BindingPreview,
    /// Localized shortcut control.
    Commands,

    /// Toggle selection handles and hover outlines on the canvas.
    ShowOutlines,
    /// Toggle sway and lift while dragging items.
    DragAnimation,
    /// Live-preview opacity when decorative drag motion is disabled.
    DragOpacity,
    /// Disable decorative motion and inertial movement.
    ReducedMotion,
    /// Meaning of the preference difference marker.
    ChangedSetting,
    /// Stable arrangement order.
    ByLayer,
    /// Localized preference.
    ScaleWithWindow,
    /// Localized preference.
    SmoothZoom,
    ZoomIncrement,
    WheelZoom,
    /// Localized preference.
    WheelPan,
    /// Localized preference.
    WheelOff,

    /// Bounded in-memory undo depth.
    UndoLimit,
    /// Cache settings keep memory limits visible and adjustable.
    ImageCache,
    /// Current CPU cache bytes and retention ceiling.
    RamCacheUsage,
    /// Last disk-cache sample from the maintenance worker.
    DiskCacheUsage,
    /// Explicitly deletes disposable disk entries.
    ClearDiskCache,
    /// Maintenance has not completed yet.
    CachePending,
    /// Explicit cache ceiling in MiB.
    CacheLimit,
    /// Explains demand allocation and safe preview eviction.
    CacheHint,
    /// Chooses how decoded previews are kept between zoom steps.
    PerformanceMode,
    /// Performance mode: decoded pyramids in RAM.
    ModeMemory,
    /// Performance mode: decoded pyramids in RAM, capped detail.
    ModeMemoryLow,
    /// Performance mode: decoded pyramids on disk.
    ModeDisk,
    /// Performance mode: decode again for every detail level.
    ModeNoCache,
    /// Explains the in-memory mode.
    ModeMemoryHint,
    /// Explains the low-quality in-memory mode.
    ModeMemoryLowHint,
    /// Explains the disk cache mode.
    ModeDiskHint,
    /// Explains the uncached mode.
    ModeNoCacheHint,
    /// Performance mode: every image at full resolution in video memory.
    ModeFull,
    /// Explains the full-resolution mode.
    ModeFullHint,
    /// Label for the detected graphics adapter.
    VideoCard,
    /// Adapter memory shared with system RAM.
    VideoShared,
    /// Label for the texture limit derived from the adapter.
    VideoLimitIs,
    /// Follow the detected video memory instead of a manual limit.
    VideoLimitAuto,
    /// Shown until video memory detection finishes.
    VideoDetecting,
    /// Progress label while a board loads into video memory.
    PreloadProgress,
    /// Warns that the board exceeded the video memory limit.
    PreloadOverflow,
    /// Tells the user the limit was set from the adapter.
    VideoDetected,
    /// Chooses which large images get zoom tiles prepared in the background.
    BakeTiles,
    /// Tile preparation: images when they first appear on screen.
    BakeVisible,
    /// Tile preparation: every image of the open board.
    BakeBoard,
    /// Tile preparation disabled.
    BakeOff,
    /// Explains tile preparation and its disk cost.
    BakeHint,
    /// Count of images still waiting for tile preparation.
    BakePending,
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
    /// Localized Frame control.
    Frame,
    /// Localized Rotation control.
    Rotation,
    /// Localized SnapRotation control.
    SnapRotation,
    /// Localized PackBestArea control.
    PackBestArea,
    /// Localized PackBestShortSide control.
    PackBestShortSide,
    /// Localized PackBestLongSide control.
    PackBestLongSide,
    /// Localized PackWorstArea control.
    PackWorstArea,
    /// Localized PackWorstShortSide control.
    PackWorstShortSide,
    /// Localized PackWorstLongSide control.
    PackWorstLongSide,
    /// Localized SplitShorterLeftover control.
    SplitShorterLeftover,
    /// Localized SplitLongerLeftover control.
    SplitLongerLeftover,
    /// Localized SplitMinimizeArea control.
    SplitMinimizeArea,
    /// Localized SplitMaximizeArea control.
    SplitMaximizeArea,
    /// Localized SplitShorterAxis control.
    SplitShorterAxis,
    /// Localized SplitLongerAxis control.
    SplitLongerAxis,
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
    /// Open the embedded original with the system image viewer.
    OpenOriginal,
    /// Wrap text to frame width.
    NoteWrap,
    /// Text size.
    NoteFontSize,

    /// Image smoothing.
    ImageSampling,
    /// Use canvas setting.
    SamplingDefault,
    /// Smooth.
    SamplingSmooth,
    /// Sharp pixels (nearest neighbor).
    SamplingNearest,
    /// Automatic: sharp pixels at 200% and above.
    SamplingAutomatic,

    /// Confirmation after the system viewer accepts an image.
    ViewerOpened,
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
    /// Window movement action.
    MoveWindow,
    /// Armed native drag instructions.
    MoveWindowHint,
    /// Explicit titlebar reveal.
    WindowControls,
    /// Pin choice and its current state.
    AboveAll,
    /// Pin to a foreign window.
    AboveWindow,
    /// Target selection instructions.
    PickWindow,
    /// Restore a maximized window.
    Restore,
    /// Borderless fullscreen.
    Fullscreen,
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
    /// Offers a native working copy after foreign-board import.
    ConversionTitle,
    /// Explains original preservation and supported conversion.
    ConversionRetained,
    /// Lists PureRef conversion limitations.
    ConversionPur,
    /// Lists BeeRef conversion limitations.
    ConversionBee,
    /// Saves an imported working copy.
    SaveAsMoth,
    /// Dismisses the conversion offer without discarding edits.
    ContinueConversion,
    /// Explains that some imported items could not be represented.
    ImportSkipped,
}
mod localization;
pub use localization::{Locale, Message, item_counts, locale, set_locale, text, translated};

/// House theme tokens and application.
pub mod theme;
pub use theme::{Tokens, tokens};
/// Installs bundled fonts and the chosen house theme.
pub fn theme(ctx: &Context, tokens: &Tokens) {
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
    fonts.font_data.insert(
        "Bricolage Grotesque".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../fonts/BricolageGrotesque-Bold.ttf"
        ))),
    );
    fonts.families.insert(
        egui::FontFamily::Name("Bricolage Grotesque".into()),
        vec![
            "Bricolage Grotesque".into(),
            "Noto Sans".into(),
            "Hack".into(),
        ],
    );
    fonts.families.insert(
        egui::FontFamily::Proportional,
        vec!["Noto Sans".into(), "Hack".into()],
    );
    fonts
        .families
        .insert(egui::FontFamily::Monospace, vec!["Hack".into()]);
    ctx.set_fonts(fonts);
    theme::apply(ctx, tokens);
}
/// Canvas overlays and navigation widgets.
pub mod canvas;
mod chrome;
/// Reusable vector controls.
pub mod icons;
/// Responsive board catalog.
pub mod library;
pub use chrome::{Actions, Status, Tab, toolbar};
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
        crate::tokens(ui.ctx()).radius_l as f32,
        egui::Stroke::new(1., crate::tokens(ui.ctx()).empty_border),
        egui::StrokeKind::Inside,
    );
    let mut actions = Actions::default();
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(16.)), |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space((size.y * 0.17).max(0.));
            let (icon, _) = ui.allocate_exact_size(egui::vec2(38., 32.), egui::Sense::hover());
            let stroke = egui::Stroke::new(1.5, crate::tokens(ui.ctx()).empty_icon);
            ui.painter().rect_stroke(
                icon,
                crate::tokens(ui.ctx()).radius_picture as f32,
                stroke,
                egui::StrokeKind::Inside,
            );
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
            ui.add_space(crate::tokens(ui.ctx()).space_content as f32);
            ui.label(text(Text::Empty));
            ui.add_space(crate::tokens(ui.ctx()).space_xs as f32);
            ui.small(text(Text::Volatile));
            ui.add_space(crate::tokens(ui.ctx()).space_content as f32);
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - 230.) / 2.).max(0.));
                actions.open = ui
                    .add_sized([125., 36.], egui::Button::new(text(Text::ChooseFiles)))
                    .clicked();
                actions.paste = ui
                    .add_sized([105., 36.], egui::Button::new(text(Text::Paste)))
                    .clicked();
            });
        });
    });
    actions
}

/// Publishes labels derived from the application's single action registry.
pub fn set_shortcuts(ctx: &Context, labels: Vec<(Text, String)>) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("active_shortcuts"), labels));
}
/// Uses current bindings, including an explicitly unbound action.
pub fn tooltip(ctx: &Context, key: Text) -> String {
    let lookup = match key {
        Text::PasteImage => Text::Paste,
        _ => key,
    };
    let shortcut = ctx
        .data(|data| data.get_temp::<Vec<(Text, String)>>(egui::Id::new("active_shortcuts")))
        .and_then(|labels| {
            labels
                .into_iter()
                .find(|(label, _)| *label == lookup)
                .map(|(_, value)| value)
        })
        .unwrap_or_default();
    if shortcut.is_empty() {
        text(key).into()
    } else {
        format!("{} ({shortcut})", text(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proportional_font_falls_back_for_menu_and_change_arrows() {
        let ctx = Context::default();
        theme(&ctx, &Tokens::DARK);
        ctx.run_ui(Default::default(), |ui| {
            ui.fonts_mut(|fonts| {
                // has_glyph confuses the fallback face with replacement; compare actual glyph pixels.
                let mut raster = |character: char| {
                    let galley = fonts.layout_no_wrap(
                        character.to_string(),
                        egui::FontId::proportional(14.),
                        Tokens::DARK.text,
                    );
                    let glyph = &galley.rows[0].glyphs[0];
                    let image = fonts.image();
                    let [left, top] = glyph.uv_rect.min.map(usize::from);
                    let [right, bottom] = glyph.uv_rect.max.map(usize::from);
                    let pixels: Vec<_> = (top..bottom)
                        .flat_map(|y| {
                            image.pixels[y * image.size[0] + left..y * image.size[0] + right]
                                .iter()
                                .copied()
                        })
                        .collect();
                    ([right - left, bottom - top], pixels)
                };
                let replacement = raster('\u{10ffff}');
                for character in "→▸▾›−=,/…".chars() {
                    let glyph = raster(character);
                    assert!(!glyph.1.is_empty());
                    assert_ne!(glyph, replacement, "replacement rendered for {character}");
                }
            });
        })
        .textures_delta
        .clear();
    }
    #[test]
    fn shortcut_labels_follow_rebinding_and_unbinding() {
        let ctx = Context::default();
        set_shortcuts(&ctx, vec![(Text::Save, "Alt+F2".into())]);
        assert!(tooltip(&ctx, Text::Save).ends_with("(Alt+F2)"));
        set_shortcuts(&ctx, vec![(Text::Save, String::new())]);
        assert_eq!(tooltip(&ctx, Text::Save), text(Text::Save));
    }
    #[test]
    fn every_text_key_has_a_non_empty_translation_and_switches_locale() {
        let keys = [
            Text::ConversionTitle,
            Text::ConversionRetained,
            Text::ConversionPur,
            Text::ConversionBee,
            Text::SaveAsMoth,
            Text::ContinueConversion,
            Text::Settings,
            Text::GeneralSettings,
            Text::AppearanceSettings,
            Text::CanvasSettings,
            Text::AddingSettings,
            Text::WindowSettings,
            Text::PerformanceSettings,
            Text::IntegrationsSettings,
            Text::DeveloperSettings,
            Text::ResetSection,
            Text::Insert,
            Text::More,
            Text::EditItem,
            Text::ArrangeOptions,
            Text::ScaleApply,
            Text::ApplyItem,
            Text::CancelItem,
            Text::Images,
            Text::Language,
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
            Text::SmoothZoom,
            Text::ZoomIncrement,
            Text::WheelZoom,
            Text::WheelPan,
            Text::WheelOff,
            Text::UndoLimit,
            Text::ImageCache,
            Text::RamCacheUsage,
            Text::DiskCacheUsage,
            Text::ClearDiskCache,
            Text::CachePending,
            Text::CacheLimit,
            Text::CacheHint,
            Text::PerformanceMode,
            Text::ModeMemory,
            Text::ModeMemoryLow,
            Text::ModeDisk,
            Text::ModeNoCache,
            Text::ModeMemoryHint,
            Text::ModeMemoryLowHint,
            Text::ModeDiskHint,
            Text::ModeNoCacheHint,
            Text::ModeFull,
            Text::ModeFullHint,
            Text::VideoCard,
            Text::VideoShared,
            Text::VideoLimitIs,
            Text::VideoLimitAuto,
            Text::VideoDetecting,
            Text::PreloadProgress,
            Text::PreloadOverflow,
            Text::VideoDetected,
            Text::BakeTiles,
            Text::BakeVisible,
            Text::BakeBoard,
            Text::BakeOff,
            Text::BakeHint,
            Text::BakePending,
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
            images: 2,
            selected: 0,
            zoom: 1.,
            zoom_increment: 10.,
            texture_bytes: 0,
            texture_budget: 0,
            message: "",
            idle: true,
            dirty: false,
            library,
            pinned: false,
            can_pin: false,
            can_attach: false,
            target_pin: false,
            maximized: false,
        }
    }
    #[test]
    fn window_controls_reveal_without_hover_through_keyboard() {
        let draw = |events| {
            let ctx = Context::default();
            theme(&ctx, &Tokens::DARK);
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1440., 900.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    toolbar(ui, status(false), &[], |_| {});
                },
            );
            output.textures_delta.clear();
            output
                .shapes
                .iter()
                .filter(|s| !matches!(s.shape, egui::Shape::Noop))
                .count()
        };
        let hidden = draw(vec![]);
        let approach = draw(vec![egui::Event::PointerMoved(egui::pos2(700., 10.))]);
        let keyboard = draw(vec![egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert!(
            approach > hidden,
            "pointer approach must reveal native controls"
        );
        assert!(
            keyboard > hidden,
            "Tab must reveal controls without a pointer"
        );
    }
    #[test]
    fn persistence_chip_distinguishes_saved_and_unsaved_documents() {
        for (path, dirty, expected) in [
            ("board.moth", false, ["Saved", "Сохранено"]),
            ("board.moth", true, ["Not saved", "Не сохранено"]),
            ("", false, ["Not saved", "Не сохранено"]),
        ] {
            let ctx = Context::default();
            theme(&ctx, &Tokens::DARK);
            for _ in 0..2 {
                ctx.run_ui(Default::default(), |ui| {
                    let mut state = status(false);
                    state.path = path;
                    state.dirty = dirty;
                    toolbar(ui, state, &[], |_| {});
                })
                .textures_delta
                .clear();
            }
            let mut output = ctx.run_ui(Default::default(), |ui| {
                let mut state = status(false);
                state.path = path;
                state.dirty = dirty;
                toolbar(ui, state, &[], |_| {});
            });
            output.textures_delta.clear();
            let status = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text)
                        if expected
                            .iter()
                            .any(|prefix| text.galley.text().starts_with(prefix)) =>
                    {
                        Some(text)
                    }
                    _ => None,
                })
                .expect("the window bar reports saved or unsaved state");
            assert_eq!(status.galley.rows.len(), 1);
            assert!(
                ["2 images · 1 note", "2 изображения · 1 заметка"]
                    .iter()
                    .any(|counts| status.galley.text().contains(counts))
            );
        }
    }
    #[test]
    fn toolbar_does_not_flash_while_journaling() {
        let draw = |idle| {
            let ctx = Context::default();
            theme(&ctx, &Tokens::DARK);
            let mut output = ctx.run_ui(Default::default(), |ui| {
                let mut state = status(false);
                state.idle = idle;
                toolbar(ui, state, &[], |_| {});
            });
            output.textures_delta.clear();
            format!("{:?}", output.shapes)
        };
        assert_eq!(draw(true), draw(false));
    }
    #[test]
    fn empty_library_has_one_open_file_button() {
        let ctx = Context::default();
        theme(&ctx, &Tokens::LIGHT);
        let mut output = ctx.run_ui(Default::default(), |ui| {
            library::Library::default().show(ui, &[]);
        });
        let count = output.shapes.iter().filter(|shape| matches!(&shape.shape, egui::Shape::Text(text_shape) if text_shape.galley.text() == text(Text::OpenFile))).count();
        assert_eq!(count, 1);
        output.textures_delta.clear();
    }

    #[test]
    fn chrome_and_library_keep_a_usable_canvas_on_small_screens() {
        for size in [
            egui::vec2(1440., 900.),
            egui::vec2(390., 844.),
            egui::vec2(320., 640.),
            egui::vec2(844., 390.),
        ] {
            for (library, tab_count) in [(false, 0), (false, 1), (false, 4), (true, 0), (true, 4)] {
                let tabs: Vec<_> = (0..tab_count)
                    .map(|slot| Tab {
                        title: "A reference board with a long title",
                        dirty: true,
                        active: slot == 0,
                        slot,
                    })
                    .collect();
                let ctx = Context::default();
                theme(&ctx, &Tokens::DARK);
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ui| {
                        toolbar(ui, status(library), &tabs, |_| {});
                        if library {
                            library::Library::default().show(ui, &[]);
                        } else {
                            egui::CentralPanel::default().show(ui, |ui| {
                                assert!(
                                    ui.max_rect().height() > size.y / 2.,
                                    "{size:?}: {:?}; titlebar bottom: {:?}",
                                    ui.max_rect(),
                                    ui.ctx()
                                        .data(|d| d
                                            .get_temp::<f32>(egui::Id::new("window_bar_bottom")))
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
        theme(&ctx, &Tokens::DARK);
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
                    toolbar(ui, status(false), &[], |_| {});
                    egui::CentralPanel::default().show(ui, |ui| {
                        let viewport = ui.max_rect();
                        canvas::minimap(
                            ui.ctx(),
                            viewport,
                            &[],
                            egui::vec2(1., 1.),
                            canvas::Minimap {
                                size: Tokens::DARK.minimap_max,
                                alignment: egui::Align2::LEFT_BOTTOM,
                                opacity: 1.,
                                distance: 1.,
                            },
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
