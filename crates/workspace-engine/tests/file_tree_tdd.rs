use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use lang_contract::LanguageId;
use tempfile::TempDir;
use workspace_engine::{
    build_explorer_command, build_folder_picker_command, create_file, create_folder, delete_entry,
    FlatTreeItem, WorkspaceIndexer, WorkspaceTreeNode,
};

/// Helper: Creates a mock directory tree for file manager TDD verification.
fn create_mock_tree_workspace() -> TempDir {
    let temp = TempDir::new().expect("Failed to create temp workspace");
    let root = temp.path();

    // 1. Git & gitignore
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join(".gitignore"), "target/\n*.tmp\n").unwrap();

    // 2. Docs directory
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("docs/guide.md"), "# Guide\n").unwrap();

    // 3. Nested source directories
    fs::create_dir_all(root.join("src/core")).unwrap();
    fs::write(
        root.join("src/core/fast_ops.pyx"),
        "cdef double dot(double a, double b): return a * b\n",
    )
    .unwrap();
    fs::write(
        root.join("src/main.py"),
        "def main():\n    print('hello')\n",
    )
    .unwrap();

    // 4. Root files
    fs::write(root.join("README.md"), "# Project Readme\n").unwrap();

    // 5. Artifact directories that MUST be pruned
    fs::create_dir_all(root.join(".venv/Lib")).unwrap();
    fs::write(root.join(".venv/pyvenv.cfg"), "version = 3.12.0\n").unwrap();
    fs::create_dir_all(root.join("src/__pycache__")).unwrap();
    fs::write(root.join("src/__pycache__/main.pyc"), b"fake_bytecode").unwrap();
    fs::create_dir_all(root.join("target/debug")).unwrap();
    fs::write(root.join("target/debug/app.exe"), b"binary").unwrap();

    temp
}

#[test]
fn test_workspace_tree_indexing_and_sorting() {
    let temp = create_mock_tree_workspace();
    let root = temp.path();
    let indexer = WorkspaceIndexer::new();

    let tree = indexer.index_tree(root);

    assert_eq!(tree.name, root.file_name().unwrap().to_string_lossy());
    assert!(tree.is_dir);

    // Root children: Should contain "docs", "src" (directories first, sorted), then "README.md"
    let child_names: Vec<&str> = tree.children.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        child_names,
        vec!["docs", "src", "README.md"],
        "Directories must precede files, both sorted alphabetically"
    );

    // Verify .git, .venv, __pycache__, and target are strictly pruned
    let all_names_flattened = collect_all_names(&tree);
    assert!(!all_names_flattened.contains(&".venv".to_string()));
    assert!(!all_names_flattened.contains(&"__pycache__".to_string()));
    assert!(!all_names_flattened.contains(&"target".to_string()));
    assert!(!all_names_flattened.contains(&".git".to_string()));

    // Verify language classifications
    let src_node = tree.children.iter().find(|c| c.name == "src").unwrap();
    assert!(src_node.is_dir);

    let main_py = src_node.children.iter().find(|c| c.name == "main.py").unwrap();
    assert_eq!(main_py.language, Some(LanguageId::Python));

    let core_node = src_node.children.iter().find(|c| c.name == "core").unwrap();
    let fast_pyx = core_node.children.iter().find(|c| c.name == "fast_ops.pyx").unwrap();
    assert_eq!(fast_pyx.language, Some(LanguageId::Cython));
}

#[test]
fn test_workspace_tree_flattening_with_10px_depth_increments() {
    let temp = create_mock_tree_workspace();
    let root = temp.path();
    let indexer = WorkspaceIndexer::new();
    let tree = indexer.index_tree(root);

    let mut expanded_dirs = HashSet::new();

    // 1. When all collapsed: only root children are visible at depth 0
    let flat_collapsed = tree.flatten_visible(&expanded_dirs);
    let names_depths: Vec<(&str, usize, bool)> = flat_collapsed
        .iter()
        .map(|item| (item.name.as_str(), item.depth, item.is_expanded))
        .collect();

    assert_eq!(
        names_depths,
        vec![
            ("docs", 0, false),
            ("src", 0, false),
            ("README.md", 0, false),
        ]
    );

    // 2. Expand "src": its children become visible at depth 1 (representing 10px indentation)
    let src_path = root.join("src");
    expanded_dirs.insert(src_path.clone());

    let flat_src_expanded = tree.flatten_visible(&expanded_dirs);
    let names_depths_src: Vec<(&str, usize, bool)> = flat_src_expanded
        .iter()
        .map(|item| (item.name.as_str(), item.depth, item.is_expanded))
        .collect();

    assert_eq!(
        names_depths_src,
        vec![
            ("docs", 0, false),
            ("src", 0, true),
            ("core", 1, false),     // depth 1 -> 10px indent
            ("main.py", 1, false), // depth 1 -> 10px indent
            ("README.md", 0, false),
        ]
    );

    // 3. Expand "src/core": its child fast_ops.pyx becomes visible at depth 2 (representing 20px indent)
    let core_path = src_path.join("core");
    expanded_dirs.insert(core_path);

    let flat_all_expanded = tree.flatten_visible(&expanded_dirs);
    let names_depths_all: Vec<(&str, usize, bool)> = flat_all_expanded
        .iter()
        .map(|item| (item.name.as_str(), item.depth, item.is_expanded))
        .collect();

    assert_eq!(
        names_depths_all,
        vec![
            ("docs", 0, false),
            ("src", 0, true),
            ("core", 1, true),
            ("fast_ops.pyx", 2, false), // depth 2 -> 20px indent
            ("main.py", 1, false),
            ("README.md", 0, false),
        ]
    );
}

