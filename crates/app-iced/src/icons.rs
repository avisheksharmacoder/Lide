//! Lucide icon integrations and helpers for Lide IDE in Iced.

pub use lucide_icons::Icon;

use iced::font::{Family, Stretch, Style, Weight};
use iced::widget::text;
use iced::{Element, Font};

/// Predefined `iced::Font` referencing the registered Lucide TrueType font family.
pub const LUCIDE_FONT: Font = Font {
    family: Family::Name("lucide"),
    weight: Weight::Normal,
    stretch: Stretch::Normal,
    style: Style::Normal,
};

/// Converts a `lucide_icons::Icon` variant into an `iced::Element` using the embedded Lucide font.
pub fn icon<'a, Message: 'a>(i: Icon) -> Element<'a, Message> {
    text(char::from(i))
        .font(LUCIDE_FONT)
        .size(14)
        .line_height(iced::widget::text::LineHeight::Relative(1.2))
        .into()
}

/// Converts a `lucide_icons::Icon` variant into an `iced::Element` with an explicit color.
pub fn icon_colored<'a, Message: 'a>(i: Icon, color: iced::Color) -> Element<'a, Message> {
    text(char::from(i))
        .font(LUCIDE_FONT)
        .size(14)
        .color(color)
        .line_height(iced::widget::text::LineHeight::Relative(1.2))
        .into()
}

/// Converts a `lucide_icons::Icon` variant into an `iced::Element` with an explicit pixel size.
pub fn icon_sized<'a, Message: 'a>(i: Icon, size: u16) -> Element<'a, Message> {
    text(char::from(i))
        .font(LUCIDE_FONT)
        .size(size)
        .line_height(iced::widget::text::LineHeight::Relative(1.2))
        .into()
}

/// Converts a `lucide_icons::Icon` variant into an `iced::Element` with an explicit pixel size and color.
pub fn icon_sized_colored<'a, Message: 'a>(i: Icon, size: u16, color: iced::Color) -> Element<'a, Message> {
    text(char::from(i))
        .font(LUCIDE_FONT)
        .size(size)
        .color(color)
        .line_height(iced::widget::text::LineHeight::Relative(1.2))
        .into()
}

/// Converts a `lucide_icons::Icon` variant into an `iced::widget::Text` widget.
pub fn icon_text(i: Icon) -> iced::widget::Text<'static> {
    text(char::from(i))
        .font(LUCIDE_FONT)
        .size(14)
        .line_height(iced::widget::text::LineHeight::Relative(1.2))
}
