use std::fs;
use app_iced::{EditorCommand, LideApp};
use editor_core::Position;
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
    pipeline_code.push_str("def target_kernel(tensor_scale: float) -> float:\n");
    pipeline_code.push_str("    return 42.0\n");

    fs::write(root.join("src/pipeline.py"), pipeline_code).unwrap();

    temp
}

#[test]
fn test_toggle_search_panel_and_query_updates() {
    let temp = build_searchable_workspace();
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::OpenWorkspace(temp.path().to_path_buf()));

    assert!(!app.search_panel_open);
    let prev_lines = app.visible_lines.clone();

    // Toggle Find-in-Files panel open and type a query
    let _ = app.dispatch(EditorCommand::ToggleSearchPanel);
    assert!(app.search_panel_open);

    let _ = app.dispatch(EditorCommand::SetSearchQuery("target_kernel".to_string()));
    assert_eq!(app.search_query, "target_kernel");

    // Viewport lines must remain identical
    assert_eq!(app.visible_lines, prev_lines);
}

#[test]
fn test_execute_workspace_search_populates_matches_and_handles_invalid_regex_gracefully() {
    let temp = build_searchable_workspace();
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::OpenWorkspace(temp.path().to_path_buf()));

    // 1. Run an invalid regex (unclosed character class)
    let _ = app.dispatch(EditorCommand::SetSearchQuery(r"def broken([".to_string()));
    let _ = app.dispatch(EditorCommand::ExecuteWorkspaceSearch { max_results: 50 });

    assert!(
        app.search_error.is_some(),
        "Invalid regex must populate search_error"
    );
    assert!(app.search_results.is_empty());

    // 2. Fix regex and search for `def (verify_token|target_kernel)`
    let _ = app.dispatch(EditorCommand::SetSearchQuery(
        r"def (verify_token|target_kernel)".to_string(),
    ));
    let _ = app.dispatch(EditorCommand::ExecuteWorkspaceSearch { max_results: 50 });

    assert_eq!(
        app.search_error, None,
        "Valid regex search must clear search_error"
    );

    assert_eq!(
        app.search_results.len(),
        2,
        "Should find matches in both auth.py and pipeline.py"
    );
}

#[test]
fn test_clicking_search_result_opens_target_file_sets_exact_cursor_and_scrolls_viewport() {
    let temp = build_searchable_workspace();
    // 20px line height, 400px viewport -> 20 visible lines at a time
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::OpenWorkspace(temp.path().to_path_buf()));

    // Initially open `src/auth.py`
    let auth_path = temp.path().join("src/auth.py");
    let _ = app.dispatch(EditorCommand::OpenWorkspaceFile(auth_path.clone()));
    assert_eq!(app.active_file_path, Some(auth_path));

    // Search for `target_kernel` (located in `src/pipeline.py` at 1-based line 121, 0-based col 4)
    let _ = app.dispatch(EditorCommand::SetSearchQuery(r"target_kernel".to_string()));
    let _ = app.dispatch(EditorCommand::ExecuteWorkspaceSearch { max_results: 50 });

    assert_eq!(app.search_results.len(), 1);
    let hit = app.search_results[0].clone();

    assert_eq!(hit.line_number, 121);
    assert_eq!(hit.column, 4);

    // Click the search result!
    let _ = app.dispatch(EditorCommand::ClickSearchResult(hit));

    // 1. Active file must automatically switch to `src/pipeline.py`
    let expected_pipeline = temp.path().join("src/pipeline.py");
    assert_eq!(app.active_file_path, Some(expected_pipeline));

    // 2. Cursor must sit at 0-based line 120 (`121 - 1`), column 4
    assert_eq!(app.buffer.cursor().head, Position::new(120, 4));

    // 3. Status bar must reflect 1-based `Ln 121, Col 5`
    let status = &app.status_bar_text;
    assert!(
        status.contains("Ln 121, Col 5"),
        "Status bar should display Ln 121, Col 5 after jump, got: {status}"
    );

    // 4. Virtualized 20-line viewport must have scrolled so 0-based line 120 is visible and active!
    assert!(
        app.visible_lines.iter().any(|row| row.line_idx == 120 && row.is_active_line),
        "Clicking search result at line 121 must scroll virtualized viewport to include line_idx 120"
    );

    // 5. Structure sidebar must automatically reflect `src/pipeline.py`'s `target_kernel` function
    assert!(
        app.outline_items.iter().any(|item| item.name == "target_kernel"),
        "Cross-file search jump must refresh Ruff AST Structure outline for newly opened file"
    );
}
