//! Dynamic Theme Engine & Color Palettes for Lide IDE in Iced.
//!
//! Provides `ThemePalette` models for real-time switching between curated
//! dark and light themes, in-memory token color mapping, and backwards-compatible constants.

use iced::Color;
use lang_contract::HighlightToken;

use crate::ThemeMode;

/// Comprehensive color tokens defining a complete IDE theme palette.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemePalette {
    pub name: String,
    pub mode: ThemeMode,

    // Background surfaces
    pub bg_app: Color,
    pub bg_editor: Color,
    pub bg_sidebar: Color,
    pub bg_toolbar: Color,
    pub bg_status_bar: Color,
    pub bg_panel: Color,
    pub bg_active_row: Color,
    pub bg_card: Color,
    pub bg_hover: Color,

    // Borders
    pub border_subtle: Color,
    pub border_focused: Color,

    // Text & Accents
    pub text_primary: Color,
    pub text_muted: Color,
    pub text_accent: Color,

    // Syntax tokens
    pub syntax_keyword: Color,
    pub syntax_function: Color,
    pub syntax_class: Color,
    pub syntax_decorator: Color,
    pub syntax_string: Color,
    pub syntax_number: Color,
    pub syntax_comment: Color,
    pub syntax_operator: Color,
    pub syntax_parameter: Color,
    pub syntax_builtin: Color,

    // Status Badges
    pub badge_error: Color,
    pub badge_warning: Color,
    pub badge_info: Color,
    pub badge_success: Color,
}

impl Default for ThemePalette {
    fn default() -> Self {
        Self::one_dark_pro()
    }
}

impl ThemePalette {
    /// 1. One Dark Pro (Default Dark)
    pub fn one_dark_pro() -> Self {
        Self {
            name: "One Dark Pro".to_string(),
            mode: ThemeMode::Dark,
            bg_app: Color::from_rgb(0.10, 0.11, 0.14),
            bg_editor: Color::from_rgb(0.12, 0.13, 0.17),
            bg_sidebar: Color::from_rgb(0.11, 0.12, 0.15),
            bg_toolbar: Color::from_rgb(0.13, 0.14, 0.18),
            bg_status_bar: Color::from_rgb(0.09, 0.10, 0.13),
            bg_panel: Color::from_rgb(0.11, 0.12, 0.16),
            bg_active_row: Color::from_rgb(0.13, 0.15, 0.19),
            bg_card: Color::from_rgb(0.14, 0.16, 0.20),
            bg_hover: Color::from_rgb(0.18, 0.20, 0.26),
            border_subtle: Color::from_rgb(0.16, 0.18, 0.23),
            border_focused: Color::from_rgb(0.38, 0.69, 0.94),
            text_primary: Color::from_rgb(0.92, 0.93, 0.96),
            text_muted: Color::from_rgb(0.60, 0.64, 0.70),
            text_accent: Color::from_rgb(0.38, 0.69, 0.94),
            syntax_keyword: Color::from_rgb(0.78, 0.47, 0.87),
            syntax_function: Color::from_rgb(0.38, 0.69, 0.94),
            syntax_class: Color::from_rgb(0.90, 0.75, 0.48),
            syntax_decorator: Color::from_rgb(0.34, 0.71, 0.76),
            syntax_string: Color::from_rgb(0.60, 0.76, 0.47),
            syntax_number: Color::from_rgb(0.82, 0.60, 0.40),
            syntax_comment: Color::from_rgb(0.36, 0.39, 0.44),
            syntax_operator: Color::from_rgb(0.67, 0.70, 0.75),
            syntax_parameter: Color::from_rgb(0.88, 0.42, 0.46),
            syntax_builtin: Color::from_rgb(0.34, 0.71, 0.76),
            badge_error: Color::from_rgb(0.88, 0.42, 0.46),
            badge_warning: Color::from_rgb(0.90, 0.75, 0.48),
            badge_info: Color::from_rgb(0.38, 0.69, 0.94),
            badge_success: Color::from_rgb(0.60, 0.76, 0.47),
        }
    }

