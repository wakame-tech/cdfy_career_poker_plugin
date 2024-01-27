use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct GameConfig {
    pub player_ids: Vec<String>,
}

#[derive(Deserialize)]
pub struct CellValue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Deserialize)]
pub struct LiveEvent {
    pub player_id: String,
    pub event_name: String,
    pub value: CellValue,
}

#[derive(Deserialize)]
pub struct RenderConfig {
    pub player_id: String,
}

#[derive(Serialize)]
pub struct GameConstraints {
    min_players: u32,
    max_players: u32,
}
