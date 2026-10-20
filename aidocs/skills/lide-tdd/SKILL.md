---
name: lide-tdd
description: "Strict Red-Green-Refactor test-driven development workflow for Lide editor features, commands, and signals without requiring a display server."
---

# Lide TDD Engineering Workflow

Follow this procedure when creating new features, editor commands, or language mechanics for Lide.

## 1. Core Rule: Zero-Display Headless Testing

All core editing mechanics, AST slicing, search queries, test runners, and signal dispatches MUST be verifiable in standard `cargo test` without an X11, Wayland, or Windows display context.

- Pure text operations belong in `crates/editor-core/tests/`.
- Language parsers, Tree-sitter tokens, and Ruff AST belong in `crates/lang-python/tests/`.
- Ripgrep and virtualenv detection belong in `crates/workspace-engine/tests/`.
- AI prompting and token streaming belong in `crates/ai-engine/tests/`.
- Floem reactive view model commands and signal updates belong in `crates/app-floem/tests/`.

---

## 2. The 3-Step Red-Green-Refactor Cycle

### Step 1: Red (Write the Failing Test First)
Create a new integration test file in the relevant crate's `tests/` directory (e.g. `crates/app-floem/tests/<feature>_tdd.rs`).

Define mock implementations for any external systems (e.g. `MockLlmClient` or `MockPytestExecutor`):

```rust
use app_floem::{EditorCommand, EditorViewModel};
use editor_core::Position;
use floem::reactive::SignalGet;

#[test]
fn test_new_command_updates_expected_signal_without_side_effects() {
    let mut vm = EditorViewModel::new(22.0, 792.0);
    vm.dispatch(EditorCommand::LoadSource("def foo():\n    pass\n".to_string()));

    // Assert initial state
    assert_eq!(vm.signals.is_dirty.get(), false);

    // Dispatch the new command under test
    // vm.dispatch(EditorCommand::MyNewCommand);

    // Assert the exact expected signal change
    // assert_eq!(vm.signals.my_signal.get(), expected_value);
}
```

Run the test and confirm it fails or does not yet compile:
```bash
cargo test -p <crate> --test <feature>_tdd
```

### Step 2: Green (Write Minimal Code to Pass)
1. Add the enum variant to `EditorCommand` in `crates/app-floem/src/lib.rs` (or the underlying core trait/struct).
2. Add necessary signals to `EditorSignals` and initialize them in `EditorViewModel::new`.
3. Implement the matching branch in `EditorViewModel::dispatch()`.
4. Re-run the test to confirm it turns green.

### Step 3: Refactor & Signal Isolation Audit
Verify that:
- The new command does **not** trigger viewport recalculation unless line count or scroll position changed.
- No allocations occur in the hot path.
- Coordinate invariants (`0-based` vs `1-based`) are strictly preserved.
- Run the full workspace suite:
  ```bash
  cargo test --workspace
  ```