    /// 2. Dracula Dark
    pub fn dracula_dark() -> Self {
        Self {
            name: "Dracula Dark".to_string(),
            mode: ThemeMode::Dark,
            bg_app: Color::from_rgb(0.13, 0.13, 0.17),
            bg_editor: Color::from_rgb(0.16, 0.16, 0.21),
            bg_sidebar: Color::from_rgb(0.12, 0.12, 0.16),
            bg_toolbar: Color::from_rgb(0.14, 0.15, 0.20),
            bg_status_bar: Color::from_rgb(0.10, 0.10, 0.13),
            bg_panel: Color::from_rgb(0.13, 0.13, 0.17),
            bg_active_row: Color::from_rgb(0.27, 0.28, 0.35),
            bg_card: Color::from_rgb(0.20, 0.22, 0.27),
            bg_hover: Color::from_rgb(0.24, 0.25, 0.32),
            border_subtle: Color::from_rgb(0.23, 0.24, 0.31),
            border_focused: Color::from_rgb(0.74, 0.58, 0.98),
            text_primary: Color::from_rgb(0.97, 0.97, 0.95),
            text_muted: Color::from_rgb(0.65, 0.70, 0.82),
            text_accent: Color::from_rgb(0.74, 0.58, 0.98),
            syntax_keyword: Color::from_rgb(1.00, 0.47, 0.78),
            syntax_function: Color::from_rgb(0.31, 0.98, 0.48),
            syntax_class: Color::from_rgb(0.55, 0.91, 0.99),
            syntax_decorator: Color::from_rgb(1.00, 0.72, 0.42),
            syntax_string: Color::from_rgb(0.95, 0.98, 0.55),
            syntax_number: Color::from_rgb(0.74, 0.58, 0.98),
            syntax_comment: Color::from_rgb(0.55, 0.60, 0.75),
            syntax_operator: Color::from_rgb(1.00, 0.47, 0.78),
            syntax_parameter: Color::from_rgb(1.00, 0.72, 0.42),
            syntax_builtin: Color::from_rgb(0.55, 0.91, 0.99),
            badge_error: Color::from_rgb(1.00, 0.33, 0.33),
            badge_warning: Color::from_rgb(0.95, 0.98, 0.55),
            badge_info: Color::from_rgb(0.55, 0.91, 0.99),
            badge_success: Color::from_rgb(0.31, 0.98, 0.48),
        }
    }

    /// 3. Nord Deep
    pub fn nord_deep() -> Self {
        Self {
            name: "Nord Deep".to_string(),
            mode: ThemeMode::Dark,
            bg_app: Color::from_rgb(0.14, 0.16, 0.20),
            bg_editor: Color::from_rgb(0.18, 0.20, 0.25),
            bg_sidebar: Color::from_rgb(0.13, 0.14, 0.17),
            bg_toolbar: Color::from_rgb(0.16, 0.18, 0.22),
            bg_status_bar: Color::from_rgb(0.11, 0.13, 0.16),
            bg_panel: Color::from_rgb(0.14, 0.16, 0.20),
            bg_active_row: Color::from_rgb(0.23, 0.26, 0.32),
            bg_card: Color::from_rgb(0.21, 0.24, 0.29),
            bg_hover: Color::from_rgb(0.26, 0.30, 0.37),
            border_subtle: Color::from_rgb(0.23, 0.26, 0.32),
            border_focused: Color::from_rgb(0.53, 0.75, 0.82),
            text_primary: Color::from_rgb(0.93, 0.94, 0.96),
            text_muted: Color::from_rgb(0.64, 0.68, 0.76),
            text_accent: Color::from_rgb(0.53, 0.75, 0.82),
            syntax_keyword: Color::from_rgb(0.51, 0.63, 0.76),
            syntax_function: Color::from_rgb(0.53, 0.75, 0.82),
            syntax_class: Color::from_rgb(0.56, 0.74, 0.73),
            syntax_decorator: Color::from_rgb(0.92, 0.80, 0.55),
            syntax_string: Color::from_rgb(0.64, 0.75, 0.55),
            syntax_number: Color::from_rgb(0.71, 0.56, 0.68),
            syntax_comment: Color::from_rgb(0.52, 0.56, 0.65),
            syntax_operator: Color::from_rgb(0.51, 0.63, 0.76),
            syntax_parameter: Color::from_rgb(0.85, 0.87, 0.91),
            syntax_builtin: Color::from_rgb(0.53, 0.75, 0.82),
            badge_error: Color::from_rgb(0.75, 0.38, 0.42),
            badge_warning: Color::from_rgb(0.92, 0.80, 0.55),
            badge_info: Color::from_rgb(0.53, 0.75, 0.82),
            badge_success: Color::from_rgb(0.64, 0.75, 0.55),
        }
    }

