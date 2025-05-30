use std::collections::VecDeque;

use egui_macroquad::egui::{
    self, Context, CursorIcon, DragAndDrop, Frame, Id, ImageButton, InnerResponse, LayerId, Layout,
    Order, Response, Sense, UiBuilder, Vec2, Widget, emath::TSTransform,
};
use macroquad::input::{KeyCode, is_key_down};
use shared::{
    ClientMsg, DeckType, Hidden, LocalCard, LocalState, NamedCardId, PlaceFrom, RelSide, Space,
};
use shrek_deck::parser::parse_line;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    BloodlessCard, CARD_HEIGHT, CARD_WIDTH, HANDBAR_HEIGHT, ImageName, SIDEBAR_WIDTH, TEXTURES,
};

#[derive(Debug, Clone)]
pub struct GameData {
    pub state: LocalState,
    pub editing_deck: bool,
    pub deck: DeckType,
    pub marrow_main: String,
    pub marrow_blood: String,
    pub marrow_error: String,
    pub seaching: Vec<NamedCardId>,
}

pub async fn draw_game(to_server: &UnboundedSender<ClientMsg>, data: &mut GameData) {
    egui_macroquad::ui(|ctx| {
        egui::TopBottomPanel::top("topbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("Deck", |ui| {
                    let blood_deck = ui.button("Blood Deck");
                    let main_deck = ui.button("Main Deck");

                    if blood_deck.clicked() {
                        data.editing_deck = true;
                        data.deck = DeckType::Blood;
                        data.marrow_error = String::new();
                    }
                    if main_deck.clicked() {
                        data.editing_deck = true;
                        data.deck = DeckType::Main;
                        data.marrow_error = String::new();
                    }
                });
            });
        });
        sidebar(ctx, to_server, data);
        handbar(ctx, to_server, data);
        timeline(ctx, RelSide::Same, data, to_server);
        timeline(ctx, RelSide::Other, data, to_server);
        middle(ctx, to_server, data);

        if data.editing_deck {
            egui::Window::new("Deck Editor")
                .resizable(true)
                .constrain(false)
                .open(&mut data.editing_deck)
                .show(ctx, |ui| {
                    ui.label("Editing your deck");
                    let marrow = match data.deck {
                        DeckType::Blood => &mut data.marrow_blood,
                        DeckType::Main => &mut data.marrow_main,
                    };
                    ui.code_editor(marrow);
                    ui.label(data.marrow_error.clone());
                    if ui.button("Done!").clicked() {
                        let mut deck = vec![];
                        for line in marrow.lines() {
                            if line.is_empty() {
                                continue;
                            }

                            match parse_line::<BloodlessCard>(line) {
                                Ok(a) => {
                                    TEXTURES.write().set_texture(a.card.name.clone(), ui.ctx());
                                    for _ in 0..a.amount {
                                        deck.push(a.card.name.clone());
                                    }
                                }
                                Err(_) => {
                                    data.marrow_error = "Error parsing Marrow syntax".to_string()
                                }
                            }
                        }
                        if data.marrow_error.is_empty() {
                            to_server
                                .send(ClientMsg::SetDeck(data.deck, VecDeque::from(deck)))
                                .unwrap()
                        }
                    }
                });
        }

        if !data.seaching.is_empty() {
            egui::Window::new("Searching...")
                .resizable(true)
                .scroll([false, true])
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("cardsearch").show(ui, |ui| {
                            for (idx, card) in data.seaching.iter().enumerate() {
                                let id = format!("searching_{:?}", card.id).into();
                                let zone = PlaceFrom::Deck(RelSide::Same, DeckType::Main, card.id);
                                drag(ui, id, zone, |ui| {
                                    ui.add(CardDisplay::new(card.clone(), to_server).at_zone(zone))
                                });
                                if idx % 8 == 7 {
                                    ui.end_row();
                                }
                            }
                        });
                    });
                    if ui.button("Done").clicked() {
                        data.seaching = vec![];
                        to_server.send(ClientMsg::FinishSearch).unwrap();
                    };
                });
        }
    });
}

struct CardDisplay<'a> {
    card: LocalCard,
    location: Option<PlaceFrom>,
    sender: &'a UnboundedSender<ClientMsg>,
}

impl<'a> CardDisplay<'a> {
    fn new<S: Into<LocalCard>>(name: S, sender: &'a UnboundedSender<ClientMsg>) -> Self {
        Self {
            card: name.into(),
            location: None,
            sender,
        }
    }

