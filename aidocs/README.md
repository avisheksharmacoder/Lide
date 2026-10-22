# Lide AI Documentation & Engineering Manual (`aidocs`)

Welcome to the **`aidocs`** directory for **Lide Editor**. This folder is designed for AI pair programmers, autonomous agents, and contributors navigating, modifying, or extending the Lide codebase.

---

## 1. Executive Summary & Core Tenets

Lide is an ultra-fast, memory-lean, single-binary desktop Python & polyglot IDE fronted by **Iced** (native GUI via The Elm Architecture) and powered by embedded Rust engines (**Astral Ruff**, **Tree-sitter**, **Ripgrep**).

When working on this repository, you must preserve five foundational invariants:

1. **Strict Headless Core Decoupling**:
   - `crates/editor-core` has **zero** dependencies on Iced, Winit, GPU rendering, or OS windowing.
   - Core text buffer mechanics (B-tree Rope, multi-cursor physics, undo/redo transaction trees, smart auto-indentation, viewport math) must run synchronously in standard `cargo test` harnesses without requiring a display server or window handle.
2. **The Elm Architecture (TEA) & Deterministic Transitions**:
   - UI state is unified into `LideApp` in `crates/app-iced`.
   - All state mutations are strictly sequenced through deterministic commands in `LideApp::update()`.
   - Views are pure rendering functions (`views::render_app()`).
   - Fonts are embedded in-memory via `include_bytes!` (`fonts.rs` & `lucide-icons`), completely bypassing Windows GDI, font cache, and registry font resolution failures.
3. **Zero-Shift Caret & Text Layout**:
   - Never inject cursor characters (such as `"▎"`) directly into the code text stream.
   - The caret is rendered as an isolated stacked overlay (`views/editor.rs`) using a transparent text spacer and an explicit 2.5px width container, ensuring lines never shift left or right when active.
4. **Sub-Millisecond SLAs & Non-Blocking Async**:
   - Never perform file I/O, regex searches, AST parsing, or formatting directly on Iced's UI main thread.
   - Offload heavy tasks to background multi-threaded Tokio workers and marshal events back via native Iced `Task::stream` and `Subscription`.
   - Retain cooperative cancellation handles (`watch::Sender<bool>`) in `LideApp` to abort running AI streams and Pytest processes on `Escape` or Cancel.
5. **Strict Test-Driven Development (TDD)**:
   - Always write tests first (Red -> Green -> Refactor) for any new command, parser, or view layout.
   - All tests must pass in headless environments (`cargo test --workspace`).

---

## 2. Multi-Crate Architecture & Responsibilities

| Crate | Path | Role & Boundaries |
| :--- | :--- | :--- |
| **`editor-core`** | `crates/editor-core/` | Headless B-tree Rope buffer (`ropey`), `Position(line, col)` coordinate math, multi-cursor navigation, smart auto-indent (`:` -> +4 spaces), block indent/dedent (`Tab`/`Shift+Tab`), atomic line-range replacement, undo/redo transaction stack, and grapheme cluster counting (`unicode-segmentation`). |
| **`lang-contract`** | `crates/lang-contract/` | Language-agnostic traits & shared data models: `AstAnalyzer`, `SyntaxHighlighter`, `CodeFormatter`, `Diagnostic`, `OutlineSymbol`, `EnclosingContext`, `HighlightToken`, `LanguageId`, and `SymbolKind`. |
| **`lang-python`** | `crates/lang-python/` | Python intelligence: `tree-sitter-python` tokenization, embedded Astral Ruff AST parser (`ruff_python_parser`), ephemeral AST outline extractor with immediate arena deallocation, Astral Ruff formatter (`ruff_python_formatter`), and `.pyd`/`.so` virtual stub resolver. |
| **`workspace-engine`** | `crates/workspace-engine/` | Filesystem indexing (`ignore::WalkBuilder`), native Ripgrep regex searcher (`grep-searcher`), zero-subprocess virtualenv detector (`pyvenv.cfg`), and non-blocking `pytest` execution with `--tb=short` traceback parsing and cooperative cancellation. |
| **`ai-engine`** | `crates/ai-engine/` | AST-scoped prompt generator (`EnclosingContext` innermost scope slicing), mockable `LlmStreamingClient` trait, local Ollama streaming client (`ollama-rs`), real-time `tokio::sync::mpsc` event streamer, and task cancellation handles. |
| **`app-iced`** | `crates/app-iced/` | Iced desktop application shell: Elm Architecture (`LideApp`, `EditorCommand`), virtualized viewport rendering, zero-shift stacked caret overlay, lock-free atomic mouse tracking, OS keyboard shortcut & text resolution, interactive draggable sidebar splitter, split panels (Ripgrep, Pytest), AI pane, and dynamic theme engine. |

