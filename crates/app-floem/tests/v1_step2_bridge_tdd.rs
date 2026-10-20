use std::time::Duration;
use ai_engine::{AiEngine, AiPromptRequest, AiStreamEvent, AiTaskKind, LlmStreamingClient};
use app_floem::{EditorCommand, EditorViewModel};
use editor_core::Position;
use floem::reactive::SignalGet;
use tokio::sync::{mpsc, watch};
use workspace_engine::{PytestCommandSpec, PytestExecutor, PytestRunner, PytestStreamEvent};

struct SlowMockAi;

impl LlmStreamingClient for SlowMockAi {
    fn stream_completion(
        &self,
        _request: AiPromptRequest,
        chunk_tx: mpsc::Sender<Result<String, String>>,
        mut cancel_rx: watch::Receiver<bool>,
    ) {
        tokio::spawn(async move {
            for i in 0..20 {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(25)) => {
                        if chunk_tx.send(Ok(format!("tok_{i} "))).await.is_err() {
                            break;
                        }
                    }
                    _ = cancel_rx.changed() => {
                        if *cancel_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });
    }
}

struct SlowMockPytest;

impl PytestExecutor for SlowMockPytest {
    fn spawn_pytest(
        &self,
        _spec: PytestCommandSpec,
        line_tx: mpsc::Sender<Result<String, String>>,
        mut cancel_rx: watch::Receiver<bool>,
    ) {
        tokio::spawn(async move {
            for i in 0..20 {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(25)) => {
                        if line_tx.send(Ok(format!("pytest_line_{i}"))).await.is_err() {
                            break;
                        }
                    }
                    _ = cancel_rx.changed() => {
                        if *cancel_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });
    }
}

#[tokio::test]
async fn test_cancel_commands_abort_live_tokio_tasks_when_rx_is_moved_to_ui_bridge() {
    let mut vm = EditorViewModel::new(20.0, 400.0);
    vm.dispatch(EditorCommand::LoadSource(
        "def compute(x: int) -> int:\n    return x + 1\n".to_string(),
    ));
    vm.dispatch(EditorCommand::SetCursor(Position::new(1, 4)));

    // 1. Start AI stream and move `rx` out of the handle (simulating `create_ext_action` pump)
    let ai_engine = AiEngine::new(SlowMockAi);
    let ai_handle = vm
        .start_ai_task_with_engine(AiTaskKind::GenerateDocstring, &ai_engine)
        .expect("Should start AI stream");
    let mut ai_rx = ai_handle.rx;

    // Dispatch CancelAiStream through the ViewModel (without calling `ai_handle.cancel()` directly)
    vm.dispatch(EditorCommand::CancelAiStream);
    assert!(!vm.signals.ai_is_streaming.get());

    let mut saw_ai_cancelled = false;
    while let Some(ev) = ai_rx.recv().await {
        if matches!(ev, AiStreamEvent::Cancelled) {
            saw_ai_cancelled = true;
            break;
        }
    }
    assert!(
        saw_ai_cancelled,
        "EditorCommand::CancelAiStream must signal the background Tokio task via stored cancel_tx"
    );

    // 2. Start Pytest run and move `rx` out of the handle
    let pytest_runner = PytestRunner::new(SlowMockPytest);
    let pt_handle = vm
        .start_pytest_with_runner(None, &pytest_runner)
        .expect("Should start pytest run");
    let mut pt_rx = pt_handle.rx;

    vm.dispatch(EditorCommand::CancelPytestRun);
    assert!(!vm.signals.pytest_is_running.get());

    let mut saw_pytest_cancelled = false;
    while let Some(ev) = pt_rx.recv().await {
        if matches!(ev, PytestStreamEvent::Cancelled) {
            saw_pytest_cancelled = true;
            break;
        }
    }
    assert!(
        saw_pytest_cancelled,
        "EditorCommand::CancelPytestRun must abort the background pytest task via stored cancel_tx"
    );
}