    fn at_zone(self, zone: PlaceFrom) -> Self {
        Self {
            card: self.card,
            location: Some(zone),
            ..self
        }
    }
}

impl Widget for CardDisplay<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(CARD_WIDTH, CARD_HEIGHT), Sense::click());

        let image = {
            let a = TEXTURES.read();
            if let Hidden::Unhidden(name) = self.card.name {
                a.get_texture(ImageName::Name(name.clone())).clone()
            } else {
                a.get_texture(ImageName::CardBack).clone()
            }
        };

        let response = response.on_hover_ui_at_pointer(|ui| {
            ui.image(image.clone());
        });

        response.context_menu(|ui| {
            let Some(location) = self.location else {
                return;
            };
            if matches!(location, PlaceFrom::Space(..)) {
                ui.label("Stats");
                ui.horizontal(|ui| {
                    ui.add(CounterButton {
                        counter: "HP".to_owned(),
                        to_server: self.sender,
                        current: self.card.counters.get("HP").cloned().unwrap_or_default(),
                        from: location,
                    })
                });
                ui.horizontal(|ui| {
                    ui.add(CounterButton {
                        counter: "DEF".to_owned(),
                        to_server: self.sender,
                        current: self.card.counters.get("DEF").cloned().unwrap_or_default(),
                        from: location,
                    })
                });
                ui.add(CounterButton {
                    counter: "POW".to_owned(),
                    to_server: self.sender,
                    current: self.card.counters.get("POW").cloned().unwrap_or_default(),
                    from: location,
                });
            }
            if !matches!(
                location,
                PlaceFrom::Hand(..)
                    | PlaceFrom::Discard(..)
                    | PlaceFrom::Deck(..)
                    | PlaceFrom::Aside(..)
            ) {
                ui.label("Counters");
                ui.add(CounterButton {
                    counter: "RED".to_owned(),
                    to_server: self.sender,
                    current: self.card.counters.get("RED").cloned().unwrap_or_default(),
                    from: location,
                });
                ui.add(CounterButton {
                    counter: "GRE".to_owned(),
                    to_server: self.sender,
                    current: self.card.counters.get("GRE").cloned().unwrap_or_default(),
                    from: location,
                });
                ui.add(CounterButton {
                    counter: "BLU".to_owned(),
                    to_server: self.sender,
                    current: self.card.counters.get("BLU").cloned().unwrap_or_default(),
                    from: location,
                });
                ui.add(CounterButton {
                    counter: "BLA".to_owned(),
                    to_server: self.sender,
                    current: self.card.counters.get("BLA").cloned().unwrap_or_default(),
                    from: location,
                });
            }
        });

        egui::Image::new(image.clone()).paint_at(ui, rect);
        response
    }
}

struct CounterButton<'a> {
    counter: String,
    to_server: &'a UnboundedSender<ClientMsg>,
    current: usize,
    from: PlaceFrom,
}

impl Widget for CounterButton<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        ui.horizontal(|ui| {
            let add = ui.button("+");
            let rem = ui.button("-");
            ui.label(format!("{}: {}", self.counter.to_uppercase(), self.current));

            if add.clicked() {
                self.to_server
                    .send(ClientMsg::AddCounter(self.from, self.counter.clone(), true))
                    .unwrap();
            }

            if rem.clicked() {
                self.to_server
                    .send(ClientMsg::AddCounter(self.from, self.counter, false))
                    .unwrap();
            }
        })
        .response
    }
}

fn timeline(
    ctx: &Context,
    side: RelSide,
    data: &mut GameData,
    to_server: &UnboundedSender<ClientMsg>,
) {
    let layout = match side {
        RelSide::Same => Layout::left_to_right(egui::Align::Center),
        RelSide::Other => Layout::right_to_left(egui::Align::Center),
    };

    let place = match side {
        RelSide::Same => egui::TopBottomPanel::bottom("self_timeline"),
        RelSide::Other => egui::TopBottomPanel::top("other_timeline"),
    };

    place.show(ctx, |ui| {
        let (_, drop) = ui.dnd_drop_zone::<PlaceFrom, _>(Frame::new(), |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(layout, |ui| {
                    ui.set_min_height(CARD_HEIGHT);
                    for (idx, card) in data
                        .state
                        .get_player(side)
                        .timeline
                        .clone()
                        .into_iter()
                        .enumerate()
                    {
                        let id = format!("timeline_{side:?}_{idx}").into();
                        let zone = PlaceFrom::Timeline(side, card.id);
                        drag(ui, id, zone, |ui| {
                            ui.add(CardDisplay::new(card, to_server).at_zone(zone))
                        });
                    }
                    ui.add_space(ui.available_width());
                });
            });
        });

        if let Some(drop) = drop {
            to_server
                .send(ClientMsg::Move {
                    from: *drop,
                    to: shared::PlaceTo::Timeline(side),
                })
                .unwrap();
            if let Some(card) = data.state.pop_card(*drop) {
                data.state.push_card(card, shared::PlaceTo::Timeline(side));
            }
        }
    });
}

