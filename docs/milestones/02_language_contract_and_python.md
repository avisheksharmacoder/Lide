# Milestone 2: Polyglot Contract & Python AST Engine (`lang-contract` + `lang-python`)

> **Goal**: Establish a language-agnostic plugin contract and implement the pure Python intelligence engine using `tree-sitter-python` for range-bounded syntax highlighting and embedded Ruff crates (`ruff_python_parser`, `ruff_linter`, `ruff_python_formatter`) for ephemeral AST inspection, structure outline extraction, and sub-millisecond linting.

---

## 1. Scope & Crate Responsibility

### 1.1 `crates/lang-contract`
- **Responsibility**: Language-agnostic traits, token styling models, diagnostic structures, and AST outline trees.
- **Dependencies**: Only `editor-core` (for coordinates like `ByteOffset`, `LineIndex`) and `serde`.

### 1.2 `crates/lang-python`
- **Responsibility**: Concrete Python support implementing `lang-contract`.
- **Dependencies**:
  - `lang-contract`
  - `tree-sitter` (Fast incremental syntax tree)
  - `tree-sitter-python` (Grammar rules for Python)
  - `ruff_python_parser` (Official Ruff Rust parser)
  - `ruff_linter` (Official Ruff linter rules)
  - `ruff_python_formatter` (Official Ruff code formatter)
  - `insta` (Dev dependency: snapshot testing for AST and lint spans)

---

## 2. Core Architecture & Trait Contracts

### 2.1 The `LanguageService` Trait (`lang-contract`)
```rust
pub trait LanguageService: Send + Sync {
    /// Incremental syntax token generator bounded to visible byte offsets.
    fn highlight_range(
        &self,
        rope: &ropey::Rope,
        byte_range: (ByteOffset, ByteOffset),
    ) -> Vec<SyntaxToken>;

    /// Extract structure symbols (classes, methods, functions) for the Structure sidebar.
    fn extract_outline(&self, rope: &ropey::Rope) -> Result<Vec<OutlineSymbol>, LanguageError>;

    /// Lint the buffer and return compiler/linter diagnostics.
    fn lint(&self, rope: &ropey::Rope) -> Result<Vec<Diagnostic>, LanguageError>;

    /// Format the entire buffer or a selected byte range.
    fn format(&self, rope: &ropey::Rope, range: Option<(ByteOffset, ByteOffset)>) -> Result<String, LanguageError>;

    /// Indentation hint for the editor-core when pressing Enter.
    fn compute_indent(&self, rope: &ropey::Rope, line: LineIndex, tab_size: usize) -> IndentAction;
}
```

### 2.2 Shared Contract Types
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightCategory {
    Keyword,
    FunctionDef,
    FunctionCall,
    ClassDef,
    StringLiteral,
    NumberLiteral,
    Comment,
    Docstring,
    TypeAnnotation,
    Operator,
    Variable,
    Punctuation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxToken {
    pub range: (ByteOffset, ByteOffset),
    pub category: HighlightCategory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub range: (ByteOffset, ByteOffset),
    pub line: LineIndex,
    pub col: ColumnIndex,
    pub severity: DiagnosticSeverity,
    pub code: String,          // e.g. "F401", "E501"
    pub message: String,
    pub fix: Option<CodeAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineSymbol {
    pub name: String,
    pub kind: SymbolKind,     // Class, Function, Method, Field
    pub line: LineIndex,
    pub byte_offset: ByteOffset,
    pub children: Vec<OutlineSymbol>,
}
```

---

## 3. Required Implementation Tasks

### Task 2.1: Range-Bounded Tree-Sitter Highlighting (`lang-python`)
- Maintain an incremental `tree_sitter::Tree` updated on buffer edits in $O(\log N)$ time.
- Implement `highlight_range`:
  - When the viewport displays lines 0–55, set `QueryCursor::set_byte_range` strictly to the byte offsets of lines 0–55.
  - Execute Tree-sitter query pattern matches only on the visible range.
  - Return flat `Vec<SyntaxToken>` with exact byte offsets.
  - Benchmark visible range highlighting on a 10,000-line file: must finish in **< 1.5 ms**.

### Task 2.2: Ephemeral Ruff AST & Outline Extraction
- Run `ruff_python_parser::parse_module` on buffer content.
- Walk the parsed AST to extract hierarchical `OutlineSymbol` tree (classes, functions, async methods, properties).
- **Ephemeral Rule**: Once the outline and lint structs are generated, the heavy `ruff_python_ast` node tree is **immediately dropped** to avoid long-term memory bloat.

### Task 2.3: In-Memory Ruff Linting
- Execute `ruff_linter` rule set (e.g. Pyflakes, pycodestyle, flake8-bugbear) on the buffer text.
- Map Ruff's diagnostic ranges (`TextRange`) to Poom's strongly typed `ByteOffset`, `LineIndex`, and `ColumnIndex`.
- Include autofix instructions (`CodeAction`) when Ruff provides quick-fixes (e.g. removing unused imports).

### Task 2.4: In-Memory Ruff Formatter
- Implement `format()` using `ruff_python_formatter::format_module`.
- Return clean formatted string without writing intermediate files to disk.
- Compute minimal text diffs against current rope to preserve cursor position during format.

---

## 4. Test-Driven Development (TDD) Plan

### Tier 2.1: Snapshot Testing (`insta`)
- Create test fixtures with various Python syntax constructs:
  - Standard class and method definitions.
  - Python 3.10+ `match / case` pattern matching.
  - Complex type hints (`Union`, `Optional`, Generic aliases).
  - Docstrings, multiline strings, and f-strings with nested expressions.
- Assert snapshot files for:
  - `insta::assert_yaml_snapshot!("python_syntax_highlighting", tokens)`
  - `insta::assert_yaml_snapshot!("python_outline_tree", outline)`
  - `insta::assert_yaml_snapshot!("python_ruff_diagnostics", diagnostics)`

### Tier 2.2: Memory Leak & Drop Assertion Test
- In a test function, parse a 10,000-line Python file with Ruff 100 times in a loop.
- Assert that peak memory does not continuously climb and drops back to baseline after each iteration (confirming no lingering AST allocations).

### Tier 2.3: 10,000-Line Syntax SLA Test
- Initialize a 10,000-line Python file.
- Perform Tree-sitter range-bounded highlight on lines 5,000 to 5,055.
- Assert query execution takes **< 1.5 ms**.

---

## 5. Acceptance Criteria Checklist

- [ ] `lang-contract` provides zero-cost traits with no GUI or heavy async dependencies.
- [ ] `lang-python` implements all methods of `LanguageService`.
- [ ] Tree-sitter queries support range bounding without full-file traversals.
- [ ] Snapshot tests via `insta` cover highlight tokens, outline symbols, and Ruff diagnostics.
- [ ] Ephemeral AST lifecycle ensures zero memory leaks across consecutive edits.
- [ ] 100% of tests pass via `cargo test -p lang-python`.