    /// 4. GitHub Dark
    pub fn github_dark() -> Self {
        Self {
            name: "GitHub Dark".to_string(),
            mode: ThemeMode::Dark,
            bg_app: Color::from_rgb(0.04, 0.05, 0.07),
            bg_editor: Color::from_rgb(0.05, 0.07, 0.09),
            bg_sidebar: Color::from_rgb(0.04, 0.05, 0.07),
            bg_toolbar: Color::from_rgb(0.09, 0.11, 0.13),
            bg_status_bar: Color::from_rgb(0.04, 0.05, 0.07),
            bg_panel: Color::from_rgb(0.05, 0.07, 0.09),
            bg_active_row: Color::from_rgb(0.09, 0.11, 0.13),
            bg_card: Color::from_rgb(0.13, 0.15, 0.18),
            bg_hover: Color::from_rgb(0.19, 0.21, 0.24),
            border_subtle: Color::from_rgb(0.19, 0.21, 0.24),
            border_focused: Color::from_rgb(0.35, 0.65, 1.00),
            text_primary: Color::from_rgb(0.90, 0.92, 0.95),
            text_muted: Color::from_rgb(0.62, 0.65, 0.70),
            text_accent: Color::from_rgb(0.35, 0.65, 1.00),
            syntax_keyword: Color::from_rgb(1.00, 0.48, 0.45),
            syntax_function: Color::from_rgb(0.82, 0.66, 1.00),
            syntax_class: Color::from_rgb(1.00, 0.65, 0.34),
            syntax_decorator: Color::from_rgb(0.47, 0.75, 1.00),
            syntax_string: Color::from_rgb(0.65, 0.84, 1.00),
            syntax_number: Color::from_rgb(0.47, 0.75, 1.00),
            syntax_comment: Color::from_rgb(0.55, 0.58, 0.62),
            syntax_operator: Color::from_rgb(1.00, 0.48, 0.45),
            syntax_parameter: Color::from_rgb(1.00, 0.65, 0.34),
            syntax_builtin: Color::from_rgb(0.47, 0.75, 1.00),
            badge_error: Color::from_rgb(1.00, 0.48, 0.45),
            badge_warning: Color::from_rgb(1.00, 0.65, 0.34),
            badge_info: Color::from_rgb(0.35, 0.65, 1.00),
            badge_success: Color::from_rgb(0.49, 0.91, 0.53),
        }
    }

    /// 5. One Light (Default Light)
    pub fn one_light() -> Self {
        Self {
            name: "One Light".to_string(),
            mode: ThemeMode::Light,
            bg_app: Color::from_rgb(0.94, 0.94, 0.95),
            bg_editor: Color::from_rgb(0.98, 0.98, 0.98),
            bg_sidebar: Color::from_rgb(0.92, 0.92, 0.92),
            bg_toolbar: Color::from_rgb(0.90, 0.90, 0.90),
            bg_status_bar: Color::from_rgb(0.86, 0.86, 0.86),
            bg_panel: Color::from_rgb(0.94, 0.94, 0.95),
            bg_active_row: Color::from_rgb(0.93, 0.94, 0.96),
            bg_card: Color::from_rgb(1.00, 1.00, 1.00),
            bg_hover: Color::from_rgb(0.89, 0.90, 0.92),
            border_subtle: Color::from_rgb(0.82, 0.82, 0.84),
            border_focused: Color::from_rgb(0.25, 0.47, 0.95),
            text_primary: Color::from_rgb(0.22, 0.23, 0.26),
            text_muted: Color::from_rgb(0.58, 0.59, 0.61),
            text_accent: Color::from_rgb(0.25, 0.47, 0.95),
            syntax_keyword: Color::from_rgb(0.65, 0.15, 0.64),
            syntax_function: Color::from_rgb(0.25, 0.47, 0.95),
            syntax_class: Color::from_rgb(0.76, 0.52, 0.00),
            syntax_decorator: Color::from_rgb(0.00, 0.52, 0.74),
            syntax_string: Color::from_rgb(0.31, 0.63, 0.31),
            syntax_number: Color::from_rgb(0.60, 0.41, 0.00),
            syntax_comment: Color::from_rgb(0.63, 0.63, 0.65),
            syntax_operator: Color::from_rgb(0.22, 0.23, 0.26),
            syntax_parameter: Color::from_rgb(0.89, 0.34, 0.29),
            syntax_builtin: Color::from_rgb(0.00, 0.52, 0.74),
            badge_error: Color::from_rgb(0.89, 0.34, 0.29),
            badge_warning: Color::from_rgb(0.76, 0.52, 0.00),
            badge_info: Color::from_rgb(0.25, 0.47, 0.95),
            badge_success: Color::from_rgb(0.31, 0.63, 0.31),
        }
    }

