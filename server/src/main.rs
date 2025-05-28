use rand::{rng, seq::SliceRandom};
use serde_json::to_string_pretty;
use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    sync::Arc,
};
use tokio::{
    net::TcpStream,
    select,
    sync::{
        RwLock, RwLockMappedWriteGuard, RwLockReadGuard, RwLockWriteGuard,
        broadcast::{self},
        mpsc,
    },
};

use futures::{SinkExt, StreamExt, future::OptionFuture};
use shared::{Card, ClientMsg, DeckType, GameState, PlayerState, ServerErr, ServerMsg, Side};
use tokio::net::TcpListener;
use tokio_websockets::{Message, ServerBuilder, WebSocketStream};

#[derive(Clone, Hash, PartialEq, Eq)]
struct GameId(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PlayerId(SocketAddr);

impl GameId {
    pub fn new() -> Self {
        Self("aaaa".to_string())
    }
}

type Global<T> = Arc<RwLock<T>>;
type Players = Global<HashMap<PlayerId, Player>>;
type Games = Global<HashMap<GameId, GameHandle>>;

#[derive(Debug)]
struct Game {
    home_player: Option<PlayerId>,
    away_player: Option<PlayerId>,
    spectators: Vec<PlayerId>,
    state: GameState,
    owner: PlayerId,
}

impl Game {
    fn get_player(&self, side: Side) -> Option<PlayerId> {
        match side {
            Side::Home => self.home_player,
            Side::Away => self.away_player,
        }
    }
    fn get_state(&self, id: PlayerId) -> Option<&PlayerState> {
        if self.home_player.is_some_and(|x| x == id) {
            Some(&self.state.home_state)
        } else if self.away_player.is_some_and(|x| x == id) {
            Some(&self.state.away_state)
        } else {
            None
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
    fn get_from_side(&self, side: Side) -> Option<&PlayerState> {
        match side {
            Side::Home => Some(&self.state.home_state),
            Side::Away => Some(&self.state.away_state),
        }
    }
    fn get_from_side_mut(&mut self, side: Side) -> Option<&mut PlayerState> {
        match side {
            Side::Home => Some(&mut self.state.home_state),
            Side::Away => Some(&mut self.state.away_state),
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
    fn get_other_state_mut(&mut self, id: &PlayerId) -> Option<&mut PlayerState> {
        if self.home_player.is_some_and(|x| x == *id) {
            Some(&mut self.state.away_state)
        } else if self.away_player.is_some_and(|x| x == *id) {
            Some(&mut self.state.home_state)
        } else {
            None
        }
    }
}

struct Player {
    game: Option<GameId>,
    name: String,
}

struct GameHandle {
    to_game: mpsc::UnboundedSender<AuthoredClientMsg>,
    game_broadcast: broadcast::Sender<DestinedServerMsg>,
}

struct PlayerGameHandle {
    to_game: mpsc::UnboundedSender<AuthoredClientMsg>,
    game_broadcast: broadcast::Receiver<DestinedServerMsg>,
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();

    let players = Players::default();
    let games = Games::default();
    while let Ok((stream, addr)) = listener.accept().await {
        let (_request, ws_stream) = ServerBuilder::new().accept(stream).await.unwrap();

        let player = Player {
            game: None,
            name: String::from("Not yet named"),
        };
        let player_id = PlayerId(addr);

        players.write().await.insert(player_id, player);

        let games = games.clone();
        let players = players.clone();
        tokio::spawn(player_task(player_id, ws_stream, players, games));
    }
}

struct AuthoredClientMsg {
    author: PlayerId,
    message: MaybeUpdate,
}

#[derive(Clone, Debug)]
enum MaybeUpdate {
    Update,
    Msg(ClientMsg),
}

impl From<ClientMsg> for MaybeUpdate {
    fn from(value: ClientMsg) -> Self {
        Self::Msg(value)
    }
}

#[derive(Clone, Copy, Debug)]
enum Destination {
    All,
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
) -> Option<Result<ServerMsg, ServerErr>> {
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
            return Some(Err(ServerErr::NotInGame {
                action: msg.get_name().to_owned(),
            }));
        }

        return None;
    }

    match msg {
        ClientMsg::JoinRoom(string) => {
            let games = games.read().await;
            let game_handle = games.get(&GameId(string.clone()));
            *current_game_handle = game_handle.map(|x| PlayerGameHandle {
                to_game: x.to_game.clone(),
                game_broadcast: x.game_broadcast.subscribe(),
            });
            drop(games);
            match current_game_handle {
                Some(game_handle) => {
                    game_handle
                        .to_game
                        .send(MaybeUpdate::Update.sent_by(player_id))
                        .unwrap();
                    game_handle
                        .to_game
                        .send(ClientMsg::Update.sent_by(player_id))
                        .unwrap();
                }
                None => return Some(Err(ServerErr::RoomDoesntExist(string))),
            }

            None
        }
        ClientMsg::CreateRoom => {
            let (to_game, from_player) = mpsc::unbounded_channel();
            let (to_players, from_game) = broadcast::channel(16);
            let handle = GameHandle {
                to_game: to_game.clone(),
                game_broadcast: to_players.clone(),
            };
            games.write().await.insert(GameId::new(), handle);
            tokio::spawn(room_task(player_id, from_player, to_players));
            *current_game_handle = Some(PlayerGameHandle {
                to_game,
                game_broadcast: from_game,
            });

            Some(Ok(ServerMsg::RoomCreated))
        }
        ClientMsg::Draw(..) => None,
        ClientMsg::Move { .. } => None,
        ClientMsg::Shuffle(..) => None,
        ClientMsg::RequestSearch => todo!(),
        ClientMsg::Update => None,
        ClientMsg::SetDeck(..) => None,
        ClientMsg::PlayAs => None,
    }
}

async fn player_task(
    player_id: PlayerId,
    mut ws_stream: WebSocketStream<TcpStream>,
    players: Players,
    games: Games,
) {
    let mut current_game_handle: Option<PlayerGameHandle> = None;
    loop {
        let broadcast_fut: OptionFuture<_> = match &mut current_game_handle {
            Some(a) => Some(a.game_broadcast.recv()).into(),
            None => None.into(),
        };
        select! {
            Some(Ok(msg)) = broadcast_fut => {
                if let Destination::Player(recv_id) = msg.author {
                    if player_id != recv_id {
                        continue
                    }
                }

                ws_stream.send(Message::text(to_string_pretty(&msg.message).unwrap())).await.unwrap();
            },
            Some(Ok(msg)) = ws_stream.next() => {
                let Some(result) = after_stream_next(player_id, msg, &games, &mut current_game_handle).await else { continue };
            },
        }
    }
}

async fn room_task(
    creator: PlayerId,
    mut from_player: mpsc::UnboundedReceiver<AuthoredClientMsg>,
    to_players: broadcast::Sender<DestinedServerMsg>,
) {
    let mut game = Game {
        home_player: None,
        away_player: None,
        spectators: vec![],
        state: GameState::default(),
        owner: creator,
    };
    to_players
        .send(ServerMsg::JoinedRoom(Box::new(game.state.create_local_for(None))).to_player(creator))
        .unwrap();
    loop {
        match from_player.recv().await {
            Some(msg) => {
                let author_side = game.get_side(msg.author);
                let message = match msg.message {
                    MaybeUpdate::Update => {
                        to_players
                            .send(
                                ServerMsg::JoinedRoom(Box::new(
                                    game.state.create_local_for(author_side),
                                ))
                                .to_player(msg.author),
                            )
                            .unwrap();

                        continue;
                    }
                    MaybeUpdate::Msg(msg) => msg,
                };
                match message {
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

                        let hand = local_state.hand.clone();

                        to_players
                            .send(ServerMsg::UpdateHand(hand).to_player(msg.author))
                            .unwrap();
                    }
                    ClientMsg::Move { from, to } => {
                        let Some(player) = game.get_state_mut(&msg.author) else {
                            to_players
                                .send(ServerErr::NotInSide.to_player(msg.author))
                                .unwrap();
                            continue;
                        };
                        let Some(local_side) = author_side else {
                            to_players
                                .send(ServerErr::NotInSide.to_player(msg.author))
                                .unwrap();
                            continue;
                        };
                        let card = match from {
                            shared::PlaceFrom::Hand(idx) => player.hand.safe_remove(idx),
                            shared::PlaceFrom::Space(side, idx) => {
                                game.state.get_row_mut(side.make_real(local_side))[idx]
                                    .take()
                                    .map(|x| x.name)
                            }
                            shared::PlaceFrom::Discard(side, idx) => game
                                .state
                                .get_state_mut(side.make_real(local_side))
                                .discard
                                .safe_remove(idx),
                            shared::PlaceFrom::Aside(idx) => todo!("Aside is not yet implemented"),
                            shared::PlaceFrom::Timeline(side, idx) => game
                                .state
                                .get_state_mut(side.make_real(local_side))
                                .timeline
                                .safe_remove(idx),
                            shared::PlaceFrom::Deck(side, deck_type, idx) => {
                                let side = side.make_real(local_side);
                                let Some(player) = game.get_from_side_mut(side) else {
                                    to_players
                                        .send(ServerErr::NoPlayerInSide(side).to_all())
                                        .unwrap();
                                    continue;
                                };
                                match deck_type {
                                    DeckType::Blood => player.blood_deck.remove(idx),
                                    DeckType::Main => player.main_deck.remove(idx),
                                }
                            }
                        };

                        let Some(card) = card else {
                            to_players
                                .send(ServerErr::NoCardIn(from).to_player(msg.author))
                                .unwrap();
                            continue;
                        };

                        let player = game
                            .get_state_mut(&msg.author)
                            .expect("Player is already known to exist from before");
                        match to {
                            shared::PlaceTo::Hand => player.hand.push(card),
                            shared::PlaceTo::Space(side, space, flipped) => {
                                game.state.get_row_mut(side.make_real(local_side))[space] =
                                    Some(Card {
                                        name: card,
                                        backside: flipped,
                                    })
                            }
                            shared::PlaceTo::Discard(side) => game
                                .state
                                .get_state_mut(side.make_real(local_side))
                                .discard
                                .push(card),
                            shared::PlaceTo::Aside => todo!("Aside is not yet implemented"),
                            shared::PlaceTo::Timeline(side) => game
                                .state
                                .get_state_mut(side.make_real(local_side))
                                .timeline
                                .push(card),
                            shared::PlaceTo::Deck(deck_to, side, deck_type) => {
                                let side = side.make_real(local_side);
                                let Some(player) = game.get_from_side_mut(side) else {
                                    to_players
                                        .send(ServerErr::NoPlayerInSide(side).to_player(msg.author))
                                        .unwrap();
                                    continue;
                                };
                                let deck = match deck_type {
                                    DeckType::Blood => &mut player.blood_deck,
                                    DeckType::Main => &mut player.main_deck,
                                };
                                match deck_to {
                                    shared::DeckTo::Top => deck.push_front(card),
                                    shared::DeckTo::Bottom => deck.push_back(card),
                                }
                            }
                            shared::PlaceTo::Liberate => (), // Do nothing. The card was removed earlier. Don't put it anywhere
                        }

                        to_players
                            .send(
                                ServerMsg::UpdateState(Box::new(
                                    game.state.create_local_for(Some(local_side)),
                                ))
                                .to_player(msg.author),
                            )
                            .unwrap();

                        if let Some(other) = game.get_player(local_side.opposite()) {
                            to_players
                                .send(
                                    ServerMsg::UpdateState(Box::new(
                                        game.state.create_local_for(Some(local_side.opposite())),
                                    ))
                                    .to_player(other),
                                )
                                .unwrap();
                        }

                        // for player in &game.spectators {
                        //     to_players
                        //         .send(
                        //             ServerMsg::UpdateState(Box::new(
                        //                 game.state.create_local_for(None),
                        //             ))
                        //             .to_player(*player),
                        //         )
                        //         .unwrap();
                        // }
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
                    ClientMsg::RequestSearch => todo!(),
                    ClientMsg::Update => {
                        to_players
                            .send(
                                ServerMsg::UpdateState(Box::new(
                                    game.state.create_local_for(author_side),
                                ))
                                .to_player(msg.author),
                            )
                            .unwrap();
                    }
                    ClientMsg::SetDeck(deck, contents) => {
                        let Some(state) = game.get_state_mut(&msg.author) else {
                            to_players
                                .send(ServerErr::NotInSide.to_player(msg.author))
                                .unwrap();
                            continue;
                        };

                        match deck {
                            DeckType::Blood => state.blood_deck = contents,
                            DeckType::Main => state.main_deck = contents,
                        }
                    }
                    ClientMsg::PlayAs => {
                        let mut author_side = author_side;
                        if game.home_player.is_none() {
                            game.home_player = Some(msg.author);
                            author_side = Some(Side::Home);
                        } else if game.away_player.is_none() {
                            game.away_player = Some(msg.author);
                            author_side = Some(Side::Away);
                        } else {
                            to_players
                                .send(ServerErr::GameIsFull.to_player(msg.author))
                                .unwrap();
                        }

                        to_players
                            .send(
                                ServerMsg::UpdateState(Box::new(
                                    game.state.create_local_for(author_side),
                                ))
                                .to_player(msg.author),
                            )
                            .unwrap();
                    }
                    ClientMsg::CreateRoom => {
                        to_players
                            .send(
                                ServerErr::AlreadyInGame {
                                    action: message.get_name().to_string(),
                                }
                                .to_player(msg.author),
                            )
                            .unwrap();
                    }
                    ClientMsg::JoinRoom(_) => {
                        to_players
                            .send(
                                ServerErr::AlreadyInGame {
                                    action: message.get_name().to_string(),
                                }
                                .to_player(msg.author),
                            )
                            .unwrap();
                    }
                }
            }
            None => todo!("cave.ogg"),
        }
    }
}

trait SafeRemove<T> {
    fn safe_remove(&mut self, idx: usize) -> Option<T>;
}

impl<T> SafeRemove<T> for Vec<T> {
    fn safe_remove(&mut self, idx: usize) -> Option<T> {
        if idx < self.len() {
            Some(self.remove(idx))
        } else {
            None
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
            message: MaybeUpdate::Msg(self),
        }
    }
}

impl FromPlayer for MaybeUpdate {
    fn sent_by(self, player: PlayerId) -> AuthoredClientMsg {
        AuthoredClientMsg {
            author: player,
            message: self,
        }
    }
}

trait ToPlayer {
    fn to_player(self, player: PlayerId) -> DestinedServerMsg;
    fn to_all(self) -> DestinedServerMsg;
}

impl ToPlayer for ServerMsg {
    fn to_player(self, player: PlayerId) -> DestinedServerMsg {
        DestinedServerMsg {
            author: Destination::Player(player),
            message: Ok(self),
        }
    }

    fn to_all(self) -> DestinedServerMsg {
        DestinedServerMsg {
            author: Destination::All,
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

    fn to_all(self) -> DestinedServerMsg {
        DestinedServerMsg {
            author: Destination::All,
            message: Err(self),
        }
    }
}
