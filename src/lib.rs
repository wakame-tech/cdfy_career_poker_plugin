#[cfg(target_arch = "wasm32")]
use anyhow::{anyhow, Result};
#[cfg(target_arch = "wasm32")]
use cdfy_sdk::*;
#[cfg(not(target_arch = "wasm32"))]
use mock::*;
use state::{Action, CareerPokerState};
use std::fmt::Debug;

pub mod card;
pub mod deck;
pub mod effect;
#[cfg(not(target_arch = "wasm32"))]
pub mod mock;
pub mod state;

pub fn will_flush(player_id: String, room_id: String, to: String) -> String {
    reserve(
        player_id,
        room_id,
        serde_json::to_string(&Action::Flush { to }).unwrap(),
        5000,
    )
}

pub fn cancel_task(room_id: String, task_id: String) {
    cancel(room_id, task_id);
}

fn from_err<E: Debug>(s: CareerPokerState, r: anyhow::Result<(), E>) -> ResultState {
    match r {
        anyhow::Result::Ok(_) => ResultState::Ok(State {
            data: serde_json::to_string(&s).unwrap(),
        }),
        anyhow::Result::Err(err) => ResultState::Err(format!("{:?}", err)),
    }
}

#[cfg(target_arch = "wasm32")]
#[fp_export_impl(cdfy_sdk)]
pub fn plugin_meta() -> PluginMeta {
    PluginMeta {
        name: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

#[cfg(target_arch = "wasm32")]
#[fp_export_impl(cdfy_sdk)]
pub fn on_create_room(player_id: String, room_id: String) -> ResultState {
    let mut state = CareerPokerState::new(room_id);
    state.players.push(player_id);
    from_err::<()>(state, Ok(()))
}

#[cfg(target_arch = "wasm32")]
#[fp_export_impl(cdfy_sdk)]
pub fn on_join_player(player_id: String, _room_id: String, state: State) -> ResultState {
    let state: Result<CareerPokerState> =
        serde_json::from_str(&state.data).map_err(|e| anyhow!("{}", e));
    let Ok(mut state) = state else {
        return ResultState::Err(state.unwrap_err().to_string());
    };
    state.join(player_id);
    from_err::<()>(state, Ok(()))
}

#[cfg(target_arch = "wasm32")]
#[fp_export_impl(cdfy_sdk)]
pub fn on_leave_player(player_id: String, _room_id: String, state: State) -> ResultState {
    let state: Result<CareerPokerState> =
        serde_json::from_str(&state.data).map_err(|e| anyhow!("{}", e));
    let Ok(mut state) = state else {
        return ResultState::Err(state.unwrap_err().to_string());
    };
    let mut state: CareerPokerState = state.into();
    state.leave(player_id);
    from_err::<()>(state, Ok(()))
}

#[cfg(target_arch = "wasm32")]
#[fp_export_impl(cdfy_sdk)]
pub fn on_task(_task_id: String, state: State) -> ResultState {
    let state: Result<CareerPokerState> =
        serde_json::from_str(&state.data).map_err(|e| anyhow!("{}", e));
    let Ok(mut state) = state else {
        return ResultState::Err(state.unwrap_err().to_string());
    };
    state.will_flush_task_id = None;
    from_err::<()>(state, Ok(()))
}

#[cfg(target_arch = "wasm32")]
#[fp_export_impl(cdfy_sdk)]
pub fn on_cancel_task(_task_id: String, state: State) -> ResultState {
    let state: Result<CareerPokerState> =
        serde_json::from_str(&state.data).map_err(|e| anyhow!("{}", e));
    let Ok(mut state) = state else {
        return ResultState::Err(state.unwrap_err().to_string());
    };
    state.will_flush_task_id = None;
    from_err::<()>(state, Ok(()))
}

#[cfg(target_arch = "wasm32")]
#[fp_export_impl(cdfy_sdk)]
pub fn rpc(_player_id: String, _room_id: String, state: State, value: String) -> ResultState {
    let state: Result<CareerPokerState> =
        serde_json::from_str(&state.data).map_err(|e| anyhow!("{}", e));
    let Ok(mut state) = state else {
        return ResultState::Err(state.unwrap_err().to_string());
    };
    let action: Result<Action> = serde_json::from_str(value.as_str()).map_err(|e| anyhow!("{}", e));
    let Ok(action) = action else {
        return ResultState::Err(action.unwrap_err().to_string());
    };
    let res = state.action(action);
    from_err(state, res)
}
