use std::collections::{HashMap, HashSet};

use redixel_core::{
    game::{InputBind, InputQuery},
    input::{InputAction, InputSource, KeyState},
};
use redixel_math::Vec2;
use winit::{
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

/// Owns all input state for the current frame and the action→source bindings.
///
/// # Lifecycle (called by the Runtime each frame)
/// ```text
/// process_event(e)  ← called for each OS window event (buffers events safely)
/// tick()            ← flushes buffer, advances JustPressed→Held, JustReleased→Up
/// [game on_update]  ← game queries just_pressed / held / just_released
/// ```
///
/// This manager utilizes a Double Buffer strategy to prevent "Phantom Clicks"
/// (where rapid press/release events within the same frame are swallowed).
pub struct InputManager<A: InputAction> {
    key_states:              HashMap<KeyCode, KeyState>,
    mouse_states:            HashMap<MouseButton, KeyState>,
    mouse_position:          Option<Vec2>,
    pending_keys:            Vec<(KeyCode, ElementState)>,
    pending_mouse:           Vec<(MouseButton, ElementState)>,
    deferred_key_releases:   Vec<(KeyCode, ElementState)>,
    deferred_mouse_releases: Vec<(MouseButton, ElementState)>,
    scroll_accumulator:      Vec2,
    scroll_delta:            Vec2,
    bindings:                HashMap<A, HashSet<InputSource>>,
}

impl<A: InputAction> InputManager<A> {
    pub fn new() -> Self {
        Self {
            key_states:              HashMap::new(),
            mouse_states:            HashMap::new(),
            mouse_position:          None,
            pending_keys:            Vec::new(),
            pending_mouse:           Vec::new(),
            deferred_key_releases:   Vec::new(),
            deferred_mouse_releases: Vec::new(),
            scroll_accumulator:      Vec2::ZERO,
            scroll_delta:            Vec2::ZERO,
            bindings:                HashMap::new(),
        }
    }

    /// Advances the input state machine at the start of a new frame.
    ///
    /// This method flushes the event queues gathered by `process_event`. It also
    /// guarantees that any rapid "Press and Release" combination happening within
    /// the exact same frame boundary is deferred to the next frame to prevent
    /// engine blindness.
    pub fn tick(&mut self) {
        Self::advance_states(&mut self.key_states);
        Self::advance_states(&mut self.mouse_states);
        self.deferred_mouse_releases.clear();

        for (btn, state) in self.pending_mouse.drain(..) {
            match state {
                ElementState::Pressed => {
                    self.mouse_states.insert(btn, KeyState::JustPressed);
                }
                ElementState::Released => {
                    if self.mouse_states.get(&btn) == Some(&KeyState::JustPressed) {
                        self.deferred_mouse_releases.push((btn, state));
                    } else {
                        self.mouse_states.insert(btn, KeyState::JustReleased);
                    }
                }
            }
        }

        self.pending_mouse.append(&mut self.deferred_mouse_releases);
        self.deferred_key_releases.clear();

        for (code, state) in self.pending_keys.drain(..) {
            match state {
                ElementState::Pressed => {
                    self.key_states.insert(code, KeyState::JustPressed);
                }
                ElementState::Released => {
                    if self.key_states.get(&code) == Some(&KeyState::JustPressed) {
                        self.deferred_key_releases.push((code, state));
                    } else {
                        self.key_states.insert(code, KeyState::JustReleased);
                    }
                }
            }
        }

        self.pending_keys.append(&mut self.deferred_key_releases);
        self.scroll_delta = self.scroll_accumulator;
        self.scroll_accumulator = Vec2::ZERO;
    }

    /// Feeds a raw OS window event into the input manager's double buffer.
    ///
    /// Returns `true` if the event was successfully consumed (keyboard, mouse button,
    /// cursor move, or scroll) and should not be forwarded further.
    pub fn process_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput { event: key_event, .. } => {
                if let PhysicalKey::Code(code) = key_event.physical_key
                    && !key_event.repeat
                {
                    self.pending_keys.push((code, key_event.state));
                }

                true
            }

            WindowEvent::PointerButton { state, button, .. } => {
                let mouse_btn: Option<MouseButton> = match button {
                    winit::event::ButtonSource::Mouse(btn) => Some(*btn),
                    winit::event::ButtonSource::Touch { .. } => Some(MouseButton::Left),
                    _ => None,
                };

                if let Some(btn) = mouse_btn {
                    self.pending_mouse.push((btn, *state));
                }

                true
            }

            WindowEvent::PointerMoved { position, .. } => {
                self.mouse_position = Some(Vec2::new(position.x as f32, position.y as f32));
                true
            }

            WindowEvent::PointerLeft { .. } => {
                self.mouse_position = None;
                true
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy): (f32, f32) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                    MouseScrollDelta::PixelDelta(px) => (px.x as f32 / 20.0, px.y as f32 / 20.0),
                };

                self.scroll_accumulator += Vec2::new(dx, dy);
                true
            }

            _ => false,
        }
    }

    // Sweeps the state map and advances the lifecycle of each input.
    // 'JustPressed' becomes 'Held', and 'JustReleased' is removed.
    fn advance_states<K: std::hash::Hash + Eq>(states: &mut HashMap<K, KeyState>) {
        states.retain(|_, state: &mut KeyState| match state {
            KeyState::JustPressed => {
                *state = KeyState::Held;
                true
            }
            KeyState::Held => true,
            KeyState::JustReleased => false,
            KeyState::Up => false,
        });
    }

    // Safely retrieves the state of a specific key.
    // Defaults to 'Up' if the key is not currently tracked.
    fn key_state(&self, key: KeyCode) -> KeyState {
        self.key_states.get(&key).copied().unwrap_or(KeyState::Up)
    }

    // Safely retrieves the state of a specific mouse button.
    // Defaults to 'Up' if the button is not currently tracked.
    fn mouse_state(&self, button: MouseButton) -> KeyState {
        self.mouse_states.get(&button).copied().unwrap_or(KeyState::Up)
    }

    // Resolves the state of a generic InputSource by checking either
    // the keyboard map or the mouse map.
    fn source_state(&self, source: &InputSource) -> KeyState {
        match source {
            InputSource::Key(k) => self.key_state(*k),
            InputSource::Mouse(b) => self.mouse_state(*b),
        }
    }

    // Returns an iterator over all input sources currently bound to the given action.
    fn sources_for<'a>(&'a self, action: &A) -> impl Iterator<Item = &'a InputSource> {
        self.bindings
            .get(action)
            .into_iter()
            .flat_map(|set: &HashSet<InputSource>| set.iter())
    }
}

