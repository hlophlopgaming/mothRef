// SPDX-License-Identifier: GPL-3.0-or-later
//! Raw native pointer events stop here; gestures consume normalized events.
use winit::event::{ElementState, Force, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
/// A pointer's source is preserved for touch-sized hit testing and pen behavior.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// Preserves conventional mouse interaction intent.
    Mouse,
    /// Routes contacts through the gesture layer.
    Touch,
    /// Reserves explicit identity for backends that classify pen input.
    Pen,
}
/// Buttons map into actions without leaking winit into canvas logic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Updates contact location without starting another gesture.
    Move,
    /// Starts a contact lifecycle.
    Down,
    /// Completes a contact lifecycle.
    Up,
    /// Releases state when a native contact is interrupted.
    Cancel,
}
/// Logical points keep camera behavior independent of monitor density.
#[derive(Clone, Copy, Debug)]
pub struct Pointer {
    /// Associates updates with the same object across operations.
    pub id: u64,
    /// Preserves device intent for gesture and hit-test decisions.
    pub kind: Kind,
    /// Logical points decouple input from monitor density.
    pub position: [f32; 2],
    /// Preserves contact force for pressure-aware tools.
    pub pressure: f32,
    /// Keeps contact lifecycle independent of native event enums.
    pub phase: Phase,
    /// Carries pan intent independently of mouse button conventions.
    pub pan: bool,
}
/// Wheel zoom shares the same gesture layer as two-finger pinch.
pub enum Input {
    /// Keeps normalized contacts separate from camera zoom intent.
    Pointer(Pointer),
    /// Shares camera zoom behavior across native wheel sources.
    Zoom {
        /// Anchors zoom beneath the pointer.
        position: [f32; 2],
        /// Multiplicative scale is independent of wheel units.
        factor: f32,
    },
}
/// Tracks the mouse position because button events do not carry coordinates.
#[derive(Default)]
pub struct Adapter {
    position: [f32; 2],
    pan: bool,
}
impl Adapter {
    /// This is the only application code that reads raw native mouse events.
    pub fn event(&mut self, event: &WindowEvent, scale: f64) -> Option<Input> {
        let pointer = match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.position = [(position.x / scale) as f32, (position.y / scale) as f32];
                Pointer {
                    id: 0,
                    kind: Kind::Mouse,
                    position: self.position,
                    pressure: 1.,
                    phase: Phase::Move,
                    pan: self.pan,
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if !matches!(button, MouseButton::Left | MouseButton::Middle) {
                    return None;
                }
                self.pan = *button == MouseButton::Middle;
                Pointer {
                    id: 0,
                    kind: Kind::Mouse,
                    position: self.position,
                    pressure: 1.,
                    phase: if *state == ElementState::Pressed {
                        Phase::Down
                    } else {
                        Phase::Up
                    },
                    pan: self.pan,
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y * 40.,
                    MouseScrollDelta::PixelDelta(p) => (p.y / scale) as f32,
                };
                return Some(Input::Zoom {
                    position: self.position,
                    factor: (y * 0.002).exp(),
                });
            }
            WindowEvent::Touch(t) => Pointer {
                id: t.id,
                kind: Kind::Touch,
                position: [(t.location.x / scale) as f32, (t.location.y / scale) as f32],
                pressure: match t.force {
                    Some(Force::Normalized(v)) => v as f32,
                    Some(Force::Calibrated {
                        force,
                        max_possible_force,
                        ..
                    }) => (force / max_possible_force) as f32,
                    None => 1.,
                },
                phase: match t.phase {
                    TouchPhase::Started => Phase::Down,
                    TouchPhase::Moved => Phase::Move,
                    TouchPhase::Ended => Phase::Up,
                    TouchPhase::Cancelled => Phase::Cancel,
                },
                pan: false,
            },
            _ => return None,
        };
        Some(Input::Pointer(pointer))
    }
}
/// Gestures stay independent of the native event format.
#[derive(Default)]
pub struct Gestures {
    touches: Vec<(u64, [f32; 2])>,
}
/// Pan and pinch deltas can be applied directly to a camera.
pub struct Gesture {
    /// Carries pan intent independently of mouse button conventions.
    pub pan: [f32; 2],
    /// Keeps camera scale available to rendering and input.
    pub zoom: f32,
    /// Keeps pinch zoom stable beneath the contacts.
    pub anchor: [f32; 2],
}
impl Gestures {
    /// Keeps at most two active contacts; extra fingers cannot grow state indefinitely.
    pub fn update(&mut self, p: Pointer) -> Option<Gesture> {
        if p.kind != Kind::Touch {
            return None;
        }
        let before = pair(&self.touches);
        match p.phase {
            Phase::Down if self.touches.len() < 2 => self.touches.push((p.id, p.position)),
            Phase::Move => {
                if let Some(t) = self.touches.iter_mut().find(|t| t.0 == p.id) {
                    t.1 = p.position;
                }
            }
            Phase::Up | Phase::Cancel => self.touches.retain(|t| t.0 != p.id),
            _ => {}
        }
        let after = pair(&self.touches);
        match (before, after) {
            (Some((a, d)), Some((b, e))) if p.phase == Phase::Move => Some(Gesture {
                pan: [b[0] - a[0], b[1] - a[1]],
                zoom: if d > 1. { e / d } else { 1. },
                anchor: a,
            }),
            _ => None,
        }
    }
    /// Two contacts reserve the pointer stream for camera gestures.
    pub fn active(&self) -> bool {
        self.touches.len() == 2
    }
}
fn pair(t: &[(u64, [f32; 2])]) -> Option<([f32; 2], f32)> {
    if t.len() != 2 {
        return None;
    }
    let a = t[0].1;
    let b = t[1].1;
    Some((
        [(a[0] + b[0]) / 2., (a[1] + b[1]) / 2.],
        (a[0] - b[0]).hypot(a[1] - b[1]),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinch_doubles_distance() {
        let mut g = Gestures::default();
        let p = Pointer {
            id: 1,
            kind: Kind::Touch,
            position: [0., 0.],
            pressure: 1.,
            phase: Phase::Down,
            pan: false,
        };
        g.update(p);
        g.update(Pointer {
            id: 2,
            position: [10., 0.],
            ..p
        });
        let result = g
            .update(Pointer {
                id: 2,
                position: [20., 0.],
                phase: Phase::Move,
                ..p
            })
            .unwrap();
        assert_eq!(result.zoom, 2.);
        assert_eq!(result.pan, [5., 0.]);
    }
}
