use crate::{
    card::{number, Card, Suit},
    deck::Deck,
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Effect {
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
}

impl Effect {
    pub fn new() -> Self {
        Self {
            river_size: None,
            suit_limits: HashSet::new(),
            effect_limits: HashSet::new(),
            turn_revoluted: false,
            is_step: false,
            revoluted: false,
        }
    }

    pub fn new_turn(effect: Effect) -> Self {
        Self {
            river_size: None,
            suit_limits: HashSet::new(),
            effect_limits: HashSet::new(),
            turn_revoluted: false,
            is_step: false,
            ..effect
        }
    }
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Game {
    // game state
    pub prompt: Vec<Prompt>,
    pub fields: HashMap<FieldKey, Deck>,
    pub river: Vec<Vec<Card>>,
    pub current: Option<String>,
    pub last_served_player_id: Option<String>,
    pub effect: Effect,
    // player state
    pub players: Vec<String>,
    pub selects: HashMap<String, Vec<Card>>,
    pub answers: HashMap<String, String>,
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
            players: player_ids.clone(),
            prompt: vec![],
            answers: HashMap::new(),
            selects: HashMap::from_iter(player_ids.iter().map(|id| (id.to_string(), Vec::new()))),
            river: vec![],
            last_served_player_id: None,
            current: None,
            fields,
            effect: Effect::new(),
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
            .position(|id| id == &player_id)
            .unwrap();
        let index = ((index as i32 + d).rem_euclid(active_player_ids.len() as i32)) as usize;
        active_player_ids[index].clone()
    }

    fn flush_river(&mut self, to: &FieldKey) -> Result<()> {
        let cards = self.river.iter().flatten().cloned().collect::<Vec<_>>();
        self.field_mut(to)?.0.extend(cards);
        self.river.clear();
        self.effect = Effect::new_turn(self.effect.clone());
        Ok(())
    }

    pub fn on_end_turn(&mut self) -> Result<()> {
        let player_id = self.current.clone().unwrap();

        let hand = self.field(&FieldKey::Hands(player_id.clone()))?;
        if hand.0.is_empty() && self.active_player_ids().len() == 1 {
            return Err(anyhow!("end"));
        }

        let skips = match self.river.last() {
            Some(top) if number(top) == 5 && !self.effect.effect_limits.contains(&5) => {
                top.len() as i32 + 1
            }
            Some(top) if number(top) == 8 && !self.effect.effect_limits.contains(&8) => 0,
            Some(top) if number(top) == 1 && !self.effect.effect_limits.contains(&1) => 0,
            _ => 1,
        };
        self.current = Some(self.get_relative_player(&player_id, skips));

        // flush
        if self.current == self.last_served_player_id {
            let to = match self.river.last() {
                Some(top) if number(top) == 2 && !self.effect.effect_limits.contains(&2) => {
                    FieldKey::Excluded
                }
                _ => FieldKey::Trushes,
            };
            self.flush_river(&to)?;
        }

        Ok(())
    }
}
