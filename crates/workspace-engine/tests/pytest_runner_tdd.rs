use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use workspace_engine::{
    build_pytest_command_spec, parse_pytest_output, PythonEnvInfo, PytestCommandSpec,
    PytestExecutor, PytestRunner, PytestStreamEvent,
};

/// Deterministic mock executor that streams simulated pytest output lines without spawning Python.
struct MockPytestExecutor {
    lines: Vec<String>,
    #[allow(dead_code)]
    exit_code: i32,
    delay_ms: u64,
}

impl PytestExecutor for MockPytestExecutor {
    fn spawn_pytest(
        &self,
        _spec: PytestCommandSpec,
        line_tx: mpsc::Sender<Result<String, String>>,
        mut cancel_rx: watch::Receiver<bool>,
    ) {
        let lines = self.lines.clone();
        let delay = self.delay_ms;

        tokio::spawn(async move {
            for line in lines {
                if *cancel_rx.borrow() {
                    break;
                }
                if delay > 0 {
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(delay)) => {}
                        _ = cancel_rx.changed() => {
                            if *cancel_rx.borrow() {
                                break;
                            }
                        }
                    }
                }
                if line_tx.send(Ok(line)).await.is_err() {
                    break;
                }
            }
        });
    }
}

// ============================================================================
// 1. VIRTUALENV INTERPRETER COMMAND RESOLUTION
// ============================================================================

#[test]
fn test_build_pytest_command_uses_discovered_virtualenv_interpreter_and_short_tb() {
    let workspace_root = Path::new("/workspace/ml_project");
    let venv = PythonEnvInfo {
        root: PathBuf::from("/workspace/ml_project/.venv"),
        interpreter_path: PathBuf::from("/workspace/ml_project/.venv/bin/python"),
        site_packages: Some(PathBuf::from(
            "/workspace/ml_project/.venv/lib/python3.12/site-packages",
        )),
        version: Some("3.12.6".to_string()),
    };

    let target_test = Path::new("tests/test_pipeline.py");
    let spec = build_pytest_command_spec(workspace_root, Some(&venv), Some(target_test));

    assert_eq!(
        spec.program,
        PathBuf::from("/workspace/ml_project/.venv/bin/python")
    );
    assert_eq!(spec.working_dir, PathBuf::from("/workspace/ml_project"));
    assert_eq!(
        spec.args,
        vec![
            "-m".to_string(),
            "pytest".to_string(),
            "--tb=short".to_string(),
            "-q".to_string(),
            "--color=no".to_string(),
            "tests/test_pipeline.py".to_string(),
        ]
    );
}

// ============================================================================
// 2. PYTEST `--tb=short` TRACEBACK & SUMMARY PARSER
// ============================================================================

#[test]
fn test_parse_pytest_output_extracts_exact_failing_file_lines_and_summary_counts() {
    let workspace_root = Path::new("/workspace/project");
    let raw_output = concat!(
        "..F.F\n",
        "=================================== FAILURES ===================================\n",
        "___________________________ test_compute_discount ____________________________\n",
        "tests/test_pricing.py:28: in test_compute_discount\n",
        "    assert compute_discount(100.0) == 85.0\n",
        "E   AssertionError: assert 90.0 == 85.0\n",
        "____________________________ test_matrix_shape _______________________________\n",
        "tests/test_tensor.py:64: in test_matrix_shape\n",
        "    assert tensor.shape == (4, 4)\n",
        "E   ValueError: Expected 4x4 matrix, got 2x2\n",
        "=========================== short test summary info ============================\n",
        "FAILED tests/test_pricing.py::test_compute_discount - AssertionError: assert 90.0 == 85.0\n",
        "FAILED tests/test_tensor.py::test_matrix_shape - ValueError: Expected 4x4 matrix, got 2x2\n",
        "2 failed, 3 passed in 0.12s\n"
    );

    let report = parse_pytest_output(workspace_root, raw_output);

    assert_eq!(report.passed, 3);
    assert_eq!(report.failed, 2);
    assert_eq!(report.failures.len(), 2);

    let f0 = &report.failures[0];
    assert_eq!(f0.test_name, "test_compute_discount");
    assert_eq!(
        f0.relative_path,
        PathBuf::from("tests/test_pricing.py")
    );
    assert_eq!(
        f0.path,
        PathBuf::from("/workspace/project/tests/test_pricing.py")
    );
    assert_eq!(f0.line_number, 28);
    assert!(f0.message.contains("AssertionError: assert 90.0 == 85.0"));

    let f1 = &report.failures[1];
    assert_eq!(f1.test_name, "test_matrix_shape");
    assert_eq!(
        f1.relative_path,
        PathBuf::from("tests/test_tensor.py")
    );
    assert_eq!(f1.line_number, 64);
    assert!(f1.message.contains("ValueError: Expected 4x4 matrix, got 2x2"));
}

// ============================================================================
// 3. NON-BLOCKING ASYNC STREAMING & CANCELLATION
// ============================================================================

#[tokio::test]
async fn test_pytest_runner_streams_lines_in_real_time_and_emits_final_report() {
    let workspace_root = Path::new("/workspace/project");
    let spec = build_pytest_command_spec(workspace_root, None, None);

    let mock_exec = MockPytestExecutor {
        lines: vec![
            ".F".to_string(),
            "=================================== FAILURES ===================================".to_string(),
            "_____________________________ test_broken_math _______________________________".to_string(),
            "tests/test_math.py:14: in test_broken_math".to_string(),
            "    assert 1 + 1 == 3".to_string(),
            "E   AssertionError: assert 2 == 3".to_string(),
            "1 failed, 1 passed in 0.03s".to_string(),
        ],
        exit_code: 1,
        delay_ms: 0,
    };

    let runner = PytestRunner::new(mock_exec);
    let mut handle = runner.start_run(spec);

    let mut streamed_lines = Vec::new();
    let mut final_report = None;

    while let Some(event) = handle.rx.recv().await {
        match event {
            PytestStreamEvent::OutputLine(line) => streamed_lines.push(line),
            PytestStreamEvent::Completed(report) => {
                final_report = Some(report);
                break;
            }
            other => panic!("Unexpected pytest event: {:?}", other),
        }
    }

    assert_eq!(streamed_lines.len(), 7);
    let report = final_report.expect("Must receive final PytestRunReport");
    assert_eq!(report.passed, 1);
    assert_eq!(report.failed, 1);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].line_number, 14);
}

#[tokio::test]
async fn test_pytest_runner_aborts_immediately_when_cancelled() {
    let workspace_root = Path::new("/workspace/project");
    let spec = build_pytest_command_spec(workspace_root, None, None);

    let mock_exec = MockPytestExecutor {
        lines: (0..10).map(|i| format!("test_line_{i}")).collect(),
        exit_code: 0,
        delay_ms: 40,
    };

    let runner = PytestRunner::new(mock_exec);
    let mut handle = runner.start_run(spec);

    // Read the first line, then cancel the test run
    let first = handle.rx.recv().await.unwrap();
    assert!(matches!(first, PytestStreamEvent::OutputLine(_)));

    handle.cancel();

    let mut saw_cancelled = false;
    while let Some(event) = handle.rx.recv().await {
        if matches!(event, PytestStreamEvent::Cancelled) {
            saw_cancelled = true;
            break;
        }
    }
    assert!(saw_cancelled, "Cancelling pytest run must emit PytestStreamEvent::Cancelled");
}
