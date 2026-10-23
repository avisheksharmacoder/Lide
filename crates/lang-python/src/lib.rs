use std::ops::Range;
use std::path::{Path, PathBuf};

use lang_contract::{
    AnalysisReport, AstAnalyzer, BinaryStubInterceptor, CodeFormatter, Diagnostic,
    DiagnosticSeverity, EnclosingContext, FormatError, HighlightSpan, HighlightToken,
    InputEditSummary, LanguageId, OutlineSymbol, StubResolution, SymbolKind, SyntaxHighlighter,
};
use ruff_python_ast::{ExceptHandler, PySourceType, Stmt, StmtClassDef, StmtFunctionDef};
use ruff_python_formatter::{format_module_source, PyFormatOptions};
use ruff_python_parser::parse_unchecked_source;
use ruff_text_size::Ranged;
use tree_sitter::{InputEdit, Node, Parser, Point, Tree};

// ============================================================================
// 1. INCREMENTAL VIEWPORT-BOUNDED TREE-SITTER HIGHLIGHTER
// ============================================================================

/// Incremental `tree-sitter-python` highlighter that only evaluates visible viewport byte ranges.
pub struct PythonSyntaxHighlighter {
    parser: Parser,
    tree: Option<Tree>,
}

impl PythonSyntaxHighlighter {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        let language = tree_sitter_python::LANGUAGE.into();
        parser
            .set_language(&language)
            .expect("Failed to load tree-sitter-python grammar");

        Self { parser, tree: None }
    }
}

impl Default for PythonSyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxHighlighter for PythonSyntaxHighlighter {
    fn language(&self) -> LanguageId {
        LanguageId::Python
    }

    fn parse_full(&mut self, source: &str) {
        self.tree = self.parser.parse(source, None);
    }

    fn apply_edit(&mut self, updated_source: &str, edit: &InputEditSummary) {
        if let Some(tree) = self.tree.as_mut() {
            let ts_edit = InputEdit {
                start_byte: edit.start_byte,
                old_end_byte: edit.old_end_byte,
                new_end_byte: edit.new_end_byte,
                start_position: Point::new(edit.start_pos.line, edit.start_pos.col),
                old_end_position: Point::new(edit.old_end_pos.line, edit.old_end_pos.col),
                new_end_position: Point::new(edit.new_end_pos.line, edit.new_end_pos.col),
            };
            tree.edit(&ts_edit);
        }
        self.tree = self.parser.parse(updated_source, self.tree.as_ref());
    }

