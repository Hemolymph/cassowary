use iced::{
    Element, Length,
    alignment::Horizontal,
    widget::{button, column, container, row, text_input},
};

#[derive(Default)]
pub struct State {
    pub room_name_input: String,
}

#[derive(Clone, Debug)]
pub enum Message {
    JoinRoom,
    MakeRoom,
    ContentChanged(String),
}

pub fn view(state: &State) -> Element<Message> {
    let room_name = text_input("Thing", &state.room_name_input).on_input(Message::ContentChanged);
    let join_button = button("Join Room").on_press(Message::JoinRoom);
    let create_button = button("Create Room").on_press(Message::MakeRoom);
    let join_ui = row![room_name, join_button].spacing(10);

    let column = column![join_ui, create_button]
        .spacing(10)
        .align_x(Horizontal::Center);

    container(column)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .padding([200, 100])
        .into()
}
