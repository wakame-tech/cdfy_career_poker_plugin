//! Daifugo rules as plain functions on `Game`.
//!
//! Ported verbatim from the original `events/*` `EventHandler::on` bodies; the
//! trait indirection and `Event` return are stripped. The one-chance resolution
//! gains an `rng`-gated rock-paper-scissors (a deliberate simplification of the
//! real RPS — see the plan).

use crate::{
    card::{card_ord, cardinal, is_same_number, match_suits, number, suits, Card},
    deck::deck_ord,
    game::{FieldKey, Game, Prompt, PromptKind},
};
use anyhow::{anyhow, Result};
use std::collections::HashSet;

/// Validate that `serves` is a legal play against the current river (ported
/// from `ValidateServe`). Includes the joker-top ordering fix.
pub fn validate_serve(game: &Game, serves: &[Card]) -> Result<()> {
    if !is_same_number(serves) {
        return Err(anyhow!("not same number"));
    }
    let Some(top) = game.river.last() else {
        // river is empty
        return Ok(());
    };
    // check ordering.
    // Joker(None) is always the strongest card regardless of revolution.
    let top_is_joker = top.iter().all(|c| c.number().is_none());
    let ordering = if !top_is_joker && (game.revoluted ^ game.turn_revoluted) {
        deck_ord(serves, top).reverse()
    } else {
        deck_ord(serves, top)
    };
    if ordering.is_lt() {
        return Err(anyhow!("must be greater than top card"));
    }
    // check river size
    let river_size = game.river_size.unwrap();
    let expected_river_size = match number(serves) {
        9 if !game.effect_limits.contains(&9) => match river_size {
            1 => 3,
            3 => 1,
            n => n,
        },
        _ => serves.len(),
    };
    if river_size != expected_river_size {
        return Err(anyhow!(
            "expected river size {} but {}",
            expected_river_size,
            river_size
        ));
    }
    // check steps
    if game.is_step && cardinal(number(serves)) - cardinal(number(top)) != 1 {
        return Err(anyhow!("must be step"));
    }
    // check suits
    if !game.suit_limits.is_empty() && !match_suits(top, serves) {
        return Err(anyhow!(
            "expected suits {:?} but {:?}",
            game.suit_limits,
            suits(serves)
        ));
    }
    Ok(())
}

/// Apply the served cards' on-play effect (ported from `EffectCard`).
pub fn effect(game: &mut Game, player_id: &str, serves: &[Card]) -> Result<()> {
    game.river_size = Some(serves.len());

    if serves.len() == 4 {
        game.revoluted = !game.revoluted;
    }

    game.river_size = Some(serves.len());

    let n = number(serves);
    if game.effect_limits.contains(&n) {
        return Ok(());
    }

    let hands = game.field(&FieldKey::Hands(player_id.to_string()))?;
    match n {
        3 => game.effect_limits.extend(1..=13),
        4 => {
            let trushes = game.field(&FieldKey::Trushes)?;
            if hands.0.is_empty() || trushes.0.is_empty() {
                return Ok(());
            }
            game.prompt.push(Prompt {
                kind: PromptKind::Select4,
                player_ids: vec![player_id.to_string()],
                question: "select cards from trushes".to_string(),
                options: vec!["ok".to_string()],
            });
        }
        5 => {}
        6 => {}
        7 => {
            if hands.0.is_empty() {
                return Ok(());
            }
            game.prompt.push(Prompt {
                kind: PromptKind::Select7,
                player_ids: vec![player_id.to_string()],
                question: "select cards from hands".to_string(),
                options: vec!["ok".to_string()],
            });
        }
        8 => {}
        9 => {
            game.river_size = match game.river_size {
                Some(1) => Some(3),
                Some(3) => Some(1),
                n => n,
            };
        }
        10 => {
            game.effect_limits.extend(1..10);
        }
        11 => {
            game.turn_revoluted = true;
        }
        12 => {
            game.is_step = true;
            game.suit_limits = suits(serves);
        }
        13 => {
            let excluded = game.field(&FieldKey::Excluded)?;
            if hands.0.is_empty() || excluded.0.is_empty() {
                return Ok(());
            }
            game.prompt.push(Prompt {
                kind: PromptKind::Select13,
                player_ids: vec![player_id.to_string()],
                question: "select cards from excluded".to_string(),
                options: vec!["ok".to_string()],
            });
        }
        1 => {}
        2 => {}
        _ => {
            return Err(anyhow!("invalid number {}", n));
        }
    };
    Ok(())
}

