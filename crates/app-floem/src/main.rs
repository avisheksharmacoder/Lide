use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use ai_engine::{AiEngine, AiTaskKind, OllamaStreamingClient};
use app_floem::{
    ActiveMenu, EditorCommand, EditorViewModel, FlatOutlineItem, RenderedLine, SidebarTab,
    StyledTextSegment,
};
use editor_core::{MoveDirection, Position};
use floem::event::{Event, EventListener};
use floem::ext_event::create_ext_action;
use floem::keyboard::{Key, NamedKey};
use floem::peniko::Color;
use floem::prelude::*;
use floem::reactive::Scope;
use floem::style::CursorStyle;
use lang_contract::{DiagnosticSeverity, HighlightToken, LanguageId, SymbolKind};
use tokio::runtime::{Builder as RuntimeBuilder, Handle as TokioHandle};
use tokio::sync::mpsc;
use workspace_engine::{
    ProcessPytestExecutor, PytestFailure, PytestRunner, SearchMatch, WorkspaceFile,
};

const LINE_HEIGHT_PX: f64 = 22.0;
const INITIAL_VIEWPORT_HEIGHT_PX: f64 = 792.0; // 36 visible rows

const INITIAL_PYTHON_SAMPLE: &str = r#"import math
from dataclasses import dataclass
from typing import Optional

@dataclass
class PyRustVectorEngine:
    """High-speed Python vector processor inspected by Ruff AST."""
    dimension: int
    scale_factor: float = 1.0

    def compute_norm(self, values: list[float]) -> float:
        scaled = [v * self.scale_factor for v in values if v >= 0.0]
        return math.sqrt(sum(x * x for x in scaled))

    async def stream_batches(self, batch_size: int) -> Optional[int]:
        try:
            total = batch_size * self.dimension
            return total
        except:
            return None

def entry_point(seed: int) -> float:
    engine = PyRustVectorEngine(dimension=128, scale_factor=0.5)
    return engine.compute_norm([float(seed), 3.14159, 2.71828])
"#;


// ============================================================================
// NON-BLOCKING TOKIO -> FLOEM UI STREAM BRIDGE (`create_ext_action`)
// ============================================================================

/// Recursively awaits items from a Tokio `mpsc::Receiver<T>` on the background Tokio threadpool
/// and marshals each item onto Floem's main UI thread using `create_ext_action`.
fn pump_tokio_stream<T: Send + 'static>(
    tokio_handle: TokioHandle,
    scope: Scope,
    mut rx: mpsc::Receiver<T>,
    on_event: Rc<dyn Fn(T)>,
) {
    let handle_clone = tokio_handle.clone();
    let on_event_clone = on_event.clone();

    let send_to_ui = create_ext_action(
        scope,
        move |maybe_next: Option<(T, mpsc::Receiver<T>)>| {
            if let Some((item, remaining_rx)) = maybe_next {
                on_event_clone(item);
                pump_tokio_stream(handle_clone, scope, remaining_rx, on_event_clone);
            }
        },
    );

    tokio_handle.spawn(async move {
        if let Some(item) = rx.recv().await {
            send_to_ui(Some((item, rx)));
        } else {
            send_to_ui(None);
        }
    });
}

/// Starts a live AST-sliced AI task on `OllamaStreamingClient` and streams tokens into Floem signals.
fn start_live_ai_task(
    vm_handle: &Rc<RefCell<EditorViewModel>>,
    ai_engine: &Arc<AiEngine<OllamaStreamingClient>>,
    tokio_handle: &TokioHandle,
    task: AiTaskKind,
) {
    let (scope, maybe_handle) = {
        let mut vm = vm_handle.borrow_mut();
        let s = vm.scope();
        let h = vm.start_ai_task_with_engine(task, ai_engine);
        (s, h)
    };

    if let Some(stream_handle) = maybe_handle {
        let vm_for_cb = vm_handle.clone();
        pump_tokio_stream(
            tokio_handle.clone(),
            scope,
            stream_handle.rx,
            Rc::new(move |ev| {
                vm_for_cb
                    .borrow_mut()
                    .dispatch(EditorCommand::ReceiveAiStreamEvent(ev));
            }),
        );
    }
}

/// Starts a live virtualenv-aware `pytest` run on `ProcessPytestExecutor` and streams output to Floem.
fn start_live_pytest_run(
    vm_handle: &Rc<RefCell<EditorViewModel>>,
    pytest_runner: &Arc<PytestRunner<ProcessPytestExecutor>>,
    tokio_handle: &TokioHandle,
    target_file: Option<PathBuf>,
) {
    let (scope, maybe_handle) = {
        let mut vm = vm_handle.borrow_mut();
        let s = vm.scope();
        let h = vm.start_pytest_with_runner(target_file, pytest_runner);
        (s, h)
    };

    if let Some(run_handle) = maybe_handle {
        let vm_for_cb = vm_handle.clone();
        pump_tokio_stream(
            tokio_handle.clone(),
            scope,
            run_handle.rx,
            Rc::new(move |ev| {
                vm_for_cb
                    .borrow_mut()
                    .dispatch(EditorCommand::ReceivePytestEvent(ev));
            }),
        );
    }
}

// ============================================================================
// VIEW RENDERING HELPERS
// ============================================================================

fn token_color(token: Option<HighlightToken>) -> Color {
    match token {
        Some(HighlightToken::Keyword) => Color::rgb8(198, 120, 221),
        Some(HighlightToken::Function) | Some(HighlightToken::Method) => Color::rgb8(97, 175, 239),
        Some(HighlightToken::Class) | Some(HighlightToken::Type) => Color::rgb8(229, 192, 123),
        Some(HighlightToken::Decorator) => Color::rgb8(86, 182, 194),
        Some(HighlightToken::String) => Color::rgb8(152, 195, 121),
        Some(HighlightToken::Number) | Some(HighlightToken::Boolean) => Color::rgb8(209, 154, 102),
        Some(HighlightToken::Comment) => Color::rgb8(92, 99, 112),
        Some(HighlightToken::Operator) => Color::rgb8(171, 178, 191),
        Some(HighlightToken::Parameter) => Color::rgb8(224, 108, 117),
        Some(HighlightToken::Builtin) => Color::rgb8(86, 182, 194),
        None => Color::rgb8(212, 218, 226),
    }
}

fn dropdown_menu_item(
    title: &'static str,
    shortcut: &'static str,
    on_click: impl Fn() + 'static,
) -> impl IntoView {
    h_stack((
        label(move || title.to_string()).style(|s| {
            s.font_size(12.5)
                .color(Color::rgb8(220, 224, 232))
        }),
        empty().style(|s| s.flex_grow(1.0)),
        label(move || shortcut.to_string()).style(|s| {
            s.font_size(11.0)
                .color(Color::rgb8(130, 138, 150))
                .font_family("Consolas".to_string())
        }),
    ))
    .on_click_stop(move |_| on_click())
    .style(|s| {
        s.width_full()
            .height(28.0)
            .padding_horiz(12.0)
            .items_center()
            .cursor(CursorStyle::Pointer)
            .border_radius(4.0)
            .hover(|s| s.background(Color::rgb8(45, 50, 64)))
    })
}

fn dropdown_divider() -> impl IntoView {
    empty().style(|s| {
        s.width_full()
            .height(1.0)
            .margin_vert(4.0)
            .background(Color::rgb8(40, 44, 56))
    })
}

