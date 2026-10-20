use std::cell::Cell;
use std::fs;
use std::rc::Rc;

use app_floem::{EditorCommand, EditorViewModel};
use editor_core::Position;
use floem::reactive::{create_effect, SignalGet};
use tempfile::TempDir;
use workspace_engine::{parse_pytest_output, PytestStreamEvent};

/// Helper: Builds a temporary workspace with a 120-line test file where line 95 (1-based)
/// has a failing pytest assertion.
fn build_pytest_workspace() -> TempDir {
    let temp = TempDir::new().expect("Failed to create temp workspace");
    let root = temp.path();

    fs::create_dir_all(root.join(".git")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();

    fs::write(
        root.join("src/calculator.py"),
        "def multiply(a: int, b: int) -> int:\n    return a * b\n",
    )
    .unwrap();

    // Build `tests/test_calculator.py` with `test_overflow_error` failing at 1-based line 95 (0-based line 94)
    let mut test_file = String::new();
    for i in 1..=90 {
        test_file.push_str(&format!("# padding comment line {i}\n"));
    }
    test_file.push_str("def test_basic_multiply():\n");           // line 91 (1-based)
    test_file.push_str("    assert 2 * 3 == 6\n");                // line 92
    test_file.push_str("\n");                                     // line 93
    test_file.push_str("def test_overflow_error():\n");           // line 94
    test_file.push_str("    assert multiply(10, 10) == 999\n");   // line 95 (FAILING LINE)

    fs::write(root.join("tests/test_calculator.py"), test_file).unwrap();

    temp
}

#[test]
fn test_toggle_pytest_panel_and_streaming_logs_do_not_redraw_code_viewport() {
    let temp = build_pytest_workspace();
    let mut vm = EditorViewModel::new(20.0, 400.0);
    vm.dispatch(EditorCommand::OpenWorkspace(temp.path().to_path_buf()));

    let pytest_renders = Rc::new(Cell::new(0usize));
    let viewport_renders = Rc::new(Cell::new(0usize));

    let pt_sig = vm.signals.pytest_output_lines;
    let pt_count = pytest_renders.clone();
    create_effect(move |_| {
        let _ = pt_sig.get();
        pt_count.set(pt_count.get() + 1);
    });

    let vp_sig = vm.signals.visible_lines;
    let vp_count = viewport_renders.clone();
    create_effect(move |_| {
        let _ = vp_sig.get();
        vp_count.set(vp_count.get() + 1);
    });

    assert_eq!(pytest_renders.get(), 1);
    assert_eq!(viewport_renders.get(), 1);

    // Trigger pytest run and stream 2 output lines
    vm.dispatch(EditorCommand::TriggerPytestRun { target_file: None });
    assert!(vm.signals.pytest_panel_open.get());
    assert!(vm.signals.pytest_is_running.get());

    vm.dispatch(EditorCommand::ReceivePytestEvent(
        PytestStreamEvent::OutputLine("collecting ... collected 2 items".to_string()),
    ));
    vm.dispatch(EditorCommand::ReceivePytestEvent(
        PytestStreamEvent::OutputLine("tests/test_calculator.py .F".to_string()),
    ));

    assert_eq!(vm.signals.pytest_output_lines.get().len(), 2);

    // Streaming pytest output lines must NEVER force the code editor viewport to re-render!
    assert_eq!(
        viewport_renders.get(),
        1,
        "Streaming pytest logs must not redraw the code viewport"
    );
}

#[test]
fn test_clicking_pytest_failure_opens_test_file_moves_cursor_and_scrolls_to_failing_line() {
    let temp = build_pytest_workspace();
    // 20px line height, 400px viewport -> 20 visible lines per screen
    let mut vm = EditorViewModel::new(20.0, 400.0);
    vm.dispatch(EditorCommand::OpenWorkspace(temp.path().to_path_buf()));

    // Start with `src/calculator.py` open in the editor
    let calc_py = temp.path().join("src/calculator.py");
    vm.dispatch(EditorCommand::OpenWorkspaceFile(calc_py.clone()));
    assert_eq!(vm.signals.active_file_path.get(), Some(calc_py));

    // Simulate a completed pytest run where `tests/test_calculator.py` failed at 1-based line 95
    let raw_pytest_output = concat!(
        ".F\n",
        "=================================== FAILURES ===================================\n",
        "____________________________ test_overflow_error _____________________________\n",
        "tests/test_calculator.py:95: in test_overflow_error\n",
        "    assert multiply(10, 10) == 999\n",
        "E   AssertionError: assert 100 == 999\n",
        "1 failed, 1 passed in 0.02s\n"
    );
    let report = parse_pytest_output(temp.path(), raw_pytest_output);

    vm.dispatch(EditorCommand::TriggerPytestRun { target_file: None });
    vm.dispatch(EditorCommand::ReceivePytestEvent(
        PytestStreamEvent::Completed(report),
    ));

    assert!(!vm.signals.pytest_is_running.get());
    assert_eq!(
        vm.signals.pytest_summary.get().as_deref(),
        Some("1 failed, 1 passed")
    );

    let failures = vm.signals.pytest_failures.get();
    assert_eq!(failures.len(), 1);
    let failure = failures[0].clone();
    assert_eq!(failure.line_number, 95);

    // Click the failing test in the bottom Pytest Runner panel!
    vm.dispatch(EditorCommand::ClickPytestFailure(failure));

    // 1. Editor must automatically open `tests/test_calculator.py`
    let expected_test_file = temp.path().join("tests/test_calculator.py");
    assert_eq!(vm.signals.active_file_path.get(), Some(expected_test_file));

    // 2. Cursor must jump to 0-based line 94 (`95 - 1`), col 0
    assert_eq!(vm.signals.cursor.get().head, Position::new(94, 0));

    // 3. Virtualized viewport must scroll so line_idx 94 (`assert multiply(10, 10) == 999`) is visible & active!
    let visible = vm.signals.visible_lines.get();
    let active_row = visible
        .iter()
        .find(|r| r.line_idx == 94 && r.is_active_line)
        .expect("Failing line 94 must be visible inside the scrolled viewport");
    assert_eq!(active_row.gutter_number, 95);
    assert_eq!(active_row.text, "    assert multiply(10, 10) == 999");

    // 4. Structure sidebar must automatically show `test_basic_multiply` and `test_overflow_error`
    let outline = vm.signals.outline_items.get();
    assert!(outline.iter().any(|s| s.name == "test_overflow_error"));
}