---

## 3. Deep Dive into `app-iced` Presentation Architecture

### 3.1 The Elm Architecture (TEA) State Cycle
$$\text{LideApp} \times \text{EditorCommand} \xrightarrow{\text{update()}} \text{LideApp} \times \text{Task}\langle\text{EditorCommand}\rangle$$

- **`LideApp`**: Central struct holding editor buffer, active workspace root, file manager tree, outline items, active theme, dialog states, and cancel handles.
- **`EditorCommand`**: Exhaustive enum covering buffer edits, navigation, sidebar resizing, search queries, test triggers, theme selection, and streaming events.
- **`LideApp::dispatch(&mut self, cmd)`**: Synchronous dispatch helper used in headless unit and integration tests.

### 3.2 View Component Hierarchy (`views/mod.rs`)
```
main_shell
├── toolbar::render_toolbar (Top Bar & Brand)
├── center_area (Row)
│   ├── sidebar::render_file_manager (Project Tree & AST Structure)
│   ├── Splitter Handle (4px Draggable Mouse Area)
│   ├── editor_column (Column)
│   │   ├── editor::render_editor (Virtualized Code Lines & Gutter)
│   │   └── Drawer: search::render_search_panel OR pytest::render_pytest_panel
│   └── ai_pane::render_ai_pane (Right AI Assistant Split Pane)
└── status_bar::render_status_bar (Coordinates, Encoding, Venv, AI Dot)
```

### 3.3 Modal Layering & Non-Shifting Floating Overlays
- Dropdown menus (File dropdown), file tree right-click context menus, and modal dialogs (Settings dialog, Font picker) are stacked on top of `main_shell` using Iced `stack(layers)`.
- Stacking prevents floating elements from pushing down or reflowing the editor canvas.

### 3.4 Virtualized Viewport & Stacked Caret Geometry (`views/editor.rs`)
- Only lines in the visible pixel window are rendered:
  - `start_line = (scroll_offset_y / line_height_px).floor() as usize`
  - `visible_count = (viewport_height_px / line_height_px).ceil() as usize`
  - Top and bottom spacers preserve exact scrollbar geometry.
- The caret is rendered using a stacked container:
  - Base layer: `code_row` with syntax tokens.
  - Overlay layer: `caret_overlay` consisting of transparent text matching the prefix before the cursor, followed by an explicit 2.5px wide accent bar.
  - Both layers use `width(Length::Fill)` within `stack`, eliminating text shift and preventing caret clipping at line ends.

### 3.5 Lock-Free Mouse Tracking & Column Calculation
- `subscriptions/mod.rs`: `LAST_MOUSE_X` and `LAST_MOUSE_Y` stored as `AtomicU32` updated on `iced::mouse::Event::CursorMoved` without memory allocations or TEA re-renders.
- Clicking an editor line (`EditorCommand::ClickEditorLine { line_idx }`) computes the column via:
  $$\text{code\_start\_x} = \text{sidebar\_width} + 4\text{px (splitter)} + \text{gutter\_width}$$
  $$\text{rel\_x} = \max(0, \text{mouse\_x} - \text{code\_start\_x})$$
  $$\text{col} = \min\left(\mathrm{round}\left(\frac{\text{rel\_x}}{\text{font\_size} \times 0.60}\right), \text{line\_len}\right)$$
- Clicking below the code in the bottom spacer (`ClickEditorEmptySpace`) jumps directly to the end of the file.

### 3.6 Keyboard Event Resolution (`subscriptions/keyboard.rs`)
- `resolve_key_event` prioritizes OS-provided `text` (respecting Shift, CapsLock, dead keys, and international layouts).
- Shifted punctuation symbols (`:`, `"`, `?`, `!`, `@`, `#`) are mapped through `modified_key`.
- Non-printable ASCII control characters are automatically filtered out.

---

## 4. Coordinate Systems & Indexing Rigor

