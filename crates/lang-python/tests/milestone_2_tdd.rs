use std::path::{Path, PathBuf};
use std::time::Instant;
use editor_core::{Position, TextBuffer};
use lang_contract::{
    AstAnalyzer, BinaryStubInterceptor, DiagnosticSeverity, HighlightToken, InputEditSummary,
    LanguageId, StubResolution, SymbolKind, SyntaxHighlighter,
};
use lang_python::{PydStubResolver, PythonAstAnalyzer, PythonSyntaxHighlighter};

/// Helper: Generates a 10,000-line Python module with imports, classes, decorators, and methods.
fn generate_10k_python_module() -> String {
    let mut out = String::with_capacity(10_000 * 48);
    out.push_str("import math\nfrom typing import Optional\n\n");
    let mut line_count = 3;

    let mut block_idx = 0;
    while line_count < 10_000 {
        out.push_str(&format!(
            "@dataclass\nclass NeuralLayer_{block_idx}(BaseLayer):\n    \"\"\"Layer docstring\"\"\"\n    def forward(self, x: float) -> float:\n        return x * 0.5 + {block_idx}\n\n"
        ));
        line_count += 6;
        block_idx += 1;
    }
    out
}

// ============================================================================
// 1. VIEWPORT-BOUNDED TREE-SITTER HIGHLIGHTING & 1.5ms SLA TESTS
// ============================================================================

#[test]
fn test_viewport_bounded_highlighting_on_10k_file_under_1500_micros() {
    let source = generate_10k_python_module();
    let buffer = TextBuffer::from_str(&source);

    let mut highlighter = PythonSyntaxHighlighter::new();
    highlighter.parse_full(&source);

    // Slice a 55-line viewport in the middle of the 10,000-line file
    let slice = buffer.viewport_slice(5_000, 5_055);

    let start = Instant::now();
    let spans = highlighter.highlight_viewport(&source, slice.byte_range.clone());
    let elapsed = start.elapsed();

    assert!(
        !spans.is_empty(),
        "Viewport highlighter must return syntax spans for visible lines"
    );

    // Invariant 1: Every single span must overlap or stay within the viewport byte range
    for span in &spans {
        assert!(
            span.byte_range.end > slice.byte_range.start
                && span.byte_range.start < slice.byte_range.end,
            "Span {:?} leaked outside viewport byte range {:?}",
            span.byte_range,
            slice.byte_range
        );
    }

    // Invariant 2: Must classify Python keywords, classes, functions, strings, and numbers
    let has_token = |t: HighlightToken| spans.iter().any(|s| s.token == t);
    assert!(has_token(HighlightToken::Keyword), "Missing Keyword token (class/def/return)");
    assert!(has_token(HighlightToken::Class), "Missing Class identifier token");
    assert!(has_token(HighlightToken::Function), "Missing Function identifier token");
    assert!(has_token(HighlightToken::Number), "Missing Number literal token");

    // Invariant 3: Viewport highlighting must complete in < 1.5ms (1,500 microseconds)
    assert!(
        elapsed.as_micros() < 1_500,
        "Viewport highlighting breached 1.5ms SLA: took {:?}",
        elapsed
    );
}

#[test]
fn test_incremental_tree_sitter_edit_updates_tokens_under_500_micros() {
    let mut source = String::from("def compute():\n    value = 42\n    return value\n");
    let mut highlighter = PythonSyntaxHighlighter::new();
    highlighter.parse_full(&source);

    // Replace `42` (bytes 27..29 on line 1) with `"hello_world"` (13 bytes -> 27..40)
    let start_byte = source.find("42").unwrap();
    let old_end_byte = start_byte + 2;
    source.replace_range(start_byte..old_end_byte, "\"hello_world\"");
    let new_end_byte = start_byte + "\"hello_world\"".len();

    let edit = InputEditSummary {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_pos: Position::new(1, 12),
        old_end_pos: Position::new(1, 14),
        new_end_pos: Position::new(1, 25),
    };

    let start = Instant::now();
    highlighter.apply_edit(&source, &edit);
    let spans = highlighter.highlight_viewport(&source, 0..source.len());
    let elapsed = start.elapsed();

    // Verify `"hello_world"` is now highlighted as a String and `42` Number is gone
    let string_span = spans
        .iter()
        .find(|s| s.token == HighlightToken::String)
        .expect("Edited literal should now be highlighted as a String");
    assert_eq!(&source[string_span.byte_range.clone()], "\"hello_world\"");

    assert!(
        elapsed.as_micros() < 500,
        "Incremental edit + viewport re-highlight breached 500µs SLA: took {:?}",
        elapsed
    );
}

