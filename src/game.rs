use crate::{
    card::{card_ord, cardinal, is_same_number, match_suits, number, suits, Card, Suit},
    deck::{deck_ord, Deck},
    plugin::LiveEvent,
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PromptKind {
    Select4,
    Select7,
    Select13,
    OneChance,
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
            kind: PromptKind::OneChance,
            player_ids,
            question: "select A if use one chance".to_string(),
            options: vec!["ok".to_string()],
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
    /// 5
    pub skip: bool,
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
            skip: false,
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
        field: String,
    },
    Pass {
        player_id: String,
    },
}

impl Action {
    pub fn from_event(event: &LiveEvent) -> Result<Self> {
        if event.event_name == "reset" {
            return Ok(Action::Reset);
        }
        if event.event_name == "distribute" {
            return Ok(Action::Distribute);
        }
        if event.event_name == "select" {
            return Ok(Action::Select {
                field: event.value.get("field").unwrap().clone(),
                player_id: event.player_id.clone(),
                card: Card::try_from(event.value.get("card").unwrap().as_str())?,
            });
        }
        if event.event_name == "answer" {
            return Ok(Action::Answer {
                player_id: event.player_id.clone(),
                answer: event.value.get("answer").unwrap().to_string(),
            });
        }
        Err(anyhow!("invalid event"))
    }
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
        self.fields
            .iter()
            .filter(|(_, deck)| !deck.0.is_empty())
            .map(|(id, _)| id.clone())
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
                    answers.insert(player_id.clone(), Some(answer));

                    // check all players answers
                    if prompt.player_ids == answers.keys().cloned().collect::<Vec<_>>() {
                        match prompt.kind {
                            PromptKind::Select4 => {
                                let cards = self.selects.get(&player_id).unwrap().clone();
                                self.transfer("trushes", &player_id, cards)?;
                                self.on_end_turn(&player_id)?;
                            }
                            PromptKind::Select7 => {
                                let cards = self.selects.get(&player_id).unwrap().clone();
                                let passer = self.get_relative_player(&player_id, -1).unwrap();
                                self.transfer(&player_id, &passer, cards)?;
                                self.on_end_turn(&player_id)?;
                            }
                            PromptKind::Select13 => {
                                let cards = self.selects.get(&player_id).unwrap().clone();
                                self.transfer("excluded", &player_id, cards)?;
                            }
                            PromptKind::OneChance => {
                                let player_id = self.current.clone().unwrap();
                                let serves = self.selects.get(&player_id).unwrap().clone();
                                self.effect_card(&player_id, serves)?;
                                self.on_end_turn(&player_id)?;
                            }
                        }
                    }
                }
                Ok(())
            }
            Action::Pass { player_id } => {
                if self.river.is_empty() {
                    return Err(anyhow!("cannot pass because river is empty"));
                }
                self.on_end_turn(&player_id)?;
                Ok(())
            }
            Action::Serve { player_id, .. } => {
                let serves = self.selects.get(&player_id).unwrap().clone();
                if serves.is_empty() || !self.servable(&serves) {
                    return Err(anyhow!("not servable"));
                }
                self.deck_mut(&player_id)?.remove(&serves)?;
                self.prompt_one_chance(&player_id);
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

    pub fn get_relative_player(&self, player_id: &str, d: i32) -> Option<String> {
        let index = self.players.iter().position(|id| id == &player_id).unwrap();
        let index = ((index as i32 + d).rem_euclid(self.active_player_ids().len() as i32)) as usize;
        Some(self.active_player_ids()[index].clone())
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
        self.current = self.last_served_player_id.clone();
        Ok(())
    }

    pub fn prompt_one_chance(&mut self, player_id: &str) {
        let player_ids = self
            .active_player_ids()
            .iter()
            .filter(|id| id != &&player_id)
            .cloned()
            .collect::<Vec<_>>();
        self.prompt = Some((Prompt::one_chance(player_ids), HashMap::new()));
    }

    pub fn effect_card(&mut self, player_id: &str, serves: Vec<Card>) -> Result<()> {
        self.effect.river_size = Some(serves.len());

        if serves.len() == 4 {
            self.effect.revoluted = !self.effect.revoluted;
        }

        self.river.push(serves.to_vec());
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
            5 => {
                self.effect.skip = true;
            }
            6 => {}
            7 => {
                if !hands.0.is_empty() {
                    self.prompt = Some((Prompt::select_7(player_id), HashMap::new()));
                }
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
                self.effect.suit_limits.extend(suits(&serves));
            }
            13 => {
                let excluded = self.deck("excluded")?;
                if hands.0.is_empty() || excluded.0.is_empty() {
                    return Ok(());
                }
                self.prompt = Some((Prompt::select_13(player_id), HashMap::new()));
            }
            // TODO
            1 => {}
            2 => {}
            _ => {
                return Err(anyhow!("invalid number {}", n));
            }
        };
        Ok(())
    }

    fn servable_9(&self, _serves: &[Card]) -> bool {
        let river_size = self.effect.river_size.unwrap();
        match river_size {
            1 | 3 => river_size == 1 || river_size == 3,
            n => river_size == n,
        }
    }

    pub fn servable(&self, serves: &[Card]) -> bool {
        let mut ok = is_same_number(serves);
        let Some(top) = self.river.last() else {
            // river is empty
            return ok;
        };
        let river_size = self.effect.river_size.unwrap();
        // check ordering
        let ordering = if self.effect.revoluted ^ self.effect.turn_revoluted {
            deck_ord(&top, serves).reverse()
        } else {
            deck_ord(&top, serves)
        };
        ok = ok && ordering.is_lt();

        // check river size
        ok = ok
            && match number(serves) {
                9 if !self.effect.effect_limits.contains(&9) => self.servable_9(serves),
                _ => serves.len() == river_size,
            };
        // check steps
        if self.effect.is_step {
            ok = ok && cardinal(number(serves)) - cardinal(number(&top)) == 1;
        }
        // check suits
        if !self.effect.suit_limits.is_empty() {
            ok = ok && match_suits(&top, serves);
        }
        ok
    }

    fn on_end_turn(&mut self, player_id: &str) -> Result<()> {
        let hand = self.deck(&player_id)?;
        if hand.0.is_empty() {
            self.last_served_player_id = self.get_relative_player(&player_id, 1);
        } else {
            self.last_served_player_id = self.get_relative_player(&player_id, 0);
        }
        if self.last_served_player_id.is_none() {
            return Err(anyhow!("end"));
        }

        // flush
        if self.current == self.last_served_player_id || self.current.is_none() {
            let to = if let Some(top) = self.river.last() {
                let n = number(top);
                if n == 2 && !self.effect.effect_limits.contains(&2) {
                    "excluded"
                } else {
                    "trushes"
                }
            } else {
                "trushes"
            };
            self.flush(to)?;
        }
        Ok(())
    }
}
