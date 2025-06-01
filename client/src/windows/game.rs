use crate::CARD_HEIGHT;
use iced::{
    Element, Length,
    alignment::Horizontal,
    widget::{Space, button, column, container, pop, row, text_input},
};
use shared::{DeckType, LocalState};

use crate::{CARD_WIDTH, ImageName, Resources, get_filegarden_link};

pub struct State {
    pub game: LocalState,
}

#[derive(Debug, Clone)]
pub enum Message {
    Unimplemented,
    LoadImage(String),
}

pub fn card_image<'a, 'b>(
    name: &'b str,
    state: &'a State,
    resources: &Resources,
) -> Element<'a, Message>
where
    'b: 'a,
{
    pop({
        let a = resources
            .textures
            .get(&ImageName::Name(name.to_string()))
            .unwrap_or(resources.textures.get(&ImageName::CardBg).unwrap());

        let img = iced::widget::image(a).width(CARD_WIDTH).height(CARD_HEIGHT);

        button(img).padding(0).on_press(Message::Unimplemented)
    })
    .on_show(|_| Message::LoadImage(name.to_string()))
    .into()
}

pub fn view<'a>(state: &'a State, resources: &Resources) -> Element<'a, Message> {
    let draw_main = card_image("BloodFlask", state, resources);
    let draw_blood = button("Draw Blood").on_press(Message::Unimplemented);
    let discard = button("Discard").on_press(Message::Unimplemented);
    let blood = "You're Blood";
    let bottom = column![blood, discard, draw_blood, draw_main].align_x(Horizontal::Center);

    let blood = "You'n't Blood";
    let opponent_discard = button("Discard").on_press(Message::Unimplemented);
    let top = column![opponent_discard, blood].align_x(Horizontal::Center);

    let sidebar = column![
        top,
        Space::with_height(Length::FillPortion(1)),
        "health",
        Space::with_height(Length::FillPortion(1)),
        bottom
    ]
    .align_x(Horizontal::Center)
    .height(Length::Fill)
    .width(Length::Shrink);

    let main = container("uwu")
        .center_x(Length::Fill)
        .center_y(Length::Fill);

    row![sidebar, main].into()
}