    /// 6. GitHub Light
    pub fn github_light() -> Self {
        Self {
            name: "GitHub Light".to_string(),
            mode: ThemeMode::Light,
            bg_app: Color::from_rgb(0.96, 0.97, 0.98),
            bg_editor: Color::from_rgb(1.00, 1.00, 1.00),
            bg_sidebar: Color::from_rgb(0.95, 0.96, 0.96),
            bg_toolbar: Color::from_rgb(0.94, 0.95, 0.96),
            bg_status_bar: Color::from_rgb(0.90, 0.91, 0.92),
            bg_panel: Color::from_rgb(1.00, 1.00, 1.00),
            bg_active_row: Color::from_rgb(0.95, 0.96, 0.98),
            bg_card: Color::from_rgb(1.00, 1.00, 1.00),
            bg_hover: Color::from_rgb(0.92, 0.93, 0.95),
            border_subtle: Color::from_rgb(0.82, 0.84, 0.87),
            border_focused: Color::from_rgb(0.04, 0.41, 0.85),
            text_primary: Color::from_rgb(0.12, 0.14, 0.16),
            text_muted: Color::from_rgb(0.40, 0.43, 0.46),
            text_accent: Color::from_rgb(0.04, 0.41, 0.85),
            syntax_keyword: Color::from_rgb(0.81, 0.13, 0.18),
            syntax_function: Color::from_rgb(0.51, 0.31, 0.87),
            syntax_class: Color::from_rgb(0.58, 0.22, 0.00),
            syntax_decorator: Color::from_rgb(0.02, 0.31, 0.68),
            syntax_string: Color::from_rgb(0.04, 0.19, 0.41),
            syntax_number: Color::from_rgb(0.02, 0.31, 0.68),
            syntax_comment: Color::from_rgb(0.43, 0.47, 0.51),
            syntax_operator: Color::from_rgb(0.81, 0.13, 0.18),
            syntax_parameter: Color::from_rgb(0.58, 0.22, 0.00),
            syntax_builtin: Color::from_rgb(0.02, 0.31, 0.68),
            badge_error: Color::from_rgb(0.81, 0.13, 0.18),
            badge_warning: Color::from_rgb(0.58, 0.22, 0.00),
            badge_info: Color::from_rgb(0.04, 0.41, 0.85),
            badge_success: Color::from_rgb(0.10, 0.50, 0.15),
        }
    }

    /// 7. Solarized Light
    pub fn solarized_light() -> Self {
        Self {
            name: "Solarized Light".to_string(),
            mode: ThemeMode::Light,
            bg_app: Color::from_rgb(0.93, 0.91, 0.84),
            bg_editor: Color::from_rgb(0.99, 0.96, 0.89),
            bg_sidebar: Color::from_rgb(0.93, 0.91, 0.84),
            bg_toolbar: Color::from_rgb(0.90, 0.87, 0.79),
            bg_status_bar: Color::from_rgb(0.88, 0.85, 0.76),
            bg_panel: Color::from_rgb(0.93, 0.91, 0.84),
            bg_active_row: Color::from_rgb(0.97, 0.94, 0.86),
            bg_card: Color::from_rgb(0.99, 0.96, 0.89),
            bg_hover: Color::from_rgb(0.89, 0.86, 0.75),
            border_subtle: Color::from_rgb(0.85, 0.82, 0.75),
            border_focused: Color::from_rgb(0.15, 0.55, 0.82),
            text_primary: Color::from_rgb(0.40, 0.48, 0.51),
            text_muted: Color::from_rgb(0.58, 0.63, 0.63),
            text_accent: Color::from_rgb(0.15, 0.55, 0.82),
            syntax_keyword: Color::from_rgb(0.52, 0.60, 0.00),
            syntax_function: Color::from_rgb(0.15, 0.55, 0.82),
            syntax_class: Color::from_rgb(0.71, 0.54, 0.00),
            syntax_decorator: Color::from_rgb(0.16, 0.63, 0.60),
            syntax_string: Color::from_rgb(0.16, 0.63, 0.60),
            syntax_number: Color::from_rgb(0.83, 0.21, 0.51),
            syntax_comment: Color::from_rgb(0.58, 0.63, 0.63),
            syntax_operator: Color::from_rgb(0.52, 0.60, 0.00),
            syntax_parameter: Color::from_rgb(0.80, 0.29, 0.09),
            syntax_builtin: Color::from_rgb(0.15, 0.55, 0.82),
            badge_error: Color::from_rgb(0.86, 0.20, 0.18),
            badge_warning: Color::from_rgb(0.71, 0.54, 0.00),
            badge_info: Color::from_rgb(0.15, 0.55, 0.82),
            badge_success: Color::from_rgb(0.52, 0.60, 0.00),
        }
    }

