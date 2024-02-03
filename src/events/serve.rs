use super::{effect_card::EffectCard, EventHandler};
use crate::{
    card::{cardinal, is_same_number, match_suits, number, suits, Card},
    deck::deck_ord,
    game::{FieldKey, Game, Prompt, PromptKind},
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidateServe {
    serves: Vec<Card>,
}

impl EventHandler for ValidateServe {
    fn on(&self, _player_id: String, game: &mut Game) -> Result<()> {
        if !is_same_number(&self.serves) {
            return Err(anyhow!("not same number"));
        }
        let Some(top) = game.river.last() else {
            // river is empty
            return Ok(());
        };
        // check ordering
        let ordering = if game.revoluted ^ game.turn_revoluted {
            deck_ord(&self.serves, top).reverse()
        } else {
            deck_ord(&self.serves, top)
        };
        if ordering.is_lt() {
            return Err(anyhow!("must be greater than top card"));
        }
        // check river size
        let river_size = game.river_size.unwrap();
        let expected_river_size = match number(&self.serves) {
            9 if !game.effect_limits.contains(&9) => match river_size {
                1 => 3,
                3 => 1,
                n => n,
            },
            _ => self.serves.len(),
        };
        if river_size != expected_river_size {
            return Err(anyhow!(
                "expected river size {} but {}",
                expected_river_size,
                river_size
            ));
        }
        // check steps
        if game.is_step && cardinal(number(&self.serves)) - cardinal(number(top)) != 1 {
            return Err(anyhow!("must be step"));
        }
        // check suits
        if !game.suit_limits.is_empty() && !match_suits(top, &self.serves) {
            return Err(anyhow!(
                "expected suits {:?} but {:?}",
                game.suit_limits,
                suits(&self.serves)
            ));
        }
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
        let validate = ValidateServe {
            serves: serves.clone(),
        };
        validate.on(player_id.clone(), game)?;

        game.field_mut(&FieldKey::Hands(player_id.clone()))?
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

        if !game.effect_limits.contains(&1) && !has_1_player_ids.is_empty() {
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
            event.on(player_id.clone(), game)?;
            game.last_served_player_id = Some(player_id.to_string());
            game.on_end_turn()?;
        }
        Ok(())
    }
}
