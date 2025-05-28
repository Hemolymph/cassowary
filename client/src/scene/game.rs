use egui_macroquad::egui::{
    self, Context, CursorIcon, DragAndDrop, Frame, Id, ImageButton, InnerResponse, LayerId, Layout,
    Order, Response, Sense, UiBuilder, Vec2, Widget, emath::TSTransform,
};
use shared::{ClientMsg, DeckType, LocalState, PlaceFrom, RelSide, Space};
use tokio::sync::mpsc::UnboundedSender;

use crate::{CARD_HEIGHT, CARD_WIDTH, HANDBAR_HEIGHT, ImageName, SIDEBAR_WIDTH, TEXTURES};

#[derive(Debug, Clone)]
pub struct GameData {
    pub state: LocalState,
}

pub async fn draw_game(to_server: &UnboundedSender<ClientMsg>, data: &mut GameData) {
    egui_macroquad::ui(|ctx| {
        sidebar(ctx, to_server, data);
        handbar(ctx, to_server, data);
        timeline(ctx, RelSide::Same, data, to_server);
        timeline(ctx, RelSide::Other, data, to_server);
        middle(ctx, to_server, data);
    });
}

struct CardDisplay {
    name: String,
    location: Option<PlaceFrom>,
}

impl CardDisplay {
    fn new<S: AsRef<str>>(name: S) -> Self {
        Self {
            name: name.as_ref().to_string(),
            location: None,
        }
    }

    fn at_zone(self, zone: PlaceFrom) -> Self {
        Self {
            name: self.name,
            location: Some(zone),
        }
    }
}

impl Widget for CardDisplay {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(CARD_WIDTH, CARD_HEIGHT), Sense::click());

        let image = {
            let a = TEXTURES.read();
            a.get(&ImageName::Name(self.name))
                .unwrap_or(a.get(&ImageName::CardBack).unwrap())
                .clone()
        };

        let response = response.on_hover_ui_at_pointer(|ui| {
            ui.image(image.clone());
        });

        response.context_menu(|ui| {
            ui.label("Stats");
            ui.horizontal(|ui| {
                ui.button("-");
                ui.button("+");
                ui.label("DMG: 0");
            });
            ui.horizontal(|ui| {
                ui.button("-");
                ui.button("+");
                ui.label("DEF: 0");
            });
            ui.horizontal(|ui| {
                ui.button("-");
                ui.button("+");
                ui.label("POW: 0");
            });
            ui.label("Counters");
            ui.horizontal(|ui| {
                ui.button("-");
                ui.button("+");
                ui.label("RED: 0");
            });
            ui.horizontal(|ui| {
                ui.button("-");
                ui.button("+");
                ui.label("BLUE: 0");
            });
            ui.horizontal(|ui| {
                ui.button("-");
                ui.button("+");
                ui.label("YELLOW: 0");
            });
        });

        egui::Image::new(image.clone()).paint_at(ui, rect);
        response
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
                        let zone = PlaceFrom::Timeline(side, idx);
                        drag(ui, id, zone, |ui| {
                            ui.add(CardDisplay::new(card).at_zone(zone))
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
        let (_, dropped_item) = ui.dnd_drop_zone::<PlaceFrom, _>(frame, |ui| {
            if let Some(card) = &data.state.get_row(side)[space] {
                let id = format!("space_{side:?}_{space:?}").into();
                let zone = PlaceFrom::Space(side, space);
                drag(ui, id, PlaceFrom::Space(side, space), |ui| {
                    ui.add(CardDisplay::new(card.name.clone()).at_zone(zone))
                });
            } else {
                Frame::new().show(ui, |ui| {
                    ui.set_max_height(CARD_HEIGHT);
                    ui.set_max_width(CARD_WIDTH);
                    ui.add(egui::Image::new(
                        TEXTURES.read()[&ImageName::CardBg].clone(),
                    ));
                });
            }
        });

        if let Some(dropped_item) = dropped_item {
            to_server
                .send(ClientMsg::Move {
                    from: *dropped_item,
                    to: shared::PlaceTo::Space(side, space, true),
                })
                .unwrap();
        }
    }
}

fn sidebar(ctx: &Context, to_server: &UnboundedSender<ClientMsg>, data: &mut GameData) {
    egui::SidePanel::left("sidebar")
        .default_width(SIDEBAR_WIDTH)
        .show(ctx, |ui| {
            ui.with_layout(Layout::bottom_up(egui::Align::Center), |ui| {
                let (_, dropped) = ui.dnd_drop_zone::<PlaceFrom, _>(Frame::new(), |ui| {
                    let draw_button = ui.add(ImageButton::new(
                        TEXTURES.read().get(&ImageName::CardBack).unwrap().clone(),
                    ));
                    if draw_button.clicked() {
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
                        .unwrap()
                }
                let (_, dropped) = ui.dnd_drop_zone::<PlaceFrom, _>(Frame::new(), |ui| {
                    let draw_button = ui.add(ImageButton::new(
                        TEXTURES.read().get(&ImageName::BloodBack).unwrap().clone(),
                    ));
                    if draw_button.clicked() {
                        to_server
                            .send(ClientMsg::Draw(RelSide::Same, DeckType::Blood))
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
                                DeckType::Blood,
                            ),
                        })
                        .unwrap()
                }
                let frame = Frame::new();
                let (_, dropped_load) = ui.dnd_drop_zone::<PlaceFrom, _>(frame, |ui| {
                    if let Some(card) = data.state.local_state.discard.first() {
                        let zone = PlaceFrom::Discard(RelSide::Same, 0);
                        drag(ui, "discard".into(), zone, |ui| {
                            ui.add(CardDisplay::new(card).at_zone(zone))
                        });
                    } else {
                        Frame::new().show(ui, |ui| {
                            ui.set_max_height(CARD_HEIGHT);
                            ui.set_max_width(CARD_WIDTH);
                            ui.add(egui::Image::new(
                                TEXTURES.read()[&ImageName::CardBg].clone(),
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
                }
                ui.add_space(ui.available_height() - CARD_HEIGHT);
                let (_, dropped_load) = ui.dnd_drop_zone::<PlaceFrom, _>(frame, |ui| {
                    if let Some(card) = data.state.distant_state.discard.first() {
                        let zone = PlaceFrom::Discard(RelSide::Other, 0);
                        drag(ui, "discard_away".into(), zone, |ui| {
                            ui.add(CardDisplay::new(card).at_zone(zone))
                        });
                    } else {
                        Frame::new().show(ui, |ui| {
                            ui.set_max_height(CARD_HEIGHT);
                            ui.set_max_width(CARD_WIDTH);
                            ui.add(egui::Image::new(
                                TEXTURES.read()[&ImageName::CardBg].clone(),
                            ));
                        });
                    }
                });

                if let Some(load) = dropped_load {
                    to_server
                        .send(ClientMsg::Move {
                            from: *load,
                            to: shared::PlaceTo::Discard(RelSide::Other),
                        })
                        .unwrap();
                }
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
                        let zone = PlaceFrom::Hand(idx);
                        drag(ui, id, PlaceFrom::Hand(idx), |ui| {
                            Frame::new()
                                .show(ui, |ui| ui.add(CardDisplay::new(card).at_zone(zone)))
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
