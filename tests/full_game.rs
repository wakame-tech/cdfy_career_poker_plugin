//! Task 10 — deterministic games driven through the `rules` functions with an
//! injected counter-based RNG closure.
//!
//! Determinism: the only nondeterminism the engine has is the `rand_u64`-backed
//! source threaded into `Deck::shuffle_with` (the deal) and the one-chance RPS.
//! Here that source is a fixed counter closure, so every run is reproducible.
//!
//! NOTE on player count: both a 2-player and a 4-player game are driven all the
//! way to the engine's `"end"` signal, asserting the full finish order. The
//! 4-player case exercises multiple finishers in sequence — `on_end_turn`
//! advances the lead past a just-finished player in seating order and flushes
//! the river for the inheriting leader.

mod common;

use common::*;
use cdfy_plugin_career_poker::card::Suit;
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

/// A complete four-player game driven to the engine's `"end"` signal, with
/// multiple finishers in sequence.
///
/// Each finisher empties on a fresh river by playing a lone 6 (the only
/// effect-free card). Because the just-finished player was the last server, the
/// lead is inherited by the next active player in seating order and the river is
/// flushed, so the next player leads on an empty river and empties in turn.
/// p3 keeps a spare 2 so it never empties and is the leftover 大貧民.
///
/// Finish order (role order, 大富豪 → 大貧民): p0, p1, p2, p3.
#[test]
fn deterministic_four_player_game_reaches_end() {
    let mut ranks: Vec<String> = vec![];

    let mut g = game(&["p0", "p1", "p2", "p3"]);
    set_hand(&mut g, "p0", vec![s(6)]);
    set_hand(&mut g, "p1", vec![c(Suit::Diamond, 6)]);
    set_hand(&mut g, "p2", vec![c(Suit::Heart, 6)]);
    set_hand(&mut g, "p3", vec![c(Suit::Clover, 6), s(2)]);
    g.current = Some("p0".into());

    // T1: p0 leads its only 6 and empties (大富豪). Lead passes to p1, river flushes.
    select(&mut g, "p0", vec![s(6)]);
    serve(&mut g, "p0").expect("p0 leads 6 and finishes");
    record_finishers(&g, &mut ranks);
    assert_eq!(g.current, Some("p1".to_string()));
    assert!(g.river.is_empty(), "river flushed for the inheriting leader p1");
    assert_eq!(g.active_player_ids(), vec!["p1", "p2", "p3"]);

    // T2: p1 leads its only 6 and empties (富豪). Lead passes to p2, river flushes.
    select(&mut g, "p1", vec![c(Suit::Diamond, 6)]);
    serve(&mut g, "p1").expect("p1 leads 6 and finishes");
    record_finishers(&g, &mut ranks);
    assert_eq!(g.current, Some("p2".to_string()));
    assert!(g.river.is_empty(), "river flushed for the inheriting leader p2");
    assert_eq!(g.active_player_ids(), vec!["p2", "p3"]);

    // T3: p2 leads its only 6 and empties → only p3 remains active ⇒ game over.
    select(&mut g, "p2", vec![c(Suit::Heart, 6)]);
    let end = serve(&mut g, "p2");
    let err = end.expect_err("emptying the second-to-last active hand ends the game");
    assert_eq!(err.to_string(), "end");
    record_finishers(&g, &mut ranks);

    // p3 is the sole leftover (大貧民).
    assert_eq!(g.active_player_ids(), vec!["p3".to_string()]);
    // The engine records the finish order in `ranks` as players empty.
    assert_eq!(g.ranks, vec!["p0", "p1", "p2"]);
    // Full role order, 大富豪 … 大貧民.
    assert_eq!(
        full_order(&g, &ranks),
        vec!["p0".to_string(), "p1".to_string(), "p2".to_string(), "p3".to_string()]
    );
    assert_eq!(g.ranks, ranks, "engine ranks match the mirrored record_finishers");
}
