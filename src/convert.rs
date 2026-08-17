//! `GameView` <-> `Game` conversion.
//!
//! The cdfy_next state is a generic `GameView`. We encode the Daifugo `Game`
//! into it and back. Card-bearing fields become zones; every non-card engine
//! flag rides on a reserved "meta" card's `attrs`.
//!
//! Invariant (tested): `from_view(&to_view(&g)).unwrap() == g` for any reachable
//! `g`. Card `id`s are assigned deterministically in `to_view` and ignored by
//! `from_view` (the `Game` model has no id slots), so they do not affect the
//! round-trip.
//!
//! ## Zone ids (parallel tasks must match)
//! - `0..N`  hand of player N (`owner = Some(N)`, `Owner`, `Stack`)
//! - `100`   river, flattened in serve order (`Public`, `Stack`)
//! - `101`   trushes / graveyard (`Public`, `Stack`)
//! - `102`   excluded (`Public`, `Stack`)
//! - `200`   meta: exactly one card, proto `9999`, `Hidden`; carries all flags.
//!
//! ## Card proto
//! - `Number(suit, n)` -> `suit_index*100 + n` (Spade=1, Diamond=2, Heart=3,
//!   Clover=4).
//! - `Joker(None)` -> `0`.
//! - `Joker(Some((suit, n)))` -> `9000 + suit_index*100 + n`.
//!
//! ## Meta attrs keys
//! Public (`META`, the UI reads these): `players: List[Str]`,
//! `river_size: Int(-1=None)`, `revoluted/turn_revoluted/is_step: Bool`,
//! `suit_limits: List[Str]`, `effect_limits: List[Int]`,
//! `last_served: Int(-1=None)`, `river_groups: List[Int]`, `ranks: List[Int]`.
//!
//! Private (`META_PRIVATE`): `prompt/selects/answers: Str` (JSON).

use crate::card::{Card, Suit};
use crate::deck::Deck;
use crate::game::{FieldKey, Game, Prompt};
use crate::wire::{Card as WCard, Face, GameView, Player, Value, Visibility, Zone, ZoneKind};
use anyhow::{anyhow, Result};
use std::collections::{BTreeMap, HashMap, HashSet};

pub const RIVER: u32 = 100;
pub const TRUSHES: u32 = 101;
pub const EXCLUDED: u32 = 102;
pub const META: u32 = 200;
/// Bookkeeping only the engine may see. Split out of `META` because the core
/// masks a zone as a whole: `selects` and `answers` are a player's in-progress
/// choices and must not reach the other seats, while the flags in `META` are
/// public and the UI reads them.
pub const META_PRIVATE: u32 = 201;
pub const META_PROTO: u32 = 9999;

// --- card <-> proto --------------------------------------------------------

fn suit_index(s: &Suit) -> u32 {
    match s {
        Suit::Spade => 1,
        Suit::Diamond => 2,
        Suit::Heart => 3,
        Suit::Clover => 4,
        Suit::UnSuited => 0,
    }
}

fn index_suit(i: u32) -> Suit {
    match i {
        1 => Suit::Spade,
        2 => Suit::Diamond,
        3 => Suit::Heart,
        4 => Suit::Clover,
        _ => Suit::UnSuited,
    }
}

fn suit_char(s: &Suit) -> String {
    s.to_string()
}

fn parse_suit_char(c: &str) -> Suit {
    match c {
        "s" => Suit::Spade,
        "d" => Suit::Diamond,
        "h" => Suit::Heart,
        "c" => Suit::Clover,
        _ => Suit::UnSuited,
    }
}

pub fn card_to_proto(c: &Card) -> u32 {
    match c {
        Card::Number(s, n) => suit_index(s) * 100 + *n as u32,
        Card::Joker(None) => 0,
        Card::Joker(Some((s, n))) => 9000 + suit_index(s) * 100 + *n as u32,
    }
}

pub fn proto_to_card(p: u32) -> Card {
    if p == 0 {
        Card::Joker(None)
    } else if p >= 9000 {
        let q = p - 9000;
        Card::Joker(Some((index_suit(q / 100), (q % 100) as u8)))
    } else {
        Card::Number(index_suit(p / 100), (p % 100) as u8)
    }
}

