#[cfg(not(target = "wasm32-unknown-unknown"))]
use rand::{thread_rng, Rng};

pub fn rand() -> u32 {
    let mut rng = thread_rng();
    rng.gen()
}

pub fn debug(message: String) {}

/// cancel task by `task_id`
pub fn cancel(_room_id: String, _task_id: String) {}

/// reserve task and execute returns `task_id`
pub fn reserve(_player_id: String, _room_id: String, _action: String, _timeout: u32) -> String {
    "dummy".to_string()
}

pub struct State {
    pub data: String,
}

pub enum ResultState {
    Ok(State),
    Err(String),
}
