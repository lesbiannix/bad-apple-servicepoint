use ratatui::style::{Color, Style};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub status_bar: Style,
    pub video: VideoTheme,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VideoTheme {
    pub characters: [char; 8],
    pub style: Style,
}

impl Theme {
    pub fn default_themes() -> HashMap<String, Self> {
        let mut themes = HashMap::new();
        themes.insert("default".to_string(), Self::default_theme());
        themes.insert("matrix".to_string(), Self::matrix_theme());
        themes.insert("inverted".to_string(), Self::inverted_theme());
        themes
    }

    fn default_theme() -> Self {
        Self {
            name: "default".to_string(),
            status_bar: Style::default().bg(Color::White).fg(Color::Black),
            video: VideoTheme {
                characters: [' ', '.', ':', '-', '=', '+', '*', '#'],
                style: Style::default(),
            },
        }
    }

    fn matrix_theme() -> Self {
        Self {
            name: "matrix".to_string(),
            status_bar: Style::default().bg(Color::Black).fg(Color::Green),
            video: VideoTheme {
                characters: [' ', '`', '.', ',', '-', '~', ':', ';'],
                style: Style::default().fg(Color::Green),
            },
        }
    }

    fn inverted_theme() -> Self {
        Self {
            name: "inverted".to_string(),
            status_bar: Style::default().bg(Color::Black).fg(Color::White),
            video: VideoTheme {
                characters: ['#', '*', '+', '=', '-', ':', '.', ' '],
                style: Style::default().bg(Color::White).fg(Color::Black),
            },
        }
    }
}