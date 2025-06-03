use crate::{CARD_HEIGHT, HANDBAR_HEIGHT, widget::context_menu};
use iced::{
    Border, Element, Length,
    alignment::{Horizontal, Vertical},
    widget::{Row, Space, button, column, container, container::Style, pop, row},
};
use iced_drag::{DragAndDrop, drag, drop_zone};
use shared::{
    ClientMsg, DeckType, LocalState, NamedCardId, PlaceFrom, PlaceTo, RelSide, Space as BoardSpace,
    TurnStep,
};

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
    let sidebar = sidebar(state, resources, dragndrop);

    let other_timeline = timeline(RelSide::Other, state, resources, dragndrop);
    let game = {
        let same_board = board_row(RelSide::Same, state, resources, dragndrop);
        let other_board = board_row(RelSide::Other, state, resources, dragndrop);

        let board = container(column![other_board, same_board].spacing(10))
            .center_x(Length::Shrink)
            .center_y(Length::Shrink);

        let row = row![board]
            .height(Length::Fill)
            .width(Length::Shrink)
            .align_y(Vertical::Center);

        container(row).center_x(Length::Fill).center_y(Length::Fill)
    };
    let hand = {
        let row = container(
            row(state.game.hand.iter().enumerate().map(|(idx, card)| {
                let id = format!("hand_{idx}_{:#?}", card.id);
                drag(
                    id,
                    dragndrop,
                    card_image(ImageName::Name(card.name.clone()), resources),
                )
                .payload(PlaceFrom::Hand(card.id))
                .into()
            }))
            .spacing(3),
        )
        .style(|x| {
            let palette = x.extended_palette();

            Style {
                background: Some(palette.background.weakest.color.into()),
                border: Border {
                    width: 1.0,
                    radius: 0.0.into(),
                    color: palette.background.strong.color,
                },
                ..Default::default()
            }
        })
        .width(Length::Fill)
        .padding(3)
        .height(Length::Fixed(HANDBAR_HEIGHT));
        let mut hand = drop_zone(dragndrop, row).on_drop(|from| {
            Message::ToServer(ClientMsg::Move {
                from,
                to: PlaceTo::Hand,
            })
        });
        hand.width = Length::Fill;
        hand.height = Length::Fixed(HANDBAR_HEIGHT);
        hand
    };
    let self_timeline = timeline(RelSide::Same, state, resources, dragndrop);

    let game = column![self_timeline, game, other_timeline, hand].width(Length::Fill);

    let turn_bar = {
        let start_turn = button(image(
            resources.textures.get(&ImageName::StartTurnBtn).unwrap(),
        ))
        .on_press(Message::ToServer(ClientMsg::TurnSet(TurnStep::Start)))
        .padding(0)
        .width(32)
        .height(32);
        let main_phase = button(image(
            resources.textures.get(&ImageName::MainPhaseBtn).unwrap(),
        ))
        .on_press(Message::ToServer(ClientMsg::TurnSet(TurnStep::Main)))
        .padding(0)
        .width(32)
        .height(32);
        let attack_phase = button(image(
            resources.textures.get(&ImageName::AttackPhaseBtn).unwrap(),
        ))
        .on_press(Message::ToServer(ClientMsg::TurnSet(TurnStep::Combat)))
        .padding(0)
        .width(32)
        .height(32);
        let end_turn = button(image(
            resources.textures.get(&ImageName::MainPhaseBtn).unwrap(),
        ))
        .on_press(Message::ToServer(ClientMsg::TurnSet(TurnStep::End)))
        .padding(0)
        .width(32)
        .height(32);
        let switch_turn = button(image(
            resources.textures.get(&ImageName::SwitchTurnBtn).unwrap(),
        ))
        .on_press(Message::ToServer(ClientMsg::TurnSet(TurnStep::Switch)))
        .padding(0)
        .width(32)
        .height(32);

        column![start_turn, main_phase, attack_phase, end_turn, switch_turn]
            .align_x(Horizontal::Center)
    };

    row![turn_bar, sidebar, game]
        .align_y(Vertical::Center)
        .into()
}

fn sidebar<'a>(
    state: &State,
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

    column![
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
    .width(Length::Shrink)
    .into()
}

