use crate::{app::App, event, ui::Focus};
use anyhow::{Context as _, Result};
use keynesis::{hash::Blake2b, passport::block::Hash, Seed};
use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub struct NewPassport {
    state: NewPassportState,
    next_step: Option<NewPassportState>,
}

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
    Created {
        hash: Hash,
    },
}

impl NewPassport {
    pub fn new() -> Self {
        let state = NewPassportState::CreateOrBind;
        let next_step = None;

        Self { state, next_step }
    }

    pub const fn title() -> &'static str {
        "widget::NewPassword"
    }

    pub fn has_focus(&self, focus: &Focus) -> bool {
        focus.check_current(Self::title())
    }

    pub fn input(&mut self, focus: &mut Focus, key: event::Key) -> bool {
        debug_assert!(self.has_focus(focus));

        self.next_step = match key {
            event::Key::Esc => match &self.state {
                NewPassportState::CreateOrBind => {
                    focus.pop();
                    return true;
                }
                NewPassportState::CreateNewPassphrase { .. } => {
                    Some(NewPassportState::CreateOrBind)
                }
                NewPassportState::CreateConfirmPassphrase { passphrase, .. } => {
                    Some(NewPassportState::CreateNewPassphrase {
                        passphrase: passphrase.clone(),
                    })
                }
                NewPassportState::Create { passphrase } => {
                    Some(NewPassportState::CreateConfirmPassphrase {
                        passphrase: passphrase.clone(),
                        confirmation: String::new(),
                        matches: None,
                    })
                }
                NewPassportState::Created { .. } => {
                    focus.pop();
                    return true;
                }
            },
            event::Key::Enter => match &self.state {
                NewPassportState::CreateOrBind => Some(NewPassportState::CreateNewPassphrase {
                    passphrase: String::new(),
                }),
                NewPassportState::CreateNewPassphrase { passphrase } => {
                    Some(NewPassportState::CreateConfirmPassphrase {
                        passphrase: passphrase.clone(),
                        confirmation: String::new(),
                        matches: None,
                    })
                }
                NewPassportState::CreateConfirmPassphrase {
                    passphrase,
                    confirmation,
                    ..
                } => {
                    if passphrase == confirmation {
                        Some(NewPassportState::Create {
                            passphrase: passphrase.clone(),
                        })
                    } else {
                        Some(NewPassportState::CreateConfirmPassphrase {
                            passphrase: passphrase.clone(),
                            confirmation: confirmation.clone(),
                            matches: Some(false),
                        })
                    }
                }
                NewPassportState::Create { .. } => None,
                NewPassportState::Created { .. } => {
                    focus.pop();
                    return true;
                }
            },
            event::Key::Backspace => {
                match &mut self.state {
                    NewPassportState::CreateNewPassphrase { passphrase } => {
                        passphrase.pop();
                    }
                    NewPassportState::CreateConfirmPassphrase {
                        confirmation,
                        matches,
                        ..
                    } => {
                        if confirmation.pop().is_some() {
                            matches.take();
                        }
                    }
                    NewPassportState::CreateOrBind => {}
                    NewPassportState::Create { .. } => {}
                    NewPassportState::Created { .. } => {}
                }
                None
            }
            event::Key::Char(c) => {
                match &mut self.state {
                    NewPassportState::CreateNewPassphrase { passphrase } => {
                        passphrase.push(c);
                    }
                    NewPassportState::CreateConfirmPassphrase {
                        confirmation,
                        matches,
                        ..
                    } => {
                        confirmation.push(c);
                        matches.take();
                    }
                    NewPassportState::CreateOrBind => {}
                    NewPassportState::Create { .. } => {}
                    NewPassportState::Created { .. } => {}
                }
                None
            }
            _ => None,
        };

        false
    }

    pub async fn update(&mut self, app: &mut App) -> Result<()> {
        if let Some(next) = self.next_step.take() {
            self.state = next;
        }

        if let NewPassportState::Create { passphrase } = &self.state {
            let seed = {
                let mut key = [0; 32];
                Blake2b::blake2b(&mut key, passphrase.as_bytes(), &[]);
                Seed::derive_from_key(&key, &[])
            };
            let hash = app
                .create_new_passport(seed)
                .await
                .context("Failed to create new passport")?;

            self.state = NewPassportState::Created { hash };
        }
        Ok(())
    }

    fn clear_and_draw<B>(&self, f: &mut Frame<B>, area: Rect) -> Rect
    where
        B: Backend,
    {
        let block = Block::default().title("New passport").borders(Borders::ALL);

        let inner = block.inner(area);

        // clear the area under the popup
        f.render_widget(Clear, area);
        f.render_widget(block, area);

        inner
    }

    pub fn draw<B>(&self, _focus: &Focus, f: &mut Frame<B>, parent_layer: Rect)
    where
        B: Backend,
    {
        let area = self.clear_and_draw(f, parent_layer);

        match &self.state {
            NewPassportState::CreateOrBind => {
                draw_create_or_bind(f, area);
            }
            NewPassportState::CreateNewPassphrase { passphrase } => {
                draw_input(
                    f,
                    area,
                    passphrase.as_str(),
                    "Please enter the passphrase that will be used to initialized the your passport",
                    "passphrase: ",
                    None,
                );
            }
            NewPassportState::CreateConfirmPassphrase {
                confirmation,
                matches,
                ..
            } => {
                let failure = if matches.unwrap_or_default() {
                    None
                } else {
                    Some("Passphrase does not match")
                };
                draw_input(
                    f,
                    area,
                    confirmation.as_str(),
                    "Confirm the passphrase",
                    "passphrase: ",
                    failure,
                );
            }
            NewPassportState::Create { .. } => {
                draw_create(f, area);
            }
            NewPassportState::Created { hash } => {
                draw_created(f, area, hash);
            }
        }
    }
}

