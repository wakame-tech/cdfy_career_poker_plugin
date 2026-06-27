use crate::card::{card_ord, Card, Suit};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::HashSet};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Deck(pub Vec<Card>);

impl From<Vec<Card>> for Deck {
    fn from(cards: Vec<Card>) -> Self {
        Self(cards)
    }
}

impl Deck {
    /// Fisher-Yates shuffle driven by an injected `u64` source. Must match
    /// cdfy_next `RngState::shuffle`: i from n-1 downto 1, j = r % (i+1), swap.
    /// The source is the host `rand_u64` draw in production (see `rng.rs`).
    pub fn shuffle_with(&mut self, rng: &mut impl FnMut() -> u64) {
        let n = self.0.len();
        if n < 2 {
            return;
        }
        for i in (1..n).rev() {
            let r = rng();
            let j = (r % (i as u64 + 1)) as usize;
            self.0.swap(i, j);
        }
    }

    pub fn sort<F>(&mut self, ord: F)
    where
        F: Fn(&Card, &Card) -> Ordering,
    {
        self.0.sort_by(ord);
    }

    pub fn all(jokers: usize) -> Self {
        let mut cards = vec![];
        for suit in Suit::suits().iter() {
            for number in 1u8..=13 {
                cards.push(Card::Number(suit.clone(), number))
            }
        }
        for _ in 0..jokers {
            cards.push(Card::Joker(None))
        }
        Self(cards)
    }

    pub fn new(cards: Vec<Card>) -> Self {
        Self(cards)
    }

    pub fn split(&self, n: usize) -> Result<Vec<Self>> {
        let mut decks = vec![];
        for _ in 0..n {
            decks.push(Deck::new(vec![]));
        }
        for (i, card) in self.0.iter().enumerate() {
            decks[i % n].0.push(card.clone());
        }
        Ok(decks)
    }

    pub fn remove(&mut self, cards: &[Card]) -> Result<()> {
        remove_items(&mut self.0, cards)
    }
}

fn remove_items<T: Eq + Clone>(items: &mut Vec<T>, removes: &[T]) -> Result<()> {
    if removes.iter().any(|i| !items.contains(i)) {
        return Err(anyhow!("remove items not in items"));
    }
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
    Ok(())
}

fn vec_ord<T, F>(l: impl Iterator<Item = T>, r: impl Iterator<Item = T>, ord: F) -> Ordering
where
    F: Fn(T, T) -> Ordering,
{
    let orderings = l.zip(r).map(|(a, b)| ord(a, b)).collect::<HashSet<_>>();
    orderings.into_iter().next().unwrap_or(Ordering::Equal)
}

pub fn deck_ord(lhs: &[Card], rhs: &[Card]) -> Ordering {
    let (mut lhs, mut rhs) = (lhs.to_vec(), rhs.to_vec());
    lhs.sort_by(card_ord);
    rhs.sort_by(card_ord);
    vec_ord(lhs.iter(), rhs.iter(), card_ord)
}
