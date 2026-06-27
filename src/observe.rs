//! Per-player view masking for the cdfy_next Daifugo plugin.
//!
//! `observe(view, player)` returns the `GameView` as `player` is allowed to see
//! it: every card in a hand zone owned by a DIFFERENT player is flipped
//! `Face::Down`. Public zones (river / trushes / excluded, `owner = None`) keep
//! their faces, and the meta zone (also `owner = None`, already `Hidden`) is left
//! untouched.

use crate::wire::{Face, GameView};

/// Mask the view for `player`: hide cards owned by other players.
pub fn observe(view: &GameView, player: u32) -> GameView {
    let mut out = view.clone();
    for zone in &mut out.zones {
        match zone.owner {
            Some(owner) if owner != player => {
                for card in &mut zone.cards {
                    card.face = Face::Down;
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Card, Suit};
    use crate::convert::to_view;
    use crate::deck::Deck;
    use crate::game::{FieldKey, Game};

    fn game() -> Game {
        let mut g = Game::new(vec!["p0".into(), "p1".into()]);
        g.fields.insert(
            FieldKey::Hands("p0".into()),
            Deck::new(vec![Card::Number(Suit::Spade, 6)]),
        );
        g.fields.insert(
            FieldKey::Hands("p1".into()),
            Deck::new(vec![Card::Number(Suit::Clover, 9)]),
        );
        g.current = Some("p0".into());
        g
    }

    #[test]
    fn own_hand_up_others_down() {
        let view = to_view(&game());
        let masked = observe(&view, 0);

        // zone 0 is p0's own hand -> Up
        let own = masked.zone(0).unwrap();
        assert!(own.cards.iter().all(|c| c.face == Face::Up));

        // zone 1 is p1's hand -> Down
        let other = masked.zone(1).unwrap();
        assert!(other.cards.iter().all(|c| c.face == Face::Down));
    }

    #[test]
    fn public_zones_unchanged() {
        let mut g = game();
        g.fields.insert(
            FieldKey::Trushes,
            Deck::new(vec![Card::Number(Suit::Diamond, 4)]),
        );
        let view = to_view(&g);
        let masked = observe(&view, 0);

        // trushes (zone 101) stays Up
        let trushes = masked.zone(101).unwrap();
        assert!(trushes.cards.iter().all(|c| c.face == Face::Up));

        // meta (zone 200) is left as-is (Down/Hidden, owner None)
        let meta = masked.zone(200).unwrap();
        assert!(meta.cards.iter().all(|c| c.face == Face::Down));
    }
}
