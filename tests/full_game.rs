//! Task 10 — deterministic games driven through the `rules` functions with an
//! injected counter-based RNG closure.
//!
//! Determinism: the only nondeterminism the engine has is the `rand_u64`-backed
//! source threaded into `Deck::shuffle_with` (the deal) and the one-chance RPS.
//! Here that source is a fixed counter closure, so every run is reproducible.
//!
//! NOTE on player count: the headline "full game to a finished state" uses 2
//! players. With 3+ players the engine panics on the *first* finisher —
//! `Game::on_end_turn` calls `get_relative_player(current)` after the current
//! player has emptied their hand, but that player is no longer in
//! `active_player_ids()`, so the `position(...).unwrap()` blows up. The
//! `four_player_first_finisher_panics` test pins that bug; until it is fixed a
//! multi-finisher game cannot be driven to completion. See the report.

mod common;

use std::panic::{catch_unwind, AssertUnwindSafe};

use common::*;
use cdfy_plugin_career_poker::game::{FieldKey, Game};
use cdfy_plugin_career_poker::rules::{answer_one_chance, deal_new_round, pass, serve};

/// A reproducible `u64` source: `start, start+1, start+2, ...`.
fn counter_rng(start: u64) -> impl FnMut() -> u64 {
    let mut n = start;
    move || {
        let v = n;
        n = n.wrapping_add(1);
        v
    }
}

/// Mirror of the engine's finish-order tracking (`lib::record_finishers`):
/// append any newly-emptied hand to `ranks`, in player order.
fn record_finishers(g: &Game, ranks: &mut Vec<String>) {
    for pid in &g.players {
        let empty = g
            .field(&FieldKey::Hands(pid.clone()))
            .map(|d| d.0.is_empty())
            .unwrap_or(false);
        if empty && !ranks.contains(pid) {
            ranks.push(pid.clone());
        }
    }
}

/// Full finish order = ranked finishers first, then the remaining player(s),
/// mirroring `lib::winners_of`.
fn full_order(g: &Game, ranks: &[String]) -> Vec<String> {
    let mut order = ranks.to_vec();
    for pid in &g.players {
        if !order.contains(pid) {
            order.push(pid.clone());
        }
    }
    order
}

/// A complete two-player game driven only through `serve`/`pass`, ending on the
/// engine's `"end"` signal, with the finish order asserted.
///
/// Script: p0 leads a 6; p1 (only 3,4 in hand) cannot beat it and passes, which
/// returns the lead to p0 and flushes the river; p0 then plays its last card (a
/// 7) and empties, leaving p1 as the sole active player ⇒ game over.
#[test]
fn deterministic_two_player_game_reaches_end() {
    let mut rng = counter_rng(0); // available to the rules; not drawn by serve/pass
    let mut ranks: Vec<String> = vec![];

    let mut g = game(&["p0", "p1"]);
    set_hand(&mut g, "p0", vec![s(6), s(7)]);
    set_hand(&mut g, "p1", vec![s(3), s(4)]);
    g.current = Some("p0".into());

    // T1: p0 leads a single 6.
    select(&mut g, "p0", vec![s(6)]);
    serve(&mut g, "p0").expect("p0 leads 6");
    record_finishers(&g, &mut ranks);
    assert_eq!(g.current, Some("p1".to_string()));

    // T2: p1 cannot beat the 6 → pass. Lead returns to p0 and the river flushes.
    pass(&mut g, "p1").expect("p1 passes");
    record_finishers(&g, &mut ranks);
    assert_eq!(g.current, Some("p0".to_string()));
    assert!(g.river.is_empty(), "river flushed after the pass");

    // T3: p0 plays its last card (7) and empties → only p1 remains active.
    select(&mut g, "p0", vec![s(7)]);
    let end = serve(&mut g, "p0");
    let err = end.expect_err("emptying the last active-but-one hand ends the game");
    assert_eq!(err.to_string(), "end");
    record_finishers(&g, &mut ranks);

    // p0 finished first (大富豪); p1 is the leftover (大貧民).
    assert_eq!(g.active_player_ids(), vec!["p1".to_string()]);
    assert_eq!(full_order(&g, &ranks), vec!["p0".to_string(), "p1".to_string()]);

    let _ = &mut rng;
}

/// The injected RNG decides the one-chance RPS, and the same RNG sequence always
/// produces the same outcome. p0 serves a 3 (三途, which would lock all effects);
/// p1 declares one-chance. When the declarer "wins" the RPS the 3's effect runs
/// (effect_limits filled); otherwise it is cancelled (effect_limits empty).
#[test]
fn one_chance_outcome_is_rng_determined() {
    // 3 active players ⇒ declarer wins iff rng() % 3 == 0.
    fn run(rng_start: u64) -> std::collections::HashSet<u8> {
        let mut g = game(&["p0", "p1", "p2"]);
        set_hand(&mut g, "p0", vec![s(3), s(6)]);
        set_hand(&mut g, "p1", vec![s(1), s(7)]); // holds an Ace
        set_hand(&mut g, "p2", vec![s(8), s(9)]);
        g.current = Some("p0".into());

        select(&mut g, "p0", vec![s(3)]);
        serve(&mut g, "p0").expect("p0 serves a 3, raising one-chance");

        let mut rng = counter_rng(rng_start);
        select(&mut g, "p1", vec![s(1)]); // p1 declares with its Ace
        answer_one_chance(&mut g, "p1", true, &mut rng).expect("resolve one-chance");
        g.effect_limits.clone()
    }

    // rng()=0 → 0 % 3 == 0 → declarer wins → the 3's effect runs.
    assert_eq!(run(0), (1..=13).collect::<std::collections::HashSet<u8>>());
    // rng()=1 → 1 % 3 != 0 → one-chance cancels the effect.
    assert!(run(1).is_empty());
}

/// A fixed RNG sequence deals identical hands every time (replayability).
#[test]
fn fixed_rng_yields_reproducible_deal() {
    fn deal() -> Game {
        let mut g = Game::new(vec!["p0".into(), "p1".into()]);
        let mut rng = counter_rng(12345);
        deal_new_round(&mut g, &mut rng).unwrap();
        g
    }
    let a = deal();
    let b = deal();
    for pid in ["p0", "p1"] {
        assert_eq!(hand(&a, pid), hand(&b, pid), "hand for {pid} is reproducible");
    }
    let total: usize = a.players.iter().map(|p| hand(&a, p).len()).sum();
    assert_eq!(total, 54, "the full 54-card deck is dealt");
}

/// KNOWN BUG (report): with 3+ players the first finisher panics. p0 plays its
/// only card while p1/p2/p3 still hold cards; `on_end_turn` then asks for the
/// player relative to p0, but p0 is no longer active, so
/// `get_relative_player`'s `unwrap` panics. This blocks driving any 3+ player
/// game to completion.
#[test]
fn four_player_first_finisher_panics() {
    let mut g = game(&["p0", "p1", "p2", "p3"]);
    set_hand(&mut g, "p0", vec![s(6)]);
    set_hand(&mut g, "p1", vec![s(7)]);
    set_hand(&mut g, "p2", vec![s(8)]);
    set_hand(&mut g, "p3", vec![s(13), s(2)]);
    g.current = Some("p0".into());
    select(&mut g, "p0", vec![s(6)]);

    let result = catch_unwind(AssertUnwindSafe(|| serve(&mut g, "p0")));
    assert!(
        result.is_err(),
        "the first finisher with >1 active player panics in on_end_turn \
         (get_relative_player unwrap) — see report"
    );
}