// --- Value attr helpers ----------------------------------------------------

fn as_int(v: &Value) -> Result<i64> {
    match v {
        Value::Int(i) => Ok(*i),
        _ => Err(anyhow!("expected Int")),
    }
}
fn as_bool(v: &Value) -> Result<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        _ => Err(anyhow!("expected Bool")),
    }
}
fn as_str(v: &Value) -> Result<&str> {
    match v {
        Value::Str(s) => Ok(s),
        _ => Err(anyhow!("expected Str")),
    }
}
fn as_list(v: &Value) -> Result<&Vec<Value>> {
    match v {
        Value::List(l) => Ok(l),
        _ => Err(anyhow!("expected List")),
    }
}

fn meta_get<'a>(attrs: &'a BTreeMap<String, Value>, k: &str) -> Result<&'a Value> {
    attrs.get(k).ok_or_else(|| anyhow!("meta attr {} missing", k))
}

// --- zone builders ---------------------------------------------------------

struct IdGen(u64);
impl IdGen {
    fn next(&mut self) -> u64 {
        let id = self.0;
        self.0 += 1;
        id
    }
}

fn cards_to_wire(cards: &[Card], ids: &mut IdGen, face: Face) -> Vec<WCard> {
    cards
        .iter()
        .map(|c| {
            // Carry a human-readable label (e.g. "3♠") in attrs so any generic
            // renderer (CLI, web UI) can display the card without re-deriving it
            // from the proto. Display data belongs in the view, not the consumer.
            let mut attrs = BTreeMap::new();
            attrs.insert("label".to_string(), Value::Str(c.to_string()));
            WCard { id: ids.next(), proto: card_to_proto(c), attrs, face }
        })
        .collect()
}

fn wire_to_cards(z: &Zone) -> Vec<Card> {
    z.cards.iter().map(|c| proto_to_card(c.proto)).collect()
}

// --- phase derivation ------------------------------------------------------

fn phase_of(game: &Game) -> String {
    if game.current.is_some() && game.active_player_ids().len() <= 1 {
        return "ended".to_string();
    }
    if let Some(p) = game.prompt.first() {
        use crate::game::PromptKind::*;
        return match p.kind {
            Select4 => "select4",
            Select7 => "select7",
            Select13 => "select13",
            UseOneChance => "one_chance",
        }
        .to_string();
    }
    "serve".to_string()
}

// --- to_view ---------------------------------------------------------------

