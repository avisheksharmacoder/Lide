---
name: lide-language-support
description: "Step-by-step guide to implement a new language (Cython, C++, C, Jupyter, Rust) using Lide's trait contracts and Tree-sitter."
---

# Adding Language Support to Lide

This skill guides you through extending Lide with support for new programming languages (e.g. Cython, C++, C, Jupyter Notebooks, or Rust).

---

## Step 1: Register the Language Variant

In `crates/lang-contract/src/lib.rs`, add or verify the variant in `LanguageId`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageId {
    Python,
    Cython,
    PydBinaryStub,
    Rust,
    C,
    Cpp,
    JupyterNotebook, // new variant
}
```

---

## Step 2: Implement Language Contracts

Under `crates/` (e.g. within a specialized crate or module like `crates/lang-<name>`):

1. **Tree-sitter Syntax Highlighting**:
   Implement `SyntaxHighlighter` from `lang-contract`.
   - Wrap the official Tree-sitter parser grammar (e.g. `tree-sitter-c`, `tree-sitter-rust`).
   - Map AST nodes to `HighlightToken` (`Keyword`, `Function`, `Method`, `Class`, `Type`, `String`, `Number`, `Comment`, `Operator`).
2. **AST Outline Extraction**:
   Implement `AstAnalyzer` from `lang-contract`.
   - Extract hierarchical symbols into `OutlineSymbol`: classes, structs, functions, methods with start/end lines and symbol kinds (`SymbolKind`).
   - Implement `find_enclosing_context()` returning `EnclosingContext` (used by the AI assistant to slice the prompt).
3. **Deterministic Formatter (Optional)**:
   Implement `CodeFormatter` from `lang-contract` if an in-process Rust formatter exists (e.g. `rustfmt` or clang-format adapter).

---

## Step 3: Wire into the Workspace Engine

In `crates/workspace-engine/src/lib.rs`:

1. Update `classify_file_language(path: &Path) -> Option<LanguageId>` to associate file extensions (`.cpp`, `.hpp`, `.pyx`, `.rs`, `.ipynb`) with the new `LanguageId`.
2. Update the sidebar badge in `crates/app-floem/src/main.rs`:
   - E.g. `LanguageId::Rust => ("RS", Color::rgb8(...))`
   - E.g. `LanguageId::JupyterNotebook => ("NB", Color::rgb8(...))`

---

## Step 4: Wire into `EditorViewModel`

In `crates/app-floem/src/lib.rs`:
1. When a workspace file is opened (`EditorCommand::OpenWorkspaceFile`), select the appropriate highlighter and analyzer based on the file's `LanguageId`.
2. Trigger ephemeral analysis (`run_ephemeral_analysis_internal()`) to refresh the AST Structure outline and diagnostics.

---

## Step 5: TDD Verification

Create a new integration test:
```rust
#[test]
fn test_new_language_syntax_and_outline() {
    let source = "...";
    let mut highlighter = NewLangHighlighter::new();
    let spans = highlighter.highlight_line(0, source);
    assert!(!spans.is_empty());

    let analyzer = NewLangAnalyzer::new();
    let symbols = analyzer.analyze_symbols(source);
    assert_eq!(symbols[0].name, "expected_symbol");
}
```
Run `cargo test --workspace` to ensure all existing tests remain green.
