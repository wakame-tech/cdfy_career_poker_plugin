use crate::{
    card::{Card, Suit},
    deck::Deck,
};
use anyhow::{anyhow, Result};
use extism_pdk::*;
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
    Trashes,
    Excluded,
    Hands(String),
}

impl std::fmt::Display for FieldKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldKey::Trashes => write!(f, "trashes"),
            FieldKey::Excluded => write!(f, "excluded"),
            FieldKey::Hands(id) => write!(f, "{}", id),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, ToBytes, FromBytes)]
#[encoding(Json)]
pub struct Game {
    // game state
    pub prompts: Vec<Prompt>,
    // workaround for "key must be a string" error
    // https://stackoverflow.com/questions/51276896/how-do-i-use-serde-to-serialize-a-hashmap-with-structs-as-keys-to-json
    #[serde_as(as = "Vec<(_, _)>")]
    pub fields: HashMap<FieldKey, Deck>,
    // river
    pub river: Vec<Vec<Card>>,
    pub river_size: Option<usize>,
    pub suit_bans: HashSet<Suit>,
    /// a number includes `effect_limits` ignore effect
    pub effect_bans: HashSet<u8>,
    /// card strength is reversed until the river is reset
    pub turn_ord_reversed: bool,
    /// when `is_step` is true, delta of previous cards number must be 1
    pub is_step: bool,
    /// when true, card strength is reversed
    pub ord_reversed: bool,
    pub current: Option<String>,
    pub last_served_player_id: Option<String>,
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
        fields.insert(FieldKey::Trashes, Deck::new(vec![]));
        fields.insert(FieldKey::Excluded, Deck::new(vec![]));

        Self {
            prompts: vec![],
            fields,

            river: vec![],
            river_size: None,
            suit_bans: HashSet::new(),
            effect_bans: HashSet::new(),
            turn_ord_reversed: false,
            is_step: false,
            ord_reversed: false,

            current: None,
            last_served_player_id: None,

            players: player_ids.clone(),
            answers: HashMap::new(),
            selects: HashMap::from_iter(player_ids.iter().map(|id| (id.to_string(), Vec::new()))),
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

    pub fn toggle_select(&mut self, player_id: &str, card: Card) -> Result<()> {
        let selects = self.selects.get_mut(player_id).unwrap();
        if let Some(index) = selects.iter().position(|c| c == &card) {
            selects.remove(index);
        } else {
            selects.push(card);
        }
        Ok(())
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
}
