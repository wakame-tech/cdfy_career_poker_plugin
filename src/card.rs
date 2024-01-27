use serde::{Deserialize, Serialize};
use std::fmt::Display;

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

const CARD_CHARACTERS: &'static str = "🂠🂡🂢🂣🂤🂥🂦🂧🂨🂩🂪🂫🂭🂮🂱🂲🂳🂴🂵🂶🂷🂸🂹🂺🂻🂽🂾🃁🃂🃃🃄🃅🃆🃇🃈🃉🃊🃋🃍🃎🃑🃒🃓🃔🃕🃖🃗🃘🃙🃚🃛🃝🃞🃟";

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

impl From<&str> for Card {
    // A-K,shdc
    fn from(e: &str) -> Self {
        if e == "joker" {
            return Card::Joker(None);
        }
        let chars = e.chars().collect::<Vec<char>>();
        let n: u8 = match chars[0] {
            'A' => 1,
            'T' => 10,
            'J' => 11,
            'Q' => 12,
            'K' => 13,
            n => n.to_string().parse().unwrap(),
        };
        let s = match chars[1] {
            'h' => Suit::Heart,
            'd' => Suit::Diamond,
            'c' => Suit::Clover,
            's' => Suit::Spade,
            _ => panic!(),
        };
        Card::Number(s, n)
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
