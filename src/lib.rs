//! Career Poker (all-role Daifugo) as a cdfy_next plugin.
//!
//! Exports the cdfy_next ABI (`setup`/`apply_action`/`legal_actions`/`observe`/
//! `status`) over the generic `GameView` JSON, with randomness drawn from the
//! `rand_u64` host function. State transitions reuse the ported rules in
//! `rules.rs`; the `GameView` <-> `Game` mapping lives in `convert.rs`.
//!
//! Redaction is **not** this plugin's job. `Zone.visibility` says who may see
//! each zone and the cdfy_next core masks the rest after `observe` returns, so
//! `observe` here is the identity function. See `docs/wire-contract.md`.

pub mod card;
pub mod convert;
pub mod deck;
pub mod game;
pub mod legal;
pub mod rng;
pub mod rules;
pub mod wire;

use crate::card::{card_ord, Card};
use crate::convert::{from_view, proto_to_card, to_view};
use crate::deck::Deck;
use crate::game::{FieldKey, Game, PromptKind};
use crate::wire::{Action, Config, GameView, Status};
use anyhow::{anyhow, Result};
use extism_pdk::*;
use serde::Deserialize;

/// Optional setup payload carried in `Config.data` (JSON). Defaults to 2 players.
#[derive(Deserialize)]
struct SetupConfig {
    #[serde(default = "default_players")]
    players: usize,
}
fn default_players() -> usize {
    2
}

fn parse_setup(cfg: &Config) -> SetupConfig {
    if cfg.data.is_empty() {
        return SetupConfig {
            players: default_players(),
        };
    }
    serde_json::from_slice(&cfg.data).unwrap_or(SetupConfig {
        players: default_players(),
    })
}

/// Resolve action card ids to internal cards by looking them up in the view
/// (the `Game` model carries no ids, so resolution must use the wire view).
fn resolve_cards(view: &GameView, ids: &[u64]) -> Result<Vec<Card>> {
    ids.iter()
        .map(|id| {
            view.zones
                .iter()
                .flat_map(|z| &z.cards)
                .find(|c| c.id == *id)
                .map(|c| proto_to_card(c.proto))
                .ok_or_else(|| anyhow!("card id {} not found", id))
        })
        .collect()
}

/// Finish order = ranked (finished) players first, then any remaining.
fn winners_of(game: &Game) -> Vec<u32> {
    let mut order: Vec<String> = game.ranks.clone();
    for pid in &game.players {
        if !order.contains(pid) {
            order.push(pid.clone());
        }
    }
    order
        .iter()
        .filter_map(|id| game.players.iter().position(|p| p == id))
        .map(|i| i as u32)
        .collect()
}

fn is_ended(game: &Game) -> bool {
    game.current.is_some() && game.active_player_ids().len() <= 1
}

#[plugin_fn]
pub fn setup(cfg: Json<Config>) -> FnResult<Json<GameView>> {
    let conf = parse_setup(&cfg.into_inner());
    let n = conf.players.max(1);
    let player_ids: Vec<String> = (0..n).map(|i| format!("p{i}")).collect();
    let mut game = Game::new(player_ids.clone());

    // Deal: shuffle the full deck (2 jokers) via the host RNG, split, sort.
    let mut deck = Deck::all(2);
    deck.shuffle_with(&mut rng::next_u64);
    let decks = deck
        .split(n)
        .map_err(|e| WithReturnCode::new(e, 1))?;
    for (i, pid) in player_ids.iter().enumerate() {
        let mut d = decks[i].clone();
        d.sort(card_ord);
        game.fields.insert(FieldKey::Hands(pid.clone()), d);
    }
    game.current = Some(player_ids[0].clone());

    Ok(Json(to_view(&game)))
}

#[plugin_fn]
pub fn apply_action(input: Json<(GameView, u32, Action)>) -> FnResult<Json<GameView>> {
    // `_player` is the acting seat (cdfy_next attribution contract: input is now
    // [view, player, action]). Career poker attributes via the game's internal
    // turn-holder (`game.current`), which the server's seat/turn gate guarantees
    // equals the actor, so the param is accepted but not yet used.
    let (view, _player, action) = input.into_inner();
    let next_turn = view.turn.wrapping_add(1);
    let mut game = from_view(&view).map_err(|e| WithReturnCode::new(e, 1))?;

    let res = dispatch(&mut game, &view, &action);
    match res {
        Ok(()) => {}
        Err(e) if e.to_string() == "end" => {
            // game-over signal from on_end_turn: append the finishing player to
            // the finish order, then settle. The mutated `game` is final.
            record_finishers(&mut game);
        }
        Err(e) => return Err(WithReturnCode::new(e, 1)),
    }

    let mut out = to_view(&game);
    out.turn = next_turn;
    Ok(Json(out))
}

