use super::{Event, EventHandler};
use crate::{card::Card, game::Game};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Select {
    pub field: String,
    pub card: Card,
}

impl EventHandler for Select {
    fn on(&self, player_id: String, game: &mut Game) -> Result<Event> {
        game.toggle_select(&player_id, self.card.clone())?;
        Ok(Event::None)
    }
}
