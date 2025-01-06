use crate::{
    card::{card_ord, cardinal, is_same_number, match_suits, number, suits, Card},
    cards_effect::{effect_cards, CardsEffect},
    deck::{deck_ord, Deck},
    game::{FieldKey, Game, Prompt, PromptKind},
};
use anyhow::{anyhow, Result};
use std::collections::HashSet;

#[derive(serde::Serialize, serde::Deserialize)]
// #[serde(tag = "name", content = "value")]
#[serde(tag = "name")]
pub enum Event {
    Distribute,
    Select {
        field: String,
        card: String,
    },
    Answer {
        option: String,
    },
    Serve,
    Pass,
    // builtin events
    None,
    Exit,
    LaunchPlugin {
        plugin_name: String,
    },
    PluginStarted {
        state_id: String,
    },
    PluginFinished {
        state_id: String,
        value: serde_json::Value,
    },
}

pub fn dispatch_event(mut game: Game, event: Event) -> Result<(Game, Event)> {
    let game = match event {
        Event::Distribute => distribute(game),
        Event::Select { field, card } => {
            let card = Card::try_from(card.as_str())?;
            game.toggle_select(&field, card)?;
            Ok(game)
        }
        Event::Answer { .. } => on_answer(game),
        Event::Serve => on_serve(game),
        Event::Pass => pass(game),
        _ => Ok(game),
    }?;
    Ok((game, Event::None))
}

fn distribute(mut game: Game) -> Result<Game> {
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
    Ok(game)
}

fn flush_river(mut game: Game, to: &FieldKey) -> Result<Game> {
    let cards = game.river.iter().flatten().cloned().collect::<Vec<_>>();
    game.field_mut(to)?.0.extend(cards);
    game.river.clear();
    game.river_size = None;
    game.suit_bans = HashSet::new();
    game.effect_bans = HashSet::new();
    game.turn_ord_reversed = false;
    game.is_step = false;
    Ok(game)
}

fn on_end_turn(mut game: Game) -> Result<Game> {
    let player_id = game.current.clone().unwrap();

    let hand = game.field(&FieldKey::Hands(player_id.clone()))?;
    if hand.0.is_empty() && game.active_player_ids().len() == 1 {
        return Err(anyhow!("end"));
    }

    let top = game
        .river
        .last()
        .expect("river must not be empty when end turn");

    // next player
    let skips = match top {
        _ if number(top) == 5 && !game.effect_bans.contains(&5) => top.len() as i32 + 1,
        _ if number(top) == 8 && !game.effect_bans.contains(&8) => 0,
        _ if number(top) == 1 && !game.effect_bans.contains(&1) => 0,
        _ => 1,
    };
    game.current = Some(game.get_relative_player(&player_id, skips));

    // flush
    if game.current == game.last_served_player_id {
        let to = if number(top) == 2 && !game.effect_bans.contains(&2) {
            FieldKey::Excluded
        } else {
            FieldKey::Trashes
        };
        game = flush_river(game, &to)?;
    }
    Ok(game)
}

fn pass(game: Game) -> Result<Game> {
    let current_player_id = game.current.clone().unwrap();
    if let Some(prompt) = game.prompts.first() {
        if prompt.player_ids.contains(&current_player_id)
            && !game.answers.contains_key(&current_player_id)
        {
            return Err(anyhow!("please answer"));
        }
    }
    if game.river.is_empty() {
        return Err(anyhow!("cannot pass because river is empty"));
    }
    Ok(game)
}

fn check_river_size(game: &Game, serves: &[Card]) -> Result<()> {
    let ok = match game.river_size.unwrap() {
        3 if number(serves) == 9 && serves.len() == 1 => !game.effect_bans.contains(&9),
        _ => game.river_size.unwrap() == serves.len(),
    };
    if !ok {
        return Err(anyhow!(
            "expected river size {} but {}",
            game.river_size.unwrap(),
            serves.len()
        ));
    }
    Ok(())
}

fn check_ordering(game: &Game, serves: &[Card]) -> Result<()> {
    let top = game.river.last().unwrap();
    let ordering = if game.ord_reversed ^ game.turn_ord_reversed {
        deck_ord(serves, top).reverse()
    } else {
        deck_ord(serves, top)
    };
    if ordering.is_lt() {
        return Err(anyhow!("must be greater than top card"));
    }
    Ok(())
}

fn check_suits(game: &Game, serves: &[Card]) -> Result<()> {
    if !game.suit_bans.is_empty() && !match_suits(game.river.last().unwrap(), serves) {
        return Err(anyhow!(
            "expected suits {:?} but {:?}",
            game.suit_bans,
            suits(serves)
        ));
    }
    Ok(())
}

fn check_steps(game: &Game, serves: &[Card]) -> Result<()> {
    if game.is_step {
        let top = game.river.last().unwrap();
        if cardinal(number(serves)) - cardinal(number(top)) != 1 {
            return Err(anyhow!("must be step"));
        }
    }
    Ok(())
}

