use ai_engine::{AiEngine, AiPromptRequest, AiStreamEvent, AiTaskKind, LlmStreamingClient};
use app_iced::{EditorCommand, LideApp};
use editor_core::Position;
use tokio::sync::{mpsc, watch};

const SAMPLE_MODULE: &str = concat!(
    "import math\n",                                    // line 0
    "\n",                                               // line 1
    "def compute_discount(price: float) -> float:\n",   // line 2
    "    return price * 0.9\n",                         // line 3
    "\n",                                               // line 4
    "def main() -> float:\n",                           // line 5
    "    return compute_discount(100.0)\n"              // line 6
);

/// Deterministic mock streaming client for headless UI testing.
struct MockAiBackend {
    chunks: Vec<String>,
}

impl LlmStreamingClient for MockAiBackend {
    fn stream_completion(
        &self,
        _request: AiPromptRequest,
        chunk_tx: mpsc::Sender<Result<String, String>>,
        cancel_rx: watch::Receiver<bool>,
    ) {
        let chunks = self.chunks.clone();
        tokio::spawn(async move {
            for chunk in chunks {
                if *cancel_rx.borrow() {
                    break;
                }
                if chunk_tx.send(Ok(chunk)).await.is_err() {
                    break;
                }
            }
        });
    }
}

#[test]
fn test_toggle_ai_pane() {
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::LoadSource(SAMPLE_MODULE.to_string()));

    assert!(!app.ai_pane_open, "AI pane should start closed");

    let _ = app.dispatch(EditorCommand::ToggleAiPane);
    assert!(app.ai_pane_open);

    let _ = app.dispatch(EditorCommand::ToggleAiPane);
    assert!(!app.ai_pane_open);
}

#[test]
fn test_trigger_ai_task_slices_enclosing_ast_function_and_initializes_stream_state() {
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::LoadSource(SAMPLE_MODULE.to_string()));

    // Place cursor inside `compute_discount` on line 3
    let _ = app.dispatch(EditorCommand::SetCursor(Position::new(3, 4)));

    // Trigger AI docstring generation
    let _ = app.dispatch(EditorCommand::TriggerAiTask(AiTaskKind::GenerateDocstring));

    assert!(
        app.ai_pane_open,
        "Triggering an AI task must automatically open the right-hand AI pane"
    );
    assert!(app.ai_is_streaming);

    let ctx = app
        .active_ai_context
        .as_ref()
        .expect("Must capture EnclosingContext for compute_discount");
    assert_eq!(ctx.symbol_name, "compute_discount");
    assert_eq!(ctx.start_line, 2);
    assert_eq!(ctx.end_line, 3);
    assert_eq!(ctx.module_imports, vec!["import math".to_string()]);
}

#[tokio::test]
async fn test_end_to_end_ai_stream_and_one_click_apply_refactor_with_single_step_undo() {
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::LoadSource(SAMPLE_MODULE.to_string()));
    let _ = app.dispatch(EditorCommand::RunEphemeralAnalysis);

    // Place cursor inside `compute_discount` (line 2..=3)
    let _ = app.dispatch(EditorCommand::SetCursor(Position::new(3, 8)));

    let mock_engine = AiEngine::new(MockAiBackend {
        chunks: vec![
            "```python\n".to_string(),
            "def compute_discount(price: float, rate: float = 0.9) -> float:\n".to_string(),
            "    \"\"\"Apply configurable discount rate.\"\"\"\n".to_string(),
            "    return round(price * rate, 2)\n".to_string(),
            "```".to_string(),
        ],
    });

    let mut handle = app
        .start_ai_task_with_engine(
            AiTaskKind::Refactor {
                instruction: "Add rate parameter and docstring".to_string(),
            },
            &mock_engine,
        )
        .expect("Should build AST prompt and start stream");

    // Pump streaming events from `ai-engine` into `app-iced`'s dispatcher
    while let Some(ev) = handle.rx.recv().await {
        let is_done = matches!(ev, AiStreamEvent::Completed { .. });
        let _ = app.dispatch(EditorCommand::ReceiveAiStreamEvent(ev));
        if is_done {
            break;
        }
    }

    assert!(!app.ai_is_streaming);
    assert!(
        app.ai_stream_text.contains("```python"),
        "Raw stream display should contain streamed markdown text"
    );
    assert!(
        app.ai_clean_code.is_some(),
        "Completed stream must populate sanitized ai_clean_code for one-click apply"
    );

    // Click "Apply Refactor" button!
    let _ = app.dispatch(EditorCommand::ApplyAiSuggestion);

    // 1. Verify lines 2..=3 were replaced by the 3-line refactored function,
    //    while `import math` (line 0) and `def main()` (shifted to line 6) remain intact!
    let rows = &app.visible_lines;
    assert_eq!(rows[0].text, "import math");
    assert_eq!(
        rows[2].text,
        "def compute_discount(price: float, rate: float = 0.9) -> float:"
    );
    assert_eq!(rows[3].text, "    \"\"\"Apply configurable discount rate.\"\"\"");
    assert_eq!(rows[4].text, "    return round(price * rate, 2)");
    assert_eq!(rows[6].text, "def main() -> float:");

    // 2. Verify a single Ctrl+Z (`EditorCommand::Undo`) reverts the entire AI refactor!
    let _ = app.dispatch(EditorCommand::Undo);
    let undone_rows = &app.visible_lines;
    assert_eq!(undone_rows[2].text, "def compute_discount(price: float) -> float:");
    assert_eq!(undone_rows[3].text, "    return price * 0.9");
    assert_eq!(undone_rows[5].text, "def main() -> float:");
}

#[test]
fn test_cancel_ai_stream_command_resets_streaming_state_cleanly() {
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::LoadSource(SAMPLE_MODULE.to_string()));
    let _ = app.dispatch(EditorCommand::SetCursor(Position::new(2, 4)));

    let _ = app.dispatch(EditorCommand::TriggerAiTask(AiTaskKind::InlineCompletion));
    let _ = app.dispatch(EditorCommand::ReceiveAiStreamEvent(AiStreamEvent::Token(
        "def compute_discount".to_string(),
    )));
    assert!(app.ai_is_streaming);

    // User clicks Cancel or presses Escape
    let _ = app.dispatch(EditorCommand::CancelAiStream);
    assert!(!app.ai_is_streaming);
}
