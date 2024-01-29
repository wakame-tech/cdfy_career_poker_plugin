use crate::{
    card::{card_ord, cardinal, is_same_number, match_suits, number, suits, Card, Suit},
    deck::{deck_ord, Deck},
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PromptKind {
    Select4,
    Select7,
    Select13,
    UseOneChance,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Prompt {
    pub kind: PromptKind,
    pub player_ids: Vec<String>,
    pub question: String,
    pub options: Vec<String>,
}

impl Prompt {
    pub fn select_4(player_id: &str) -> Self {
        Self {
            kind: PromptKind::Select4,
            player_ids: vec![player_id.to_string()],
            question: "select cards from trushes".to_string(),
            options: vec!["ok".to_string()],
        }
    }

    pub fn select_7(player_id: &str) -> Self {
        Self {
            kind: PromptKind::Select7,
            player_ids: vec![player_id.to_string()],
            question: "select cards from hands".to_string(),
            options: vec!["ok".to_string()],
        }
    }

    pub fn select_13(player_id: &str) -> Self {
        Self {
            kind: PromptKind::Select13,
            player_ids: vec![player_id.to_string()],
            question: "select cards from excluded".to_string(),
            options: vec!["ok".to_string()],
        }
    }

    pub fn one_chance(player_ids: Vec<String>) -> Self {
        Self {
            kind: PromptKind::UseOneChance,
            player_ids,
            question: "select A if use one chance".to_string(),
            options: vec!["serve".to_string(), "skip".to_string()],
        }
    }
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Game {
    pub players: Vec<String>,
    pub selects: HashMap<String, Vec<Card>>,
    pub prompt: Option<(Prompt, HashMap<String, Option<String>>)>,
    pub current: Option<String>,
    pub last_served_player_id: Option<String>,

    pub river: Vec<Vec<Card>>,
    /// players deck + trushes + excluded
    pub fields: HashMap<String, Deck>,
    pub effect: Effect,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Action {
    Reset,
    Distribute,
    Select {
        field: String,
        player_id: String,
        card: Card,
    },
    Answer {
        player_id: String,
        answer: String,
    },
    Serve {
        player_id: String,
    },
    Pass {
        player_id: String,
    },
}

impl Game {
    pub fn deck_mut(&mut self, id: &str) -> Result<&mut Deck> {
        let Some(deck) = self.fields.get_mut(id) else {
            return Err(anyhow!("field {} not found", id));
        };
        Ok(deck)
    }

    pub fn deck(&self, id: &str) -> Result<&Deck> {
        let Some(deck) = self.fields.get(id) else {
            return Err(anyhow!("field {} not found", id));
        };
        Ok(deck)
    }

    pub fn transfer(&mut self, from: &str, to: &str, cards: Vec<Card>) -> Result<()> {
        self.deck_mut(from)?.remove(&cards)?;
        self.deck_mut(to)?.0.extend(cards.to_vec());
        Ok(())
    }
}

impl Game {
    pub fn new(player_ids: Vec<String>) -> Self {
        let player_decks = player_ids
            .iter()
            .map(|id| (id.clone(), Deck::new(vec![])))
            .collect::<HashMap<_, _>>();
        let mut fields = HashMap::from_iter(vec![
            ("trushes".to_string(), Deck::new(vec![])),
            ("excluded".to_string(), Deck::new(vec![])),
        ]);
        fields.extend(player_decks);

        Self {
            players: player_ids.clone(),
            prompt: None,
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
            .filter(|id| !self.deck(id).unwrap().0.is_empty())
            .cloned()
            .collect()
    }

    pub fn apply_action(&mut self, action: Action) -> Result<()> {
        match action {
            // game actions
            Action::Reset => {
                *self = Self::new(self.players.clone());
                Ok(())
            }
            Action::Distribute => {
                if self.players.is_empty() {
                    return Err(anyhow!("players is empty"));
                }
                let mut deck = Deck::all(2);
                deck.shuffle();
                let mut decks = deck.split(self.players.len())?;
                for (i, player_id) in self.players.iter().enumerate() {
                    decks[i].sort(card_ord);
                    self.fields.insert(player_id.to_string(), decks[i].clone());
                }
                self.current = Some(self.players[0].clone());
                Ok(())
            }
            // player actions
            Action::Select {
                player_id, card, ..
            } => {
                self.toggle_select(&player_id, card);
                Ok(())
            }
            Action::Answer { player_id, answer } => {
                if let Some((prompt, answers)) = &mut self.prompt {
                    if !prompt.options.contains(&answer) {
                        return Err(anyhow!("invalid answer"));
                    }
                    // validate answer
                    match prompt.kind {
                        PromptKind::Select4 => {
                            let n_cards = self.river.last().unwrap().len();
                            if self.selects.get(&player_id).unwrap().len() != n_cards {
                                return Err(anyhow!("please select {} cards in trushes", n_cards));
                            }
                        }
                        PromptKind::Select7 => {
                            let n_cards = self.river.last().unwrap().len();
                            if self.selects.get(&player_id).unwrap().len() != n_cards {
                                return Err(anyhow!("please select {} cards in hands", n_cards));
                            }
                        }
                        PromptKind::Select13 => {
                            let n_cards = self.river.last().unwrap().len();
                            if self.selects.get(&player_id).unwrap().len() != n_cards {
                                return Err(anyhow!("please select {} cards in excluded", n_cards));
                            }
                        }
                        PromptKind::UseOneChance => {
                            let serves = self.selects.get(&player_id).unwrap().clone();
                            if answer == "serve".to_string()
                                && (serves.len() != 1 || number(&serves) != 1)
                            {
                                return Err(anyhow!("please select A"));
                            }
                        }
                    }
                    answers.insert(player_id.clone(), Some(answer));

                    // check all players answers
                    if prompt.player_ids.iter().collect::<HashSet<_>>()
                        == answers.keys().collect::<HashSet<_>>()
                    {
                        match prompt.kind {
                            PromptKind::Select4 => {
                                let cards = self.selects.get(&player_id).unwrap().clone();
                                self.transfer("trushes", &player_id, cards)?;
                                self.deck_mut(&player_id)?.sort(card_ord);
                                self.on_end_turn(&player_id)?;
                            }
                            PromptKind::Select7 => {
                                let cards = self.selects.get(&player_id).unwrap().clone();
                                let passer = self.get_relative_player(&player_id, -1);
                                self.transfer(&player_id, &passer, cards)?;
                                self.deck_mut(&player_id)?.sort(card_ord);
                                self.on_end_turn(&player_id)?;
                            }
                            PromptKind::Select13 => {
                                let cards = self.selects.get(&player_id).unwrap().clone();
                                self.transfer("excluded", &player_id, cards)?;
                                self.deck_mut(&player_id)?.sort(card_ord);
                                self.on_end_turn(&player_id)?;
                            }
                            PromptKind::UseOneChance => {
                                let player_id = self.current.clone().unwrap();
                                let serves = self.river.last().unwrap().clone();
                                self.effect_card(&player_id, &serves)?;
                                self.last_served_player_id = Some(player_id.to_string());
                                self.on_end_turn(&player_id)?;
                            }
                        }
                        // reset prompt
                        self.prompt = None;
                    }
                }
                Ok(())
            }
            Action::Pass { player_id } => {
                if self.current != Some(player_id.clone()) {
                    return Err(anyhow!("not your turn"));
                }
                if self.river.is_empty() {
                    return Err(anyhow!("cannot pass because river is empty"));
                }
                self.on_end_turn(&player_id)?;
                Ok(())
            }
            Action::Serve { player_id, .. } => {
                let serves = self.selects.get(&player_id).unwrap().clone();

                if self.current != Some(player_id.clone()) {
                    // reset select
                    self.selects.insert(player_id.clone(), vec![]);
                    return Err(anyhow!("not your turn"));
                }
                if serves.is_empty() {
                    // reset select
                    self.selects.insert(player_id.clone(), vec![]);
                    return Err(anyhow!("please select cards"));
                }
                if let Err(e) = self.servable(&serves) {
                    // reset select
                    self.selects.insert(player_id.clone(), vec![]);
                    return Err(anyhow!("{}", e));
                }
                self.deck_mut(&player_id)?.remove(&serves)?;

                let serves = self.selects.get(&player_id).unwrap().clone();
                self.river.push(serves.clone());

                let has_1_player_ids = self
                    .active_player_ids()
                    .iter()
                    .filter(|id| {
                        id != &&player_id
                            && self
                                .deck(id)
                                .unwrap()
                                .0
                                .iter()
                                .any(|c| c.number() == Some(1))
                    })
                    .cloned()
                    .collect::<Vec<_>>();

                if !self.effect.effect_limits.contains(&1) && !has_1_player_ids.is_empty() {
                    self.prompt = Some((Prompt::one_chance(has_1_player_ids), HashMap::new()));
                } else {
                    // effect immediately
                    let player_id = self.current.clone().unwrap();
                    self.effect_card(&player_id, &serves)?;
                    self.last_served_player_id = Some(player_id.to_string());
                    self.on_end_turn(&player_id)?;
                }
                Ok(())
            }
        }
    }

    fn toggle_select(&mut self, player_id: &str, card: Card) {
        if self.selects.get(player_id).unwrap().contains(&card) {
            let index = self
                .selects
                .get(player_id)
                .unwrap()
                .iter()
                .position(|c| c == &card)
                .unwrap();
            self.selects.get_mut(player_id).unwrap().remove(index);
        } else {
            self.selects.get_mut(player_id).unwrap().push(card);
        }
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

    fn flush_river(&mut self, to: &str) -> Result<()> {
        let cards = self.river.iter().flatten().cloned().collect::<Vec<_>>();
        self.deck_mut(to)?.0.extend(cards);
        self.river.clear();
        Ok(())
    }

    pub fn flush(&mut self, to: &str) -> Result<()> {
        self.flush_river(to)?;
        self.effect = Effect::new_turn(self.effect.clone());
        Ok(())
    }

    pub fn effect_card(&mut self, player_id: &str, serves: &[Card]) -> Result<()> {
        self.effect.river_size = Some(serves.len());

        if serves.len() == 4 {
            self.effect.revoluted = !self.effect.revoluted;
        }

        self.effect.river_size = Some(serves.len());

        let n = number(&serves);

        let hands = self.deck(player_id)?;
        match n {
            3 => self.effect.effect_limits.extend(1..=13),
            4 => {
                let trushes = self.deck("trushes")?;
                if hands.0.is_empty() || trushes.0.is_empty() {
                    return Ok(());
                }
                self.prompt = Some((Prompt::select_4(player_id), HashMap::new()));
            }
            5 => {}
            6 => {}
            7 => {
                if !hands.0.is_empty() {
                    return Ok(());
                }
                self.prompt = Some((Prompt::select_7(player_id), HashMap::new()));
            }
            8 => {}
            9 => {
                self.effect.river_size = match self.effect.river_size {
                    Some(1) => Some(3),
                    Some(3) => Some(1),
                    n => n,
                };
            }
            10 => {
                self.effect.effect_limits.extend(1..10);
            }
            11 => {
                self.effect.turn_revoluted = true;
            }
            12 => {
                self.effect.is_step = true;
                self.effect.suit_limits = suits(&serves);
            }
            13 => {
                let excluded = self.deck("excluded")?;
                if hands.0.is_empty() || excluded.0.is_empty() {
                    return Ok(());
                }
                self.prompt = Some((Prompt::select_13(player_id), HashMap::new()));
            }
            1 => {}
            2 => {}
            _ => {
                return Err(anyhow!("invalid number {}", n));
            }
        };
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

    fn on_end_turn(&mut self, player_id: &str) -> Result<()> {
        // reset select
        self.selects.insert(player_id.to_string(), vec![]);

        let hand = self.deck(&player_id)?;
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
                    "excluded"
                }
                _ => "trushes",
            };
            self.flush(to)?;
        }

        Ok(())
    }
}