/// Serve the cards currently in `game.selects[player_id]` (ported from `Serve`).
/// The caller (`apply_action`) populates `selects` from the action payload.
pub fn serve(game: &mut Game, player_id: &str) -> Result<()> {
    if let Some(prompt) = game.prompt.first() {
        if prompt.player_ids.contains(&player_id.to_string())
            && !game.answers.contains_key(player_id)
        {
            return Err(anyhow!("please answer"));
        }
    }

    let serves = game.selects.get(player_id).unwrap().clone();
    // reset select
    game.selects.insert(player_id.to_string(), vec![]);

    if game.current.as_deref() != Some(player_id) {
        return Err(anyhow!("not your turn"));
    }
    if serves.is_empty() {
        return Err(anyhow!("please select cards"));
    }
    validate_serve(game, &serves)?;

    game.field_mut(&FieldKey::Hands(player_id.to_string()))?
        .remove(&serves)?;
    game.river.push(serves.clone());

    let has_1_player_ids = game
        .active_player_ids()
        .iter()
        .filter(|id| {
            id.as_str() != player_id
                && game
                    .field(&FieldKey::Hands(id.to_string()))
                    .unwrap()
                    .0
                    .iter()
                    .any(|c| c.number() == Some(1))
        })
        .cloned()
        .collect::<Vec<_>>();

    if !game.effect_limits.contains(&1) && !has_1_player_ids.is_empty() {
        game.prompt.push(Prompt {
            kind: PromptKind::UseOneChance,
            player_ids: has_1_player_ids,
            question: "select A if use one chance".to_string(),
            options: vec!["serve".to_string(), "skip".to_string()],
        });
    }
    // end phase
    if game.prompt.is_empty() {
        let player_id = game.current.clone().unwrap();
        effect(game, &player_id, &serves)?;
        game.last_served_player_id = Some(player_id.to_string());
        game.on_end_turn()?;
    }
    Ok(())
}

/// Pass the turn (ported from `Pass`).
pub fn pass(game: &mut Game, player_id: &str) -> Result<()> {
    if let Some(prompt) = game.prompt.first() {
        if prompt.player_ids.contains(&player_id.to_string())
            && !game.answers.contains_key(player_id)
        {
            return Err(anyhow!("please answer"));
        }
    }

    if game.current.as_deref() != Some(player_id) {
        return Err(anyhow!("not your turn"));
    }
    if game.river.is_empty() {
        return Err(anyhow!("cannot pass because river is empty"));
    }
    game.on_end_turn()?;
    Ok(())
}

/// Answer a pending Select4/7/13 prompt with the cards in
/// `game.selects[player_id]` (ported from `Answer` + the per-kind validate/apply
/// handlers).
pub fn answer_select(game: &mut Game, player_id: &str) -> Result<()> {
    let Some(prompt) = game.prompt.last().cloned() else {
        return Err(anyhow!("no prompt"));
    };
    let n_cards = game.river.last().unwrap().len();
    let selected = game.selects.get(player_id).unwrap().len();
    match prompt.kind {
        PromptKind::Select4 => {
            if selected != n_cards {
                return Err(anyhow!("please select {} cards in trushes", n_cards));
            }
        }
        PromptKind::Select7 => {
            if selected != n_cards {
                return Err(anyhow!("please select {} cards in hands", n_cards));
            }
        }
        PromptKind::Select13 => {
            if selected != n_cards {
                return Err(anyhow!("please select {} cards in excluded", n_cards));
            }
        }
        PromptKind::UseOneChance => return Err(anyhow!("wrong prompt kind for select")),
    }
    game.answers.insert(player_id.to_string(), "ok".to_string());

    let all_answered = prompt.player_ids.iter().collect::<HashSet<_>>()
        == game.answers.keys().collect::<HashSet<_>>();
    if all_answered {
        game.answers.clear();
        let cards = game.selects.get(player_id).unwrap().clone();
        match prompt.kind {
            PromptKind::Select4 => {
                game.transfer(
                    &FieldKey::Trushes,
                    &FieldKey::Hands(player_id.to_string()),
                    cards,
                )?;
                game.field_mut(&FieldKey::Hands(player_id.to_string()))?
                    .sort(card_ord);
            }
            PromptKind::Select7 => {
                let passer = game.get_relative_player(player_id, -1);
                game.transfer(
                    &FieldKey::Hands(player_id.to_string()),
                    &FieldKey::Hands(passer.clone()),
                    cards,
                )?;
                game.field_mut(&FieldKey::Hands(passer))?.sort(card_ord);
            }
            PromptKind::Select13 => {
                game.transfer(
                    &FieldKey::Excluded,
                    &FieldKey::Hands(player_id.to_string()),
                    cards,
                )?;
                game.field_mut(&FieldKey::Hands(player_id.to_string()))?
                    .sort(card_ord);
            }
            PromptKind::UseOneChance => unreachable!(),
        }
        // resolve the prompt
        game.prompt.pop();
        // reset select
        game.selects.insert(player_id.to_string(), vec![]);

        game.last_served_player_id = Some(player_id.to_string());
        game.on_end_turn()?;
    }
    Ok(())
}

