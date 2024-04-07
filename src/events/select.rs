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
        if game.selects.get(&player_id).unwrap().contains(&self.card) {
            let index = game
                .selects
                .get(&player_id)
                .unwrap()
                .iter()
                .position(|c| c == &self.card)
                .unwrap();
            game.selects.get_mut(&player_id).unwrap().remove(index);
        } else {
            game.selects
                .get_mut(&player_id)
                .unwrap()
                .push(self.card.clone());
        }
        Ok(Event::None)
    }
}