fn board_row(
    side: RelSide,
    data: &mut GameData,
    to_server: &UnboundedSender<ClientMsg>,
    ui: &mut egui::Ui,
) {
    let spaces = match side {
        RelSide::Same => [Space::First, Space::Second, Space::Third, Space::Fourth],
        RelSide::Other => [Space::Fourth, Space::Third, Space::Second, Space::First],
    };
    for space in spaces {
        let frame = Frame::new();
        let mut text = String::from("Empty");
        if let Some(card) = &data.state.get_row(side)[space] {
            text = format!(
                "{} / {} / {}",
                card.counters
                    .get("HP")
                    .copied()
                    .map(|x| x.to_string())
                    .unwrap_or("X".to_string()),
                card.counters
                    .get("DEF")
                    .copied()
                    .map(|x| x.to_string())
                    .unwrap_or("X".to_string()),
                card.counters
                    .get("POW")
                    .copied()
                    .map(|x| x.to_string())
                    .unwrap_or("X".to_string()),
            );
        }
        ui.vertical_centered(|ui| {
            if side == RelSide::Other {
                ui.label(text.clone());
            }
            let (_, dropped_item) = ui.dnd_drop_zone::<PlaceFrom, _>(frame, |ui| {
                if let Some(card) = &data.state.get_row(side)[space] {
                    let id = format!("space_{side:?}_{space:?}").into();
                    let zone = PlaceFrom::Space(side, space);
                    drag(ui, id, PlaceFrom::Space(side, space), |ui| {
                        ui.add(CardDisplay::new(card.clone(), to_server).at_zone(zone))
                    });
                } else {
                    Frame::new().show(ui, |ui| {
                        ui.set_max_height(CARD_HEIGHT);
                        ui.set_max_width(CARD_WIDTH);
                        ui.add(egui::Image::new(
                            TEXTURES.read().get_texture(ImageName::CardBg).clone(),
                        ));
                    });
                }
            });

            if let Some(dropped_item) = dropped_item {
                to_server
                    .send(ClientMsg::Move {
                        from: *dropped_item,
                        to: shared::PlaceTo::Space(side, space, is_key_down(KeyCode::LeftShift)),
                    })
                    .unwrap();
                if let Some(card) = data.state.pop_card(*dropped_item) {
                    data.state.push_card(
                        card,
                        shared::PlaceTo::Space(side, space, is_key_down(KeyCode::LeftShift)),
                    );
                }
            }
            if side == RelSide::Same {
                ui.label(text);
            }
        });
    }
}

