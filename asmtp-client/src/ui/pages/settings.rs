use crate::{app::App, event, ui::Focus};
use anyhow::Result;
use asmtp_network::Version as NetworkVersion;
use std::time::{Duration, Instant};
use structopt::clap::crate_version;
use tui::{
    backend::Backend,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table, TableState},
    Frame,
};

pub struct Settings {
    started: Instant,

    cursor: usize,
    items: Vec<Row<'static>>,
}

impl Settings {
    pub fn new(_app: &App) -> Self {
        let started = Instant::now();
        let cursor = 0;

        let items = Vec::new();

        Self {
            started,
            cursor,
            items,
        }
    }

    pub const fn title() -> &'static str {
        "Settings"
    }

    fn has_focus(&self, focus: &Focus) -> bool {
        focus.check_current(Self::title())
    }

    pub fn input(&mut self, focus: &mut Focus, key: event::Key) {
        match key {
            event::Key::Esc => {
                assert!(self.has_focus(focus));
                focus.pop();
            }
            event::Key::Up => {
                self.cursor = if let Some(a) = self.cursor.checked_sub(1) {
                    a
                } else {
                    self.items.len().wrapping_sub(1)
                };
            }
            event::Key::Down => {
                self.cursor = self.cursor.wrapping_add(1).wrapping_rem(self.items.len());
            }
            _ => {}
        }
    }

    pub fn update(&mut self, app: &mut App) -> Result<()> {
        self.items.clear();

        let network_version = format!(
            "{current} ({min} <= {current} <= {max})",
            current = NetworkVersion::CURRENT,
            min = NetworkVersion::MIN,
            max = NetworkVersion::MAX,
        );

        self.items.push(Row::new(vec![
            "started since".to_owned(),
            format_duration_since(self.started),
        ]));
        self.items
            .push(Row::new(vec!["software version", crate_version!()]));
        self.items.push(Row::new(vec![
            "network version".to_owned(),
            network_version,
        ]));

        if let Some(stats) = app.network.stats() {
            if let Ok(stats) = stats.lock() {
                self.items.push(Row::new(vec![
                    "Current key".to_string(),
                    stats.current_id.to_string(),
                ]));
                self.items.push(Row::new(vec![
                    "Remote Peer Id".to_string(),
                    stats.peer_id.to_string(),
                ]));
                self.items.push(Row::new(vec![
                    "Remote Peer Address".to_string(),
                    stats.peer_address.to_string(),
                ]));
                self.items.push(Row::new(vec![
                    "Network Session Id".to_string(),
                    stats.session_id.to_string(),
                ]));
                self.items.push(Row::new(vec![
                    "Network Connection Time".to_string(),
                    format_duration_since(stats.connection_established_since),
                ]));
                self.items.push(Row::new(vec![
                    "Network received messages".to_string(),
                    format!(
                        "{} ({})",
                        stats.number_message_received,
                        format_duration_since(stats.last_message_received)
                    ),
                ]));
                self.items.push(Row::new(vec![
                    "Network sent messages".to_string(),
                    format!(
                        "{} ({})",
                        stats.number_message_sent,
                        format_duration_since(stats.last_message_sent)
                    ),
                ]));

                if let Some(error) = stats.error.as_ref() {
                    let duration = stats
                        .last_error_received
                        .map(format_duration_since)
                        .unwrap_or_default();
                    let mut chain = error.chain();
                    if let Some(error) = chain.next() {
                        self.items.push(Row::new(vec![
                            "Network error".to_string(),
                            format!("{} {}", error, duration),
                        ]));
                    }
                    for error in chain {
                        self.items
                            .push(Row::new(vec![String::new(), error.to_string()]));
                    }
                }
            }
        }
        if let Some(error) = app.network.connection_failure() {
            let mut chain = error.chain();
            if let Some(error) = chain.next() {
                self.items.push(Row::new(vec![
                    "Connection failed".to_string(),
                    error.to_string(),
                ]));
            }
            for error in chain {
                self.items
                    .push(Row::new(vec![String::new(), error.to_string()]));
            }
        }

        Ok(())
    }

    pub fn draw<B>(&self, focus: &Focus, f: &mut Frame<B>, parent_layer: Rect)
    where
        B: Backend,
    {
        let block = Block::default().title("Settings").borders(Borders::ALL);
        let mut state = TableState::default();
        let selected_style = if self.has_focus(focus) {
            state.select(Some(self.cursor));
            Style::default()
                .bg(Color::LightYellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        };

        let table = Table::new(self.items.clone())
            .widths(&[Constraint::Ratio(1, 3), Constraint::Ratio(2, 3)])
            .highlight_style(selected_style)
            .block(block);
        f.render_stateful_widget(table, parent_layer, &mut state);
    }
}

fn format_duration_since(since: Instant) -> String {
    format!("{:?} ago", Duration::from_secs(since.elapsed().as_secs()))
}
