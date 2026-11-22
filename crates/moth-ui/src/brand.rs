// SPDX-License-Identifier: GPL-3.0-or-later
//! Bundled app marks: raw pixels avoid image decoding on the event-loop thread.

/// The moth with a gallery is the default; the engraved mark is optional.
pub fn rgba(alternative: bool) -> &'static [u8] {
    if alternative {
        include_bytes!("../../../assets/icons/engraved.rgba")
    } else {
        include_bytes!("../../../assets/icons/moth.rgba")
    }
}

/// Native desktop APIs which require encoded icon data use the same artwork.
pub fn png(alternative: bool) -> &'static [u8] {
    if alternative {
        include_bytes!("../../../docs/assets/icon/mothref-icon-engraved-512.png")
    } else {
        include_bytes!("../../../docs/assets/icon/mothref-icon-512.png")
    }
}

/// Updates the mark; returns whether native icons also need updating.
pub fn set(ctx: &egui::Context, alternative: bool) -> bool {
    ctx.data_mut(|data| {
        let id = egui::Id::unique("app_icon");
        let changed = data.get_temp::<bool>(id) != Some(alternative);
        data.insert_temp(id, alternative);
        changed
    })
}

/// The current window's choice, including before preferences have loaded.
pub fn alternative(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp(egui::Id::unique("app_icon")).unwrap_or(false))
}

pub(crate) fn paint(ui: &egui::Ui, rect: egui::Rect) {
    let alternative = alternative(ui.ctx());
    let id = egui::Id::unique(("app_icon_texture", alternative));
    let texture = ui
        .ctx()
        .data(|data| data.get_temp::<egui::TextureHandle>(id));
    let texture = texture.unwrap_or_else(|| {
        let texture = ui.ctx().load_texture(
            "app icon",
            egui::ColorImage::from_rgba_unmultiplied([64, 64], rgba(alternative)),
            egui::TextureOptions::LINEAR,
        );
        ui.ctx()
            .data_mut(|data| data.insert_temp(id, texture.clone()));
        texture
    });
    ui.painter().image(
        texture.id(),
        rect,
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
        crate::theme::IMAGE_TINT,
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn both_marks_have_native_pixels_and_the_choice_is_per_window() {
        for choice in [false, true] {
            assert_eq!(super::rgba(choice).len(), 64 * 64 * 4);
            assert!(super::png(choice).starts_with(b"\x89PNG\r\n\x1a\n"));
        }
        let first = egui::Context::default();
        let second = egui::Context::default();
        assert!(super::set(&first, true));
        assert!(!super::set(&first, true));
        assert!(super::alternative(&first));
        assert!(!super::alternative(&second));
    }
}
