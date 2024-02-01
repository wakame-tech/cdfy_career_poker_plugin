use crate::{
    card::Card,
    game::{Answer, Distribute, EventHandler, Game, Pass, Select, Serve},
    plugin::{LiveEvent, RenderConfig},
};
use anyhow::{anyhow, Result};
use tera::Tera;

static APP_HTML: &[u8] = include_bytes!("templates/app.html");

pub fn from_event(event: &LiveEvent) -> Result<Box<dyn EventHandler>> {
    if event.event_name == "distribute" {
        return Ok(Box::new(Distribute));
    }
    if event.event_name == "select" {
        return Ok(Box::new(Select {
            field: event.value.get("field").unwrap().clone(),
            card: Card::try_from(event.value.get("card").unwrap().as_str())?,
        }));
    }
    if event.event_name == "answer" {
        return Ok(Box::new(Answer {
            answer: event.value.get("option").unwrap().to_string(),
        }));
    }
    if event.event_name == "serve" {
        return Ok(Box::new(Serve));
    }
    if event.event_name == "pass" {
        return Ok(Box::new(Pass));
    }
    Err(anyhow!("invalid event"))
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
        .last()
        .map(|p| {
            p.player_ids.contains(&config.player_id)
                && !game.answers.contains_key(&config.player_id)
        })
        .unwrap_or(false);
    context.insert("show_prompt", &show_prompt);
    context.insert("prompt", &game.prompt);

    let html = Tera::one_off(std::str::from_utf8(APP_HTML).unwrap(), &context, false)?;
    Ok(html)
}
