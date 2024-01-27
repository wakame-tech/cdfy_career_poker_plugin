#[cfg(not(target_arch = "wasm32"))]
use crate::mock::*;
use crate::{
    card::Card,
    deck::{is_same_number, match_suits, number, remove_items, with_jokers, Deck, DeckStyle},
    effect::{effect_card, effect_one_chance, servable_9, Effect},
    will_flush,
};
use anyhow::{anyhow, Result};
#[cfg(target_arch = "wasm32")]
use cdfy_sdk::{debug, rand};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CareerPokerState {
    pub room_id: String,
    pub current: Option<String>,
    pub players: Vec<String>,
    pub river: Vec<Deck>,
    pub will_flush_task_id: Option<String>,
    pub last_served_player_id: Option<String>,
    /// players deck + trushes + excluded
    pub fields: HashMap<String, Deck>,
    pub effect: Effect,
    /// pair of user id to deck id for prompt cards
    pub prompts: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Action {
    Distribute,
    Pass {
        player_id: String,
    },
    Flush {
        to: String,
    },
    OneChance {
        player_id: String,
        serves: Vec<Card>,
    },
    Select {
        from: String,
        player_id: String,
        serves: Vec<Card>,
    },
    ServeAnother {
        player_id: String,
        serves: Vec<Card>,
    },
    Serve {
        player_id: String,
        serves: Vec<Card>,
    },
}

impl Default for CareerPokerState {
    fn default() -> Self {
        Self {
            room_id: "".to_string(),
            river: vec![],
            will_flush_task_id: None,
            players: vec![],
            last_served_player_id: None,
            current: None,
            fields: HashMap::from_iter(vec![
                ("trushes".to_string(), Deck::new(vec![])),
                ("excluded".to_string(), Deck::new(vec![])),
            ]),
            effect: Effect::new(),
            prompts: HashMap::new(),
        }
    }
}

impl CareerPokerState {
    pub fn new(room_id: String) -> Self {
        Self {
            room_id,
            ..Default::default()
        }
    }

    fn reset(&mut self) -> Self {
        Self {
            room_id: self.room_id.clone(),
            players: self.players.clone(),
            ..Default::default()
        }
    }

    fn deck_mut(&mut self, id: &str) -> Result<&mut Deck> {
        let Some(deck) = self.fields.get_mut(id) else {
            return Err(anyhow!("field {} not found", id));
        };
        Ok(deck)
    }

    fn deck(&self, id: &str) -> Result<&Deck> {
        let Some(deck) = self.fields.get(id) else {
            return Err(anyhow!("field {} not found", id));
        };
        Ok(deck)
    }

    pub fn join(&mut self, player_id: String) {
        if !self.players.contains(&player_id) {
            self.players.push(player_id.clone());
        }
    }

    pub fn leave(&mut self, player_id: String) {
        if let Some(i) = self.players.iter().position(|id| id == &player_id) {
            self.players.remove(i);
            self.fields.remove(&player_id);
        }
    }

    fn distribute(&mut self) -> Result<()> {
        if self.players.is_empty() {
            return Err(anyhow!("players is empty"));
        }
        self.reset();
        let cards = with_jokers(2);
        for player_id in self.players.iter() {
            self.fields.insert(
                player_id.to_string(),
                Deck {
                    cards: vec![],
                    style: DeckStyle::Arrange,
                },
            );
        }

        for (i, card) in cards.into_iter().enumerate() {
            let player_id = &self.players[i % self.players.len()];
            if let Some(hand) = self.fields.get_mut(player_id) {
                hand.cards.push(card);
            }
        }
        for player_id in self.players.iter() {
            if let Some(hand) = self.fields.get_mut(player_id) {
                hand.cards.sort_by(|a, b| card_ord(a, b))
            }
        }
        self.current = Some(self.players[0].clone());
        Ok(())
    }

    pub fn get_relative_player(&self, player_id: &str, d: i32) -> Option<String> {
        let player_index = self.players.iter().position(|id| id == &player_id).unwrap();
        let mut delta: i32 = d;
        loop {
            let index =
                ((player_index as i32 + delta).rem_euclid(self.players.len() as i32)) as usize;
            if let Some(hand) = self.fields.get(&self.players[index]) {
                if !hand.cards.is_empty() {
                    return Some(self.players[index].clone());
                }
            }
            if delta as usize == self.players.len() {
                return None;
            }
            delta += 1;
        }
    }

    pub fn will_flush(&mut self, player_id: &str, to: &str) {
        self.will_flush_task_id = Some(will_flush(
            player_id.to_string(),
            self.room_id.to_string(),
            to.to_string(),
        ));
    }

    pub fn flush(&mut self, to: String) -> Result<()> {
        let cards = self
            .river
            .iter()
            .map(|d| d.cards.clone())
            .flatten()
            .collect::<Vec<_>>();
        let Some(deck) = self.fields.get_mut(to.as_str()) else {
            return Err(anyhow!("field {} not found", to));
        };
        deck.cards.extend(cards);
        self.effect = Effect::new_turn(self.effect.clone());
        self.river.clear();
        self.current = self.last_served_player_id.clone();
        Ok(())
    }

    pub fn next(&mut self, player_id: &str) {
        self.current = self.get_relative_player(&player_id, 1);
        if self.current == self.last_served_player_id || self.current.is_none() {
            self.will_flush(player_id, "trushes");
        }
    }

    fn pass(&mut self, player_id: String) -> Result<()> {
        if self.river.is_empty() {
            return Err(anyhow!("cannot pass because river is empty"));
        }
        self.next(&player_id);
        Ok(())
    }

    fn transfer(&mut self, from_deck_id: &str, to_deck_id: &str, cards: &Vec<Card>) -> Result<()> {
        let from_deck = self.deck_mut(from_deck_id)?;
        remove_items(&mut from_deck.cards, &cards);
        let to_deck = self.deck_mut(to_deck_id)?;
        to_deck.cards.extend(cards.clone());
        Ok(())
    }

    fn select_trushes(&mut self, player_id: String, serves: Vec<Card>) -> Result<()> {
        let Some(lasts) = self.river.last() else {
           return Err(anyhow!("river is empty")); 
        };
        let n = self.deck("trushes")?.cards.len().min(lasts.cards.len());
        if n != serves.len() {
            return Err(anyhow!("invalid serves size"));
        }
        self.transfer("trushes", player_id.as_str(), &serves)?;
        self.prompts.remove(&player_id);
        self.next(&player_id);
        Ok(())
    }

    fn select_excluded(&mut self, player_id: String, serves: Vec<Card>) -> Result<()> {
        let Some(lasts) = self.river.last() else {
           return Err(anyhow!("river is empty")); 
        };
        let n = self.deck("trushes")?.cards.len().min(lasts.cards.len());
        if n != serves.len() {
            return Err(anyhow!("invalid serves size"));
        }
        self.transfer("excluded", player_id.as_str(), &serves)?;
        self.prompts.remove(&player_id);
        self.next(&player_id);
        Ok(())
    }

    fn select_passes(&mut self, player_id: String, serves: Vec<Card>) -> Result<()> {
        let Some(lasts) = self.river.last() else {
           return Err(anyhow!("river is empty")); 
        };
        let n = self.deck(&player_id)?.cards.len().min(lasts.cards.len());
        if n != serves.len() {
            return Err(anyhow!("invalid serves size"));
        }
        let left_id = self.get_relative_player(&player_id, -1).unwrap();
        self.transfer(&player_id, &left_id, &serves)?;
        self.prompts.remove(&player_id);
        self.next(&player_id);
        Ok(())
    }

    fn one_chance(&mut self, player_id: String, serves: Vec<Card>) -> Result<()> {
        let hand = self.deck(&player_id)?;
        // cannot move up a game using OneChance
        if self.effect.effect_limits.contains(&1) || hand.cards == serves {
            return Err(anyhow!("cannot move up a game using OneChance"));
        }
        self.transfer(&player_id, "trushes", &serves)?;

        // FIXME: use a result of janken subgame
        let active_players = self
            .fields
            .values()
            .filter(|hand| !hand.cards.is_empty())
            .count();
        if rand() % active_players as u32 != 0 {
            return Ok(());
        }
        effect_one_chance(self, &player_id, &serves);
        Ok(())
    }

    fn serve(&mut self, player_id: String, serves: Vec<Card>) -> Result<()> {
        if serves.is_empty() || !servable(&self, &serves) {
            return Err(anyhow!("not servable"));
        }
        let hand = self.deck_mut(&player_id)?;
        remove_items(&mut hand.cards, &serves);
        effect_card(self, &player_id, &serves);

        let hand = self.deck(&player_id)?;
        if hand.cards.is_empty() {
            self.last_served_player_id = self.get_relative_player(&player_id, 1);
        } else {
            self.last_served_player_id = self.get_relative_player(&player_id, 0);
        }
        if self.last_served_player_id.is_none() {
            return Err(anyhow!("end"));
        }
        Ok(())
    }

    pub fn action(&mut self, action: Action) -> anyhow::Result<()> {
        match action {
            Action::Distribute => self.distribute(),
            Action::Pass { player_id } => self.pass(player_id),
            Action::Flush { to } => self.flush(to),
            Action::OneChance { player_id, serves } => self.one_chance(player_id, serves),
            Action::Select {
                from,
                player_id,
                serves,
            } => match from.as_str() {
                "trushes" => self.select_trushes(player_id, serves),
                "excluded" => self.select_excluded(player_id, serves),
                _ => Err(anyhow!("field {} not found", from)),
            },
            Action::ServeAnother { player_id, serves } => self.select_passes(player_id, serves),
            Action::Serve { player_id, serves } => self.serve(player_id, serves),
        }
    }
}

pub fn cardinal(n: u8) -> i32 {
    ((n + 10) % 13).into()
}

pub fn card_ord(l: &Card, r: &Card) -> Ordering {
    let (ln, rn) = (l.number(), r.number());
    match (ln, rn) {
        (None, None) => Ordering::Less,
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (Some(i), Some(j)) => cardinal(i).cmp(&cardinal(j)),
    }
}

fn vec_ord<T, F>(l: impl Iterator<Item = T>, r: impl Iterator<Item = T>, ord: F) -> Ordering
where
    F: Fn(T, T) -> Ordering,
{
    let orderings = l.zip(r).map(|(a, b)| ord(a, b)).collect::<HashSet<_>>();
    orderings.into_iter().next().unwrap_or(Ordering::Equal)
}

fn deck_ord(lhs: &Vec<Card>, rhs: &Vec<Card>) -> Ordering {
    let (mut lhs, mut rhs) = (lhs.clone(), rhs.clone());
    lhs.sort_by(|a, b| card_ord(a, b));
    rhs.sort_by(|a, b| card_ord(a, b));
    vec_ord(lhs.iter(), rhs.iter(), card_ord)
}

pub fn servable(state: &CareerPokerState, serves: &Vec<Card>) -> bool {
    let mut ok = is_same_number(serves);
    let Some(lasts) = state.river.last() else {
        // river is empty
        return ok;
    };
    let river_size = state.effect.river_size.unwrap();
    // check ordering
    let ordering = if state.effect.revoluted ^ state.effect.turn_revoluted {
        deck_ord(&lasts.cards, serves).reverse()
    } else {
        deck_ord(&lasts.cards, serves)
    };
    ok = ok && ordering.is_lt();

    // check river size
    ok = ok
        && match number(serves) {
            9 if !state.effect.effect_limits.contains(&9) => servable_9(state, serves),
            _ => serves.len() == river_size,
        };
    // check steps
    if state.effect.is_step {
        ok = ok && cardinal(number(serves)) - cardinal(number(&lasts.cards)) == 1;
    }
    // check suits
    if !state.effect.suit_limits.is_empty() {
        ok = ok && match_suits(&lasts.cards, serves);
    }
    ok
}

#[cfg(test)]
mod tests {
    use crate::{
        deck::Deck,
        state::{servable, Action, CareerPokerState},
    };
    use std::collections::HashMap;

    #[test]
    fn test_servable() {
        let mut state = CareerPokerState::new("".to_string());
        let serves = vec!["3h".into(), "3d".into()];
        assert_eq!(servable(&state, &serves), true);

        state.effect.river_size = Some(1);
        state.river.push(Deck::new(vec!["Kh".into()]));

        let serves = vec!["Ah".into()];
        assert_eq!(servable(&state, &serves), true);
    }

    #[test]
    fn test_get_relative_player() {
        let mut state = CareerPokerState::new("".to_string());
        state.fields = HashMap::from_iter(vec![
            ("a".to_string(), Deck::new(vec!["Ah".into()])),
            ("b".to_string(), Deck::new(vec!["Ah".into()])),
            ("c".to_string(), Deck::new(vec!["Ah".into()])),
        ]);
        state.players = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(state.get_relative_player("a", 1), Some("b".to_string()));
        assert_eq!(state.get_relative_player("a", -1), Some("c".to_string()));
        assert_eq!(state.get_relative_player("a", 2), Some("c".to_string()));
        assert_eq!(state.get_relative_player("a", 3), Some("a".to_string()));

        let mut state = CareerPokerState::new("".to_string());
        state.fields = HashMap::from_iter(vec![
            ("a".to_string(), Deck::new(vec!["Ah".into()])),
            ("b".to_string(), Deck::new(vec!["Ah".into()])),
        ]);
        state.players = vec!["a".to_string(), "b".to_string()];
        assert_eq!(state.get_relative_player("a", 1), Some("b".to_string()));
        assert_eq!(state.get_relative_player("a", -1), Some("b".to_string()));
        assert_eq!(state.get_relative_player("a", 2), Some("a".to_string()));
    }

    #[test]
    fn test_effect_12() {
        let mut state = CareerPokerState::new("".to_string());
        state.fields = HashMap::from_iter(vec![
            ("a".to_string(), Deck::new(vec!["Ah".into()])),
            ("b".to_string(), Deck::new(vec!["Ah".into()])),
        ]);
        state.players = vec!["a".to_string(), "b".to_string()];
        state.serve("a".to_string(), vec!["Qh".into()]);
        println!("{:?}", state.effect);
        assert_eq!(servable(&state, &vec!["Ks".into()]), false);
    }

    #[test]
    fn test_effect_4() {
        let mut state = CareerPokerState::new("".to_string());
        state.fields = HashMap::from_iter(vec![
            ("a".to_string(), Deck::new(vec![])),
            ("trushes".to_string(), Deck::new(vec!["Ah".into()])),
        ]);
        state.river = vec![Deck::new(vec!["4h".into()])];
        state.players = vec!["a".to_string(), "b".to_string()];
        state
            .action(Action::Select {
                from: "trushes".to_string(),
                player_id: "a".to_string(),
                serves: vec!["Ah".into()],
            })
            .unwrap();
        assert_eq!(state.fields.get("trushes").unwrap(), &Deck::new(vec![]));
        assert_eq!(
            state.fields.get("a").unwrap(),
            &Deck::new(vec!["Ah".into()])
        );
    }
}