    /// 8. Quiet Light
    pub fn quiet_light() -> Self {
        Self {
            name: "Quiet Light".to_string(),
            mode: ThemeMode::Light,
            bg_app: Color::from_rgb(0.92, 0.92, 0.92),
            bg_editor: Color::from_rgb(0.96, 0.96, 0.96),
            bg_sidebar: Color::from_rgb(0.93, 0.93, 0.93),
            bg_toolbar: Color::from_rgb(0.89, 0.89, 0.89),
            bg_status_bar: Color::from_rgb(0.87, 0.87, 0.87),
            bg_panel: Color::from_rgb(0.96, 0.96, 0.96),
            bg_active_row: Color::from_rgb(0.92, 0.92, 0.92),
            bg_card: Color::from_rgb(1.00, 1.00, 1.00),
            bg_hover: Color::from_rgb(0.87, 0.87, 0.87),
            border_subtle: Color::from_rgb(0.84, 0.84, 0.84),
            border_focused: Color::from_rgb(0.48, 0.24, 0.62),
            text_primary: Color::from_rgb(0.20, 0.20, 0.20),
            text_muted: Color::from_rgb(0.47, 0.47, 0.47),
            text_accent: Color::from_rgb(0.48, 0.24, 0.62),
            syntax_keyword: Color::from_rgb(0.48, 0.24, 0.62),
            syntax_function: Color::from_rgb(0.67, 0.22, 0.19),
            syntax_class: Color::from_rgb(0.29, 0.51, 0.80),
            syntax_decorator: Color::from_rgb(0.48, 0.24, 0.62),
            syntax_string: Color::from_rgb(0.27, 0.55, 0.15),
            syntax_number: Color::from_rgb(0.61, 0.34, 0.11),
            syntax_comment: Color::from_rgb(0.67, 0.67, 0.67),
            syntax_operator: Color::from_rgb(0.47, 0.47, 0.47),
            syntax_parameter: Color::from_rgb(0.67, 0.22, 0.19),
            syntax_builtin: Color::from_rgb(0.48, 0.24, 0.62),
            badge_error: Color::from_rgb(0.67, 0.22, 0.19),
            badge_warning: Color::from_rgb(0.61, 0.34, 0.11),
            badge_info: Color::from_rgb(0.29, 0.51, 0.80),
            badge_success: Color::from_rgb(0.27, 0.55, 0.15),
        }
    }

