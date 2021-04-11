mod keys;
mod messages;
mod passports;
mod settings;

use self::{keys::Keys, messages::Messages, passports::Passports, settings::Settings};
use crate::{app::App, event, ui::Focus};
use anyhow::{Context as _, Result};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::DOT,
    text::Spans,
    widgets::{Block, Borders, Tabs},
    Frame,
};

pub struct Pages {
    pages: Vec<Page>,
    selected: usize,
}

enum Page {
    Keys(Keys),
    Passports(Passports),
    Messages(Messages),
    Settings(Settings),
}

impl Pages {
    pub async fn new(app: &App) -> Self {
        Self {
            pages: vec![
                Page::Keys(Keys::new(app)),
                Page::Passports(Passports::new(app).await),
                Page::Messages(Messages::new(app)),
                Page::Settings(Settings::new(app)),
            ],
            selected: 0,
        }
    }

    pub async fn update(&mut self, app: &mut App) -> Result<()> {
        for page in self.pages.iter_mut() {
            page.update(app).await?
        }
        Ok(())
    }

    pub fn input(&mut self, focus: &mut Focus, key: event::Key) {
        if focus.is_root() {
            match key {
                event::Key::Left => {
                    self.selected = self.selected.wrapping_sub(1) % self.pages.len();
                }
                event::Key::Right => {
                    self.selected = self.selected.wrapping_add(1) % self.pages.len();
                }
                event::Key::Enter => {
                    let title = self.pages[self.selected].title().to_owned();
                    focus.push(title);
                }
                _ => {}
            }
        } else {
            self.pages[self.selected].input(focus, key);
        }
    }

    pub fn draw<B>(&self, focus: &Focus, f: &mut Frame<B>, parent_layer: Rect)
    where
        B: Backend,
    {
        let parent_layer = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(
                [
                    //
                    Constraint::Length(3),
                    Constraint::Min(1),
                ]
                .as_ref(),
            )
            .split(parent_layer);

        let titles = self
            .pages
            .iter()
            .map(|p| p.title())
            .map(Spans::from)
            .collect();

        let selected_style = if focus.is_root() {
            Style::default()
                .bg(Color::LightYellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK)
        } else {
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        };

        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::White))
            .highlight_style(selected_style)
            .select(self.selected)
            .divider(DOT);
        f.render_widget(tabs, parent_layer[0]);

        if let Some(page) = self.pages.get(self.selected) {
            page.draw(focus, f, parent_layer[1])
        } else {
            // ERROR!
        }
    }
}

impl Page {
    pub fn title(&self) -> &str {
        match self {
            Self::Keys { .. } => Keys::title(),
            Self::Passports { .. } => Passports::title(),
            Self::Messages { .. } => Messages::title(),
            Self::Settings { .. } => Settings::title(),
        }
    }

    pub async fn update(&mut self, app: &mut App) -> Result<()> {
        let result = match self {
            Self::Keys(keys) => keys.update(app).await,
            Self::Passports(passports) => passports.update(app).await,
            Self::Messages(messages) => messages.update(app).await,
            Self::Settings(settings) => settings.update(app),
        };

        result.with_context(|| format!("Failed to update page {}", self.title()))
    }

    pub fn input(&mut self, focus: &mut Focus, key: event::Key) {
        match self {
            Self::Keys(keys) => keys.input(focus, key),
            Self::Passports(passports) => passports.input(focus, key),
            Self::Messages(messages) => messages.input(focus, key),
            Self::Settings(settings) => settings.input(focus, key),
        }
    }

    pub fn draw<B>(&self, focus: &Focus, f: &mut Frame<B>, parent_layer: Rect)
    where
        B: Backend,
    {
        match self {
            Self::Keys(keys) => keys.draw(focus, f, parent_layer),
            Self::Passports(passports) => passports.draw(focus, f, parent_layer),
            Self::Messages(messages) => messages.draw(focus, f, parent_layer),
            Self::Settings(settings) => settings.draw(focus, f, parent_layer),
        }
    }
}
