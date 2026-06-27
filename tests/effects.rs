//! Task 9 — one test per card effect.
//!
//! Each test constructs a `Game`, stages a hand/river, applies the play through
//! the public `rules`/`game` API, and asserts the resulting state/flags.
//!
//! Flag-setting effects (3, 4, 7, 9, 10, 11, 12, 13, revolution) are exercised
//! through `rules::effect` (the on-play effect dispatch). Turn-flow effects that
//! live in `Game::on_end_turn` (5 skip, 8 clear, 2 exclude) are exercised
//! through `on_end_turn`/`pass`. The one-chance prompt is raised inside
//! `rules::serve`, so that effect is driven through `serve`.

mod common;

use common::*;
use std::collections::HashSet;

use cdfy_plugin_career_poker::card::Suit;
use cdfy_plugin_career_poker::game::{FieldKey, PromptKind};
use cdfy_plugin_career_poker::rules::{effect, serve};

/// 3 — 三途の川: locks every effect for the current river (effect_limits 1..=13).
#[test]
fn three_locks_all_effects() {
    let mut g = game(&["p0", "p1"]);
    set_hand(&mut g, "p0", vec![s(3)]);
    effect(&mut g, "p0", &[s(3)]).unwrap();
    assert_eq!(g.effect_limits, (1..=13).collect::<HashSet<u8>>());
}

/// 4 — 死者蘇生: prompts the server to take cards from the trushes (Select4).
#[test]
fn four_raises_select4_prompt() {
    let mut g = game(&["p0", "p1"]);
    set_hand(&mut g, "p0", vec![s(4), s(6)]);
    set_field(&mut g, FieldKey::Trushes, vec![s(7)]);
    effect(&mut g, "p0", &[s(4)]).unwrap();
    assert_eq!(g.prompt.last().map(|p| p.kind.clone()), Some(PromptKind::Select4));
    assert_eq!(g.prompt.last().unwrap().player_ids, vec!["p0".to_string()]);
}

/// 4 — no trushes to revive from ⇒ no prompt.
#[test]
fn four_without_trushes_no_prompt() {
    let mut g = game(&["p0", "p1"]);
    set_hand(&mut g, "p0", vec![s(4), s(6)]);
    effect(&mut g, "p0", &[s(4)]).unwrap();
    assert!(g.prompt.is_empty());
}

/// 5 — スキップ: the next `cards.len()+1` players are skipped. A single 5 with
/// 4 active players advances the turn by 2 (skips one player).
#[test]
fn five_skips_players() {
    let mut g = game(&["p0", "p1", "p2", "p3"]);
    for p in ["p0", "p1", "p2", "p3"] {
        set_hand(&mut g, p, vec![s(6)]);
    }
    g.current = Some("p0".into());
    g.river = vec![vec![s(5)]];
    g.river_size = Some(1);
    g.last_served_player_id = Some("p0".into()); // not the next player ⇒ no flush
    g.on_end_turn().unwrap();
    assert_eq!(g.current, Some("p2".to_string())); // p1 skipped
    assert!(!g.river.is_empty());
}

/// 7 — 7渡し: prompts the server to hand cards to the previous player (Select7).
#[test]
fn seven_raises_select7_prompt() {
    let mut g = game(&["p0", "p1"]);
    set_hand(&mut g, "p0", vec![s(7), s(6)]);
    effect(&mut g, "p0", &[s(7)]).unwrap();
    assert_eq!(g.prompt.last().map(|p| p.kind.clone()), Some(PromptKind::Select7));
}

/// 8 — 8切り: the server clears the river and leads again (skips=0 ⇒ flush).
#[test]
fn eight_clears_river_and_restarts() {
    let mut g = game(&["p0", "p1"]);
    set_hand(&mut g, "p0", vec![s(6)]); // non-empty so the round does not "end"
    set_hand(&mut g, "p1", vec![s(9)]);
    g.current = Some("p0".into());
    g.river = vec![vec![s(8)]];
    g.river_size = Some(1);
    g.last_served_player_id = Some("p0".into());
    g.on_end_turn().unwrap();
    assert!(g.river.is_empty(), "river is cleared");
    assert_eq!(g.current, Some("p0".to_string()), "server leads again");
    assert_eq!(field_cards(&g, &FieldKey::Trushes), vec![s(8)], "8 goes to trushes");
    assert_eq!(g.river_size, None);
}

/// 9 — 阿修羅: toggles the required river size 1↔3.
#[test]
fn nine_single_makes_river_size_three() {
    let mut g = game(&["p0", "p1"]);
    set_hand(&mut g, "p0", vec![s(9)]);
    effect(&mut g, "p0", &[s(9)]).unwrap();
    assert_eq!(g.river_size, Some(3));
}

