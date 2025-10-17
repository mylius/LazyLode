use ratatui::style::Color;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Theme {
    // Background transparency setting
    #[serde(default)]
    pub transparent_backgrounds: bool,

    // Background colors
    pub base: Option<[u8; 3]>,
    pub surface0: Option<[u8; 3]>,
    pub surface1: Option<[u8; 3]>,
    pub surface2: Option<[u8; 3]>,

    // Content colors
    pub text: Option<[u8; 3]>,
    pub subtext0: Option<[u8; 3]>,
    pub subtext1: Option<[u8; 3]>,

    // Primary colors
    pub blue: Option<[u8; 3]>,
    pub lavender: Option<[u8; 3]>,
    pub sapphire: Option<[u8; 3]>,
    pub mauve: Option<[u8; 3]>,
    pub red: Option<[u8; 3]>,
    pub peach: Option<[u8; 3]>,
    pub yellow: Option<[u8; 3]>,
    pub green: Option<[u8; 3]>,

    // Header colors
    pub header_bg: Option<[u8; 3]>,
    pub header_fg: Option<[u8; 3]>,

    // Row colors
    pub row_even_bg: Option<[u8; 3]>,
    pub row_odd_bg: Option<[u8; 3]>,
}

impl Theme {
    pub fn default() -> Self {
        Self {
            transparent_backgrounds: false,
            base: None,
            surface0: None,
            surface1: None,
            surface2: None,
            text: None,
            subtext0: None,
            subtext1: None,
            blue: None,
            lavender: None,
            sapphire: None,
            mauve: None,
            red: None,
            peach: None,
            yellow: None,
            green: None,
            header_bg: None,
            header_fg: None,
            row_even_bg: None,
            row_odd_bg: None,
        }
    }

    fn color(&self, rgb: Option<[u8; 3]>, default: Color) -> Color {
        rgb.map_or(default, |[r, g, b]| Color::Rgb(r, g, b))
    }

    pub fn bg_color(&self, rgb: Option<[u8; 3]>) -> Color {
        if self.transparent_backgrounds {
            Color::Reset
        } else {
            self.color(rgb, Color::Reset)
        }
    }

    pub fn base_color(&self) -> Color {
        self.bg_color(self.base)
    }

    pub fn surface0_color(&self) -> Color {
        self.bg_color(self.surface0)
    }

    pub fn surface1_color(&self) -> Color {
        self.bg_color(self.surface1)
    }

    pub fn surface2_color(&self) -> Color {
        self.bg_color(self.surface2)
    }

    pub fn text_color(&self) -> Color {
        self.color(self.text, Color::White)
    }

    pub fn subtext0_color(&self) -> Color {
        self.color(self.subtext0, Color::Gray)
    }

    pub fn subtext1_color(&self) -> Color {
        self.color(self.subtext1, Color::DarkGray)
    }

    pub fn accent_color(&self) -> Color {
        self.color(self.mauve, Color::Cyan)
    }

    pub fn header_bg_color(&self) -> Color {
        self.bg_color(self.header_bg)
    }

    pub fn header_fg_color(&self) -> Color {
        self.color(self.header_fg, Color::White)
    }

    pub fn row_even_bg_color(&self) -> Color {
        self.bg_color(self.row_even_bg)
    }

    pub fn row_odd_bg_color(&self) -> Color {
        self.bg_color(self.row_odd_bg)
    }
}
