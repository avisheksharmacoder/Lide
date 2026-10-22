//! Subscriptions for Iced TEA (Keyboard shortcuts, streaming bridges, mouse drag events).

pub mod keyboard;

use std::sync::atomic::{AtomicU32, Ordering};
use iced::Subscription;

use crate::{EditorCommand, LideApp};

static LAST_MOUSE_X: AtomicU32 = AtomicU32::new(0);
static LAST_MOUSE_Y: AtomicU32 = AtomicU32::new(0);

/// Updates the last recorded mouse position (lock-free atomics).
pub fn update_last_mouse_pos(point: iced::Point) {
    LAST_MOUSE_X.store(point.x.to_bits(), Ordering::Relaxed);
    LAST_MOUSE_Y.store(point.y.to_bits(), Ordering::Relaxed);
}

/// Returns the last recorded mouse position.
pub fn get_last_mouse_pos() -> iced::Point {
    let x = f32::from_bits(LAST_MOUSE_X.load(Ordering::Relaxed));
    let y = f32::from_bits(LAST_MOUSE_Y.load(Ordering::Relaxed));
    iced::Point::new(x, y)
}

/// Sets a mock mouse position for headless TDD testing without requiring display hardware.
pub fn set_mock_mouse_pos(point: iced::Point) {
    update_last_mouse_pos(point);
}

/// Root subscription combining global keyboard shortcuts and event listeners.
pub fn app_subscriptions(app: &LideApp) -> Subscription<EditorCommand> {
    let is_resizing = app.is_resizing_sidebar;

    iced::event::listen().map(move |event| match event {
        iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
            update_last_mouse_pos(position);
            if is_resizing {
                EditorCommand::ResizeSidebar(position.x)
            } else {
                EditorCommand::NoOp
            }
        }
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))
            if is_resizing =>
        {
            EditorCommand::EndResizeSidebar
        }
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key,
            modified_key,
            modifiers,
            text,
            ..
        }) => keyboard::resolve_key_event(key, modified_key, modifiers, text.as_deref())
            .unwrap_or(EditorCommand::NoOp),
        _ => EditorCommand::NoOp,
    })
}