impl<A: InputAction> Default for InputManager<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: InputAction> InputQuery<A> for InputManager<A> {
    fn just_pressed(&self, action: A) -> bool {
        self.sources_for(&action)
            .any(|s: &InputSource| self.source_state(s) == KeyState::JustPressed)
    }

    fn held(&self, action: A) -> bool {
        self.sources_for(&action)
            .any(|s: &InputSource| self.source_state(s).is_down())
    }

    fn just_released(&self, action: A) -> bool {
        self.sources_for(&action)
            .any(|s: &InputSource| self.source_state(s) == KeyState::JustReleased)
    }

    fn key_held(&self, key: KeyCode) -> bool {
        self.key_state(key).is_down()
    }

    fn key_just_pressed(&self, key: KeyCode) -> bool {
        self.key_state(key) == KeyState::JustPressed
    }

    fn key_just_released(&self, key: KeyCode) -> bool {
        self.key_state(key) == KeyState::JustReleased
    }

    fn mouse_held(&self, button: MouseButton) -> bool {
        self.mouse_state(button).is_down()
    }

    fn mouse_just_pressed(&self, button: MouseButton) -> bool {
        self.mouse_state(button) == KeyState::JustPressed
    }

    fn mouse_just_released(&self, button: MouseButton) -> bool {
        self.mouse_state(button) == KeyState::JustReleased
    }

    fn mouse_position(&self) -> Option<Vec2> {
        self.mouse_position
    }

    fn scroll_delta(&self) -> Vec2 {
        self.scroll_delta
    }
}

impl<A: InputAction> InputBind<A> for InputManager<A> {
    fn bind(&mut self, action: A, source: InputSource) {
        self.bindings.entry(action).or_default().insert(source);
    }

