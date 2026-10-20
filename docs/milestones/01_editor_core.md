# Milestone 1: The Headless Rope & Viewport Kernel (`editor-core`)

> **Goal**: Build a 100% headless, pure Rust text-editing engine capable of loading a 10,000-line file in under 1 millisecond, supporting multi-cursor editing, grapheme-cluster-aware movement, infinite undo/redo, and viewport line slicing.

---

## 1. Scope & Crate Responsibility

- **Crate**: `crates/editor-core`
- **Dependencies Allowed**:
  - `ropey` (B-tree chunked text rope)
  - `unicode-segmentation` (Grapheme cluster and word boundary detection)
  - `unicode-width` (Visual column width calculation for tabs, CJK, and emojis)
  - `smallvec` (Inline optimization for multi-cursors)
  - `serde` (Serialization of edit actions and configurations)
  - `thiserror` (Clean error types)
  - `proptest` (Dev dependency: property-based fuzz testing)
  - `criterion` (Dev dependency: micro-benchmarks for 10k-line SLA)
- **Forbidden**: Zero GUI crates (`floem`, `winit`), zero async runtime (`tokio`), zero network, zero filesystem IO.

---

## 2. Core Architecture & Types

### 2.1 Coordinate Types
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteOffset(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CharIndex(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineIndex(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColumnIndex(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferPosition {
    pub line: LineIndex,
    pub col: ColumnIndex,
    pub offset: ByteOffset,
}
```

### 2.2 Cursor & Selection Model
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionAffinity {
    Upstream,
    Downstream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub anchor: BufferPosition,
    pub head: BufferPosition,
    pub affinity: SelectionAffinity,
    pub preferred_col: Option<ColumnIndex>, // Sticky column during vertical navigation
}

impl Cursor {
    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.head
    }
    pub fn min(&self) -> BufferPosition { ... }
    pub fn max(&self) -> BufferPosition { ... }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorSet {
    cursors: SmallVec<[Cursor; 4]>,
    primary_idx: usize,
}
```

### 2.3 Buffer & Undo/Redo Engine
```rust
pub struct Buffer {
    rope: ropey::Rope,
    cursors: CursorSet,
    history: HistoryTree,
    line_ending: LineEnding,
    tab_size: usize,
    revision: u64,
}

pub struct EditAction {
    pub delete_range: Option<(ByteOffset, ByteOffset)>,
    pub insert_text: Option<String>,
    pub cursor_before: CursorSet,
    pub cursor_after: CursorSet,
}
```

### 2.4 Viewport Math & Slicing
```rust
pub struct ViewportSlice {
    pub start_line: LineIndex,
    pub end_line: LineIndex,
    pub lines: Vec<RopeSlice<'static>>,
    pub total_lines: usize,
    pub byte_range: (ByteOffset, ByteOffset),
}
```

---

## 3. Required Implementation Tasks

### Task 1.1: Streaming Rope Loader & Line Indexer
- Wrap `ropey::Rope` with safe byte, char, and line conversion methods.
- Implement streaming loader: `Buffer::from_reader<R: std::io::Read>(reader: R) -> Result<Buffer, EditorError>`.
- Support LF (`\n`) and CRLF (`\r\n`) detection and normalization.

### Task 1.2: Grapheme-Aware Multi-Cursor Navigation
- Movement primitives:
  - Character left/right (respecting multi-byte UTF-8 and Unicode grapheme clusters).
  - Word left/right (using `unicode-segmentation`).
  - Line start, line end, first non-whitespace character.
  - Page up, page down, buffer start, buffer end.
  - Up/Down navigation preserving `preferred_col`.
- Selection expansion (`Shift + Movement`).
- Multi-cursor management:
  - Add cursor above/below.
  - Overlap resolution: automatically merge overlapping or intersecting selections into a single normalized cursor.

### Task 1.3: Text Edit Mutations
- Atomic batch operations:
  - `insert_char`, `insert_text` across all active cursors.
  - `delete_backwards` (Backspace) and `delete_forwards` (Delete).
  - Multi-line block indentation (Tab / Shift+Tab).
  - Auto-indentation on newline (`\n` copies leading whitespace of current line).
  - Comment toggle (prefix line with `#` or custom comment syntax).

### Task 1.4: Infinite Undo/Redo Tree
- Commit coalescing: group rapid consecutive typings within a configurable debounce threshold (e.g. 300ms) into a single undo step.
- Full state restoration: undoing an action restores the exact text and all cursor positions/selections prior to the change.
- Redo branch management (tree-based or linear stack).

### Task 1.5: Viewport Slicing Kernel
- Given a scroll offset in lines (or pixel height / line height) and visible window size (e.g. lines 100 to 155):
  - Slice visible lines from `ropey` in $O(\log N)$ time.
  - Calculate total line count and line number gutter width.
  - Return exact `ByteOffset` start and end bounds of visible slice for syntax highlighter queries.

---

## 4. Test-Driven Development (TDD) Plan

### Tier 1.1: Deterministic Unit Tests
- `test_utf8_grapheme_cluster_cursor_movement`:
  - Insert string `"Hello, 🌍! 👨‍👩‍👧‍👦"`
  - Verify navigating right step-by-step lands on valid grapheme cluster boundaries without splitting emojis.
- `test_multi_cursor_overlap_merge`:
  - Place cursor at offset 5 and cursor at offset 6.
  - Navigate right; assert cursors merge into a single cursor when their ranges intersect.
- `test_auto_indentation_on_newline`:
  - Line containing `"    def calculate(x):"`
  - Press enter; verify new line has at least 4 spaces (or 8 spaces if indentation rules apply).
- `test_undo_redo_roundtrip`:
  - Type text, delete range, undo twice, redo once; assert Rope checksum matches expected string.

### Tier 1.2: Property-Based Fuzz Testing (`proptest`)
- Generate 1,000 random sequences of operations:
  - Random insertions of ASCII, UTF-8 Chinese, Cyrillic, and Emoji strings.
  - Random multi-cursor placements.
  - Random backspaces, deletes, and multi-line selections.
  - Random undo/redo operations.
- Invariants to assert after **every single mutation**:
  1. No cursor offset ever points inside a multi-byte UTF-8 character boundary (`rope.byte_to_char(offset)` round-trips cleanly).
  2. Cursors in `CursorSet` are sorted by position and never overlap.
  3. `undo()` to the root state always yields the exact initial text.

### Tier 1.3: 10,000-Line SLA Benchmark (`criterion`)
- Benchmark loading a 10,000-line Python file (~400 KB) into `Buffer`.
- Assert runtime is **< 1.0 ms**.
- Benchmark viewport slicing: query lines 5,000–5,055 from a 10k-line buffer. Assert runtime is **< 50 microseconds**.

---

## 5. Acceptance Criteria Checklist

- [ ] All `editor-core` code compiles with `#![deny(clippy::all)]`.
- [ ] 100% of unit tests pass via `cargo test -p editor-core`.
- [ ] 1,000+ fuzz iterations pass in `cargo test -p editor-core --test fuzz`.
- [ ] Benchmark asserts 10k-line file load is < 1ms and viewport slice is < 50µs.
- [ ] Zero GUI or async dependencies in `editor-core/Cargo.toml`.
