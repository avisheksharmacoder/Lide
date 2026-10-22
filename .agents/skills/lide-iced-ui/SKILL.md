---
name: lide-iced-ui
description: "Guidelines and patterns for Iced Elm Architecture (TEA), widget layouts, in-memory font tokens, and native Tokio streaming tasks."
---

# Iced TEA UI & Tokio Streaming Architecture in Lide

This skill covers the presentation architecture in `crates/app-iced`: The Elm Architecture (TEA), in-memory TrueType font embedding, widget layouts, and native Tokio streaming tasks.

---

## 1. The Elm Architecture (TEA) State Transitions

In Iced, state mutation and rendering are strictly separated:
$$\text{State} \times \text{Message} \longrightarrow \text{State} \times \text{Task}\langle\text{Message}\rangle$$

1. **State**: Unified in `LideApp`. All buffer modifications, viewport metrics, and panel states reside here.
2. **Deterministic `update()`**: Every user interaction, hotkey, or background event dispatches an `EditorCommand`.
3. **Pure Function `view()`**: Layout is rendered purely from current state with zero state mutation during render passes.
4. **Subscriptions**: Global keyboard events and continuous background streams are managed through `LideApp::subscription`.

---

## 2. In-Memory TrueType Font Embedding (`fonts.rs`)

To bypass Windows GDI, registry entries, and OS font directory permission bugs:
1. TrueType font bytes are statically embedded into the binary using `include_bytes!("../../../fonts/...")`.
2. Fonts are registered with Iced at application launch:
   ```rust
   iced::application("Lide", LideApp::update, LideApp::view)
       .font(JETBRAINS_MONO_MEDIUM_BYTES)
       .font(JETBRAINS_MONO_BOLD_BYTES)
       .font(JETBRAINS_MONO_ITALIC_BYTES)
       .font(JETBRAINS_MONO_BOLD_ITALIC_BYTES)
       .default_font(FONT_JETBRAINS_MONO)
   ```
3. Font tokens are statically defined:
   ```rust
   pub const FONT_JETBRAINS_MONO: iced::Font = iced::Font::with_name("JetBrains Mono");
   ```
4. **Vector Icon Font Embedding (`lucide-icons`)**:
   - Monospace coding fonts (JetBrains Mono) do not contain color emoji glyphs.
   - Vector iconography is powered by `lucide-icons` (`features = ["iced"]`), registering `LUCIDE_FONT_BYTES` at startup.
   - Icons are rendered via `crate::icons::icon(Icon::...)` into resolution-independent vector widgets matching IDE theme colors.

---

## 3. Native Tokio Async Streaming Tasks

Unlike single-shot UI bridges, Iced supports native streaming tasks:

```rust
EditorCommand::TriggerAiTask(task) => {
    if let Some(req) = self.prepare_ai_task_request(task) {
        let ai_engine = AiEngine::new(ai_engine::OllamaStreamingClient::default());
        let handle = ai_engine.start_stream(req);
        self.active_ai_cancel = Some(handle.cancel_sender());
        let mut rx = handle.rx;
        return Task::stream(iced::stream::channel(128, move |mut output| async move {
            use iced::futures::SinkExt;
            while let Some(event) = rx.recv().await {
                if output.send(EditorCommand::ReceiveAiStreamEvent(event)).await.is_err() {
                    break;
                }
            }
        }));
    }
    Task::none()
}
```

---

## 4. Retaining Cancellation Control in `LideApp`

When streaming tasks run in the background:
- `handle.cancel_sender()` stores a cloneable `watch::Sender<bool>` in `self.active_ai_cancel` or `self.active_pytest_cancel`.
- When the user presses `Escape` or clicks Cancel, `EditorCommand::CancelAiStream` or `EditorCommand::CancelPytestRun` sends `true` via `cancel_tx.send(true)`.

---

## 5. Virtualized Viewport Rendering Math

The viewport height is measured in pixels and divided by `line_height_px` (22.0 px):
- Total rows visible: `(viewport_height_px / line_height_px).ceil() as usize + 2` (buffer margin).
- Visible start index: `(scroll_offset_y / line_height_px).floor() as usize`.
- `views::editor::render_editor` renders strictly this slice of visible lines (`app.visible_lines`).
- Scrolling dispatches `EditorCommand::ScrollToPixel { offset_y, viewport_height }`, which re-slices `app.visible_lines` and refreshes syntax spans.
