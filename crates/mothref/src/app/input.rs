// SPDX-License-Identifier: GPL-3.0-or-later
//! Groups pointer adaptation, gesture recognition and current native input state.
use moth_platform::input::{Adapter, Gestures, Kind};

#[derive(Default)]
pub(super) struct InputState {
    pub adapter: Adapter,
    pub gestures: Gestures,
    pub cursor: Option<[f32; 2]>,
    pub hover: bool,
    pub command: bool,
    pub native_pressed: Option<(Kind, u64)>,
}
