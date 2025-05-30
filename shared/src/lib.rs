use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    ops::{Index, IndexMut},
};

use serde::{Deserialize, Serialize};

// This is my single worst piece of code.
// If you don't know how to read this, don't worry. You won't.
// Just turn around while you can.
// You can see when in development I started working on UI code because the code starts looking more and more and more and more unwieldy as it goes.

// This ID  only matters inside a room.

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Copy)]
pub struct CardId(pub usize);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ServerMsg {
    UpdateHand(Vec<NamedCardId>, LocalDeckTop, LocalDeckTop),
    UpdateSpaces {
        home_row: Box<LocalRow>,
        away_row: Box<LocalRow>,
    },
    UpdateDiscard(RelSide, Vec<NamedCardId>),
    UpdateTimeline(RelSide, Vec<LocalCard>),
    BeginSearch(Vec<NamedCardId>),
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
    RoomAlreadyExist,
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
            ServerMsg::UpdateHand(..) => "update hand",
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
    RequestSearch(DeckType),
    Update,
    CreateRoom(String),
    SetDeck(DeckType, VecDeque<String>),
    JoinRoom(String),
    PlayAs,
    AddCounter(PlaceFrom, String, bool),
    CreateCounter(PlaceFrom, String),
    FinishSearch,
    LeaveRoom,
    AddBlood(RelSide, bool),
    AddHealth(bool),
    EndTurn,
    CreateCard(String),
}

impl ClientMsg {
    pub fn is_game_action(&self) -> bool {
        match self {
            ClientMsg::Draw(..) => true,
            ClientMsg::Move { .. } => true,
            ClientMsg::Shuffle(..) => true,
            ClientMsg::RequestSearch(..) => true,
            ClientMsg::Update => true,
            ClientMsg::SetDeck(..) => true,
            ClientMsg::PlayAs => true,
            ClientMsg::CreateRoom(..) => false,
            ClientMsg::JoinRoom(..) => false,
            ClientMsg::AddCounter(..) => true,
            ClientMsg::CreateCounter(..) => true,
            ClientMsg::FinishSearch => true,
            ClientMsg::LeaveRoom => true,
            ClientMsg::AddBlood(rel_side, _) => true,
            ClientMsg::EndTurn => true,
            ClientMsg::AddHealth(_) => true,
            ClientMsg::CreateCard(_) => true,
        }
    }