pub fn to_view(game: &Game) -> GameView {
    let mut ids = IdGen(0);
    let mut zones: Vec<Zone> = vec![];

    // hand zones 0..N
    for (i, pid) in game.players.iter().enumerate() {
        let hand = game
            .field(&FieldKey::Hands(pid.clone()))
            .map(|d| d.0.clone())
            .unwrap_or_default();
        zones.push(Zone {
            id: i as u32,
            owner: Some(i as u32),
            kind: ZoneKind::Stack,
            visibility: Visibility::Owner,
            cards: cards_to_wire(&hand, &mut ids, Face::Up),
        });
    }

    // river (flattened) + river_groups
    let river_flat: Vec<Card> = game.river.iter().flatten().cloned().collect();
    let river_groups: Vec<Value> = game
        .river
        .iter()
        .map(|g| Value::Int(g.len() as i64))
        .collect();
    zones.push(Zone {
        id: RIVER,
        owner: None,
        kind: ZoneKind::Stack,
        visibility: Visibility::Public,
        cards: cards_to_wire(&river_flat, &mut ids, Face::Up),
    });

    // trushes / excluded
    let trushes = game.field(&FieldKey::Trushes).map(|d| d.0.clone()).unwrap_or_default();
    zones.push(Zone {
        id: TRUSHES,
        owner: None,
        kind: ZoneKind::Stack,
        visibility: Visibility::Public,
        cards: cards_to_wire(&trushes, &mut ids, Face::Up),
    });
    let excluded = game.field(&FieldKey::Excluded).map(|d| d.0.clone()).unwrap_or_default();
    zones.push(Zone {
        id: EXCLUDED,
        owner: None,
        kind: ZoneKind::Stack,
        visibility: Visibility::Public,
        cards: cards_to_wire(&excluded, &mut ids, Face::Up),
    });

    // meta card attrs
    let mut attrs: BTreeMap<String, Value> = BTreeMap::new();
    attrs.insert(
        "players".into(),
        Value::List(game.players.iter().map(|p| Value::Str(p.clone())).collect()),
    );
    attrs.insert(
        "river_size".into(),
        Value::Int(game.river_size.map(|n| n as i64).unwrap_or(-1)),
    );
    attrs.insert("revoluted".into(), Value::Bool(game.revoluted));
    attrs.insert("turn_revoluted".into(), Value::Bool(game.turn_revoluted));
    attrs.insert("is_step".into(), Value::Bool(game.is_step));
    let mut suit_limits: Vec<Value> = game
        .suit_limits
        .iter()
        .map(|s| Value::Str(suit_char(s)))
        .collect();
    suit_limits.sort_by(|a, b| format!("{:?}", a).cmp(&format!("{:?}", b)));
    attrs.insert("suit_limits".into(), Value::List(suit_limits));
    let mut effect_limits: Vec<i64> = game.effect_limits.iter().map(|n| *n as i64).collect();
    effect_limits.sort();
    attrs.insert(
        "effect_limits".into(),
        Value::List(effect_limits.into_iter().map(Value::Int).collect()),
    );
    let last_served = game
        .last_served_player_id
        .as_ref()
        .and_then(|id| game.players.iter().position(|p| p == id))
        .map(|i| i as i64)
        .unwrap_or(-1);
    attrs.insert("last_served".into(), Value::Int(last_served));
    attrs.insert("river_groups".into(), Value::List(river_groups));
    let ranks: Vec<Value> = game
        .ranks
        .iter()
        .filter_map(|id| game.players.iter().position(|p| p == id))
        .map(|i| Value::Int(i as i64))
        .collect();
    attrs.insert("ranks".into(), Value::List(ranks));

    zones.push(Zone {
        id: META,
        owner: None,
        kind: ZoneKind::Set,
        visibility: Visibility::Public,
        cards: vec![WCard {
            id: ids.next(),
            proto: META_PROTO,
            attrs,
            face: Face::Down,
        }],
    });

    // The engine's own bookkeeping. `Hidden`, so the core strips it from every
    // observed view: `selects` is what a player has picked but not yet
    // committed, and `answers` accumulates during the simultaneous one-chance
    // phase. Neither belongs on another seat's screen.
    let mut private: BTreeMap<String, Value> = BTreeMap::new();
    private.insert(
        "prompt".into(),
        Value::Str(serde_json::to_string(&game.prompt).unwrap()),
    );
    private.insert(
        "selects".into(),
        Value::Str(serde_json::to_string(&game.selects).unwrap()),
    );
    private.insert(
        "answers".into(),
        Value::Str(serde_json::to_string(&game.answers).unwrap()),
    );

    zones.push(Zone {
        id: META_PRIVATE,
        owner: None,
        kind: ZoneKind::Set,
        visibility: Visibility::Hidden,
        cards: vec![WCard {
            id: ids.next(),
            proto: META_PROTO,
            attrs: private,
            face: Face::Down,
        }],
    });

    let active_player = game
        .current
        .as_ref()
        .and_then(|id| game.players.iter().position(|p| p == id))
        .map(|i| i as u32);

    GameView {
        players: (0..game.players.len() as u32)
            .map(|id| Player { id })
            .collect(),
        zones,
        counters: BTreeMap::new(),
        phase: phase_of(game),
        turn: 0,
        active_player,
    }
}

// --- from_view -------------------------------------------------------------