    /// Resolves a palette by name and mode. Falls back to default mode palette if name is unrecognized.
    pub fn resolve(theme_name: &str, mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => match theme_name {
                "Dracula Dark" => Self::dracula_dark(),
                "Nord Deep" => Self::nord_deep(),
                "GitHub Dark" => Self::github_dark(),
                _ => Self::one_dark_pro(),
            },
            ThemeMode::Light => match theme_name {
                "GitHub Light" => Self::github_light(),
                "Solarized Light" => Self::solarized_light(),
                "Quiet Light" => Self::quiet_light(),
                _ => Self::one_light(),
            },
        }
    }

    /// Maps a syntax highlight token to the corresponding theme color.
    pub fn token_color(&self, token: Option<HighlightToken>) -> Color {
        match token {
            Some(HighlightToken::Keyword) => self.syntax_keyword,
            Some(HighlightToken::Function) | Some(HighlightToken::Method) => self.syntax_function,
            Some(HighlightToken::Class) | Some(HighlightToken::Type) => self.syntax_class,
            Some(HighlightToken::Decorator) => self.syntax_decorator,
            Some(HighlightToken::String) => self.syntax_string,
            Some(HighlightToken::Number) | Some(HighlightToken::Boolean) => self.syntax_number,
            Some(HighlightToken::Comment) => self.syntax_comment,
            Some(HighlightToken::Operator) => self.syntax_operator,
            Some(HighlightToken::Parameter) => self.syntax_parameter,
            Some(HighlightToken::Builtin) => self.syntax_builtin,
            None => self.text_primary,
        }
    }

    /// Convenience alias returning warning diagnostic token color.
    pub fn diagnostic_warning(&self) -> Color {
        self.badge_warning
    }

    /// Convenience alias returning error diagnostic token color.
    pub fn diagnostic_error(&self) -> Color {
        self.badge_error
    }
}

// ============================================================================
// Legacy Backwards-Compatible Constants (Default One Dark Pro)
// ============================================================================

pub const BG_APP: Color = Color::from_rgb(0.10, 0.11, 0.14);
pub const BG_MAIN: Color = BG_APP;
pub const BG_EDITOR: Color = Color::from_rgb(0.12, 0.13, 0.17);
pub const BG_SIDEBAR: Color = Color::from_rgb(0.11, 0.12, 0.15);
pub const BG_TOOLBAR: Color = Color::from_rgb(0.13, 0.14, 0.18);
pub const BG_STATUS_BAR: Color = Color::from_rgb(0.09, 0.10, 0.13);
pub const BG_PANEL: Color = Color::from_rgb(0.11, 0.12, 0.16);
pub const BG_ACTIVE_ROW: Color = Color::from_rgb(0.13, 0.15, 0.19);
pub const BG_CARD: Color = Color::from_rgb(0.14, 0.16, 0.20);

pub const BORDER_SUBTLE: Color = Color::from_rgb(0.16, 0.18, 0.23);
pub const BORDER_FOCUSED: Color = Color::from_rgb(0.38, 0.69, 0.94);

pub const TEXT_PRIMARY: Color = Color::from_rgb(0.83, 0.85, 0.89);
pub const TEXT_MUTED: Color = Color::from_rgb(0.36, 0.39, 0.44);
pub const TEXT_ACCENT: Color = Color::from_rgb(0.38, 0.69, 0.94);

pub const SYNTAX_KEYWORD: Color = Color::from_rgb(0.78, 0.47, 0.87);
pub const SYNTAX_FUNCTION: Color = Color::from_rgb(0.38, 0.69, 0.94);
pub const SYNTAX_CLASS: Color = Color::from_rgb(0.90, 0.75, 0.48);
pub const SYNTAX_DECORATOR: Color = Color::from_rgb(0.34, 0.71, 0.76);
pub const SYNTAX_STRING: Color = Color::from_rgb(0.60, 0.76, 0.47);
pub const SYNTAX_NUMBER: Color = Color::from_rgb(0.82, 0.60, 0.40);
pub const SYNTAX_COMMENT: Color = Color::from_rgb(0.36, 0.39, 0.44);
pub const SYNTAX_OPERATOR: Color = Color::from_rgb(0.67, 0.70, 0.75);
pub const SYNTAX_PARAMETER: Color = Color::from_rgb(0.88, 0.42, 0.46);
pub const SYNTAX_BUILTIN: Color = Color::from_rgb(0.34, 0.71, 0.76);

pub const BADGE_ERROR: Color = Color::from_rgb(0.88, 0.42, 0.46);
pub const BADGE_WARNING: Color = Color::from_rgb(0.90, 0.75, 0.48);
pub const BADGE_INFO: Color = Color::from_rgb(0.38, 0.69, 0.94);
pub const BADGE_SUCCESS: Color = Color::from_rgb(0.60, 0.76, 0.47);

/// Returns the default One Dark Pro syntax color for a given `HighlightToken`.
pub fn token_color(token: Option<HighlightToken>) -> Color {
    ThemePalette::one_dark_pro().token_color(token)
}
