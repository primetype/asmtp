use iced::{button, radio, Background, Color, Vector};

pub struct Radio;

pub enum Button {
    Primary,
    Secondary,
}

impl radio::StyleSheet for Radio {
    fn active(&self) -> radio::Style {
        radio::Style {
            background: Background::Color(Color::WHITE),
            dot_color: Color::WHITE,
            border_width: 0.1,
            border_color: Color::WHITE,
        }
    }

    fn hovered(&self) -> radio::Style {
        self.active()
    }
}

impl button::StyleSheet for Button {
    fn active(&self) -> button::Style {
        button::Style {
            background: Some(Background::Color(match self {
                Button::Primary => Color::from_rgb(0.11, 0.42, 0.87),
                Button::Secondary => Color::from_rgb(0.5, 0.5, 0.5),
            })),
            border_radius: 12.,
            shadow_offset: Vector::new(1.0, 1.0),
            text_color: Color::from_rgb8(0xEE, 0xEE, 0xEE),
            ..button::Style::default()
        }
    }

    fn hovered(&self) -> button::Style {
        button::Style {
            text_color: Color::WHITE,
            shadow_offset: Vector::new(1.0, 2.0),
            ..self.active()
        }
    }
}
