use crate::{
    screen::{id, state_loaded_route, Screen},
    style, Event, Message, State,
};
use anyhow::Context;
use iced::{button, Button, Color, Column, Command, Element, Row, Text};
use keynesis::Seed;

pub struct ChosePassportScreen {
    alias: String,
    seed: Seed,
    new_passport_submit: button::State,
    link_passport_submit: button::State,
}

impl ChosePassportScreen {
    pub fn new() -> Self {
        Self {
            alias: String::new(),
            seed: Seed::from([0; Seed::SIZE]),
            new_passport_submit: button::State::new(),
            link_passport_submit: button::State::new(),
        }
    }
}

impl Screen for ChosePassportScreen {
    fn title(&self) -> String {
        "Passport".to_owned()
    }

    fn update(&mut self, state: &mut State, event: Message) -> Command<Message> {
        if let Event::UpdateState(new_state) = event.event() {
            self.alias = new_state.alias().to_owned();
            Command::none()
        } else if let Event::Submit = event.event() {
            let mut state = state.clone();
            let seed = self.seed.clone();

            Command::perform(
                async move {
                    match tokio::task::spawn_blocking(|| {
                        state.create_passport(seed)?;
                        Ok(state)
                    })
                    .await
                    {
                        Ok(Ok(state)) => Ok(state),
                        Ok(Err(error)) => Err(error),
                        Err(error) => Err(error).context("Failed to await the passport creation"),
                    }
                },
                |r| match r {
                    Err(error) => Message::error(error),
                    Ok(state) => state_loaded_route(state),
                },
            )
        } else {
            dbg!(event);
            Command::none()
        }
    }

    fn view(&mut self, state: &mut State) -> Element<'_, Message> {
        let page = Column::new().padding(10).spacing(20);

        let description_row = Row::new().push(
            Text::new(format!(
                "So this is device \"{device}\" ({public_key}). \"{device}\" \
             is not linked to a passport yet. The passport is the collection \
             of all your keys. If you already have a passport, say so, \
             otherwise we will create a new one together",
                device = self.alias,
                public_key = state.public_key().unwrap()
            ))
            .color(Color::WHITE),
        );

        let choice = Row::new()
            .spacing(20)
            .push(
                Button::new(&mut self.new_passport_submit, Text::new("New Passport"))
                    .style(style::Button::Primary)
                    .on_press(Message::to(
                        id::PASSPORT_CHOICE,
                        id::PASSPORT_CHOICE,
                        Event::Submit,
                    )),
            )
            .push(
                Button::new(&mut self.link_passport_submit, Text::new("Link Passport"))
                    .style(style::Button::Secondary),
            );

        page.push(description_row).push(choice).into()
    }
}
