use std::cell::Cell;
use std::rc::Rc;

use ai_engine::{AiEngine, AiPromptRequest, AiStreamEvent, AiTaskKind, LlmStreamingClient};
use app_floem::{EditorCommand, EditorViewModel};
use editor_core::Position;
use floem::reactive::{create_effect, SignalGet};
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

// ============================================================================
// 1. RIGHT-HAND AI PANE TOGGLE & REACTIVE SIGNAL ISOLATION
// ============================================================================

#[test]
fn test_toggle_ai_pane_signal_isolation_does_not_redraw_editor_viewport() {
    let mut vm = EditorViewModel::new(20.0, 400.0);
    vm.dispatch(EditorCommand::LoadSource(SAMPLE_MODULE.to_string()));

    let ai_pane_renders = Rc::new(Cell::new(0usize));
    let viewport_renders = Rc::new(Cell::new(0usize));

    let pane_sig = vm.signals.ai_pane_open;
    let pane_count = ai_pane_renders.clone();
    create_effect(move |_| {
        let _ = pane_sig.get();
        pane_count.set(pane_count.get() + 1);
    });

    let vp_sig = vm.signals.visible_lines;
    let vp_count = viewport_renders.clone();
    create_effect(move |_| {
        let _ = vp_sig.get();
        vp_count.set(vp_count.get() + 1);
    });

    assert!(!vm.signals.ai_pane_open.get(), "AI pane should start closed");
    assert_eq!(ai_pane_renders.get(), 1);
    assert_eq!(viewport_renders.get(), 1);

    // Toggle AI pane open and closed
    vm.dispatch(EditorCommand::ToggleAiPane);
    assert!(vm.signals.ai_pane_open.get());
    assert_eq!(ai_pane_renders.get(), 2);

    vm.dispatch(EditorCommand::ToggleAiPane);
    assert!(!vm.signals.ai_pane_open.get());
    assert_eq!(ai_pane_renders.get(), 3);

    // Code viewport must NOT have re-rendered when toggling the right-hand AI pane!
    assert_eq!(
        viewport_renders.get(),
        1,
        "Toggling right-hand AI split pane must never redraw the code viewport"
    );
}

// ============================================================================
// 2. TRIGGERING AI TASK SLICES ENCLOSING AST NODE & OPENS PANE
// ============================================================================

#[test]
fn test_trigger_ai_task_slices_enclosing_ast_function_and_initializes_stream_state() {
    let mut vm = EditorViewModel::new(20.0, 400.0);
    vm.dispatch(EditorCommand::LoadSource(SAMPLE_MODULE.to_string()));

    // Place cursor inside `compute_discount` on line 3
    vm.dispatch(EditorCommand::SetCursor(Position::new(3, 4)));

    // Trigger AI docstring generation
    vm.dispatch(EditorCommand::TriggerAiTask(AiTaskKind::GenerateDocstring));

    assert!(
        vm.signals.ai_pane_open.get(),
        "Triggering an AI task must automatically open the right-hand AI pane"
    );
    assert!(vm.signals.ai_is_streaming.get());

    let ctx = vm
        .signals
        .active_ai_context
        .get()
        .expect("Must capture EnclosingContext for compute_discount");
    assert_eq!(ctx.symbol_name, "compute_discount");
    assert_eq!(ctx.start_line, 2);
    assert_eq!(ctx.end_line, 3);
    assert_eq!(ctx.module_imports, vec!["import math".to_string()]);
}

// ============================================================================
// 3. END-TO-END MOCK STREAMING & ONE-CLICK APPLY REFACTOR WITH UNDO/REDO
// ============================================================================

#[tokio::test]
async fn test_end_to_end_ai_stream_and_one_click_apply_refactor_with_single_step_undo() {
    let mut vm = EditorViewModel::new(20.0, 400.0);
    vm.dispatch(EditorCommand::LoadSource(SAMPLE_MODULE.to_string()));
    vm.dispatch(EditorCommand::RunEphemeralAnalysis);

    // Place cursor inside `compute_discount` (line 2..=3)
    vm.dispatch(EditorCommand::SetCursor(Position::new(3, 8)));

    let mock_engine = AiEngine::new(MockAiBackend {
        chunks: vec![
            "```python\n".to_string(),
            "def compute_discount(price: float, rate: float = 0.9) -> float:\n".to_string(),
            "    \"\"\"Apply configurable discount rate.\"\"\"\n".to_string(),
            "    return round(price * rate, 2)\n".to_string(),
            "```".to_string(),
        ],
    });

    let mut handle = vm
        .start_ai_task_with_engine(
            AiTaskKind::Refactor {
                instruction: "Add rate parameter and docstring".to_string(),
            },
            &mock_engine,
        )
        .expect("Should build AST prompt and start stream");

    // Pump streaming events from `ai-engine` into `app-floem`'s reactive dispatcher
    while let Some(ev) = handle.rx.recv().await {
        let is_done = matches!(ev, AiStreamEvent::Completed { .. });
        vm.dispatch(EditorCommand::ReceiveAiStreamEvent(ev));
        if is_done {
            break;
        }
    }

    assert!(!vm.signals.ai_is_streaming.get());
    assert!(
        vm.signals.ai_stream_text.get().contains("```python"),
        "Raw stream display should contain streamed markdown text"
    );
    assert!(
        vm.signals.ai_clean_code.get().is_some(),
        "Completed stream must populate sanitized ai_clean_code for one-click apply"
    );

    // Click "Apply Refactor" button!
    vm.dispatch(EditorCommand::ApplyAiSuggestion);

    // 1. Verify lines 2..=3 were replaced by the 3-line refactored function,
    //    while `import math` (line 0) and `def main()` (now shifted to line 6) remain intact!
    let rows = vm.signals.visible_lines.get();
    assert_eq!(rows[0].text, "import math");
    assert_eq!(
        rows[2].text,
        "def compute_discount(price: float, rate: float = 0.9) -> float:"
    );
    assert_eq!(rows[3].text, "    \"\"\"Apply configurable discount rate.\"\"\"");
    assert_eq!(rows[4].text, "    return round(price * rate, 2)");
    assert_eq!(rows[6].text, "def main() -> float:");

    // 2. Verify a single Ctrl+Z (`EditorCommand::Undo`) reverts the entire AI refactor!
    vm.dispatch(EditorCommand::Undo);
    let undone_rows = vm.signals.visible_lines.get();
    assert_eq!(undone_rows[2].text, "def compute_discount(price: float) -> float:");
    assert_eq!(undone_rows[3].text, "    return price * 0.9");
    assert_eq!(undone_rows[5].text, "def main() -> float:");
}

// ============================================================================
// 4. MID-STREAM CANCELLATION VIA EDITOR COMMAND
// ============================================================================

#[test]
fn test_cancel_ai_stream_command_resets_streaming_state_cleanly() {
    let mut vm = EditorViewModel::new(20.0, 400.0);
    vm.dispatch(EditorCommand::LoadSource(SAMPLE_MODULE.to_string()));
    vm.dispatch(EditorCommand::SetCursor(Position::new(2, 4)));

    vm.dispatch(EditorCommand::TriggerAiTask(AiTaskKind::InlineCompletion));
    vm.dispatch(EditorCommand::ReceiveAiStreamEvent(AiStreamEvent::Token(
        "def compute_discount".to_string(),
    )));
    assert!(vm.signals.ai_is_streaming.get());

    // User clicks Cancel or presses Escape
    vm.dispatch(EditorCommand::CancelAiStream);
    assert!(!vm.signals.ai_is_streaming.get());
}
