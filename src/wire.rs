//! cdfy_next plugin wire types.
//!
//! These mirror cdfy_next's `core::wire` JSON shapes exactly (see
//! `cdfy_next/docs/wire-contract.md`). There is no shared crate: both sides
//! conform to the JSON described there. The unit test below is the drift guard
//! that pins the exact serialized bytes.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A plugin-defined attribute value. Externally tagged so it serializes as
/// `{"Int": 3}`, `{"Str": "x"}`, `{"Bool": true}`, `{"List": [..]}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Int(i64),
    Str(String),
    Bool(bool),
    List(Vec<Value>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Face {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZoneKind {
    Stack,
    Set,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Owner,
    Hidden,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Card {
    pub id: u64,
    pub proto: u32,
    pub attrs: BTreeMap<String, Value>,
    pub face: Face,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    pub id: u32,
    pub owner: Option<u32>,
    pub kind: ZoneKind,
    pub visibility: Visibility,
    pub cards: Vec<Card>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub id: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GameView {
    pub players: Vec<Player>,
    pub zones: Vec<Zone>,
    pub counters: BTreeMap<String, i64>,
    pub phase: String,
    pub turn: u32,
    pub active_player: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub kind: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Status {
    Running,
    Ended { winners: Vec<u32> },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub data: Vec<u8>,
}

impl GameView {
    pub fn zone(&self, id: u32) -> Option<&Zone> {
        self.zones.iter().find(|z| z.id == id)
    }
    pub fn zone_mut(&mut self, id: u32) -> Option<&mut Zone> {
        self.zones.iter_mut().find(|z| z.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the exact JSON bytes against the contract: bare-number ids,
    /// `face` `"Up"`/`"Down"`, `null` owner, external-tagged `Value`.
    #[test]
    fn gameview_json_shape() {
        let mut attrs = BTreeMap::new();
        attrs.insert("power".to_string(), Value::Int(3));
        let view = GameView {
            players: vec![Player { id: 0 }],
            zones: vec![Zone {
                id: 100,
                owner: None,
                kind: ZoneKind::Stack,
                visibility: Visibility::Public,
                cards: vec![Card {
                    id: 10,
                    proto: 5,
                    attrs,
                    face: Face::Up,
                }],
            }],
            counters: BTreeMap::new(),
            phase: "serve".to_string(),
            turn: 1,
            active_player: Some(0),
        };
        let json = serde_json::to_string(&view).unwrap();
        assert_eq!(
            json,
            r#"{"players":[{"id":0}],"zones":[{"id":100,"owner":null,"kind":"Stack","visibility":"Public","cards":[{"id":10,"proto":5,"attrs":{"power":{"Int":3}},"face":"Up"}]}],"counters":{},"phase":"serve","turn":1,"active_player":0}"#
        );
    }

    /// `Status` is external-tagged: `"Running"` and `{"Ended":{"winners":[0]}}`.
    #[test]
    fn status_json_shape() {
        assert_eq!(serde_json::to_string(&Status::Running).unwrap(), r#""Running""#);
        assert_eq!(
            serde_json::to_string(&Status::Ended { winners: vec![0] }).unwrap(),
            r#"{"Ended":{"winners":[0]}}"#
        );
    }

    /// `Action` round-trips with a JSON byte array in `data`.
    #[test]
    fn action_json_shape() {
        let a = Action {
            kind: "serve".to_string(),
            data: vec![91, 93],
        };
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            r#"{"kind":"serve","data":[91,93]}"#
        );
    }

    /// Face `Down` serializes as the bare string.
    #[test]
    fn face_json_shape() {
        assert_eq!(serde_json::to_string(&Face::Down).unwrap(), r#""Down""#);
    }
}