    pub fn get_name(&self) -> &'static str {
        match self {
            ClientMsg::Draw(rel_side, deck_type) => "draw",
            ClientMsg::Move { from, to } => "move",
            ClientMsg::Shuffle(deck_type) => "shuffle",
            ClientMsg::RequestSearch(..) => "request search",
            ClientMsg::Update => "update",
            ClientMsg::CreateRoom(..) => "create room",
            ClientMsg::SetDeck(deck_type, vec_deque) => "set deck",
            ClientMsg::JoinRoom(_) => "join room",
            ClientMsg::PlayAs => "play in game",
            ClientMsg::AddCounter(..) => "add one to counter",
            ClientMsg::CreateCounter(..) => "create new counter",
            ClientMsg::FinishSearch => "done searching",
            ClientMsg::LeaveRoom => "leaving room",
            ClientMsg::AddBlood(rel_side, _) => "add blood",
            ClientMsg::EndTurn => "end turn",
            ClientMsg::AddHealth(_) => "add health",
            ClientMsg::CreateCard(_) => "create card",
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
    Hand(CardId),
    Space(RelSide, Space),
    Discard(RelSide, CardId),
    Aside(CardId),
    Timeline(RelSide, CardId),
    Deck(RelSide, DeckType, CardId),
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
    pub fn to_local(self, ids: &BTreeMap<CardId, String>) -> LocalRow {
        LocalRow {
            first: self.first.map(|x| x.to_local(ids)),
            second: self.second.map(|x| x.to_local(ids)),
            third: self.third.map(|x| x.to_local(ids)),
            fourth: self.fourth.map(|x| x.to_local(ids)),
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
    pub fn to_local(self, ids: &BTreeMap<CardId, String>) -> LocalCard {
        let name = if self.backside {
            Hidden::Hidden
        } else {
            Hidden::Unhidden(ids.get(&self.id).unwrap().clone())
        };
        LocalCard {
            name,
            counters: self.counters,
            id: self.id,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameState {
    pub home_state: PlayerState,
    pub away_state: PlayerState,
    pub home_row: Row,
    pub away_row: Row,
    pub floating_cards: Vec<(Card, (usize, usize))>,
    pub health: usize,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            home_state: PlayerState::default(),
            away_state: PlayerState::default(),
            home_row: Row::default(),
            away_row: Row::default(),
            floating_cards: Vec::default(),
            health: 20,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PlayerState {
    pub hand: Vec<CardId>,
    pub main_deck: VecDeque<CardId>,
    pub blood_deck: VecDeque<CardId>,
    pub blood: usize,
    pub discard: Vec<CardId>,
    pub timeline: Vec<Card>,
    pub searching: Option<DeckType>,
}

impl PlayerState {
    pub fn get_deck(&self, which: DeckType) -> &VecDeque<CardId> {
        match which {
            DeckType::Blood => &self.blood_deck,
            DeckType::Main => &self.main_deck,
        }
    }
    pub fn get_deck_mut(&mut self, which: DeckType) -> &mut VecDeque<CardId> {
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
    pub id: CardId,
    pub backside: bool,
    pub counters: HashMap<String, usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalCard {
    /// None if on the backside
    pub name: Hidden<String>,
    pub id: CardId,
    pub counters: HashMap<String, usize>,
}

impl LocalCard {
    pub fn flipped(self, flipped: bool) -> Self {
        let name = if flipped { Hidden::Hidden } else { self.name };
        Self { name, ..self }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Hidden<T> {
    Hidden,
    Unhidden(T),
}

impl Card {
    pub fn from_id(id: CardId, backside: bool) -> Self {
        Self {
            id,
            backside,
            counters: HashMap::new(),
        }
    }

    pub fn flipped(self, flipped: bool) -> Self {
        Self {
            backside: flipped,
            ..self
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum LocalDeckTop {
    Empty,
    Card,
    Revealed(String),
}

/// Represents what is known about a player's state
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalPlayer {
    pub blood: usize,
    pub discard: Vec<NamedCardId>,
    pub timeline: Vec<LocalCard>,
    pub main_deck_top: LocalDeckTop,
    pub blood_deck_top: LocalDeckTop,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NamedCardId {
    pub name: String,
    pub id: CardId,
}

/// Represents what is known about the game state
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalState {
    pub local_state: LocalPlayer,
    pub distant_state: LocalPlayer,
    pub local_row: LocalRow,
    pub distant_row: LocalRow,
    pub hand: Vec<NamedCardId>,
    pub floating_cards: Vec<(LocalCard, (usize, usize))>,
    pub health: usize,
}

impl LocalState {
    pub fn get_row(&self, side: RelSide) -> &LocalRow {
        match side {
            RelSide::Same => &self.local_row,
            RelSide::Other => &self.distant_row,
        }
    }
    pub fn get_row_mut(&mut self, side: RelSide) -> &mut LocalRow {
        match side {
            RelSide::Same => &mut self.local_row,
            RelSide::Other => &mut self.distant_row,
        }
    }
    pub fn get_player(&self, side: RelSide) -> &LocalPlayer {
        match side {
            RelSide::Same => &self.local_state,
            RelSide::Other => &self.distant_state,
        }
    }
    pub fn get_state_mut(&mut self, side: RelSide) -> &mut LocalPlayer {
        match side {
            RelSide::Same => &mut self.local_state,
            RelSide::Other => &mut self.distant_state,
        }
    }
    pub fn pop_card(&mut self, from: PlaceFrom) -> Option<LocalCardOrNamedId> {
        match from {
            PlaceFrom::Hand(idx) => self.hand.find_remove(idx).map(Into::into),
            PlaceFrom::Space(side, idx) => self.get_row_mut(side)[idx].take().map(Into::into),
            PlaceFrom::Discard(side, idx) => self
                .get_state_mut(side)
                .discard
                .find_remove(idx)
                .map(Into::into),
            PlaceFrom::Aside(idx) => todo!("Aside is not yet implemented"),
            PlaceFrom::Timeline(side, idx) => self
                .get_state_mut(side)
                .timeline
                .find_remove(idx)
                .map(Into::into),
            PlaceFrom::Deck(side, deck_type, idx) => None,
        }
    }
    pub fn push_card(&mut self, card: LocalCardOrNamedId, to: PlaceTo) -> Option<()> {
        match to {
            PlaceTo::Hand => self.hand.push(card.into()),
            PlaceTo::Space(side, space, flipped) => {
                let card: LocalCard = card.into();
                self.get_row_mut(side)[space] = Some(card.flipped(flipped))
            }
            PlaceTo::Discard(side) => self.get_state_mut(side).discard.push(card.into()),
            PlaceTo::Aside => todo!("Aside is not yet implemented"),
            PlaceTo::Timeline(side) => self.get_state_mut(side).timeline.push(card.into()),
            PlaceTo::Deck(deck_to, side, deck_type) => (),
            PlaceTo::Liberate => (), // Do nothing. The card was removed earlier. Don't put it anywhere
        }
        Some(())
    }
}

pub enum LocalCardOrNamedId {
    Name(NamedCardId),
    Card(LocalCard),
}

// I'm gonna be frank with myself here. This should not be here.
//
// Then again, *who* should be here, right? Whoever is it that has
// authority to decide who of us actually should exist?
//
// There is no such authority, neither for us, nor for this impl.
impl From<LocalCardOrNamedId> for NamedCardId {
    fn from(value: LocalCardOrNamedId) -> Self {
        match value {
            LocalCardOrNamedId::Card(card) => match card.name {
                Hidden::Hidden => Self {
                    name: "".to_owned(),
                    id: card.id,
                },
                Hidden::Unhidden(name) => Self { name, id: card.id },
            },
            LocalCardOrNamedId::Name(name) => name,
        }
    }
}
impl From<LocalCardOrNamedId> for LocalCard {
    fn from(value: LocalCardOrNamedId) -> LocalCard {
        match value {
            LocalCardOrNamedId::Card(card) => card,
            LocalCardOrNamedId::Name(card) => LocalCard {
                name: Hidden::Unhidden(card.name),
                counters: HashMap::new(),
                id: card.id,
            },
        }
    }
}

impl From<LocalCard> for LocalCardOrNamedId {
    fn from(value: LocalCard) -> Self {
        Self::Card(value)
    }
}

// What on earth was I doing that made this necessary
// impl From<Card> for LocalCardOrName {
//     fn from(value: Card) -> Self {
//         Self::Card(value.to_local())
//     }
// }

impl From<NamedCardId> for LocalCardOrNamedId {
    fn from(value: NamedCardId) -> Self {
        Self::Name(value)
    }
}

impl GameState {
    pub fn pop_card(&mut self, from: PlaceFrom, local_side: Side) -> Option<CardOrName> {
        match from {
            PlaceFrom::Hand(idx) => self
                .get_state_mut(local_side)
                .hand
                .find_remove(idx)
                .map(Into::into),
            PlaceFrom::Space(side, idx) => self.get_row_mut(side.make_real(local_side))[idx]
                .take()
                .map(Into::into),
            PlaceFrom::Discard(side, idx) => self
                .get_state_mut(side.make_real(local_side))
                .discard
                .find_remove(idx)
                .map(Into::into),
            PlaceFrom::Aside(idx) => todo!("Aside is not yet implemented"),
            PlaceFrom::Timeline(side, idx) => self
                .get_state_mut(side.make_real(local_side))
                .timeline
                .find_remove(idx)
                .map(Into::into),
            PlaceFrom::Deck(side, deck_type, idx) => {
                let side = side.make_real(local_side);
                let player = self.get_state_mut(side);
                match deck_type {
                    DeckType::Blood => player.blood_deck.find_remove(idx).map(Into::into),
                    DeckType::Main => player.main_deck.find_remove(idx).map(Into::into),
                }
            }
        }
    }
    pub fn get_card(&self, from: PlaceFrom, local_side: Side) -> Option<CardOrNameRef> {
        match from {
            PlaceFrom::Hand(idx) => self.get_state(local_side).hand.find(idx).map(Into::into),
            PlaceFrom::Space(side, idx) => self
                .get_row(side.make_real(local_side))
                .get(idx)
                .map(Into::into),
            PlaceFrom::Discard(side, idx) => self
                .get_state(side.make_real(local_side))
                .discard
                .find(idx)
                .map(Into::into),
            PlaceFrom::Aside(idx) => todo!("Aside is not yet implemented"),
            PlaceFrom::Timeline(side, idx) => self
                .get_state(side.make_real(local_side))
                .timeline
                .find(idx)
                .map(Into::into),
            PlaceFrom::Deck(side, deck_type, idx) => {
                let side = side.make_real(local_side);
                let player = self.get_state(side);
                match deck_type {
                    DeckType::Blood => player.blood_deck.find(idx).map(Into::into),
                    DeckType::Main => player.main_deck.find(idx).map(Into::into),
                }
            }
        }
    }
    pub fn get_card_mut(&mut self, from: PlaceFrom, local_side: Side) -> Option<CardOrNameMut> {
        match from {
            PlaceFrom::Hand(idx) => self
                .get_state_mut(local_side)
                .hand
                .find_mut(idx)
                .map(Into::into),
            PlaceFrom::Space(side, idx) => self
                .get_row_mut(side.make_real(local_side))
                .get_mut(idx)
                .map(Into::into),
            PlaceFrom::Discard(side, idx) => self
                .get_state_mut(side.make_real(local_side))
                .discard
                .find_mut(idx)
                .map(Into::into),
            PlaceFrom::Aside(idx) => todo!("Aside is not yet implemented"),
            PlaceFrom::Timeline(side, idx) => self
                .get_state_mut(side.make_real(local_side))
                .timeline
                .find_mut(idx)
                .map(Into::into),
            PlaceFrom::Deck(side, deck_type, idx) => {
                let side = side.make_real(local_side);
                let player = self.get_state_mut(side);
                match deck_type {
                    DeckType::Blood => player.blood_deck.find_mut(idx).map(Into::into),
                    DeckType::Main => player.main_deck.find_mut(idx).map(Into::into),
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
                    DeckTo::Top => deck.push_back(card.into()),
                    DeckTo::Bottom => deck.push_front(card.into()),
                }
            }
            PlaceTo::Liberate => (), // Do nothing. The card was removed earlier. Don't put it anywhere
        }
        Some(())
    }

    pub fn create_local_for(
        &self,
        side: Option<Side>,
        ids: &BTreeMap<CardId, String>,
    ) -> LocalState {
        let local_state = self.get_state(side.unwrap_or(Side::Home));
        let away_state = self.get_state(side.unwrap_or(Side::Home).opposite());
        let local_row = self.get_row(side.unwrap_or(Side::Home));
        let away_row = self.get_row(side.unwrap_or(Side::Home).opposite());

        LocalState {
            local_state: local_state.create_local(ids),
            distant_state: away_state.create_local(ids),
            local_row: local_row.clone().to_local(ids),
            distant_row: away_row.clone().to_local(ids),
            hand: local_state
                .hand
                .iter()
                .map(|x| {
                    let a = ids.get(x).unwrap().clone();
                    NamedCardId { name: a, id: *x }
                })
                .collect(),
            floating_cards: vec![],
            health: self.health,
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
    pub fn create_local(&self, ids: &BTreeMap<CardId, String>) -> LocalPlayer {
        let main_deck_top = {
            if self.main_deck.is_empty() {
                LocalDeckTop::Empty
            } else {
                LocalDeckTop::Card
            }
        };
        let blood_deck_top = {
            if self.blood_deck.is_empty() {
                LocalDeckTop::Empty
            } else {
                LocalDeckTop::Card
            }
        };
        LocalPlayer {
            blood: self.blood,
            discard: self
                .discard
                .iter()
                .map(|id| NamedCardId {
                    name: ids.get(id).unwrap().to_string(),
                    id: *id,
                })
                .collect(),
            timeline: self
                .timeline
                .clone()
                .into_iter()
                .map(|x| x.to_local(ids))
                .collect(),
            main_deck_top,
            blood_deck_top,
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
    Name(CardId),
}

pub enum CardOrNameRef<'a> {
    Card(&'a Card),
    Name(&'a CardId),
}

pub enum CardOrNameMut<'a> {
    Card(&'a mut Card),
    Name(&'a mut CardId),
}

impl From<CardOrName> for CardId {
    fn from(value: CardOrName) -> Self {
        match value {
            CardOrName::Card(card) => card.id,
            CardOrName::Name(name) => name,
        }
    }
}
impl From<CardOrName> for Card {
    fn from(value: CardOrName) -> Card {
        match value {
            CardOrName::Card(card) => card,
            CardOrName::Name(id) => Card {
                id,
                backside: false,
                counters: HashMap::new(),
            },
        }
    }
}

impl From<Card> for CardOrName {
    fn from(value: Card) -> Self {
        Self::Card(value)
    }
}
impl From<CardId> for CardOrName {
    fn from(value: CardId) -> Self {
        Self::Name(value)
    }
}

impl From<NamedCardId> for LocalCard {
    fn from(value: NamedCardId) -> Self {
        Self {
            name: Hidden::Unhidden(value.name),
            counters: HashMap::new(),
            id: value.id,
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

pub trait Find<T, I> {
    fn find(&self, what: I) -> Option<&T>;
    fn find_mut(&mut self, what: I) -> Option<&mut T>;
    fn find_remove(&mut self, what: I) -> Option<T>;
}

impl<I: PartialEq> Find<I, I> for Vec<I> {
    fn find_remove(&mut self, what: I) -> Option<I> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if *x == what {
                found = Some(idx);
                break;
            }
        }

        found.map(|x| self.remove(x))
    }

    fn find(&self, what: I) -> Option<&I> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if *x == what {
                found = Some(idx);
                break;
            }
        }

        found.and_then(|x| self.get(x))
    }

    fn find_mut(&mut self, what: I) -> Option<&mut I> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if *x == what {
                found = Some(idx);
                break;
            }
        }

        found.and_then(|x| self.get_mut(x))
    }
}

impl<I: PartialEq> Find<I, I> for VecDeque<I> {
    fn find_remove(&mut self, what: I) -> Option<I> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if *x == what {
                found = Some(idx);
                break;
            }
        }

        found.and_then(|x| self.remove(x))
    }

    fn find(&self, what: I) -> Option<&I> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if *x == what {
                found = Some(idx);
                break;
            }
        }

        found.and_then(|x| self.get(x))
    }

    fn find_mut(&mut self, what: I) -> Option<&mut I> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if *x == what {
                found = Some(idx);
                break;
            }
        }

        found.and_then(|x| self.get_mut(x))
    }
}

impl Find<NamedCardId, CardId> for Vec<NamedCardId> {
    fn find_remove(&mut self, what: CardId) -> Option<NamedCardId> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if x.id == what {
                found = Some(idx);
                break;
            }
        }

        found.map(|x| self.remove(x))
    }

    fn find(&self, what: CardId) -> Option<&NamedCardId> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if x.id == what {
                found = Some(idx);
                break;
            }
        }

        found.and_then(|x| self.get(x))
    }

    fn find_mut(&mut self, what: CardId) -> Option<&mut NamedCardId> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if x.id == what {
                found = Some(idx);
                break;
            }
        }

        found.and_then(|x| self.get_mut(x))
    }
}

impl Find<LocalCard, CardId> for Vec<LocalCard> {
    fn find_remove(&mut self, what: CardId) -> Option<LocalCard> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if x.id == what {
                found = Some(idx);
                break;
            }
        }

        found.map(|x| self.remove(x))
    }
    fn find(&self, what: CardId) -> Option<&LocalCard> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if x.id == what {
                found = Some(idx);
                break;
            }
        }

        found.and_then(|x| self.get(x))
    }

    fn find_mut(&mut self, what: CardId) -> Option<&mut LocalCard> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if x.id == what {
                found = Some(idx);
                break;
            }
        }

        found.and_then(|x| self.get_mut(x))
    }
}

