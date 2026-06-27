use crate::{
    card::{number, Card, Suit},
    deck::Deck,
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub enum PromptKind {
    Select4,
    Select7,
    Select13,
    UseOneChance,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub struct Prompt {
    pub kind: PromptKind,
    pub player_ids: Vec<String>,
    pub question: String,
    pub options: Vec<String>,
}

#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldKey {
    Trushes,
    Excluded,
    Hands(String),
}

impl std::fmt::Display for FieldKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldKey::Trushes => write!(f, "trushes"),
            FieldKey::Excluded => write!(f, "excluded"),
            FieldKey::Hands(id) => write!(f, "{}", id),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Game {
    // game state
    pub prompt: Vec<Prompt>,
    // workaround for "key must be a string" error
    // https://stackoverflow.com/questions/51276896/how-do-i-use-serde-to-serialize-a-hashmap-with-structs-as-keys-to-json
    #[serde_as(as = "Vec<(_, _)>")]
    pub fields: HashMap<FieldKey, Deck>,
    // river
    pub river: Vec<Vec<Card>>,
    pub river_size: Option<usize>,
    pub suit_limits: HashSet<Suit>,
    /// a number includes `effect_limits` ignore effect
    pub effect_limits: HashSet<u8>,
    /// card strength is reversed until the river is reset
    pub turn_revoluted: bool,
    /// when `is_step` is true, delta of previous cards number must be 1
    pub is_step: bool,
    /// when `revoluted` is true, card strength is reversed
    pub revoluted: bool,

    pub current: Option<String>,
    pub last_served_player_id: Option<String>,
    // player state
    pub players: Vec<String>,
    pub selects: HashMap<String, Vec<Card>>,
    pub answers: HashMap<String, String>,
    /// finish order: players appended as their hands empty (first = winner).
    pub ranks: Vec<String>,
}

impl Game {
    pub fn new(player_ids: Vec<String>) -> Self {
        let mut fields = player_ids
            .iter()
            .map(|id| (FieldKey::Hands(id.clone()), Deck::new(vec![])))
            .collect::<HashMap<_, _>>();
        fields.insert(FieldKey::Trushes, Deck::new(vec![]));
        fields.insert(FieldKey::Excluded, Deck::new(vec![]));

        Self {
            prompt: vec![],
            fields,

            river: vec![],
            river_size: None,
            suit_limits: HashSet::new(),
            effect_limits: HashSet::new(),
            turn_revoluted: false,
            is_step: false,
            revoluted: false,

            current: None,
            last_served_player_id: None,

            players: player_ids.clone(),
            answers: HashMap::new(),
            selects: HashMap::from_iter(player_ids.iter().map(|id| (id.to_string(), Vec::new()))),
            ranks: vec![],
        }
    }

    pub fn field_mut(&mut self, id: &FieldKey) -> Result<&mut Deck> {
        let Some(deck) = self.fields.get_mut(id) else {
            return Err(anyhow!("field {} not found", id));
        };
        Ok(deck)
    }

    pub fn field(&self, id: &FieldKey) -> Result<&Deck> {
        let Some(deck) = self.fields.get(id) else {
            return Err(anyhow!("field {} not found", id));
        };
        Ok(deck)
    }

    pub fn transfer(&mut self, from: &FieldKey, to: &FieldKey, cards: Vec<Card>) -> Result<()> {
        self.field_mut(from)?.remove(&cards)?;
        self.field_mut(to)?.0.extend(cards.to_vec());
        Ok(())
    }

    pub fn active_player_ids(&self) -> Vec<String> {
        self.players
            .iter()
            .filter(|id| {
                !self
                    .field(&FieldKey::Hands(id.to_string()))
                    .unwrap()
                    .0
                    .is_empty()
            })
            .cloned()
            .collect()
    }

    pub fn get_relative_player(&self, player_id: &str, d: i32) -> String {
        let active_player_ids = self.active_player_ids();
        let index = active_player_ids
            .iter()
            .position(|id| id == player_id)
            .unwrap();
        let index = ((index as i32 + d).rem_euclid(active_player_ids.len() as i32)) as usize;
        active_player_ids[index].clone()
    }

    /// Next still-active player in seating order after `player_id`, honoring
    /// `skips`.
    ///
    /// - `skips == 1`: the immediate next active player (normal turn).
    /// - `skips == 0`: keep `player_id` if still active (8-cut / 2 / one-chance
    ///   "same player restarts"); but if `player_id` just finished, fall through
    ///   to the next active player so the lead is passed on.
    /// - `skips == n` (n > 1): advance past `n` active players (5-skip).
    ///
    /// Unlike [`get_relative_player`], this walks the seating order
    /// (`self.players`) so it works even when `player_id` is no longer active
    /// (just emptied their hand). Returns `None` only when nobody is active.
    pub fn next_active_player(&self, player_id: &str, skips: i32) -> Option<String> {
        let active = self.active_player_ids();
        if active.is_empty() {
            return None;
        }
        let is_active = active.iter().any(|p| p == player_id);
        if skips == 0 && is_active {
            return Some(player_id.to_string());
        }
        // skips == 0 with a just-finished player behaves like skips == 1.
        let steps = if skips <= 0 { 1 } else { skips };
        let seat = self.players.iter().position(|p| p == player_id)?;
        let n = self.players.len();
        let mut idx = seat;
        let mut count = 0;
        loop {
            idx = (idx + 1) % n;
            let pid = &self.players[idx];
            if active.iter().any(|p| p == pid) {
                count += 1;
                if count == steps {
                    return Some(pid.clone());
                }
            }
        }
    }

    fn flush_river(&mut self, to: &FieldKey) -> Result<()> {
        let cards = self.river.iter().flatten().cloned().collect::<Vec<_>>();
        self.field_mut(to)?.0.extend(cards);
        self.river.clear();

        self.river_size = None;
        self.suit_limits = HashSet::new();
        self.effect_limits = HashSet::new();
        self.turn_revoluted = false;
        self.is_step = false;

        Ok(())
    }

    pub fn on_end_turn(&mut self) -> Result<()> {
        let player_id = self.current.clone().unwrap();

        // Record the finisher: a player who just emptied their hand is appended
        // to the finish order (first finisher = 大富豪). Idempotent.
        let hand_empty = self
            .field(&FieldKey::Hands(player_id.clone()))?
            .0
            .is_empty();
        if hand_empty && !self.ranks.contains(&player_id) {
            self.ranks.push(player_id.clone());
        }

        // The round ends once at most one active player remains.
        if self.active_player_ids().len() <= 1 {
            return Err(anyhow!("end"));
        }

        let top = self
            .river
            .last()
            .expect("river must not be empty when end turn")
            .clone();

        // next player
        let skips = match &top {
            _ if number(&top) == 5 && !self.effect_limits.contains(&5) => top.len() as i32 + 1,
            _ if number(&top) == 8 && !self.effect_limits.contains(&8) => 0,
            _ if number(&top) == 1 && !self.effect_limits.contains(&1) => 0,
            _ => 1,
        };
        self.current = self.next_active_player(&player_id, skips);

        // Flush when the lead returns to the last server. If that server has
        // since finished, the lead is inherited by the next active player after
        // them, so the new leader gets a fresh river.
        let flush_anchor = match &self.last_served_player_id {
            Some(lead) if self.active_player_ids().iter().any(|p| p == lead) => Some(lead.clone()),
            Some(lead) => self.next_active_player(lead, 1),
            None => None,
        };
        if self.current.is_some() && self.current == flush_anchor {
            let to = if number(&top) == 2 && !self.effect_limits.contains(&2) {
                FieldKey::Excluded
            } else {
                FieldKey::Trushes
            };
            self.flush_river(&to)?;
        }
        Ok(())
    }
}
