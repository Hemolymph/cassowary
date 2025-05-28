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
    UpdateDiscard(RelSide, Vec<String>),
    UpdateTimeline(RelSide, Vec<String>),
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
    GameIsFull,
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ClientMsg {
    Draw(RelSide, DeckType),
    Move { from: PlaceFrom, to: PlaceTo },
    Shuffle(DeckType),
    RequestSearch,
    Update,
    CreateRoom,
    SetDeck(DeckType, VecDeque<String>),
    JoinRoom(String),
    PlayAs,
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
            ClientMsg::PlayAs => true,
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
            ClientMsg::PlayAs => "play in game",
        }
    }
}

/// Places cards can be sent to in the deck.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum DeckTo {
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PlaceFrom {
    Hand(usize),
    Space(RelSide, Space),
    Discard(RelSide, usize),
    Aside(usize),
    Timeline(RelSide, usize),
    Deck(RelSide, DeckType, usize),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Space {
    First = 0,
    Second = 1,
    Third = 2,
    Fourth = 3,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Sinde Liberate is not a place, this does not accurately represent Bloodless move destinations.
pub enum PlaceTo {
    Hand,
    Space(RelSide, Space, bool),
    Discard(RelSide),
    Aside,
    Timeline(RelSide),
    Deck(DeckTo, RelSide, DeckType),
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

impl PlayerState {
    pub fn get_deck(&self, which: DeckType) -> &VecDeque<String> {
        match which {
            DeckType::Blood => &self.blood_deck,
            DeckType::Main => &self.main_deck,
        }
    }
    pub fn get_deck_mut(&mut self, which: DeckType) -> &mut VecDeque<String> {
        match which {
            DeckType::Blood => &mut self.blood_deck,
            DeckType::Main => &mut self.main_deck,
        }
    }
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

impl Side {
    pub fn opposite(self) -> Self {
        match self {
            Side::Home => Side::Away,
            Side::Away => Side::Home,
        }
    }
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
    pub local_state: LocalPlayer,
    pub distant_state: LocalPlayer,
    pub local_row: Row,
    pub distant_row: Row,
    pub hand: Vec<String>,
    pub floating_cards: Vec<(String, (usize, usize))>,
}

impl LocalState {
    pub fn get_row(&self, side: RelSide) -> &Row {
        match side {
            RelSide::Same => &self.local_row,
            RelSide::Other => &self.distant_row,
        }
    }
    pub fn get_player(&self, side: RelSide) -> &LocalPlayer {
        match side {
            RelSide::Same => &self.local_state,
            RelSide::Other => &self.distant_state,
        }
    }
}

impl GameState {
    pub fn create_local_for(&self, side: Option<Side>) -> LocalState {
        let local_state = self.get_state(side.unwrap_or(Side::Home));
        let away_state = self.get_state(side.unwrap_or(Side::Home).opposite());
        let local_row = self.get_row(side.unwrap_or(Side::Home));
        let away_row = self.get_row(side.unwrap_or(Side::Home).opposite());

        LocalState {
            local_state: local_state.create_local(),
            distant_state: away_state.create_local(),
            local_row: local_row.clone(),
            distant_row: away_row.clone(),
            hand: local_state.hand.clone(),
            floating_cards: vec![],
        }
    }

    pub fn get_row(&self, side: Side) -> &Row {
        match side {
            Side::Home => &self.home_row,
            Side::Away => &self.away_row,
        }
    }
    pub fn get_row_mut(&mut self, side: Side) -> &mut Row {
        match side {
            Side::Home => &mut self.home_row,
            Side::Away => &mut self.away_row,
        }
    }
    pub fn get_state(&self, side: Side) -> &PlayerState {
        match side {
            Side::Home => &self.home_state,
            Side::Away => &self.away_state,
        }
    }
    pub fn get_state_mut(&mut self, side: Side) -> &mut PlayerState {
        match side {
            Side::Home => &mut self.home_state,
            Side::Away => &mut self.away_state,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelSide {
    Same,
    Other,
}

impl RelSide {
    pub fn make_real(self, local: Side) -> Side {
        match self {
            RelSide::Same => local,
            RelSide::Other => local.opposite(),
        }
    }
}
