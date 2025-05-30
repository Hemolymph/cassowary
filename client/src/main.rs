mod scene;
use egui_macroquad::egui;
use egui_macroquad::egui::Context;
use egui_macroquad::egui::ImageSource;
use egui_macroquad::egui::ahash::HashMap;
use egui_macroquad::egui::include_image;
use egui_macroquad::egui::mutex::RwLock;
use futures::SinkExt;
use futures::StreamExt;
use scene::GameData;
use scene::LobbyData;
use scene::Scene;
use shared::DeckType;
use shared::LocalDeckTop;
use shared::RelSide;
use shrek_deck::GetCardInfo;
use shrek_deck::tts::CardShape;
use std::sync::LazyLock;
use std::thread;
use tokio::select;
use tokio_websockets::Message;

use futures::never::Never;
use http::Uri;
use shared::ClientMsg;
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

#[derive(Clone)]
struct BloodlessCard {
    name: String,
}

impl GetCardInfo for BloodlessCard {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_front_image(&self) -> Result<String, shrek_deck::CardError> {
        Ok(get_filegarden_link(self.get_name()))
    }

    fn get_back_image(&self) -> Result<String, shrek_deck::CardError> {
        Ok("https://file.garden/ZJSEzoaUL3bz8vYK/bloodlesscards/00%20back.png".to_string())
    }

    fn get_card_shape(&self) -> Result<CardShape, shrek_deck::CardError> {
        Ok(CardShape::RoundedRectangle)
    }

    fn parse(string: &str) -> Result<Self, shrek_deck::parser::ParseError> {
        Ok(BloodlessCard {
            name: string.to_owned(),
        })
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ImageName {
    CardBack,
    CardBg,
    BloodBack,
    Name(String),
}

impl From<LocalDeckTop> for ImageName {
    fn from(value: LocalDeckTop) -> Self {
        match value {
            LocalDeckTop::Empty => Self::CardBg,
            LocalDeckTop::Card => Self::CardBack,
            LocalDeckTop::Revealed(card) => Self::Name(card),
        }
    }
}

#[derive(Default)]
struct Resources<'a> {
    pub textures: HashMap<ImageName, ImageSource<'a>>,
}

impl<'a> Resources<'a> {
    fn setup(&mut self) {
        self.textures
            .insert(ImageName::CardBack, include_image!("imgs/card_back.png"));
        self.textures
            .insert(ImageName::BloodBack, include_image!("imgs/flask_back.png"));
        self.textures
            .insert(ImageName::CardBg, include_image!("imgs/cardbg.png"));
    }
    fn set_texture(&mut self, path: String, ctx: &Context) {
        let source = ImageSource::Uri(get_filegarden_link(&path).into());
        ctx.try_load_image(source.uri().unwrap(), egui::SizeHint::Scale(1.0.into()))
            .unwrap();
        self.textures.insert(ImageName::Name(path.clone()), source);
    }
    fn get_texture(&self, image: ImageName) -> ImageSource<'a> {
        match image {
            ImageName::CardBack => include_image!("imgs/card_back.png"),
            ImageName::BloodBack => include_image!("imgs/flask_back.png"),
            ImageName::CardBg => include_image!("imgs/cardbg.png"),
            ImageName::Name(path) => ImageSource::Uri(get_filegarden_link(&path).into()),
        }
    }
}

static TEXTURES: LazyLock<RwLock<Resources>> = LazyLock::new(|| RwLock::new(Resources::default()));

fn window_conf() -> Conf {
    Conf {
        window_title: "Cassowary".to_string(),
        window_width: 1400,
        window_height: 800,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    egui_macroquad::cfg(|ctx| {
        egui_extras::install_image_loaders(ctx);
        let mut stp = TEXTURES.write();
        stp.setup();
        stp.set_texture("BloodFlask".to_string(), ctx);
        stp.set_texture("Daemon".to_string(), ctx);
    });
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

    let mut current_scene = Scene::LobbySelect(LobbyData {
        room: String::new(),
    });

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
        ServerErr::GameIsFull => println!("Game is full"),
        ServerErr::RoomAlreadyExist => println!("Room alrady exist"),
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
            ServerMsg::UpdateHand(vec, blood_top, main_top) => {
                board_state.local_state.main_deck_top = main_top;
                board_state.local_state.blood_deck_top = blood_top;
                board_state.hand = vec;
            }
            ServerMsg::UpdateSpaces { home_row, away_row } => {
                board_state.local_row = *home_row;
                board_state.distant_row = *away_row;
            }
            ServerMsg::UpdateDiscard(side, vec) => match side {
                RelSide::Same => board_state.local_state.discard = vec,
                RelSide::Other => board_state.distant_state.discard = vec,
            },
            ServerMsg::UpdateTimeline(side, vec) => match side {
                RelSide::Same => board_state.local_state.timeline = vec,
                RelSide::Other => board_state.distant_state.timeline = vec,
            },
            ServerMsg::BeginSearch(vec) => match current_scene {
                Scene::LobbySelect(lobby_data) => todo!(),
                Scene::Game(game_data) => game_data.seaching = vec,
            },
            ServerMsg::UpdateState(new_state) => {
                *board_state = *new_state;
            }
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
            to_server.send(ClientMsg::PlayAs).unwrap();
            *current_scene = Scene::Game(GameData {
                state: *state,
                editing_deck: false,
                deck: DeckType::Main,
                marrow_main: String::new(),
                marrow_blood: String::new(),
                marrow_error: String::new(),
                seaching: vec![],
                creating: String::new(),
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
                let Some(message) = message else { return Err(ChannelError::LocalToNetworkClosed) };
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

const SIDEBAR_PADDING: f32 = 5.0;
const CARD_WIDTH: f32 = 80.0;
const CARD_HEIGHT: f32 = CARD_WIDTH * (3.5 / 2.5);
const SIDEBAR_WIDTH: f32 = SIDEBAR_PADDING * 2. + CARD_WIDTH;
const HANDBAR_HEIGHT: f32 = SIDEBAR_PADDING * 2. + CARD_HEIGHT;

fn get_filegarden_link(name: &str) -> String {
    format!(
        "https://file.garden/ZJSEzoaUL3bz8vYK/bloodlesscards/{}.png",
        name.replace(' ', "").replace('ä', "a")
    )
}
