use crate::game::Game;
use crate::{
    card::Card,
    events::{answer::Answer, distribute::Distribute, pass::Pass, select::Select, serve::Serve},
    plugin::LiveEvent,
};
use anyhow::{anyhow, Result};

pub mod answer;
pub mod distribute;
pub mod effect_card;
pub mod pass;
pub mod select;
pub mod serve;

pub trait EventHandler {
    fn on(&self, player_id: String, game: &mut Game) -> Result<()>;
}

pub fn from_live_event(event: &LiveEvent) -> Result<Box<dyn EventHandler>> {
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