fn font_choice_chip(
    display_name: &'static str,
    font_family_value: &'static str,
    current_signal: RwSignal<String>,
    on_select: impl Fn() + 'static,
) -> impl IntoView {
    let font_val = font_family_value.to_string();
    h_stack((
        label(move || display_name.to_string()).style(move |s| {
            let is_sel = current_signal.get() == display_name;
            s.font_size(12.0)
                .font_family(font_val.clone())
                .color(if is_sel {
                    Color::WHITE
                } else {
                    Color::rgb8(190, 196, 208)
                })
        }),
        label(move || {
            if current_signal.get() == display_name {
                "  ✓"
            } else {
                ""
            }
        })
        .style(move |s| {
            s.font_size(11.0)
                .font_bold()
                .color(Color::rgb8(152, 195, 121))
        }),
    ))
    .on_click_stop(move |_| on_select())
    .style(move |s| {
        let is_sel = current_signal.get() == display_name;
        s.padding_horiz(12.0)
            .padding_vert(6.0)
            .margin_right(8.0)
            .margin_bottom(6.0)
            .border_radius(5.0)
            .cursor(CursorStyle::Pointer)
            .border(1.0)
            .border_color(if is_sel {
                Color::rgb8(97, 175, 239)
            } else {
                Color::rgb8(45, 50, 65)
            })
            .background(if is_sel {
                Color::rgb8(36, 44, 60)
            } else {
                Color::rgb8(28, 31, 40)
            })
            .hover(move |s| {
                if current_signal.get() != display_name {
                    s.background(Color::rgb8(42, 48, 64))
                } else {
                    s
                }
            })
    })
}

fn render_code_segment(
    line_text: &str,
    seg: &StyledTextSegment,
    editor_font: RwSignal<String>,
) -> impl IntoView {
    let slice = line_text
        .get(seg.rel_byte_range.clone())
        .unwrap_or("")
        .to_string();
    let fg = token_color(seg.token);

    label(move || slice.clone()).style(move |s| {
        s.color(fg)
            .font_size(13.5)
            .font_family(editor_font.get())
    })
}

fn render_editor_row(
    row: RenderedLine,
    vm_handle: Rc<RefCell<EditorViewModel>>,
    editor_font: RwSignal<String>,
) -> impl IntoView {
    let line_idx = row.line_idx;
    let gutter_num = row.gutter_number;
    let is_active = row.is_active_line;
    let diag = row.highest_diagnostic;
    let text_clone = row.text.clone();
    let segments = row.styled_segments.clone();

    let (badge_str, badge_color) = match diag {
        Some(DiagnosticSeverity::Error) => ("●", Color::rgb8(224, 108, 117)),
        Some(DiagnosticSeverity::Warning) => ("▲", Color::rgb8(229, 192, 123)),
        Some(DiagnosticSeverity::Information) | Some(DiagnosticSeverity::Hint) => {
            ("•", Color::rgb8(97, 175, 239))
        }
        None => (" ", Color::TRANSPARENT),
    };

    let row_bg = if is_active {
        Color::rgb8(33, 37, 48)
    } else {
        Color::rgb8(26, 28, 35)
    };

    let gutter_fg = if is_active {
        Color::rgb8(240, 244, 250)
    } else {
        Color::rgb8(92, 99, 112)
    };

    let is_empty = text_clone.is_empty();

    h_stack((
        label(|| "▎".to_string()).style(move |s| {
            s.width(4.0)
                .font_size(12.0)
                .color(Color::rgb8(97, 175, 239))
                .apply_if(!is_active, |s| s.hide())
        }),
        h_stack((
            label(move || badge_str.to_string())
                .style(move |s| s.width(14.0).font_size(11.0).color(badge_color)),
            label(move || format!("{gutter_num:>5}")).style(move |s| {
                s.width(46.0)
                    .font_size(12.5)
                    .color(gutter_fg)
                    .font_family(editor_font.get())
            }),
        ))
        .style(|s| {
            s.padding_left(6.0)
                .padding_right(12.0)
                .items_center()
        }),
        dyn_stack(
            move || segments.clone().into_iter().enumerate(),
            move |(idx, seg)| (*idx, seg.rel_byte_range.start, seg.rel_byte_range.end),
            move |(_, seg)| render_code_segment(&text_clone, &seg, editor_font),
        )
        .style(|s| s.padding_left(10.0).items_center()),
        label(|| "▏".to_string()).style(move |s| {
            s.width(6.0)
                .font_size(13.5)
                .color(Color::rgb8(97, 175, 239))
                .font_family(editor_font.get())
                .apply_if(!is_active || !is_empty, |s| s.hide())
        }),
    ))
    .on_click_stop(move |_| {
        let mut vm = vm_handle.borrow_mut();
        vm.dispatch(EditorCommand::CloseMenu);
        vm.dispatch(EditorCommand::SetCursor(Position::new(line_idx, 0)));
    })
    .style(move |s| {
        s.width_full()
            .height(LINE_HEIGHT_PX)
            .items_center()
            .background(row_bg)
    })
}

fn render_workspace_file_item(
    file: WorkspaceFile,
    vm_handle: Rc<RefCell<EditorViewModel>>,
) -> impl IntoView {
    let (badge, badge_color) = match file.language {
        Some(LanguageId::Python) => ("PY", Color::rgb8(97, 175, 239)),
        Some(LanguageId::Cython) => ("CY", Color::rgb8(229, 192, 123)),
        Some(LanguageId::PydBinaryStub) => ("PYD", Color::rgb8(198, 120, 221)),
        Some(LanguageId::Rust) => ("RS", Color::rgb8(209, 154, 102)),
        Some(LanguageId::C) | Some(LanguageId::Cpp) => ("C++", Color::rgb8(86, 182, 194)),
        None => ("··", Color::rgb8(110, 118, 132)),
    };

    let rel_display = file.relative_path.to_string_lossy().to_string();
    let full_path = file.path.clone();

    h_stack((
        label(move || badge.to_string()).style(move |s| {
            s.width(28.0)
                .font_size(10.5)
                .font_bold()
                .color(badge_color)
        }),
        label(move || rel_display.clone())
            .style(|s| s.font_size(12.5).color(Color::rgb8(212, 218, 226))),
    ))
    .on_click_stop(move |_| {
        let mut vm = vm_handle.borrow_mut();
        vm.dispatch(EditorCommand::CloseMenu);
        vm.dispatch(EditorCommand::OpenWorkspaceFile(full_path.clone()));
    })
    .style(|s| {
        s.width_full()
            .height(26.0)
            .padding_horiz(10.0)
            .items_center()
            .cursor(CursorStyle::Pointer)
            .hover(|s| s.background(Color::rgb8(34, 38, 48)))
    })
}

fn render_outline_item(
    item: FlatOutlineItem,
    vm_handle: Rc<RefCell<EditorViewModel>>,
) -> impl IntoView {
    let icon = match item.kind {
        SymbolKind::Class => "C",
        SymbolKind::Method => "m",
        SymbolKind::AsyncFunction => "⚡",
        SymbolKind::Function => "ƒ",
        _ => "•",
    };
    let icon_color = match item.kind {
        SymbolKind::Class => Color::rgb8(229, 192, 123),
        SymbolKind::Method | SymbolKind::Function => Color::rgb8(97, 175, 239),
        SymbolKind::AsyncFunction => Color::rgb8(198, 120, 221),
        _ => Color::rgb8(171, 178, 191),
    };

    let indent_px = (item.depth as f64) * 16.0 + 10.0;
    let target_line = item.start_line;
    let display_name = item.name.clone();
    let detail_text = item.detail.clone().unwrap_or_default();

    h_stack((
        label(move || icon.to_string()).style(move |s| {
            s.width(18.0)
                .font_size(12.0)
                .font_bold()
                .color(icon_color)
        }),
        label(move || display_name.clone())
            .style(|s| s.font_size(12.5).color(Color::rgb8(212, 218, 226))),
        label(move || format!(" {}", detail_text))
            .style(|s| s.font_size(11.5).color(Color::rgb8(110, 118, 132))),
    ))
    .on_click_stop(move |_| {
        let mut vm = vm_handle.borrow_mut();
        vm.dispatch(EditorCommand::CloseMenu);
        vm.dispatch(EditorCommand::ClickStructureSymbol {
            start_line: target_line,
        });
    })
    .style(move |s| {
        s.width_full()
            .height(26.0)
            .padding_left(indent_px)
            .items_center()
            .cursor(CursorStyle::Pointer)
            .hover(|s| s.background(Color::rgb8(34, 38, 48)))
    })
}

