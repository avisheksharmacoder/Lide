# Claude Assistant Guidelines for Lide Editor

You are working on **Lide**, an ultra-fast, memory-lean desktop IDE written in 100% pure Rust using the **Iced** native GUI toolkit (The Elm Architecture), with embedded **Astral Ruff**, **Tree-sitter**, and **Ripgrep**.

---

## 1. Quick Commands

```bash
# Build the entire workspace
cargo build --workspace

# Run all unit, integration, and headless UI tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p editor-core
cargo test -p lang-python
cargo test -p workspace-engine
cargo test -p ai-engine
cargo test -p app-iced

# Run performance benchmarks
cargo bench -p editor-core

# Launch Lide in release mode
cargo run -p app-iced --release
```

---

## 2. Inviolable Architectural Tenets

1. **Zero GUI Coupling in `crates/editor-core`**:
   - `editor-core` must **NEVER** import Iced, Floem, Winit, GPU dependencies, or async runtimes.
   - All buffer edits, cursor math, auto-indentation, and undo/redo stacks must be synchronous and verified via headless `cargo test`.
2. **The Elm Architecture (TEA) in `crates/app-iced`**:
   - UI state is managed via strictly sequenced deterministic transitions in `update()` and pure widget rendering in `view()`.
   - Typography uses zero-network in-memory TrueType embedding via `include_bytes!` (`fonts.rs`), bypassing OS registry scans and system font resolution issues.
   - Background streaming (Ollama AI, Pytest, Ripgrep) uses native Tokio async tasks and Iced subscriptions.
3. **Sub-Millisecond SLAs & Non-Blocking Async**:
   - **Never** perform disk I/O, regex searching, AST parsing, or formatting directly on the main UI thread.
   - Offload heavy tasks to background Tokio worker threads and bridge events back using Iced `Task::stream` and `Subscription`.
4. **Coordinate Safety**:
   - Buffer coordinates (`editor-core`) are **0-based** line numbers (`0..N`) and character column offsets.
   - Ripgrep (`SearchMatch`) and Pytest tracebacks emit **1-based** line numbers. Always convert using `.saturating_sub(1)` when navigating to buffer positions.
5. **Ephemeral AST Disposal**:
   - In `lang-python`, parse Ruff AST arenas, query hierarchical symbols and enclosing scopes, and **drop the AST immediately**. Never retain AST arenas long-term in signals to ensure idle RAM stays under 40 MB.
6. **Atomic Format-on-Save**:
   - Always format in-memory with Astral Ruff, write atomically via a temp file with rename fallback, preserve cursor positions, and clear the dirty flag.
7. **Strict TDD Workflow**:
   - Write or update headless unit tests before implementing production code (Red -> Green -> Refactor).

---

## 3. Workspace Crate Map

- `crates/editor-core`: Pure-Rust Rope (`ropey`), `Position(line, col)`, multi-cursor physics, smart auto-indent (`:` -> +4 spaces), `Tab`/`Shift+Tab`, undo/redo stack.
- `crates/lang-contract`: Language-agnostic traits (`SyntaxHighlighter`, `AstAnalyzer`, `CodeFormatter`), `Diagnostic`, `OutlineSymbol`, `EnclosingContext`, `LanguageId`.
- `crates/lang-python`: Tree-sitter Python highlighting, embedded Astral Ruff parser & formatter, `.pyd`/`.so` binary stub resolver.
- `crates/workspace-engine`: Multi-threaded file indexer (`ignore`), Ripgrep regex search (`grep-searcher`), zero-subprocess virtualenv detector (`pyvenv.cfg`), async Pytest runner.
- `crates/ai-engine`: AST-pruned prompt generator, mockable `LlmStreamingClient`, local Ollama client (`ollama-rs`), stream cancellation handles.
- `crates/app-iced`: Iced desktop application shell (`LideApp`, `EditorCommand`), TEA update transitions, virtualized viewport, split panels, keyboard shortcuts, embedded fonts.

For detailed architecture and specifications, see [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and [`aidocs/README.md`](aidocs/README.md).