/// Append any newly-emptied hands to the finish order (in player index order).
fn record_finishers(game: &mut Game) {
    for pid in game.players.clone() {
        let empty = game
            .field(&FieldKey::Hands(pid.clone()))
            .map(|d| d.0.is_empty())
            .unwrap_or(false);
        if empty && !game.ranks.contains(&pid) {
            game.ranks.push(pid);
        }
    }
}

fn dispatch(game: &mut Game, view: &GameView, action: &Action) -> Result<()> {
    let mut rng = rng::next_u64;
    match action.kind.as_str() {
        "serve" => {
            let current = game
                .current
                .clone()
                .ok_or_else(|| anyhow!("no current player"))?;
            let ids: Vec<u64> = decode_ids(&action.data)?;
            let cards = resolve_cards(view, &ids)?;
            game.selects.insert(current.clone(), cards);
            let r = rules::serve(game, &current);
            record_finishers(game);
            r
        }
        "pass" => {
            let current = game
                .current
                .clone()
                .ok_or_else(|| anyhow!("no current player"))?;
            let r = rules::pass(game, &current);
            record_finishers(game);
            r
        }
        "select" => {
            let prompt = game
                .prompt
                .last()
                .cloned()
                .ok_or_else(|| anyhow!("no pending prompt"))?;
            let player = prompt
                .player_ids
                .first()
                .cloned()
                .ok_or_else(|| anyhow!("prompt has no player"))?;
            let ids: Vec<u64> = decode_ids(&action.data)?;
            let cards = resolve_cards(view, &ids)?;
            game.selects.insert(player.clone(), cards);
            let r = rules::answer_select(game, &player);
            record_finishers(game);
            r
        }
        "one_chance" => {
            let prompt = game
                .prompt
                .last()
                .cloned()
                .ok_or_else(|| anyhow!("no pending prompt"))?;
            if prompt.kind != PromptKind::UseOneChance {
                return Err(anyhow!("no one-chance prompt"));
            }
            // first prompted player who has not answered yet
            let player = prompt
                .player_ids
                .iter()
                .find(|id| !game.answers.contains_key(*id))
                .cloned()
                .ok_or_else(|| anyhow!("one-chance already answered"))?;
            let ids: Vec<u64> = decode_ids(&action.data)?;
            let use_it = !ids.is_empty();
            if use_it {
                let cards = resolve_cards(view, &ids)?;
                game.selects.insert(player.clone(), cards);
            }
            let r = rules::answer_one_chance(game, &player, use_it, &mut rng);
            record_finishers(game);
            r
        }
        "next_round" => {
            if !is_ended(game) {
                return Err(anyhow!("round not ended"));
            }
            rules::deal_new_round(game, &mut rng)
        }
        other => Err(anyhow!("unknown action kind {}", other)),
    }
}

fn decode_ids(data: &[u8]) -> Result<Vec<u64>> {
    if data.is_empty() {
        return Ok(vec![]);
    }
    Ok(serde_json::from_slice(data)?)
}

#[plugin_fn]
pub fn legal_actions(input: Json<(GameView, u32)>) -> FnResult<Json<Vec<Action>>> {
    let (view, player) = input.into_inner();
    Ok(Json(crate::legal::legal_actions(&view, player)))
}

/// Identity. Daifugo has no per-seat masking beyond what `Zone.visibility`
/// already states: hands are `Owner`, the river and the discard piles are
/// `Public`, and the engine's bookkeeping is `Hidden`. The core applies that
/// after this returns.
///
/// This used to flip `face` to `Down` on other seats' hands while leaving
/// `proto` intact, which leaked every hand to every client.
#[plugin_fn]
pub fn observe(input: Json<(GameView, u32)>) -> FnResult<Json<GameView>> {
    let (view, _player) = input.into_inner();
    Ok(Json(view))
}

#[plugin_fn]
pub fn status(view: Json<GameView>) -> FnResult<Json<Status>> {
    let view = view.into_inner();
    let game = from_view(&view).map_err(|e| WithReturnCode::new(e, 1))?;
    let s = if is_ended(&game) {
        Status::Ended {
            winners: winners_of(&game),
        }
    } else {
        Status::Running
    };
    Ok(Json(s))
}