fn render_search_match_row(
    hit: SearchMatch,
    vm_handle: Rc<RefCell<EditorViewModel>>,
) -> impl IntoView {
    let location_badge = format!(
        "{}:{}:{}",
        hit.relative_path.to_string_lossy(),
        hit.line_number,
        hit.column + 1
    );
    let snippet = hit.line_text.trim().to_string();
    let hit_for_click = hit.clone();

    h_stack((
        label(move || location_badge.clone()).style(|s| {
            s.width(260.0)
                .font_size(12.0)
                .font_bold()
                .color(Color::rgb8(97, 175, 239))
                .font_family("Consolas".to_string())
        }),
        label(move || snippet.clone()).style(|s| {
            s.font_size(12.0)
                .color(Color::rgb8(212, 218, 226))
                .font_family("Consolas".to_string())
        }),
    ))
    .on_click_stop(move |_| {
        let mut vm = vm_handle.borrow_mut();
        vm.dispatch(EditorCommand::CloseMenu);
        vm.dispatch(EditorCommand::ClickSearchResult(hit_for_click.clone()));
    })
    .style(|s| {
        s.width_full()
            .height(24.0)
            .padding_horiz(14.0)
            .items_center()
            .cursor(CursorStyle::Pointer)
            .hover(|s| s.background(Color::rgb8(34, 38, 48)))
    })
}

fn render_pytest_failure_row(
    failure: PytestFailure,
    vm_handle: Rc<RefCell<EditorViewModel>>,
) -> impl IntoView {
    let loc_label = format!(
        "● {}:{} ({})",
        failure.relative_path.to_string_lossy(),
        failure.line_number,
        failure.test_name
    );
    let msg_label = failure.message.clone();
    let failure_for_click = failure.clone();

    v_stack((
        label(move || loc_label.clone()).style(|s| {
            s.font_size(12.0)
                .font_bold()
                .color(Color::rgb8(224, 108, 117))
                .font_family("Consolas".to_string())
        }),
        label(move || format!("  {}", msg_label)).style(|s| {
            s.font_size(11.5)
                .color(Color::rgb8(171, 178, 191))
                .font_family("Consolas".to_string())
        }),
    ))
    .on_click_stop(move |_| {
        let mut vm = vm_handle.borrow_mut();
        vm.dispatch(EditorCommand::CloseMenu);
        vm.dispatch(EditorCommand::ClickPytestFailure(failure_for_click.clone()));
    })
    .style(|s| {
        s.width_full()
            .padding_horiz(12.0)
            .padding_vert(4.0)
            .cursor(CursorStyle::Pointer)
            .hover(|s| s.background(Color::rgb8(34, 38, 48)))
    })
}

// ============================================================================
// MAIN APPLICATION VIEW
// ============================================================================