#[test]
fn nine_triple_makes_river_size_one() {
    let mut g = game(&["p0", "p1"]);
    effect(
        &mut g,
        "p0",
        &[c(Suit::Spade, 9), c(Suit::Heart, 9), c(Suit::Diamond, 9)],
    )
    .unwrap();
    assert_eq!(g.river_size, Some(1));
}

/// 10 — 十戒: locks effects 1..10 for the current river.
#[test]
fn ten_locks_effects_one_to_nine() {
    let mut g = game(&["p0", "p1"]);
    set_hand(&mut g, "p0", vec![s(10)]);
    effect(&mut g, "p0", &[s(10)]).unwrap();
    assert_eq!(g.effect_limits, (1..10).collect::<HashSet<u8>>());
}

/// 11 — J バック: reverses card strength until the river is flushed.
#[test]
fn eleven_sets_turn_revoluted() {
    let mut g = game(&["p0", "p1"]);
    set_hand(&mut g, "p0", vec![s(11)]);
    assert!(!g.turn_revoluted);
    effect(&mut g, "p0", &[s(11)]).unwrap();
    assert!(g.turn_revoluted);
}

/// 12 — 摩訶鉢特摩: enables step mode and pins the river to the served suits.
#[test]
fn twelve_sets_step_and_suit_limits() {
    let mut g = game(&["p0", "p1"]);
    set_hand(&mut g, "p0", vec![s(12)]);
    effect(&mut g, "p0", &[s(12)]).unwrap();
    assert!(g.is_step);
    assert_eq!(g.suit_limits, HashSet::from([Suit::Spade]));
}

/// 13 — ロイヤルレリーフ: prompts the server to take cards from the excluded
/// pile (Select13).
#[test]
fn thirteen_raises_select13_prompt() {
    let mut g = game(&["p0", "p1"]);
    set_hand(&mut g, "p0", vec![s(13), s(6)]);
    set_field(&mut g, FieldKey::Excluded, vec![s(7)]);
    effect(&mut g, "p0", &[s(13)]).unwrap();
    assert_eq!(g.prompt.last().map(|p| p.kind.clone()), Some(PromptKind::Select13));
}

/// 2 — 除外: when a 2 is the top card at flush, the river goes to the excluded
/// pile instead of the trushes. Here p0 has served the 2 and p1 passes, so the
/// turn returns to p0 (the last server) and the river is flushed.
#[test]
fn two_flushes_river_to_excluded() {
    let mut g = game(&["p0", "p1"]);
    set_hand(&mut g, "p0", vec![s(6)]);
    set_hand(&mut g, "p1", vec![s(9)]);
    g.current = Some("p1".into());
    g.river = vec![vec![s(2)]];
    g.river_size = Some(1);
    g.last_served_player_id = Some("p0".into());
    cdfy_plugin_career_poker::rules::pass(&mut g, "p1").unwrap();
    assert_eq!(field_cards(&g, &FieldKey::Excluded), vec![s(2)]);
    assert!(g.river.is_empty());
    // and not in the trushes
    assert!(field_cards(&g, &FieldKey::Trushes).is_empty());
}

/// 1 — ワンチャンス: serving while another active player holds an Ace raises the
/// UseOneChance prompt for that player.
#[test]
fn one_raises_one_chance_prompt() {
    let mut g = game(&["p0", "p1"]);
    set_hand(&mut g, "p0", vec![s(6), s(3)]);
    set_hand(&mut g, "p1", vec![s(1)]); // holds an Ace
    g.current = Some("p0".into());
    select(&mut g, "p0", vec![s(6)]);
    serve(&mut g, "p0").unwrap();
    let prompt = g.prompt.last().expect("one-chance prompt raised");
    assert_eq!(prompt.kind, PromptKind::UseOneChance);
    assert!(prompt.player_ids.contains(&"p1".to_string()));
}

/// Revolution: playing four same-number cards toggles `revoluted`.
#[test]
fn four_cards_toggle_revolution() {
    let mut g = game(&["p0", "p1"]);
    set_hand(&mut g, "p0", vec![s(6)]);
    assert!(!g.revoluted);
    effect(
        &mut g,
        "p0",
        &[
            c(Suit::Spade, 6),
            c(Suit::Heart, 6),
            c(Suit::Diamond, 6),
            c(Suit::Clover, 6),
        ],
    )
    .unwrap();
    assert!(g.revoluted, "four of a kind triggers revolution");
}
