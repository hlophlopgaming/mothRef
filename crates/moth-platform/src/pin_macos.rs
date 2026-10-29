// SPDX-License-Identifier: GPL-3.0-or-later
//! macOS: the board follows a chosen application. App activation is announced
//! by `NSWorkspace` notifications, so nothing polls while pinned. When the
//! chosen app becomes active its windows come to the front, and the board is
//! ordered in front of them without activating MothRef. Popups and panels of
//! that app are new windows ordered above the board, so they stay usable.
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject};
use objc2_app_kit::{
    NSRunningApplication, NSView, NSWindow, NSWorkspace, NSWorkspaceApplicationKey,
    NSWorkspaceDidActivateApplicationNotification, NSWorkspaceDidTerminateApplicationNotification,
};
use objc2_foundation::NSNotification;
use std::ptr::NonNull;
use std::sync::Arc;
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};

type Observer = Retained<ProtocolObject<dyn NSObjectProtocol>>;

#[derive(Default)]
pub(super) struct Pin {
    /// Process id of the chosen application.
    target: Option<i32>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
    observers: Vec<Observer>,
}

fn board(window: &Window) -> Option<Retained<NSWindow>> {
    let RawWindowHandle::AppKit(handle) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    // SAFETY: winit keeps `ns_view` alive for the window's lifetime, and pinning
    // runs on the event-loop (main) thread that AppKit requires.
    let view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
    view.window()
}

fn application(notification: NonNull<NSNotification>) -> Option<i32> {
    // SAFETY: AppKit passes a valid notification for the duration of the call.
    let notification = unsafe { notification.as_ref() };
    let info = notification.userInfo()?;
    // SAFETY: the key is an AppKit constant string.
    let key: &AnyObject = unsafe { NSWorkspaceApplicationKey };
    let app = info
        .objectForKey(key)?
        .downcast::<NSRunningApplication>()
        .ok()?;
    Some(app.processIdentifier())
}

impl Pin {
    pub(super) fn new(wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            target: None,
            wake: Some(wake),
            observers: Vec::new(),
        }
    }
    pub(super) fn available(&self) -> bool {
        self.target.is_some_and(|pid| {
            NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
                .is_some_and(|app| !app.isTerminated())
        })
    }
    pub(super) fn enable(&mut self, window: &Window, enabled: bool) -> Result<(), String> {
        self.stop();
        if !enabled {
            return Ok(());
        }
        let target = self.target.ok_or("No target window selected")?;
        if !self.available() {
            return Err("The chosen window has closed".into());
        }
        let board = board(window).ok_or("Missing native window")?;
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        let raise = board.clone();
        let activated = block2::RcBlock::new(move |notification: NonNull<NSNotification>| {
            if application(notification) == Some(target) {
                raise.orderFrontRegardless();
            }
        });
        let wake = self.wake.clone();
        let terminated = block2::RcBlock::new(move |notification: NonNull<NSNotification>| {
            if application(notification) == Some(target)
                && let Some(wake) = &wake
            {
                wake();
            }
        });
        // SAFETY: AppKit constant names, no object filter, delivered on the posting
        // (main) thread; observers are removed in `stop` before being dropped.
        unsafe {
            self.observers
                .push(center.addObserverForName_object_queue_usingBlock(
                    Some(NSWorkspaceDidActivateApplicationNotification),
                    None,
                    None,
                    &activated,
                ));
            self.observers
                .push(center.addObserverForName_object_queue_usingBlock(
                    Some(NSWorkspaceDidTerminateApplicationNotification),
                    None,
                    None,
                    &terminated,
                ));
        }
        if NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .is_some_and(|app| app.processIdentifier() == target)
        {
            board.orderFrontRegardless();
        }
        Ok(())
    }
    /// Polled by the app only while the user is choosing: the next application
    /// that becomes frontmost, other than MothRef, is the target.
    pub(super) fn pick(&mut self, _window: &Window) -> Option<bool> {
        let own = std::process::id() as i32;
        let pid = NSWorkspace::sharedWorkspace()
            .frontmostApplication()?
            .processIdentifier();
        if pid <= 0 || pid == own {
            return None;
        }
        self.target = Some(pid);
        Some(true)
    }
    fn stop(&mut self) {
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        for observer in self.observers.drain(..) {
            // SAFETY: each observer was returned by this notification center.
            unsafe { center.removeObserver(observer.as_ref()) };
        }
    }
}
impl Drop for Pin {
    fn drop(&mut self) {
        self.stop();
    }
}
