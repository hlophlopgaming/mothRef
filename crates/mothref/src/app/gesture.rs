// SPDX-License-Identifier: GPL-3.0-or-later
//! What the pointer does on the canvas: the active [`Tool`] decides what a press
//! starts, and the [`Gesture`] is the one thing a press is doing until release.
//!
//! A gesture is a single value, so the canvas can never be panning and moving
//! items at once, and each variant carries the pointer that owns it. New tools
//! (freehand, shapes, eraser) add a tool and, where a press does something new,
//! a gesture variant, instead of more flags beside the old ones.
use moth_platform::input::Kind;
use moth_scene::{Rect, Uuid};

/// What a primary press on the canvas does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Tool {
    /// Select, move and resize items; drag on empty canvas draws a selection.
    #[default]
    Select,
    /// The next click on an image reports the color under it.
    PickColor,
    /// The next click on an image reports its coordinates and details.
    PickInfo,
}
impl Tool {
    /// One-shot tools fall back to selection after their click.
    pub(super) fn picks(self) -> bool {
        matches!(self, Tool::PickColor | Tool::PickInfo)
    }
}

/// One press on the canvas, owned by the pointer that started it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Press {
    pub(super) owner: (Kind, u64),
    /// Screen point of the press; re-projected from `anchor` when the camera moves.
    pub(super) start: [f32; 2],
    pub(super) last: [f32; 2],
    /// World point of the press, so zooming mid-drag keeps the drag glued to the board.
    pub(super) anchor: [f32; 2],
}

#[derive(Default)]
pub(super) enum Gesture {
    #[default]
    Idle,
    /// The camera follows the pointer.
    Pan(Press),
    /// A selection rectangle from the press point.
    Marquee(Press),
    /// The selection follows the pointer; `grabbed` is the item under the press.
    Move {
        press: Press,
        grabbed: (Uuid, Rect),
        /// Drops copies and leaves the originals in place.
        duplicate: bool,
    },
    /// Handles of the selection frame scale or crop the selection.
    Resize {
        owner: (Kind, u64),
        resize: super::resize::Resize,
    },
    /// Vertical drag zooms around the press point.
    ZoomDrag {
        owner: (Kind, u64),
        anchor: [f32; 2],
        last: f32,
    },
}
impl Gesture {
    /// The pointer that owns the gesture; presses of other pointers are ignored.
    pub(super) fn owner(&self) -> Option<(Kind, u64)> {
        match self {
            Gesture::Idle => None,
            Gesture::Pan(press) | Gesture::Marquee(press) | Gesture::Move { press, .. } => {
                Some(press.owner)
            }
            Gesture::Resize { owner, .. } | Gesture::ZoomDrag { owner, .. } => Some(*owner),
        }
    }
    pub(super) fn press(&self) -> Option<&Press> {
        match self {
            Gesture::Pan(press) | Gesture::Marquee(press) | Gesture::Move { press, .. } => {
                Some(press)
            }
            _ => None,
        }
    }
    pub(super) fn press_mut(&mut self) -> Option<&mut Press> {
        match self {
            Gesture::Pan(press) | Gesture::Marquee(press) | Gesture::Move { press, .. } => {
                Some(press)
            }
            _ => None,
        }
    }
    /// `(start, last, pan)` of a press-and-drag, the shape the canvas and drag
    /// previews draw from.
    pub(super) fn drag(&self) -> Option<([f32; 2], [f32; 2], bool)> {
        self.press()
            .map(|press| (press.start, press.last, matches!(self, Gesture::Pan(_))))
    }
    pub(super) fn grabbed(&self) -> Option<(Uuid, Rect)> {
        match self {
            Gesture::Move { grabbed, .. } => Some(*grabbed),
            _ => None,
        }
    }
    pub(super) fn duplicate(&self) -> bool {
        matches!(
            self,
            Gesture::Move {
                duplicate: true,
                ..
            }
        )
    }
    pub(super) fn resize(&self) -> Option<&super::resize::Resize> {
        match self {
            Gesture::Resize { resize, .. } => Some(resize),
            _ => None,
        }
    }
}

/// What a finished gesture leaves on screen until the journal catches up.
#[derive(Default)]
pub(super) struct Settling {
    /// A released resize whose previews stay until its commands are acknowledged.
    pub(super) resize: Option<super::resize::Resize>,
    /// The last item press dropped copies; their originals keep a plain outline
    /// while the copies land.
    pub(super) copies: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press() -> Press {
        Press {
            owner: (Kind::Mouse, 0),
            start: [1., 2.],
            last: [3., 4.],
            anchor: [0., 0.],
        }
    }
    #[test]
    fn views_follow_the_variant() {
        assert_eq!(Gesture::Idle.drag(), None);
        assert_eq!(
            Gesture::Pan(press()).drag(),
            Some(([1., 2.], [3., 4.], true))
        );
        assert_eq!(
            Gesture::Marquee(press()).drag(),
            Some(([1., 2.], [3., 4.], false))
        );
        let rect = Rect {
            x: 0.,
            y: 0.,
            width: 1.,
            height: 1.,
        };
        let moving = Gesture::Move {
            press: press(),
            grabbed: (Uuid::nil(), rect),
            duplicate: true,
        };
        assert_eq!(moving.drag(), Some(([1., 2.], [3., 4.], false)));
        assert_eq!(moving.grabbed(), Some((Uuid::nil(), rect)));
        assert!(moving.duplicate());
        assert_eq!(moving.owner(), Some((Kind::Mouse, 0)));
        let zoom = Gesture::ZoomDrag {
            owner: (Kind::Pen, 7),
            anchor: [0., 0.],
            last: 0.,
        };
        assert_eq!(zoom.drag(), None);
        assert_eq!(zoom.owner(), Some((Kind::Pen, 7)));
        assert!(Tool::PickColor.picks() && !Tool::Select.picks());
    }
}
