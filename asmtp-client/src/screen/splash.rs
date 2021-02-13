use crate::{screen::Screen, Message, State};
use iced::{Color, Command, Element, Text};

pub struct SplashScreen {}

impl SplashScreen {
    pub fn new() -> Self {
        Self {}
    }
}

impl Screen for SplashScreen {
    fn title(&self) -> String {
        "loading".to_owned()
    }

    fn update(&mut self, _state: &mut State, _event: Message) -> Command<Message> {
        Command::none()
    }

    fn view(&mut self, _state: &mut State) -> Element<'_, Message> {
        Text::new("loading").color(Color::WHITE).into()
    }
}
