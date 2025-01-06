use crate::{
    card::{number, suits, Card},
    game::{FieldKey, Game, Prompt, PromptKind},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum CardsEffect {
    Revolutions,
    Styx,
    Reborn(usize),
    Skip(usize),
    InfiniteRevolutions,
    Passer(usize),
    MoveCards(FieldKey, Vec<Card>),
    Asura,
    TenCommandments,
    TurnReversing,
    Mahapadma,
    RoyalRelief,
}

impl CardsEffect {
    pub(crate) fn new(game: &Game, cards: &[Card]) -> Vec<CardsEffect> {
        let mut effects = Vec::new();

        if !game.effect_bans.contains(&3) && cards.len() == 4 {
            effects.push(CardsEffect::Revolutions);
        }

        let n = number(cards);
        let cards_effects = match n {
            3 if !game.effect_bans.contains(&3) => vec![CardsEffect::Styx],
            4 if !game.effect_bans.contains(&4)
                && !game.fields[&FieldKey::Trashes].0.is_empty() =>
            {
                vec![CardsEffect::Reborn(
                    cards.len().min(game.fields[&FieldKey::Trashes].0.len()),
                )]
            }
            5 if !game.effect_bans.contains(&5) => vec![CardsEffect::Skip(cards.len())],
            6 if !game.effect_bans.contains(&6) && cards.len() == 4 => {
                vec![CardsEffect::InfiniteRevolutions]
            }
            7 if !game.effect_bans.contains(&7) => vec![CardsEffect::Passer(cards.len())],
            8 if !game.effect_bans.contains(&8) => {
                vec![CardsEffect::MoveCards(FieldKey::Trashes, cards.to_vec())]
            }
            9 if !game.effect_bans.contains(&9) => vec![CardsEffect::Asura],
            10 if !game.effect_bans.contains(&10) => vec![CardsEffect::TenCommandments],
            11 if !game.effect_bans.contains(&11) => vec![CardsEffect::TurnReversing],
            12 if !game.effect_bans.contains(&12) => vec![CardsEffect::Mahapadma],
            13 if !game.effect_bans.contains(&13)
                && !game.fields[&FieldKey::Excluded].0.is_empty() =>
            {
                vec![CardsEffect::RoyalRelief]
            }
            _ => vec![],
        };
        effects.extend(cards_effects);
        effects
    }
}

pub(crate) fn effect_cards(mut game: Game, effect: CardsEffect) -> Result<Game> {
    let player_id = game.current.clone().unwrap();
    match effect {
        CardsEffect::Revolutions => {
            game.ord_reversed = !game.ord_reversed;
        }
        CardsEffect::Styx => {
            game.effect_bans.extend(1..=13);
        }
        CardsEffect::Reborn(n) => {
            let prompt = Prompt {
                kind: PromptKind::Select4,
                player_ids: vec![player_id.to_string()],
                question: "select cards from trashes".to_string(),
                options: vec!["ok".to_string()],
            };
            game.prompts.push(prompt);
        }
        CardsEffect::Skip(n) => {
            game.river_size = Some(n + 1);
        }
        CardsEffect::InfiniteRevolutions => {
            game.ord_reversed = !game.ord_reversed;
            game.turn_ord_reversed = !game.turn_ord_reversed;
        }
        CardsEffect::Passer(n) => {
            let prompt = Prompt {
                kind: PromptKind::Select7,
                player_ids: vec![player_id.to_string()],
                question: "select cards from hands".to_string(),
                options: vec!["ok".to_string()],
            };
            game.prompts.push(prompt);
        }
        CardsEffect::MoveCards(field, cards) => {
            let deck = game.field_mut(&field)?;
            deck.0.extend(cards.to_vec());
        }
        CardsEffect::Asura => {
            game.river_size = Some(3);
        }
        CardsEffect::TenCommandments => {
            game.effect_bans.extend(1..10);
        }
        CardsEffect::TurnReversing => {
            game.turn_ord_reversed = !game.turn_ord_reversed;
        }
        CardsEffect::Mahapadma => {
            game.is_step = true;
            game.suit_bans = suits(&game.river.last().unwrap());
        }
        CardsEffect::RoyalRelief => {
            let prompt = Prompt {
                kind: PromptKind::Select13,
                player_ids: vec![player_id.to_string()],
                question: "select cards from excluded".to_string(),
                options: vec!["ok".to_string()],
            };
            game.prompts.push(prompt);
        }
    };
    Ok(game)
}
