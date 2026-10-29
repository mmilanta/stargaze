//! Relative mouse-look and keyboard-only observation controls.
use crate::{App, KeyCode, MAX_FOV_DEG, MIN_FOV_DEG, ZOOM_STEP, ui};
use winit::window::CursorGrabMode;

#[derive(Default)]
pub(crate) struct MouseLook {
    pub focused: bool,
    requested: bool,
    pub captured: bool,
}

impl App {
    pub(crate) fn observing(&self) -> bool {
        self.world_visible()
            && self.discard.is_none()
            && !self
                .game
                .as_ref()
                .is_some_and(|g| g.result_open || g.confirm_clear)
    }

    pub(crate) fn sync_mouse_capture(&mut self) {
        let Some(window) = &self.window else { return };
        let size = window.inner_size();
        let requested = self.observing()
            && self.mouse_look.focused
            && !self.occluded
            && size.width > 0
            && size.height > 0;
        if requested == self.mouse_look.requested {
            return;
        }
        self.mouse_look.requested = requested;
        if requested {
            // Prefer a locked pointer; fall back to confinement on backends
            // without locking. Raw deltas keep looking independent of edges.
            let result = window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
            self.mouse_look.captured = result.is_ok();
            if let Err(err) = result {
                log::warn!("could not capture mouse for observation: {err}");
            }
        } else {
            if let Err(err) = window.set_cursor_grab(CursorGrabMode::None) {
                log::warn!("could not release mouse: {err}");
            }
            self.mouse_look.captured = false;
        }
        window.set_cursor_visible(!self.mouse_look.captured);
        // Never carry an editor click or hover into the observation HUD.
        self.mouse_down = false;
        self.cursor = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    }

    pub(crate) fn observation_motion(&mut self, delta: (f64, f64), height: u32) -> bool {
        if !self.observing()
            || !self.mouse_look.focused
            || !self.mouse_look.captured
            || self.occluded
        {
            return false;
        }
        self.state.look_around(delta, height)
    }

    pub(crate) fn target_pages(&self) -> usize {
        self.state.scene.targets.len().div_ceil(10).max(1)
    }

    pub(crate) fn target_index(&self, key: KeyCode) -> Option<usize> {
        if !self.targets_open || self.game.is_some() {
            return None;
        }
        let slot = match key {
            KeyCode::Digit1 => 0,
            KeyCode::Digit2 => 1,
            KeyCode::Digit3 => 2,
            KeyCode::Digit4 => 3,
            KeyCode::Digit5 => 4,
            KeyCode::Digit6 => 5,
            KeyCode::Digit7 => 6,
            KeyCode::Digit8 => 7,
            KeyCode::Digit9 => 8,
            KeyCode::Digit0 => 9,
            _ => return None,
        };
        let index = self.target_page.min(self.target_pages() - 1) * 10 + slot;
        (index < self.state.scene.targets.len()).then_some(index)
    }

    fn target_key(&mut self, key: KeyCode) -> bool {
        if !self.targets_open || self.game.is_some() {
            return false;
        }
        self.target_page = self.target_page.min(self.target_pages() - 1);
        match key {
            KeyCode::PageUp => self.target_page = self.target_page.saturating_sub(1),
            KeyCode::PageDown => {
                self.target_page = (self.target_page + 1).min(self.target_pages() - 1)
            }
            _ => {
                if let Some(index) = self.target_index(key) {
                    self.state.aim_target(index);
                } else {
                    return false;
                }
            }
        }
        true
    }

    /// Keep sky locking on right-click, but never route observation clicks to
    /// the HUD. The captured pointer aims at the reticle, not its old position.
    pub(crate) fn observation_button(
        &mut self,
        button: crate::MouseButton,
        state: crate::ElementState,
        (width, height): (u32, u32),
    ) -> bool {
        if !self.observing() {
            return false;
        }
        if button == crate::MouseButton::Right && state == crate::ElementState::Pressed {
            self.state
                .lock_at(width as f64 * 0.5, height as f64 * 0.5, width, height);
        }
        true
    }

    /// Only exposure adjustment repeats while held; toggles, target selection,
    /// and time jumps must remain single-press actions.
    pub(crate) fn repeat_observation_key(&mut self, key: KeyCode) -> bool {
        if !self.observing() || !matches!(key, KeyCode::Comma | KeyCode::Period) {
            return false;
        }
        self.observation_key(key);
        true
    }

    pub(crate) fn observation_key(&mut self, key: KeyCode) {
        if self.target_key(key) || self.state.time_key(key) || self.air_key(key) {
            return;
        }
        match key {
            KeyCode::KeyP => self.open_settings(),
            KeyCode::KeyR | KeyCode::KeyS | KeyCode::KeyO => self.state.toggle_star_orientation(),
            KeyCode::KeyK => {
                if let Some(r) = &self.renderer {
                    let (w, h) = r.size();
                    self.state.lock_at(w as f64 * 0.5, h as f64 * 0.5, w, h);
                }
            }
            KeyCode::KeyU => self.state.unlock(),
            KeyCode::Home => self.state.reset_view(),
            KeyCode::F10 => self.request_leave(crate::Leave::Title),
            KeyCode::KeyG if self.game.is_none() => {
                self.targets_open = !self.targets_open;
                self.air_open = false;
                self.show_hud = true;
            }
            KeyCode::KeyB if self.game.is_none() => self.cycle_ground(),
            KeyCode::KeyJ if self.game.is_none() => {
                self.air_open = !self.air_open;
                self.targets_open = false;
                self.show_hud = true;
            }
            KeyCode::KeyE => self.find_event(false),
            KeyCode::KeyT => self.find_event(true),
            KeyCode::KeyH => {
                self.show_hud = !self.show_hud;
                if !self.show_hud {
                    self.targets_open = false;
                    self.air_open = false;
                }
            }
            KeyCode::KeyL => self.show_labels = !self.show_labels,
            KeyCode::KeyA => {
                self.auto_exposure = !self.auto_exposure;
                if self.auto_exposure {
                    self.ev_bias = 0.0;
                }
                self.sync_exposure();
            }
            KeyCode::Comma | KeyCode::Period => {
                let step = if key == KeyCode::Comma { -0.25 } else { 0.25 };
                self.ev_bias = (self.ev_bias + step).clamp(ui::MIN_EV, ui::MAX_EV);
                self.sync_exposure();
            }
            KeyCode::Equal | KeyCode::NumpadAdd => {
                self.state.observer.fov_y =
                    (self.state.observer.fov_y * ZOOM_STEP).max(MIN_FOV_DEG.to_radians());
            }
            KeyCode::Minus | KeyCode::NumpadSubtract => {
                self.state.observer.fov_y =
                    (self.state.observer.fov_y / ZOOM_STEP).min(MAX_FOV_DEG.to_radians());
            }
            _ => {}
        }
    }
}
