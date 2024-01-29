use crate::{
    card::Card,
    game::{Action, Game},
    plugin::{LiveEvent, RenderConfig},
};
use anyhow::{anyhow, Result};
use tera::Tera;

static APP_HTML: &[u8] = include_bytes!("templates/app.html");

impl Action {
    pub fn from_event(event: &LiveEvent) -> Result<Self> {
        if event.event_name == "reset" {
            return Ok(Action::Reset);
        }
        if event.event_name == "distribute" {
            return Ok(Action::Distribute);
        }
        if event.event_name == "select" {
            return Ok(Action::Select {
                field: event.value.get("field").unwrap().clone(),
                player_id: event.player_id.clone(),
                card: Card::try_from(event.value.get("card").unwrap().as_str())?,
            });
        }
        if event.event_name == "answer" {
            return Ok(Action::Answer {
                player_id: event.player_id.clone(),
                answer: event.value.get("option").unwrap().to_string(),
            });
        }
        if event.event_name == "serve" {
            return Ok(Action::Serve {
                player_id: event.player_id.clone(),
            });
        }
        if event.event_name == "pass" {
            return Ok(Action::Pass {
                player_id: event.player_id.clone(),
            });
        }
        Err(anyhow!("invalid event"))
    }
}

pub fn render_game(game: &Game, config: &RenderConfig) -> Result<String> {
    let mut context = tera::Context::new();

    let is_current = game.current == Some(config.player_id.clone());
    context.insert("is_current", &is_current);

    context.insert("current", &game.current);

    let trushes = game
        .fields
        .get("trushes")
        .unwrap()
        .0
        .iter()
        .map(|c| {
            (
                c.char().to_string(),
                c.to_string(),
                game.selects[&config.player_id].contains(c),
            )
        })
        .collect::<Vec<_>>();
    context.insert("trushes", &trushes);

    let excluded = game
        .fields
        .get("excluded")
        .unwrap()
        .0
        .iter()
        .map(|c| {
            (
                c.char().to_string(),
                c.to_string(),
                game.selects[&config.player_id].contains(c),
            )
        })
        .collect::<Vec<_>>();
    context.insert("excluded", &excluded);

    let river = game
        .river
        .last()
        .cloned()
        .unwrap_or(vec![])
        .iter()
        .map(|c| (c.char().to_string(), c.to_string()))
        .collect::<Vec<_>>();
    context.insert("river", &river);

    let hands = game.fields[&config.player_id]
        .0
        .iter()
        .map(|c| {
            (
                c.char().to_string(),
                c.to_string(),
                game.selects[&config.player_id].contains(c),
            )
        })
        .collect::<Vec<_>>();
    context.insert("hands", &hands);

    let show_prompt = game
        .prompt
        .as_ref()
        .map(|(p, m)| {
            p.player_ids.contains(&config.player_id) && !m.contains_key(&config.player_id)
        })
        .unwrap_or(false);
    context.insert("show_prompt", &show_prompt);
    context.insert("prompt", &game.prompt);

    let html = Tera::one_off(std::str::from_utf8(APP_HTML).unwrap(), &context, false)?;
    Ok(html)
}
