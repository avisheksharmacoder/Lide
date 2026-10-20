# Milestone 3: Minimal Floem Virtual Viewport (`app-floem`)

> **Goal**: Construct the reactive Floem visual shell where the GUI is a dumb, reactive projection of `editor-core`. Implement a virtualized viewport that shapes and paints strictly the visible 40–60 lines using `cosmic-text`, wired to fine-grained `RwSignal` state, and tested headlessly using `floem::headless`.

---

## 1. Scope & Crate Responsibility

- **Crate**: `crates/app-floem` (and the top-level binary in `src/main.rs`)
- **Dependencies Allowed**:
  - `editor-core`
  - `lang-contract`
  - `lang-python`
  - `floem` (Fine-grained reactive GUI framework)
  - `cosmic-text` (Text shaping and glyph layout)
  - `taffy` (Flexbox/Grid layout engine used internally by Floem)
  - `wgpu` / `tiny-skia` (Floem graphics backends)
  - `crossbeam-channel` (Thread-safe communication between async workers and Floem signals)
  - `floem::headless` (Dev dependency: headless UI testing harness)

---

## 2. Core Architecture & Signal Topology

### 2.1 Fine-Grained Reactive Signals
To eliminate unnecessary redraws, state is split into surgical signals:

```rust
pub struct EditorSignals {
    /// Increments on text edits; triggers viewport re-slice and syntax token fetching.
    pub buffer_rev: RwSignal<u64>,
    
    /// Current cursor positions and primary cursor coordinate; updates status bar & line highlight.
    pub primary_cursor: RwSignal<BufferPosition>,
    pub cursor_count: RwSignal<usize>,
    
    /// Vertical scroll offset in pixels or line units.
    pub scroll_offset: RwSignal<f64>,
    
    /// Visible line range currently displayed in the window (e.g. 120..175).
    pub visible_line_range: RwSignal<(LineIndex, LineIndex)>,
    
    /// Diagnostics list for gutter marks and problems view.
    pub diagnostics: RwSignal<im::Vector<Diagnostic>>,
    
    /// Structure outline tree for sidebar navigation.
    pub outline: RwSignal<Vec<OutlineSymbol>>,
    
    /// Current active file path & dirty state.
    pub active_file: RwSignal<Option<PathBuf>>,
    pub is_dirty: RwSignal<bool>,
}
```

### 2.2 Unidirectional Command Flow
All user interactions (keystrokes, mouse clicks, menu items) are converted into `EditorCommand` events:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorCommand {
    InsertChar(char),
    InsertText(String),
    DeleteBackward,
    DeleteForward,
    MoveCursor(MovementDirection, SelectionMode),
    AddCursorAt(LineIndex, ColumnIndex),
    ScrollLines(i32),
    ScrollPixels(f64),
    OpenFile(PathBuf),
    SaveFile,
    FormatDocument,
    Undo,
    Redo,
}
```

---

## 3. Required Implementation Tasks

### Task 3.1: Virtualized Editor Canvas
- Implement Floem view using `virtual_stack` (or a custom canvas implementing Floem's `VirtualVector` trait).
- Viewport Virtualization:
  - Measure line height from current font metrics using `cosmic-text` (e.g. 20.0 px per line).
  - Given canvas height (e.g. 1080 px), visible line count = `ceil(1080 / 20) + 2` padding lines (~56 lines).
  - Ask `editor-core` strictly for `lines[first_visible..last_visible]`.
  - Remaining thousands of lines in the buffer generate **zero widgets, zero text shaping, and zero GPU allocations**.

### Task 3.2: `cosmic-text` Line Shaping & Syntax Coloring
- For each visible line:
  - Retrieve syntax tokens from `lang-python` for the line's byte slice.
  - Build `cosmic_text::Buffer` with colored spans according to theme tokens (keywords, strings, functions, comments).
  - Shape text and paint glyphs onto Floem's render context.
  - Draw cursor rectangle(s) and selection background highlight boxes.

### Task 3.3: Line Number Gutter & Active Line Highlight
- Left-side gutter:
  - Dynamically sized based on total line count (e.g. 4 characters width for 10k lines).
  - Render line numbers for visible lines.
  - Display diagnostic markers (warning yellow dot, error red dot) next to line numbers.
- Active line highlight:
  - Render subtle background tint behind the line containing the primary cursor.

### Task 3.4: Three-Pane Layout (Taffy Flexbox)
- **Left Sidebar**:
  - Top tab: File explorer tree.
  - Bottom tab: Structure Outline tree (collapsible classes/functions synced with `outline` signal).
- **Center Main Viewport**:
  - Gutter + Editor Canvas + Horizontal/Vertical scrollbars.
- **Bottom Status Bar**:
  - Left: Active file name, encoding (`UTF-8`), line endings (`LF`).
  - Center: Cursor coordinates (`Ln 54, Col 12`), active selections count.
  - Right: Python interpreter version (`Python 3.12 (venv)`), memory usage (RSS MB).

---

## 4. Test-Driven Development (TDD) Plan

### Tier 3.1: Headless Signal Reactivity Tests (`floem_reactive`)
- Create a test without opening any window:
  ```rust
  #[test]
  fn test_typing_updates_buffer_rev_and_cursor() {
      let runtime = floem_reactive::create_runtime();
      let signals = EditorSignals::new();
      
      // Dispatch InsertChar('a')
      signals.dispatch(EditorCommand::InsertChar('a'));
      
      assert_eq!(signals.buffer_rev.get_untracked(), 1);
      assert_eq!(signals.primary_cursor.get_untracked().col, ColumnIndex(1));
      runtime.dispose();
  }
  ```

### Tier 3.2: Headless Viewport Scrolling Test (`floem::headless`)
- Mount the editor view in `floem::headless`.
- Load a synthetic 10,000-line buffer.
- Initial assertion: `visible_line_range.get()` is `(LineIndex(0), LineIndex(55))`.
- Simulate vertical scroll by 1,000 pixels (50 lines down).
- Assert:
  1. `visible_line_range.get()` updates to `(LineIndex(50), LineIndex(105))`.
  2. Total instantiated line view elements remains fixed (~56 items), proving memory bounded layout.

### Tier 3.3: Selection & Click-to-Move Test
- In headless test harness, simulate mouse click event at pixel position `(x: 120.0, y: 60.0)` (corresponding to line 3, column 10).
- Assert cursor position updates to `LineIndex(3), ColumnIndex(10)`.

---

## 5. Acceptance Criteria Checklist

- [ ] Window opens cleanly on Linux (Wayland / X11) with WGPU and falls back gracefully to `tiny-skia` on CPU.
- [ ] 10,000-line file displays its first frame in < 5ms without GUI stutter.
- [ ] Scrolling through a 10,000-line file stays at a smooth 60–120 FPS.
- [ ] Gutter displays correct line numbers and adapts width to line counts.
- [ ] Multi-cursors display distinct visual blinking carets and selection ranges.
- [ ] Headless Floem test suite passes in CI without an X11/Wayland display server.
