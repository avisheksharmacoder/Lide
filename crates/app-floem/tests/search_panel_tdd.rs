use std::cell::Cell;
use std::fs;
use std::rc::Rc;

use app_floem::{EditorCommand, EditorViewModel};
use editor_core::Position;
use floem::reactive::{create_effect, SignalGet};
use tempfile::TempDir;

/// Helper: Creates a multi-file Python workspace where a target function appears deep inside
/// a 200-line module so click-to-jump must both switch files and scroll the viewport.
fn build_searchable_workspace() -> TempDir {
    let temp = TempDir::new().expect("Failed to create temp workspace");
    let root = temp.path();

    fs::create_dir_all(root.join(".git")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();

    // File 1: `src/auth.py`
    fs::write(
        root.join("src/auth.py"),
        "def verify_token(token: str) -> bool:\n    return len(token) == 32\n",
    )
    .unwrap();

    // File 2: `src/pipeline.py` (150 lines, with `target_kernel` at 1-based line 121, 0-based line 120)
    let mut pipeline_code = String::new();
    for i in 0..120 {
        pipeline_code.push_str(&format!("x_{i} = {i}\n"));
    }
    pipeline_code.push_str("def target_kernel(tensor_ scale: float) -> float:\n");
    pipeline_code.push_str("    return 42.0\n");

    fs::write(root.join("src/pipeline.py"), pipeline_code).unwrap();

    temp
}

// ============================================================================
// 1. BOTTOM SEARCH PANEL TOGGLE & REACTIVE SIGNAL ISOLATION
// ============================================================================

#[test]
fn test_toggle_search_panel_and_query_updates_do_not_redraw_code_viewport() {
    let temp = build_searchable_workspace();
    let mut vm = EditorViewModel::new(20.0, 400.0);
    vm.dispatch(EditorCommand::OpenWorkspace(temp.path().to_path_buf()));

    let search_panel_renders = Rc::new(Cell::new(0usize));
    let viewport_renders = Rc::new(Cell::new(0usize));

    let panel_sig = vm.signals.search_panel_open;
    let panel_count = search_panel_renders.clone();
    create_effect(move |_| {
        let _ = panel_sig.get();
        panel_count.set(panel_count.get() + 1);
    });

    let vp_sig = vm.signals.visible_lines;
    let vp_count = viewport_renders.clone();
    create_effect(move |_| {
        let _ = vp_sig.get();
        vp_count.set(vp_count.get() + 1);
    });

    assert!(!vm.signals.search_panel_open.get());
    assert_eq!(search_panel_renders.get(), 1);
    assert_eq!(viewport_renders.get(), 1);

    // Toggle Find-in-Files panel open and type a query
    vm.dispatch(EditorCommand::ToggleSearchPanel);
    assert!(vm.signals.search_panel_open.get());
    assert_eq!(search_panel_renders.get(), 2);

    vm.dispatch(EditorCommand::SetSearchQuery("target_kernel".to_string()));
    assert_eq!(vm.signals.search_query.get(), "target_kernel");

    // Viewport must NOT have re-rendered!
    assert_eq!(
        viewport_renders.get(),
        1,
        "Toggling search panel or editing regex query must never redraw the editor viewport"
    );
}

// ============================================================================
// 2. RIPGREP REGEX SEARCH EXECUTION & INVALID REGEX ERROR HANDLING
// ============================================================================

#[test]
fn test_execute_workspace_search_populates_matches_and_handles_invalid_regex_gracefully() {
    let temp = build_searchable_workspace();
    let mut vm = EditorViewModel::new(20.0, 400.0);
    vm.dispatch(EditorCommand::OpenWorkspace(temp.path().to_path_buf()));

    // 1. Run an invalid regex (unclosed character class)
    vm.dispatch(EditorCommand::SetSearchQuery(r"def broken([".to_string()));
    vm.dispatch(EditorCommand::ExecuteWorkspaceSearch { max_results: 50 });

    assert!(
        vm.signals.search_error.get().is_some(),
        "Invalid regex must populate search_error signal"
    );
    assert!(vm.signals.search_results.get().is_empty());

    // 2. Fix regex and search for `def (verify_token|target_kernel)`
    vm.dispatch(EditorCommand::SetSearchQuery(
        r"def (verify_token|target_kernel)".to_string(),
    ));
    vm.dispatch(EditorCommand::ExecuteWorkspaceSearch { max_results: 50 });

    assert_eq!(
        vm.signals.search_error.get(),
        None,
        "Valid regex search must clear search_error"
    );

    let results = vm.signals.search_results.get();
    assert_eq!(results.len(), 2, "Should find matches in both auth.py and pipeline.py");
}

// ============================================================================
// 3. CLICK-TO-JUMP CROSS-FILE NAVIGATION & VIEWPORT AUTO-SCROLL
// ============================================================================

#[test]
fn test_clicking_search_result_opens_target_file_sets_exact_cursor_and_scrolls_viewport() {
    let temp = build_searchable_workspace();
    // 20px line height, 400px viewport -> 20 visible lines at a time
    let mut vm = EditorViewModel::new(20.0, 400.0);
    vm.dispatch(EditorCommand::OpenWorkspace(temp.path().to_path_buf()));

    // Initially open `src/auth.py`
    let auth_path = temp.path().join("src/auth.py");
    vm.dispatch(EditorCommand::OpenWorkspaceFile(auth_path.clone()));
    assert_eq!(vm.signals.active_file_path.get(), Some(auth_path));

    // Search for `target_kernel` (located in `src/pipeline.py` at 1-based line 121, 0-based col 4)
    vm.dispatch(EditorCommand::SetSearchQuery(r"target_kernel".to_string()));
    vm.dispatch(EditorCommand::ExecuteWorkspaceSearch { max_results: 50 });

    let results = vm.signals.search_results.get();
    assert_eq!(results.len(), 1);
    let hit = results[0].clone();

    assert_eq!(hit.line_number, 121);
    assert_eq!(hit.column, 4);

    // Click the search result!
    vm.dispatch(EditorCommand::ClickSearchResult(hit));

    // 1. Active file must automatically switch to `src/pipeline.py`
    let expected_pipeline = temp.path().join("src/pipeline.py");
    assert_eq!(vm.signals.active_file_path.get(), Some(expected_pipeline));

    // 2. Cursor must sit at 0-based line 120 (`121 - 1`), column 4
    assert_eq!(vm.signals.cursor.get().head, Position::new(120, 4));

    // 3. Status bar must reflect 1-based `Ln 121, Col 5`
    let status = vm.signals.status_bar_text.get();
    assert!(
        status.contains("Ln 121, Col 5"),
        "Status bar should display Ln 121, Col 5 after jump, got: {status}"
    );

    // 4. Virtualized 20-line viewport must have scrolled so 0-based line 120 is visible and active!
    let visible = vm.signals.visible_lines.get();
    assert!(
        visible.iter().any(|row| row.line_idx == 120 && row.is_active_line),
        "Clicking search result at line 121 must scroll virtualized viewport to include line_idx 120"
    );

    // 5. Structure sidebar must automatically reflect `src/pipeline.py`'s `target_kernel` function
    let outline = vm.signals.outline_items.get();
    assert!(
        outline.iter().any(|item| item.name == "target_kernel"),
        "Cross-file search jump must refresh Ruff AST Structure outline for the newly opened file"
    );
}