    fn unbind(&mut self, action: A) {
        self.bindings.remove(&action);
    }

    fn clear_bindings(&mut self) {
        self.bindings.clear();
    }
}

#[cfg(test)]
mod tests {
    use winit::event::MouseButton;

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum Action {
        Jump,
        Shoot,
        Fire,
    }

    fn key_event(code: KeyCode, state: ElementState, repeat: bool) -> WindowEvent {
        use winit::{
            event::KeyEvent,
            keyboard::{Key, KeyLocation, NativeKey},
        };

        WindowEvent::KeyboardInput {
            device_id:    None,
            is_synthetic: false,
            event:        KeyEvent {
                physical_key: PhysicalKey::Code(code),
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                text: None,
                text_with_all_modifiers: None,
                location: KeyLocation::Standard,
                state,
                repeat,
                key_without_modifiers: Key::Unidentified(NativeKey::Unidentified),
            },
        }
    }

    fn mouse_event(button: MouseButton, state: ElementState) -> WindowEvent {
        WindowEvent::PointerButton {
            device_id: None,
            state,
            position: winit::dpi::PhysicalPosition::new(0.0, 0.0),
            button: winit::event::ButtonSource::Mouse(button),
            primary: true,
        }
    }

    fn scroll_event(x: f32, y: f32) -> WindowEvent {
        WindowEvent::MouseWheel {
            device_id: None,
            delta:     MouseScrollDelta::LineDelta(x, y),
            phase:     winit::event::TouchPhase::Moved,
        }
    }

    fn pointer_event(x: f64, y: f64) -> WindowEvent {
        WindowEvent::PointerMoved {
            device_id: None,
            position:  winit::dpi::PhysicalPosition::new(x, y),
            primary:   true,
            source:    winit::event::PointerSource::Mouse,
        }
    }

    #[test]
    fn key_lifecycle_just_pressed_to_held() {
        let mut mgr: InputManager<Action> = InputManager::new();
        mgr.bind(Action::Jump, KeyCode::Space.into());

        mgr.process_event(&key_event(KeyCode::Space, ElementState::Pressed, false));
        assert!(!mgr.just_pressed(Action::Jump));

        mgr.tick();
        assert!(mgr.just_pressed(Action::Jump));
        assert!(mgr.held(Action::Jump));

        mgr.tick();
        assert!(!mgr.just_pressed(Action::Jump));
        assert!(mgr.held(Action::Jump));
    }

    #[test]
    fn key_lifecycle_just_released() {
        let mut mgr: InputManager<Action> = InputManager::new();
        mgr.bind(Action::Jump, KeyCode::Space.into());

        mgr.process_event(&key_event(KeyCode::Space, ElementState::Pressed, false));
        mgr.tick();

        mgr.process_event(&key_event(KeyCode::Space, ElementState::Released, false));
        mgr.tick();

        assert!(mgr.just_released(Action::Jump));
        assert!(!mgr.held(Action::Jump));

        mgr.tick();
        assert!(!mgr.just_released(Action::Jump));
    }

    #[test]
    fn ignore_keyboard_repeat_events() {
        let mut mgr: InputManager<Action> = InputManager::new();
        mgr.bind(Action::Jump, KeyCode::Space.into());

        mgr.process_event(&key_event(KeyCode::Space, ElementState::Pressed, false));
        mgr.tick();

        mgr.process_event(&key_event(KeyCode::Space, ElementState::Pressed, true));
        mgr.tick();

        assert!(!mgr.just_pressed(Action::Jump));
        assert!(mgr.held(Action::Jump));
    }

    #[test]
    fn multiple_keys_bound_to_same_action() {
        let mut mgr: InputManager<Action> = InputManager::new();
        mgr.bind(Action::Jump, KeyCode::Space.into());
        mgr.bind(Action::Jump, KeyCode::ArrowUp.into());

        mgr.process_event(&key_event(KeyCode::ArrowUp, ElementState::Pressed, false));
        mgr.tick();

        assert!(mgr.just_pressed(Action::Jump));
    }