fn validate_serve(game: &Game, serves: &[Card]) -> Result<()> {
    if !is_same_number(&serves) {
        return Err(anyhow!("not same number"));
    }
    if game.river.is_empty() {
        return Ok(());
    };
    check_ordering(game, &serves)?;
    check_river_size(game, &serves)?;
    check_steps(game, &serves)?;
    check_suits(game, &serves)?;
    Ok(())
}

fn on_serve(mut game: Game) -> Result<Game> {
    let player_id = game.current.clone().unwrap();
    if let Some(prompt) = game.prompts.first() {
        if prompt.player_ids.contains(&player_id) && !game.answers.contains_key(&player_id) {
            return Err(anyhow!("please answer"));
        }
    }

    // validate serve
    let serves = game.selects.get(&player_id).unwrap().clone();
    if game.current != Some(player_id.clone()) {
        return Err(anyhow!("not your turn"));
    }
    if serves.is_empty() {
        return Err(anyhow!("please select cards"));
    }
    validate_serve(&game, &serves)?;
    // serve to river
    game.field_mut(&FieldKey::Hands(player_id.clone()))?
        .remove(&serves)?;
    game.river.push(serves.clone());

    let has_1_player_ids = game
        .active_player_ids()
        .iter()
        .filter(|id| {
            id != &&player_id
                && game
                    .field(&FieldKey::Hands(id.to_string()))
                    .unwrap()
                    .0
                    .iter()
                    .any(|c| c.number() == Some(1))
        })
        .cloned()
        .collect::<Vec<_>>();

    if !game.effect_bans.contains(&1) && !has_1_player_ids.is_empty() {
        let prompt = Prompt {
            kind: PromptKind::UseOneChance,
            player_ids: has_1_player_ids,
            question: "select A if use one chance".to_string(),
            options: vec!["serve".to_string(), "skip".to_string()],
        };
        game.prompts.push(prompt);
    }

    let effects = CardsEffect::new(&game, &serves);
    for effect in effects {
        game = effect_cards(game, effect)?;
    }
    game.last_served_player_id = Some(player_id.to_string());
    game = on_end_turn(game)?;
    Ok(game)
}

fn on_answer(mut game: Game) -> Result<Game> {
    let Some(prompt) = game.prompts.last().cloned() else {
        return Err(anyhow!("no prompt"));
    };
    game = match prompt.kind {
        PromptKind::Select4 => try_effect_4(game),
        PromptKind::Select7 => try_effect_7(game),
        PromptKind::Select13 => try_effect_13(game),
        PromptKind::UseOneChance => try_effect_one_chance(game),
    }?;
    let all_answered = prompt.player_ids.iter().collect::<HashSet<_>>()
        == game.answers.keys().collect::<HashSet<_>>();
    if all_answered {
        let player_id = game.current.clone().unwrap();
        game.answers.clear();
        game.prompts.pop();
        // reset select
        game.selects.insert(player_id.to_string(), vec![]);

        game.last_served_player_id = Some(player_id.to_string());
        game = on_end_turn(game)?;
    }

    Ok(game)
}

fn try_effect_4(mut game: Game) -> Result<Game> {
    let player_id = game.current.clone().unwrap();
    let selects = game.selects.get(&player_id).unwrap().clone();
    let n = game.river.last().unwrap().len();
    if selects.len() != n {
        return Err(anyhow!("please select {} cards in trashes", n));
    }
    game.transfer(
        &FieldKey::Trashes,
        &FieldKey::Hands(player_id.clone()),
        selects,
    )?;
    Ok(game)
}

fn try_effect_7(mut game: Game) -> Result<Game> {
    let player_id = game.current.clone().unwrap();
    let n_cards = game.river.last().unwrap().len();
    if game.selects.get(&player_id).unwrap().len() != n_cards {
        return Err(anyhow!("please select {} cards in hands", n_cards));
    }
    let selects = game.selects.get(&player_id).unwrap().clone();
    let passer: String = game.get_relative_player(&player_id, -1);
    game.transfer(
        &FieldKey::Hands(player_id.to_string()),
        &FieldKey::Hands(passer.clone()),
        selects,
    )?;
    game.field_mut(&FieldKey::Hands(passer))?.sort(card_ord);
    Ok(game)
}

fn try_effect_13(mut game: Game) -> Result<Game> {
    let player_id = game.current.clone().unwrap();
    let n_cards = game.river.last().unwrap().len();
    if game.selects.get(&player_id).unwrap().len() != n_cards {
        return Err(anyhow!("please select {} cards in excluded", n_cards));
    }
    let selects = game.selects.get(&player_id).unwrap().clone();
    game.transfer(
        &FieldKey::Excluded,
        &FieldKey::Hands(player_id.clone()),
        selects,
    )?;
    game.field_mut(&FieldKey::Hands(player_id.to_string()))?
        .sort(card_ord);
    Ok(game)
}

fn try_effect_one_chance(game: Game) -> Result<Game> {
    let player_id = game.current.clone().unwrap();
    let serves = game.selects.get(&player_id).unwrap().clone();
    if serves.len() != 1 || number(&serves) != 1 {
        return Err(anyhow!("please select A"));
    }
    Ok(game)
}
