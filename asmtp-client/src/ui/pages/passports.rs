use crate::{
    app::App,
    event,
    ui::{util, widget, Focus},
};
use anyhow::{Context as _, Result};
use keynesis::{key::ed25519::PublicKey, passport::block::Hash};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

pub struct Passports {
    key: Option<Option<PublicKey>>,
    passports: Vec<Hash>,

    selected: usize,
    cursor: usize,

    new_passport: Option<widget::NewPassport>,
}

impl Passports {
    pub async fn new(app: &App) -> Self {
        let key = app.current_key().map(|k| k.public_key().cloned());

        let mut passports = Self {
            key,
            passports: Vec::new(),

            cursor: 0,
            selected: 0,
            new_passport: None,
        };

        passports.reset_list(app);

        passports
    }

    pub const fn title() -> &'static str {
        "Passports"
    }

    fn has_focus(&self, focus: &Focus) -> bool {
        focus.check_current(Self::title())
    }

    pub fn input(&mut self, focus: &mut Focus, key: event::Key) {
        if self.has_focus(focus) {
            if self.new_passport.is_some() {
                self.new_passport = None;
            }

            match key {
                event::Key::Enter => {
                    self.selected = self.cursor;
                }
                event::Key::Esc => {
                    self.cursor = self.selected;
                    focus.pop();
                }
                event::Key::Up => {
                    self.cursor = if let Some(a) = self.cursor.checked_sub(1) {
                        a
                    } else {
                        self.passports.len().wrapping_sub(1)
                    };
                }
                event::Key::Down => {
                    self.cursor = self
                        .cursor
                        .wrapping_add(1)
                        .wrapping_rem(self.passports.len());
                }
                event::Key::Char('+') if self.new_passport.is_none() => {
                    self.new_passport = Some(widget::NewPassport::new());
                    focus.push(widget::NewPassport::title());
                }
                _ => {}
            }
        } else if let Some(new_passport) = self.new_passport.as_mut() {
            if new_passport.has_focus(focus) && new_passport.input(focus, key) {
                self.new_passport = None;
            }
        } else {
            // error !
        }
    }

    pub async fn update(&mut self, app: &mut App) -> Result<()> {
        let new_key = app.current_key().map(|k| k.public_key().cloned());
        if new_key != self.key {
            self.new_passport = None;
        }
        self.key = new_key;

        if let Some(new_passport) = self.new_passport.as_mut() {
            new_passport.update(app).await?;
        }

        self.reset_list(app);

        app.set_current_passport(self.passports.get(self.selected).copied())
            .await
            .context("Failed to set the currently selected passport")?;

        Ok(())
    }

    fn reset_list(&mut self, app: &App) {
        self.passports.clear();
        if let Some(current_key) = app.current_key() {
            if let Some(pk) = current_key.public_key() {
                if let Some(p) = app.passports.get_by_key(pk) {
                    self.passports.extend(p.into_iter().map(|p| p.id()));
                }
            }
        }

        if self.passports.is_empty() && self.new_passport.is_none() {
            // force creating a passport
            self.new_passport = Some(widget::NewPassport::new());
        };
    }

    fn draw_list<B>(&self, focus: &Focus, f: &mut Frame<B>, parent_layer: Rect)
    where
        B: Backend,
    {
        let layer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Ratio(1, 1)])
            .split(parent_layer);

        let items = self
            .passports
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let selector = if self.selected == i { "X" } else { " " };
                format!("[{}] {}", selector, p)
            })
            .map(ListItem::new)
            .collect::<Vec<_>>();

        let selected_style = if self.has_focus(focus) {
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

        let list = List::new(items)
            .block(Block::default().title("Passports").borders(Borders::ALL))
            .highlight_style(selected_style);

        let mut selected = ListState::default();
        if self.has_focus(focus) {
            selected.select(Some(self.cursor));
        } else {
            selected.select(Some(self.selected));
        }
        f.render_stateful_widget(list, layer[0], &mut selected);
    }

    fn popup_area(&self, parent_layer: Rect) -> Rect {
        // create an area within the parent layer
        util::centered_rect(60, 60, parent_layer)
    }

    pub fn draw<B>(&self, focus: &Focus, f: &mut Frame<B>, parent_layer: Rect)
    where
        B: Backend,
    {
        self.draw_list(focus, f, parent_layer);

        if let Some(new_passport) = self.new_passport.as_ref() {
            let popup_area = self.popup_area(parent_layer);
            new_passport.draw(focus, f, popup_area);
        }
    }
}