/// Rebuild the internal model from a view.
///
/// Takes the **authoritative** view, never an observed one: `META_PRIVATE` is
/// `Hidden`, so the core has already stripped it from anything a client holds.
/// Every plugin entry point is called by the engine with the authoritative
/// state, so this holds.
pub fn from_view(view: &GameView) -> Result<Game> {
    let meta_zone = view
        .zone(META)
        .ok_or_else(|| anyhow!("meta zone {} missing", META))?;
    let meta = meta_zone
        .cards
        .first()
        .ok_or_else(|| anyhow!("meta card missing"))?;
    let attrs = &meta.attrs;

    let private_zone = view
        .zone(META_PRIVATE)
        .ok_or_else(|| anyhow!("private meta zone {} missing", META_PRIVATE))?;
    let private = &private_zone
        .cards
        .first()
        .ok_or_else(|| anyhow!("private meta card missing"))?
        .attrs;

    let players: Vec<String> = as_list(meta_get(attrs, "players")?)?
        .iter()
        .map(|v| as_str(v).map(|s| s.to_string()))
        .collect::<Result<_>>()?;

    let mut fields: HashMap<FieldKey, Deck> = HashMap::new();
    for (i, pid) in players.iter().enumerate() {
        let zone = view
            .zone(i as u32)
            .ok_or_else(|| anyhow!("hand zone {} missing", i))?;
        fields.insert(FieldKey::Hands(pid.clone()), Deck::new(wire_to_cards(zone)));
    }
    let trushes_zone = view.zone(TRUSHES).ok_or_else(|| anyhow!("trushes zone missing"))?;
    fields.insert(FieldKey::Trushes, Deck::new(wire_to_cards(trushes_zone)));
    let excluded_zone = view.zone(EXCLUDED).ok_or_else(|| anyhow!("excluded zone missing"))?;
    fields.insert(FieldKey::Excluded, Deck::new(wire_to_cards(excluded_zone)));

    // river: chunk the flat zone by river_groups
    let river_zone = view.zone(RIVER).ok_or_else(|| anyhow!("river zone missing"))?;
    let river_flat = wire_to_cards(river_zone);
    let groups: Vec<usize> = as_list(meta_get(attrs, "river_groups")?)?
        .iter()
        .map(|v| as_int(v).map(|i| i as usize))
        .collect::<Result<_>>()?;
    let mut river: Vec<Vec<Card>> = vec![];
    let mut offset = 0usize;
    for g in groups {
        river.push(river_flat[offset..offset + g].to_vec());
        offset += g;
    }

    let river_size = match as_int(meta_get(attrs, "river_size")?)? {
        -1 => None,
        n => Some(n as usize),
    };
    let suit_limits: HashSet<Suit> = as_list(meta_get(attrs, "suit_limits")?)?
        .iter()
        .map(|v| as_str(v).map(parse_suit_char))
        .collect::<Result<_>>()?;
    let effect_limits: HashSet<u8> = as_list(meta_get(attrs, "effect_limits")?)?
        .iter()
        .map(|v| as_int(v).map(|i| i as u8))
        .collect::<Result<_>>()?;
    let revoluted = as_bool(meta_get(attrs, "revoluted")?)?;
    let turn_revoluted = as_bool(meta_get(attrs, "turn_revoluted")?)?;
    let is_step = as_bool(meta_get(attrs, "is_step")?)?;

    let current = view
        .active_player
        .and_then(|i| players.get(i as usize).cloned());
    let last_served_player_id = match as_int(meta_get(attrs, "last_served")?)? {
        -1 => None,
        i => players.get(i as usize).cloned(),
    };

    let prompt: Vec<Prompt> = serde_json::from_str(as_str(meta_get(private, "prompt")?)?)?;
    let selects: HashMap<String, Vec<Card>> =
        serde_json::from_str(as_str(meta_get(private, "selects")?)?)?;
    let answers: HashMap<String, String> =
        serde_json::from_str(as_str(meta_get(private, "answers")?)?)?;
    let ranks: Vec<String> = as_list(meta_get(attrs, "ranks")?)?
        .iter()
        .map(|v| as_int(v).map(|i| players[i as usize].clone()))
        .collect::<Result<_>>()?;

    Ok(Game {
        prompt,
        fields,
        river,
        river_size,
        suit_limits,
        effect_limits,
        turn_revoluted,
        is_step,
        revoluted,
        current,
        last_served_player_id,
        players,
        selects,
        answers,
        ranks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{card_ord, Card, Suit};
    use crate::game::{Prompt, PromptKind};

    /// The plugin's only redaction duty: label each zone correctly. The core
    /// masks from these labels, so a wrong one here is a leak.
    #[test]
    fn every_zone_declares_who_may_see_it() {
        let v = to_view(&dealt_game());

        for seat in 0..3u32 {
            let z = v.zone(seat).expect("hand zone");
            assert_eq!(z.visibility, Visibility::Owner, "hand {seat}");
            assert_eq!(z.owner, Some(seat), "hand {seat} needs an owner to mask by");
        }

        for id in [RIVER, TRUSHES, EXCLUDED, META] {
            assert_eq!(v.zone(id).expect("zone").visibility, Visibility::Public, "zone {id}");
        }

        // selects/answers are a seat's uncommitted choices.
        assert_eq!(v.zone(META_PRIVATE).expect("private meta").visibility, Visibility::Hidden);
    }

    #[test]
    fn public_meta_carries_no_per_seat_state() {
        let v = to_view(&dealt_game());
        let attrs = &v.zone(META).unwrap().cards[0].attrs;
        for k in ["prompt", "selects", "answers"] {
            assert!(!attrs.contains_key(k), "{k} must live in META_PRIVATE");
        }
        // and the UI's keys stay where the UI looks for them
        for k in ["players", "ranks", "river_groups", "revoluted", "suit_limits", "is_step", "river_size"] {
            assert!(attrs.contains_key(k), "{k} missing from the public meta");
        }
    }

    fn dealt_game() -> Game {
        let mut g = Game::new(vec!["p0".into(), "p1".into(), "p2".into()]);
        let mut deck = Deck::all(2);
        let mut seed: u64 = 12345;
        let mut rng = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            seed
        };
        deck.shuffle_with(&mut rng);
        let decks = deck.split(3).unwrap();
        for (i, pid) in g.players.clone().iter().enumerate() {
            let mut d = decks[i].clone();
            d.sort(card_ord);
            g.fields.insert(FieldKey::Hands(pid.clone()), d);
        }
        g.current = Some("p0".into());
        g
    }

    #[test]
    fn roundtrip_fresh_dealt() {
        let g = dealt_game();
        let v = to_view(&g);
        let g2 = from_view(&v).unwrap();
        assert_eq!(g, g2);
    }

    #[test]
    fn roundtrip_mid_river_with_flags_and_prompt() {
        let mut g = dealt_game();
        // construct a mid-river state with effect flags + a pending prompt
        g.river = vec![
            vec![Card::Number(Suit::Spade, 3)],
            vec![Card::Number(Suit::Heart, 7)],
        ];
        g.river_size = Some(1);
        g.revoluted = true;
        g.turn_revoluted = true;
        g.is_step = true;
        g.suit_limits = HashSet::from([Suit::Spade, Suit::Heart]);
        g.effect_limits = HashSet::from([5u8, 8u8]);
        g.last_served_player_id = Some("p1".into());
        g.current = Some("p2".into());
        g.prompt = vec![Prompt {
            kind: PromptKind::Select7,
            player_ids: vec!["p2".into()],
            question: "select cards from hands".into(),
            options: vec!["ok".into()],
        }];
        g.selects.insert("p2".into(), vec![Card::Joker(None)]);
        g.answers.insert("p2".into(), "ok".into());
        // move some cards to trushes / excluded to exercise those zones
        g.fields.insert(
            FieldKey::Trushes,
            Deck::new(vec![Card::Number(Suit::Clover, 4)]),
        );
        g.fields.insert(
            FieldKey::Excluded,
            Deck::new(vec![Card::Number(Suit::Diamond, 2)]),
        );
        g.ranks = vec!["p0".into()];

        let v = to_view(&g);
        let g2 = from_view(&v).unwrap();
        assert_eq!(g, g2);
    }

    #[test]
    fn proto_roundtrip_all_cards() {
        for s in Suit::suits() {
            for n in 1u8..=13 {
                let c = Card::Number(s.clone(), n);
                assert_eq!(proto_to_card(card_to_proto(&c)), c);
            }
        }
        assert_eq!(proto_to_card(card_to_proto(&Card::Joker(None))), Card::Joker(None));
        let paired = Card::Joker(Some((Suit::Heart, 11)));
        assert_eq!(proto_to_card(card_to_proto(&paired)), paired);
    }
}
