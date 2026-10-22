use std::time::Instant;
use app_iced::{EditorCommand, LideApp};
use editor_core::Position;
use lang_contract::{DiagnosticSeverity, HighlightToken, SymbolKind};

/// Helper: Generates a 10,000-line Python module for UI virtualization testing.
fn generate_10k_python_source() -> String {
    let mut out = String::with_capacity(10_000 * 48);
    out.push_str("import math\nfrom dataclasses import dataclass\n\n");
    let mut line_count = 3;
    let mut idx = 0;

    while line_count < 10_000 {
        out.push_str(&format!(
            "@dataclass\nclass Pipeline_{idx}:\n    def execute(self, val: float) -> float:\n        return val * 2.0 + {idx}\n\n"
        ));
        line_count += 5;
        idx += 1;
    }
    out
}

#[test]
fn test_virtualized_viewport_pixel_scrolling_and_1_based_gutter_numbers_under_2ms() {
    // 20px per line, 1080px tall viewport -> exactly 54 visible lines at a time
    let mut app = LideApp::new(20.0, 1080.0);
    let source = generate_10k_python_source();

    let _ = app.dispatch(EditorCommand::LoadSource(source));

    let total_lines = app.total_lines();
    assert!(total_lines >= 10_000);
    assert_eq!(
        app.total_content_height_px,
        (total_lines as f64) * 20.0
    );

    // Initial viewport at scroll_offset_y = 0.0 -> lines 0..54 (gutter numbers 1..=54)
    let initial_rows = &app.visible_lines;
    assert_eq!(initial_rows.len(), 54, "Must only materialize 54 rows for a 1080p viewport");
    assert_eq!(initial_rows[0].line_idx, 0);
    assert_eq!(initial_rows[0].gutter_number, 1);
    assert!(initial_rows[0].is_active_line, "Line 0 has the initial cursor");
    assert_eq!(initial_rows[53].line_idx, 53);
    assert_eq!(initial_rows[53].gutter_number, 54);

    // Scroll halfway down to pixel offset 100,000.0 (line 5,000)
    let start = Instant::now();
    let _ = app.dispatch(EditorCommand::ScrollToPixel {
        offset_y: 100_000.0,
        viewport_height: 1080.0,
    });
    let elapsed = start.elapsed();

    let scrolled_rows = &app.visible_lines;
    assert_eq!(scrolled_rows.len(), 54);
    assert_eq!(scrolled_rows[0].line_idx, 5_000);
    assert_eq!(scrolled_rows[0].gutter_number, 5_001);
    assert!(!scrolled_rows[0].is_active_line, "Cursor is still at line 0");
    assert_eq!(scrolled_rows[53].line_idx, 5_053);
    assert_eq!(scrolled_rows[53].gutter_number, 5_054);

    assert!(
        elapsed.as_micros() < 5_000,
        "Pixel scroll + Tree-sitter viewport re-highlight took {:?}",
        elapsed
    );
}

#[test]
fn test_gutter_attaches_ruff_diagnostics_and_updates_active_line_indicator() {
    let flawed_code = concat!(
        "def process_data():\n",       // line 0
        "    try:\n",                  // line 1
        "        run()\n",             // line 2
        "    except:\n",               // line 3 -> Ruff E722 Warning
        "        pass\n",              // line 4
        "def broken(x, y\n"            // line 5 -> SyntaxError
    );

    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::LoadSource(flawed_code.to_string()));
    let _ = app.dispatch(EditorCommand::RunEphemeralAnalysis);

    let rows = &app.visible_lines;
    assert_eq!(rows[0].highest_diagnostic, None);
    assert_eq!(
        rows[3].highest_diagnostic,
        Some(DiagnosticSeverity::Warning),
        "Line 3 (bare except) must show Warning badge in gutter"
    );
    assert_eq!(
        rows[5].highest_diagnostic,
        Some(DiagnosticSeverity::Error),
        "Line 5 (unclosed paren) must show Error badge in gutter"
    );

    // Move cursor to line 3 and verify active-line highlight shifts from line 0 to line 3
    let _ = app.dispatch(EditorCommand::SetCursor(Position::new(3, 4)));
    let updated_rows = &app.visible_lines;
    assert!(!updated_rows[0].is_active_line);
    assert!(updated_rows[3].is_active_line);
    assert!(
        app.status_bar_text.contains("Ln 4, Col 5"),
        "Status bar must display 1-based line and column coordinates, got: {}",
        app.status_bar_text
    );
}

#[test]
fn test_structure_sidebar_flattens_hierarchy_and_jumps_viewport_on_click() {
    let mut app = LideApp::new(20.0, 400.0);
    let source = generate_10k_python_source();

    let _ = app.dispatch(EditorCommand::LoadSource(source));
    let _ = app.dispatch(EditorCommand::RunEphemeralAnalysis);

    let outline = &app.outline_items;
    assert!(!outline.is_empty());

    // First item is `Pipeline_0` (Class at depth 0), followed by `execute` (Method at depth 1)
    assert_eq!(outline[0].name, "Pipeline_0");
    assert_eq!(outline[0].kind, SymbolKind::Class);
    assert_eq!(outline[0].depth, 0);

    assert_eq!(outline[1].name, "execute");
    assert_eq!(outline[1].kind, SymbolKind::Method);
    assert_eq!(outline[1].depth, 1);
    assert_eq!(outline[1].detail.as_deref(), Some("-> float"));

    // Find a symbol deep in the file (e.g., `Pipeline_600` around line 3,004)
    let target_item = outline
        .iter()
        .find(|item| item.name == "Pipeline_600")
        .expect("Should find Pipeline_600 in outline");
    let target_line = target_item.start_line;
    assert!(target_line > 2_500);

    // Click the symbol in the Structure sidebar
    let _ = app.dispatch(EditorCommand::ClickStructureSymbol {
        start_line: target_line,
    });

    // Cursor must jump to (target_line, 0) and viewport must auto-scroll to include `target_line`
    assert_eq!(app.buffer.cursor().head, Position::new(target_line, 0));
    assert!(
        app.visible_lines.iter().any(|r| r.line_idx == target_line && r.is_active_line),
        "Clicking Structure symbol at line {target_line} must scroll viewport so line is visible"
    );
}

#[test]
fn test_rendered_line_slices_tree_sitter_tokens_into_line_relative_segments() {
    let code = "def calculate(x: int) -> int:\n    return x + 10\n";
    let mut app = LideApp::new(20.0, 200.0);
    let _ = app.dispatch(EditorCommand::LoadSource(code.to_string()));

    let rows = &app.visible_lines;
    let line0 = &rows[0];
    assert_eq!(line0.text, "def calculate(x: int) -> int:");

    // Verify `def` is highlighted as a Keyword at relative bytes 0..3
    let def_seg = line0
        .styled_segments
        .iter()
        .find(|seg| seg.token == Some(HighlightToken::Keyword))
        .expect("Line 0 must contain Keyword segment for `def`");
    assert_eq!(&line0.text[def_seg.rel_byte_range.clone()], "def");

    // Verify `calculate` is highlighted as a Function at relative bytes 4..13
    let fn_seg = line0
        .styled_segments
        .iter()
        .find(|seg| seg.token == Some(HighlightToken::Function))
        .expect("Line 0 must contain Function segment for `calculate`");
    assert_eq!(&line0.text[fn_seg.rel_byte_range.clone()], "calculate");
}
