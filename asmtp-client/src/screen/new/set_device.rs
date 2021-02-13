use crate::{
    screen::{id, state_loaded_route, Screen},
    style, Event, Message, State,
};
use iced::{button, text_input, Button, Color, Column, Command, Element, Row, Text, TextInput};

pub struct SetDeviceScreen {
    state: text_input::State,
    submit: button::State,
    alias: String,
}

impl SetDeviceScreen {
    pub fn new() -> Self {
        Self {
            state: text_input::State::new(),
            alias: String::new(),
            submit: button::State::new(),
        }
    }
}

impl Screen for SetDeviceScreen {
    fn title(&self) -> String {
        "Device Setup".to_owned()
    }

    fn update(&mut self, state: &mut State, event: Message) -> Command<Message> {
        if let Event::UpdateState(new_state) = event.event() {
            self.alias = new_state.alias().to_owned();
            Command::none()
        } else if let Event::SetAliasUpdate(new_alias) = event.event() {
            self.alias = new_alias.clone();
            Command::none()
        } else if let Event::Submit = event.event() {
            let mut state = state.clone();
            state.set_alias(&self.alias);
            let message = state_loaded_route(state);

            Command::from(async { message })
        } else {
            dbg!(event);
            Command::none()
        }
    }

    fn view(&mut self, _state: &mut State) -> Element<'_, Message> {
        let page = Column::new();

        let description_row = Text::new("Hi, it seems this is the first time you are login on your ASMTP client app. We will walk you through the setup. We are creating a device alias and an associated key. The Alias and the keys are associated to this device and this device only").color(Color::WHITE);

        let alias_row = Row::new()
            .push(Text::new("new device: ").color(Color::WHITE))
            .push(TextInput::new(
                &mut self.state,
                "alias...",
                &self.alias,
                |alias| Message::to(id::SET_DEVICE, id::SET_DEVICE, Event::SetAliasUpdate(alias)),
            ))
            .push(
                Button::new(&mut self.submit, Text::new("Save"))
                    .on_press(Message::to(id::SET_DEVICE, id::SET_DEVICE, Event::Submit))
                    .style(style::Button::Primary),
            );

        page.push(description_row).push(alias_row).into()
    }
}