fn sidebar(ctx: &Context, to_server: &UnboundedSender<ClientMsg>, data: &mut GameData) {
    egui::SidePanel::left("sidebar")
        .default_width(SIDEBAR_WIDTH)
        .show(ctx, |ui| {
            let (_, dropped_load) = ui.dnd_drop_zone::<PlaceFrom, _>(Frame::new(), |ui| {
                if let Some(card) = data.state.distant_state.discard.first() {
                    let zone = PlaceFrom::Discard(RelSide::Other, card.id);
                    drag(ui, "discard_away".into(), zone, |ui| {
                        ui.add(CardDisplay::new(card.clone(), to_server).at_zone(zone))
                    });
                } else {
                    Frame::new().show(ui, |ui| {
                        ui.set_max_height(CARD_HEIGHT);
                        ui.set_max_width(CARD_WIDTH);
                        ui.add(
                            egui::Image::new(
                                TEXTURES.read().get_texture(ImageName::CardBg).clone(),
                            )
                            .max_width(CARD_WIDTH)
                            .max_height(CARD_HEIGHT),
                        );
                    });
                }
            });
            ui.horizontal(|ui| {
                if ui.button("+").clicked() {
                    to_server
                        .send(ClientMsg::AddBlood(RelSide::Other, true))
                        .unwrap();
                }
                if ui.button("-").clicked() {
                    to_server
                        .send(ClientMsg::AddBlood(RelSide::Other, false))
                        .unwrap();
                }
                ui.label(format!("Blood: {}", data.state.distant_state.blood));
            });

            if let Some(load) = dropped_load {
                to_server
                    .send(ClientMsg::Move {
                        from: *load,
                        to: shared::PlaceTo::Discard(RelSide::Other),
                    })
                    .unwrap();
                if let Some(card) = data.state.pop_card(*load) {
                    data.state
                        .push_card(card, shared::PlaceTo::Discard(RelSide::Other));
                }
            }
            ui.with_layout(Layout::bottom_up(egui::Align::Center), |ui| {
                let (_, dropped) = ui.dnd_drop_zone::<PlaceFrom, _>(Frame::new(), |ui| {
                    ui.set_max_width(CARD_WIDTH);
                    let main_draw = ui.add(ImageButton::new(
                        TEXTURES
                            .read()
                            .get_texture(data.state.local_state.main_deck_top.clone().into())
                            .clone(),
                    ));
                    main_draw.context_menu(|ui| {
                        if ui.button("Shuffle").clicked() {
                            to_server.send(ClientMsg::Shuffle(DeckType::Main)).unwrap();
                        }
                        if ui.button("Search").clicked() {
                            to_server
                                .send(ClientMsg::RequestSearch(DeckType::Main))
                                .unwrap();
                        }
                    });
                    if main_draw.clicked() {
                        to_server
                            .send(ClientMsg::Draw(RelSide::Same, DeckType::Main))
                            .unwrap();
                    }
                });
                if let Some(dropped) = dropped {
                    to_server
                        .send(ClientMsg::Move {
                            from: *dropped,
                            to: shared::PlaceTo::Deck(
                                shared::DeckTo::Top,
                                RelSide::Same,
                                DeckType::Main,
                            ),
                        })
                        .unwrap();
                    if let Some(card) = data.state.pop_card(*dropped) {
                        data.state.push_card(
                            card,
                            shared::PlaceTo::Deck(
                                shared::DeckTo::Top,
                                RelSide::Same,
                                DeckType::Main,
                            ),
                        );
                    }
                }
                let (_, dropped) = ui.dnd_drop_zone::<PlaceFrom, _>(Frame::new(), |ui| {
                    ui.set_max_width(CARD_WIDTH);
                    let blood_draw = ui.add(ImageButton::new(
                        TEXTURES
                            .read()
                            .get_texture(data.state.local_state.blood_deck_top.clone().into())
                            .clone(),
                    ));
                    if blood_draw.clicked() {
                        to_server
                            .send(ClientMsg::Draw(RelSide::Same, DeckType::Blood))
                            .unwrap();
                    }
                    blood_draw.context_menu(|ui| {
                        if ui.button("Shuffle").clicked() {
                            to_server.send(ClientMsg::Shuffle(DeckType::Blood)).unwrap();
                        }
                    });
                });
                if let Some(dropped) = dropped {
                    to_server
                        .send(ClientMsg::Move {
                            from: *dropped,
                            to: shared::PlaceTo::Deck(
                                shared::DeckTo::Top,
                                RelSide::Same,
                                DeckType::Blood,
                            ),
                        })
                        .unwrap();
                    if let Some(card) = data.state.pop_card(*dropped) {
                        data.state.push_card(
                            card,
                            shared::PlaceTo::Deck(
                                shared::DeckTo::Top,
                                RelSide::Same,
                                DeckType::Blood,
                            ),
                        );
                    }
                }
                let frame = Frame::new();
                let (_, dropped_load) = ui.dnd_drop_zone::<PlaceFrom, _>(frame, |ui| {
                    if let Some(card) = data.state.local_state.discard.first() {
                        let zone = PlaceFrom::Discard(RelSide::Same, card.id);
                        drag(ui, "discard".into(), zone, |ui| {
                            ui.add(CardDisplay::new(card.clone(), to_server).at_zone(zone))
                        });
                    } else {
                        Frame::new().show(ui, |ui| {
                            ui.set_max_height(CARD_HEIGHT);
                            ui.set_max_width(CARD_WIDTH);
                            ui.add(egui::Image::new(
                                TEXTURES.read().get_texture(ImageName::CardBg).clone(),
                            ));
                        });
                    }
                });

                if let Some(load) = dropped_load {
                    to_server
                        .send(ClientMsg::Move {
                            from: *load,
                            to: shared::PlaceTo::Discard(RelSide::Same),
                        })
                        .unwrap();
                    if let Some(card) = data.state.pop_card(*load) {
                        data.state
                            .push_card(card, shared::PlaceTo::Discard(RelSide::Same));
                    }
                }

                ui.horizontal(|ui| {
                    if ui.button("+").clicked() {
                        to_server
                            .send(ClientMsg::AddBlood(RelSide::Same, true))
                            .unwrap();
                    }
                    if ui.button("-").clicked() {
                        to_server
                            .send(ClientMsg::AddBlood(RelSide::Same, false))
                            .unwrap();
                    }
                    ui.label(format!("Blood: {}", data.state.local_state.blood));
                });
            });
        });
}

