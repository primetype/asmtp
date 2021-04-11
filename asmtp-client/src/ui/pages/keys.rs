use crate::{
    app::App,
    event,
    ui::{util, widget, Focus},
};
use anyhow::{Context as _, Result};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

pub struct Keys {
    keys: Vec<String>,
    selected: usize,
    cursor: usize,

    new_key_state: Option<widget::NewKey>,
}

impl Keys {
    pub fn new(app: &App) -> Self {
        let keys = Vec::new();

        let mut keys = Self {
            keys,
            cursor: 0,
            selected: 0,
            new_key_state: None,
        };

        keys.collect_keys(app);
        keys.new_key_state = if keys.keys.is_empty() {
            Some(widget::NewKey::new())
        } else {
            None
        };

        keys
    }

    pub const fn title() -> &'static str {
        "Keys"
    }

    fn has_focus(&self, focus: &Focus) -> bool {
        focus.check_current(Self::title())
    }

    pub fn input(&mut self, focus: &mut Focus, key: event::Key) {
        if self.has_focus(focus) {
            if self.new_key_state.is_some() {
                self.new_key_state = None;
            }
            match key {
                event::Key::Enter => {
                    if self.selected == self.cursor {
                        arboard::Clipboard::new()
                            .unwrap()
                            .set_text(self.keys[self.selected].clone())
                            .unwrap();
                    }
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
                        self.keys.len().wrapping_sub(1)
                    };
                }
                event::Key::Down => {
                    self.cursor = self.cursor.wrapping_add(1).wrapping_rem(self.keys.len());
                }
                event::Key::Char('+') if self.new_key_state.is_none() => {
                    self.new_key_state = Some(widget::NewKey::new());
                    focus.push(widget::NewKey::title());
                }
                _ => {}
            }
        } else if let Some(new_key) = self.new_key_state.as_mut() {
            if new_key.has_focus(focus) && new_key.input(focus, key) {
                self.new_key_state = None;
            }
        } else {
            // error !
        }
    }

    fn collect_keys(&mut self, app: &App) {
        self.keys = app
            .keys
            .iter()
            .enumerate()
            .map(|(i, k)| {
                let selector = if self.selected == i { "X" } else { " " };
                if let Some(key) = k.public_key() {
                    if let Some(alias) = k.alias() {
                        format!("[{}] {} ({})", selector, alias, key)
                    } else {
                        format!("[{}] {}", selector, key)
                    }
                } else {
                    format!("[{}] <locked>", selector)
                }
            })
            .collect::<Vec<_>>();
    }

    pub async fn update(&mut self, app: &mut App) -> Result<()> {
        if let Some(new_key) = self.new_key_state.as_mut() {
            new_key.update(app).await?;
        }

        if !app.keys.is_empty() {
            app.set_current_key(self.selected)
                .await
                .context("Failed to set the selected key")?;
        }

        self.collect_keys(app);

        Ok(())
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
            .keys
            .iter()
            .cloned()
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
            .block(Block::default().title("Identities").borders(Borders::ALL))
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

        if let Some(new_key) = self.new_key_state.as_ref() {
            let popup_area = self.popup_area(parent_layer);
            new_key.draw(focus, f, popup_area);
        }
    }
}
