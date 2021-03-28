use crate::{
    app::App,
    event,
    ui::{util, Focus},
};
use anyhow::{Context as _, Result};
use keynesis::{hash::Blake2b, key::ed25519::PublicKey, passport::block::Hash, Seed};
use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

enum NewPassportState {
    CreateOrBind,
    CreateNewPassphrase {
        passphrase: String,
    },
    CreateConfirmPassphrase {
        passphrase: String,
        confirmation: String,
        matches: Option<bool>,
    },
    Create {
        passphrase: String,
    },
}

pub struct Passports {
    key: Option<Option<PublicKey>>,
    passports: Vec<Hash>,

    selected: usize,
    cursor: usize,

    new_passport: Option<NewPassportState>,
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
        match key {
            event::Key::Enter => match self.new_passport.take() {
                None => {
                    self.selected = self.cursor;
                }
                Some(NewPassportState::CreateOrBind) => {
                    self.new_passport = Some(NewPassportState::CreateNewPassphrase {
                        passphrase: String::new(),
                    });
                }
                Some(NewPassportState::CreateNewPassphrase { passphrase }) => {
                    self.new_passport = Some(NewPassportState::CreateConfirmPassphrase {
                        passphrase,
                        confirmation: String::new(),
                        matches: None,
                    });
                }
                Some(NewPassportState::CreateConfirmPassphrase {
                    passphrase,
                    confirmation,
                    ..
                }) if passphrase == confirmation => {
                    self.new_passport = Some(NewPassportState::Create { passphrase });
                }
                Some(NewPassportState::CreateConfirmPassphrase {
                    passphrase,
                    confirmation,
                    ..
                }) => {
                    self.new_passport = Some(NewPassportState::CreateConfirmPassphrase {
                        passphrase,
                        confirmation,
                        matches: Some(false),
                    });
                }
                Some(NewPassportState::Create { .. }) => {
                    focus.pop();
                }
            },
            event::Key::Esc => match self.new_passport.take() {
                None => {
                    self.cursor = self.selected;
                    focus.pop();
                }
                Some(NewPassportState::CreateOrBind) => {
                    self.new_passport = None;
                    focus.pop();
                }
                Some(NewPassportState::CreateNewPassphrase { .. }) => {
                    self.new_passport = Some(NewPassportState::CreateOrBind);
                }
                Some(NewPassportState::CreateConfirmPassphrase { passphrase, .. }) => {
                    self.new_passport = Some(NewPassportState::CreateNewPassphrase { passphrase });
                }
                Some(NewPassportState::Create { passphrase }) => {
                    self.new_passport = Some(NewPassportState::CreateConfirmPassphrase {
                        passphrase,
                        confirmation: String::new(),
                        matches: None,
                    });
                }
            },
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
                self.new_passport = Some(NewPassportState::CreateOrBind);
                focus.push("New Passport");
            }
            event::Key::Backspace => match self.new_passport.as_mut() {
                None => {}
                Some(NewPassportState::CreateOrBind) => {}
                Some(NewPassportState::CreateNewPassphrase { passphrase }) => {
                    passphrase.pop();
                }
                Some(NewPassportState::CreateConfirmPassphrase { confirmation, .. }) => {
                    confirmation.pop();
                }
                Some(NewPassportState::Create { .. }) => {}
            },
            event::Key::Char(c) => match self.new_passport.as_mut() {
                None => {}
                Some(NewPassportState::CreateOrBind) => {}
                Some(NewPassportState::CreateNewPassphrase { passphrase }) => {
                    passphrase.push(c);
                }
                Some(NewPassportState::CreateConfirmPassphrase { confirmation, .. }) => {
                    confirmation.push(c);
                }
                Some(NewPassportState::Create { .. }) => {}
            },
            _ => {}
        }
    }

    pub async fn update(&mut self, app: &mut App) -> Result<()> {
        let new_key = app.current_key().map(|k| k.public_key().cloned());
        if new_key != self.key {
            self.new_passport = None;
        }
        self.key = new_key;

        match self.new_passport.take() {
            Some(NewPassportState::Create { passphrase }) => {
                let seed = {
                    let mut key = [0; 32];
                    Blake2b::blake2b(&mut key, passphrase.as_bytes(), &[]);
                    Seed::derive_from_key(&key, &[])
                };
                app.create_new_passport(seed)
                    .await
                    .context("Failed to create new passport")?;
            }
            state => {
                self.new_passport = state;
            }
        };

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
            self.new_passport = Some(NewPassportState::CreateOrBind);
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

    fn draw_popup_area<B>(&self, f: &mut Frame<B>, parent_layer: Rect) -> Rect
    where
        B: Backend,
    {
        // create an area within the parent layer
        let area = util::centered_rect(60, 60, parent_layer);
        let block = Block::default().title("New passport").borders(Borders::ALL);

        let inner = block.inner(area);

        // clear the area under the popup
        f.render_widget(Clear, area);
        f.render_widget(block, area);

        inner
    }

    fn draw_popup_computing<B>(&self, f: &mut Frame<B>, parent_layer: Rect)
    where
        B: Backend,
    {
        let layer = parent_layer;

        let message = Span::raw("Creating new passport... please wait...");
        let message = Paragraph::new(message)
            .block(Block::default())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false });

        f.render_widget(message, layer);
    }

    fn draw_popup_create_or_bind<B>(&self, f: &mut Frame<B>, parent_layer: Rect)
    where
        B: Backend,
    {
        let layer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(parent_layer);
        let message_layer = layer[0];
        let action_layer = layer[2];

        let message = Span::raw("It is not currently possible to bind a new key to a passport. You can create a passport though. The new feature will be added soon!");
        let message = Paragraph::new(message)
            .block(Block::default())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false });

        let action = Span::styled(
            "Press <Enter> for next step",
            Style::default()
                .bg(Color::LightYellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK),
        );
        let action = Paragraph::new(action)
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false });

        f.render_widget(message, message_layer);
        f.render_widget(action, action_layer);
    }

    fn draw_popup_new_passphrase<B>(
        &self,
        f: &mut Frame<B>,
        parent_layer: Rect,
        comment: impl AsRef<str>,
        passphrase: &str,
        confirming: bool,
        already_tried: bool,
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

        let input = if !confirming {
            Spans::from(vec![
                Span::raw("Enter new passphrase: "),
                Span::styled(passphrase, Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    "█",
                    Style::default()
                        .fg(Color::LightYellow)
                        .add_modifier(Modifier::SLOW_BLINK),
                ),
            ])
        } else {
            Spans::from(vec![
                Span::raw("Confirm new passphrase name: "),
                Span::styled(passphrase, Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    "█ ",
                    Style::default()
                        .fg(Color::LightYellow)
                        .add_modifier(Modifier::SLOW_BLINK),
                ),
                if already_tried {
                    Span::styled(
                        "Does not matches",
                        Style::default()
                            .fg(Color::LightRed)
                            .add_modifier(Modifier::ITALIC),
                    )
                } else {
                    Span::raw("")
                },
            ])
        };
        let input = Paragraph::new(input)
            .block(Block::default())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false });

        let action = if confirming {
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

        match &self.new_passport {
            None => {}
            Some(NewPassportState::Create { .. }) => {
                let popup_area = self.draw_popup_area(f, parent_layer);
                self.draw_popup_computing(f, popup_area);
            }
            Some(NewPassportState::CreateOrBind) => {
                let popup_area = self.draw_popup_area(f, parent_layer);
                self.draw_popup_create_or_bind(f, popup_area);
            }
            Some(NewPassportState::CreateNewPassphrase { passphrase }) => {
                let popup_area = self.draw_popup_area(f, parent_layer);
                let comment = "Please enter the shared key passphrase";
                self.draw_popup_new_passphrase(
                    f,
                    popup_area,
                    comment,
                    passphrase.as_str(),
                    false,
                    false,
                );
            }
            Some(NewPassportState::CreateConfirmPassphrase {
                confirmation,
                matches,
                ..
            }) => {
                let popup_area = self.draw_popup_area(f, parent_layer);
                let comment = "Please confirm the shared key passphrase";
                self.draw_popup_new_passphrase(
                    f,
                    popup_area,
                    comment,
                    confirmation.as_str(),
                    true,
                    matches.is_some(),
                );
            }
        }
    }
}
