use crate::card::{Card, Suit};
#[cfg(not(target_arch = "wasm32"))]
use crate::mock::*;
#[cfg(target_arch = "wasm32")]
use cdfy_sdk::rand;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fmt::{Display, Formatter},
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum DeckStyle {
    Arrange,
    Stack,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Deck {
    pub style: DeckStyle,
    pub cards: Vec<Card>,
}

impl Display for Deck {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.cards.is_empty() {
            write!(f, "(empty)")
        } else {
            write!(
                f,
                "[{}]",
                self.cards
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    }
}

pub fn with_jokers(jokers: usize) -> Vec<Card> {
    let mut cards = vec![];
    for suit in Suit::suits().iter() {
        for number in 1u8..=13 {
            cards.push(Card::Number(suit.clone(), number))
        }
    }
    for _ in 0..jokers {
        cards.push(Card::Joker(None))
    }
    shuffle(&mut cards);
    cards
}

pub fn shuffle<T>(items: &mut Vec<T>) {
    let l = items.len();
    for i in 0..l {
        items.swap(i, (rand() % l as u32) as usize);
    }
}

pub fn is_same_number(cards: &Vec<Card>) -> bool {
    let numbers: HashSet<_> = cards.iter().filter_map(|c| c.number()).collect();
    // if only jokers, len == 0
    numbers.len() <= 1
}

pub fn numbers(cards: &Vec<Card>) -> HashSet<u8> {
    cards
        .iter()
        .filter_map(|c| c.number())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<HashSet<_>>()
}

/// returns cards number, if only jokers, returns 14
pub fn number(cards: &Vec<Card>) -> u8 {
    numbers(cards).into_iter().next().unwrap_or(14)
}

pub fn suits(cards: &Vec<Card>) -> HashSet<Suit> {
    cards
        .iter()
        .map(|c| c.suit())
        .filter(|s| s != &Suit::UnSuited)
        .collect::<HashSet<_>>()
}

/// `other` contains all suits of `self`
pub fn match_suits(lhs: &Vec<Card>, rhs: &Vec<Card>) -> bool {
    let (lhs, rhs) = (suits(lhs), suits(rhs));
    rhs.is_superset(&lhs)
}

pub fn remove_items<T: Eq + Clone>(items: &mut Vec<T>, removes: &Vec<T>) {
    let indices = removes
        .iter()
        .map(|c| items.iter().position(|h| h == c))
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default();
    *items = items
        .iter()
        .enumerate()
        .filter(|(i, _)| !indices.contains(i))
        .map(|(_, c)| c.clone())
        .collect();
}

impl Deck {
    pub fn new(cards: Vec<Card>) -> Self {
        Self {
            style: DeckStyle::Arrange,
            cards,
        }
    }
}
