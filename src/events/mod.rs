use crate::game::Game;
use anyhow::Result;
use extism_pdk::ToBytes;

pub mod answer;
pub mod distribute;
pub mod effect_card;
pub mod pass;
pub mod select;
pub mod serve;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "name", content = "value")]
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
    // buildin events
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

impl ToBytes<'_> for Event {
    type Bytes = Vec<u8>;

    fn to_bytes(&self) -> Result<Self::Bytes, anyhow::Error> {
        Ok(serde_json::to_vec(self)?)
    }
}

pub trait EventHandler {
    fn on(&self, player_id: String, game: &mut Game) -> Result<Event>;
}