fn draw_create_or_bind<B>(f: &mut Frame<B>, area: Rect)
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
        .split(area);
    let message_layer = layer[0];
    let action_layer = layer[2];

    let message = Span::raw("It is not currently possible to bind a new key to a passport. You can create a passport though. The new feature will be added soon!");
    let message = Paragraph::new(message)
        .block(Block::default())
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });

    let action = Span::styled(
        "Next (<Enter>) or Previous (<Esc>)",
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

fn draw_input<B>(
    f: &mut Frame<B>,
    area: Rect,
    input: &str,
    comment: &str,
    prompt: &str,
    failure: Option<&str>,
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
        .split(area);
    let message_layer = layer[0];
    let input_layer = layer[2];
    let action_layer = layer[4];

    let message = Span::raw(comment);
    let message = Paragraph::new(message)
        .block(Block::default())
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });

    let input = Spans::from(vec![
        Span::raw(prompt),
        Span::styled(input, Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            "█ ",
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::SLOW_BLINK),
        ),
        if let Some(failure) = failure {
            Span::styled(
                failure,
                Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::ITALIC),
            )
        } else {
            Span::raw("")
        },
    ]);
    let input = Paragraph::new(input)
        .block(Block::default())
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });

    let action = Span::raw("Next (<Enter>) or Previous (<Esc>)");
    let action = Paragraph::new(action)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false });

    f.render_widget(message, message_layer);
    f.render_widget(input, input_layer);
    f.render_widget(action, action_layer);
}

fn draw_create<B>(f: &mut Frame<B>, area: Rect)
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
        .split(area);
    let message_layer = layer[0];
    let action_layer = layer[2];

    let message = Span::raw("Creating new passport... please wait...");
    let message = Paragraph::new(message)
        .block(Block::default())
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });

    let action = Span::raw("Next (<Enter>) or Previous (<Esc>)");
    let action = Paragraph::new(action)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false });

    f.render_widget(message, message_layer);
    f.render_widget(action, action_layer);
}

fn draw_created<B>(f: &mut Frame<B>, area: Rect, hash: &Hash)
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
        .split(area);
    let message_layer = layer[0];
    let action_layer = layer[2];

    let message = Span::raw(format!("Passport created: {}", hash));
    let message = Paragraph::new(message)
        .block(Block::default())
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });

    let action = Span::raw("Finish (<Enter> or <Esc>)");
    let action = Paragraph::new(action)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false });

    f.render_widget(message, message_layer);
    f.render_widget(action, action_layer);
}