#[test]
fn test_create_file_and_create_folder_in_workspace() {
    let temp = create_mock_tree_workspace();
    let root = temp.path();

    // 1. Create a new folder under src
    let src_dir = root.join("src");
    let new_folder = create_folder(&src_dir, "utils").expect("Folder creation should succeed");
    assert!(new_folder.is_dir());
    assert_eq!(new_folder, src_dir.join("utils"));

    // Duplicate folder creation should fail
    let dup_folder_res = create_folder(&src_dir, "utils");
    assert!(dup_folder_res.is_err());

    // 2. Create a new file under the newly created folder
    let new_file = create_file(&new_folder, "math_helper.py").expect("File creation should succeed");
    assert!(new_file.is_file());
    assert_eq!(new_file, new_folder.join("math_helper.py"));

    // Duplicate file creation should fail
    let dup_file_res = create_file(&new_folder, "math_helper.py");
    assert!(dup_file_res.is_err());

    // 3. Verify the re-indexed tree contains the new items
    let indexer = WorkspaceIndexer::new();
    let updated_tree = indexer.index_tree(root);
    let all_names = collect_all_names(&updated_tree);
    assert!(all_names.contains(&"utils".to_string()));
    assert!(all_names.contains(&"math_helper.py".to_string()));
}

#[test]
fn test_delete_file_and_folder_in_workspace() {
    let temp = create_mock_tree_workspace();
    let root = temp.path();

    let guide_file = root.join("docs/guide.md");
    assert!(guide_file.exists());

    // 1. Delete file
    delete_entry(&guide_file).expect("File deletion should succeed");
    assert!(!guide_file.exists());

    // 2. Delete folder
    let docs_dir = root.join("docs");
    assert!(docs_dir.exists());
    delete_entry(&docs_dir).expect("Folder deletion should succeed");
    assert!(!docs_dir.exists());

    // 3. Verify tree no longer contains docs
    let indexer = WorkspaceIndexer::new();
    let updated_tree = indexer.index_tree(root);
    let all_names = collect_all_names(&updated_tree);
    assert!(!all_names.contains(&"docs".to_string()));
    assert!(!all_names.contains(&"guide.md".to_string()));
}

#[test]
fn test_build_explorer_command_for_file_and_directory() {
    let temp = create_mock_tree_workspace();
    let root = temp.path();

    let main_py = root.join("src/main.py");
    let (prog, args) = build_explorer_command(&main_py);
    assert!(!prog.is_empty());
    assert!(!args.is_empty());

    let src_dir = root.join("src");
    let (dir_prog, dir_args) = build_explorer_command(&src_dir);
    assert!(!dir_prog.is_empty());
    assert!(!dir_args.is_empty());
}

#[test]
fn test_build_folder_picker_command() {
    let (prog, args) = build_folder_picker_command();
    assert!(!prog.is_empty(), "Folder picker program must not be empty");
    assert!(!args.is_empty(), "Folder picker args must not be empty");

    #[cfg(windows)]
    {
        assert_eq!(prog, "powershell");
        assert!(args.iter().any(|a| a.contains("FolderBrowserDialog")));
        assert!(args.iter().any(|a| a == "-Sta"), "Must include -Sta flag for Windows Forms STA apartment requirement");
        assert!(args.iter().any(|a| a == "Bypass"), "Must include Bypass execution policy for restricted environments");
    }
}

fn collect_all_names(node: &WorkspaceTreeNode) -> Vec<String> {
    let mut names = vec![node.name.clone()];
    for child in &node.children {
        names.extend(collect_all_names(child));
    }
    names
}
