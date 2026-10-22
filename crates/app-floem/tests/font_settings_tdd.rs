use app_floem::{ActiveMenu, EditorCommand, EditorViewModel};
use floem::reactive::SignalGet;

#[test]
fn test_default_typography_configuration() {
    let scope = floem::reactive::Scope::new();
    let vm = EditorViewModel::new(scope, 22.0, 792.0);

    // Initial default state
    assert_eq!(vm.signals.font_dialog_open.get(), false);
    assert_eq!(
        vm.signals.ide_font.get(),
        "Quicksand",
        "Default IDE UI font must be Quicksand, got: {}",
        vm.signals.ide_font.get()
    );
    assert_eq!(
        vm.signals.editor_font.get(),
        "JetBrains Mono",
        "Default Code Editor font must be JetBrains Mono, got: {}",
        vm.signals.editor_font.get()
    );
    assert_eq!(
        vm.signals.terminal_font.get(),
        "JetBrains Mono",
        "Default Terminal font must be JetBrains Mono, got: {}",
        vm.signals.terminal_font.get()
    );
}

#[test]
fn test_toggle_font_dialog_and_menu_dismissal() {
    let scope = floem::reactive::Scope::new();
    let mut vm = EditorViewModel::new(scope, 22.0, 792.0);

    // Open a menu first
    vm.dispatch(EditorCommand::ToggleMenu(ActiveMenu::Settings));
    assert_eq!(vm.signals.active_menu.get(), ActiveMenu::Settings);

    // Opening font dialog dismisses active menu
    vm.dispatch(EditorCommand::ToggleFontDialog);
    assert_eq!(vm.signals.font_dialog_open.get(), true);
    assert_eq!(vm.signals.active_menu.get(), ActiveMenu::None);

    // Toggling again closes the dialog
    vm.dispatch(EditorCommand::ToggleFontDialog);
    assert_eq!(vm.signals.font_dialog_open.get(), false);
}

#[test]
fn test_font_selection_reactive_dispatch() {
    let scope = floem::reactive::Scope::new();
    let mut vm = EditorViewModel::new(scope, 22.0, 792.0);

    // Switch IDE font to Segoe UI
    vm.dispatch(EditorCommand::SetIdeFont("Segoe UI".to_string()));
    assert_eq!(vm.signals.ide_font.get(), "Segoe UI");

    // Switch Editor font to Cascadia Code
    vm.dispatch(EditorCommand::SetEditorFont("Cascadia Code".to_string()));
    assert_eq!(vm.signals.editor_font.get(), "Cascadia Code");

    // Switch Terminal font to Consolas
    vm.dispatch(EditorCommand::SetTerminalFont("Consolas".to_string()));
    assert_eq!(vm.signals.terminal_font.get(), "Consolas");

    // Reset back to defaults
    vm.dispatch(EditorCommand::SetIdeFont("Quicksand".to_string()));
    vm.dispatch(EditorCommand::SetEditorFont("JetBrains Mono".to_string()));
    vm.dispatch(EditorCommand::SetTerminalFont("JetBrains Mono".to_string()));

    assert_eq!(vm.signals.ide_font.get(), "Quicksand");
    assert_eq!(vm.signals.editor_font.get(), "JetBrains Mono");
    assert_eq!(vm.signals.terminal_font.get(), "JetBrains Mono");
}

#[test]
fn test_bundled_fonts_embedded_and_valid() {
    use app_floem::bundled_fonts::{ensure_bundled_fonts, BUNDLED_FONTS};

    assert_eq!(BUNDLED_FONTS.len(), 11);

    for font in BUNDLED_FONTS {
        assert!(
            font.data.len() > 200_000,
            "Font {} must be a genuine TrueType font (size: {} bytes)",
            font.filename,
            font.data.len()
        );
        // Verify TTF magic number: 0x00010000 or 'OTTO' or 'true'
        assert!(
            font.data.starts_with(&[0x00, 0x01, 0x00, 0x00]) || font.data.starts_with(b"OTTO"),
            "Font {} must have valid TTF/OTF header",
            font.filename
        );
    }

    // Verify ensure_bundled_fonts executes safely
    ensure_bundled_fonts();
}
