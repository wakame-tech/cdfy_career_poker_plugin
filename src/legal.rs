//! Legal-action enumeration for the cdfy_next Daifugo plugin.
//!
//! `legal_actions(view, player)` returns the concrete `Action`s the given player
//! may take from the supplied `GameView`. Each `serve`/`select`/`one_chance`
//! action carries the specific card ids in `data` (JSON array of `u64`).
//!
//! The Daifugo rules are NOT reimplemented here: candidate plays are validated
//! by calling [`crate::rules::validate_serve`] (the same predicate the real
//! state transition uses), so legality stays in lockstep with `apply_action`.

use crate::card::{is_same_number, Card};
use crate::convert::{from_view, proto_to_card, EXCLUDED, TRUSHES};
use crate::game::{Game, PromptKind};
use crate::rules::validate_serve;
use crate::wire::{Action, GameView};
use std::collections::BTreeSet;

/// Largest same-number group a serve may contain (quad).
const MAX_GROUP: usize = 4;
/// Safety cap on enumerated `select` subsets, so a large source field cannot
/// blow up the action list.
const MAX_SELECT_SUBSETS: usize = 200;

fn serve_action(ids: &[u64]) -> Action {
    Action {
        kind: "serve".to_string(),
        data: serde_json::to_vec(ids).unwrap(),
    }
}

fn pass_action() -> Action {
    Action {
        kind: "pass".to_string(),
        data: serde_json::to_vec(&Vec::<u64>::new()).unwrap(),
    }
}

fn select_action(ids: &[u64]) -> Action {
    Action {
        kind: "select".to_string(),
        data: serde_json::to_vec(ids).unwrap(),
    }
}

fn one_chance_action(ids: &[u64]) -> Action {
    Action {
        kind: "one_chance".to_string(),
        data: serde_json::to_vec(ids).unwrap(),
    }
}

/// `(id, Card)` pairs for the cards of a zone, in zone order.
fn zone_cards(view: &GameView, zone_id: u32) -> Vec<(u64, Card)> {
    view.zone(zone_id)
        .map(|z| z.cards.iter().map(|c| (c.id, proto_to_card(c.proto))).collect())
        .unwrap_or_default()
}

/// All index combinations of `0..n` with size in `1..=max_k`.
fn combinations_upto(n: usize, max_k: usize) -> Vec<Vec<usize>> {
    let mut out = vec![];
    for k in 1..=max_k.min(n) {
        let mut cur = vec![];
        combine(0, n, k, &mut cur, &mut out);
    }
    out
}

/// All index combinations of `0..n` with exactly size `k`.
fn combinations_exact(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut out = vec![];
    if k == 0 || k > n {
        return out;
    }
    let mut cur = vec![];
    combine(0, n, k, &mut cur, &mut out);
    out
}

fn combine(start: usize, n: usize, k: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
    if cur.len() == k {
        out.push(cur.clone());
        return;
    }
    for i in start..n {
        cur.push(i);
        combine(i + 1, n, k, cur, out);
        cur.pop();
    }
}

/// Enumerate every legal `serve` (each carrying its card ids) plus `pass` when
/// the river is non-empty.
fn serve_actions(game: &Game, hand: &[(u64, Card)]) -> Vec<Action> {
    let mut actions = vec![];
    // Dedupe groups that look identical by card prototype (e.g. 2-deck dupes).
    let mut seen: BTreeSet<Vec<u32>> = BTreeSet::new();

    for combo in combinations_upto(hand.len(), MAX_GROUP) {
        let cards: Vec<Card> = combo.iter().map(|&i| hand[i].1.clone()).collect();
        if !is_same_number(&cards) {
            continue;
        }
        if validate_serve(game, &cards).is_err() {
            continue;
        }
        let mut key: Vec<u32> = cards.iter().map(crate::convert::card_to_proto).collect();
        key.sort_unstable();
        if !seen.insert(key) {
            continue;
        }
        let ids: Vec<u64> = combo.iter().map(|&i| hand[i].0).collect();
        actions.push(serve_action(&ids));
    }

    if !game.river.is_empty() {
        actions.push(pass_action());
    }
    actions
}

/// Enumerate the valid `select` answers for a pending Select4/7/13 prompt: every
/// subset of the source field of the required size (capped).
fn select_actions(view: &GameView, game: &Game, kind: &PromptKind, player_index: u32) -> Vec<Action> {
    let n_cards = game.river.last().map(|g| g.len()).unwrap_or(1);
    let source_zone = match kind {
        PromptKind::Select4 => TRUSHES,
        PromptKind::Select7 => player_index, // own hand zone
        PromptKind::Select13 => EXCLUDED,
        PromptKind::UseOneChance => return vec![],
    };
    let pool = zone_cards(view, source_zone);
    let mut actions = vec![];
    for combo in combinations_exact(pool.len(), n_cards) {
        if actions.len() >= MAX_SELECT_SUBSETS {
            break;
        }
        let ids: Vec<u64> = combo.iter().map(|&i| pool[i].0).collect();
        actions.push(select_action(&ids));
    }
    actions
}

