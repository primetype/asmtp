mod message;
mod screen;
mod state;
mod style;

use self::{
    message::{Event, Message},
    screen::Screens,
    state::State,
};
use iced::{executor, Application, Color, Command, Element};

pub struct App {
    state: State,

    screens: Screens,
}

impl Application for App {
    type Executor = executor::Default;
    type Message = Message;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Message>) {
        let state = State::default();
        let (screens, command) = Screens::new();
        (Self { state, screens }, command)
    }

    fn title(&self) -> String {
        format!("{} - ASMTP", self.screens.title())
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        if matches!(message.event(), Event::None) {
            Command::none()
        } else {
            self.screens.update(&mut self.state, message)
        }
    }

    fn view(&mut self) -> Element<'_, Self::Message> {
        self.screens.view(&mut self.state)
    }

    fn background_color(&self) -> Color {
        Color::BLACK
    }
}
