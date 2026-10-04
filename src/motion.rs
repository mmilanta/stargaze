//! Short UI transitions stop repainting when settled; pressed keys stay in their screen.
use crate::ui::{Rect, UiVertex};
use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};
use winit::keyboard::KeyCode;

#[derive(Clone, Copy)]
pub struct Input {
    pub screen: u32,
    pub selected: usize,
    pub cursor: (f64, f64),
    pub key: Option<KeyCode>,
    pub mouse: bool,
    pub viewport: [f32; 2],
}
struct State {
    screen: u32,
    selected: usize,
    entered: Instant,
    selection_at: Instant,
    ignore_key: Option<KeyCode>,
    ignore_mouse: bool,
}
pub struct Motion {
    state: RefCell<State>,
    enabled: bool,
}
impl Default for Motion {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            state: RefCell::new(State {
                screen: u32::MAX,
                selected: usize::MAX,
                entered: now,
                selection_at: now,
                ignore_key: None,
                ignore_mouse: false,
            }),
            enabled: !cfg!(test),
        }
    }
}
#[derive(Clone, Copy)]
struct Frame {
    age: f32,
    selection_age: f32,
    input: Input,
}
thread_local! {static FRAME:Cell<Option<Frame>>=const{Cell::new(None)};}
impl Motion {
    pub fn begin(&self, input: Input) {
        self.begin_at(input, Instant::now());
    }
    fn begin_at(&self, input: Input, now: Instant) {
        let mut state = self.state.borrow_mut();
        if state.screen != input.screen {
            state.screen = input.screen;
            state.entered = now;
            state.selection_at = now;
            state.selected = input.selected;
            state.ignore_key = input.key;
            state.ignore_mouse = input.mouse;
        }
        if state.selected != input.selected {
            state.selected = input.selected;
            state.selection_at = now;
        }
        if input.key != state.ignore_key {
            state.ignore_key = None;
        }
        if !input.mouse {
            state.ignore_mouse = false;
        }
        let mut filtered = input;
        if filtered.key == state.ignore_key {
            filtered.key = None;
        }
        if state.ignore_mouse {
            filtered.mouse = false;
        }
        let age = if self.enabled {
            now.saturating_duration_since(state.entered).as_secs_f32()
        } else {
            999.0
        };
        let selection_age = if self.enabled {
            now.saturating_duration_since(state.selection_at)
                .as_secs_f32()
        } else {
            999.0
        };
        FRAME.set(Some(Frame {
            age,
            selection_age,
            input: filtered,
        }));
    }
    pub fn animating(&self) -> bool {
        let state = self.state.borrow();
        self.enabled
            && state.screen != u32::MAX
            && (state.entered.elapsed() < Duration::from_millis(1200)
                || state.selection_at.elapsed() < Duration::from_millis(260))
    }
}
pub fn pressed(label: &str, r: Rect) -> bool {
    FRAME.get().is_some_and(|f| {
        if f.input.mouse && r.contains(f.input.cursor.0, f.input.cursor.1) {
            return true;
        }
        let Some(key) = f.input.key else {
            return false;
        };
        match key {
            KeyCode::KeyS => label.ends_with("[S]"),
            KeyCode::KeyR => label.ends_with("[R]"),
            KeyCode::KeyD => label.ends_with("[D]"),
            KeyCode::KeyM => label.ends_with("[M]"),
            KeyCode::KeyL => label.ends_with("[L]"),
            KeyCode::KeyG => label.ends_with("[G]"),
            KeyCode::KeyH => label.ends_with("[H]"),
            KeyCode::KeyA => label.ends_with("[A]"),
            KeyCode::KeyU => label.ends_with("[U]"),
            KeyCode::F11 => label.ends_with("[F11]"),
            KeyCode::KeyV => label.ends_with("[V]"),
            KeyCode::KeyX => label.ends_with("[X]"),
            KeyCode::KeyZ => label.ends_with("[Z]"),
            KeyCode::KeyY => label.ends_with("[Y]"),
            KeyCode::Enter => label.ends_with("[Enter]"),
            KeyCode::Escape => label.ends_with("[Esc]"),
            KeyCode::Tab => label.ends_with("[Tab]"),
            KeyCode::Space => label.ends_with("[Spc]"),
            KeyCode::Delete | KeyCode::Backspace => label.ends_with("[Del]"),
            KeyCode::PageDown => label.ends_with("[PgDn]"),
            KeyCode::PageUp => label.ends_with("[PgUp]"),
            KeyCode::ArrowLeft => label == "←",
            KeyCode::ArrowRight => label == "→",
            _ => [
                KeyCode::F1,
                KeyCode::F2,
                KeyCode::F3,
                KeyCode::F4,
                KeyCode::F5,
                KeyCode::F6,
                KeyCode::F7,
                KeyCode::F8,
                KeyCode::F9,
            ]
            .iter()
            .position(|k| *k == key)
            .is_some_and(|i| label.ends_with(&format!("[F{}]", i + 1))),
        }
    })
}
pub fn snap() -> f32 {
    FRAME
        .get()
        .map_or(1.0, |f| (f.selection_age / 0.26).clamp(0.0, 1.0))
}
pub fn progress(duration: f32) -> f32 {
    FRAME
        .get()
        .map_or(1.0, |f| (f.age / duration).clamp(0.0, 1.0))
}
pub fn rise(v: &mut [UiVertex], duration: f32, delay: f32) {
    let Some(frame) = FRAME.get() else {
        return;
    };
    let t = ((frame.age - delay) / duration).clamp(0.0, 1.0);
    let ease = 1.0 - (1.0 - t).powi(3);
    let shift = 18.0 * (1.0 - ease) * 2.0 / frame.input.viewport[1];
    for vertex in v {
        vertex.pos[1] -= shift;
        vertex.color[3] *= ease;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pressed_key_release_and_screen_change_do_not_carry() {
        let m = Motion {
            enabled: true,
            ..Motion::default()
        };
        let now = Instant::now();
        let mut i = Input {
            screen: 1,
            selected: 0,
            cursor: (50., 50.),
            key: None,
            mouse: false,
            viewport: [1280., 720.],
        };
        let r = Rect {
            x: 0.,
            y: 0.,
            w: 100.,
            h: 100.,
        };
        m.begin_at(i, now);
        i.key = Some(KeyCode::Enter);
        m.begin_at(i, now + Duration::from_secs(2));
        assert!(pressed("Play [Enter]", r));
        i.screen = 2;
        m.begin_at(i, now + Duration::from_secs(3));
        assert!(!pressed("Apply [Enter]", r));
        i.key = None;
        m.begin_at(i, now + Duration::from_secs(4));
        i.key = Some(KeyCode::Enter);
        m.begin_at(i, now + Duration::from_secs(5));
        assert!(pressed("Apply [Enter]", r));
        i.key = None;
        m.begin_at(i, now + Duration::from_secs(6));
        assert!(!pressed("Apply [Enter]", r));
        FRAME.set(None);
    }
}
