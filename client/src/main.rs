use egui_macroquad::egui;
use egui_macroquad::egui::Align;
use egui_macroquad::egui::Frame;
use egui_macroquad::egui::ImageSource;
use egui_macroquad::egui::Layout;
use egui_macroquad::egui::Vec2;
use egui_macroquad::egui::Widget;
use egui_macroquad::egui::ahash::HashMap;
use egui_macroquad::egui::ahash::HashMapExt;
use egui_macroquad::egui::include_image;
use egui_macroquad::egui::mutex::RwLock;
use futures::SinkExt;
use futures::StreamExt;
use image::ImageReader;
use shared::DeckType;
use shared::PlaceFrom;
use shared::RelSide;
use shared::Space;
use std::collections::VecDeque;
use std::sync::LazyLock;
use std::thread;
use tokio::select;
use tokio_websockets::Message;

use futures::never::Never;
use http::Uri;
use shared::{ClientMsg, LocalState, Side};
use shared::{ServerErr, ServerMsg};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::error::TryRecvError;
use tokio_websockets::ClientBuilder;

use macroquad::prelude::*;
use tokio::{
    runtime::{self},
    sync::mpsc::UnboundedReceiver,
};

type ComResult<T> = Result<T, CommunicationError>;

#[derive(Debug)]
enum CommunicationError {
    SerdeReceiveError,
    SerdeSendError,
    Closed,
}

#[derive(Debug)]
enum ChannelError {
    LocalToNetworkClosed,
    NetworkToLocalClosed,
    NetworkToServerError,
}

#[derive(Debug)]
enum NetRuntimeError {
    TokioBuildError,
    ChannelError(ChannelError),
}

