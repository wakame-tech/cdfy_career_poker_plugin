use crate::{game::Game, plugin::RenderConfig};
use anyhow::Result;
use tera::Tera;

static APP_HTML: &[u8] = include_bytes!("templates/app.html");

pub fn render_game(game: &Game, config: &RenderConfig) -> Result<String> {
    let mut context = tera::Context::new();
    let cards = &game.fields[&config.player_id].0;
    let hands = cards
        .iter()
        .map(|c| (c.char().to_string(), c.to_string()))
        .collect::<Vec<_>>();
    context.insert("hands", &hands);
    let html = Tera::one_off(std::str::from_utf8(APP_HTML).unwrap(), &context, false)?;
    Ok(html)
}
