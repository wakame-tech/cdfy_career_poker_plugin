use game::Game;

pub mod card;
pub mod deck;
pub mod effect;
// pub mod state;

use crate::{
    game::Action,
    plugin::{GameConfig, LiveEvent},
};
use extism_pdk::*;
use tera::Tera;

mod game;
mod plugin;

static APP_HTML: &[u8] = include_bytes!("templates/app.html");

impl ToBytes<'_> for Game {
    type Bytes = Vec<u8>;

    fn to_bytes(&self) -> Result<Self::Bytes, Error> {
        Ok(serde_json::to_vec(self)?)
    }
}

impl FromBytesOwned for Game {
    fn from_bytes_owned(bytes: &[u8]) -> Result<Self, Error> {
        Ok(serde_json::from_slice(&bytes)?)
    }
}

#[plugin_fn]
pub fn init_game(Json(_): Json<GameConfig>) -> FnResult<()> {
    let game = Game::default();
    var::set("game", &game)?;
    Ok(())
}

// debug
#[plugin_fn]
pub fn get_state(_: ()) -> FnResult<String> {
    Ok(var::get("game")?
        .map(|s: Game| serde_json::to_string(&s).unwrap())
        .unwrap_or("nil".to_string()))
}

#[plugin_fn]
pub fn handle_event(Json(event): Json<LiveEvent>) -> FnResult<()> {
    let mut game: Game = var::get("game")?.unwrap();
    let action = Action::from_event(&event)?;
    game.apply_action(action)?;
    var::set("game", &game)?;
    Ok(())
}

#[plugin_fn]
pub fn render(_: ()) -> FnResult<String> {
    let game: Game = var::get("game")?.unwrap();
    let mut context = tera::Context::new();
    context.insert("game", &game);
    Ok(Tera::one_off(
        std::str::from_utf8(APP_HTML).unwrap(),
        &context,
        false,
    )?)
}
