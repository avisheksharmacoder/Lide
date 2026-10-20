# Lide AI Documentation & Engineering Manual (`aidocs`)

Welcome to the **`aidocs`** directory for **Lide Editor**. This folder is designed for AI pair programmers, autonomous agents, and contributors navigating, modifying, or extending the Lide codebase.

---

## 1. Executive Summary & Core Tenets

Lide is a pure-Rust, single-binary desktop Python & polyglot IDE fronted by **Floem** (native GUI) and powered by embedded Rust engines (**Astral Ruff**, **Tree-sitter**, **Ripgrep**).

When working on this repository, you must preserve four foundational invariants:

1. **Strict GUI Decoupling**:
   - `crates/editor-core` has **zero** dependencies on Floem, Winit, GPU rendering, or OS windowing.
   - Core text buffer mechanics (Rope, cursor physics, undo/redo trees, auto-indentation, viewport math) must run synchronously in standard `cargo test` harnesses without a window or GPU context.
2. **Granular Signal Reactivity**:
   - UI state is partitioned into isolated `RwSignal<T>` primitives in `crates/app-floem`.
   - Never trigger monolithic full-screen redraws. Scrolling updates only viewport indices; typing updates the active row and cursor; test runs update the test panel; AI token streaming updates only the assistant view.
3. **Sub-Millisecond SLAs & Non-Blocking Async**:
   - Never perform file I/O, regex searches, AST parsing, or formatting directly on Floem's UI main thread.
   - Offload heavy tasks to background multi-threaded Tokio workers and marshal events back via `floem::ext_event::create_ext_action`.
4. **Strict Test-Driven Development (TDD)**:
   - Always write tests first (Red -> Green -> Refactor) for any new command, parser, or signal handler.
   - All tests must pass in headless environments without display servers.

---

## 2. Multi-Crate Architecture & Responsibilities

| Crate | Path | Role & Boundaries |
| :--- | :--- | :--- |
| **`editor-core`** | `crates/editor-core/` | Headless B-tree Rope buffer (`ropey`), `Position(line, col)` math, multi-cursor navigation, smart auto-indent (`:` -> +4 spaces), `Tab`/`Shift+Tab`, atomic line-range replacement, undo/redo transaction stack. |
| **`lang-contract`** | `crates/lang-contract/` | Language-agnostic traits & shared data models: `AstAnalyzer`, `SyntaxHighlighter`, `CodeFormatter`, `Diagnostic`, `OutlineSymbol`, `EnclosingContext`, `HighlightToken`, `LanguageId`. |
| **`lang-python`** | `crates/lang-python/` | Python intelligence: `tree-sitter-python` tokenization, embedded Astral Ruff AST parser (`ruff_python_parser`), ephemeral AST outline extractor, Astral Ruff formatter (`ruff_python_formatter`), and `.pyd`/`.so` stub resolver. |
| **`workspace-engine`** | `crates/workspace-engine/` | Filesystem indexing (`ignore::WalkBuilder`), native Ripgrep regex searcher (`grep-searcher`), zero-subprocess virtualenv detector (`pyvenv.cfg`), and non-blocking `pytest` execution with `--tb=short` traceback parsing. |
| **`ai-engine`** | `crates/ai-engine/` | AST-scoped prompt generator, mockable `LlmStreamingClient` trait, local Ollama streaming client (`ollama-rs`), real-time `mpsc` event streamer, and task cancellation handles. |
| **`app-floem`** | `crates/app-floem/` | Floem desktop application shell: reactive `EditorViewModel`, `EditorSignals`, virtualized viewport rendering, split panels (Ripgrep, Pytest), AI pane, status bar, and keyboard shortcuts. |

---

## 3. Coordinate Systems & Indexing Rigor

To prevent multi-byte UTF-8 crashes and off-by-one errors, follow these coordinate rules:

- **Buffer Coordinates (`editor-core`)**:
  - `line`: 0-indexed line number (`0..rope.len_lines()`).
  - `column`: 0-indexed Unicode character/grapheme offset within that line.
  - Conversions between byte offsets and character indices must always use `rope.char_to_byte()` and `rope.byte_to_char()`.
- **Search & Tooling Coordinates (`workspace-engine`)**:
  - Ripgrep (`SearchMatch`) emits **1-based** line numbers (`1..=N`) and **0-based** column offsets.
  - When opening search hits or test failures, translate to editor coordinates via:
    `Position::new(hit.line_number.saturating_sub(1), hit.column)`.
- **Pytest Traceback Coordinates**:
  - Tracebacks (`test_file.py:42: AssertionError`) contain **1-based** line numbers. Translate to `line_number.saturating_sub(1)`.

---

## 4. Key Developer & Agent Workflows

### Running Headless Tests Across All Crates
```bash
cargo test --workspace
```

### Running Specific Crate TDD Suites
```bash
# Verify editor core Rope, undo/redo, auto-indent
cargo test -p editor-core

# Verify Astral Ruff AST parsing & formatting
cargo test -p lang-python

# Verify Ripgrep searcher & async pytest runner
cargo test -p workspace-engine

# Verify AI prompt builder & Ollama client
cargo test -p ai-engine

# Verify Floem view model signals & Tokio bridge
cargo test -p app-floem
```

### Benchmarking
```bash
cargo bench -p editor-core
```

### Launching the Desktop Application
```bash
cargo run -p app-floem --release
```

---

## 5. Guidelines for Adding New Language Support

When adding support for a new language (e.g. Cython, C++, C, Rust, or Jupyter):

1. **Register Language ID**: Add the variant to `LanguageId` in `crates/lang-contract/src/lib.rs`.
2. **Implement Contracts**:
   - Implement `SyntaxHighlighter` using Tree-sitter.
   - Implement `AstAnalyzer` to extract hierarchical symbols (`OutlineSymbol`) and enclosing scopes (`EnclosingContext`).
   - Implement `CodeFormatter` for format-on-save support.
3. **Wire into View Model**:
   - Update file indexing in `crates/workspace-engine/src/lib.rs` to associate extensions with the new `LanguageId`.
   - Update `EditorViewModel` in `crates/app-floem/src/lib.rs` to route analysis and highlighting to the language implementation.
4. **Maintain Pure Headless Unit Tests**: Write full test suites validating syntax highlighting tokens and outline extraction before touching UI components.

---

## 6. AI Assistant Entrypoints & Skills

To ensure any AI assistant automatically adheres to Lide's engineering standards, the following entrypoints are configured:

| Assistant | Entrypoint File | Purpose |
| :--- | :--- | :--- |
| **Claude / Claude Code** | [`CLAUDE.md`](../CLAUDE.md) | Injected into Claude's prompt at the start of every session. |
| **Cursor** | [`.cursorrules`](../.cursorrules) | Injected into Cursor's agent and inline completion context. |
| **Gemini / Antigravity** | [`GEMINI.md`](../GEMINI.md) | Loaded as workspace rules for Gemini and Antigravity agents. |

### Available AI Skills (`aidocs/skills/` & `.agents/skills/`)

- **[`lide-tdd`](skills/lide-tdd/SKILL.md)**: Procedure for Red-Green-Refactor test-driven development without a display server.
- **[`lide-language-support`](skills/lide-language-support/SKILL.md)**: Step-by-step guide to implement a new language (Cython, C++, C, Jupyter, Rust) using trait contracts.
- **[`lide-floem-ui`](skills/lide-floem-ui/SKILL.md)**: Patterns for Floem reactive signals, `create_ext_action` stream pumps, and zero-redraw layouts.

