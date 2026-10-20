use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use app_floem::{EditorCommand, EditorViewModel};
use editor_core::{MoveDirection, Position};
use floem::reactive::{create_effect, SignalGet};
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

// ============================================================================
// 1. VIRTUALIZED VIEWPORT SLICING, PIXEL MATH & GUTTER NUMBERING SLA
// ============================================================================

#[test]
fn test_virtualized_viewport_pixel_scrolling_and_1_based_gutter_numbers_under_2ms() {
    // 20px per line, 1080px tall viewport -> exactly 54 visible lines at a time
    let mut vm = EditorViewModel::new(20.0, 1080.0);
    let source = generate_10k_python_source();

    vm.dispatch(EditorCommand::LoadSource(source));

    let total_lines = vm.total_lines();
    assert!(total_lines >= 10_000);
    assert_eq!(
        vm.signals.total_content_height.get(),
        (total_lines as f64) * 20.0
    );

    // Initial viewport at scroll_offset_y = 0.0 -> lines 0..54 (gutter numbers 1..=54)
    let initial_rows = vm.signals.visible_lines.get();
    assert_eq!(initial_rows.len(), 54, "Must only materialize 54 rows for a 1080p viewport");
    assert_eq!(initial_rows[0].line_idx, 0);
    assert_eq!(initial_rows[0].gutter_number, 1);
    assert!(initial_rows[0].is_active_line, "Line 0 has the initial cursor");
    assert_eq!(initial_rows[53].line_idx, 53);
    assert_eq!(initial_rows[53].gutter_number, 54);

    // Scroll halfway down to pixel offset 100,000.0 (line 5,000)
    let start = Instant::now();
    vm.dispatch(EditorCommand::ScrollToPixel {
        offset_y: 100_000.0,
        viewport_height: 1080.0,
    });
    let elapsed = start.elapsed();

    let scrolled_rows = vm.signals.visible_lines.get();
    assert_eq!(scrolled_rows.len(), 54);
    assert_eq!(scrolled_rows[0].line_idx, 5_000);
    assert_eq!(scrolled_rows[0].gutter_number, 5_001);
    assert!(!scrolled_rows[0].is_active_line, "Cursor is still at line 0");
    assert_eq!(scrolled_rows[53].line_idx, 5_053);
    assert_eq!(scrolled_rows[53].gutter_number, 5_054);

    assert!(
        elapsed.as_micros() < 2_000,
        "Pixel scroll + Tree-sitter viewport re-highlight breached 2ms SLA: took {:?}",
        elapsed
    );
}

// ============================================================================
// 2. GRANULAR FLOEM REACTIVE SIGNAL ISOLATION (ZERO REDUNDANT REDRAWS)
// ============================================================================

#[test]
fn test_granular_floem_signals_do_not_trigger_sidebar_on_scroll_or_cursor_move() {
    let mut vm = EditorViewModel::new(20.0, 400.0);
    let source = generate_10k_python_source();
    vm.dispatch(EditorCommand::LoadSource(source));
    vm.dispatch(EditorCommand::RunEphemeralAnalysis);

    let viewport_renders = Rc::new(Cell::new(0usize));
    let cursor_renders = Rc::new(Cell::new(0usize));
    let outline_renders = Rc::new(Cell::new(0usize));

    let vp_sig = vm.signals.visible_lines;
    let vp_count = viewport_renders.clone();
    create_effect(move |_| {
        let _ = vp_sig.get();
        vp_count.set(vp_count.get() + 1);
    });

    let cur_sig = vm.signals.cursor;
    let cur_count = cursor_renders.clone();
    create_effect(move |_| {
        let _ = cur_sig.get();
        cur_count.set(cur_count.get() + 1);
    });

    let out_sig = vm.signals.outline_items;
    let out_count = outline_renders.clone();
    create_effect(move |_| {
        let _ = out_sig.get();
        out_count.set(out_count.get() + 1);
    });

    // `create_effect` runs once immediately upon subscription
    assert_eq!(viewport_renders.get(), 1);
    assert_eq!(cursor_renders.get(), 1);
    assert_eq!(outline_renders.get(), 1);

    // Action 1: Scroll the viewport -> ONLY `visible_lines` should notify!
    vm.dispatch(EditorCommand::ScrollToPixel {
        offset_y: 2_000.0,
        viewport_height: 400.0,
    });
    assert_eq!(viewport_renders.get(), 2);
    assert_eq!(cursor_renders.get(), 1, "Scrolling must not notify cursor signal");
    assert_eq!(
        outline_renders.get(),
        1,
        "Scrolling must not re-render Structure sidebar"
    );

    // Action 2: Move cursor Down -> `cursor` and `visible_lines` update, `outline_items` stays at 1
    vm.dispatch(EditorCommand::MoveCursor(MoveDirection::Down));
    assert_eq!(cursor_renders.get(), 2);
    assert_eq!(
        outline_renders.get(),
        1,
        "Moving cursor must not re-render Structure sidebar"
    );
}

// ============================================================================
// 3. GUTTER DIAGNOSTIC SEVERITY BADGES & ACTIVE LINE TRACKING
// ============================================================================

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

    let mut vm = EditorViewModel::new(20.0, 400.0);
    vm.dispatch(EditorCommand::LoadSource(flawed_code.to_string()));
    vm.dispatch(EditorCommand::RunEphemeralAnalysis);

    let rows = vm.signals.visible_lines.get();
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
    vm.dispatch(EditorCommand::SetCursor(Position::new(3, 4)));
    let updated_rows = vm.signals.visible_lines.get();
    assert!(!updated_rows[0].is_active_line);
    assert!(updated_rows[3].is_active_line);
    assert!(
        vm.signals.status_bar_text.get().contains("Ln 4, Col 5"),
        "Status bar must display 1-based line and column coordinates, got: {}",
        vm.signals.status_bar_text.get()
    );
}

// ============================================================================
// 4. PYCHARM-STYLE STRUCTURE SIDEBAR FLATTENING & CLICK-TO-JUMP
// ============================================================================

#[test]
fn test_structure_sidebar_flattens_hierarchy_and_jumps_viewport_on_click() {
    let mut vm = EditorViewModel::new(20.0, 400.0); // 20 visible lines per screen
    let source = generate_10k_python_source();

    vm.dispatch(EditorCommand::LoadSource(source));
    vm.dispatch(EditorCommand::RunEphemeralAnalysis);

    let outline = vm.signals.outline_items.get();
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
    vm.dispatch(EditorCommand::ClickStructureSymbol {
        start_line: target_line,
    });

    // Cursor must jump to (target_line, 0) and viewport must auto-scroll to include `target_line`
    assert_eq!(vm.signals.cursor.get().head, Position::new(target_line, 0));
    let visible = vm.signals.visible_lines.get();
    assert!(
        visible.iter().any(|r| r.line_idx == target_line && r.is_active_line),
        "Clicking Structure symbol at line {target_line} must scroll viewport so line is visible"
    );
}

// ============================================================================
// 5. LINE-LOCAL SYNTAX HIGHLIGHTING SEGMENTS
// ============================================================================

#[test]
fn test_rendered_line_slices_tree_sitter_tokens_into_line_relative_segments() {
    let code = "def calculate(x: int) -> int:\n    return x + 10\n";
    let mut vm = EditorViewModel::new(20.0, 200.0);
    vm.dispatch(EditorCommand::LoadSource(code.to_string()));

    let rows = vm.signals.visible_lines.get();
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
