use rand::{rng, seq::SliceRandom};
use serde_json::to_string_pretty;
use shared::Find;
use shared::{CardId, CardOrName, CardOrNameMut, NamedCardId};
use std::sync::Weak;
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    net::SocketAddr,
    sync::Arc,
};
use tokio::sync::broadcast::error::RecvError;
use tokio::task::JoinHandle;
use tokio::{
    net::TcpStream,
    select,
    sync::{
        RwLock,
        broadcast::{self},
        mpsc,
    },
};

use futures::{SinkExt, StreamExt, future::OptionFuture};
use shared::{ClientMsg, DeckType, GameState, PlayerState, ServerErr, ServerMsg, Side};
use tokio::net::TcpListener;
use tokio_websockets::{Message, ServerBuilder, WebSocketStream};

#[derive(Clone, Hash, PartialEq, Eq)]
struct GameId(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PlayerId(SocketAddr);

type Global<T> = Arc<RwLock<T>>;
/// Using a Weak pointer since all the player tasks will be holding Arc pointers anyways.
/// The Games struct should not be able to hold the GameHandle. Once all player tasks leave
/// the game room should be dropped. This ensures that.
type Games = Global<HashMap<GameId, Weak<GameHandle>>>;

#[derive(Debug)]
struct Game {
    next_id: usize,
    cards: BTreeMap<CardId, String>,
    home_player: Option<PlayerId>,
    away_player: Option<PlayerId>,
    spectators: Vec<PlayerId>,
    state: GameState,
}

impl Game {
    fn update_all(&self, to_players: &broadcast::Sender<DestinedServerMsg>) {
        if let Some(player) = self.home_player {
            to_players
                .send(
                    ServerMsg::UpdateState(Box::new(
                        self.state.create_local_for(Some(Side::Home), &self.cards),
                    ))
                    .to_player(player),
                )
                .unwrap();
        }

        if let Some(player) = self.away_player {
            to_players
                .send(
                    ServerMsg::UpdateState(Box::new(
                        self.state.create_local_for(Some(Side::Away), &self.cards),
                    ))
                    .to_player(player),
                )
                .unwrap();
        }

        for player in &self.spectators {
            to_players
                .send(
                    ServerMsg::UpdateState(Box::new(
                        self.state.create_local_for(Some(Side::Home), &self.cards),
                    ))
                    .to_player(*player),
                )
                .unwrap();
        }
    }
    fn is_desolate(&self) -> bool {
        self.home_player.is_none() && self.away_player.is_none() && self.spectators.is_empty()
    }
    fn add_card(&mut self, card: String) -> CardId {
        let id = CardId(self.next_id);
        self.cards.insert(id, card);
        self.next_id += 1;
        id
    }
    fn get_player(&self, side: Side) -> Option<PlayerId> {
        match side {
            Side::Home => self.home_player,
            Side::Away => self.away_player,
        }
    }
    fn get_side(&self, id: PlayerId) -> Option<Side> {
        if self.home_player.is_some_and(|x| x == id) {
            Some(Side::Home)
        } else if self.away_player.is_some_and(|x| x == id) {
            Some(Side::Away)
        } else {
            None
        }
    }
    fn get_state_mut(&mut self, id: &PlayerId) -> Option<&mut PlayerState> {
        if self.home_player.is_some_and(|x| x == *id) {
            Some(&mut self.state.home_state)
        } else if self.away_player.is_some_and(|x| x == *id) {
            Some(&mut self.state.away_state)
        } else {
            None
        }
    }
}

struct GameHandle {
    to_game: mpsc::UnboundedSender<AuthoredClientMsg>,
    game_broadcast: broadcast::Sender<DestinedServerMsg>,
}

struct PlayerGameHandle {
    _game: Arc<GameHandle>,
    to_game: mpsc::UnboundedSender<AuthoredClientMsg>,
    game_broadcast: broadcast::Receiver<DestinedServerMsg>,
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.2:3000").await.unwrap();
    let mut tasks = vec![];

    let games = Games::default();
    while let Ok((stream, addr)) = listener.accept().await {
        let (_request, ws_stream) = ServerBuilder::new().accept(stream).await.unwrap();

        let player_id = PlayerId(addr);

        let games = games.clone();
        tasks.push(tokio::spawn(player_task(player_id, ws_stream, games)));
    }

    let join_all = futures::future::join_all(tasks).await;

    let mut tasks = vec![];

    for x in join_all {
        match x {
            Ok(mut x) => tasks.append(&mut x),
            Err(x) => eprintln!("Player task failed with: {x:#?}"),
        }
    }

    let join_all = futures::future::join_all(tasks).await;

    for x in join_all {
        match x {
            Ok(()) => (),
            Err(x) => eprintln!("Player subtask failed with: {x:#?}"),
        }
    }
}

struct AuthoredClientMsg {
    author: PlayerId,
    message: ClientMsg,
}

#[derive(Clone, Copy, Debug)]
enum Destination {
    Player(PlayerId),
}

#[derive(Clone, Debug)]
struct DestinedServerMsg {
    author: Destination,
    message: Result<ServerMsg, ServerErr>,
}

/// This function returns a message to be sent to the client
async fn after_stream_next(
    player_id: PlayerId,
    msg: Message,
    games: &Games,
    current_game_handle: &mut Option<PlayerGameHandle>,
) -> Option<(Result<ServerMsg, ServerErr>, Option<JoinHandle<()>>)> {
    if msg.is_close() {
        return None;
    }
    let msg = msg.as_text()?;
    let msg = match serde_json::from_str::<ClientMsg>(msg) {
        Ok(a) => a,
        Err(_) => todo!("Couldn't parse msg from client"),
    };

    if msg.is_game_action() {
        if let Some(current_game_handle) = current_game_handle {
            current_game_handle
                .to_game
                .send(msg.sent_by(player_id))
                .unwrap();
        } else {
            return Some((
                Err(ServerErr::NotInGame {
                    action: msg.get_name().to_owned(),
                }),
                None,
            ));
        }

        return None;
    }

    match msg {
        ClientMsg::JoinRoom(string) => {
            let games = games.read().await;
            let game_handle = games.get(&GameId(string.clone())).and_then(|x| x.upgrade());
            *current_game_handle = game_handle.map(|x| PlayerGameHandle {
                to_game: x.to_game.clone(),
                game_broadcast: x.game_broadcast.subscribe(),
                _game: x,
            });
            drop(games);
            match current_game_handle {
                Some(game_handle) => {
                    game_handle
                        .to_game
                        .send(ClientMsg::JoinRoom(string).sent_by(player_id))
                        .unwrap();
                }
                None => return Some((Err(ServerErr::RoomDoesntExist(string)), None)),
            }

            None
        }
        ClientMsg::CreateRoom(room) => {
            let (to_game, from_player) = mpsc::unbounded_channel();
            let (to_players, from_game) = broadcast::channel(16);
            let handle = GameHandle {
                to_game: to_game.clone(),
                game_broadcast: to_players.clone(),
            };
            let mut games = games.write().await;
            if games
                .get(&GameId(room.clone()))
                .and_then(|x| x.upgrade())
                .is_some()
            {
                return Some((Err(ServerErr::RoomAlreadyExist), None));
            }
            let handle = Arc::new(handle);
            let weak_handle = Arc::downgrade(&handle);
            games.insert(GameId(room.clone()), weak_handle);
            let task = tokio::spawn(room_task(room, player_id, from_player, to_players));
            *current_game_handle = Some(PlayerGameHandle {
                to_game,
                game_broadcast: from_game,
                _game: handle,
            });

            Some((Ok(ServerMsg::RoomCreated), Some(task)))
        }
        ClientMsg::RequestSearch(..) => None,
        ClientMsg::Draw(..) => None,
        ClientMsg::Move { .. } => None,
        ClientMsg::Shuffle(..) => None,
        ClientMsg::Update => None,
        ClientMsg::SetDeck(..) => None,
        ClientMsg::PlayAs => None,
        ClientMsg::AddCounter(..) => None,
        ClientMsg::CreateCounter(..) => None,
        ClientMsg::FinishSearch => None,
        ClientMsg::LeaveRoom => None,
        ClientMsg::AddBlood(..) => None,
        ClientMsg::EndTurn => None,
        ClientMsg::AddHealth(..) => None,
        ClientMsg::CreateCard(..) => None,
    }
}

async fn player_task(
    player_id: PlayerId,
    mut ws_stream: WebSocketStream<TcpStream>,
    games: Games,
) -> Vec<JoinHandle<()>> {
    let mut current_game_handle: Option<PlayerGameHandle> = None;
    let mut tasks = vec![];
    loop {
        let broadcast_fut: OptionFuture<_> = match &mut current_game_handle {
            Some(a) => Some(a.game_broadcast.recv()).into(),
            None => None.into(),
        };
        select! {
            Some(msg) = broadcast_fut => {
                match msg {
                    Ok(msg) => {
                        let Destination::Player(recv_id) = msg.author;
                        if player_id != recv_id {
                            continue
                        }

                        ws_stream.send(Message::text(to_string_pretty(&msg.message).unwrap())).await.unwrap();
                    },
                    Err(RecvError::Closed) => {
                        current_game_handle = None;
                    },
                    Err(RecvError::Lagged(..)) => match &mut current_game_handle {
                        Some(a) => {
                            a.to_game.send(ClientMsg::Update.sent_by(player_id)).unwrap()
                        },
                        None => continue,
                    },
                }
            },
            msg = ws_stream.next() => {
                match msg {
                    Some(Ok(msg)) => {
                        if msg.is_close() {
                            eprintln!("Received close message");
                            break;
                        }
                        let Some((result, task)) = after_stream_next(player_id, msg, &games, &mut current_game_handle).await else { continue };
                        ws_stream.send(Message::text(to_string_pretty(&result).unwrap())).await.unwrap();
                        if let Some(task) = task {
                            tasks.push(task);
                        }
                    },
                    None => {
                        eprintln!("Safe disconnection happened");
                        break
                    },
                    Some(Err(err)) => match &current_game_handle {
                        Some(x) => {
                            eprintln!("Player connection failed with {err:#?}");
                            x.to_game.send(ClientMsg::LeaveRoom.sent_by(player_id)).unwrap();
                            break;
                        },
                        None => {
                            eprintln!("2 Player connection failed with {err:#?}");
                            break;
                        },
                    },
                }
            },
        }
    }

    eprintln!("Player task died");

    tasks
}

async fn room_task(
    id: String,
    creator: PlayerId,
    mut from_player: mpsc::UnboundedReceiver<AuthoredClientMsg>,
    to_players: broadcast::Sender<DestinedServerMsg>,
) {
    let mut game = Game {
        next_id: 0,
        cards: BTreeMap::new(),
        home_player: None,
        away_player: None,
        spectators: vec![],
        state: GameState::default(),
    };

    to_players
        .send(
            ServerMsg::JoinedRoom(Box::new(game.state.create_local_for(None, &game.cards)))
                .to_player(creator),
        )
        .unwrap();
    loop {
        match from_player.recv().await {
            Some(msg) => {
                let author_side = game.get_side(msg.author);
                match msg.message {
                    ClientMsg::Draw(deck_owner, which_deck) => {
                        let Some(local_side) = author_side else {
                            to_players
                                .send(ServerErr::NotInSide.to_player(msg.author))
                                .unwrap();
                            continue;
                        };
                        let draw_from = game.state.get_state_mut(deck_owner.make_real(local_side));

                        let Some(card) = draw_from.get_deck_mut(which_deck).pop_back() else {
                            continue;
                        };

                        let local_state = game.state.get_state_mut(local_side);

                        local_state.hand.push(card);

                        game.update_all(&to_players);
                    }
                    ClientMsg::Move { from, to } => {
                        let Some(local_side) = author_side else {
                            to_players
                                .send(ServerErr::NotInSide.to_player(msg.author))
                                .unwrap();
                            continue;
                        };

                        let card: Option<CardOrName> = game.state.pop_card(from, local_side);

                        let Some(card) = card else {
                            to_players
                                .send(ServerErr::NoCardIn(from).to_player(msg.author))
                                .unwrap();
                            continue;
                        };

                        game.state.push_card(card, to, local_side);

                        if game.state.get_state(local_side).searching.is_some() {
                            to_players
                                .send(
                                    ServerMsg::BeginSearch(
                                        game.state
                                            .get_state(local_side)
                                            .main_deck
                                            .iter()
                                            .map(|id| NamedCardId {
                                                id: *id,
                                                name: game.cards.get(id).unwrap().clone(),
                                            })
                                            .collect(),
                                    )
                                    .to_player(msg.author),
                                )
                                .unwrap();
                        }

                        to_players
                            .send(
                                ServerMsg::UpdateState(Box::new(
                                    game.state.create_local_for(Some(local_side), &game.cards),
                                ))
                                .to_player(msg.author),
                            )
                            .unwrap();

                        if let Some(other) = game.get_player(local_side.opposite()) {
                            to_players
                                .send(
                                    ServerMsg::UpdateState(Box::new(game.state.create_local_for(
                                        Some(local_side.opposite()),
                                        &game.cards,
                                    )))
                                    .to_player(other),
                                )
                                .unwrap();
                        }
                    }
                    ClientMsg::Shuffle(deck) => {
                        let Some(state) = game.get_state_mut(&msg.author) else {
                            to_players
                                .send(ServerErr::NotInSide.to_player(msg.author))
                                .unwrap();
                            continue;
                        };

                        match deck {
                            DeckType::Blood => {
                                let mut deck = Vec::from(state.blood_deck.clone());
                                deck.shuffle(&mut rng());
                                state.blood_deck = VecDeque::from(deck);
                            }
                            DeckType::Main => {
                                let mut deck = Vec::from(state.main_deck.clone());
                                deck.shuffle(&mut rng());
                                state.main_deck = VecDeque::from(deck);
                            }
                        }
                    }
                    ClientMsg::RequestSearch(deck) => {
                        let Some(local_side) = author_side else {
                            to_players
                                .send(ServerErr::NotInSide.to_player(msg.author))
                                .unwrap();
                            continue;
                        };

                        game.state.get_state_mut(local_side).searching = Some(deck);

                        to_players
                            .send(
                                ServerMsg::BeginSearch(
                                    game.state
                                        .get_state(local_side)
                                        .main_deck
                                        .iter()
                                        .map(|id| NamedCardId {
                                            id: *id,
                                            name: game.cards.get(id).unwrap().clone(),
                                        })
                                        .collect(),
                                )
                                .to_player(msg.author),
                            )
                            .unwrap();
                    }
                    ClientMsg::Update => {
                        to_players
                            .send(
                                ServerMsg::UpdateState(Box::new(
                                    game.state.create_local_for(author_side, &game.cards),
                                ))
                                .to_player(msg.author),
                            )
                            .unwrap();
                    }
                    ClientMsg::SetDeck(deck, contents) => {
                        let Some(local_side) = author_side else {
                            to_players
                                .send(ServerErr::NotInSide.to_player(msg.author))
                                .unwrap();
                            continue;
                        };

                        let contents = contents.into_iter().map(|x| game.add_card(x)).collect();

                        let state = game.state.get_state_mut(local_side);

                        match deck {
                            DeckType::Blood => state.blood_deck = contents,
                            DeckType::Main => state.main_deck = contents,
                        }

                        to_players
                            .send(
                                ServerMsg::UpdateState(Box::new(
                                    game.state.create_local_for(Some(local_side), &game.cards),
                                ))
                                .to_player(msg.author),
                            )
                            .unwrap();

                        if let Some(other) = game.get_player(local_side.opposite()) {
                            to_players
                                .send(
                                    ServerMsg::UpdateState(Box::new(game.state.create_local_for(
                                        Some(local_side.opposite()),
                                        &game.cards,
                                    )))
                                    .to_player(other),
                                )
                                .unwrap();
                        }
                    }
                    ClientMsg::PlayAs => {
                        let mut author_side = author_side;
                        game.spectators.find_remove(msg.author);
                        if game.home_player.is_none() {
                            game.home_player = Some(msg.author);
                            author_side = Some(Side::Home);
                        } else if game.away_player.is_none() {
                            game.away_player = Some(msg.author);
                            author_side = Some(Side::Away);
                        } else {
                            game.spectators.push(msg.author);
                            to_players
                                .send(ServerErr::GameIsFull.to_player(msg.author))
                                .unwrap();
                        }

                        to_players
                            .send(
                                ServerMsg::UpdateState(Box::new(
                                    game.state.create_local_for(author_side, &game.cards),
                                ))
                                .to_player(msg.author),
                            )
                            .unwrap();
                    }
                    ClientMsg::CreateRoom(..) => {
                        to_players
                            .send(
                                ServerErr::AlreadyInGame {
                                    action: msg.message.get_name().to_string(),
                                }
                                .to_player(msg.author),
                            )
                            .unwrap();
                    }
                    ClientMsg::JoinRoom(ref room) => {
                        if *room == id {
                            game.spectators.push(msg.author);
                            to_players
                                .send(
                                    ServerMsg::JoinedRoom(Box::new(
                                        game.state.create_local_for(author_side, &game.cards),
                                    ))
                                    .to_player(msg.author),
                                )
                                .unwrap();
                            to_players
                                .send(
                                    ServerMsg::UpdateState(Box::new(
                                        game.state.create_local_for(None, &game.cards),
                                    ))
                                    .to_player(msg.author),
                                )
                                .unwrap();
                            continue;
                        }
                        to_players
                            .send(
                                ServerErr::AlreadyInGame {
                                    action: msg.message.get_name().to_string(),
                                }
                                .to_player(msg.author),
                            )
                            .unwrap();
                    }
                    ClientMsg::AddCounter(from, counter, up) => {
                        let Some(local_side) = author_side else {
                            to_players
                                .send(ServerErr::NotInSide.to_player(msg.author))
                                .unwrap();
                            continue;
                        };
                        let Some(CardOrNameMut::Card(card)) =
                            game.state.get_card_mut(from, local_side)
                        else {
                            panic!();
                        };

                        let add = if up { 1 } else { -1 };

                        let num = card.counters.entry(counter).or_insert(0);
                        *num = num.saturating_add_signed(add);

                        game.update_all(&to_players);
                    }
                    ClientMsg::CreateCounter(from, counter) => {
                        let Some(local_side) = author_side else {
                            to_players
                                .send(ServerErr::NotInSide.to_player(msg.author))
                                .unwrap();
                            continue;
                        };
                        let Some(CardOrNameMut::Card(card)) =
                            game.state.get_card_mut(from, local_side)
                        else {
                            panic!();
                        };

                        card.counters.entry(counter).or_insert(0);
                    }
                    ClientMsg::FinishSearch => {
                        let Some(local_side) = author_side else {
                            to_players
                                .send(ServerErr::NotInSide.to_player(msg.author))
                                .unwrap();
                            continue;
                        };
                        game.state.get_state_mut(local_side).searching = None;
                    }
                    ClientMsg::LeaveRoom => {
                        println!("Player is found to have left and the room is processing that.");
                        match author_side {
                            Some(Side::Home) => game.home_player = None,
                            Some(Side::Away) => game.away_player = None,
                            None => (),
                        }

                        game.spectators.find_remove(msg.author);

                        if game.is_desolate() {
                            println!("{game:#?}");
                            println!("Room is desolate.");
                            break;
                        }
                    }
                    ClientMsg::AddBlood(rel_side, up) => {
                        let Some(local_side) = author_side else {
                            to_players
                                .send(ServerErr::NotInSide.to_player(msg.author))
                                .unwrap();
                            continue;
                        };

                        let side = rel_side.make_real(local_side);

                        let blood = game.state.get_state_mut(side).blood;
                        if up {
                            game.state.get_state_mut(side).blood = blood.saturating_add(1);
                        } else {
                            game.state.get_state_mut(side).blood = blood.saturating_sub(1);
                        }
                        game.update_all(&to_players);
                    }
                    ClientMsg::EndTurn => todo!(),
                    ClientMsg::AddHealth(up) => {
                        let health = game.state.health;
                        if up {
                            game.state.health = health.saturating_add(1);
                        } else {
                            game.state.health = health.saturating_sub(1);
                        }
                        game.update_all(&to_players);
                    }
                    ClientMsg::CreateCard(card) => {
                        let Some(local_side) = author_side else {
                            to_players
                                .send(ServerErr::NotInSide.to_player(msg.author))
                                .unwrap();
                            continue;
                        };

                        let card = game.add_card(card);
                        let state = game.state.get_state_mut(local_side);
                        state.hand.push(card);

                        game.update_all(&to_players);
                    }
                }
            }
            None => panic!("cave.ogg"),
        }
    }
}

trait FromPlayer {
    fn sent_by(self, player: PlayerId) -> AuthoredClientMsg;
}

impl FromPlayer for ClientMsg {
    fn sent_by(self, player: PlayerId) -> AuthoredClientMsg {
        AuthoredClientMsg {
            author: player,
            message: self,
        }
    }
}

trait ToPlayer {
    fn to_player(self, player: PlayerId) -> DestinedServerMsg;
}

impl ToPlayer for ServerMsg {
    fn to_player(self, player: PlayerId) -> DestinedServerMsg {
        DestinedServerMsg {
            author: Destination::Player(player),
            message: Ok(self),
        }
    }
}

impl ToPlayer for ServerErr {
    fn to_player(self, player: PlayerId) -> DestinedServerMsg {
        DestinedServerMsg {
            author: Destination::Player(player),
            message: Err(self),
        }
    }
}
