use iced::Theme;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AppTheme {
    Light,
    Dark,
}

impl Default for AppTheme {
    fn default() -> Self {
        Self::Dark
    }
}

impl AppTheme {
    pub fn toggle(&self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }

    pub fn to_iced_theme(&self) -> Theme {
        match self {
            Self::Light => Theme::TokyoNightLight,
            Self::Dark => Theme::TokyoNightStorm,
        }
    }
}
