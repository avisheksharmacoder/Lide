//! Global keyboard shortcut dispatcher and subscription for Lide IDE in Iced.

use iced::keyboard::{self, key::Named, Key, Modifiers};
use iced::Subscription;

use crate::{EditorCommand, LideApp, MoveDirection};

/// Resolves a full keyboard event (including raw key, modified key, modifiers, and OS typed text)
/// into an optional `EditorCommand`.
/// Exposed as a pure function pointer for both runtime subscription and headless TDD verification.
pub fn resolve_key_event(
    key: Key,
    modified_key: Key,
    modifiers: Modifiers,
    text: Option<&str>,
) -> Option<EditorCommand> {
    // 1. Escape key: Dispatches unified Escape command (handled deterministically in update)
    if key == Key::Named(Named::Escape) {
        return Some(EditorCommand::Escape);
    }

    // 2. Global Hotkeys with Control / Command
    if modifiers.command() {
        if let Key::Character(ref s) = key {
            let s_lower = s.to_ascii_lowercase();
            match s_lower.as_str() {
                "s" => return Some(EditorCommand::SaveActiveFile),
                "n" => return Some(EditorCommand::NewDocument),
                "o" => return Some(EditorCommand::TriggerOpenFolderPrompt),
                "b" => return Some(EditorCommand::ToggleFileManager),
                "z" => {
                    if modifiers.shift() {
                        return Some(EditorCommand::Redo);
                    } else {
                        return Some(EditorCommand::Undo);
                    }
                }
                "y" => return Some(EditorCommand::Redo),
                "f" if modifiers.shift() => return Some(EditorCommand::ToggleSearchPanel),
                "t" if modifiers.shift() => return Some(EditorCommand::TogglePytestPanel),
                "a" if modifiers.shift() => return Some(EditorCommand::ToggleAiPane),
                "i" => return Some(EditorCommand::ToggleAiPane),
                "," => return Some(EditorCommand::ToggleFontDialog),
                _ => {}
            }
        }
    }

    // 3. F5: Pytest Run
    if key == Key::Named(Named::F5) {
        return Some(EditorCommand::TriggerPytestRun { target_file: None });
    }

    // 4. Editor Navigation & Editing Keys
    match key {
        Key::Named(Named::ArrowUp) => return Some(EditorCommand::MoveCursor(MoveDirection::Up)),
        Key::Named(Named::ArrowDown) => return Some(EditorCommand::MoveCursor(MoveDirection::Down)),
        Key::Named(Named::ArrowLeft) => return Some(EditorCommand::MoveCursor(MoveDirection::Left)),
        Key::Named(Named::ArrowRight) => return Some(EditorCommand::MoveCursor(MoveDirection::Right)),
        Key::Named(Named::Home) => return Some(EditorCommand::MoveCursor(MoveDirection::LineStart)),
        Key::Named(Named::End) => return Some(EditorCommand::MoveCursor(MoveDirection::LineEnd)),
        Key::Named(Named::Enter) => return Some(EditorCommand::InsertNewlineAutoIndent),
        Key::Named(Named::Backspace) => return Some(EditorCommand::Backspace),
        Key::Named(Named::Tab) => {
            if modifiers.shift() {
                return Some(EditorCommand::DedentLine);
            } else {
                return Some(EditorCommand::IndentLine);
            }
        }
        Key::Named(Named::Space) => {
            if !modifiers.command() && !modifiers.alt() {
                return Some(EditorCommand::InsertText(" ".to_string()));
            } else {
                return None;
            }
        }
        _ => {}
    }

    // 5. Text Insertion (Letters, shifted characters, numbers, punctuation)
    if !modifiers.command() && !modifiers.alt() {
        // Priority A: OS-provided typed text (respects CapsLock, Shift, dead keys, and international layouts)
        if let Some(t) = text {
            let is_printable = t.chars().all(|c| !c.is_control());
            if !t.is_empty() && is_printable {
                return Some(EditorCommand::InsertText(t.to_string()));
            }
        }

        // Priority B: `modified_key` (e.g. Key::Character("A"), Key::Character(":"))
        if let Key::Character(ref s) = modified_key {
            if !s.is_empty() {
                if modifiers.shift() {
                    return Some(EditorCommand::InsertText(s.to_uppercase()));
                } else {
                    return Some(EditorCommand::InsertText(s.to_string()));
                }
            }
        }

        // Priority C: Base `key` (with fallback shift capitalization)
        if let Key::Character(ref s) = key {
            if !s.is_empty() {
                if modifiers.shift() {
                    return Some(EditorCommand::InsertText(s.to_uppercase()));
                } else {
                    return Some(EditorCommand::InsertText(s.to_string()));
                }
            }
        }
    }

    None
}

/// Resolves a key press and modifiers into an optional `EditorCommand`.
/// Preserved for backwards compatibility with tests and callers.
pub fn resolve_key_action(key: Key, modifiers: Modifiers) -> Option<EditorCommand> {
    let modified_key = key.clone();
    resolve_key_event(key, modified_key, modifiers, None)
}

/// Creates the keyboard event subscription for the active application state.
pub fn keyboard_subscription(_app: &LideApp) -> Subscription<EditorCommand> {
    iced::event::listen().map(|event| match event {
        iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modified_key,
            modifiers,
            text,
            ..
        }) => resolve_key_event(key, modified_key, modifiers, text.as_deref())
            .unwrap_or(EditorCommand::NoOp),
        _ => EditorCommand::NoOp,
    })
}