    #[test]
    fn mouse_button_lifecycle() {
        let mut mgr: InputManager<Action> = InputManager::new();
        mgr.bind(Action::Shoot, MouseButton::Left.into());

        mgr.process_event(&mouse_event(MouseButton::Left, ElementState::Pressed));
        mgr.tick();

        assert!(mgr.just_pressed(Action::Shoot));
        assert!(mgr.mouse_just_pressed(MouseButton::Left));

        mgr.tick();
        assert!(!mgr.just_pressed(Action::Shoot));
        assert!(mgr.held(Action::Shoot));

        mgr.process_event(&mouse_event(MouseButton::Left, ElementState::Released));
        mgr.tick();

        assert!(mgr.just_released(Action::Shoot));
    }

    #[test]
    fn mouse_scroll_double_buffering() {
        let mut mgr: InputManager<Action> = InputManager::new();

        mgr.process_event(&scroll_event(0.0, 1.0));
        mgr.process_event(&scroll_event(0.0, 2.0));

        assert_eq!(mgr.scroll_delta(), Vec2::ZERO);

        mgr.tick();
        let delta: Vec2 = mgr.scroll_delta();
        assert!((delta.y - 3.0).abs() < 1e-3);

        mgr.tick();
        assert_eq!(mgr.scroll_delta(), Vec2::ZERO);
    }

    #[test]
    fn mouse_position_tracking() {
        let mut mgr: InputManager<Action> = InputManager::new();

        mgr.process_event(&pointer_event(320.0, 240.0));

        let pos: Vec2 = mgr.mouse_position().unwrap();
        assert!((pos.x - 320.0).abs() < 1e-3);
        assert!((pos.y - 240.0).abs() < 1e-3);
    }

    #[test]
    fn phantom_click_deferral_prevents_lost_inputs() {
        let mut mgr: InputManager<Action> = InputManager::new();
        mgr.bind(Action::Fire, MouseButton::Right.into());

        mgr.process_event(&mouse_event(MouseButton::Right, ElementState::Pressed));
        mgr.process_event(&mouse_event(MouseButton::Right, ElementState::Released));

        mgr.tick();

        assert!(mgr.just_pressed(Action::Fire));
        assert!(mgr.held(Action::Fire));
        assert!(!mgr.just_released(Action::Fire));

        mgr.tick();

        assert!(!mgr.just_pressed(Action::Fire));
        assert!(!mgr.held(Action::Fire));
        assert!(mgr.just_released(Action::Fire));
    }

    #[test]
    fn mixed_source_same_action_either_triggers() {
        let mut mgr: InputManager<Action> = InputManager::new();
        mgr.bind(Action::Fire, KeyCode::KeyF.into());
        mgr.bind(Action::Fire, MouseButton::Right.into());

        mgr.process_event(&key_event(KeyCode::KeyF, ElementState::Pressed, false));
        mgr.tick();
        assert!(mgr.just_pressed(Action::Fire));

        mgr.process_event(&key_event(KeyCode::KeyF, ElementState::Released, false));
        mgr.tick();
        assert!(mgr.just_released(Action::Fire));
        mgr.tick();

        mgr.process_event(&mouse_event(MouseButton::Right, ElementState::Pressed));
        mgr.tick();
        assert!(mgr.just_pressed(Action::Fire));

        mgr.process_event(&mouse_event(MouseButton::Right, ElementState::Released));
        mgr.tick();
        assert!(mgr.just_released(Action::Fire));
        mgr.tick();

        mgr.process_event(&key_event(KeyCode::KeyF, ElementState::Pressed, false));
        mgr.process_event(&mouse_event(MouseButton::Right, ElementState::Pressed));
        mgr.tick();
        assert!(mgr.just_pressed(Action::Fire));
        assert!(mgr.held(Action::Fire));

        mgr.process_event(&key_event(KeyCode::KeyF, ElementState::Released, false));
        mgr.tick();
        assert!(mgr.held(Action::Fire));
        assert!(mgr.key_just_released(KeyCode::KeyF));
        assert!(mgr.mouse_held(MouseButton::Right));

        mgr.process_event(&mouse_event(MouseButton::Right, ElementState::Released));
        mgr.tick();
        assert!(!mgr.held(Action::Fire));
        assert!(mgr.just_released(Action::Fire));
    }
}