/// Enumerate `one_chance` answers: one offer per held Ace plus a skip (`[]`).
fn one_chance_actions(hand: &[(u64, Card)]) -> Vec<Action> {
    let mut actions = vec![];
    for (id, card) in hand {
        if card.number() == Some(1) {
            actions.push(one_chance_action(&[*id]));
        }
    }
    // skip
    actions.push(one_chance_action(&[]));
    actions
}

/// Concrete legal actions for `player` from `view`.
pub fn legal_actions(view: &GameView, player: u32) -> Vec<Action> {
    let Ok(game) = from_view(view) else {
        return vec![];
    };
    let Some(player_id) = game.players.get(player as usize).cloned() else {
        return vec![];
    };

    // Round over: nothing to enumerate.
    if game.current.is_some() && game.active_player_ids().len() <= 1 {
        return vec![];
    }

    let hand = zone_cards(view, player);

    // Pending prompt takes precedence over the normal serve turn.
    if let Some(prompt) = game.prompt.last() {
        if !prompt.player_ids.contains(&player_id) {
            return vec![];
        }
        return match prompt.kind {
            PromptKind::UseOneChance => {
                if game.answers.contains_key(&player_id) {
                    return vec![];
                }
                one_chance_actions(&hand)
            }
            ref kind => select_actions(view, &game, kind, player),
        };
    }

    // Normal serve turn.
    if view.active_player != Some(player) {
        return vec![];
    }
    serve_actions(&game, &hand)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Card, Suit};
    use crate::convert::{card_to_proto, to_view};
    use crate::deck::Deck;
    use crate::game::{FieldKey, Game};

    fn n(s: Suit, v: u8) -> Card {
        Card::Number(s, v)
    }

    /// Resolve the protos an action's card ids map to, against the source view.
    fn action_protos(view: &GameView, action: &Action) -> Vec<u32> {
        let ids: Vec<u64> = serde_json::from_slice(&action.data).unwrap();
        let mut protos: Vec<u32> = ids
            .iter()
            .map(|id| {
                view.zones
                    .iter()
                    .flat_map(|z| &z.cards)
                    .find(|c| c.id == *id)
                    .unwrap()
                    .proto
            })
            .collect();
        protos.sort_unstable();
        protos
    }

    fn serve_groups(view: &GameView, actions: &[Action]) -> Vec<Vec<u32>> {
        actions
            .iter()
            .filter(|a| a.kind == "serve")
            .map(|a| action_protos(view, a))
            .collect()
    }

    fn two_player_game(p0: Vec<Card>, p1: Vec<Card>) -> Game {
        let mut g = Game::new(vec!["p0".into(), "p1".into()]);
        g.fields.insert(FieldKey::Hands("p0".into()), Deck::new(p0));
        g.fields.insert(FieldKey::Hands("p1".into()), Deck::new(p1));
        g.current = Some("p0".into());
        g
    }

    #[test]
    fn empty_river_all_singles_and_pairs_present() {
        let g = two_player_game(
            vec![n(Suit::Spade, 6), n(Suit::Heart, 6), n(Suit::Diamond, 3)],
            vec![n(Suit::Clover, 9)],
        );
        let view = to_view(&g);
        let actions = legal_actions(&view, 0);
        let groups = serve_groups(&view, &actions);

        // singles
        assert!(groups.contains(&vec![card_to_proto(&n(Suit::Spade, 6))]));
        assert!(groups.contains(&vec![card_to_proto(&n(Suit::Heart, 6))]));
        assert!(groups.contains(&vec![card_to_proto(&n(Suit::Diamond, 3))]));
        // pair of 6s
        let mut pair = vec![
            card_to_proto(&n(Suit::Spade, 6)),
            card_to_proto(&n(Suit::Heart, 6)),
        ];
        pair.sort_unstable();
        assert!(groups.contains(&pair));
        // no pass on empty river
        assert!(!actions.iter().any(|a| a.kind == "pass"));
    }

    #[test]
    fn top_single_only_beating_singles_and_pass() {
        let mut g = two_player_game(
            vec![n(Suit::Diamond, 3), n(Suit::Clover, 13), n(Suit::Spade, 2)],
            vec![n(Suit::Clover, 9)],
        );
        g.river = vec![vec![n(Suit::Spade, 6)]];
        g.river_size = Some(1);
        g.last_served_player_id = Some("p1".into());
        let view = to_view(&g);
        let actions = legal_actions(&view, 0);
        let groups = serve_groups(&view, &actions);

        // 13 and 2 beat 6; 3 does not.
        assert!(groups.contains(&vec![card_to_proto(&n(Suit::Clover, 13))]));
        assert!(groups.contains(&vec![card_to_proto(&n(Suit::Spade, 2))]));
        assert!(!groups.contains(&vec![card_to_proto(&n(Suit::Diamond, 3))]));
        // pass available against a non-empty river
        assert!(actions.iter().any(|a| a.kind == "pass"));
    }

    #[test]
    fn not_my_turn_is_empty() {
        let g = two_player_game(vec![n(Suit::Spade, 6)], vec![n(Suit::Clover, 9)]);
        let view = to_view(&g);
        // active player is p0; p1 has nothing to do.
        assert!(legal_actions(&view, 1).is_empty());
    }
}
