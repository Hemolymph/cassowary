// mod scene;
mod windows;
use futures::SinkExt;
use futures::StreamExt;
use iced::Element;
use iced::Subscription;
use iced::Task;
use iced::stream;
use iced::widget::image::Handle;
use iced::widget::text_input;
use tokio::sync::mpsc;
// use scene::GameData;
// use scene::LobbyData;
// use scene::Scene;
use shared::DeckType;
use shared::LocalDeckTop;
use shared::RelSide;
use shrek_deck::GetCardInfo;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::LazyLock;
use std::thread;
use std::time::Duration;
use tokio::select;
use tokio::sync::RwLock;
use tokio_websockets::Error;
use tokio_websockets::Message;
use windows::GameMessage;
use windows::GameState;
use windows::MenuMessage;
use windows::MenuState;
use windows::Window;
use windows::game_view;
use windows::menu_view;

use http::Uri;
use shared::ClientMsg;
use shared::{ServerErr, ServerMsg};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::error::TryRecvError;
use tokio_websockets::ClientBuilder;

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

    fn parse(string: &str) -> Result<Self, shrek_deck::parser::ParseError> {
        Ok(BloodlessCard {
            name: string.to_owned(),
        })
    }
}

async fn fetch_image(url: &str) -> Result<Vec<u8>, reqwest::Error> {
    let bytes = reqwest::get(get_filegarden_link(url))
        .await?
        .bytes()
        .await?;
    Ok(bytes.to_vec())
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
    NetworkToServerError(Error),
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
    StartTurnBtn,
    MainPhaseBtn,
    AttackPhaseBtn,
    EndTurnBtn,
    SwitchTurnBtn,
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
struct Resources {
    pub textures: HashMap<ImageName, Handle>,
    pub loading: HashSet<String>,
}

impl Resources {
    fn setup(&mut self) {
        self.textures.insert(
            ImageName::CardBg,
            Handle::from_bytes(include_bytes!("imgs/cardbg.png").to_vec()),
        );
    }
    fn get_texture(&self, image: &ImageName, to_textures: &UnboundedSender<String>) -> Handle {
        match image {
            ImageName::Name(name) => self.textures.get(image).cloned().unwrap_or_else(|| {
                if !self.loading.contains(name) {
                    to_textures.send(name.to_string()).unwrap();
                }
                self.textures.get(&ImageName::CardBg).unwrap().clone()
            }),
            _ => self.textures.get(image).unwrap().clone(),
        }
    }
}

// static TEXTURES: LazyLock<RwLock<Resources>> = LazyLock::new(|| RwLock::new(Resources::default()));

// fn window_conf() -> Conf {
//     Conf {
//         window_title: "Cassowary".to_string(),
//         window_width: 1400,
//         window_height: 800,
//         ..Default::default()
//     }
// }

// #[macroquad::main(window_conf)]
fn main() {
    // Here to_serv and from_serv are actually using "serv" to refer to the networking tokio runtime.
    // That's is because the only reason you'd use these is to connect to the server.
    // However I know I will get confused without these comments so have fun

    // let (to_server, from_local) = tokio::sync::mpsc::unbounded_channel::<ClientMsg>();
    // let (to_server_ping, from_local_ping) = tokio::sync::mpsc::unbounded_channel::<()>();
    // let (to_local, mut from_serv) =
    //     tokio::sync::mpsc::unbounded_channel::<ComResult<Result<ServerMsg, ServerErr>>>();
    // let runtime_task = thread::spawn(|| {
    //     let Ok(rt) = runtime::Builder::new_current_thread().enable_all().build() else {
    //         return Err(NetRuntimeError::TokioBuildError);
    //     };
    //     rt.spawn(heartbeat(to_server_ping));
    //     rt.block_on(game_rt(from_local_ping, from_local, to_local))
    //         .map_err(NetRuntimeError::ChannelError)
    // });
    //

    iced::application(Cassowary::default, update, view)
        .title("Cassowary")
        .subscription(network)
        .run()
        .unwrap();

    // match runtime_task.join() {
    //     Ok(Err(a)) => eprintln!("Network runtime failed: {a:#?}"),
    //     Err(panic) => eprintln!("Network runtime task panicked with {panic:#?}"),
    // }
}

struct Cassowary {
    window: Window,
    to_server: Option<mpsc::UnboundedSender<ClientMsg>>,
    resources: Resources,
}

impl Default for Cassowary {
    fn default() -> Self {
        let mut resources = Resources::default();
        resources.setup();
        Self {
            window: Window::Menu(MenuState::default()),
            to_server: None,
            resources,
        }
    }
}

#[derive(Debug)]
enum CassMessage {
    InitialState,
    Connected(mpsc::UnboundedSender<ClientMsg>),
    ServerMsgRecv(ServerMsg),
    ServerErrRecv(ServerErr),
    CommsError(CommunicationError),
    Menu(MenuMessage),
    Game(GameMessage),
    GoToMenu,
    LoadedImage(String, Handle),
}

fn update(state: &mut Cassowary, message: CassMessage) -> Task<CassMessage> {
    match (message, &mut state.window) {
        (CassMessage::GoToMenu, _) => {
            state.window = Window::Menu(MenuState::default());
            Task::none()
        }
        (CassMessage::InitialState, _) => Task::done(CassMessage::GoToMenu),
        (CassMessage::Menu(message), Window::Menu(menu_state)) => match message {
            MenuMessage::JoinRoom => {
                if let Some(to_server) = &state.to_server {
                    to_server
                        .send(ClientMsg::JoinRoom(menu_state.room_name_input.clone()))
                        .unwrap();
                }
                Task::none()
            }
            MenuMessage::MakeRoom => {
                if let Some(to_server) = &state.to_server {
                    to_server
                        .send(ClientMsg::CreateRoom(menu_state.room_name_input.clone()))
                        .unwrap();
                }
                Task::none()
            }
            MenuMessage::ContentChanged(text) => {
                menu_state.room_name_input = text;
                Task::none()
            }
        },
        (CassMessage::Connected(sender), _) => {
            state.to_server = Some(sender);
            Task::none()
        }
        (CassMessage::ServerMsgRecv(server_msg), Window::Menu(menu_state)) => match server_msg {
            ServerMsg::RoomCreated => Task::none(),
            ServerMsg::JoinedRoom(local_state) => {
                state.window = Window::Game(GameState { game: *local_state });
                Task::none()
            }
            _ => panic!("Game action while out of game"),
        },
        (CassMessage::ServerMsgRecv(server_msg), Window::Game(state)) => todo!(),
        (CassMessage::ServerErrRecv(server_err), _) => {
            println!("{server_err:?}");
            Task::none()
        }
        (CassMessage::CommsError(communication_error), _) => todo!(),
        (CassMessage::Menu(message), Window::Game(state)) => todo!(),
        (CassMessage::Game(message), Window::Menu(state)) => todo!(),
        (CassMessage::Game(message), Window::Game(state)) => match message {
            GameMessage::Unimplemented => todo!(),
            GameMessage::LoadImage(image) => {
                let image2 = image.clone();
                Task::perform(
                    async move { fetch_image(&image2).await.unwrap() },
                    move |x| CassMessage::LoadedImage(image, Handle::from_bytes(x)),
                )
            }
        },
        (CassMessage::LoadedImage(string, handle), _) => {
            state
                .resources
                .textures
                .insert(ImageName::Name(string), handle);
            Task::none()
        }
    }
}

fn view(state: &Cassowary) -> Element<CassMessage> {
    match &state.window {
        Window::Menu(state) => menu_view(state).map(CassMessage::Menu),
        Window::Game(game_state) => game_view(game_state, &state.resources).map(CassMessage::Game),
    }
}

async fn from_local_ping() {
    tokio::time::sleep(Duration::from_secs(10)).await;
}

fn network(state: &Cassowary) -> Subscription<CassMessage> {
    Subscription::run(|| {
        stream::channel(100, async |mut output| {
            let (sender, mut recv) = mpsc::unbounded_channel();
            output.send(CassMessage::Connected(sender)).await.unwrap();
            let addr =
                std::option_env!("CASSIE_SERVER").unwrap_or("ws://cassie.hemolymph.net:3001");
            let uri = Uri::from_static(addr);
            let (mut client, _) = ClientBuilder::from_uri(uri).connect().await.unwrap();

            loop {
                select! {
                    () = from_local_ping() => {
                        client.send(Message::ping("ping!")).await.unwrap()
                    }
                    Some(Ok(msg)) = client.next() => {
                        if msg.is_close() {
                            output.send(CassMessage::CommsError(CommunicationError::Closed)).await.unwrap();
                        }

                        let Some(msg) = msg.as_text() else { continue };

                        let msg = serde_json::from_str::<Result<ServerMsg, ServerErr>>(msg)
                            .map_err(|_| CommunicationError::SerdeReceiveError);

                        match msg {
                            Ok(Ok(ok)) => output.send(CassMessage::ServerMsgRecv(ok)).await.unwrap(),
                            Ok(Err(err)) => output.send(CassMessage::ServerErrRecv(err)).await.unwrap(),
                            Err(err) => output.send(CassMessage::CommsError(err)).await.unwrap(),
                        }
                    },
                    message = recv.recv() => {
                        let Some(message) = message else { panic!("Local to network closed") };
                        let msg = serde_json::to_string_pretty(&message);
                        match msg {
                            Ok(msg) => {client.send(Message::text(msg)).await.unwrap();},/* .map_err(ChannelError::NetworkToServerError)? */
                            Err(_) => {
                                output
                                .send(CassMessage::CommsError(CommunicationError::SerdeSendError)).await.unwrap();
                                // .map_err(|_| ChannelError::NetworkToLocalClosed)?,
                            },
                        };
                    },
                }
            }
        })
    })
}

// fn process_server_error(msg: ServerErr, current_scene: &mut Cassowary) {
//     match msg {
//         ServerErr::RoomDoesntExist(string) => println!("Room doesn't exist"),
//         ServerErr::NotInGame { action } => println!("Not in game, but tried to {action}"),
//         ServerErr::NotInSide => println!("Player is not currently playing"),
//         ServerErr::NoPlayerInSide(side) => println!("Player is not in {side:?}"),
//         ServerErr::NoCardIn(place_from) => println!("No card in {place_from:?}"),
//         ServerErr::SideOccupied(side) => println!("{side:?} is already occupied"),
//         ServerErr::AlreadyInGame { action } => println!("Already in game"),
//         ServerErr::GameIsFull => println!("Game is full"),
//         ServerErr::RoomAlreadyExist => println!("Room alrady exist"),
//     }
// }

// fn process_server_message(
//     msg: ServerMsg,
//     current_scene: &mut Scene,
//     to_server: &UnboundedSender<ClientMsg>,
// ) {
//     let board_state = match current_scene {
//         Scene::LobbySelect(_) => None,
//         Scene::Game(game_data) => Some(&mut game_data.state),
//     };
//     if msg.is_game_action() {
//         let board_state = board_state.unwrap();
//         match msg {
//             ServerMsg::UpdateHand(vec, blood_top, main_top) => {
//                 board_state.local_state.main_deck_top = main_top;
//                 board_state.local_state.blood_deck_top = blood_top;
//                 board_state.hand = vec;
//             }
//             ServerMsg::UpdateSpaces { home_row, away_row } => {
//                 board_state.local_row = *home_row;
//                 board_state.distant_row = *away_row;
//             }
//             ServerMsg::UpdateDiscard(side, vec) => match side {
//                 RelSide::Same => board_state.local_state.discard = vec,
//                 RelSide::Other => board_state.distant_state.discard = vec,
//             },
//             ServerMsg::UpdateTimeline(side, vec) => match side {
//                 RelSide::Same => board_state.local_state.timeline = vec,
//                 RelSide::Other => board_state.distant_state.timeline = vec,
//             },
//             ServerMsg::BeginSearch(vec) => match current_scene {
//                 Scene::LobbySelect(lobby_data) => todo!(),
//                 Scene::Game(game_data) => game_data.seaching = vec,
//             },
//             ServerMsg::UpdateState(new_state) => {
//                 *board_state = *new_state;
//             }
//             ServerMsg::JoinedRoom(..) => panic!("??"),
//             ServerMsg::RoomCreated => panic!("??"),
//         }
//         return;
//     }
//     match msg {
//         ServerMsg::UpdateHand(..) => panic!("??"),
//         ServerMsg::UpdateSpaces { .. } => panic!("??"),
//         ServerMsg::UpdateDiscard(..) => panic!("??"),
//         ServerMsg::UpdateTimeline(..) => panic!("??"),
//         ServerMsg::BeginSearch(..) => panic!("??"),
//         ServerMsg::UpdateState(..) => panic!("??"),
//         ServerMsg::RoomCreated => (),
//         ServerMsg::JoinedRoom(state) => {
//             to_server.send(ClientMsg::PlayAs).unwrap();
//             *current_scene = Scene::Game(GameData {
//                 state: *state,
//                 editing_deck: false,
//                 deck: DeckType::Main,
//                 marrow_main: String::new(),
//                 marrow_blood: String::new(),
//                 marrow_error: String::new(),
//                 seaching: vec![],
//                 creating: String::new(),
//                 viewing_aside: false,
//             })
//         }
//     }
// }

// async fn game_rt(
//     mut from_local_ping: UnboundedReceiver<()>,
//     mut from_local: UnboundedReceiver<ClientMsg>,
//     to_local: UnboundedSender<ComResult<Result<ServerMsg, ServerErr>>>,
// ) -> Result<Never, ChannelError> {
//     let addr = std::option_env!("CASSIE_SERVER").unwrap_or("ws://cassie.hemolymph.net:3001");
//     let uri = Uri::from_static(addr);
//     let (mut client, _) = ClientBuilder::from_uri(uri).connect().await.unwrap();

//     loop {
//         select! {
//             Some(()) = from_local_ping.recv() => {
//                 client.send(Message::ping("ping!")).await.unwrap()
//             }
//             Some(Ok(msg)) = client.next() => {
//                 if msg.is_close() {
//                     to_local.send(Err(CommunicationError::Closed)).unwrap();
//                 }

//                 let Some(msg) = msg.as_text() else { continue };

//                 let msg = serde_json::from_str::<Result<ServerMsg, ServerErr>>(msg)
//                     .map_err(|_| CommunicationError::SerdeReceiveError);

//                 to_local.send(msg).map_err(|_| ChannelError::NetworkToLocalClosed)?;
//             },
//             message = from_local.recv() => {
//                 let Some(message) = message else { return Err(ChannelError::LocalToNetworkClosed) };
//                 let msg = serde_json::to_string_pretty(&message);
//                 match msg {
//                     Ok(msg) => client.send(Message::text(msg)).await.map_err(ChannelError::NetworkToServerError)?,
//                     Err(_) => to_local
//                         .send(Err(CommunicationError::SerdeSendError))
//                         .map_err(|_| ChannelError::NetworkToLocalClosed)?,
//                 }
//             },
//         }
//     }
// }

const SIDEBAR_PADDING: f32 = 5.0;
const CARD_WIDTH: f32 = 100.0;
const CARD_HEIGHT: f32 = CARD_WIDTH * (3.5 / 2.5);
const SIDEBAR_WIDTH: f32 = SIDEBAR_PADDING * 2. + CARD_WIDTH;
const HANDBAR_HEIGHT: f32 = SIDEBAR_PADDING * 2. + CARD_HEIGHT;

fn get_filegarden_link(name: &str) -> String {
    format!(
        "https://file.garden/ZJSEzoaUL3bz8vYK/bloodlesscards/{}.png",
        name.replace(' ', "").replace('ä', "a")
    )
}