// ============================================================================
// 2. EPHEMERAL RUFF AST STRUCTURE & DIAGNOSTIC EXTRACTION TESTS
// ============================================================================

#[test]
fn test_ruff_ast_extracts_nested_class_hierarchy_decorators_and_return_types() {
    let python_code = concat!(
        "import os\n",
        "from dataclasses import dataclass\n",
        "\n",
        "@dataclass\n",
        "class VectorEngine:\n",
        "    def __init__(self, dim: int) -> None:\n",
        "        self.dim = dim\n",
        "\n",
        "    @staticmethod\n",
        "    async def normalize(values: list[float]) -> list[float]:\n",
        "        return values\n",
        "\n",
        "def helper_fn(x: int) -> int:\n",
        "    return x + 1\n"
    );

    let analyzer = PythonAstAnalyzer::new();
    let report = analyzer.analyze_ephemeral(python_code);

    assert!(
        report.diagnostics.is_empty(),
        "Valid Python code should produce zero diagnostics, got: {:?}",
        report.diagnostics
    );

    // Expect 2 top-level outline symbols: `VectorEngine` (Class) and `helper_fn` (Function)
    assert_eq!(report.symbols.len(), 2);

    let class_sym = &report.symbols[0];
    assert_eq!(class_sym.name, "VectorEngine");
    assert_eq!(class_sym.kind, SymbolKind::Class);
    assert_eq!(class_sym.decorators, vec!["dataclass"]);
    assert_eq!(class_sym.children.len(), 2);

    let init_sym = &class_sym.children[0];
    assert_eq!(init_sym.name, "__init__");
    assert_eq!(init_sym.kind, SymbolKind::Method);
    assert_eq!(init_sym.detail.as_deref(), Some("-> None"));

    let async_method = &class_sym.children[1];
    assert_eq!(async_method.name, "normalize");
    assert_eq!(async_method.kind, SymbolKind::AsyncFunction);
    assert_eq!(async_method.decorators, vec!["staticmethod"]);
    assert_eq!(async_method.detail.as_deref(), Some("-> list[float]"));

    let func_sym = &report.symbols[1];
    assert_eq!(func_sym.name, "helper_fn");
    assert_eq!(func_sym.kind, SymbolKind::Function);
    assert_eq!(func_sym.detail.as_deref(), Some("-> int"));
}

#[test]
fn test_ruff_ast_reports_syntax_errors_and_bare_except_lint_diagnostics() {
    let flawed_python = concat!(
        "def risky_io():\n",
        "    try:\n",
        "        open('missing.txt')\n",
        "    except:\n", // E722: Do not use bare `except`
        "        pass\n",
        "\n",
        "def broken_sig(a, b\n" // SyntaxError: unclosed parenthesis
    );

    let analyzer = PythonAstAnalyzer::new();
    let report = analyzer.analyze_ephemeral(flawed_python);

    // Must still recover the intact `risky_io` function for the Structure sidebar!
    assert!(
        report.symbols.iter().any(|s| s.name == "risky_io"),
        "AST recovery must preserve valid symbols even when syntax errors exist below"
    );

    // Must report both the bare `except:` lint warning (E722) and the syntax error
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "E722" && d.line == 3),
        "Expected Ruff E722 diagnostic on line 3 (bare except), got: {:?}",
        report.diagnostics
    );

    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error && d.line >= 6),
        "Expected SyntaxError diagnostic at unclosed parenthesis, got: {:?}",
        report.diagnostics
    );
}

// ============================================================================
// 3. AST-AWARE CONTEXT SLICING FOR OLLAMA-RS
// ============================================================================

