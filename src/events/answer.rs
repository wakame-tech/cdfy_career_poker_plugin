use super::{effect_card::EffectCard, EventHandler};
use crate::{
    card::{card_ord, number},
    game::{FieldKey, Game, PromptKind},
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Serialize, Deserialize)]
pub struct Answer {
    pub answer: String,
}

impl EventHandler for Answer {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()> {
        let Some(prompt) = game.prompt.last().cloned() else {
            return Err(anyhow!("no prompt"));
        };
        // validate answer
        let validate: Box<dyn EventHandler> = match prompt.kind {
            PromptKind::Select4 => Box::new(ValidatePromptSelect4),
            PromptKind::Select7 => Box::new(ValidatePromptSelect7),
            PromptKind::Select13 => Box::new(ValidatePromptSelect13),
            PromptKind::UseOneChance => Box::new(ValidatePromptSelectOneChance {
                answer: self.answer.clone(),
            }),
        };
        validate.on(player_id.to_string(), game)?;
        game.answers
            .insert(player_id.to_string(), self.answer.clone());

        // check all players answers
        let all_answered = prompt.player_ids.iter().collect::<HashSet<_>>()
            == game.answers.keys().collect::<HashSet<_>>();
        if all_answered {
            game.answers.clear();
            let answer_prompt: Box<dyn EventHandler> = match prompt.kind {
                PromptKind::Select4 => Box::new(AnswerPromptSelect4),
                PromptKind::Select7 => Box::new(AnswerPromptSelect7),
                PromptKind::Select13 => Box::new(AnswerPromptSelect13),
                PromptKind::UseOneChance => Box::new(AnswerPromptSelectOneChance),
            };
            answer_prompt.on(player_id.to_string(), game)?;

            // reset select
            game.selects.insert(player_id.to_string(), vec![]);

            game.last_served_player_id = Some(player_id.to_string());
            game.on_end_turn()?;
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
            &FieldKey::Hands(passer.clone()),
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

        game.transfer(
            &FieldKey::Excluded,
            &FieldKey::Hands(player_id.clone()),
            cards,
        )?;
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
