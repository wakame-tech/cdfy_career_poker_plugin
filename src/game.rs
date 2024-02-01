use crate::{
    card::{card_ord, cardinal, is_same_number, match_suits, number, suits, Card, Suit},
    deck::{deck_ord, Deck},
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
enum FieldKey {
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

pub trait EventHandler {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Distribute;

impl EventHandler for Distribute {
    fn on(&self, _player_id: String, game: &mut Game) -> Result<()> {
        if game.players.is_empty() {
            return Err(anyhow!("players is empty"));
        }
        let mut deck = Deck::all(2);
        deck.shuffle();
        let mut decks = deck.split(game.players.len())?;
        for (i, player_id) in game.players.iter().enumerate() {
            decks[i].sort(card_ord);
            game.fields
                .insert(FieldKey::Hands(player_id.to_string()), decks[i].clone());
        }
        game.current = Some(game.players[0].clone());
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Select {
    pub field: String,
    pub card: Card,
}

impl EventHandler for Select {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()> {
        if game.selects.get(&player_id).unwrap().contains(&self.card) {
            let index = game
                .selects
                .get(&player_id)
                .unwrap()
                .iter()
                .position(|c| c == &self.card)
                .unwrap();
            game.selects.get_mut(&player_id).unwrap().remove(index);
        } else {
            game.selects.get_mut(&player_id).unwrap().push(self.card);
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Answer {
    pub answer: String,
}

impl EventHandler for Answer {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()> {
        if let Some(prompt) = game.prompt.clone().last() {
            // validate answer
            game.validate_prompt(prompt, player_id.to_string(), self.answer.clone())?;
            game.answers.insert(player_id.to_string(), self.answer);

            // check all players answers
            if prompt.player_ids.iter().collect::<HashSet<_>>()
                == game.answers.keys().collect::<HashSet<_>>()
            {
                game.answers.clear();
                let answer_prompt = match prompt.kind {
                    PromptKind::Select4 => AnswerPromptSelect4,
                    _ => todo!(),
                };
                answer_prompt.on(player_id, game)?;

                // reset select
                game.selects.insert(player_id.to_string(), vec![]);

                game.last_served_player_id = Some(player_id.to_string());
                game.on_end_turn()?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidatePromptSelect4;

impl EventHandler for ValidatePromptSelect4 {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()> {
        let n_cards = game.river.last().unwrap().len();
        if game.selects.get(&player_id).unwrap().len() != n_cards {
            return Err(anyhow!("please select {} cards in trushes", n_cards));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnswerPromptSelect4;

impl EventHandler for AnswerPromptSelect4 {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()> {
        let cards = game.selects.get(&player_id).unwrap().clone();
        game.transfer(
            &FieldKey::Trushes,
            &FieldKey::Hands(player_id.clone()),
            cards,
        )?;
        game.field_mut(&FieldKey::Hands(player_id))?.sort(card_ord);
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidatePromptSelect7;

impl EventHandler for ValidatePromptSelect7 {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()> {
        let n_cards = game.river.last().unwrap().len();
        if game.selects.get(&player_id).unwrap().len() != n_cards {
            return Err(anyhow!("please select {} cards in hands", n_cards));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnswerPromptSelect7;

impl EventHandler for AnswerPromptSelect7 {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()> {
        let cards = game.selects.get(&player_id).unwrap().clone();
        let passer: String = game.get_relative_player(&player_id, -1);
        game.transfer(
            &FieldKey::Hands(player_id.to_string()),
            &FieldKey::Hands(passer),
            cards,
        )?;
        game.field_mut(&FieldKey::Hands(passer))?.sort(card_ord);
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidatePromptSelect13;

impl EventHandler for ValidatePromptSelect13 {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()> {
        let n_cards = game.river.last().unwrap().len();
        if game.selects.get(&player_id).unwrap().len() != n_cards {
            return Err(anyhow!("please select {} cards in excluded", n_cards));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnswerPromptSelect13;

impl EventHandler for AnswerPromptSelect13 {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()> {
        let cards = game.selects.get(&player_id).unwrap().clone();

        game.transfer(&FieldKey::Excluded, &FieldKey::Hands(player_id), cards)?;
        game.field_mut(&FieldKey::Hands(player_id.to_string()))?
            .sort(card_ord);
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidatePromptSelectOneChance {
    answer: String,
}

impl EventHandler for ValidatePromptSelectOneChance {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()> {
        let serves = game.selects.get(&player_id).unwrap().clone();
        if self.answer == "serve".to_string() && (serves.len() != 1 || number(&serves) != 1) {
            return Err(anyhow!("please select A"));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnswerPromptSelectOneChance;

impl EventHandler for AnswerPromptSelectOneChance {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()> {
        let serves = game
            .river
            .last()
            .cloned()
            .expect("river is empty on UseOneChance");
        let event = EffectCard { serves };
        event.on(player_id, game)?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Serve;

impl EventHandler for Serve {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()> {
        if let Some(prompt) = game.prompt.first() {
            if prompt.player_ids.contains(&player_id) && !game.answers.contains_key(&player_id) {
                return Err(anyhow!("please answer"));
            }
        }

        let serves = game.selects.get(&player_id).unwrap().clone();
        // reset select
        game.selects.insert(player_id.to_string(), vec![]);

        if game.current != Some(player_id.clone()) {
            return Err(anyhow!("not your turn"));
        }
        if serves.is_empty() {
            return Err(anyhow!("please select cards"));
        }
        if let Err(e) = game.servable(&serves) {
            return Err(anyhow!("{}", e));
        }

        game.field_mut(&FieldKey::Hands(player_id))?
            .remove(&serves)?;
        game.river.push(serves.clone());

        let has_1_player_ids = game
            .active_player_ids()
            .iter()
            .filter(|id| {
                id != &&player_id
                    && game
                        .field(&FieldKey::Hands(id.to_string()))
                        .unwrap()
                        .0
                        .iter()
                        .any(|c| c.number() == Some(1))
            })
            .cloned()
            .collect::<Vec<_>>();

        if !game.effect.effect_limits.contains(&1) && !has_1_player_ids.is_empty() {
            let prompt = Prompt {
                kind: PromptKind::UseOneChance,
                player_ids: has_1_player_ids,
                question: "select A if use one chance".to_string(),
                options: vec!["serve".to_string(), "skip".to_string()],
            };
            game.prompt.push(prompt);
        }
        // end phase
        if game.prompt.is_empty() {
            let player_id = game.current.clone().unwrap();
            let event = EffectCard { serves };
            event.on(player_id, game)?;
            game.last_served_player_id = Some(player_id.to_string());
            game.on_end_turn()?;
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Pass;

impl EventHandler for Pass {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()> {
        if let Some(prompt) = game.prompt.first() {
            if prompt.player_ids.contains(&player_id) && !game.answers.contains_key(&player_id) {
                return Err(anyhow!("please answer"));
            }
        }

        if game.current != Some(player_id.clone()) {
            return Err(anyhow!("not your turn"));
        }
        if game.river.is_empty() {
            return Err(anyhow!("cannot pass because river is empty"));
        }
        game.on_end_turn()?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EffectCard {
    serves: Vec<Card>,
}

impl EventHandler for EffectCard {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()> {
        let serves = self.serves.clone();
        game.effect.river_size = Some(serves.len());

        if serves.len() == 4 {
            game.effect.revoluted = !game.effect.revoluted;
        }

        game.effect.river_size = Some(serves.len());

        let n = number(&serves);
        if game.effect.effect_limits.contains(&n) {
            return Ok(());
        }

        let hands = game.field(&FieldKey::Hands(player_id))?;
        match n {
            3 => game.effect.effect_limits.extend(1..=13),
            4 => {
                let trushes = game.field(&FieldKey::Trushes)?;
                if hands.0.is_empty() || trushes.0.is_empty() {
                    return Ok(());
                }
                let prompt = Prompt {
                    kind: PromptKind::Select4,
                    player_ids: vec![player_id.to_string()],
                    question: "select cards from trushes".to_string(),
                    options: vec!["ok".to_string()],
                };
                game.prompt.push(prompt);
            }
            5 => {}
            6 => {}
            7 => {
                if hands.0.is_empty() {
                    return Ok(());
                }
                let prompt = Prompt {
                    kind: PromptKind::Select7,
                    player_ids: vec![player_id.to_string()],
                    question: "select cards from hands".to_string(),
                    options: vec!["ok".to_string()],
                };
                game.prompt.push(prompt);
            }
            8 => {}
            9 => {
                game.effect.river_size = match game.effect.river_size {
                    Some(1) => Some(3),
                    Some(3) => Some(1),
                    n => n,
                };
            }
            10 => {
                game.effect.effect_limits.extend(1..10);
            }
            11 => {
                game.effect.turn_revoluted = true;
            }
            12 => {
                game.effect.is_step = true;
                game.effect.suit_limits = suits(&serves);
            }
            13 => {
                let excluded = game.field(&FieldKey::Excluded)?;
                if hands.0.is_empty() || excluded.0.is_empty() {
                    return Ok(());
                }
                let prompt = Prompt {
                    kind: PromptKind::Select13,
                    player_ids: vec![player_id.to_string()],
                    question: "select cards from excluded".to_string(),
                    options: vec!["ok".to_string()],
                };
                game.prompt.push(prompt);
            }
            1 => {}
            2 => {}
            _ => {
                return Err(anyhow!("invalid number {}", n));
            }
        };
        Ok(())
    }
}

impl Game {
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
        self.field_mut(from)?.remove(&cards);
        self.field_mut(to)?.0.extend(cards.to_vec());
        Ok(())
    }
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

    fn active_player_ids(&self) -> Vec<String> {
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

    pub fn servable(&self, serves: &[Card]) -> Result<()> {
        if !is_same_number(serves) {
            return Err(anyhow!("not same number"));
        }
        let Some(top) = self.river.last() else {
            // river is empty
            return Ok(());
        };
        // check ordering
        let ordering = if self.effect.revoluted ^ self.effect.turn_revoluted {
            deck_ord(serves, &top).reverse()
        } else {
            deck_ord(serves, &top)
        };
        if ordering.is_lt() {
            return Err(anyhow!("must be greater than top card"));
        }
        // check river size
        let river_size = self.effect.river_size.unwrap();
        let expected_river_size = match number(serves) {
            9 if !self.effect.effect_limits.contains(&9) => match river_size {
                1 => 3,
                3 => 1,
                n => n,
            },
            _ => serves.len(),
        };
        if river_size != expected_river_size {
            return Err(anyhow!(
                "expected river size {} but {}",
                expected_river_size,
                river_size
            ));
        }
        // check steps
        if self.effect.is_step && cardinal(number(serves)) - cardinal(number(&top)) != 1 {
            return Err(anyhow!("must be step"));
        }
        // check suits
        if !self.effect.suit_limits.is_empty() && !match_suits(&top, serves) {
            return Err(anyhow!(
                "expected suits {:?} but {:?}",
                self.effect.suit_limits,
                suits(serves)
            ));
        }
        Ok(())
    }

    fn on_end_turn(&mut self) -> Result<()> {
        let player_id = self.current.clone().unwrap();

        let hand = self.field(&FieldKey::Hands(player_id))?;
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
