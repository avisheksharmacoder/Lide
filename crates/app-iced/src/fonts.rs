//! In-memory TrueType font embedding for Lide IDE using Iced.
//!
//! Statically embeds JetBrains Mono TrueType fonts at compile time.
//! This ensures zero external network requests, zero administrative elevation,
//! and complete immunity to missing OS directory paths.

use iced::font::{Family, Stretch, Style, Weight};
use iced::Font;

/// Raw TrueType font bytes embedded into the executable binary.
pub const JETBRAINS_MONO_MEDIUM_BYTES: &[u8] =
    include_bytes!("../../../fonts/JetBrainsMono-Medium.ttf");
pub const JETBRAINS_MONO_BOLD_BYTES: &[u8] =
    include_bytes!("../../../fonts/JetBrainsMono-Bold.ttf");
pub const JETBRAINS_MONO_ITALIC_BYTES: &[u8] =
    include_bytes!("../../../fonts/JetBrainsMono-Italic.ttf");
pub const JETBRAINS_MONO_BOLD_ITALIC_BYTES: &[u8] =
    include_bytes!("../../../fonts/JetBrainsMono-BoldItalic.ttf");

/// All font byte slices to be passed to Iced's application font loader.
pub const EMBEDDED_FONTS: &[&[u8]] = &[
    JETBRAINS_MONO_MEDIUM_BYTES,
    JETBRAINS_MONO_BOLD_BYTES,
    JETBRAINS_MONO_ITALIC_BYTES,
    JETBRAINS_MONO_BOLD_ITALIC_BYTES,
];

/// The primary monospaced font for the code editor and terminal.
///
/// Binds directly to the bundled `JetBrainsMono-Medium.ttf` TrueType font face,
/// which declares font family `"JetBrains Mono"` and weight 500 (`Weight::Medium`).
pub const FONT_JETBRAINS_MONO: Font = Font {
    family: Family::Name("JetBrains Mono"),
    weight: Weight::Medium,
    stretch: Stretch::Normal,
    style: Style::Normal,
};

/// Medium weight variant (matches the primary base weight of the bundled TTF).
pub const FONT_JETBRAINS_MONO_MEDIUM: Font = FONT_JETBRAINS_MONO;

/// Bold weight variant for syntax keywords and emphasis.
pub const FONT_JETBRAINS_MONO_BOLD: Font = Font {
    family: Family::Name("JetBrains Mono"),
    weight: Weight::Bold,
    stretch: Stretch::Normal,
    style: Style::Normal,
};

/// Italic variant for syntax comments and docstrings.
pub const FONT_JETBRAINS_MONO_ITALIC: Font = Font {
    family: Family::Name("JetBrains Mono"),
    weight: Weight::Normal,
    stretch: Stretch::Normal,
    style: Style::Italic,
};

/// Cascadia Code fallback font token.
pub const FONT_CASCADIA_CODE: Font = Font {
    family: Family::Name("Cascadia Code"),
    weight: Weight::Normal,
    stretch: Stretch::Normal,
    style: Style::Normal,
};

/// Consolas fallback font token.
pub const FONT_CONSOLAS: Font = Font {
    family: Family::Name("Consolas"),
    weight: Weight::Normal,
    stretch: Stretch::Normal,
    style: Style::Normal,
};

/// Maps a font name string to an `iced::Font`.
pub fn resolve_font_by_name(name: &str) -> Font {
    match name {
        "Cascadia Code" => FONT_CASCADIA_CODE,
        "Consolas" => FONT_CONSOLAS,
        "JetBrains Mono Medium" => FONT_JETBRAINS_MONO_MEDIUM,
        _ => FONT_JETBRAINS_MONO,
    }
}
