//! Career Poker (all-role Daifugo) as a cdfy_next plugin.
//!
//! The cdfy_next ABI (`setup`/`apply_action`/`legal_actions`/`observe`/
//! `status`) is added in a later task. This module currently exposes the
//! internal game model + wire types + host RNG bridge.

pub mod card;
pub mod convert;
pub mod deck;
pub mod game;
pub mod rng;
pub mod rules;
pub mod wire;
