use ratatui::style::Color;

// Mock Theme struct for testing
struct Theme {
    row_even_bg: Option<[u8; 3]>,
    row_odd_bg: Option<[u8; 3]>,
    surface0: Option<[u8; 3]>,
    surface1: Option<[u8; 3]>,
}

impl Theme {
    fn color(&self, rgb: Option<[u8; 3]>, default: Color) -> Color {
        rgb.map_or(default, |[r, g, b]| Color::Rgb(r, g, b))
    }

    fn surface0_color(&self) -> Color {
        self.color(self.surface0, Color::Reset)
    }

    fn surface1_color(&self) -> Color {
        self.color(self.surface1, Color::Reset)
    }

    fn row_even_bg_color(&self) -> Color {
        self.color(self.row_even_bg, self.surface0_color())
    }

    fn row_odd_bg_color(&self) -> Color {
        self.color(self.row_odd_bg, self.surface1_color())
    }
}

fn main() {
    // Test with defined row colors
    let theme_with_rows = Theme {
        row_even_bg: Some([30, 30, 46]),
        row_odd_bg: Some([49, 50, 68]),
        surface0: Some([30, 30, 46]),
        surface1: Some([49, 50, 68]),
    };

    println!("Theme with row colors:");
    println!("Even row: {:?}", theme_with_rows.row_even_bg_color());
    println!("Odd row: {:?}", theme_with_rows.row_odd_bg_color());

    // Test with undefined row colors (should fallback to surface colors)
    let theme_without_rows = Theme {
        row_even_bg: None,
        row_odd_bg: None,
        surface0: Some([30, 30, 46]),
        surface1: Some([49, 50, 68]),
    };

    println!("\nTheme without row colors (fallback):");
    println!("Even row: {:?}", theme_without_rows.row_even_bg_color());
    println!("Odd row: {:?}", theme_without_rows.row_odd_bg_color());

    // Test alternating logic
    println!("\nAlternating row test:");
    for i in 0..5 {
        let bg = if i % 2 == 0 {
            theme_with_rows.row_even_bg_color()
        } else {
            theme_with_rows.row_odd_bg_color()
        };
        println!("Row {}: {:?}", i, bg);
    }
}