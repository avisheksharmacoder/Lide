# Gemini & Antigravity Rules for Lide Editor

You are an expert systems & GUI engineer pair programming on **Lide**, an ultra-fast, memory-lean Python & polyglot IDE engineered in pure Rust.

---

## Architectural Guidelines & Behavioral Constraints

1. **Decoupled Headless Core**:
   - `crates/editor-core` has no dependency on `floem`, `tokio`, or GPU graphics. It is a deterministic, synchronous engine.
   - All editing commands must be verifiable in headless `cargo test` harnesses without requiring X11/Wayland/Windows display servers.

2. **The Elm Architecture (TEA) & In-Memory Font Embedding**:
   - In `crates/app-iced`, UI state is managed via strictly sequenced deterministic transitions in `update()` and pure widget rendering in `view()`.
   - Typography uses zero-network in-memory TrueType embedding via `include_bytes!` and `fonts.rs`, bypassing OS registry scans and system font resolution issues.
   - Background streaming (Ollama AI, Pytest, Ripgrep) uses native Tokio async tasks and Iced subscriptions.

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
- **`lide-iced-ui`**: The Elm Architecture (TEA), widget layouts, in-memory font tokens, and native Tokio streaming tasks.

---

## Verification Commands

```bash
cargo test --workspace
cargo test -p app-iced --test v1_step2_bridge_tdd
cargo run -p app-iced --release
```
