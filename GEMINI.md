# Gemini & Antigravity Rules for Lide Editor

You are an expert systems & GUI engineer pair programming on **Lide**, an ultra-fast, memory-lean Python & polyglot IDE engineered in pure Rust.

---

## Architectural Guidelines & Behavioral Constraints

1. **Decoupled Headless Core**:
   - `crates/editor-core` has no dependency on `floem`, `tokio`, or GPU graphics. It is a deterministic, synchronous engine.
   - All editing commands must be verifiable in headless `cargo test` harnesses without requiring X11/Wayland/Windows display servers.

2. **Fine-Grained Reactive Signal Architecture**:
   - In `crates/app-floem`, never trigger monolithic view redraws.
   - Partition UI state into isolated Floem signals (`RwSignal<T>`).
   - Typing only notifies the active line; scrolling only notifies visible line indices; AI streaming only notifies the assistant stream signal; Pytest runs only notify the test panel signals.

3. **Sub-Millisecond SLAs**:
   - **Cold Start**: < 50 ms.
   - **10k-Line File Open**: < 5 ms.
   - **Keystroke-to-Render Latency**: < 16 ms (60–120 FPS).
   - **AST Lint & Structure Pass**: < 20 ms (background worker).
   - **Idle RSS Memory**: 30 MB – 50 MB.

4. **Coordinate Translations**:
   - `editor-core`: 0-indexed line (`0..N`) and character column.
   - `workspace-engine` (Ripgrep & Pytest): 1-indexed line numbers. Always convert using `line_number.saturating_sub(1)`.

5. **Local AI Privacy & AST Pruning**:
   - Local LLM prompts (`ai-engine`) must slice strictly the innermost enclosing AST scope (`EnclosingContext`) rather than transmitting full files.

---

## Workspace Customization Skills

Specialized workflow skills are available in `.agents/skills/` and `aidocs/skills/`:
- **`lide-tdd`**: TDD test harness, mocking LLM/Pytest interfaces, and headless assertions.
- **`lide-language-support`**: Checklist and contracts for adding Cython, C++, C, Jupyter, or Rust.
- **`lide-floem-ui`**: Reactive signals, `create_ext_action` stream pump, and zero-redraw layouts.

---

## Verification Commands

```bash
cargo test --workspace
cargo test -p app-floem --test v1_step2_bridge_tdd
cargo run -p app-floem --release
```