#[derive(Debug, Clone)]
enum Scene {
    LobbySelect(LobbyData),
    Game(GameData),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LobbyData {
    room: String,
}

#[derive(Debug, Clone)]
struct GameData {
    state: LocalState,
    local_side: Option<Side>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Image {
    CardBack,
    CardBg,
    Name(String),
}

static TEXTURES: LazyLock<RwLock<HashMap<Image, ImageSource>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

#[macroquad::main("Cassowary")]
async fn main() {
    // Here to_serv and from_serv are actually using "serv" to refer to the networking tokio runtime.
    // That's is because the only reason you'd use these is to connect to the server.
    // However I know I will get confused without these comments so have fun
    let (to_server, from_local) = tokio::sync::mpsc::unbounded_channel::<ClientMsg>();
    let (to_local, mut from_serv) =
        tokio::sync::mpsc::unbounded_channel::<ComResult<Result<ServerMsg, ServerErr>>>();
    let runtime_task = thread::spawn(|| {
        let Ok(rt) = runtime::Builder::new_current_thread().enable_all().build() else {
            return Err(NetRuntimeError::TokioBuildError);
        };
        rt.block_on(game_rt(from_local, to_local))
            .map_err(NetRuntimeError::ChannelError)
    });

    let mut local_side = Some(Side::Home);

    let mut current_scene = Scene::LobbySelect(LobbyData {
        room: String::new(),
    });
    TEXTURES
        .write()
        .insert(Image::CardBack, include_image!("imgs/card_back.png"));
    TEXTURES
        .write()
        .insert(Image::CardBg, include_image!("imgs/cardbg.png"));

    let image = ImageSource::Uri(get_filegarden_link("BloodFlask").into());

    TEXTURES
        .write()
        .insert(Image::Name("BloodFlask".to_string()), image);
    let image = ImageSource::Uri(get_filegarden_link("Daemon").into());
    TEXTURES
        .write()
        .insert(Image::Name("Daemon".to_string()), image);

    loop {
        if runtime_task.is_finished() {
            break;
        }
        match from_serv.try_recv() {
            Ok(Ok(Ok(msg))) => process_server_message(msg, &mut current_scene, &to_server),
            Ok(Ok(Err(msg))) => process_server_error(msg, &mut current_scene),
            Ok(Err(error)) => panic!("Error: {error:#?}"),
            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => panic!("NetworkToLocal channel closed"),
        }
        clear_background(GRAY);

        current_scene.draw(&to_server).await;

        egui_macroquad::draw();

        next_frame().await;
    }

    match runtime_task.join() {
        Ok(Err(a)) => eprintln!("Network runtime failed: {a:#?}"),
        Err(panic) => eprintln!("Network runtime task panicked with {panic:#?}"),
    }
}

fn process_server_error(msg: ServerErr, current_scene: &mut Scene) {
    match msg {
        ServerErr::RoomDoesntExist(string) => println!("Room doesn't exist"),
        ServerErr::NotInGame { action } => println!("Not in game, but tried to {action}"),
        ServerErr::NotInSide => println!("Player is not currently playing"),
        ServerErr::NoPlayerInSide(side) => println!("Player is not in {side:?}"),
        ServerErr::NoCardIn(place_from) => println!("No card in {place_from:?}"),
        ServerErr::SideOccupied(side) => println!("{side:?} is already occupied"),
        ServerErr::AlreadyInGame { action } => println!("Already in game"),
    }
}

fn process_server_message(
    msg: ServerMsg,
    current_scene: &mut Scene,
    to_server: &UnboundedSender<ClientMsg>,
) {
    let board_state = match current_scene {
        Scene::LobbySelect(_) => None,
        Scene::Game(game_data) => Some(&mut game_data.state),
    };
    if msg.is_game_action() {
        let board_state = board_state.unwrap();
        match msg {
            ServerMsg::UpdateHand(vec) => board_state.hand = vec,
            ServerMsg::UpdateSpaces { home_row, away_row } => {
                board_state.home_row = *home_row;
                board_state.away_row = *away_row;
            }
            ServerMsg::UpdateDiscard(side, vec) => match side {
                Side::Home => board_state.home_state.discard = vec,
                Side::Away => board_state.away_state.discard = vec,
            },
            ServerMsg::UpdateTimeline(side, vec) => match side {
                Side::Home => board_state.home_state.timeline = vec,
                Side::Away => board_state.away_state.timeline = vec,
            },
            ServerMsg::BeginSearch(vec) => todo!(),
            ServerMsg::UpdateState(new_state) => *board_state = *new_state,
            ServerMsg::JoinedRoom(..) => panic!("??"),
            ServerMsg::RoomCreated => panic!("??"),
        }
        return;
    }
    match msg {
        ServerMsg::UpdateHand(..) => panic!("??"),
        ServerMsg::UpdateSpaces { .. } => panic!("??"),
        ServerMsg::UpdateDiscard(..) => panic!("??"),
        ServerMsg::UpdateTimeline(..) => panic!("??"),
        ServerMsg::BeginSearch(..) => panic!("??"),
        ServerMsg::UpdateState(..) => panic!("??"),
        ServerMsg::RoomCreated => (),
        ServerMsg::JoinedRoom(state) => {
            to_server.send(ClientMsg::PlayAs(Side::Home)).unwrap();
            to_server
                .send(ClientMsg::SetDeck(
                    DeckType::Main,
                    VecDeque::from(vec![
                        "Daemon".to_string(),
                        "Daemon".to_string(),
                        "Daemon".to_string(),
                        "Daemon".to_string(),
                        "Daemon".to_string(),
                    ]),
                ))
                .unwrap();
            to_server
                .send(ClientMsg::SetDeck(
                    DeckType::Blood,
                    VecDeque::from(vec![
                        "BloodFlask".to_string(),
                        "BloodFlask".to_string(),
                        "BloodFlask".to_string(),
                        "BloodFlask".to_string(),
                        "BloodFlask".to_string(),
                        "BloodFlask".to_string(),
                    ]),
                ))
                .unwrap();
            *current_scene = Scene::Game(GameData {
                state: *state,
                local_side: None,
            })
        }
    }
}

async fn game_rt(
    mut from_local: UnboundedReceiver<ClientMsg>,
    to_local: UnboundedSender<ComResult<Result<ServerMsg, ServerErr>>>,
) -> Result<Never, ChannelError> {
    let uri = Uri::from_static("ws://127.0.0.1:3000");
    let (mut client, _) = ClientBuilder::from_uri(uri).connect().await.unwrap();

    loop {
        select! {
            Some(Ok(msg)) = client.next() => {
                if msg.is_close() {
                    to_local.send(Err(CommunicationError::Closed)).unwrap();
                }

                let Some(msg) = msg.as_text() else { continue };

                let msg = serde_json::from_str::<Result<ServerMsg, ServerErr>>(msg)
                    .map_err(|_| CommunicationError::SerdeReceiveError);

                to_local.send(msg).map_err(|_| ChannelError::NetworkToLocalClosed)?;
            },
            message = from_local.recv() => {
                let Some(message) = message else { panic!() };
                let msg = serde_json::to_string_pretty(&message);
                match msg {
                    Ok(msg) => client.send(Message::text(msg)).await.map_err(|_| ChannelError::NetworkToServerError)?,
                    Err(_) => to_local
                        .send(Err(CommunicationError::SerdeSendError))
                        .map_err(|_| ChannelError::NetworkToLocalClosed)?,
                }
            },
        }
    }
}

impl Scene {
    async fn draw(&mut self, to_server: &UnboundedSender<ClientMsg>) {
        match self {
            Scene::LobbySelect(data) => {
                if let Some(a) = draw_lobby_select(to_server, data) {
                    *self = a
                }
            }
            Scene::Game(data) => draw_game(to_server, data).await,
        }
    }
}

fn draw_lobby_select(
    to_server: &UnboundedSender<ClientMsg>,
    scene: &mut LobbyData,
) -> Option<Scene> {
    let mut next_scene = None;
    egui_macroquad::ui(|ctx| {
        // Get the screen size from egui
        let screen_rect = ctx.screen_rect();
        let window_size = egui::vec2(300.0, 100.0);

        // Calculate center position
        let center_pos = screen_rect.center() - window_size / 2.0;
        let win = egui::Window::new("Cassowary")
            .resizable(true)
            .collapsible(false)
            .movable(false)
            .resizable(false)
            .current_pos(center_pos);

        win.show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut scene.room);
                    let join_room = ui.button("Join Room");
                    if join_room.clicked() {
                        to_server
                            .send(ClientMsg::JoinRoom(scene.room.clone()))
                            .unwrap();
                    }
                });
                let create_room = ui.button("Create Room");

