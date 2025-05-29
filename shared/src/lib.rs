use std::{
    collections::{HashMap, VecDeque},
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
    UpdateTimeline(RelSide, Vec<LocalCard>),
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
    AddCounter(PlaceFrom, String, bool),
    CreateCounter(PlaceFrom, String),
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
            ClientMsg::AddCounter(..) => true,
            ClientMsg::CreateCounter(..) => true,
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
            ClientMsg::AddCounter(..) => "add one to counter",
            ClientMsg::CreateCounter(..) => "create new counter",
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RowBase<T> {
    first: Option<T>,
    second: Option<T>,
    third: Option<T>,
    fourth: Option<T>,
}

impl<T> Default for RowBase<T> {
    fn default() -> Self {
        Self {
            first: None,
            second: None,
            third: None,
            fourth: None,
        }
    }
}

pub type Row = RowBase<Card>;
pub type LocalRow = RowBase<LocalCard>;

impl<T> Index<Space> for RowBase<T> {
    type Output = Option<T>;

    fn index(&self, index: Space) -> &Self::Output {
        match index {
            Space::First => &self.first,
            Space::Second => &self.second,
            Space::Third => &self.third,
            Space::Fourth => &self.fourth,
        }
    }
}

impl<T> IndexMut<Space> for RowBase<T> {
    fn index_mut(&mut self, index: Space) -> &mut Self::Output {
        match index {
            Space::First => &mut self.first,
            Space::Second => &mut self.second,
            Space::Third => &mut self.third,
            Space::Fourth => &mut self.fourth,
        }
    }
}

impl Row {
    pub fn to_local(self) -> LocalRow {
        LocalRow {
            first: self.first.map(Card::to_local),
            second: self.second.map(Card::to_local),
            third: self.third.map(Card::to_local),
            fourth: self.fourth.map(Card::to_local),
        }
    }
}

impl<T> RowBase<T> {
    pub fn get(&self, idx: Space) -> Option<&T> {
        match idx {
            Space::First => self.first.as_ref(),
            Space::Second => self.second.as_ref(),
            Space::Third => self.third.as_ref(),
            Space::Fourth => self.fourth.as_ref(),
        }
    }
    pub fn get_mut(&mut self, idx: Space) -> Option<&mut T> {
        match idx {
            Space::First => self.first.as_mut(),
            Space::Second => self.second.as_mut(),
            Space::Third => self.third.as_mut(),
            Space::Fourth => self.fourth.as_mut(),
        }
    }
}

impl Card {
    pub fn to_local(self) -> LocalCard {
        let name = if self.backside {
            Hidden::Hidden
        } else {
            Hidden::Unhidden(self.name)
        };
        LocalCard {
            name,
            counters: self.counters,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GameState {
    pub home_state: PlayerState,
    pub away_state: PlayerState,
    pub home_row: Row,
    pub away_row: Row,
    pub floating_cards: Vec<(Card, (usize, usize))>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PlayerState {
    pub hand: Vec<String>,
    pub main_deck: VecDeque<String>,
    pub blood_deck: VecDeque<String>,
    pub blood: usize,
    pub discard: Vec<String>,
    pub timeline: Vec<Card>,
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
    pub counters: HashMap<String, usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalCard {
    /// None if on the backside
    pub name: Hidden<String>,
    pub counters: HashMap<String, usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Hidden<T> {
    Hidden,
    Unhidden(T),
}

impl Card {
    pub fn from_string(name: String, backside: bool) -> Self {
        Self {
            name,
            backside,
            counters: HashMap::new(),
        }
    }
    pub fn from_str(name: &str, backside: bool) -> Self {
        Self::from_string(name.to_string(), backside)
    }

    pub fn flipped(self, flipped: bool) -> Self {
        Self {
            backside: flipped,
            ..self
        }
    }
}

/// Represents what is known about a player's state
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalPlayer {
    pub blood: usize,
    pub discard: Vec<String>,
    pub timeline: Vec<LocalCard>,
}

/// Represents what is known about the game state
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalState {
    pub local_state: LocalPlayer,
    pub distant_state: LocalPlayer,
    pub local_row: Row,
    pub distant_row: Row,
    pub hand: Vec<String>,
    pub floating_cards: Vec<(LocalCard, (usize, usize))>,
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
    pub fn pop_card(&mut self, from: PlaceFrom, local_side: Side) -> Option<CardOrName> {
        match from {
            PlaceFrom::Hand(idx) => self
                .get_state_mut(local_side)
                .hand
                .safe_remove(idx)
                .map(Into::into),
            PlaceFrom::Space(side, idx) => self.get_row_mut(side.make_real(local_side))[idx]
                .take()
                .map(Into::into),
            PlaceFrom::Discard(side, idx) => self
                .get_state_mut(side.make_real(local_side))
                .discard
                .safe_remove(idx)
                .map(Into::into),
            PlaceFrom::Aside(idx) => todo!("Aside is not yet implemented"),
            PlaceFrom::Timeline(side, idx) => self
                .get_state_mut(side.make_real(local_side))
                .timeline
                .safe_remove(idx)
                .map(Into::into),
            PlaceFrom::Deck(side, deck_type, idx) => {
                let side = side.make_real(local_side);
                let player = self.get_state_mut(side);
                match deck_type {
                    DeckType::Blood => player.blood_deck.remove(idx).map(Into::into),
                    DeckType::Main => player.main_deck.remove(idx).map(Into::into),
                }
            }
        }
    }
    pub fn get_card(&self, from: PlaceFrom, local_side: Side) -> Option<CardOrNameRef> {
        match from {
            PlaceFrom::Hand(idx) => self.get_state(local_side).hand.get(idx).map(Into::into),
            PlaceFrom::Space(side, idx) => self
                .get_row(side.make_real(local_side))
                .get(idx)
                .map(Into::into),
            PlaceFrom::Discard(side, idx) => self
                .get_state(side.make_real(local_side))
                .discard
                .get(idx)
                .map(Into::into),
            PlaceFrom::Aside(idx) => todo!("Aside is not yet implemented"),
            PlaceFrom::Timeline(side, idx) => self
                .get_state(side.make_real(local_side))
                .timeline
                .get(idx)
                .map(Into::into),
            PlaceFrom::Deck(side, deck_type, idx) => {
                let side = side.make_real(local_side);
                let player = self.get_state(side);
                match deck_type {
                    DeckType::Blood => player.blood_deck.get(idx).map(Into::into),
                    DeckType::Main => player.main_deck.get(idx).map(Into::into),
                }
            }
        }
    }
    pub fn get_card_mut(&mut self, from: PlaceFrom, local_side: Side) -> Option<CardOrNameMut> {
        match from {
            PlaceFrom::Hand(idx) => self
                .get_state_mut(local_side)
                .hand
                .get_mut(idx)
                .map(Into::into),
            PlaceFrom::Space(side, idx) => self
                .get_row_mut(side.make_real(local_side))
                .get_mut(idx)
                .map(Into::into),
            PlaceFrom::Discard(side, idx) => self
                .get_state_mut(side.make_real(local_side))
                .discard
                .get_mut(idx)
                .map(Into::into),
            PlaceFrom::Aside(idx) => todo!("Aside is not yet implemented"),
            PlaceFrom::Timeline(side, idx) => self
                .get_state_mut(side.make_real(local_side))
                .timeline
                .get_mut(idx)
                .map(Into::into),
            PlaceFrom::Deck(side, deck_type, idx) => {
                let side = side.make_real(local_side);
                let player = self.get_state_mut(side);
                match deck_type {
                    DeckType::Blood => player.blood_deck.get_mut(idx).map(Into::into),
                    DeckType::Main => player.main_deck.get_mut(idx).map(Into::into),
                }
            }
        }
    }
    pub fn push_card(&mut self, card: CardOrName, to: PlaceTo, local_side: Side) -> Option<()> {
        match to {
            PlaceTo::Hand => self.get_state_mut(local_side).hand.push(card.into()),
            PlaceTo::Space(side, space, flipped) => {
                let card: Card = card.into();
                self.get_row_mut(side.make_real(local_side))[space] = Some(card.flipped(flipped))
            }
            PlaceTo::Discard(side) => self
                .get_state_mut(side.make_real(local_side))
                .discard
                .push(card.into()),
            PlaceTo::Aside => todo!("Aside is not yet implemented"),
            PlaceTo::Timeline(side) => self
                .get_state_mut(side.make_real(local_side))
                .timeline
                .push(card.into()),
            PlaceTo::Deck(deck_to, side, deck_type) => {
                let side = side.make_real(local_side);
                let player = self.get_state_mut(side);
                let deck = match deck_type {
                    DeckType::Blood => &mut player.blood_deck,
                    DeckType::Main => &mut player.main_deck,
                };
                match deck_to {
                    DeckTo::Top => deck.push_front(card.into()),
                    DeckTo::Bottom => deck.push_back(card.into()),
                }
            }
            PlaceTo::Liberate => (), // Do nothing. The card was removed earlier. Don't put it anywhere
        }
        Some(())
    }

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
            timeline: self
                .timeline
                .clone()
                .into_iter()
                .map(|x| x.to_local())
                .collect(),
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

pub enum CardOrName {
    Card(Card),
    Name(String),
}

pub enum CardOrNameRef<'a> {
    Card(&'a Card),
    Name(&'a String),
}

pub enum CardOrNameMut<'a> {
    Card(&'a mut Card),
    Name(&'a mut String),
}

impl From<CardOrName> for String {
    fn from(value: CardOrName) -> Self {
        match value {
            CardOrName::Card(card) => card.name,
            CardOrName::Name(name) => name,
        }
    }
}
impl From<CardOrName> for Card {
    fn from(value: CardOrName) -> Card {
        match value {
            CardOrName::Card(card) => card,
            CardOrName::Name(name) => dbg!(Card {
                name,
                backside: false,
                counters: HashMap::new(),
            }),
        }
    }
}

impl From<Card> for CardOrName {
    fn from(value: Card) -> Self {
        Self::Card(value)
    }
}
impl From<String> for CardOrName {
    fn from(value: String) -> Self {
        Self::Name(value)
    }
}

impl From<String> for LocalCard {
    fn from(value: String) -> Self {
        Self {
            name: Hidden::Unhidden(value),
            counters: HashMap::new(),
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

impl<'a> From<&'a Card> for CardOrNameRef<'a> {
    fn from(value: &'a Card) -> Self {
        Self::Card(value)
    }
}
impl<'a> From<&'a String> for CardOrNameRef<'a> {
    fn from(value: &'a String) -> Self {
        Self::Name(value)
    }
}

impl<'a> From<&'a mut Card> for CardOrNameMut<'a> {
    fn from(value: &'a mut Card) -> Self {
        Self::Card(value)
    }
}
impl<'a> From<&'a mut String> for CardOrNameMut<'a> {
    fn from(value: &'a mut String) -> Self {
        Self::Name(value)
    }
}
