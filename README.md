# Lide Editor

A lightweight, pure-Rust desktop IDE built on [Floem](https://github.com/lapce/floem) with embedded Astral Ruff, Ripgrep, and local AI assistance.

---

> **Note:** This editor is currently under active development and is not yet ready for production use.

## Why Lide? (Rationale & Motivation)

Existing development environments like VS Code, PyCharm, and Neovim are mature and capable. However, they make distinct trade-offs:

1. **Memory & Runtime Overhead:** Modern feature-rich IDEs typically run on runtime layers—either the JVM or Electron/Chromium/Node.js. They commonly consume 1 GB to 3 GB of RAM, take seconds to start up, and rely on multiple background processes communicating over JSON-RPC IPC.
2. **Terminal vs. GUI Gap:** Terminal editors (such as Neovim or Helix) are fast and memory-efficient, but lack native GUI primitives for side-by-side test failure traceback navigation, interactive outline sidebars, and reactive token streaming without complex plugin configurations.
3. **External Daemon Latency:** Traditional editors invoke external daemons and formatters via subprocesses or language server protocols, introducing round-trip latency on simple actions like format-on-save or file search.

### The Problem Lide Solves
Lide was created to explore a single-binary, native alternative:
- **Zero Web Technologies:** Written entirely in Rust, rendering through Floem (WGPU / software rasterization) without web views, DOM reflows, or browser runtimes.
- **In-Process Python Tooling:** Directly embeds **Astral Ruff** (`ruff_python_parser`, `ruff_python_formatter`), **Tree-sitter**, and **Ripgrep** (`grep-searcher`) into the same binary. Parsing, formatting, and searching occur without IPC overhead or external process startup delays.
- **Sub-50ms Cold Start & ~40 MB RAM:** Launches immediately and idles at a fraction of the memory footprint of Electron- or JVM-based IDEs.
- **Local-First Privacy:** AI features connect directly to a local **Ollama** instance, slicing only the active function/class AST node to minimize token usage and keep code completely on-device.

---

## Feature List (One-Line Summary)

- **B-Tree Rope Buffer:** Powered by `ropey`, handling large files with $O(\log N)$ edits and sub-microsecond line/column indexing.
- **Virtualized Viewport:** Renders only visible rows (~36 lines), maintaining 60–120 FPS scrolling even on 10,000+ line files.
- **Python Smart Auto-Indentation:** Automatically carries forward indentation and adds 4 PEP 8 spaces on colons (`:`) when pressing `Enter`.
- **PEP 8 Indent & Dedent:** Indents or dedents the active line by 4 spaces via `Tab` and `Shift+Tab`.
- **In-Process Ruff AST Structure:** Parses Python code in microseconds to populate a live class, function, and method outline sidebar.
- **Atomic Format-on-Save (`Ctrl+S`):** Formats buffer contents using the deterministic Astral Ruff formatter and saves atomically with rename fallback.
- **Real-Time Diagnostics:** Displays syntax and Ruff error/warning glyphs in the gutter and summary counts in the status bar.
- **Ripgrep Regex Search (`Ctrl+Shift+F`):** Embedded multi-threaded regex search across workspace files with click-to-jump coordinates.
- **Non-Blocking Pytest Runner (`Ctrl+Shift+T`):** Streams test output in the background, parses `--tb=short` tracebacks, and provides clickable jump-to-failure navigation.
- **Virtualenv Auto-Detection:** Automatically discovers `.venv`, `venv`, Poetry, or `uv` environments and reads Python versions via `pyvenv.cfg` without subprocess calls.
- **AST-Pruned Local AI Assistant (`Ctrl+I`):** Streams suggestions from local Ollama models with prompt pruning focused strictly on the enclosing function or class.
- **One-Click AI Refactor Application:** Strips markdown code blocks and replaces the active AST line range with single-step `Ctrl+Z` undo support.
- **Transactional Undo / Redo (`Ctrl+Z` / `Ctrl+Y`):** Restores exact buffer states and cursor coordinates across edits.
- **Dual-Mode Left Sidebar:** Toggle between a workspace file tree with language badges and a live Ruff AST structure tree.
- **Granular Reactive Signals:** Floem signal architecture ensures typing, scrolling, test runs, or AI streams only update their respective UI components.

---

## Supported Languages

### Supported Now
- **Python (`.py`, `.pyi`)**
  - Incremental Tree-sitter syntax highlighting.
  - In-process Astral Ruff AST structure parsing and hierarchical outline.
  - In-process deterministic Astral Ruff code formatting.
  - Gutter diagnostics and syntax error detection.
  - Virtualenv detection and virtualenv-aware `pytest` execution.
  - AST-scoped local AI completions, docstrings, refactoring, and test generation.

### Upcoming / In-Progress
- **Cython (`.pyx`, `.pxd`):** Syntax highlighting, binary extension stub resolution, and mixed Python/C extension navigation.
- **C++ (`.cpp`, `.hpp`, `.cc`, `.h`):** Tree-sitter highlighting and native extension integration.
- **C (`.c`, `.h`):** Tree-sitter highlighting, C-API Python module inspection, and header stub navigation.
- **Jupyter Notebooks (`.ipynb`):** Native notebook JSON parsing, cell-based virtualized rendering, and code execution.
- **Rust (`.rs`):** Syntax highlighting and integration for PyO3 / maturin polyglot extension workflows.

---

## Keyboard Shortcuts

| Shortcut | Command | Description |
| :--- | :--- | :--- |
| **`Enter`** | `InsertNewlineAutoIndent` | Newline with Python colon smart auto-indent (+4 spaces). |
| **`Tab`** | `IndentLine` | Indent current line by 4 PEP 8 spaces. |
| **`Shift + Tab`** | `DedentLine` | Dedent current line by 4 PEP 8 spaces. |
| **`Ctrl + S`** | `SaveActiveFile` | Atomic save with Astral Ruff format-on-save. |
| **`Ctrl + Shift + F`** | `ToggleSearchPanel` | Toggle the bottom Ripgrep regex search panel. |
| **`Ctrl + Shift + T`** | `TogglePytestPanel` | Toggle the bottom Pytest runner panel. |
| **`Ctrl + I`** | `ToggleAiPane` | Toggle the right-hand Ollama AI Assistant pane. |
| **`Escape`** | `CancelAiStream` / `CancelPytestRun` | Abort active AI token stream or stop running Pytest process. |
| **`Ctrl + Z`** | `Undo` | Step backward in buffer edit history. |
| **`Ctrl + Shift + Z`** / **`Ctrl + Y`** | `Redo` | Step forward in buffer edit history. |
| **`Arrow Keys`** | `MoveCursor` | Move cursor Up, Down, Left, or Right. |
| **`Home` / `End`** | `MoveCursor` | Move cursor to the start or end of the current line. |

---

## Workspace Layout

```
Poom/
├── Cargo.toml                    # Virtual workspace root manifest
├── docs/                         # Specifications & documentation
│   ├── README.md                 # Project summary and guide
│   └── ARCHITECTURE.md           # Engineering specifications & SLAs
├── crates/
│   ├── editor-core/              # Rope buffer, cursor movement, auto-indent, undo/redo
│   ├── lang-contract/            # Language-agnostic traits, diagnostic types, AST contracts
│   ├── lang-python/              # Tree-sitter highlighter, Astral Ruff parser & formatter
│   ├── workspace-engine/         # File indexer, Ripgrep searcher, virtualenv detector, pytest runner
│   ├── ai-engine/                # AST context pruning & Ollama streaming client
│   └── app-floem/                # Floem native desktop GUI, virtual viewport & keybindings
└── target/                       # Build artifacts (git-ignored)
```

---

## Building and Running

### Prerequisites
- **Rust Toolchain:** Stable Rust 1.80+ ([rustup.rs](https://rustup.rs/))
- **Ollama (Optional):** Required only for local AI features ([ollama.com](https://ollama.com/))
  ```bash
  ollama pull qwen2.5-coder:7b
  ```

### Run the Editor
```bash
cargo run -p app-floem --release
```

### Run Test Suite
```bash
cargo test --workspace
```
