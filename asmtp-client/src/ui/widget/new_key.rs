use crate::{app::App, event, ui::Focus};
use anyhow::{Context as _, Result};
use keynesis::key::ed25519::PublicKey;
use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub struct NewKey {
    state: NewKeyState,
    next_step: Option<NewKeyState>,
}

#[derive(Clone)]
enum NewKeyState {
    EnteringName { name: String },
    Confirm { name: String },
    Create { name: String },
    Created { name: String, key: PublicKey },
}

impl NewKey {
    pub fn new() -> Self {
        let state = NewKeyState::EnteringName {
            name: String::new(),
        };
        let next_step = None;

        Self { state, next_step }
    }

    pub const fn title() -> &'static str {
        "widget::NewKey"
    }

    pub fn has_focus(&self, focus: &Focus) -> bool {
        focus.check_current(Self::title())
    }

    pub fn input(&mut self, focus: &mut Focus, key: event::Key) -> bool {
        debug_assert!(self.has_focus(focus));

        self.next_step = match key {
            event::Key::Enter => match &self.state {
                NewKeyState::EnteringName { name } => {
                    Some(NewKeyState::Confirm { name: name.clone() })
                }
                NewKeyState::Confirm { name } => Some(NewKeyState::Create { name: name.clone() }),
                NewKeyState::Create { .. } => None,
                NewKeyState::Created { .. } => {
                    focus.pop();
                    return true;
                }
            },
            event::Key::Esc => match &self.state {
                NewKeyState::EnteringName { .. } => {
                    focus.pop();
                    return true;
                }
                NewKeyState::Confirm { name } => {
                    Some(NewKeyState::EnteringName { name: name.clone() })
                }
                NewKeyState::Create { name } => Some(NewKeyState::Confirm { name: name.clone() }),
                NewKeyState::Created { .. } => {
                    focus.pop();
                    return true;
                }
            },
            event::Key::Backspace => match &mut self.state {
                NewKeyState::EnteringName { name } => {
                    name.pop();
                    None
                }
                NewKeyState::Confirm { .. } => None,
                NewKeyState::Create { .. } => None,
                NewKeyState::Created { .. } => None,
            },
            event::Key::Char(c) => match &mut self.state {
                NewKeyState::EnteringName { name } => {
                    if name.len() <= 32 {
                        name.push(c);
                    }
                    None
                }
                NewKeyState::Confirm { .. } => None,
                NewKeyState::Create { .. } => None,
                NewKeyState::Created { .. } => None,
            },
            _ => None,
        };

        false
    }

    pub async fn update(&mut self, app: &mut App) -> Result<()> {
        if let Some(next) = self.next_step.take() {
            self.state = next;
        }

        if let NewKeyState::Create { name } = &self.state {
            let key = app
                .create_new_key(&name)
                .await
                .context("Failed to create new key")?;

            self.state = NewKeyState::Created {
                name: name.clone(),
                key,
            };
        }
        Ok(())
    }

    fn clear_and_draw<B>(&self, f: &mut Frame<B>, area: Rect) -> Rect
    where
        B: Backend,
    {
        let block = Block::default().title("New Key").borders(Borders::ALL);

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
            NewKeyState::EnteringName { name } => {
                draw_enter_device_name(f, area, name.as_str());
            }
            NewKeyState::Confirm { name } => {
                draw_confirm_device_name(f, area, name.as_str());
            }
            NewKeyState::Create { name } => {
                draw_creating(f, area, name.as_str());
            }
            NewKeyState::Created { name, key } => {
                draw_created(f, area, name.as_str(), key);
            }
        }
    }
}

fn draw_enter_device_name<B>(f: &mut Frame<B>, area: Rect, name: &str)
where
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

    let message = Span::raw("Enter the alias to give to this key");
    let message = Paragraph::new(message)
        .block(Block::default())
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });

    let input = Spans::from(vec![
        Span::raw("key alias: "),
        Span::styled(name, Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            "█ ",
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::SLOW_BLINK),
        ),
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

fn draw_confirm_device_name<B>(f: &mut Frame<B>, area: Rect, name: &str)
where
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

    let message = Span::raw("Confirm the alias to give to this key");
    let message = Paragraph::new(message)
        .block(Block::default())
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });

    let input = Spans::from(vec![
        Span::raw("key alias: "),
        Span::styled(name, Style::default().add_modifier(Modifier::BOLD)),
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

fn draw_creating<B>(f: &mut Frame<B>, area: Rect, name: &str)
where
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

    let message = Span::raw("The new key is being created...");
    let message = Paragraph::new(message)
        .block(Block::default())
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });

    let input = Spans::from(vec![
        Span::raw("key alias: "),
        Span::styled(name, Style::default().add_modifier(Modifier::BOLD)),
    ]);
    let input = Paragraph::new(input)
        .block(Block::default())
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });

    let action = Block::default().borders(Borders::ALL);

    f.render_widget(message, message_layer);
    f.render_widget(input, input_layer);
    f.render_widget(action, action_layer);
}

fn draw_created<B>(f: &mut Frame<B>, area: Rect, name: &str, key: &PublicKey)
where
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

    let message = Span::raw("The new key has been created");
    let message = Paragraph::new(message)
        .block(Block::default())
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });

    let input = Spans::from(vec![
        Span::raw("key alias: "),
        Span::styled(name, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("\nkey: "),
        Span::styled(
            key.to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]);
    let input = Paragraph::new(input)
        .block(Block::default())
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });

    let action = Span::raw("Finish (<Enter> or <Esc>)");
    let action = Paragraph::new(action)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false });

    f.render_widget(message, message_layer);
    f.render_widget(input, input_layer);
    f.render_widget(action, action_layer);
}
