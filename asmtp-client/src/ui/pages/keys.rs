use crate::{
    app::App,
    event,
    ui::{util, Focus},
};
use anyhow::{Context as _, Result};
use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

pub struct Keys {
    keys: Vec<String>,
    selected: usize,
    cursor: usize,

    new_key_state: Option<CreatingNewKeyState>,
}

#[derive(Clone)]
enum CreatingNewKeyState {
    EnteringName { name: String },
    Confirm { name: String },
    Create { name: String },
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
            Some(CreatingNewKeyState::EnteringName {
                name: String::new(),
            })
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
        match key {
            event::Key::Enter => match self.new_key_state.take() {
                None => {
                    if self.selected == self.cursor {
                        arboard::Clipboard::new()
                            .unwrap()
                            .set_text(self.keys[self.selected].clone())
                            .unwrap();
                    }
                    self.selected = self.cursor;
                }
                Some(CreatingNewKeyState::EnteringName { name }) => {
                    self.new_key_state = Some(CreatingNewKeyState::Confirm { name });
                }
                Some(CreatingNewKeyState::Confirm { name }) => {
                    self.new_key_state = Some(CreatingNewKeyState::Create { name });
                    focus.pop();
                }
                Some(CreatingNewKeyState::Create { .. }) => {
                    self.new_key_state = None;
                }
            },
            event::Key::Esc => match self.new_key_state.take() {
                None => {
                    self.cursor = self.selected;
                    focus.pop();
                }
                Some(CreatingNewKeyState::EnteringName { .. }) => {
                    self.new_key_state = None;
                    focus.pop();
                }
                Some(CreatingNewKeyState::Confirm { name }) => {
                    self.new_key_state = Some(CreatingNewKeyState::EnteringName { name });
                }
                Some(CreatingNewKeyState::Create { name }) => {
                    self.new_key_state = Some(CreatingNewKeyState::Confirm { name });
                }
            },
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
                focus.push("popup");
                self.new_key_state = Some(CreatingNewKeyState::EnteringName {
                    name: String::new(),
                });
            }
            event::Key::Backspace => {
                if let Some(CreatingNewKeyState::EnteringName { name }) =
                    self.new_key_state.as_mut()
                {
                    name.pop();
                }
            }
            event::Key::Char(c) => {
                if let Some(CreatingNewKeyState::EnteringName { name }) =
                    self.new_key_state.as_mut()
                {
                    if name.len() < 32 {
                        name.push(c);
                    } else {
                        // TODO: Error!
                    }
                }
            }
            _ => {}
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
        match self.new_key_state.take() {
            Some(CreatingNewKeyState::Create { name }) => {
                app.create_new_key(&name)
                    .await
                    .context("Failed to create new key")?;
            }
            state => self.new_key_state = state,
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

    fn draw_popup_area<B>(&self, f: &mut Frame<B>, parent_layer: Rect) -> Rect
    where
        B: Backend,
    {
        // create an area within the parent layer
        let area = util::centered_rect(60, 60, parent_layer);
        let block = Block::default()
            .title("New device key")
            .borders(Borders::ALL);

        let inner = block.inner(area);

        // clear the area under the popup
        f.render_widget(Clear, area);
        f.render_widget(block, area);

        inner
    }

    fn draw_popup_input_device_name<B>(
        &self,
        f: &mut Frame<B>,
        parent_layer: Rect,
        device_name: &str,
        comment: impl AsRef<str>,
        editing: bool,
    ) where
        B: Backend,
    {
        let layer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),
                Constraint::Min(1),
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(parent_layer);
        let message_layer = layer[0];
        let input_layer = layer[2];
        let action_layer = layer[4];

        let message = Span::raw(comment.as_ref());
        let message = Paragraph::new(message)
            .block(Block::default())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false });

        let input = if editing {
            Spans::from(vec![
                Span::raw("Enter device name: "),
                Span::styled(device_name, Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    "█",
                    Style::default()
                        .fg(Color::LightYellow)
                        .add_modifier(Modifier::SLOW_BLINK),
                ),
            ])
        } else {
            Spans::from(vec![
                Span::raw("Confirm device name: "),
                Span::styled(device_name, Style::default().add_modifier(Modifier::BOLD)),
            ])
        };
        let input = Paragraph::new(input)
            .block(Block::default())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false });

        let action = if editing {
            Span::raw("Press <Enter> when ready")
        } else {
            Span::styled(
                "Press <Enter> to confirm",
                Style::default()
                    .bg(Color::LightYellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK),
            )
        };
        let action = Paragraph::new(action)
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false });

        f.render_widget(message, message_layer);
        f.render_widget(input, input_layer);
        f.render_widget(action, action_layer);
    }

    pub fn draw<B>(&self, focus: &Focus, f: &mut Frame<B>, parent_layer: Rect)
    where
        B: Backend,
    {
        self.draw_list(focus, f, parent_layer);

        match &self.new_key_state {
            Some(CreatingNewKeyState::Create { .. }) => {}
            None => {}
            Some(CreatingNewKeyState::EnteringName { name }) => {
                let popup_area = self.draw_popup_area(f, parent_layer);
                let comment = "Set the new key's device name. This key will be used to identity your device within your passport.";
                self.draw_popup_input_device_name(f, popup_area, name.as_str(), comment, true);
            }
            Some(CreatingNewKeyState::Confirm { name }) => {
                let popup_area = self.draw_popup_area(f, parent_layer);
                let comment = "Please confirm you are happy with the name of the key. This will be set in the passport and may not be changed.";
                self.draw_popup_input_device_name(f, popup_area, name.as_str(), comment, false);
            }
        }
    }
}