/// Resolve a pending `UseOneChance` prompt for `player_id`.
///
/// `use_it` = whether the player plays their Ace (their Ace must already be in
/// `selects[player_id]`). When all prompted players have answered, an
/// `rng`-gated RPS decides the outcome: the declarer (server) wins with
/// probability `1/active_players`; on a declarer win the served card's effect
/// runs, otherwise the one-chance cancels it. With nobody using one-chance the
/// served effect runs as normal.
pub fn answer_one_chance(
    game: &mut Game,
    player_id: &str,
    use_it: bool,
    rng: &mut impl FnMut() -> u64,
) -> Result<()> {
    let Some(prompt) = game.prompt.last().cloned() else {
        return Err(anyhow!("no prompt"));
    };
    if prompt.kind != PromptKind::UseOneChance {
        return Err(anyhow!("no one-chance prompt"));
    }
    if !prompt.player_ids.contains(&player_id.to_string()) {
        return Err(anyhow!("not prompted for one-chance"));
    }
    if use_it {
        let serves = game.selects.get(player_id).unwrap().clone();
        if serves.len() != 1 || number(&serves) != 1 {
            return Err(anyhow!("please select A"));
        }
    }
    let answer = if use_it { "serve" } else { "skip" };
    game.answers.insert(player_id.to_string(), answer.to_string());

    let all_answered = prompt.player_ids.iter().collect::<HashSet<_>>()
        == game.answers.keys().collect::<HashSet<_>>();
    if all_answered {
        let any_used = game.answers.values().any(|a| a == "serve");
        game.answers.clear();
        // reset prompted players' selects
        for pid in &prompt.player_ids {
            game.selects.insert(pid.clone(), vec![]);
        }
        game.prompt.pop();

        let server = game.current.clone().unwrap();
        let serves = game
            .river
            .last()
            .cloned()
            .expect("river is empty on UseOneChance");

        let run_effect = if any_used {
            // RPS: declarer wins w.p. 1/active_players -> effect runs.
            let n = game.active_player_ids().len().max(1) as u64;
            (rng() % n) == 0
        } else {
            true
        };
        if run_effect {
            effect(game, &server, &serves)?;
        }

        game.last_served_player_id = Some(server);
        game.on_end_turn()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::Suit;
    use crate::deck::Deck;

    fn game_with_hand(p0_hand: Vec<Card>, p1_hand: Vec<Card>) -> Game {
        let mut g = Game::new(vec!["p0".into(), "p1".into()]);
        g.fields.insert(FieldKey::Hands("p0".into()), Deck::new(p0_hand));
        g.fields.insert(FieldKey::Hands("p1".into()), Deck::new(p1_hand));
        g.current = Some("p0".into());
        g
    }

    fn set_serves(g: &mut Game, who: &str, cards: Vec<Card>) {
        g.selects.insert(who.into(), cards);
    }

    #[test]
    fn serve_pair_on_empty_river_ok() {
        let mut g = game_with_hand(
            vec![
                Card::Number(Suit::Spade, 6),
                Card::Number(Suit::Heart, 6),
                Card::Number(Suit::Diamond, 3),
            ],
            vec![Card::Number(Suit::Clover, 9)],
        );
        set_serves(&mut g, "p0", vec![
            Card::Number(Suit::Spade, 6),
            Card::Number(Suit::Heart, 6),
        ]);
        assert!(serve(&mut g, "p0").is_ok());
        assert_eq!(g.river, vec![vec![Card::Number(Suit::Spade, 6), Card::Number(Suit::Heart, 6)]]);
    }

    #[test]
    fn serve_not_same_number_rejected() {
        let mut g = game_with_hand(
            vec![Card::Number(Suit::Spade, 6), Card::Number(Suit::Heart, 7)],
            vec![Card::Number(Suit::Clover, 9)],
        );
        set_serves(&mut g, "p0", vec![
            Card::Number(Suit::Spade, 6),
            Card::Number(Suit::Heart, 7),
        ]);
        assert!(serve(&mut g, "p0").is_err());
    }

    #[test]
    fn serve_must_beat_top() {
        let mut g = game_with_hand(
            vec![Card::Number(Suit::Spade, 3)],
            vec![Card::Number(Suit::Clover, 9)],
        );
        g.river = vec![vec![Card::Number(Suit::Spade, 6)]];
        g.river_size = Some(1);
        set_serves(&mut g, "p0", vec![Card::Number(Suit::Spade, 3)]);
        assert!(serve(&mut g, "p0").is_err());
    }

    #[test]
    fn serve_beats_top_ok() {
        let mut g = game_with_hand(
            vec![Card::Number(Suit::Clover, 13), Card::Number(Suit::Spade, 2)],
            vec![Card::Number(Suit::Clover, 9)],
        );
        g.river = vec![vec![Card::Number(Suit::Spade, 6)]];
        g.river_size = Some(1);
        set_serves(&mut g, "p0", vec![Card::Number(Suit::Clover, 13)]);
        assert!(serve(&mut g, "p0").is_ok());
    }

    #[test]
    fn serve_wrong_river_size_rejected() {
        let mut g = game_with_hand(
            vec![Card::Number(Suit::Spade, 8), Card::Number(Suit::Heart, 8)],
            vec![Card::Number(Suit::Clover, 9)],
        );
        // river top is a single; a pair cannot follow
        g.river = vec![vec![Card::Number(Suit::Spade, 6)]];
        g.river_size = Some(1);
        set_serves(&mut g, "p0", vec![
            Card::Number(Suit::Spade, 8),
            Card::Number(Suit::Heart, 8),
        ]);
        assert!(serve(&mut g, "p0").is_err());
    }

    #[test]
    fn serve_step_requires_consecutive() {
        let mut g = game_with_hand(
            vec![Card::Number(Suit::Spade, 10)],
            vec![Card::Number(Suit::Clover, 9)],
        );
        g.river = vec![vec![Card::Number(Suit::Spade, 6)]];
        g.river_size = Some(1);
        g.is_step = true;
        set_serves(&mut g, "p0", vec![Card::Number(Suit::Spade, 10)]);
        // 10 does not directly follow 6
        assert!(serve(&mut g, "p0").is_err());
    }

    #[test]
    fn serve_suit_limit_enforced() {
        let mut g = game_with_hand(
            vec![Card::Number(Suit::Heart, 9)],
            vec![Card::Number(Suit::Clover, 9)],
        );
        g.river = vec![vec![Card::Number(Suit::Spade, 6)]];
        g.river_size = Some(1);
        g.suit_limits = HashSet::from([Suit::Spade]);
        set_serves(&mut g, "p0", vec![Card::Number(Suit::Heart, 9)]);
        // heart does not match the required spade suit
        assert!(serve(&mut g, "p0").is_err());
    }

    #[test]
    fn pass_on_empty_river_rejected() {
        let mut g = game_with_hand(
            vec![Card::Number(Suit::Spade, 6)],
            vec![Card::Number(Suit::Clover, 9)],
        );
        assert!(pass(&mut g, "p0").is_err());
    }

    #[test]
    fn pass_on_nonempty_river_ok() {
        let mut g = game_with_hand(
            vec![Card::Number(Suit::Spade, 6)],
            vec![Card::Number(Suit::Clover, 9)],
        );
        g.river = vec![vec![Card::Number(Suit::Heart, 7)]];
        g.river_size = Some(1);
        g.last_served_player_id = Some("p1".into());
        assert!(pass(&mut g, "p0").is_ok());
    }

    #[test]
    fn pass_not_your_turn_rejected() {
        let mut g = game_with_hand(
            vec![Card::Number(Suit::Spade, 6)],
            vec![Card::Number(Suit::Clover, 9)],
        );
        g.river = vec![vec![Card::Number(Suit::Heart, 7)]];
        g.river_size = Some(1);
        assert!(pass(&mut g, "p1").is_err());
    }
}
