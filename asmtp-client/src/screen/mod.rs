mod error;
pub mod id;
mod new;
mod splash;

use crate::{Event, Message, State};
use anyhow::Context as _;
use iced::{Color, Column, Command, Element, Text};

pub trait Screen {
    fn title(&self) -> String;

    fn update(&mut self, state: &mut State, event: Message) -> Command<Message>;

    fn view(&mut self, state: &mut State) -> Element<'_, Message>;
}

pub struct Screens {
    pages: Vec<Box<dyn Screen>>,
    current: usize,
}

impl Screens {
    pub fn new() -> (Self, Command<Message>) {
        let screens = Self {
            pages: vec![
                // Splash screen, used at the beginning while setting
                // up the different resources
                Box::new(splash::SplashScreen::new()),
                Box::new(error::ErrorScreen::new()),
                Box::new(new::SetDeviceScreen::new()),
                Box::new(new::ChosePassportScreen::new()),
            ],
            current: id::SPLASH_SCREEN,
        };

        let command = Command::perform(
            async {
                // this is a blocking task
                match tokio::task::spawn_blocking(|| State::load("asmtp.json")).await {
                    Ok(Err(error)) => Err(error),
                    Err(error) => Err(error).context("Failed to await loading the state"),
                    Ok(Ok(state)) => Ok(state),
                }
            },
            |result| match result {
                Err(error) => Message::error(error),
                Ok(state) => state_loaded_route(state),
            },
        );

        (screens, command)
    }

    pub fn title(&self) -> String {
        self.pages[self.current].title()
    }

    pub fn update(&mut self, state: &mut State, event: Message) -> Command<Message> {
        let mut batch = Vec::new();
        if let Event::UpdateState(new_state) = event.event() {
            *state = new_state.clone();
            let new_state = new_state.clone();

            let cmd = Command::perform(
                async move {
                    match tokio::task::spawn_blocking(move || new_state.save("asmtp.json")).await {
                        Ok(Err(error)) => Err(error),
                        Err(error) => Err(error).context("Failed to await saving the state"),
                        Ok(Ok(state)) => Ok(state),
                    }
                },
                |result| match result {
                    Err(error) => Message::error(error),
                    Ok(()) => Message::none(),
                },
            );

            batch.push(cmd);
        }

        let to = event.to_screen();
        self.current = event.display_screen();
        batch.push(self.pages[to].update(state, event));

        Command::batch(batch)
    }

    pub fn view(&mut self, state: &mut State) -> Element<'_, Message> {
        let title = self.pages[self.current].title();
        let inner = self.pages[self.current].view(state);

        Column::new()
            .spacing(20)
            .padding(20)
            .push(Text::new(title).color(Color::WHITE).size(50))
            .push(inner)
            .into()
    }
}

fn state_loaded_route(state: State) -> Message {
    let to = if !state.has_alias() || !state.has_key() {
        id::SET_DEVICE
    } else if !state.has_passport() {
        id::PASSPORT_CHOICE
    } else {
        todo!("successfully loaded page")
    };

    Message::to(to, to, Event::UpdateState(state))
}
