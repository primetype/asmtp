use crate::{message::Event, screen::Screen, Message, State};
use iced::{Color, Column, Command, Element, Text};
use std::sync::Arc;

pub struct ErrorScreen {
    error: Option<Arc<anyhow::Error>>,
}

impl ErrorScreen {
    pub fn new() -> Self {
        Self { error: None }
    }
}

impl Screen for ErrorScreen {
    fn title(&self) -> String {
        "error".to_owned()
    }

    fn update(&mut self, _state: &mut State, event: Message) -> Command<Message> {
        if !matches!(event.event(), Event::Error(_)) {
            let error = anyhow::anyhow!("Received an unexpected message event ({:?})", event)
                .context("Internal error");
            return Command::from(async { Message::error(error) });
        }

        if let Event::Error(error) = event.into_event() {
            self.error = Some(error);
            Command::none()
        } else {
            unreachable!()
        }
    }

    fn view(&mut self, _state: &mut State) -> Element<'_, Message> {
        let mut column = Column::new().spacing(20);

        if let Some(error) = self.error.take() {
            for error in error.chain() {
                column = column.push(Text::new(error.to_string()).color(Color::WHITE));
            }
        }

        column.into()
    }
}
