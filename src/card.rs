use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::HashSet, fmt::Display};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Suit {
    #[serde(rename = "?")]
    UnSuited,
    #[serde(rename = "s")]
    Spade,
    #[serde(rename = "d")]
    Diamond,
    #[serde(rename = "h")]
    Heart,
    #[serde(rename = "c")]
    Clover,
}

impl Suit {
    pub fn suits() -> Vec<Suit> {
        vec![Suit::Spade, Suit::Diamond, Suit::Heart, Suit::Clover]
    }
}

impl Display for Suit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Suit::Spade => write!(f, "s"),
            Suit::Diamond => write!(f, "d"),
            Suit::Heart => write!(f, "h"),
            Suit::Clover => write!(f, "c"),
            Suit::UnSuited => write!(f, "*"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Card {
    Number(Suit, u8),
    Joker(Option<(Suit, u8)>),
}

const CARD_CHARACTERS: &str = "🂠🂡🂢🂣🂤🂥🂦🂧🂨🂩🂪🂫🂭🂮🂱🂲🂳🂴🂵🂶🂷🂸🂹🂺🂻🂽🂾🃁🃂🃃🃄🃅🃆🃇🃈🃉🃊🃋🃍🃎🃑🃒🃓🃔🃕🃖🃗🃘🃙🃚🃛🃝🃞🃟";

impl Card {
    /// if joker, returns unsuited
    pub fn suit(&self) -> Suit {
        match self {
            Card::Number(s, _) => s.clone(),
            Card::Joker(Some((s, _))) => s.clone(),
            Card::Joker(None) => Suit::UnSuited,
        }
    }

    pub fn number(&self) -> Option<u8> {
        match self {
            Card::Number(_, n) => Some(*n),
            Card::Joker(Some((_, n))) => Some(*n),
            Card::Joker(None) => None,
        }
    }

    pub fn char(&self) -> char {
        match self {
            Card::Number(s, n) => {
                let offset: usize = match s {
                    Suit::Spade => 0,
                    Suit::Heart => 1,
                    Suit::Diamond => 2,
                    Suit::Clover => 3,
                    Suit::UnSuited => 0,
                } * 13;
                let index = 1 + offset + (*n - 1) as usize;
                CARD_CHARACTERS.chars().nth(index).unwrap()
            }
            Card::Joker(_) => CARD_CHARACTERS.chars().last().unwrap(),
        }
    }
}

impl TryFrom<&str> for Card {
    type Error = anyhow::Error;

    // A-K,shdc
    fn try_from(e: &str) -> Result<Self> {
        if e == "joker" {
            return Ok(Card::Joker(None));
        }
        let chars = e.chars().collect::<Vec<char>>();
        let n: u8 = match chars[0] {
            'A' => Ok(1),
            'T' => Ok(10),
            'J' => Ok(11),
            'Q' => Ok(12),
            'K' => Ok(13),
            n => n.to_string().parse(),
        }?;
        let s = match chars[1] {
            'h' => Ok(Suit::Heart),
            'd' => Ok(Suit::Diamond),
            'c' => Ok(Suit::Clover),
            's' => Ok(Suit::Spade),
            _ => Err(anyhow!("invalid suit")),
        }?;
        Ok(Card::Number(s, n))
    }
}

impl Display for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn s(n: u8) -> String {
            match n {
                1 => "A".to_string(),
                10 => "T".to_string(),
                11 => "J".to_string(),
                12 => "Q".to_string(),
                13 => "K".to_string(),
                _ => n.to_string(),
            }
        }
        match self {
            Card::Number(suit, number) => write!(f, "{}{}", s(*number), suit),
            Card::Joker(None) => write!(f, "joker"),
            Card::Joker(Some((suit, number))) => write!(f, "joker(as {}{})", s(*number), suit),
        }
    }
}

pub fn is_same_number(cards: &[Card]) -> bool {
    let numbers: HashSet<_> = cards.iter().filter_map(|c| c.number()).collect();
    // if only jokers, len == 0
    numbers.len() <= 1
}

pub fn numbers(cards: &[Card]) -> HashSet<u8> {
    cards
        .iter()
        .filter_map(|c| c.number())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<HashSet<_>>()
}

/// returns cards number, if only jokers, returns 14
pub fn number(cards: &[Card]) -> u8 {
    numbers(cards).into_iter().next().unwrap_or(14)
}

pub fn suits(cards: &[Card]) -> HashSet<Suit> {
    cards
        .iter()
        .map(|c| c.suit())
        .filter(|s| s != &Suit::UnSuited)
        .collect::<HashSet<_>>()
}

/// `other` contains all suits of `self`
pub fn match_suits(lhs: &[Card], rhs: &[Card]) -> bool {
    let (lhs, rhs) = (suits(lhs), suits(rhs));
    rhs.is_superset(&lhs)
}

pub fn cardinal(n: u8) -> i32 {
    ((n + 10) % 13).into()
}

pub fn card_ord(l: &Card, r: &Card) -> Ordering {
    let (ln, rn) = (l.number(), r.number());
    match (ln, rn) {
        (None, None) => Ordering::Less,
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (Some(i), Some(j)) => cardinal(i).cmp(&cardinal(j)),
    }
}
