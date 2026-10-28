// SPDX-License-Identifier: GPL-3.0-or-later
//! A floating window level alone stays on its own Space and below full-screen apps,
//! so "above all windows" also has to follow the user across Spaces.
use objc2_app_kit::{NSView, NSWindowCollectionBehavior};
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};

/// Shows a pinned window on every Space, including next to full-screen apps.
pub(crate) fn float_everywhere(window: &Window, enabled: bool) {
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    // SAFETY: winit keeps `ns_view` alive for the window's lifetime, and window
    // effects are applied on the event-loop (main) thread that AppKit requires.
    let view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
    let Some(ns_window) = view.window() else {
        return;
    };
    let ours = NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::FullScreenAuxiliary;
    let mut behavior = ns_window.collectionBehavior();
    if enabled {
        // AppKit rejects these in combination with the flags above.
        behavior.remove(
            NSWindowCollectionBehavior::MoveToActiveSpace
                | NSWindowCollectionBehavior::FullScreenPrimary,
        );
        behavior.insert(ours);
    } else {
        behavior.remove(ours);
    }
    ns_window.setCollectionBehavior(behavior);
}
