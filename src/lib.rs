use crate::game_view::Ctx;
use anyhow::anyhow;
use card::Card;
use events::{
    answer::Answer, distribute::Distribute, pass::Pass, select::Select, serve::Serve, Event,
    EventHandler,
};
use extism_pdk::*;
use game::Game;

pub mod card;
pub mod deck;

mod events;
mod game;
mod game_view;

#[derive(serde::Deserialize)]
pub struct GameConfig {
    pub player_ids: Vec<String>,
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

#[derive(serde::Deserialize)]
pub struct HandleEventArg {
    pub player_id: String,
    pub event: Event,
}

pub fn into_event_handler(event: &Event) -> anyhow::Result<Option<Box<dyn EventHandler>>> {
    match event {
        Event::Distribute => Ok(Some(Box::new(Distribute))),
        Event::Select { field, card } => Ok(Some(Box::new(Select {
            field: field.clone(),
            card: Card::try_from(card.as_str())?,
        }))),
        Event::Answer { option } => Ok(Some(Box::new(Answer {
            answer: option.clone(),
        }))),
        Event::Serve => Ok(Some(Box::new(Serve))),
        Event::Pass => Ok(Some(Box::new(Pass))),
        _ => Ok(None),
    }
}

#[plugin_fn]
pub fn handle_event(
    Json(HandleEventArg { player_id, event }): Json<HandleEventArg>,
) -> FnResult<Event> {
    let mut game: Game = var::get("game")?.ok_or(anyhow!("Game not found"))?;
    let Some(handler) = into_event_handler(&event)? else {
        return Ok(Event::None);
    };
    let res = handler.on(player_id, &mut game)?;
    var::set("game", &game)?;
    Ok(res)
}

#[derive(serde::Deserialize)]
pub struct RenderConfig {
    pub player_id: String,
}

#[plugin_fn]
pub fn render(Json(config): Json<RenderConfig>) -> FnResult<String> {
    let game: Game = var::get("game")?.ok_or(anyhow!("Game not found"))?;
    let ctx = Ctx::new(&game, config.player_id)?;
    let html = ctx.render()?;
    Ok(html)
}