                if create_room.clicked() {
                    to_server.send(ClientMsg::CreateRoom).unwrap();
                }
            })
        });
    });

    next_scene
}

const SIDEBAR_PADDING: f32 = 5.0;
const CARD_WIDTH: f32 = 80.0;
const CARD_HEIGHT: f32 = CARD_WIDTH * (3.5 / 2.5);
const SIDEBAR_WIDTH: f32 = SIDEBAR_PADDING * 2. + CARD_WIDTH;
const HANDBAR_HEIGHT: f32 = SIDEBAR_PADDING * 2. + CARD_HEIGHT;

async fn draw_game(to_server: &UnboundedSender<ClientMsg>, data: &mut GameData) {
    egui_macroquad::ui(|ctx| {
        egui_extras::install_image_loaders(ctx);
        egui::SidePanel::left("sidebar")
            .default_width(SIDEBAR_WIDTH)
            .show(ctx, |ui| {
                let draw_button = ui.button("MAIN");
                if draw_button.clicked() {
                    to_server
                        .send(ClientMsg::Draw(RelSide::Same, DeckType::Main))
                        .unwrap();
                }
                let draw_button = ui.button("BLOOD");
                if draw_button.clicked() {
                    to_server
                        .send(ClientMsg::Draw(RelSide::Same, DeckType::Blood))
                        .unwrap();
                }
                let frame = Frame::new();
                let (_, dropped_load) = ui.dnd_drop_zone::<PlaceFrom, _>(frame, |ui| {
                    ui.label("DISCARD");
                });

                if let Some(load) = dropped_load {
                    to_server
                        .send(ClientMsg::Move {
                            from: *load,
                            to: shared::PlaceTo::Discard,
                        })
                        .unwrap();
                }
            });
        egui::TopBottomPanel::bottom("hand")
            .default_height(SIDEBAR_WIDTH)
            .show(ctx, |ui| {
                let frame = Frame::new();
                let (_, moved) = ui.dnd_drop_zone::<PlaceFrom, _>(frame, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (idx, card) in data.state.hand.iter().enumerate() {
                            let id = format!("hand_{idx}").into();
                            ui.dnd_drag_source(id, PlaceFrom::Hand(idx), |ui| {
                                Frame::new().show(ui, |ui| {
                                    let image = {
                                        let a = TEXTURES.read();
                                        a.get(&Image::Name(card.clone()))
                                            .unwrap_or(a.get(&Image::CardBack).unwrap())
                                            .clone()
                                    };
                                    Frame::new().show(ui, |ui| {
                                        ui.set_max_height(CARD_HEIGHT);
                                        ui.set_max_width(CARD_WIDTH);
                                        ui.add(egui::Image::new(image));
                                    });
                                });
                            });
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

        egui::CentralPanel::default().show(ctx, |ui| {
            let height = ui.available_height() / 2.;
            ui.add_space(height - CARD_HEIGHT);
            ui.horizontal(|ui| {
                let width = ui.available_width() / 2.;
                ui.add_space(width - (CARD_WIDTH + 2.) * 2.);
                egui::Grid::new("spaces")
                    .spacing(Vec2::new(2., 2.))
                    .show(ui, |ui| {
                        for space in [Space::First, Space::Second, Space::Third, Space::Fourth] {
                            let frame = Frame::new();
                            let (_, dropped_item) = ui.dnd_drop_zone::<PlaceFrom, _>(frame, |ui| {
                                if let Some(card) = &data.state.away_row[space] {
                                    let id = format!("space_away_{space:?}").into();
                                    ui.dnd_drag_source(
                                        id,
                                        PlaceFrom::Space(Side::Away, space),
                                        |ui| {
                                            let image = {
                                                let a = TEXTURES.read();
                                                a.get(&Image::Name(card.name.clone()))
                                                    .unwrap_or(a.get(&Image::CardBack).unwrap())
                                                    .clone()
                                            };
                                            Frame::new().show(ui, |ui| {
                                                ui.set_max_height(CARD_HEIGHT);
                                                ui.set_max_width(CARD_WIDTH);
                                                ui.add(egui::Image::new(image));
                                            });
                                        },
                                    );
                                } else {
                                    Frame::new().show(ui, |ui| {
                                        ui.set_max_height(CARD_HEIGHT);
                                        ui.set_max_width(CARD_WIDTH);
                                        ui.add(egui::Image::new(
                                            TEXTURES.read()[&Image::CardBg].clone(),
                                        ));
                                    });
                                }
                            });

                            if let Some(dropped_item) = dropped_item {
                                to_server
                                    .send(ClientMsg::Move {
                                        from: *dropped_item,
                                        to: shared::PlaceTo::Space(Side::Away, space, true),
                                    })
                                    .unwrap();
                            }
                        }
                        ui.end_row();
                        for space in [Space::First, Space::Second, Space::Third, Space::Fourth] {
                            let frame = Frame::new();
                            let (_, dropped_item) = ui.dnd_drop_zone::<PlaceFrom, _>(frame, |ui| {
                                if let Some(card) = &data.state.home_row[space] {
                                    let id = format!("space_home_{space:?}").into();
                                    ui.dnd_drag_source(
                                        id,
                                        PlaceFrom::Space(Side::Home, space),
                                        |ui| {
                                            let image = {
                                                let a = TEXTURES.read();
                                                a.get(&Image::Name(card.name.clone()))
                                                    .unwrap_or(a.get(&Image::CardBack).unwrap())
                                                    .clone()
                                            };
                                            Frame::new().show(ui, |ui| {
                                                ui.set_max_height(CARD_HEIGHT);
                                                ui.set_max_width(CARD_WIDTH);
                                                ui.add(egui::Image::new(image));
                                            });
                                        },
                                    );
                                } else {
                                    Frame::new().show(ui, |ui| {
                                        ui.set_max_height(CARD_HEIGHT);
                                        ui.set_max_width(CARD_WIDTH);
                                        ui.add(egui::Image::new(
                                            TEXTURES.read()[&Image::CardBg].clone(),
                                        ));
                                    });
                                }
                            });

                            if let Some(dropped_item) = dropped_item {
                                to_server
                                    .send(ClientMsg::Move {
                                        from: *dropped_item,
                                        to: shared::PlaceTo::Space(Side::Home, space, true),
                                    })
                                    .unwrap();
                            }
                        }
                    });
            });
        });
    });
    return;

    let screen_height = screen_height();
    let screen_width = screen_width();
    let draw_btn_rect = Rect {
        x: SIDEBAR_PADDING,
        y: screen_height - CARD_HEIGHT - SIDEBAR_PADDING,
        w: CARD_WIDTH,
        h: CARD_HEIGHT,
    };
    draw_rectangle(0., 0., SIDEBAR_WIDTH, screen_height, RED);
    draw_rectangle(
        SIDEBAR_PADDING,
        screen_height - CARD_HEIGHT - SIDEBAR_PADDING,
        CARD_WIDTH,
        CARD_HEIGHT,
        YELLOW,
    );
    draw_rectangle(
        SIDEBAR_PADDING * 2. + CARD_WIDTH,
        screen_height - HANDBAR_HEIGHT,
        screen_width - SIDEBAR_WIDTH,
        HANDBAR_HEIGHT,
        GREEN,
    );

    for (idx, card) in data.state.hand.iter().enumerate() {
        let card_x = SIDEBAR_WIDTH + SIDEBAR_PADDING + (CARD_HEIGHT + SIDEBAR_PADDING) * idx as f32;
        let card_y = screen_height - HANDBAR_HEIGHT + SIDEBAR_PADDING;
        draw_rectangle(card_x, card_y, CARD_WIDTH, CARD_HEIGHT, WHITE);
        draw_text(card, card_x, card_y + 9., 18., BLACK);
    }

    if is_mouse_button_pressed(MouseButton::Left) {
        let mpos = mouse_position();
        let mpos = vec2(mpos.0, mpos.1);
        if draw_btn_rect.contains(mpos) {
            to_server
                .send(ClientMsg::Draw(RelSide::Same, DeckType::Main))
                .unwrap();
        }
    }
}

fn get_filegarden_link(name: &str) -> String {
    format!(
        "https://file.garden/ZJSEzoaUL3bz8vYK/bloodlesscards/{}.png",
        name.replace(' ', "").replace('ä', "a")
    )
}

pub struct CardDisplay;

impl Widget for CardDisplay {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let image = {
            let a = TEXTURES.read();
            a.get(&Image::Name(card.name.clone()))
                .unwrap_or(a.get(&Image::CardBack).unwrap())
                .clone()
        };
        Frame::new().show(ui, |ui| {
            ui.set_max_height(CARD_HEIGHT);
            ui.set_max_width(CARD_WIDTH);
            ui.add(egui::Image::new(image));
        });
    }
}
