use crate::CARD_HEIGHT;
use iced::{
    Element, Length,
    alignment::Horizontal,
    widget::{Row, Space, button, column, container, pop, row},
};
use iced_drag::{DragAndDrop, drag, drop_zone};
use shared::{ClientMsg, DeckType, LocalState, NamedCardId, PlaceFrom, PlaceTo, RelSide};

use crate::{CARD_WIDTH, ImageName, Resources};
use iced::widget::image;

pub struct State {
    pub game: LocalState,
    pub searching: Vec<NamedCardId>,
}

#[derive(Debug, Clone)]
pub enum Message {
    LoadImage(String),
    ToServer(ClientMsg),
}

pub fn card_image<'a, 'b>(name: ImageName, resources: &Resources) -> Element<'a, Message>
where
    'b: 'a,
{
    match name {
        ImageName::Name(name) => pop({
            let a = resources
                .textures
                .get(&ImageName::Name(name.clone()))
                .unwrap_or(resources.textures.get(&ImageName::CardBg).unwrap());

            image(a).width(CARD_WIDTH).height(CARD_HEIGHT)
        })
        .on_show(move |_| Message::LoadImage(name.clone()))
        .into(),
        _ => image(resources.get_texture(&name))
            .width(CARD_WIDTH)
            .height(CARD_HEIGHT)
            .into(),
    }
}

pub fn view<'a>(
    state: &'a State,
    resources: &Resources,
    dragndrop: &'a DragAndDrop,
) -> Element<'a, Message> {
    let draw_main = button(card_image(ImageName::CardBack, resources))
        .padding(0.0)
        .on_press(Message::ToServer(ClientMsg::Draw(
            RelSide::Same,
            DeckType::Main,
        )));
    let draw_blood = button(card_image(ImageName::BloodBack, resources))
        .padding(0.0)
        .on_press(Message::ToServer(ClientMsg::Draw(
            RelSide::Same,
            DeckType::Blood,
        )));
    let discard = {
        let drop_zone_content = {
            if state.game.local_state.discard.is_empty() {
                card_image(ImageName::CardBg, resources)
            } else {
                drag(
                    "self_discard".to_string(),
                    dragndrop,
                    card_image(
                        ImageName::Name(state.game.local_state.discard[0].name.clone()),
                        resources,
                    ),
                )
                .payload(PlaceFrom::Discard(
                    RelSide::Same,
                    state.game.local_state.discard[0].id,
                ))
                .into()
            }
        };
        let drop_zone = drop_zone(dragndrop, drop_zone_content).on_drop(|from| {
            println!("????");
            Message::ToServer(ClientMsg::Move {
                from,
                to: PlaceTo::Discard(RelSide::Same),
            })
        });
        drop_zone
    };
    let local_blood = "You're Blood";
    let bottom = column![local_blood, discard, draw_blood, draw_main]
        .spacing(2)
        .padding(2)
        .align_x(Horizontal::Center);

    let opponent_blood = "You'n't Blood";
    let opponent_discard = {
        let drop_zone_content = {
            if state.game.distant_state.discard.is_empty() {
                card_image(ImageName::CardBg, resources)
            } else {
                drag(
                    "other_discard".to_owned(),
                    dragndrop,
                    card_image(
                        ImageName::Name(state.game.distant_state.discard[0].name.clone()),
                        resources,
                    ),
                )
                .payload(PlaceFrom::Discard(
                    RelSide::Other,
                    state.game.distant_state.discard[0].id,
                ))
                .into()
            }
        };
        let drop_zone = drop_zone(dragndrop, drop_zone_content).on_drop(|from| {
            Message::ToServer(ClientMsg::Move {
                from,
                to: PlaceTo::Discard(RelSide::Other),
            })
        });
        drop_zone
    };
    let top = column![opponent_discard, opponent_blood]
        .padding(2)
        .spacing(2)
        .align_x(Horizontal::Center);

    let sidebar = column![
        top,
        Space::with_height(Length::FillPortion(1)),
        "health",
        Space::with_height(Length::FillPortion(1)),
        bottom
    ]
    .padding(2)
    .spacing(2)
    .align_x(Horizontal::Center)
    .height(Length::Fill)
    .width(Length::Shrink);

    let hand = {
        let row = row(state.game.hand.iter().enumerate().map(|(idx, card)| {
            let id = format!("hand_{idx}_{:#?}", card.id);
            drag(
                id,
                dragndrop,
                card_image(ImageName::Name(card.name.clone()), resources),
            )
            .payload(PlaceFrom::Hand(card.id))
            .into()
        }));
        row
    };

    let main = container(hand)
        .center_x(Length::Fill)
        .center_y(Length::Fill);

    row![sidebar, main].into()
}
