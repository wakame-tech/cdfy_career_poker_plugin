use super::EventHandler;
use crate::{
    card::card_ord,
    deck::Deck,
    game::{FieldKey, Game},
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Distribute;

impl EventHandler for Distribute {
    fn on(&self, _player_id: String, game: &mut Game) -> Result<()> {
        if game.players.is_empty() {
            return Err(anyhow!("players is empty"));
        }
        let mut deck = Deck::all(2);
        deck.shuffle();
        let mut decks = deck.split(game.players.len())?;
        for (i, player_id) in game.players.iter().enumerate() {
            decks[i].sort(card_ord);
            game.fields
                .insert(FieldKey::Hands(player_id.to_string()), decks[i].clone());
        }
        game.current = Some(game.players[0].clone());
        Ok(())
    }
}
