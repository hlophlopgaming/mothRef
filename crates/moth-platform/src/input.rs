// SPDX-License-Identifier: GPL-3.0-or-later
//! Raw native pointer events stop here; gestures consume normalized events.
#[cfg(target_os = "linux")]
mod wayland;
use winit::event::{ElementState, Force, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
/// A pointer's source is preserved for touch-sized hit testing and pen behavior.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// Preserves conventional mouse interaction intent.
    Mouse,
    /// Routes contacts through the gesture layer.
    Touch,
    /// Preserves native tablet-tool identity and pressure.
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
    /// Wheel pan is normalized here alongside pointer input.
    Pan {
        /// Pointer anchor used to reject scrolling over UI.
        position: [f32; 2],
        /// Logical point delta independent of the native wheel units.
        delta: [f32; 2],
    },
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
/// Native extensions feed the same pointer and import paths as winit.
#[derive(Debug)]
pub enum NativeEvent {
    /// Preserves tablet identity without putting recognition in the gesture layer.
    Pointer(Pointer),
    /// Hands a local path to the existing background import worker.
    Drop(std::path::PathBuf),
    /// Surfaces native backend and transfer failures to the user.
    Error(String),
}
/// Tracks the mouse position because button events do not carry coordinates.
#[derive(Default)]
pub struct Adapter {
    position: [f32; 2],
    pan: bool,
    button: Option<MouseButton>,
    /// Wheel preference: 0 zoom, 1 pan, 2 disabled. Pinch remains available.
    pub wheel: u8,
    shift: bool,
    control: bool,
    #[cfg(target_os = "linux")]
    native: Option<wayland::Native>,
}
impl Adapter {
    /// Supplements winit where the native backend supplies richer input.
    pub fn attach(
        &mut self,
        window: std::sync::Arc<winit::window::Window>,
        display: winit::event_loop::OwnedDisplayHandle,
        wake: std::sync::Arc<dyn Fn() + Send + Sync>,
    ) -> std::io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            if self.native.is_none() {
                self.native = wayland::Native::new(window, display, wake)?;
            }
        }
        #[cfg(not(target_os = "linux"))]
        let _ = (window, display, wake);
        Ok(())
    }
    /// Drains a bounded native queue without waiting for device or file I/O.
    pub fn native(&self) -> impl Iterator<Item = NativeEvent> + '_ {
        #[cfg(target_os = "linux")]
        {
            self.native
                .iter()
                .flat_map(|native| native.events.try_iter())
                .take(256)
        }
        #[cfg(not(target_os = "linux"))]
        std::iter::empty()
    }
    /// This is the only application code that reads raw native mouse events.
    pub fn event(&mut self, event: &WindowEvent, scale: f64) -> Option<Input> {
        let pointer = match event {
            WindowEvent::ModifiersChanged(modifiers) => {
                self.shift = modifiers.state().shift_key();
                self.control = modifiers.state().control_key();
                return None;
            }
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
                match state {
                    ElementState::Pressed if self.button.is_none() => self.button = Some(*button),
                    ElementState::Released if self.button == Some(*button) => self.button = None,
                    _ => return None,
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
                let delta = match delta {
                    MouseScrollDelta::LineDelta(x, y) => [*x * 40., *y * 40.],
                    MouseScrollDelta::PixelDelta(p) => [(p.x / scale) as f32, (p.y / scale) as f32],
                };
                if self.shift || self.wheel == 1 {
                    return Some(Input::Pan {
                        position: self.position,
                        delta: if self.shift && self.control {
                            [delta[1], delta[0]]
                        } else {
                            delta
                        },
                    });
                }
                if self.wheel == 2 {
                    return None;
                }
                let y = delta[1];
                return Some(Input::Zoom {
                    position: self.position,
                    factor: (y * 0.002).exp(),
                });
            }
            WindowEvent::Focused(false) => {
                self.button = None;
                self.pan = false;
                Pointer {
                    id: 0,
                    kind: Kind::Mouse,
                    position: self.position,
                    pressure: 0.,
                    phase: Phase::Cancel,
                    pan: false,
                }
            }
            WindowEvent::Touch(t) => Pointer {
                id: t.id,
                kind: Kind::Touch,
                position: [(t.location.x / scale) as f32, (t.location.y / scale) as f32],
                pressure: match t.force {
                    Some(Force::Normalized(v)) => pressure(v),
                    Some(Force::Calibrated {
                        force,
                        max_possible_force,
                        ..
                    }) => pressure(force / max_possible_force),
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
            Phase::Down if self.touches.iter().any(|t| t.0 == p.id) => {}
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
    /// Focus loss must release contacts even when the OS sends no touch-up events.
    pub fn cancel(&mut self) {
        self.touches.clear();
    }
    /// Two contacts reserve the pointer stream for camera gestures.
    pub fn active(&self) -> bool {
        self.touches.len() == 2
    }
}
fn pressure(value: f64) -> f32 {
    if value.is_finite() {
        value.clamp(0., 1.) as f32
    } else {
        1.
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
    fn wheel_preferences_and_modifiers_route_through_normalized_pan() {
        let event = WindowEvent::MouseWheel {
            device_id: winit::event::DeviceId::dummy(),
            delta: MouseScrollDelta::LineDelta(1., 2.),
            phase: TouchPhase::Moved,
        };
        let mut adapter = Adapter::default();
        assert!(matches!(adapter.event(&event,1.),Some(Input::Zoom {factor,..}) if factor>1.));
        adapter.wheel = 1;
        assert!(matches!(
            adapter.event(&event, 1.),
            Some(Input::Pan {
                delta: [40., 80.],
                ..
            })
        ));
        adapter.wheel = 2;
        assert!(adapter.event(&event, 1.).is_none());
        adapter.event(
            &WindowEvent::ModifiersChanged(winit::keyboard::ModifiersState::SHIFT.into()),
            1.,
        );
        assert!(matches!(
            adapter.event(&event, 1.),
            Some(Input::Pan {
                delta: [40., 80.],
                ..
            })
        ));
        adapter.event(
            &WindowEvent::ModifiersChanged(
                (winit::keyboard::ModifiersState::SHIFT | winit::keyboard::ModifiersState::CONTROL)
                    .into(),
            ),
            1.,
        );
        assert!(matches!(
            adapter.event(&event, 1.),
            Some(Input::Pan {
                delta: [80., 40.],
                ..
            })
        ));
    }
    #[test]
    fn releasing_another_button_does_not_end_pan() {
        let mut adapter = Adapter::default();
        let button = |state, button| WindowEvent::MouseInput {
            device_id: winit::event::DeviceId::dummy(),
            state,
            button,
        };
        assert!(
            adapter
                .event(&button(ElementState::Pressed, MouseButton::Middle), 1.)
                .is_some()
        );
        assert!(
            adapter
                .event(&button(ElementState::Pressed, MouseButton::Left), 1.)
                .is_none()
        );
        assert!(
            adapter
                .event(&button(ElementState::Released, MouseButton::Left), 1.)
                .is_none()
        );
        assert!(matches!(
            adapter.event(&button(ElementState::Released, MouseButton::Middle), 1.),
            Some(Input::Pointer(Pointer {
                phase: Phase::Up,
                pan: true,
                ..
            }))
        ));
        adapter.event(&button(ElementState::Pressed, MouseButton::Middle), 1.);
        assert!(matches!(
            adapter.event(&WindowEvent::Focused(false), 1.),
            Some(Input::Pointer(Pointer {
                phase: Phase::Cancel,
                ..
            }))
        ));
        assert!(
            adapter
                .event(&button(ElementState::Pressed, MouseButton::Left), 1.)
                .is_some()
        );
    }
    #[test]
    fn touch_adapter_keeps_kind_force_scale_and_lifecycle() {
        let mut adapter = Adapter::default();
        for (native, expected) in [
            (TouchPhase::Started, Phase::Down),
            (TouchPhase::Moved, Phase::Move),
            (TouchPhase::Ended, Phase::Up),
            (TouchPhase::Cancelled, Phase::Cancel),
        ] {
            let event = WindowEvent::Touch(winit::event::Touch {
                device_id: winit::event::DeviceId::dummy(),
                phase: native,
                location: winit::dpi::PhysicalPosition::new(40., 80.),
                force: Some(Force::Normalized(0.375)),
                id: 42,
            });
            let Some(Input::Pointer(p)) = adapter.event(&event, 2.) else {
                panic!("touch must normalize to a pointer");
            };
            assert_eq!((p.id, p.kind, p.phase), (42, Kind::Touch, expected));
            assert_eq!(p.position, [20., 40.]);
            assert_eq!(p.pressure, 0.375);
        }
    }
    #[test]
    fn pressure_is_finite_and_normalized() {
        for (value, expected) in [
            (0.25, 0.25),
            (-1., 0.),
            (2., 1.),
            (f64::NAN, 1.),
            (f64::INFINITY, 1.),
        ] {
            assert_eq!(pressure(value), expected);
        }
    }
    #[test]
    fn duplicate_contacts_and_focus_loss_do_not_leave_a_pinch() {
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
        g.update(p);
        assert!(!g.active());
        g.update(Pointer { id: 2, ..p });
        assert!(g.active());
        g.cancel();
        assert!(!g.active());
        assert!(
            g.update(Pointer {
                phase: Phase::Move,
                ..p
            })
            .is_none()
        );
    }
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