fn handbar(ctx: &Context, to_server: &UnboundedSender<ClientMsg>, data: &mut GameData) {
    egui::TopBottomPanel::bottom("hand")
        .min_height(HANDBAR_HEIGHT)
        .default_height(HANDBAR_HEIGHT)
        .show(ctx, |ui| {
            let frame = Frame::new();
            let (_, moved) = ui.dnd_drop_zone::<PlaceFrom, _>(frame, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.set_min_height(HANDBAR_HEIGHT);
                    for (idx, card) in data.state.hand.iter().enumerate() {
                        let id = format!("hand_{idx}").into();
                        let zone = PlaceFrom::Hand(card.id);
                        drag(ui, id, PlaceFrom::Hand(card.id), |ui| {
                            Frame::new()
                                .show(ui, |ui| {
                                    ui.add(CardDisplay::new(card.clone(), to_server).at_zone(zone))
                                })
                                .inner
                        });
                        // ui.dnd_drag_source(id, PlaceFrom::Hand(idx), |ui| {
                        //     Frame::new().show(ui, |ui| card_display(card.clone(), ui));
                        // });
                    }
                    ui.add_space(ui.available_width());
                });
            });

            if let Some(moved) = moved {
                to_server
                    .send(ClientMsg::Move {
                        from: *moved,
                        to: shared::PlaceTo::Hand,
                    })
                    .unwrap();
                if let Some(card) = data.state.pop_card(*moved) {
                    data.state.push_card(card, shared::PlaceTo::Hand);
                }
            }
        });
}

fn middle(ctx: &Context, to_server: &UnboundedSender<ClientMsg>, data: &mut GameData) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let height = ui.available_height() / 2.;
        ui.add_space(height - CARD_HEIGHT);
        ui.horizontal(|ui| {
            let width = ui.available_width() / 2.;
            ui.add_space(width - (CARD_WIDTH + 2.) * 2.);
            egui::Grid::new("spaces")
                .spacing(Vec2::new(2., 2.))
                .show(ui, |ui| {
                    board_row(RelSide::Other, data, to_server, ui);
                    ui.end_row();
                    board_row(RelSide::Same, data, to_server, ui);
                });
        });
    });
}

fn drag(
    ui: &mut egui::Ui,
    id: Id,
    payload: PlaceFrom,
    add_contents: impl FnOnce(&mut egui::Ui) -> Response,
) -> InnerResponse<Response> {
    if ui.ctx().is_being_dragged(id) {
        DragAndDrop::set_payload(ui.ctx(), payload);

        // Paint the body to a new layer:
        let layer_id = LayerId::new(Order::Tooltip, id);
        let InnerResponse { inner, response } =
            ui.scope_builder(UiBuilder::new().layer_id(layer_id), add_contents);

        // Now we move the visuals of the body to where the mouse is.
        // Normally you need to decide a location for a widget first,
        // because otherwise that widget cannot interact with the mouse.
        // However, a dragged component cannot be interacted with anyway
        // (anything with `Order::Tooltip` always gets an empty [`Response`])
        // So this is fine!

        if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
            let delta = pointer_pos - response.rect.center();
            ui.ctx()
                .transform_layer_shapes(layer_id, TSTransform::from_translation(delta));
        }

        InnerResponse::new(inner, response)
    } else {
        let InnerResponse { inner, response } = ui.scope(add_contents);
        let inner = inner.on_hover_cursor(CursorIcon::Grab);
        if inner.interact(Sense::drag()).drag_started() {
            ui.ctx().set_dragged_id(id);
        }

        InnerResponse::new(inner, response)
    }
}
