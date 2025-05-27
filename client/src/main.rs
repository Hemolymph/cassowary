mod drawing;
use drawing::Card;
use drawing::Click;
use drawing::Draw;
use drawing::Rect;
use egui_macroquad::egui;
use egui_macroquad::egui::Frame;
use egui_macroquad::egui::ImageSource;
use egui_macroquad::egui::Widget;
use egui_macroquad::egui::ahash::HashMap;
use egui_macroquad::egui::ahash::HashMapExt;
use egui_macroquad::egui::include_image;
use egui_macroquad::egui::mutex::RwLock;
use futures::SinkExt;
use futures::StreamExt;
use image::ImageReader;
use macroquad::color::BLACK;
use macroquad::color::GRAY;
use macroquad::color::GREEN;
use macroquad::color::RED;
use macroquad::color::WHITE;
use macroquad::color::YELLOW;
use macroquad::text::draw_text;
use macroquad::texture::Texture2D;
use macroquad::texture::load_texture;
use macroquad::window::clear_background;
use macroquad::window::next_frame;
use macroquad::window::screen_height;
use macroquad::window::screen_width;
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

static TEXTURES: LazyLock<RwLock<HashMap<Image, Texture2D>>> =
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
    TEXTURES.write().insert(
        Image::CardBack,
        load_texture("src/imgs/card_back.png").await.unwrap(),
    );
    TEXTURES.write().insert(
        Image::CardBg,
        load_texture("src/imgs/cardbg.png").await.unwrap(),
    );
    TEXTURES.write().insert(
        Image::Name("BloodFlask".to_string()),
        load_texture(&get_filegarden_link("BloodFlask"))
            .await
            .unwrap(),
    );
    TEXTURES.write().insert(
        Image::Name("Daemon".to_string()),
        load_texture(&get_filegarden_link("Daemon")).await.unwrap(),
    );

    // let image = ImageSource::Uri(get_filegarden_link("BloodFlask").into());

    // TEXTURES
    //     .write()
    //     .insert(Image::Name("BloodFlask".to_string()), image);
    // let image = ImageSource::Uri(get_filegarden_link("Daemon").into());
    // TEXTURES
    //     .write()
    //     .insert(Image::Name("Daemon".to_string()), image);

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
                let Some(message) = message else { panic!("Channel closed") };
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
    egui_macroquad::ui(|ctx| {});

    let screen_height = screen_height();
    let screen_width = screen_width();
    let sidebar = Rect {
        x: 0.0,
        y: 0.0,
        w: SIDEBAR_WIDTH,
        h: screen_height,
        color: RED,
    };
    let main_draw = Rect {
        x: SIDEBAR_PADDING,
        y: screen_height - CARD_HEIGHT - SIDEBAR_PADDING,
        w: CARD_WIDTH,
        h: CARD_HEIGHT,
        color: YELLOW,
    }
    .clickable();
    let blood_draw = Rect {
        x: SIDEBAR_PADDING,
        y: screen_height - (CARD_HEIGHT - SIDEBAR_PADDING) * 2. - SIDEBAR_PADDING,
        w: CARD_WIDTH,
        h: CARD_HEIGHT,
        color: YELLOW,
    }
    .clickable();
    let hand = Rect {
        x: SIDEBAR_PADDING * 2. + CARD_WIDTH,
        y: screen_height - HANDBAR_HEIGHT,
        w: screen_width - SIDEBAR_WIDTH,
        h: HANDBAR_HEIGHT,
        color: GREEN,
    };
    sidebar.draw().await;
    main_draw.draw().await;
    blood_draw.draw().await;
    hand.draw().await;

    for (idx, name) in data.state.hand.iter().enumerate() {
        let x = SIDEBAR_WIDTH + SIDEBAR_PADDING + (CARD_HEIGHT + SIDEBAR_PADDING) * idx as f32;
        let y = screen_height - HANDBAR_HEIGHT + SIDEBAR_PADDING;
        let rect = Rect {
            x,
            y,
            w: CARD_WIDTH,
            h: CARD_HEIGHT,
            color: WHITE,
        };
        let card = Card {
            rect,
            image: name.to_string(),
        };
        card.draw().await;
        draw_text(name, x, y + 9., 18., BLACK);
    }

    if main_draw.is_clicked() {
        to_server
            .send(ClientMsg::Draw(RelSide::Same, DeckType::Main))
            .unwrap();
    }
    if blood_draw.is_clicked() {
        to_server
            .send(ClientMsg::Draw(RelSide::Same, DeckType::Blood))
            .unwrap();
    }
}

fn get_filegarden_link(name: &str) -> String {
    format!(
        "https://file.garden/ZJSEzoaUL3bz8vYK/bloodlesscards/{}.png",
        name.replace(' ', "").replace('ä', "a")
    )
}
