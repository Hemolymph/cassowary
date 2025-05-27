use std::{
    collections::VecDeque,
    ops::{Index, IndexMut},
};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ServerMsg {
    UpdateHand(Vec<String>),
    UpdateSpaces {
        home_row: Box<Row>,
        away_row: Box<Row>,
    },
    UpdateDiscard(Side, Vec<String>),
    UpdateTimeline(Side, Vec<String>),
    BeginSearch(Vec<String>),
    UpdateState(Box<LocalState>),
    RoomCreated,
    JoinedRoom(Box<LocalState>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ServerErr {
    RoomDoesntExist(String),
    NotInGame { action: String },
    NotInSide,
    NoPlayerInSide(Side),
    NoCardIn(PlaceFrom),
    SideOccupied(Side),
    AlreadyInGame { action: String },
}

impl ServerMsg {
    pub fn is_game_action(&self) -> bool {
        match self {
            ServerMsg::UpdateHand(..) => true,
            ServerMsg::UpdateSpaces { .. } => true,
            ServerMsg::UpdateDiscard(..) => true,
            ServerMsg::UpdateTimeline(..) => true,
            ServerMsg::BeginSearch(..) => true,
            ServerMsg::UpdateState(..) => true,
            ServerMsg::RoomCreated => false,
            ServerMsg::JoinedRoom(..) => false,
        }
    }

    pub fn get_name(&self) -> &'static str {
        match self {
            ServerMsg::UpdateHand(vec) => "update hand",
            ServerMsg::UpdateSpaces { home_row, away_row } => "update spaces",
            ServerMsg::UpdateDiscard(side, vec) => "update discard",
            ServerMsg::UpdateTimeline(side, vec) => "update timeline",
            ServerMsg::BeginSearch(vec) => "begin search",
            ServerMsg::UpdateState(local_state) => "update state",
            ServerMsg::RoomCreated => "room created",
            ServerMsg::JoinedRoom(local_state) => "join room",
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum ClientMsg {
    Draw(RelSide, DeckType),
    Move { from: PlaceFrom, to: PlaceTo },
    Shuffle(DeckType),
    RequestSearch,
    Update,
    CreateRoom,
    SetDeck(DeckType, VecDeque<String>),
    JoinRoom(String),
    PlayAs(Side),
}

impl ClientMsg {
    pub fn is_game_action(&self) -> bool {
        match self {
            ClientMsg::Draw(..) => true,
            ClientMsg::Move { .. } => true,
            ClientMsg::Shuffle(..) => true,
            ClientMsg::RequestSearch => true,
            ClientMsg::Update => true,
            ClientMsg::SetDeck(..) => true,
            ClientMsg::PlayAs(..) => true,
            ClientMsg::CreateRoom => false,
            ClientMsg::JoinRoom(..) => false,
        }
    }

    pub fn get_name(&self) -> &'static str {
        match self {
            ClientMsg::Draw(rel_side, deck_type) => "draw",
            ClientMsg::Move { from, to } => "move",
            ClientMsg::Shuffle(deck_type) => "shuffle",
            ClientMsg::RequestSearch => "request search",
            ClientMsg::Update => "update",
            ClientMsg::CreateRoom => "create room",
            ClientMsg::SetDeck(deck_type, vec_deque) => "set deck",
            ClientMsg::JoinRoom(_) => "join room",
            ClientMsg::PlayAs(side) => "play as side",
        }
    }
}

/// Places cards can be sent to in the deck.
#[derive(Serialize, Deserialize)]
pub enum DeckTo {
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PlaceFrom {
    Hand(usize),
    Space(Side, Space),
    Discard(usize),
    Aside(usize),
    Timeline(usize),
    Deck(Side, DeckType, usize),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Space {
    First = 0,
    Second = 1,
    Third = 2,
    Fourth = 3,
}

#[derive(Serialize, Deserialize)]
/// Sinde Liberate is not a place, this does not accurately represent Bloodless move destinations.
pub enum PlaceTo {
    Hand,
    Space(Side, Space, bool),
    Discard,
    Aside,
    Timeline,
    Deck(DeckTo, Side, DeckType),
    /// While not technically a place, this works
    Liberate,
}

#[derive(Serialize, Deserialize)]
pub struct Move {}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Row {
    first: Option<Card>,
    second: Option<Card>,
    third: Option<Card>,
    fourth: Option<Card>,
}

impl Index<Space> for Row {
    type Output = Option<Card>;

    fn index(&self, index: Space) -> &Self::Output {
        match index {
            Space::First => &self.first,
            Space::Second => &self.second,
            Space::Third => &self.third,
            Space::Fourth => &self.fourth,
        }
    }
}

impl IndexMut<Space> for Row {
    fn index_mut(&mut self, index: Space) -> &mut Self::Output {
        match index {
            Space::First => &mut self.first,
            Space::Second => &mut self.second,
            Space::Third => &mut self.third,
            Space::Fourth => &mut self.fourth,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GameState {
    pub home_state: PlayerState,
    pub away_state: PlayerState,
    pub home_row: Row,
    pub away_row: Row,
    pub floating_cards: Vec<(String, (usize, usize))>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PlayerState {
    pub hand: Vec<String>,
    pub main_deck: VecDeque<String>,
    pub blood_deck: VecDeque<String>,
    pub blood: usize,
    pub discard: Vec<String>,
    pub timeline: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DeckType {
    Blood,
    Main,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Home,
    Away,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Card {
    pub name: String,
    pub backside: bool,
}

impl Card {
    pub fn from_string(name: String, backside: bool) -> Self {
        Self { name, backside }
    }
    pub fn from_str(name: &str, backside: bool) -> Self {
        Self::from_string(name.to_string(), backside)
    }
}

/// Represents what is known about a player's state
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalPlayer {
    pub blood: usize,
    pub discard: Vec<String>,
    pub timeline: Vec<String>,
}

/// Represents what is known about the game state
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalState {
    pub home_state: LocalPlayer,
    pub away_state: LocalPlayer,
    pub home_row: Row,
    pub away_row: Row,
    pub hand: Vec<String>,
    pub floating_cards: Vec<(String, (usize, usize))>,
}

impl GameState {
    pub fn create_local_for(&self, side: Option<Side>) -> LocalState {
        let hand = match side {
            Some(Side::Home) => self.home_state.hand.clone(),
            Some(Side::Away) => self.away_state.hand.clone(),
            None => vec![],
        };

        LocalState {
            home_state: self.home_state.create_local(),
            away_state: self.away_state.create_local(),
            home_row: self.home_row.clone(),
            away_row: self.away_row.clone(),
            hand,
            floating_cards: vec![],
        }
    }
}

impl PlayerState {
    pub fn create_local(&self) -> LocalPlayer {
        LocalPlayer {
            blood: self.blood,
            discard: self.discard.clone(),
            timeline: self.timeline.clone(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum RelSide {
    Same,
    Other,
}
