---
name: lide-tdd
description: "Strict Red-Green-Refactor test-driven development workflow for Lide editor features, commands, and TEA state transitions without requiring a display server."
---

# Lide TDD Engineering Workflow

Follow this procedure when creating new features, editor commands, or language mechanics for Lide.

## 1. Core Rule: Zero-Display Headless Testing

All core editing mechanics, AST slicing, search queries, test runners, and TEA state transitions MUST be verifiable in standard `cargo test` without an X11, Wayland, or Windows display context.

- **`crates/editor-core/tests/`**: Pure text buffer operations, multi-cursor physics, smart auto-indent (`:` -> +4 spaces), block indent/dedent (`Tab`/`Shift+Tab`), and undo/redo stacks.
- **`crates/lang-python/tests/`**: Tree-sitter tokenization, Astral Ruff AST parser, ephemeral symbol outline extraction, and code formatting.
- **`crates/workspace-engine/tests/`**: Ripgrep regex searcher, zero-subprocess virtualenv detection (`pyvenv.cfg`), and async Pytest execution.
- **`crates/ai-engine/tests/`**: AST-scoped prompt generator, mockable `LlmStreamingClient`, and Ollama event streaming.
- **`crates/app-iced/tests/`**: The Elm Architecture (TEA) commands, `LideApp` state transitions, mock mouse placement, caret overlay invariants, and headless widget rendering.

---

## 2. The 3-Step Red-Green-Refactor Cycle

### Step 1: Red (Write the Failing Test First)
Create a new integration test file in `crates/app-iced/tests/<feature>_tdd.rs` (or add to an existing suite):

```rust
use app_iced::subscriptions::set_mock_mouse_pos;
use app_iced::views::editor::render_editor;
use app_iced::{EditorCommand, LideApp, Position};
use iced::Point;

#[test]
fn test_new_command_updates_state_without_layout_shift() {
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::InsertText("def sample():\n    pass\n".to_string()));

    // Assert initial state
    assert_eq!(app.buffer.cursor().head.line, 1);
    assert_eq!(app.visible_lines.len(), 2);

    // Mock mouse coordinates if testing mouse interactions
    set_mock_mouse_pos(Point::new(120.0, 30.0));

    // Dispatch the new command under test
    let _ = app.dispatch(EditorCommand::MyNewCommand);

    // Assert exact expected state mutation
    // assert_eq!(app.my_expected_property, expected_value);

    // Assert headless view rendering produces valid element trees without panicking
    let _editor_element = render_editor(&app);
}
```

Run the test and confirm it fails or does not yet compile:
```bash
cargo test -p app-iced --test <feature>_tdd
```

### Step 2: Green (Write Minimal Code to Pass)
1. Add the enum variant to `EditorCommand` in `crates/app-iced/src/lib.rs`.
2. Add necessary fields to `LideApp` and initialize them in `LideApp::new()`.
3. Implement the matching branch in `LideApp::update(&mut self, message: EditorCommand) -> Task<EditorCommand>`.
4. If testing keyboard shortcuts, add the mapping to `subscriptions/keyboard.rs::resolve_key_event`.
5. Re-run the test to confirm it turns green.

### Step 3: Refactor & Invariant Verification
Verify that:
- Caret and line text layout remain strictly zero-shift (no character glyphs injected into `code_row`).
- No heap allocations occur on hotpaths (`CursorMoved`, typing, gutter rendering).
- Coordinate invariants (`0-based` buffer vs `1-based` external tools) are strictly preserved.
- Run the full workspace test suite:
  ```bash
  cargo test --workspace
  ```

