use crate::{app::App, event, ui::Focus};
use anyhow::Result;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders},
    Frame,
};

pub struct Messages {}

impl Messages {
    pub fn new(app: &App) -> Self {
        Self {}
    }

    pub const fn title() -> &'static str {
        "Messages"
    }

    fn has_focus(&self, focus: &Focus) -> bool {
        focus.check_current(Self::title())
    }

    pub fn input(&mut self, focus: &mut Focus, key: event::Key) {
        //
    }

    pub async fn update(&mut self, app: &mut App) -> Result<()> {
        if let Some(current_passport) = app.get_current_passport() {
            // select all the shared public keys of the passport
            // then select all the topics associated to any of them
        }

        Ok(())
    }

    pub fn draw<B>(&self, focus: &Focus, f: &mut Frame<B>, parent_layer: Rect)
    where
        B: Backend,
    {
        let layers = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(1, 5),
                Constraint::Ratio(1, 5),
                Constraint::Ratio(3, 5),
            ])
            .split(parent_layer);
        let topics_area = layers[0];
        let messages_area = layers[1];
        let message_area = layers[2];

        let block = Block::default().borders(Borders::ALL);

        f.render_widget(block.clone(), topics_area);
        f.render_widget(block.clone(), messages_area);
        f.render_widget(block, message_area);
    }
}