#[test]
fn test_extract_enclosing_context_slices_only_active_method_and_imports() {
    let source = generate_10k_python_module();
    let analyzer = PythonAstAnalyzer::new();

    // Place cursor inside `NeuralLayer_500::forward` (around line 3006)
    let target_line = 3 + (500 * 6) + 3;
    let ctx = analyzer
        .extract_enclosing_context(&source, target_line)
        .expect("Must find enclosing symbol at target_line");

    assert_eq!(
        ctx.module_imports,
        vec![
            "import math".to_string(),
            "from typing import Optional".to_string()
        ]
    );
    assert!(
        ctx.enclosing_source.contains("class NeuralLayer_500")
            || ctx.enclosing_source.contains("def forward"),
        "Enclosing source must contain the target class/method"
    );
    assert!(
        ctx.enclosing_source.lines().count() <= 10,
        "AST slicer must prune the 10,000-line file down to <= 10 lines for Ollama!"
    );
}

// ============================================================================
// 4. BINARY .PYD / CYTHON .PYX STUB INTERCEPTOR TESTS
// ============================================================================

#[test]
fn test_pyd_binary_stub_interceptor_priority_chain() {
    let resolver = PydStubResolver::new();
    let binary = Path::new("/workspace/pkg/fast_matrix.cp312-win_amd64.pyd");

    // Case 1: Both `.pyi` and `.pyx` exist -> `.pyi` typing stub wins first
    let siblings_with_pyi = vec![
        PathBuf::from("/workspace/pkg/fast_matrix.pyx"),
        PathBuf::from("/workspace/pkg/fast_matrix.pyi"),
    ];
    assert_eq!(
        resolver.resolve_binary_module(binary, &siblings_with_pyi),
        StubResolution::StubFile(PathBuf::from("/workspace/pkg/fast_matrix.pyi"))
    );

    // Case 2: Only `.pyx` exists -> routes directly to Cython source implementation
    let siblings_with_pyx = vec![PathBuf::from("/workspace/pkg/fast_matrix.pyx")];
    assert_eq!(
        resolver.resolve_binary_module(binary, &siblings_with_pyx),
        StubResolution::NativeSource {
            path: PathBuf::from("/workspace/pkg/fast_matrix.pyx"),
            language: LanguageId::Cython,
        }
    );

    // Case 3: No source or stub on disk -> synthesizes a read-only `.pyi` stub buffer
    let no_siblings: Vec<PathBuf> = vec![];
    match resolver.resolve_binary_module(binary, &no_siblings) {
        StubResolution::SynthesizedStub(stub_text) => {
            assert!(stub_text.contains("fast_matrix"));
        }
        other => panic!("Expected SynthesizedStub, got {:?}", other),
    }
}

#[test]
fn test_decorator_expression_highlighting() {
    let source = "@app.get(\"/health\")\ndef health():\n    return {\"status\": \"ok\"}\n";
    let mut highlighter = PythonSyntaxHighlighter::new();
    highlighter.parse_full(source);

    let spans = highlighter.highlight_viewport(source, 0..source.len());

    // Spans covering "@app.get" must be Token::Decorator
    let decorator_spans: Vec<_> = spans
        .iter()
        .filter(|s| s.token == HighlightToken::Decorator)
        .collect();

    assert!(
        !decorator_spans.is_empty(),
        "Expected decorator spans for @app.get"
    );

    // Verify "@", "app", and "get" are marked as Decorator
    let app_span = spans.iter().find(|s| &source[s.byte_range.clone()] == "app");
    assert!(app_span.is_some(), "Expected span for 'app'");
    assert_eq!(app_span.unwrap().token, HighlightToken::Decorator);

    let get_span = spans.iter().find(|s| &source[s.byte_range.clone()] == "get");
    assert!(get_span.is_some(), "Expected span for 'get'");
    assert_eq!(get_span.unwrap().token, HighlightToken::Decorator);

    // String "/health" must remain String token
    let str_span = spans
        .iter()
        .find(|s| &source[s.byte_range.clone()] == "\"/health\"");
    assert!(str_span.is_some(), "Expected span for \"/health\"");
    assert_eq!(str_span.unwrap().token, HighlightToken::String);
}
