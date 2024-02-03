use crate::game::Game;
use anyhow::Result;

pub mod answer;
pub mod distribute;
pub mod effect_card;
pub mod pass;
pub mod select;
pub mod serve;

pub trait EventHandler {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()>;
}
