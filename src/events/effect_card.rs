use super::EventHandler;
use crate::{
    card::{number, suits, Card},
    game::{FieldKey, Game, Prompt, PromptKind},
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct EffectCard {
    pub serves: Vec<Card>,
}

impl EventHandler for EffectCard {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()> {
        let serves = self.serves.clone();
        game.river_size = Some(serves.len());

        if serves.len() == 4 {
            game.revoluted = !game.revoluted;
        }

        game.river_size = Some(serves.len());

        let n = number(&serves);
        if game.effect_limits.contains(&n) {
            return Ok(());
        }

        let hands = game.field(&FieldKey::Hands(player_id.clone()))?;
        match n {
            3 => game.effect_limits.extend(1..=13),
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
                game.river_size = match game.river_size {
                    Some(1) => Some(3),
                    Some(3) => Some(1),
                    n => n,
                };
            }
            10 => {
                game.effect_limits.extend(1..10);
            }
            11 => {
                game.turn_revoluted = true;
            }
            12 => {
                game.is_step = true;
                game.suit_limits = suits(&serves);
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
