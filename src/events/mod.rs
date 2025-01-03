use crate::game::Game;
use anyhow::Result;

pub mod answer;
pub mod distribute;
pub mod pass;
pub mod select;
pub mod serve;

pub trait EventHandler {
    fn on(&self, player_id: String, game: &mut Game) -> Result<Event>;
}

#[derive(serde::Serialize, serde::Deserialize)]
// #[serde(tag = "name", content = "value")]
#[serde(tag = "name")]
pub enum Event {
    Distribute,
    Select {
        field: String,
        card: String,
    },
    Answer {
        option: String,
    },
    Serve,
    Pass,
    // builtin events
    None,
    Exit,
    LaunchPlugin {
        plugin_name: String,
    },
    PluginStarted {
        state_id: String,
    },
    PluginFinished {
        state_id: String,
        value: serde_json::Value,
    },
}
