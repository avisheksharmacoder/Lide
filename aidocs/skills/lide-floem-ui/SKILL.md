---
name: lide-floem-ui
description: "Guidelines and patterns for Floem reactive signals, zero-redraw isolation, and recursive Tokio-to-Floem async bridge pumps."
---

# Floem Reactive UI & Tokio Bridge Patterns in Lide

This skill covers the presentation architecture in `crates/app-floem`: fine-grained reactivity, layout styling, and background Tokio streaming bridges.

---

## 1. Fine-Grained Reactive Signal Rules

In Floem, view closures re-run whenever a reactive signal they track changes. To avoid UI stutter:

1. **Never read unrelated signals inside tight render loops**:
   - The virtualized code viewport reads `signals.visible_lines.get()`.
   - The cursor overlay reads `signals.cursor.get()`.
   - The status bar reads `signals.status_bar_text.get()`.
   - Modifying cursor coordinates must **never** notify `visible_lines` unless the line index scrolled out of view.
2. **Untracked Reads in Event Handlers**:
   - In event callbacks (`on_click_stop`, `on_event_stop`), read current values with `.get_untracked()` to avoid establishing unnecessary reactive subscriptions.

---

## 2. Recursive Tokio -> Floem Stream Pump (`pump_tokio_stream`)

Because Floem's `create_ext_action` creates a **single-shot** callback, streaming continuous events (such as AI tokens or stdout lines) requires a recursive pump:

```rust
use floem::ext_event::create_ext_action;
use floem::reactive::Scope;
use std::rc::Rc;
use tokio::runtime::Handle as TokioHandle;
use tokio::sync::mpsc;

pub fn pump_tokio_stream<T: Send + 'static>(
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
                // Recursively schedule the next ext_action on the UI thread
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
```

---

## 3. Retaining Cancellation Control in `EditorViewModel`

When `handle.rx` is moved into `pump_tokio_stream`:
- The handle's `mpsc::Receiver` is transferred to the async loop.
- The `EditorViewModel` must retain a cloneable `watch::Sender<bool>` cancellation trigger:
  ```rust
  let handle = runner.start_run(spec);
  self.active_pytest_cancel = Some(handle.cancel_sender());
  ```
- When the user presses `Escape` or triggers cancellation, send `true` via `cancel_tx.send(true)`.

---

## 4. Virtualized Viewport Rendering Math

The viewport height is measured in pixels and divided by `line_height_px` (22.0 px):
- Total rows visible: `(viewport_height_px / line_height_px).ceil() as usize + 2` (buffer margin).
- Visible start index: `(scroll_offset_y / line_height_px).floor() as usize`.
- `dyn_stack` renders strictly this slice of lines. Scrolling shifts `scroll_offset_y`, updating only `signals.visible_lines` and `signals.scroll_offset_y`.
