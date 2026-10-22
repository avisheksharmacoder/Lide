# Lide Modern UI Design & Architecture Specification (`ui_design.md`)

This document outlines the modern, borderless visual design system and component architecture for the **Lide Editor** desktop interface built with Floem.

---

## 1. Design Philosophy: Borderless & Tonal Elevation

Traditional and early-prototype IDEs rely on heavy 1px solid borders (`#282c34` / `#363b48`) to demarcate windows, gutters, and panels. In modern professional tools (such as Zed, JetBrains Fleet, and Cursor), harsh dividing lines are replaced by **tonal background hierarchy** and subtle elevation:

- **Tonal Contrast**: Distinct functional zones are separated by nuanced background value steps (darker sidebars, slightly elevated code viewports, raised floating menus).
- **Reduced Visual Clutter**: Eliminates double-borders and wireframe boxiness, expanding usable screen area and reducing visual fatigue.
- **Organic Geometry**: Soft corner rounding (`border_radius: 4.0`–`6.0` px) on interactive buttons, search inputs, and dropdown overlays.

---

## 2. Color System & Tonal Hierarchy

| Component | Background Color | Hex Code | Visual Role |
| :--- | :--- | :--- | :--- |
| **Window Background / Topbar** | Deep Charcoal | `#1a1c23` | Clean host for window title, brand, and menu triggers. |
| **Top Menu Buttons (File / Settings)** | Surface Neutral | `#242731` | Subtle buttons with smooth hover highlight (`#2e323f`). |
| **Floating Dropdown Overlays** | Elevated Floating Panel | `#222530` | Raised floating cards with rounded corners (`6.0px`). |
| **Left Sidebar (Files / Structure)** | Dark Subdued | `#14151b` | Keeps file trees and symbols secondary to the active editor. |
| **Main Code Viewport** | Primary Canvas | `#1c1e26` | High-contrast, glare-free canvas for syntax-highlighted code. |
| **Active Editor Row** | Soft Blue-Gray | `#252834` | Highlights cursor line without obstructing syntax readability. |
| **Bottom Dock (Search / Pytest)** | Anchored Dark | `#16171e` | Distinguishable from the code viewport when toggled open. |
| **Bottom Status Bar** | Base Foundation | `#121318` | Clean, low-key status metadata strip. |

---

## 3. Top Toolbar & Menu System

The prototype buttons (`Load Sample`, `10,000-Line Benchmark`, and top-row shortcut buttons) have been eliminated. The top toolbar now features:

```
[  LIDE  ]   [ File ▾ ]   [ Settings ▾ ]                                    [ v0.1.0 ]
```

### 3.1. File Dropdown Menu
Activated by clicking **File** (or toggled via keyboard). Options:
1. **New File (`Ctrl+N`)**: Clears the buffer for a fresh document.
2. **Open Workspace... (`Ctrl+O`)**: Re-indexes the current directory or workspace.
3. **Save Active File (`Ctrl+S`)**: Formats the document with Astral Ruff and writes atomically to disk.
4. *--- Separator ---*
5. **Close Menu (`Escape`)**: Dismisses the active dropdown overlay.

### 3.2. Settings Dropdown Menu
Activated by clicking **Settings**. Options:
1. **Toggle Find in Files (`Ctrl+Shift+F`)**: Toggles the bottom Ripgrep search panel.
2. **Toggle Pytest Runner (`Ctrl+Shift+T`)**: Toggles the bottom Pytest runner panel.
3. **Toggle AI Assistant (`Ctrl+I`)**: Toggles the right-hand Ollama assistant split pane.
4. *--- Separator ---*
5. **AI Model (`qwen2.5-coder:7b`)**: Displays the active local LLM model name.
6. **Format-on-Save (Active)**: Indicates automatic Ruff formatting on save.
7. *--- Separator ---*
8. **Shortcuts Guide**: Quick overview of navigation and editing hotkeys.

---

## 4. Editor Enhancements & Caret Visualization

- **Visual Cursor / Caret**: On the active row, a distinct vertical cursor marker `|` or focused caret indicator is rendered at the cursor column position, resolving the previous issue where only the whole line background was shaded.
- **Click-to-Line Navigation**: Clicking any line continues to position the primary cursor and updates status bar line/column telemetry.
- **Click-Outside Dismissal**: Clicking inside the editor viewport or pressing `Escape` automatically closes any open dropdown menu.

---

## 5. Reactive Signal Flow for Dropdown Overlays

Dropdown state is managed via fine-grained Floem signals without triggering full viewport re-renders:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveMenu {
    None,
    File,
    Settings,
}
```

- `active_menu: RwSignal<ActiveMenu>`
- Clicking **File** toggles `ActiveMenu::File` or `ActiveMenu::None`.
- Clicking **Settings** toggles `ActiveMenu::Settings` or `ActiveMenu::None`.
- View closures for each dropdown check `active_menu.get() == ActiveMenu::File/Settings` and conditionally display using Floem's `.apply_if(!is_open, |s| s.hide())`.

---

## 6. Absolute Overlay Geometry (Preventing Screen Shift)

When dropdown menus were initially placed in a vertical layout flow (`v_stack`), opening a menu pushed the rest of the IDE down by the menu's height. This was resolved by switching to **Absolute Floating Overlays**:
- Dropdown menus use `s.absolute().inset_top(36.0).inset_left(...).z_index(100)` to remove them from Taffy's document flow.
- The parent header maintains a fixed height (`height(36.0)`), guaranteeing that the code canvas and sidebar below never shift when toggling menus.
- Floating menus are styled with an elevated background (`#21242e`), a subtle border (`#303646`), and a `6.0px` corner radius.

