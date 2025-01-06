use crate::game_view::Ctx;
use anyhow::anyhow;
use event::{dispatch_event, Event};
use extism_pdk::*;
use game::Game;
use serde::{Deserialize, Serialize};

mod card;
mod cards_effect;
mod deck;
mod event;
mod game;
mod game_view;

#[derive(Serialize, Deserialize, ToBytes, FromBytes)]
#[encoding(Json)]
struct GameAndEvent {
    game: Game,
    event: Event,
}

impl From<(Game, Event)> for GameAndEvent {
    fn from((game, event): (Game, Event)) -> Self {
        Self { game, event }
    }
}

#[derive(serde::Deserialize)]
struct GameConfig {
    player_ids: Vec<String>,
}

#[plugin_fn]
pub fn init_game(Json(config): Json<GameConfig>) -> FnResult<Game> {
    let game = Game::new(config.player_ids);
    Ok(game)
}

// debug
#[plugin_fn]
pub fn get_state(_: ()) -> FnResult<Game> {
    let game = var::get("game")?.ok_or(anyhow!("Game not found"))?;
    Ok(game)
}

#[derive(serde::Deserialize)]
struct HandleEventArg {
    game: Game,
    player_id: String,
    event: Event,
}

#[plugin_fn]
pub fn handle_event(
    Json(HandleEventArg {
        game,
        player_id,
        event,
    }): Json<HandleEventArg>,
) -> FnResult<GameAndEvent> {
    if game.current != Some(player_id) {
        return Err(anyhow!("not your turn").into());
    }
    let (game, event) = dispatch_event(game, event)?;
    Ok((game, event).into())
}

#[derive(serde::Deserialize)]
struct RenderConfig {
    game: Game,
    player_id: String,
}

#[plugin_fn]
pub fn render(Json(RenderConfig { game, player_id }): Json<RenderConfig>) -> FnResult<String> {
    let ctx = Ctx::new(&game, player_id)?;
    let html = ctx.render()?;
    Ok(html)
}
