use super::{Event, EventHandler};
use crate::game::Game;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Pass;

impl EventHandler for Pass {
    fn on(&self, player_id: String, game: &mut Game) -> Result<Event> {
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
        Ok(Event::None)
    }
}
