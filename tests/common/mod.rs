//! Shared test helpers for constructing `Game`s and cards.
//!
//! Linking against the crate from an integration test works because `build.rs`
//! passes `-Wl,-undefined,dynamic_lookup` (host only, test binaries only) so the
//! unresolved extism host imports pulled in by `lib.rs`'s `#[plugin_fn]` exports
//! are left for runtime lookup. The tests never call those exports.
#![allow(dead_code)]

use cdfy_plugin_career_poker::card::{Card, Suit};
use cdfy_plugin_career_poker::deck::Deck;
use cdfy_plugin_career_poker::game::{FieldKey, Game};

/// A numbered spade.
pub fn s(n: u8) -> Card {
    Card::Number(Suit::Spade, n)
}

/// A numbered card of an explicit suit.
pub fn c(suit: Suit, n: u8) -> Card {
    Card::Number(suit, n)
}

/// `Game` with the given players, starting from empty hands/river. The caller
/// installs hands via [`set_hand`] and sets `current`.
pub fn game(players: &[&str]) -> Game {
    Game::new(players.iter().map(|p| p.to_string()).collect())
}

pub fn set_hand(g: &mut Game, pid: &str, cards: Vec<Card>) {
    g.fields
        .insert(FieldKey::Hands(pid.to_string()), Deck::new(cards));
}

pub fn set_field(g: &mut Game, key: FieldKey, cards: Vec<Card>) {
    g.fields.insert(key, Deck::new(cards));
}

pub fn hand(g: &Game, pid: &str) -> Vec<Card> {
    g.field(&FieldKey::Hands(pid.to_string())).unwrap().0.clone()
}

pub fn field_cards(g: &Game, key: &FieldKey) -> Vec<Card> {
    g.field(key).unwrap().0.clone()
}

/// Put `cards` into `selects[pid]` (the staged play the rules functions read).
pub fn select(g: &mut Game, pid: &str, cards: Vec<Card>) {
    g.selects.insert(pid.to_string(), cards);
}
