use crate::{screen::id, state::State};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Message {
    to_screen: usize,
    display_screen: usize,
    event: Event,
}

#[derive(Debug, Clone)]
pub enum Event {
    None,

    Error(Arc<anyhow::Error>),
    UpdateState(State),
    Submit,
    Choice(usize),

    SetAliasUpdate(String),
}

impl Message {
    pub fn to(display_screen: usize, to_screen: usize, event: Event) -> Self {
        Self {
            display_screen,
            to_screen,
            event,
        }
    }

    pub fn none() -> Self {
        Self::to(0, 0, Event::None)
    }

    pub fn error(error: anyhow::Error) -> Self {
        Self::to(
            id::ERROR_SCREEN,
            id::ERROR_SCREEN,
            Event::Error(Arc::new(error)),
        )
    }

    pub fn event(&self) -> &Event {
        &self.event
    }

    pub fn into_event(self) -> Event {
        self.event
    }

    pub fn to_screen(&self) -> usize {
        self.to_screen
    }

    pub fn display_screen(&self) -> usize {
        self.display_screen
    }
}
