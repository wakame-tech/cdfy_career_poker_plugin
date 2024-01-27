#[cfg(not(target_arch = "wasm32"))]
use crate::mock::cancel;
use crate::{
    card::{Card, Suit},
    deck::{number, suits, Deck, DeckStyle},
    state::CareerPokerState,
};
use anyhow::{anyhow, Result};
#[cfg(target_arch = "wasm32")]
use cdfy_sdk::cancel;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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

pub fn effect_revolution(state: &mut CareerPokerState, _player_id: &str, serves: &Vec<Card>) {
    if serves.len() == 4 {
        state.effect.revoluted = !state.effect.revoluted;
    }
}

pub fn effect_3(state: &mut CareerPokerState, player_id: &str, _serves: &Vec<Card>) {
    if !state.effect.effect_limits.contains(&3) {
        state.effect.effect_limits.extend(1..=13)
    }
    state.next(player_id);
}

pub fn effect_4(state: &mut CareerPokerState, player_id: &str, _serves: &Vec<Card>) {
    if !state.effect.effect_limits.contains(&4) {
        let hands = state.fields.get(player_id).expect("deck not found");
        let trushes = state.fields.get("trushes").expect("trushes not found");
        if hands.cards.is_empty() || trushes.cards.is_empty() {
            state.next(player_id);
            return;
        }
        state
            .prompts
            .insert(player_id.to_string(), "trushes".to_string());
    } else {
        state.next(player_id);
    }
}

pub fn effect_5(state: &mut CareerPokerState, player_id: &str, serves: &Vec<Card>) {
    if !state.effect.effect_limits.contains(&5) {
        state.current = state.get_relative_player(player_id, 1 + serves.len() as i32);
        if state.current == Some(player_id.to_string()) {
            state.will_flush(player_id, "trushes");
        }
    } else {
        state.next(player_id);
    }
}

pub fn effect_7(state: &mut CareerPokerState, player_id: &str, _serves: &Vec<Card>) {
    let hands = state.fields.get(player_id).unwrap();
    if !state.effect.effect_limits.contains(&7) && !hands.cards.is_empty() {
        state
            .prompts
            .insert(player_id.to_string(), player_id.to_string());
    } else {
        state.next(player_id);
    }
}

pub fn effect_8(state: &mut CareerPokerState, player_id: &str, _serves: &Vec<Card>) {
    if !state.effect.effect_limits.contains(&8) {
        state.will_flush(player_id, "trushes");
    } else {
        state.next(player_id);
    }
}

pub fn effect_9(state: &mut CareerPokerState, player_id: &str, _serves: &Vec<Card>) {
    if !state.effect.effect_limits.contains(&9) {
        state.effect.river_size = match state.effect.river_size {
            Some(1) => Some(3),
            Some(3) => Some(1),
            n => n,
        };
    }
    state.next(&player_id);
}

pub fn servable_9(state: &CareerPokerState, _serves: &Vec<Card>) -> bool {
    let river_size = state.effect.river_size.unwrap();
    match river_size {
        1 | 3 => river_size == 1 || river_size == 3,
        n => river_size == n,
    }
}

pub fn effect_10(state: &mut CareerPokerState, player_id: &str, _serves: &Vec<Card>) {
    if !state.effect.effect_limits.contains(&10) {
        state.effect.effect_limits.extend(1..10);
    }
    state.next(&player_id);
}

pub fn effect_11(state: &mut CareerPokerState, player_id: &str, _serves: &Vec<Card>) {
    if !state.effect.effect_limits.contains(&11) {
        state.effect.turn_revoluted = true;
    }
    state.next(&player_id);
}

pub fn effect_12(state: &mut CareerPokerState, player_id: &str, serves: &Vec<Card>) {
    if !state.effect.effect_limits.contains(&12) {
        state.effect.is_step = true;
        state.effect.suit_limits.extend(suits(serves));
    }
    state.next(&player_id);
}

pub fn effect_13(state: &mut CareerPokerState, player_id: &str, _serves: &Vec<Card>) {
    if !state.effect.effect_limits.contains(&13) {
        let hands = state.fields.get(player_id).unwrap();
        let excluded = state.fields.get("excluded").expect("excluded not found");
        if hands.cards.is_empty() || excluded.cards.is_empty() {
            state.next(&player_id);
        }
        state
            .prompts
            .insert(player_id.to_string(), "excluded".to_string());
    } else {
        state.next(&player_id);
    }
}

pub fn effect_one_chance(state: &mut CareerPokerState, player_id: &str, serves: &Vec<Card>) {
    if let Some(task_id) = state.will_flush_task_id.as_ref() {
        cancel(state.room_id.clone(), task_id.to_string());
    }
    state.flush("trushes".to_string());
    let trushes = state.fields.get_mut("trushes").expect("trushes not found");
    trushes.cards.extend(serves.clone());
    state.current = Some(player_id.to_string());
}

pub fn effect_2(state: &mut CareerPokerState, player_id: &str, _serves: &Vec<Card>) {
    let hands = state.fields.get(player_id).unwrap();
    let trushes = state.fields.get("trushes").expect("trushes not found");
    if !state.effect.effect_limits.contains(&2)
        && !hands.cards.is_empty()
        && !trushes.cards.is_empty()
    {
        state.will_flush(player_id, "excluded");
    } else {
        state.next(&player_id);
    }
}

pub fn effect_card(
    state: &mut CareerPokerState,
    player_id: &str,
    serves: &Vec<Card>,
) -> Result<()> {
    state.effect.river_size = Some(serves.len());

    effect_revolution(state, player_id, serves);
    state.river.push(Deck {
        style: DeckStyle::Arrange,
        cards: serves.clone(),
    });
    state.effect.river_size = Some(serves.len());

    let n = number(serves);
    match n {
        3 => effect_3(state, player_id, serves),
        4 => effect_4(state, player_id, serves),
        5 => effect_5(state, player_id, serves),
        6 => state.next(&player_id),
        7 => effect_7(state, player_id, serves),
        8 => effect_8(state, player_id, serves),
        9 => effect_9(state, player_id, serves),
        10 => effect_10(state, player_id, serves),
        11 => effect_11(state, player_id, serves),
        12 => effect_12(state, player_id, serves),
        13 => effect_13(state, player_id, serves),
        1 => state.next(&player_id),
        2 => effect_2(state, player_id, serves),
        14 => state.next(&player_id),
        _ => {
            return Err(anyhow!("invalid number {}", n));
        }
    };
    Ok(())
}
