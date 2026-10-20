# Poom Documentation & Development Milestones

Welcome to the architectural specifications and development roadmap for **Poom**, a blazing-fast, test-driven code editor engineered in Rust with **Floem**.

---

## Document Index

1. **[System Architecture](ARCHITECTURE.md)**
   - Core tenets, multi-crate workspace layout, crate dependency graph, coordinate systems, threading model, and performance SLAs.
2. **[Milestone 1 — Headless Rope & Viewport Kernel (`editor-core`)](milestones/01_editor_core.md)**
   - Rope data buffer, multi-cursor engine, selection ranges, undo/redo history tree, viewport line math, and property-based fuzz testing.
3. **[Milestone 2 — Polyglot Contract & Python AST Engine (`lang-contract` + `lang-python`)](milestones/02_language_contract_and_python.md)**
   - Language-agnostic traits, range-bounded Tree-sitter highlighting, embedded Ruff parser/linter/formatter, ephemeral AST disposal, and `insta` snapshot tests.
4. **[Milestone 3 — Minimal Floem Virtual Viewport (`app-floem`)](milestones/03_floem_virtual_viewport.md)**
   - Reactive signal architecture, `virtual_stack` / custom virtual canvas, `cosmic-text` line shaping, gutter, status bar, and headless UI tests.
5. **[Milestone 4 — Inline Ruff Diagnostics & Formatting on Save](milestones/04_diagnostics_and_formatting.md)**
   - Async diagnostic background pipeline, squiggly underlines, gutter indicators, problems panel, format-on-save cursor preservation, and workspace file indexing.
6. **[Milestone 5 — AST-Aware Local AI Assistant (`ai-engine`)](milestones/05_ai_assistant_engine.md)**
   - Mockable Ollama streaming client, AST-aware context pruning, cancellation tokens, inline ghost-text completion, and side-panel assistant.
7. **[Milestone 6 — Polyglot Extensions & Binary Stubs](milestones/06_polyglot_extensions.md)**
   - Virtual Stub Interceptor for `.pyd`/`.so` compiled extensions, Cython `.pyx`/`.pxd` support, and async LSP adapters for C/C++ (`clangd`) and Rust (`rust-analyzer`).

---

## Milestone Status Overview

| Milestone | Crate(s) | Primary Deliverable | Status |
| :--- | :--- | :--- | :--- |
| **Milestone 1** | `editor-core` | Headless Rope & Viewport Kernel with Property Tests | ✅ Complete |
| **Milestone 2** | `lang-contract`, `lang-python` | Syntax Highlighting, Outline & Ruff Linter/Formatter | ✅ Complete |
| **Milestone 3** | `app-floem` | Floem Reactive Virtual Canvas & Headless UI Harness | ✅ Complete |
| **Milestone 4** | `workspace-engine`, `app-floem` | Diagnostics UI, Format on Save, File Discovery | ✅ Complete |
| **Milestone 5** | `ai-engine`, `app-floem` | AST-Guided Ollama Streaming Assistant | ✅ Complete |
| **Milestone 6** | `lang-contract`, `workspace-engine` | Polyglot C/Cython/Rust Support & `.pyd` Virtual Stubs | 📝 Planned |

---

## Guiding Principles for Implementation

- **Strict TDD**: Write tests before implementation (Red -> Green -> Refactor).
- **Zero Headless Regressions**: All core logic must pass in `cargo test` without an X11/Wayland/GPU display.
- **Microsecond SLAs**: Keep all operations sub-millisecond wherever possible; never block the UI thread on IO, AST parsing, or formatting.