fn app_view(tokio_handle: TokioHandle) -> impl IntoView {
    let mut initial_vm = EditorViewModel::new(LINE_HEIGHT_PX, INITIAL_VIEWPORT_HEIGHT_PX);
    if let Ok(cwd) = std::env::current_dir() {
        initial_vm.dispatch(EditorCommand::OpenWorkspace(cwd));
    }
    initial_vm.dispatch(EditorCommand::LoadSource(INITIAL_PYTHON_SAMPLE.to_string()));
    initial_vm.dispatch(EditorCommand::RunEphemeralAnalysis);

    let signals = initial_vm.signals;
    let vm = Rc::new(RefCell::new(initial_vm));

    let ai_engine = Arc::new(AiEngine::new(OllamaStreamingClient::default()));
    let pytest_runner = Arc::new(PytestRunner::new(ProcessPytestExecutor::new()));

    let vm_menu_file = vm.clone();
    let vm_menu_settings = vm.clone();
    let vm_file_new = vm.clone();
    let vm_file_open = vm.clone();
    let vm_file_save = vm.clone();
    let vm_file_close = vm.clone();

    let vm_sett_search = vm.clone();
    let vm_sett_pytest = vm.clone();
    let vm_sett_ai = vm.clone();
    let vm_sett_fonts = vm.clone();
    let vm_sett_close = vm.clone();

    let vm_font_close = vm.clone();
    let vm_font_done = vm.clone();
    let vm_font_reset = vm.clone();
    let vm_font_backdrop = vm.clone();
    let vm_font_ide_quick = vm.clone();
    let vm_font_ide_segoe = vm.clone();
    let vm_font_ide_inter = vm.clone();
    let vm_font_ed_jb = vm.clone();
    let vm_font_ed_cascadia = vm.clone();
    let vm_font_ed_consolas = vm.clone();
    let vm_font_term_jb = vm.clone();
    let vm_font_term_cascadia = vm.clone();
    let vm_font_term_consolas = vm.clone();

    let vm_tab_files = vm.clone();
    let vm_tab_struct = vm.clone();
    let vm_files_list = vm.clone();
    let vm_outline = vm.clone();
    let vm_rows = vm.clone();
    let vm_wheel = vm.clone();
    let vm_keys = vm.clone();

    let vm_ai_doc = vm.clone();
    let vm_ai_ref = vm.clone();
    let vm_ai_pytest = vm.clone();
    let vm_ai_apply = vm.clone();
    let vm_ai_cancel = vm.clone();
    let ai_eng_doc = ai_engine.clone();
    let ai_eng_ref = ai_engine.clone();
    let ai_eng_test = ai_engine.clone();
    let rt_ai_doc = tokio_handle.clone();
    let rt_ai_ref = tokio_handle.clone();
    let rt_ai_test = tokio_handle.clone();

    let vm_search_fn = vm.clone();
    let vm_search_cls = vm.clone();
    let vm_search_hits = vm.clone();

    let vm_pt_all = vm.clone();
    let vm_pt_file = vm.clone();
    let vm_pt_stop = vm.clone();
    let vm_pt_failures = vm.clone();
    let pt_runner_all = pytest_runner.clone();
    let pt_runner_file = pytest_runner.clone();
    let rt_pt_all = tokio_handle.clone();
    let rt_pt_file = tokio_handle.clone();

    // Top Header Bar & Brand
    let top_bar = h_stack((
        h_stack((
            label(|| "LIDE".to_string()).style(move |s| {
                s.font_size(13.5)
                    .font_bold()
                    .color(Color::rgb8(97, 175, 239))
                    .font_family(signals.ide_font.get())
            }),
        ))
        .style(|s| s.margin_right(18.0).items_center()),

        // File Menu Trigger Button
        label(|| "File".to_string())
            .on_click_stop(move |_| {
                vm_menu_file
                    .borrow_mut()
                    .dispatch(EditorCommand::ToggleMenu(ActiveMenu::File));
            })
            .style(move |s| {
                let is_active = signals.active_menu.get() == ActiveMenu::File;
                s.padding_horiz(12.0)
                    .padding_vert(5.0)
                    .margin_right(4.0)
                    .font_size(12.5)
                    .font_family(signals.ide_font.get())
                    .color(if is_active {
                        Color::WHITE
                    } else {
                        Color::rgb8(212, 218, 226)
                    })
                    .background(if is_active {
                        Color::rgb8(45, 52, 68)
                    } else {
                        Color::TRANSPARENT
                    })
                    .border_radius(4.0)
                    .cursor(CursorStyle::Pointer)
                    .hover(|s| s.background(Color::rgb8(38, 43, 56)))
            }),

        // Settings Menu Trigger Button
        label(|| "Settings".to_string())
            .on_click_stop(move |_| {
                vm_menu_settings
                    .borrow_mut()
                    .dispatch(EditorCommand::ToggleMenu(ActiveMenu::Settings));
            })
            .style(move |s| {
                let is_active = signals.active_menu.get() == ActiveMenu::Settings;
                s.padding_horiz(12.0)
                    .padding_vert(5.0)
                    .margin_right(8.0)
                    .font_size(12.5)
                    .font_family(signals.ide_font.get())
                    .color(if is_active {
                        Color::WHITE
                    } else {
                        Color::rgb8(212, 218, 226)
                    })
                    .background(if is_active {
                        Color::rgb8(45, 52, 68)
                    } else {
                        Color::TRANSPARENT
                    })
                    .border_radius(4.0)
                    .cursor(CursorStyle::Pointer)
                    .hover(|s| s.background(Color::rgb8(38, 43, 56)))
            }),

        empty().style(|s| s.flex_grow(1.0)),

        // Active Document name indicator
        label(move || {
            if let Some(path) = signals.active_file_path.get() {
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Untitled".to_string())
            } else {
                "demo.py".to_string()
            }
        })
        .style(|s| {
            s.font_size(12.0)
                .color(Color::rgb8(140, 148, 162))
                .margin_right(14.0)
        }),

        // Quick indicator pill for dirty state
        label(|| "● Modified".to_string()).style(move |s| {
            let dirty = signals.is_dirty.get();
            s.font_size(11.0)
                .color(Color::rgb8(229, 192, 123))
                .margin_right(10.0)
                .apply_if(!dirty, |s| s.hide())
        }),
    ))
    .style(|s| {
        s.width_full()
            .height(36.0)
            .padding_horiz(14.0)
            .items_center()
            .background(Color::rgb8(26, 28, 35))
    });

    let file_dropdown = v_stack((
        dropdown_menu_item("New File", "Ctrl+N", {
            let vm = vm_file_new.clone();
            move || {
                vm.borrow_mut().dispatch(EditorCommand::NewDocument);
            }
        }),
        dropdown_menu_item("Open Workspace...", "Ctrl+O", {
            let vm = vm_file_open.clone();
            move || {
                if let Ok(cwd) = std::env::current_dir() {
                    let mut m = vm.borrow_mut();
                    m.dispatch(EditorCommand::OpenWorkspace(cwd));
                    m.dispatch(EditorCommand::CloseMenu);
                }
            }
        }),
        dropdown_menu_item("Save Active File", "Ctrl+S", {
            let vm = vm_file_save.clone();
            move || {
                let mut m = vm.borrow_mut();
                m.dispatch(EditorCommand::SaveActiveFile);
                m.dispatch(EditorCommand::CloseMenu);
            }
        }),
        dropdown_divider(),
        dropdown_menu_item("Close Menu", "Esc", {
            let vm = vm_file_close.clone();
            move || {
                vm.borrow_mut().dispatch(EditorCommand::CloseMenu);
            }
        }),
    ))
    .style(move |s| {
        let is_open = signals.active_menu.get() == ActiveMenu::File;
        s.absolute()
            .inset_top(36.0)
            .inset_left(64.0)
            .z_index(100)
            .width(240.0)
            .padding_vert(6.0)
            .padding_horiz(4.0)
            .background(Color::rgb8(33, 36, 46))
            .border(1.0)
            .border_color(Color::rgb8(48, 54, 70))
            .border_radius(6.0)
            .apply_if(!is_open, |s| s.hide())
    });

    let settings_dropdown = v_stack((
        dropdown_menu_item("Toggle Find in Files", "Ctrl+Shift+F", {
            let vm = vm_sett_search.clone();
            move || {
                let mut m = vm.borrow_mut();
                m.dispatch(EditorCommand::ToggleSearchPanel);
                m.dispatch(EditorCommand::CloseMenu);
            }
        }),
        dropdown_menu_item("Toggle Pytest Runner", "Ctrl+Shift+T", {
            let vm = vm_sett_pytest.clone();
            move || {
                let mut m = vm.borrow_mut();
                m.dispatch(EditorCommand::TogglePytestPanel);
                m.dispatch(EditorCommand::CloseMenu);
            }
        }),
        dropdown_menu_item("Toggle AI Assistant", "Ctrl+I", {
            let vm = vm_sett_ai.clone();
            move || {
                let mut m = vm.borrow_mut();
                m.dispatch(EditorCommand::ToggleAiPane);
                m.dispatch(EditorCommand::CloseMenu);
            }
        }),
        dropdown_divider(),
        dropdown_menu_item("Font Settings...", "Fonts", {
            let vm = vm_sett_fonts.clone();
            move || {
                let mut m = vm.borrow_mut();
                m.dispatch(EditorCommand::ToggleFontDialog);
            }
        }),
        dropdown_divider(),
        h_stack((
            label(|| "AI Model:".to_string()).style(|s| s.font_size(12.0).color(Color::rgb8(170, 178, 190))),
            empty().style(|s| s.flex_grow(1.0)),
            label(move || signals.ai_model_name.get()).style(|s| {
                s.font_size(11.5)
                    .color(Color::rgb8(198, 120, 221))
                    .font_family("Consolas".to_string())
            }),
        ))
        .style(|s| s.width_full().height(26.0).padding_horiz(12.0).items_center()),
        h_stack((
            label(|| "Format on Save:".to_string()).style(|s| s.font_size(12.0).color(Color::rgb8(170, 178, 190))),
            empty().style(|s| s.flex_grow(1.0)),
            label(|| "Enabled (Ruff)".to_string()).style(|s| {
                s.font_size(11.5)
                    .color(Color::rgb8(152, 195, 121))
                    .font_family("Consolas".to_string())
            }),
        ))
        .style(|s| s.width_full().height(26.0).padding_horiz(12.0).items_center()),
        dropdown_divider(),
        dropdown_menu_item("Close Menu", "Esc", {
            let vm = vm_sett_close.clone();
            move || {
                vm.borrow_mut().dispatch(EditorCommand::CloseMenu);
            }
        }),
    ))
    .style(move |s| {
        let is_open = signals.active_menu.get() == ActiveMenu::Settings;
        s.absolute()
            .inset_top(36.0)
            .inset_left(120.0)
            .z_index(100)
            .width(280.0)
            .padding_vert(6.0)
            .padding_horiz(4.0)
            .background(Color::rgb8(33, 36, 46))
            .border(1.0)
            .border_color(Color::rgb8(48, 54, 70))
            .border_radius(6.0)
            .apply_if(!is_open, |s| s.hide())
    });

    // Left Dual-Mode Sidebar (Project Files | Ruff AST Structure)
    let sidebar_tab_header = h_stack((
        label(|| "FILES".to_string())
            .on_click_stop(move |_| {
                vm_tab_files
                    .borrow_mut()
                    .dispatch(EditorCommand::SwitchSidebarTab(SidebarTab::ProjectFiles));
            })
            .style(move |s| {
                let is_active =
                    signals.active_sidebar_tab.get() == SidebarTab::ProjectFiles;
                s.flex_grow(1.0)
                    .padding_vert(8.0)
                    .justify_center()
                    .font_size(11.0)
                    .font_bold()
                    .cursor(CursorStyle::Pointer)
                    .color(if is_active {
                        Color::rgb8(229, 192, 123)
                    } else {
                        Color::rgb8(130, 138, 152)
                    })
                    .background(if is_active {
                        Color::rgb8(32, 35, 44)
                    } else {
                        Color::TRANSPARENT
                    })
            }),
        label(|| "STRUCTURE".to_string())
            .on_click_stop(move |_| {
                vm_tab_struct
                    .borrow_mut()
                    .dispatch(EditorCommand::SwitchSidebarTab(SidebarTab::Structure));
            })
            .style(move |s| {
                let is_active =
                    signals.active_sidebar_tab.get() == SidebarTab::Structure;
                s.flex_grow(1.0)
                    .padding_vert(8.0)
                    .justify_center()
                    .font_size(11.0)
                    .font_bold()
                    .cursor(CursorStyle::Pointer)
                    .color(if is_active {
                        Color::rgb8(229, 192, 123)
                    } else {
                        Color::rgb8(130, 138, 152)
                    })
                    .background(if is_active {
                        Color::rgb8(32, 35, 44)
                    } else {
                        Color::TRANSPARENT
                    })
            }),
    ))
    .style(|s| s.width_full());

    let files_pane = scroll(
        dyn_stack(
            move || signals.workspace_files.get(),
            move |file| file.path.clone(),
            move |file| render_workspace_file_item(file, vm_files_list.clone()),
        )
        .style(|s| s.flex_col().width_full()),
    )
    .style(move |s| {
        s.width_full()
            .flex_grow(1.0)
            .apply_if(
                signals.active_sidebar_tab.get() != SidebarTab::ProjectFiles,
                |s| s.hide(),
            )
    });

    let structure_pane = scroll(
        dyn_stack(
            move || signals.outline_items.get(),
            move |item| (item.start_line, item.name.clone(), item.depth),
            move |item| render_outline_item(item, vm_outline.clone()),
        )
        .style(|s| s.flex_col().width_full()),
    )
    .style(move |s| {
        s.width_full()
            .flex_grow(1.0)
            .apply_if(
                signals.active_sidebar_tab.get() != SidebarTab::Structure,
                |s| s.hide(),
            )
    });

    let left_sidebar = v_stack((sidebar_tab_header, files_pane, structure_pane)).style(|s| {
        s.width(250.0)
            .height_full()
            .background(Color::rgb8(20, 22, 28))
    });

    // Center Virtualized Viewport
    let editor_viewport = dyn_stack(
        move || signals.visible_lines.get(),
        move |row| {
            (
                row.line_idx,
                row.is_active_line,
                row.cursor_col,
                row.highest_diagnostic,
                row.text.clone(),
            )
        },
        move |row| render_editor_row(row, vm_rows.clone(), signals.editor_font),
    )
    .style(|s| {
        s.flex_col()
            .flex_grow(1.0)
            .height_full()
            .background(Color::rgb8(26, 28, 35))
    })
    .on_event_stop(EventListener::PointerWheel, move |event| {
        if let Event::PointerWheel(wheel_event) = event {
            let mut model = vm_wheel.borrow_mut();
            let current_y = model.signals.scroll_offset_y.get_untracked();
            let next_y = (current_y - wheel_event.delta.y * 1.5).max(0.0);
            model.dispatch(EditorCommand::ScrollToPixel {
                offset_y: next_y,
                viewport_height: INITIAL_VIEWPORT_HEIGHT_PX,
            });
        }
    });

    // Right-Hand Toggleable AST-Sliced AI Assistant Split Pane (Live Ollama Bridge)
    let right_ai_pane = v_stack((
        label(move || format!("OLLAMA AI ASSISTANT ({})", signals.ai_model_name.get())).style(
            |s| {
                s.width_full()
                    .padding_horiz(12.0)
                    .padding_vert(8.0)
                    .font_size(11.0)
                    .font_bold()
                    .color(Color::rgb8(198, 120, 221))
            },
        ),
        label(move || match signals.active_ai_context.get() {
            Some(ctx) => format!(
                "AST Slice: {} (Ln {}..={})",
                ctx.symbol_name,
                ctx.start_line + 1,
                ctx.end_line + 1
            ),
            None => "AST Slice: Move cursor inside a def/class".to_string(),
        })
        .style(|s| {
            s.width_full()
                .padding_horiz(12.0)
                .padding_vert(6.0)
                .font_size(11.5)
                .color(Color::rgb8(229, 192, 123))
                .background(Color::rgb8(28, 31, 39))
        }),
        h_stack((
            label(|| "+ Docstring".to_string())
                .on_click_stop(move |_| {
                    start_live_ai_task(
                        &vm_ai_doc,
                        &ai_eng_doc,
                        &rt_ai_doc,
                        AiTaskKind::GenerateDocstring,
                    );
                })
                .style(|s| {
                    s.padding_horiz(8.0)
                        .padding_vert(4.0)
                        .margin_right(6.0)
                        .font_size(11.5)
                        .background(Color::rgb8(45, 52, 66))
                        .color(Color::WHITE)
                        .border_radius(4.0)
                        .cursor(CursorStyle::Pointer)
                }),
            label(|| "Refactor".to_string())
                .on_click_stop(move |_| {
                    start_live_ai_task(
                        &vm_ai_ref,
                        &ai_eng_ref,
                        &rt_ai_ref,
                        AiTaskKind::Refactor {
                            instruction: "Optimize & add strict type annotations".to_string(),
                        },
                    );
                })
                .style(|s| {
                    s.padding_horiz(8.0)
                        .padding_vert(4.0)
                        .margin_right(6.0)
                        .font_size(11.5)
                        .background(Color::rgb8(45, 52, 66))
                        .color(Color::WHITE)
                        .border_radius(4.0)
                        .cursor(CursorStyle::Pointer)
                }),
            label(|| "+ Pytest".to_string())
                .on_click_stop(move |_| {
                    start_live_ai_task(
                        &vm_ai_pytest,
                        &ai_eng_test,
                        &rt_ai_test,
                        AiTaskKind::WritePytest,
                    );
                })
                .style(|s| {
                    s.padding_horiz(8.0)
                        .padding_vert(4.0)
                        .margin_right(6.0)
                        .font_size(11.5)
                        .background(Color::rgb8(45, 52, 66))
                        .color(Color::WHITE)
                        .border_radius(4.0)
                        .cursor(CursorStyle::Pointer)
                }),
            label(|| "Cancel".to_string())
                .on_click_stop(move |_| {
                    vm_ai_cancel
                        .borrow_mut()
                        .dispatch(EditorCommand::CancelAiStream);
                })
                .style(|s| {
                    s.padding_horiz(8.0)
                        .padding_vert(4.0)
                        .font_size(11.5)
                        .background(Color::rgb8(92, 48, 52))
                        .color(Color::WHITE)
                        .border_radius(4.0)
                        .cursor(CursorStyle::Pointer)
                }),
        ))
        .style(|s| s.width_full().padding_horiz(12.0).padding_vert(8.0)),
        scroll(
            label(move || signals.ai_stream_text.get()).style(|s| {
                s.width_full()
                    .padding(12.0)
                    .font_size(12.5)
                    .color(Color::rgb8(152, 195, 121))
                    .font_family("Consolas".to_string())
            }),
        )
        .style(|s| {
            s.width_full()
                .flex_grow(1.0)
                .background(Color::rgb8(18, 19, 25))
        }),
        h_stack((
            label(|| "Apply Refactor to Buffer (Undoable via Ctrl+Z)".to_string())
                .on_click_stop(move |_| {
                    vm_ai_apply
                        .borrow_mut()
                        .dispatch(EditorCommand::ApplyAiSuggestion);
                })
                .style(move |s| {
                    let ready = signals.ai_clean_code.get().is_some();
                    s.width_full()
                        .padding_vert(7.0)
                        .justify_center()
                        .font_size(12.0)
                        .font_bold()
                        .border_radius(4.0)
                        .cursor(if ready {
                            CursorStyle::Pointer
                        } else {
                            CursorStyle::Default
                        })
                        .background(if ready {
                            Color::rgb8(48, 112, 78)
                        } else {
                            Color::rgb8(42, 46, 54)
                        })
                        .color(if ready {
                            Color::WHITE
                        } else {
                            Color::rgb8(110, 118, 132)
                        })
                }),
        ))
        .style(|s| {
            s.width_full()
                .padding(10.0)
        }),
    ))
    .style(move |s| {
        s.width(350.0)
            .height_full()
            .background(Color::rgb8(22, 24, 31))
            .apply_if(!signals.ai_pane_open.get(), |s| s.hide())
    });

    // Bottom Toggleable Find-in-Files Panel
    let bottom_search_panel = v_stack((
        h_stack((
            label(|| "FIND IN FILES (RIPGREP REGEX):".to_string()).style(|s| {
                s.font_size(11.0)
                    .font_bold()
                    .color(Color::rgb8(229, 192, 123))
                    .margin_right(12.0)
            }),
            label(move || {
                let q = signals.search_query.get();
                if q.is_empty() {
                    "Pattern: <select preset>".to_string()
                } else {
                    format!("Pattern: /{q}/")
                }
            })
            .style(|s| {
                s.padding_horiz(8.0)
                    .padding_vert(3.0)
                    .margin_right(10.0)
                    .font_size(12.0)
                    .background(Color::rgb8(18, 19, 25))
                    .color(Color::rgb8(152, 195, 121))
                    .border_radius(4.0)
                    .font_family("Consolas".to_string())
            }),
            label(|| "Search `def |fn `".to_string())
                .on_click_stop(move |_| {
                    let mut m = vm_search_fn.borrow_mut();
                    m.dispatch(EditorCommand::SetSearchQuery(
                        r"\b(def|fn)\s+[a-zA-Z_]+".to_string(),
                    ));
                    m.dispatch(EditorCommand::ExecuteWorkspaceSearch { max_results: 100 });
                })
                .style(|s| {
                    s.padding_horiz(8.0)
                        .padding_vert(3.0)
                        .margin_right(6.0)
                        .font_size(11.5)
                        .background(Color::rgb8(45, 52, 66))
                        .color(Color::WHITE)
                        .border_radius(4.0)
                        .cursor(CursorStyle::Pointer)
                }),
            label(|| "Search `class |struct `".to_string())
                .on_click_stop(move |_| {
                    let mut m = vm_search_cls.borrow_mut();
                    m.dispatch(EditorCommand::SetSearchQuery(
                        r"\b(class|struct)\s+[a-zA-Z_]+".to_string(),
                    ));
                    m.dispatch(EditorCommand::ExecuteWorkspaceSearch { max_results: 100 });
                })
                .style(|s| {
                    s.padding_horiz(8.0)
                        .padding_vert(3.0)
                        .margin_right(12.0)
                        .font_size(11.5)
                        .background(Color::rgb8(45, 52, 66))
                        .color(Color::WHITE)
                        .border_radius(4.0)
                        .cursor(CursorStyle::Pointer)
                }),
            label(move || match signals.search_error.get() {
                Some(err) => format!("Regex Error: {err}"),
                None => format!("{} matches", signals.search_results.get().len()),
            })
            .style(|s| s.font_size(11.5).color(Color::rgb8(171, 178, 191))),
        ))
        .style(|s| {
            s.width_full()
                .height(34.0)
                .padding_horiz(14.0)
                .items_center()
                .background(Color::rgb8(24, 26, 33))
        }),
        scroll(
            dyn_stack(
                move || signals.search_results.get(),
                move |hit| (hit.path.clone(), hit.line_number, hit.column),
                move |hit| render_search_match_row(hit, vm_search_hits.clone()),
            )
            .style(|s| s.flex_col().width_full()),
        )
        .style(|s| s.width_full().flex_grow(1.0)),
    ))
    .style(move |s| {
        s.width_full()
            .height(175.0)
            .background(Color::rgb8(20, 22, 28))
            .apply_if(!signals.search_panel_open.get(), |s| s.hide())
    });

    // Bottom Toggleable Pytest Runner Panel (Live Virtualenv `ProcessPytestExecutor` Bridge)
    let bottom_pytest_panel = v_stack((
        h_stack((
            label(|| "PYTEST RUNNER (VIRTUALENV ASYNC):".to_string()).style(|s| {
                s.font_size(11.0)
                    .font_bold()
                    .color(Color::rgb8(86, 182, 194))
                    .margin_right(12.0)
            }),
            label(|| "▶ Run Workspace Tests".to_string())
                .on_click_stop(move |_| {
                    start_live_pytest_run(&vm_pt_all, &pt_runner_all, &rt_pt_all, None);
                })
                .style(|s| {
                    s.padding_horiz(9.0)
                        .padding_vert(3.0)
                        .margin_right(6.0)
                        .font_size(11.5)
                        .background(Color::rgb8(48, 102, 78))
                        .color(Color::WHITE)
                        .border_radius(4.0)
                        .cursor(CursorStyle::Pointer)
                }),
            label(|| "▶ Run Active File".to_string())
                .on_click_stop(move |_| {
                    let target = vm_pt_file
                        .borrow()
                        .signals
                        .active_file_path
                        .get_untracked();
                    start_live_pytest_run(&vm_pt_file, &pt_runner_file, &rt_pt_file, target);
                })
                .style(|s| {
                    s.padding_horiz(9.0)
                        .padding_vert(3.0)
                        .margin_right(6.0)
                        .font_size(11.5)
                        .background(Color::rgb8(45, 76, 108))
                        .color(Color::WHITE)
                        .border_radius(4.0)
                        .cursor(CursorStyle::Pointer)
                }),
            label(|| "■ Stop".to_string())
                .on_click_stop(move |_| {
                    vm_pt_stop
                        .borrow_mut()
                        .dispatch(EditorCommand::CancelPytestRun);
                })
                .style(|s| {
                    s.padding_horiz(9.0)
                        .padding_vert(3.0)
                        .margin_right(12.0)
                        .font_size(11.5)
                        .background(Color::rgb8(92, 48, 52))
                        .color(Color::WHITE)
                        .border_radius(4.0)
                        .cursor(CursorStyle::Pointer)
                }),
            label(move || {
                signals
                    .pytest_summary
                    .get()
                    .unwrap_or_else(|| "Idle (Click Run to execute pytest)".to_string())
            })
            .style(move |s| {
                let has_failures = !signals.pytest_failures.get().is_empty();
                s.font_size(12.0).font_bold().color(if has_failures {
                    Color::rgb8(224, 108, 117)
                } else {
                    Color::rgb8(152, 195, 121)
                })
            }),
        ))
        .style(|s| {
            s.width_full()
                .height(34.0)
                .padding_horiz(14.0)
                .items_center()
                .background(Color::rgb8(24, 26, 33))
        }),
        // Split body: Left = Clickable Failures, Right = Live Streaming Stdout Lines
        h_stack((
            scroll(
                dyn_stack(
                    move || signals.pytest_failures.get(),
                    move |f| (f.path.clone(), f.line_number, f.test_name.clone()),
                    move |f| render_pytest_failure_row(f, vm_pt_failures.clone()),
                )
                .style(|s| s.flex_col().width_full()),
            )
            .style(|s| {
                s.width(380.0)
                    .height_full()
                    .background(Color::rgb8(22, 24, 30))
            }),
            scroll(
                dyn_stack(
                    move || signals.pytest_output_lines.get().into_iter().enumerate(),
                    move |(idx, line)| (*idx, line.clone()),
                    move |(_, line)| {
                        label(move || line.clone()).style(move |s| {
                            s.padding_horiz(12.0)
                                .padding_vert(1.5)
                                .font_size(12.0)
                                .color(Color::rgb8(180, 188, 200))
                                .font_family(signals.terminal_font.get())
                        })
                    },
                )
                .style(|s| s.flex_col().width_full()),
            )
            .style(|s| {
                s.flex_grow(1.0)
                    .height_full()
                    .background(Color::rgb8(18, 19, 25))
            }),
        ))
        .style(|s| s.width_full().flex_grow(1.0)),
    ))
    .style(move |s| {
        s.width_full()
            .height(190.0)
            .background(Color::rgb8(20, 22, 28))
            .apply_if(!signals.pytest_panel_open.get(), |s| s.hide())
    });

    // Bottom Status Bar
    let status_bar = h_stack((label(move || signals.status_bar_text.get()).style(move |s| {
        s.font_size(12.0)
            .color(Color::rgb8(180, 188, 200))
            .font_family(signals.ide_font.get())
    }),))
    .style(|s| {
        s.width_full()
            .height(26.0)
            .padding_horiz(14.0)
            .items_center()
            .background(Color::rgb8(18, 19, 24))
    });

    // Font Configuration Modal Dialog Overlay
    let font_settings_dialog = {
        let vm_close_hdr = vm_font_close.clone();
        let vm_done_btn = vm_font_done.clone();
        let vm_reset_btn = vm_font_reset.clone();
        let vm_backdrop = vm_font_backdrop.clone();

        v_stack((
            // Dialog Card
            v_stack((
                // Header row
                h_stack((
                    v_stack((
                        label(|| "Typography & Font Settings".to_string()).style(move |s| {
                            s.font_size(16.0)
                                .font_bold()
                                .font_family(signals.ide_font.get())
                                .color(Color::rgb8(240, 244, 252))
                        }),
                        label(|| "Configure typefaces for IDE UI, code editor viewport, and terminal.".to_string()).style(move |s| {
                            s.font_size(12.0)
                                .font_family(signals.ide_font.get())
                                .color(Color::rgb8(140, 148, 162))
                                .margin_top(2.0)
                        }),
                    )),
                    empty().style(|s| s.flex_grow(1.0)),
                    label(|| "✕".to_string())
                        .on_click_stop(move |_| {
                            vm_close_hdr.borrow_mut().dispatch(EditorCommand::ToggleFontDialog);
                        })
                        .style(|s| {
                            s.font_size(15.0)
                                .color(Color::rgb8(150, 158, 172))
                                .cursor(CursorStyle::Pointer)
                                .padding(6.0)
                                .hover(|s| s.color(Color::WHITE))
                        }),
                ))
                .style(|s| s.width_full().margin_bottom(16.0)),

                // Section 1: IDE / UI Font
                label(|| "1. IDE / UI Font".to_string()).style(move |s| {
                    s.font_size(12.5)
                        .font_bold()
                        .font_family(signals.ide_font.get())
                        .color(Color::rgb8(97, 175, 239))
                        .margin_bottom(6.0)
                }),
                h_stack((
                    font_choice_chip(
                        "Quicksand",
                        "Quicksand",
                        signals.ide_font,
                        {
                            let vm = vm_font_ide_quick.clone();
                            move || {
                                vm.borrow_mut().dispatch(EditorCommand::SetIdeFont("Quicksand".to_string()));
                            }
                        },
                    ),
                    font_choice_chip(
                        "Segoe UI",
                        "Segoe UI",
                        signals.ide_font,
                        {
                            let vm = vm_font_ide_segoe.clone();
                            move || {
                                vm.borrow_mut().dispatch(EditorCommand::SetIdeFont("Segoe UI".to_string()));
                            }
                        },
                    ),
                    font_choice_chip(
                        "Inter",
                        "Inter",
                        signals.ide_font,
                        {
                            let vm = vm_font_ide_inter.clone();
                            move || {
                                vm.borrow_mut().dispatch(EditorCommand::SetIdeFont("Inter".to_string()));
                            }
                        },
                    ),
                ))
                .style(|s| s.width_full().margin_bottom(12.0)),

                // Section 2: Code Editor Font
                label(|| "2. Code Editor Font".to_string()).style(move |s| {
                    s.font_size(12.5)
                        .font_bold()
                        .font_family(signals.ide_font.get())
                        .color(Color::rgb8(152, 195, 121))
                        .margin_bottom(6.0)
                }),
                h_stack((
                    font_choice_chip(
                        "JetBrains Mono",
                        "JetBrains Mono",
                        signals.editor_font,
                        {
                            let vm = vm_font_ed_jb.clone();
                            move || {
                                vm.borrow_mut().dispatch(EditorCommand::SetEditorFont("JetBrains Mono".to_string()));
                            }
                        },
                    ),
                    font_choice_chip(
                        "Cascadia Code",
                        "Cascadia Code",
                        signals.editor_font,
                        {
                            let vm = vm_font_ed_cascadia.clone();
                            move || {
                                vm.borrow_mut().dispatch(EditorCommand::SetEditorFont("Cascadia Code".to_string()));
                            }
                        },
                    ),
                    font_choice_chip(
                        "Consolas",
                        "Consolas",
                        signals.editor_font,
                        {
                            let vm = vm_font_ed_consolas.clone();
                            move || {
                                vm.borrow_mut().dispatch(EditorCommand::SetEditorFont("Consolas".to_string()));
                            }
                        },
                    ),
                ))
                .style(|s| s.width_full().margin_bottom(12.0)),

                // Section 3: Terminal Font
                label(|| "3. Terminal Font".to_string()).style(move |s| {
                    s.font_size(12.5)
                        .font_bold()
                        .font_family(signals.ide_font.get())
                        .color(Color::rgb8(229, 192, 123))
                        .margin_bottom(6.0)
                }),
                h_stack((
                    font_choice_chip(
                        "JetBrains Mono",
                        "JetBrains Mono",
                        signals.terminal_font,
                        {
                            let vm = vm_font_term_jb.clone();
                            move || {
                                vm.borrow_mut().dispatch(EditorCommand::SetTerminalFont("JetBrains Mono".to_string()));
                            }
                        },
                    ),
                    font_choice_chip(
                        "Cascadia Code",
                        "Cascadia Code",
                        signals.terminal_font,
                        {
                            let vm = vm_font_term_cascadia.clone();
                            move || {
                                vm.borrow_mut().dispatch(EditorCommand::SetTerminalFont("Cascadia Code".to_string()));
                            }
                        },
                    ),
                    font_choice_chip(
                        "Consolas",
                        "Consolas",
                        signals.terminal_font,
                        {
                            let vm = vm_font_term_consolas.clone();
                            move || {
                                vm.borrow_mut().dispatch(EditorCommand::SetTerminalFont("Consolas".to_string()));
                            }
                        },
                    ),
                ))
                .style(|s| s.width_full().margin_bottom(12.0)),

                // Live Preview Box
                v_stack((
                    h_stack((
                        label(|| "LIVE EDITOR PREVIEW".to_string()).style(|s| {
                            s.font_size(10.5)
                                .font_bold()
                                .color(Color::rgb8(140, 148, 160))
                        }),
                        empty().style(|s| s.flex_grow(1.0)),
                        label(move || {
                            let f = signals.editor_font.get();
                            if f == "JetBrains Mono" {
                                "Active: JetBrains Mono"
                            } else if f == "Cascadia Code" {
                                "Active: Cascadia Code"
                            } else {
                                "Active: Consolas"
                            }
                        })
                        .style(|s| s.font_size(11.0).color(Color::rgb8(229, 192, 123))),
                    ))
                    .style(|s| s.width_full().margin_bottom(6.0)),
                    h_stack((
                        label(|| "1  ".to_string()).style(move |s| {
                            s.color(Color::rgb8(92, 99, 112))
                                .font_size(13.0)
                                .font_family(signals.editor_font.get())
                        }),
                        label(|| "def ".to_string()).style(move |s| {
                            s.color(Color::rgb8(198, 120, 221))
                                .font_size(13.0)
                                .font_family(signals.editor_font.get())
                        }),
                        label(|| "compute_norm".to_string()).style(move |s| {
                            s.color(Color::rgb8(97, 175, 239))
                                .font_size(13.0)
                                .font_family(signals.editor_font.get())
                        }),
                        label(|| "(scale: float = 1.0) -> float:".to_string()).style(move |s| {
                            s.color(Color::rgb8(224, 108, 117))
                                .font_size(13.0)
                                .font_family(signals.editor_font.get())
                        }),
                    )),
                    h_stack((
                        label(|| "2  ".to_string()).style(move |s| {
                            s.color(Color::rgb8(92, 99, 112))
                                .font_size(13.0)
                                .font_family(signals.editor_font.get())
                        }),
                        label(|| "    \"\"\"Calculates vector norm with high-speed Ruff AST.\"\"\"".to_string()).style(move |s| {
                            s.color(Color::rgb8(152, 195, 121))
                                .font_size(13.0)
                                .font_family(signals.editor_font.get())
                        }),
                    )),
                ))
                .style(|s| {
                    s.width_full()
                        .padding(12.0)
                        .background(Color::rgb8(20, 22, 28))
                        .border_radius(6.0)
                        .border(1.0)
                        .border_color(Color::rgb8(42, 46, 58))
                        .margin_bottom(16.0)
                }),

                // Footer Actions
                h_stack((
                    label(|| "Reset Defaults".to_string())
                        .on_click_stop(move |_| {
                            let mut m = vm_reset_btn.borrow_mut();
                            m.dispatch(EditorCommand::SetIdeFont("Quicksand".to_string()));
                            m.dispatch(EditorCommand::SetEditorFont("JetBrains Mono".to_string()));
                            m.dispatch(EditorCommand::SetTerminalFont("JetBrains Mono".to_string()));
                        })
                        .style(move |s| {
                            s.padding_horiz(14.0)
                                .padding_vert(7.0)
                                .font_size(12.0)
                                .font_family(signals.ide_font.get())
                                .color(Color::rgb8(180, 186, 200))
                                .background(Color::rgb8(36, 40, 52))
                                .border_radius(5.0)
                                .cursor(CursorStyle::Pointer)
                                .hover(|s| s.background(Color::rgb8(44, 50, 64)))
                        }),
                    empty().style(|s| s.flex_grow(1.0)),
                    label(|| "Done".to_string())
                        .on_click_stop(move |_| {
                            vm_done_btn.borrow_mut().dispatch(EditorCommand::ToggleFontDialog);
                        })
                        .style(move |s| {
                            s.padding_horiz(18.0)
                                .padding_vert(7.0)
                                .font_size(12.0)
                                .font_bold()
                                .font_family(signals.ide_font.get())
                                .color(Color::WHITE)
                                .background(Color::rgb8(97, 175, 239))
                                .border_radius(5.0)
                                .cursor(CursorStyle::Pointer)
                                .hover(|s| s.background(Color::rgb8(78, 156, 220)))
                        }),
                ))
                .style(|s| s.width_full().items_center()),
            ))
            .on_click_stop(|_| {})
            .style(|s| {
                s.width(560.0)
                    .background(Color::rgb8(28, 32, 42))
                    .border(1.0)
                    .border_color(Color::rgb8(48, 54, 70))
                    .border_radius(10.0)
                    .padding(22.0)
            }),
        ))
        .on_click_stop(move |_| {
            vm_backdrop.borrow_mut().dispatch(EditorCommand::ToggleFontDialog);
        })
        .style(move |s| {
            let is_open = signals.font_dialog_open.get();
            s.absolute()
                .size_full()
                .inset_top(0.0)
                .inset_left(0.0)
                .z_index(500)
                .justify_center()
                .items_center()
                .background(Color::rgba8(10, 12, 16, 220))
                .apply_if(!is_open, |s| s.hide())
        })
    };

    v_stack((
        top_bar,
        h_stack((left_sidebar, editor_viewport, right_ai_pane))
            .style(|s| s.width_full().flex_grow(1.0)),
        bottom_search_panel,
        bottom_pytest_panel,
        status_bar,
        file_dropdown,
        settings_dropdown,
        font_settings_dialog,
    ))
    .style(|s| s.width_full().height_full())
    .keyboard_navigable()
    .on_event_stop(EventListener::KeyDown, move |event| {
        if let Event::KeyDown(key_event) = event {
            let mut model = vm_keys.borrow_mut();
            match &key_event.key.logical_key {
                Key::Named(NamedKey::ArrowUp) => {
                    model.dispatch(EditorCommand::MoveCursor(MoveDirection::Up));
                }
                Key::Named(NamedKey::ArrowDown) => {
                    model.dispatch(EditorCommand::MoveCursor(MoveDirection::Down));
                }
                Key::Named(NamedKey::ArrowLeft) => {
                    model.dispatch(EditorCommand::MoveCursor(MoveDirection::Left));
                }
                Key::Named(NamedKey::ArrowRight) => {
                    model.dispatch(EditorCommand::MoveCursor(MoveDirection::Right));
                }
                Key::Named(NamedKey::Home) => {
                    model.dispatch(EditorCommand::MoveCursor(MoveDirection::LineStart));
                }
                Key::Named(NamedKey::End) => {
                    model.dispatch(EditorCommand::MoveCursor(MoveDirection::LineEnd));
                }
                Key::Named(NamedKey::Escape) => {
                    if model.signals.font_dialog_open.get_untracked() {
                        model.dispatch(EditorCommand::ToggleFontDialog);
                    }
                    model.dispatch(EditorCommand::CloseMenu);
                    model.dispatch(EditorCommand::CancelAiStream);
                    model.dispatch(EditorCommand::CancelPytestRun);
                }
                Key::Named(NamedKey::Backspace) => {
                    model.dispatch(EditorCommand::Backspace);
                    model.dispatch(EditorCommand::RunEphemeralAnalysis);
                }
                Key::Named(NamedKey::Enter) => {
                    model.dispatch(EditorCommand::InsertNewlineAutoIndent);
                    model.dispatch(EditorCommand::RunEphemeralAnalysis);
                }
                Key::Named(NamedKey::Tab) => {
                    if key_event.modifiers.shift() {
                        model.dispatch(EditorCommand::DedentLine);
                    } else {
                        model.dispatch(EditorCommand::IndentLine);
                    }
                    model.dispatch(EditorCommand::RunEphemeralAnalysis);
                }
                Key::Character(ch) => {
                    let is_shortcut =
                        key_event.modifiers.control() || key_event.modifiers.meta();

                    if is_shortcut {
                        match ch.as_str() {
                            "n" | "N" => {
                                model.dispatch(EditorCommand::NewDocument);
                            }
                            "o" | "O" => {
                                if let Ok(cwd) = std::env::current_dir() {
                                    model.dispatch(EditorCommand::OpenWorkspace(cwd));
                                }
                            }
                            "s" | "S" => {
                                model.dispatch(EditorCommand::SaveActiveFile);
                            }
                            "f" | "F" if key_event.modifiers.shift() => {
                                model.dispatch(EditorCommand::ToggleSearchPanel);
                            }
                            "t" | "T" if key_event.modifiers.shift() => {
                                model.dispatch(EditorCommand::TogglePytestPanel);
                            }
                            "i" | "I" => {
                                model.dispatch(EditorCommand::ToggleAiPane);
                            }
                            "z" | "Z" => {
                                if key_event.modifiers.shift() {
                                    model.dispatch(EditorCommand::Redo);
                                } else {
                                    model.dispatch(EditorCommand::Undo);
                                }
                                model.dispatch(EditorCommand::RunEphemeralAnalysis);
                            }
                            "y" | "Y" => {
                                model.dispatch(EditorCommand::Redo);
                                model.dispatch(EditorCommand::RunEphemeralAnalysis);
                            }
                            "," => {
                                model.dispatch(EditorCommand::ToggleFontDialog);
                            }
                            _ => {}
                        }
                    } else {
                        model.dispatch(EditorCommand::InsertText(ch.to_string()));
                        model.dispatch(EditorCommand::RunEphemeralAnalysis);
                    }
                }
                _ => {}
            }
        }
    })
}

fn main() {
    app_floem::bundled_fonts::ensure_bundled_fonts();

    let runtime = RuntimeBuilder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Failed to initialize background Tokio runtime");

    let tokio_handle = runtime.handle().clone();
    floem::launch(move || app_view(tokio_handle));
}