impl Find<Card, CardId> for Vec<Card> {
    fn find_remove(&mut self, what: CardId) -> Option<Card> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if x.id == what {
                found = Some(idx);
                break;
            }
        }

        found.map(|x| self.remove(x))
    }
    fn find(&self, what: CardId) -> Option<&Card> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if x.id == what {
                found = Some(idx);
                break;
            }
        }

        found.and_then(|x| self.get(x))
    }

    fn find_mut(&mut self, what: CardId) -> Option<&mut Card> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if x.id == what {
                found = Some(idx);
                break;
            }
        }

        found.and_then(|x| self.get_mut(x))
    }
}

impl Find<Card, CardId> for VecDeque<Card> {
    fn find_remove(&mut self, what: CardId) -> Option<Card> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if x.id == what {
                found = Some(idx);
                break;
            }
        }

        found.and_then(|x| self.remove(x))
    }
    fn find(&self, what: CardId) -> Option<&Card> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if x.id == what {
                found = Some(idx);
                break;
            }
        }

        found.and_then(|x| self.get(x))
    }

    fn find_mut(&mut self, what: CardId) -> Option<&mut Card> {
        let mut found = None;
        for (idx, x) in self.iter().enumerate() {
            if x.id == what {
                found = Some(idx);
                break;
            }
        }

        found.and_then(|x| self.get_mut(x))
    }
}

impl<'a> From<&'a Card> for CardOrNameRef<'a> {
    fn from(value: &'a Card) -> Self {
        Self::Card(value)
    }
}
impl<'a> From<&'a CardId> for CardOrNameRef<'a> {
    fn from(value: &'a CardId) -> Self {
        Self::Name(value)
    }
}

impl<'a> From<&'a mut Card> for CardOrNameMut<'a> {
    fn from(value: &'a mut Card) -> Self {
        Self::Card(value)
    }
}
impl<'a> From<&'a mut CardId> for CardOrNameMut<'a> {
    fn from(value: &'a mut CardId) -> Self {
        Self::Name(value)
    }
}
