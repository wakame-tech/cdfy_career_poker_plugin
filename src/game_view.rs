use crate::{
    card::Card,
    events::{
        answer::Answer, distribute::Distribute, pass::Pass, select::Select, serve::Serve,
        EventHandler,
    },
    game::{FieldKey, Game},
    plugin::{LiveEvent, RenderConfig},
};
use anyhow::{anyhow, Result};
use tera::Tera;

static APP_HTML: &[u8] = include_bytes!("templates/app.html");

pub fn from_event(event: &LiveEvent) -> Result<Box<dyn EventHandler>> {
    match event {
        LiveEvent { event_name, .. } if event_name == "distribute" => Ok(Box::new(Distribute)),
        LiveEvent {
            event_name, value, ..
        } if event_name == "select" => Ok(Box::new(Select {
            field: value.get("field").unwrap().clone(),
            card: Card::try_from(event.value.get("card").unwrap().as_str())?,
        })),
        LiveEvent {
            event_name, value, ..
        } if event_name == "answer" => Ok(Box::new(Answer {
            answer: value.get("option").unwrap().to_string(),
        })),
        LiveEvent { event_name, .. } if event_name == "serve" => Ok(Box::new(Serve)),
        LiveEvent { event_name, .. } if event_name == "pass" => Ok(Box::new(Pass)),
        _ => Err(anyhow!("invalid event")),
    }
}

pub fn render_game(game: &Game, config: &RenderConfig) -> Result<String> {
    let mut context = tera::Context::new();

    let is_current = game.current == Some(config.player_id.clone());
    context.insert("is_current", &is_current);

    context.insert("current", &game.current);

    let trushes = game
        .fields
        .get(&FieldKey::Trushes)
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
        .get(&FieldKey::Excluded)
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

    let hands = game.fields[&FieldKey::Hands(config.player_id.to_string())]
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