To prevent multi-byte UTF-8 crashes and off-by-one errors, follow these coordinate rules:

- **Buffer Coordinates (`editor-core`)**:
  - `line`: 0-indexed line number (`0..rope.len_lines()`).
  - `column`: 0-indexed Unicode character/grapheme offset within that line.
  - Conversions between byte offsets and character indices must always use `rope.char_to_byte()` and `rope.byte_to_char()`.
  - Line boundary queries use `buffer.line_grapheme_len(line_idx)`.
- **Search & Tooling Coordinates (`workspace-engine`)**:
  - Ripgrep (`SearchMatch`) emits **1-based** line numbers (`1..=N`) and **0-based** column offsets.
  - When opening search hits or test failures, translate to editor coordinates via:
    `Position::new(hit.line_number.saturating_sub(1), hit.column)`.
- **Pytest Traceback Coordinates**:
  - Tracebacks (`test_file.py:42: AssertionError`) contain **1-based** line numbers. Translate to `line_number.saturating_sub(1)`.

---

## 5. Key Developer & Agent Workflows

### Running Headless Tests Across All Crates
```bash
cargo test --workspace
```

### Running Specific Crate TDD Suites
```bash
# Verify editor core Rope, multi-cursor, undo/redo, auto-indent
cargo test -p editor-core

# Verify Astral Ruff AST parsing & formatting
cargo test -p lang-python

# Verify Ripgrep searcher & async pytest runner
cargo test -p workspace-engine

# Verify AI prompt builder & Ollama client
cargo test -p ai-engine

# Verify Iced TEA update commands, caret overlay, mouse click & subscriptions
cargo test -p app-iced
```

### Benchmarking
```bash
cargo bench -p editor-core
```

### Launching the Desktop Application
```bash
cargo run -p app-iced --release
```

---

## 6. Guidelines for Adding New Language Support

When adding support for a new language (e.g. Cython, C++, C, Rust, or Jupyter):

1. **Register Language ID**: Add the variant to `LanguageId` in `crates/lang-contract/src/lib.rs`.
2. **Implement Contracts**:
   - Implement `SyntaxHighlighter` using Tree-sitter.
   - Implement `AstAnalyzer` to extract hierarchical symbols (`OutlineSymbol`) and enclosing scopes (`EnclosingContext`).
   - Implement `CodeFormatter` for format-on-save support.
3. **Wire into Workspace Engine & Views**:
   - Update `crates/workspace-engine/src/lib.rs` (`classify_file_language`) to associate extensions with the new `LanguageId`.
   - Update `crates/app-iced/src/views/sidebar.rs` to render the language icon/badge in the file tree.
   - Update `crates/app-iced/src/lib.rs` to route file opening and analysis to the new language implementation.
4. **Maintain Pure Headless Unit Tests**: Write full test suites validating syntax highlighting tokens and outline extraction before touching UI components.

---

## 7. AI Assistant Entrypoints & Skills

To ensure any AI assistant automatically adheres to Lide's engineering standards, the following entrypoints are configured:

| Assistant | Entrypoint File | Purpose |
| :--- | :--- | :--- |
| **Claude / Claude Code** | [`CLAUDE.md`](../CLAUDE.md) | Injected into Claude's prompt at the start of every session. |
| **Cursor** | [`.cursorrules`](../.cursorrules) | Injected into Cursor's agent and inline completion context. |
| **Gemini / Antigravity** | [`GEMINI.md`](../GEMINI.md) | Loaded as workspace rules for Gemini and Antigravity agents. |

### Available AI Skills (`aidocs/skills/` & `.agents/skills/`)

- **[`lide-iced-ui`](skills/lide-iced-ui/SKILL.md)**: The Elm Architecture (TEA), widget layouts, zero-shift caret overlays, lock-free mouse coordinates, and native Tokio streaming tasks.
- **[`lide-tdd`](skills/lide-tdd/SKILL.md)**: Procedure for Red-Green-Refactor test-driven development in `app-iced` without a display server.
- **[`lide-language-support`](skills/lide-language-support/SKILL.md)**: Step-by-step guide to implement a new language (Cython, C++, C, Jupyter, Rust) using trait contracts.
- **[`lide-floem-ui`](skills/lide-floem-ui/SKILL.md)**: *(Deprecated)* Legacy Floem reference superseded by `lide-iced-ui`.