fn timeline<'a>(
    side: RelSide,
    state: &State,
    resources: &Resources,
    dragging: &'a DragAndDrop,
) -> Element<'a, Message> {
    let timeline = &state.game.get_player(side).timeline;
    let mut row = Row::new().height(HANDBAR_HEIGHT);
    for card in timeline {
        let id = format!("timeline:{:#?}", card.id);
        let image = match &card.name {
            shared::Hidden::Hidden => card_image(ImageName::CardBack, resources),
            shared::Hidden::Unhidden(name) => card_image(ImageName::Name(name.clone()), resources),
        };

        let draggable = drag(id, dragging, image).payload(PlaceFrom::Timeline(side, card.id));

        row = row.push(draggable);
    }

    let mut zone = drop_zone(dragging, row).on_drop(move |from| {
        Message::ToServer(ClientMsg::Move {
            from,
            to: PlaceTo::Timeline(side),
        })
    });
    zone.height = Length::Fixed(HANDBAR_HEIGHT);
    zone.width = Length::Fill;

    zone.into()
}
fn board_row<'a>(
    side: RelSide,
    state: &State,
    resources: &Resources,
    dragging: &'a DragAndDrop,
) -> Element<'a, Message> {
    let spaces = if side == RelSide::Same {
        [
            BoardSpace::First,
            BoardSpace::Second,
            BoardSpace::Third,
            BoardSpace::Fourth,
        ]
    } else {
        [
            BoardSpace::Fourth,
            BoardSpace::Third,
            BoardSpace::Second,
            BoardSpace::First,
        ]
    };

    let board_row = match side {
        RelSide::Same => &state.game.local_row,
        RelSide::Other => &state.game.distant_row,
    };

    let mut row = Row::new();
    for space in spaces {
        match &board_row[space] {
            Some(card) => {
                let from = PlaceFrom::Space(side, space);
                row = row.push({
                    let underlay = drag("row_{side:#?}_{space:#?}".to_owned(), dragging, {
                        match &card.name {
                            shared::Hidden::Hidden => card_image(ImageName::CardBack, resources),
                            shared::Hidden::Unhidden(card) => {
                                card_image(ImageName::Name(card.clone()), resources)
                            }
                        }
                    })
                    .payload(from);

                    context_menu(underlay, move || {
                        container(column![
                            row![
                                button("+").on_press(Message::ToServer(ClientMsg::AddCounter(
                                    from,
                                    "HP".to_string(),
                                    true
                                ))),
                                container("HP").center_x(Length::Fill),
                                button("-").on_press(Message::ToServer(ClientMsg::AddCounter(
                                    from,
                                    "HP".to_string(),
                                    false
                                )))
                            ]
                            .align_y(Vertical::Center),
                            row![
                                button("+").on_press(Message::ToServer(ClientMsg::AddCounter(
                                    from,
                                    "DEF".to_string(),
                                    true
                                ))),
                                container("DEF").center_x(Length::Fill),
                                button("-").on_press(Message::ToServer(ClientMsg::AddCounter(
                                    from,
                                    "DEF".to_string(),
                                    false
                                )))
                            ]
                            .align_y(Vertical::Center),
                            row![
                                button("+").on_press(Message::ToServer(ClientMsg::AddCounter(
                                    from,
                                    "POW".to_string(),
                                    true
                                ))),
                                container("POW").center_x(Length::Fill),
                                button("-").on_press(Message::ToServer(ClientMsg::AddCounter(
                                    from,
                                    "POW".to_string(),
                                    false
                                )))
                            ]
                            .align_y(Vertical::Center),
                        ])
                        .style(container::bordered_box)
                        .max_width(100)
                        .into()
                    })
                })
            }
            None => {
                row = row.push(
                    drop_zone(dragging, card_image(ImageName::CardBg, resources)).on_drop(
                        move |from| {
                            Message::ToServer(ClientMsg::Move {
                                from,
                                to: PlaceTo::Space(side, space, false),
                            })
                        },
                    ),
                );
            }
        }
    }
    let align_y = match side {
        RelSide::Same => Vertical::Top,
        RelSide::Other => Vertical::Bottom,
    };
    row.spacing(10)
        .align_y(align_y)
        .height(Length::FillPortion(1))
        .into()
}
