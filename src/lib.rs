use crate::{
    events::from_live_event,
    game_view::Ctx,
    plugin::{GameConfig, LiveEvent, RenderConfig},
};
use anyhow::{anyhow, Result};
use extism_pdk::*;
use game::Game;

pub mod card;
pub mod deck;

mod events;
mod game;
mod game_view;
mod plugin;

impl ToBytes<'_> for Game {
    type Bytes = Vec<u8>;

    fn to_bytes(&self) -> Result<Self::Bytes, Error> {
        Ok(serde_json::to_vec(self)?)
    }
}

impl FromBytesOwned for Game {
    fn from_bytes_owned(bytes: &[u8]) -> Result<Self, Error> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[plugin_fn]
pub fn init_game(Json(config): Json<GameConfig>) -> FnResult<()> {
    let game = Game::new(config.player_ids);
    var::set("game", &game)?;
    Ok(())
}

// debug
#[plugin_fn]
pub fn get_state(_: ()) -> FnResult<Game> {
    let game = var::get("game")?.ok_or(anyhow!("Game not found"))?;
    Ok(game)
}

#[plugin_fn]
pub fn handle_event(Json(event): Json<LiveEvent>) -> FnResult<()> {
    let mut game: Game = var::get("game")?.ok_or(anyhow!("Game not found"))?;
    let handler = from_live_event(&event)?;
    handler.on(event.player_id, &mut game)?;
    var::set("game", &game)?;
    Ok(())
}

#[plugin_fn]
pub fn render(Json(config): Json<RenderConfig>) -> FnResult<String> {
    let game: Game = var::get("game")?.ok_or(anyhow!("Game not found"))?;
    let ctx = Ctx::new(&game, &config)?;
    let html = ctx.render()?;
    Ok(html)
}