    fn highlight_viewport(
        &mut self,
        _source: &str,
        viewport_bytes: Range<usize>,
    ) -> Vec<HighlightSpan> {
        let Some(tree) = self.tree.as_ref() else {
            return Vec::new();
        };
        if viewport_bytes.is_empty() {
            return Vec::new();
        }

        let mut spans = Vec::with_capacity(128);
        let root = tree.root_node();

        // Use a TreeCursor with O(log N) byte seeking at the top level so we skip
        // all statements preceding `viewport_bytes.start` without walking them.
        let mut cursor = root.walk();
        if cursor
            .goto_first_child_for_byte(viewport_bytes.start)
            .is_some()
        {
            loop {
                let node = cursor.node();
                if node.start_byte() >= viewport_bytes.end {
                    break;
                }
                collect_viewport_spans(node, &viewport_bytes, &mut spans);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        spans
    }
}

/// Recursively visits only AST nodes overlapping `viewport_bytes` and emits semantic tokens.
fn collect_viewport_spans(
    node: Node<'_>,
    viewport_bytes: &Range<usize>,
    out: &mut Vec<HighlightSpan>,
) {
    let start = node.start_byte();
    let end = node.end_byte();

    // Prune entire subtrees outside the visible viewport
    if end <= viewport_bytes.start || start >= viewport_bytes.end {
        return;
    }

    let kind = node.kind();

    // Atomic leaf-like nodes (do not descend into string interpolation children for flat token spans)
    if let Some(token) = classify_atomic_node(kind) {
        let clamped_start = start.max(viewport_bytes.start);
        let clamped_end = end.min(viewport_bytes.end);
        if clamped_start < clamped_end {
            out.push(HighlightSpan {
                byte_range: clamped_start..clamped_end,
                token,
            });
        }
        return;
    }

    if node.child_count() == 0 {
        if let Some(token) = classify_leaf_node(node, kind) {
            let clamped_start = start.max(viewport_bytes.start);
            let clamped_end = end.min(viewport_bytes.end);
            if clamped_start < clamped_end {
                out.push(HighlightSpan {
                    byte_range: clamped_start..clamped_end,
                    token,
                });
            }
        }
        return;
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.start_byte() >= viewport_bytes.end {
                break;
            }
            if child.end_byte() > viewport_bytes.start {
                collect_viewport_spans(child, viewport_bytes, out);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

#[inline]
fn classify_atomic_node(kind: &str) -> Option<HighlightToken> {
    match kind {
        "string" | "concatenated_string" => Some(HighlightToken::String),
        "comment" => Some(HighlightToken::Comment),
        "integer" | "float" => Some(HighlightToken::Number),
        "true" | "false" => Some(HighlightToken::Boolean),
        _ => None,
    }
}

#[inline]
fn classify_leaf_node(node: Node<'_>, kind: &str) -> Option<HighlightToken> {
    if kind == "identifier" {
        if let Some(parent) = node.parent() {
            // Check if this identifier is part of the decorator path/name (e.g. @app.get)
            let mut curr = parent;
            let mut in_decorator = false;
            let mut in_arg_list = false;
            loop {
                if curr.kind() == "argument_list" {
                    in_arg_list = true;
                }
                if curr.kind() == "decorator" {
                    in_decorator = true;
                    break;
                }
                if curr.kind().ends_with("_statement")
                    || curr.kind() == "block"
                    || curr.kind() == "module"
                {
                    break;
                }
                match curr.parent() {
                    Some(p) => curr = p,
                    None => break,
                }
            }
            if in_decorator && !in_arg_list {
                return Some(HighlightToken::Decorator);
            }

            match parent.kind() {
                "class_definition" => {
                    if parent.child_by_field_name("name") == Some(node) {
                        return Some(HighlightToken::Class);
                    }
                }
                "function_definition" => {
                    if parent.child_by_field_name("name") == Some(node) {
                        return Some(HighlightToken::Function);
                    }
                }
                "call" => {
                    if parent.child_by_field_name("function") == Some(node) {
                        return Some(HighlightToken::Function);
                    }
                }
                "decorator" => return Some(HighlightToken::Decorator),
                "type" => return Some(HighlightToken::Type),
                "typed_parameter" | "default_parameter" | "parameters" => {
                    return Some(HighlightToken::Parameter);
                }
                _ => {}
            }
        }
        return None;
    }

    if !node.is_named() {
        return match kind {
            "def" | "class" | "return" | "import" | "from" | "as" | "async" | "await" | "if"
            | "elif" | "else" | "for" | "while" | "try" | "except" | "finally" | "with"
            | "pass" | "break" | "continue" | "lambda" | "yield" | "raise" | "in" | "is"
            | "not" | "and" | "or" | "match" | "case" | "global" | "nonlocal" | "assert"
            | "del" => Some(HighlightToken::Keyword),
            "@" => Some(HighlightToken::Decorator),
            "." => {
                let mut curr = node.parent();
                while let Some(p) = curr {
                    if p.kind() == "argument_list" {
                        break;
                    }
                    if p.kind() == "decorator" {
                        return Some(HighlightToken::Decorator);
                    }
                    if p.kind().ends_with("_statement") || p.kind() == "block" || p.kind() == "module" {
                        break;
                    }
                    curr = p.parent();
                }
                None
            }
            "=" | "+" | "-" | "*" | "/" | "//" | "%" | "**" | "->" | "==" | "!=" | "<" | "<="
            | ">" | ">=" | "+=" | "-=" | "*=" | "/=" => Some(HighlightToken::Operator),
            _ => None,
        };
    }

    None
}

// ============================================================================
// 2. EPHEMERAL RUFF AST ANALYZER & AI CONTEXT SLICER
// ============================================================================

/// Ephemeral AST analyzer powered by `ruff_python_parser`.
/// Extracts `OutlineSymbol` and `Diagnostic` vectors and immediately frees the Ruff AST.
#[derive(Default)]
pub struct PythonAstAnalyzer;

impl PythonAstAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl AstAnalyzer for PythonAstAnalyzer {
    fn language(&self) -> LanguageId {
        LanguageId::Python
    }

    fn analyze_ephemeral(&self, source: &str) -> AnalysisReport {
        let line_index = LineIndex::new(source);
        let parsed = parse_unchecked_source(source, PySourceType::Python);

        let mut symbols = Vec::new();
        let mut diagnostics = Vec::new();

        // 1. Extract recovered syntax errors from Ruff's parser
        for err in parsed.errors() {
            let start = err.location.start().to_usize();
            let end = err.location.end().to_usize();
            let (line, col) = line_index.line_col(start);
            diagnostics.push(Diagnostic {
                byte_range: start..end,
                line,
                col,
                severity: DiagnosticSeverity::Error,
                code: "SyntaxError".to_string(),
                message: err.error.to_string(),
            });
        }

        // 2. Walk the recovered AST to extract structure symbols and lint diagnostics
        let module = parsed.syntax();
        for stmt in &module.body {
            if let Some(sym) = extract_outline_symbol(stmt, source, &line_index, false) {
                symbols.push(sym);
            }
            collect_lint_diagnostics(stmt, &line_index, &mut diagnostics);
        }

        // 3. Explicitly drop `parsed` so the full Ruff AST allocation is freed immediately
        drop(parsed);

        AnalysisReport {
            symbols,
            diagnostics,
        }
    }

    fn extract_enclosing_context(
        &self,
        source: &str,
        cursor_line: usize,
    ) -> Option<EnclosingContext> {
        let line_index = LineIndex::new(source);
        let parsed = parse_unchecked_source(source, PySourceType::Python);
        let module = parsed.syntax();

        let mut module_imports = Vec::new();
        let mut matched_context: Option<EnclosingContext> = None;

        for stmt in &module.body {
            match stmt {
                Stmt::Import(_) | Stmt::ImportFrom(_) => {
                    let start = stmt.range().start().to_usize();
                    let end = stmt.range().end().to_usize();
                    if let Some(slice) = source.get(start..end) {
                        module_imports.push(slice.trim().to_string());
                    }
                }
                Stmt::ClassDef(class_def) => {
                    let (start_byte, end_byte) = full_class_byte_range(class_def);
                    let start_line = line_index.line_of(start_byte);
                    let end_line = line_index.line_of(end_byte.saturating_sub(1));

                    if cursor_line >= start_line && cursor_line <= end_line {
                        // If the entire class is compact (<= 10 lines), return the class slice;
                        // otherwise drill down to the specific enclosing method.
                        if end_line.saturating_sub(start_line) + 1 <= 10 {
                            let enclosing_source =
                                source.get(start_byte..end_byte)?.trim_end().to_string();
                            matched_context = Some(EnclosingContext {
                                symbol_name: class_def.name.to_string(),
                                kind: SymbolKind::Class,
                                start_line,
                                end_line,
                                module_imports: Vec::new(),
                                enclosing_source,
                            });
                        } else {
                            for child in &class_def.body {
                                if let Stmt::FunctionDef(func_def) = child {
                                    let (f_start, f_end) = full_function_byte_range(func_def);
                                    let f_start_line = line_index.line_of(f_start);
                                    let f_end_line = line_index.line_of(f_end.saturating_sub(1));
                                    if cursor_line >= f_start_line && cursor_line <= f_end_line {
                                        let enclosing_source =
                                            source.get(f_start..f_end)?.trim_end().to_string();
                                        matched_context = Some(EnclosingContext {
                                            symbol_name: format!(
                                                "{}.{}",
                                                class_def.name, func_def.name
                                            ),
                                            kind: if func_def.is_async {
                                                SymbolKind::AsyncFunction
                                            } else {
                                                SymbolKind::Method
                                            },
                                            start_line: f_start_line,
                                            end_line: f_end_line,
                                            module_imports: Vec::new(),
                                            enclosing_source,
                                        });
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                Stmt::FunctionDef(func_def) => {
                    let (start_byte, end_byte) = full_function_byte_range(func_def);
                    let start_line = line_index.line_of(start_byte);
                    let end_line = line_index.line_of(end_byte.saturating_sub(1));

                    if cursor_line >= start_line && cursor_line <= end_line {
                        let enclosing_source =
                            source.get(start_byte..end_byte)?.trim_end().to_string();
                        matched_context = Some(EnclosingContext {
                            symbol_name: func_def.name.to_string(),
                            kind: if func_def.is_async {
                                SymbolKind::AsyncFunction
                            } else {
                                SymbolKind::Function
                            },
                            start_line,
                            end_line,
                            module_imports: Vec::new(),
                            enclosing_source,
                        });
                    }
                }
                _ => {}
            }
        }

        drop(parsed);

        matched_context.map(|mut ctx| {
            ctx.module_imports = module_imports;
            ctx
        })
    }
}

fn extract_outline_symbol(
    stmt: &Stmt,
    source: &str,
    line_index: &LineIndex,
    in_class: bool,
) -> Option<OutlineSymbol> {
    match stmt {
        Stmt::ClassDef(class_def) => {
            let (start_byte, end_byte) = full_class_byte_range(class_def);
            let start_line = line_index.line_of(start_byte);
            let end_line = line_index.line_of(end_byte.saturating_sub(1));

            let decorators = class_def
                .decorator_list
                .iter()
                .filter_map(|d| {
                    let r = d.expression.range();
                    source
                        .get(r.start().to_usize()..r.end().to_usize())
                        .map(|s| s.trim().to_string())
                })
                .collect();

            let children = class_def
                .body
                .iter()
                .filter_map(|child| extract_outline_symbol(child, source, line_index, true))
                .collect();

            Some(OutlineSymbol {
                name: class_def.name.to_string(),
                kind: SymbolKind::Class,
                start_line,
                end_line,
                detail: None,
                decorators,
                children,
            })
        }
        Stmt::FunctionDef(func_def) => {
            let (start_byte, end_byte) = full_function_byte_range(func_def);
            let start_line = line_index.line_of(start_byte);
            let end_line = line_index.line_of(end_byte.saturating_sub(1));

            let decorators = func_def
                .decorator_list
                .iter()
                .filter_map(|d| {
                    let r = d.expression.range();
                    source
                        .get(r.start().to_usize()..r.end().to_usize())
                        .map(|s| s.trim().to_string())
                })
                .collect();

            let detail = func_def.returns.as_ref().and_then(|ret| {
                let r = ret.range();
                source
                    .get(r.start().to_usize()..r.end().to_usize())
                    .map(|s| format!("-> {}", s.trim()))
            });

            let kind = if func_def.is_async {
                SymbolKind::AsyncFunction
            } else if in_class {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            };

            Some(OutlineSymbol {
                name: func_def.name.to_string(),
                kind,
                start_line,
                end_line,
                detail,
                decorators,
                children: Vec::new(),
            })
        }
        _ => None,
    }
}

/// Recursively checks AST statements for lint violations such as Ruff rule E722 (bare `except:`).
fn collect_lint_diagnostics(stmt: &Stmt, line_index: &LineIndex, out: &mut Vec<Diagnostic>) {
    match stmt {
        Stmt::FunctionDef(f) => {
            for s in &f.body {
                collect_lint_diagnostics(s, line_index, out);
            }
        }
        Stmt::ClassDef(c) => {
            for s in &c.body {
                collect_lint_diagnostics(s, line_index, out);
            }
        }
        Stmt::Try(try_stmt) => {
            for s in &try_stmt.body {
                collect_lint_diagnostics(s, line_index, out);
            }
            for handler in &try_stmt.handlers {
                let ExceptHandler::ExceptHandler(h) = handler;
                if h.type_.is_none() {
                    let start = h.range.start().to_usize();
                    let end = h.range.end().to_usize();
                    let (line, col) = line_index.line_col(start);
                    out.push(Diagnostic {
                        byte_range: start..end,
                        line,
                        col,
                        severity: DiagnosticSeverity::Warning,
                        code: "E722".to_string(),
                        message: "Do not use bare `except`".to_string(),
                    });
                }
                for s in &h.body {
                    collect_lint_diagnostics(s, line_index, out);
                }
            }
            for s in &try_stmt.orelse {
                collect_lint_diagnostics(s, line_index, out);
            }
            for s in &try_stmt.finalbody {
                collect_lint_diagnostics(s, line_index, out);
            }
        }
        Stmt::If(if_stmt) => {
            for s in &if_stmt.body {
                collect_lint_diagnostics(s, line_index, out);
            }
            for clause in &if_stmt.elif_else_clauses {
                for s in &clause.body {
                    collect_lint_diagnostics(s, line_index, out);
                }
            }
        }
        Stmt::For(for_stmt) => {
            for s in &for_stmt.body {
                collect_lint_diagnostics(s, line_index, out);
            }
        }
        Stmt::While(while_stmt) => {
            for s in &while_stmt.body {
                collect_lint_diagnostics(s, line_index, out);
            }
        }
        Stmt::With(with_stmt) => {
            for s in &with_stmt.body {
                collect_lint_diagnostics(s, line_index, out);
            }
        }
        _ => {}
    }
}

fn full_class_byte_range(class_def: &StmtClassDef) -> (usize, usize) {
    let start = class_def
        .decorator_list
        .first()
        .map(|d| d.range().start().to_usize())
        .unwrap_or_else(|| class_def.range().start().to_usize());
    let end = class_def.range().end().to_usize();
    (start, end)
}

fn full_function_byte_range(func_def: &StmtFunctionDef) -> (usize, usize) {
    let start = func_def
        .decorator_list
        .first()
        .map(|d| d.range().start().to_usize())
        .unwrap_or_else(|| func_def.range().start().to_usize());
    let end = func_def.range().end().to_usize();
    (start, end)
}

/// Fast O(log N) byte-offset-to-line lookup table.
struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    fn new(source: &str) -> Self {
        let mut line_starts = Vec::with_capacity(source.len() / 40 + 1);
        line_starts.push(0);
        for (idx, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(idx + 1);
            }
        }
        Self { line_starts }
    }

    #[inline]
    fn line_of(&self, byte_offset: usize) -> usize {
        match self.line_starts.binary_search(&byte_offset) {
            Ok(line) => line,
            Err(next_line) => next_line.saturating_sub(1),
        }
    }

    #[inline]
    fn line_col(&self, byte_offset: usize) -> (usize, usize) {
        let line = self.line_of(byte_offset);
        let col = byte_offset.saturating_sub(self.line_starts[line]);
        (line, col)
    }
}

// ============================================================================
// 3. BINARY .PYD / CYTHON .PYX STUB INTERCEPTOR
// ============================================================================

/// Resolves compiled Python extension binaries (`.pyd` / `.so`) to `.pyi` stubs or `.pyx` Cython sources.
#[derive(Default)]
pub struct PydStubResolver;

impl PydStubResolver {
    pub fn new() -> Self {
        Self
    }
}

impl BinaryStubInterceptor for PydStubResolver {
    fn resolve_binary_module(
        &self,
        binary_path: &Path,
        sibling_files: &[PathBuf],
    ) -> StubResolution {
        // Extract PEP 3149 base module name before ABI tags (e.g. `fast_matrix.cp312-win_amd64.pyd` -> `fast_matrix`)
        let file_name = binary_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("extension");
        let module_stem = file_name.split('.').next().unwrap_or(file_name);

        let matches_stem = |candidate: &Path, expected_ext: &str| -> bool {
            let Some(ext) = candidate.extension().and_then(|e| e.to_str()) else {
                return false;
            };
            if !ext.eq_ignore_ascii_case(expected_ext) {
                return false;
            }
            candidate
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s == module_stem)
                .unwrap_or(false)
        };

        // Priority 1: Adjacent `.pyi` typing stub file
        if let Some(pyi_path) = sibling_files.iter().find(|p| matches_stem(p, "pyi")) {
            return StubResolution::StubFile(pyi_path.clone());
        }

        // Priority 2: Native implementation source in workspace (`.pyx` Cython, `.rs` PyO3, `.cpp`, `.c`)
        let native_candidates = [
            ("pyx", LanguageId::Cython),
            ("pxd", LanguageId::Cython),
            ("rs", LanguageId::Rust),
            ("cpp", LanguageId::Cpp),
            ("cc", LanguageId::Cpp),
            ("c", LanguageId::C),
        ];

        for (ext, language) in native_candidates {
            if let Some(src_path) = sibling_files.iter().find(|p| matches_stem(p, ext)) {
                return StubResolution::NativeSource {
                    path: src_path.clone(),
                    language,
                };
            }
        }

        // Priority 3: Synthesize a clean read-only `.pyi` buffer
        let synthesized = format!(
            "# Synthesized read-only Python stub for compiled binary module `{module_stem}`\n\
             # Binary path: {}\n\n\
             from typing import Any\n\n\
             def __getattr__(name: str) -> Any: ...\n",
            binary_path.display()
        );

        StubResolution::SynthesizedStub(synthesized)
    }
}

// ============================================================================
// 4. IN-MEMORY RUFF CODE FORMATTER
// ============================================================================

/// In-memory Python formatter powered by Astral's `ruff_python_formatter`.
#[derive(Default)]
pub struct PythonFormatter;

impl PythonFormatter {
    pub fn new() -> Self {
        Self
    }
}

impl CodeFormatter for PythonFormatter {
    fn language(&self) -> LanguageId {
        LanguageId::Python
    }

    fn format_source(&self, source: &str) -> Result<String, FormatError> {
        // 1. Fast syntax-error gate via ruff_python_parser so incomplete code is never mangled
        let parsed = parse_unchecked_source(source, PySourceType::Python);
        if let Some(first_err) = parsed.errors().first() {
            return Err(FormatError::SyntaxError(first_err.error.to_string()));
        }
        drop(parsed);

        // 2. Format AST & CST comments in memory using Ruff's canonical Python options
        let options = PyFormatOptions::default();
        match format_module_source(source, options) {
            Ok(printed) => Ok(printed.into_code()),
            Err(err) => Err(FormatError::SyntaxError(err.to_string())),
        }
    }
}