---

## 7. Window Titlebar & Top Bar Integration Architecture

### Native OS Titlebar (Non-Client Area) vs. Custom CSD Titlebar

On Windows, the top bar containing the **Minimize (`—`)**, **Maximize (`□`)**, and **Close (`✕`)** buttons is rendered by the operating system's Desktop Window Manager (DWM). Application UI cannot directly inject Floem widgets into the OS title bar.

Modern IDEs achieve a unified top bar using one of two patterns:

1. **Integrated Flush Topbar (Current & Recommended for Native Stability)**:
   - The native OS window title bar remains active, providing native Windows 11 Snap Assist, Aero Snap, multi-monitor DPI scaling, and standard window drag.
   - Lide's top bar (height 36px) sits immediately below the title bar, sharing the exact dark charcoal hue (`#1a1c23`), creating a clean, seamless appearance without compromising native window management.

2. **Custom Client-Side Decorations (CSD / Frameless Window - VS Code Style)**:
   - The window is configured as undecorated (`show_titlebar(false)`).
   - Lide renders a unified top bar at `y=0`:
     - **Left**: `[LIDE]` logo, `[File]`, `[Settings]` menus.
     - **Center**: Active file title (`demo.py`) with window-drag hit testing.
     - **Right**: Custom minimize, maximize/restore, and close buttons.
   - Requires custom window dragging and resizing logic.

---

## 8. Typography Architecture & Font Configuration Dialog

Lide implements a tri-role typography system allowing users to independently customize typefaces for:
1. **IDE / UI Font**: Window header, buttons, menus, tabs, and status telemetry. Defaults to **`Quicksand`** (alternatives: `Segoe UI`, `Inter`).
2. **Code Editor Font**: Code lines, syntax tokens, and gutter line numbers. Defaults to **`JetBrains Mono`** (alternatives: `Cascadia Code`, `Consolas`).
3. **Terminal Font**: Pytest output stream, compilation logs, and Ripgrep search results. Defaults to **`JetBrains Mono`** (alternatives: `Cascadia Code`, `Consolas`).

### 8.1. Fontdb & Cosmic-Text Resolution Contract
Unlike browser CSS engines that parse comma-separated fallback chains (e.g. `"JetBrains Mono, Cascadia Code, Consolas"`), Floem's font stack (`cosmic-text` and `fontdb`) queries font family names by **exact string equality**. 
- Passing comma lists causes `fontdb.query()` to fail and fall back to Floem's generic default font.
- Lide strictly enforces single, clean family identifiers (`"JetBrains Mono"`, `"Cascadia Code"`, `"Consolas"`, `"Quicksand"`).

### 8.2. Bundled In-Binary Fonts (`include_bytes!`) & Zero-Network Loading
To guarantee authentic JetBrains Mono glyphs without requiring any network downloads, installer scripts, or administrative elevation:
- All 11 JetBrains Mono font weights (Medium, Bold, Italic, BoldItalic, Light, ExtraLight, ExtraBold, etc.) from `fonts/` are statically embedded directly into the executable binary using `include_bytes!` in `crates/app-floem/src/bundled_fonts.rs`.
- On startup, `app_floem::bundled_fonts::ensure_bundled_fonts()`:
  1. Calls Win32 GDI `AddFontMemResourceEx` to register all font bytes into the process font table in memory.
  2. Ensures font files are written to `%LOCALAPPDATA%\Microsoft\Windows\Fonts` and registers them via `AddFontResourceExW(FR_PRIVATE)` so `fontdb` and DirectWrite can discover the files on disk.
- Startup is instantaneous (< 1 ms), completely offline, and requires zero network access.

### 8.3. Reactive Font Signals
Typography is dynamically linked to granular Floem signals:
- `ide_font: RwSignal<String>`
- `editor_font: RwSignal<String>`
- `terminal_font: RwSignal<String>`
- `font_dialog_open: RwSignal<bool>`

Selecting a different font immediately updates all dependent style closures without restarting the application or dropping frames.

### 8.4. Font Settings Modal Dialog
Accessible via **Settings ▾ -> Font Settings...** (or `Ctrl+,`):
- Centered modal dialog card (`560px` wide) on a dimmed backdrop overlay (`rgba(12, 14, 18, 0.86)`).
- Dedicated selection chips for **IDE Font**, **Editor Font**, and **Terminal Font** with active indicators (`✓`).
- Live code preview box rendering syntax-highlighted Python in the actively selected editor font.
- One-click **Reset Defaults** and **Done** dismissal buttons.

### 8.5. Windows System Font Fallbacks & Offline Support
- **Editor & Terminal**: Defaults to the embedded **`JetBrains Mono`** binary resource. Cascadia Code and Consolas are also available as built-in Windows alternatives.
- **IDE UI**: Defaults to **`Quicksand`**, with instant fallback to **`Segoe UI`** (native Windows standard).
- **No Network Dependency**: All editor fonts are 100% self-contained in the Lide executable.
