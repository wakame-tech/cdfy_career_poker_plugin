use crate::{
    card::Card,
    game::{FieldKey, Game, Prompt},
};
use anyhow::{anyhow, Result};
use tera::Tera;

static APP_HTML: &[u8] = include_bytes!("templates/htmx.html");

/// text, data, selected
type DeckView = Vec<(String, String, bool)>;

pub struct Ctx {
    is_current: bool,
    current: Option<String>,
    trashes: DeckView,
    excluded: DeckView,
    river: DeckView,
    hands: DeckView,
    show_prompt: bool,
    prompt: Vec<Prompt>,
}

impl Ctx {
    fn into_deck_view(cards: &[Card], selects: &[Card]) -> DeckView {
        cards
            .iter()
            .map(|c| (c.char().to_string(), c.to_string(), selects.contains(c)))
            .collect()
    }

    pub fn new(game: &Game, player_id: String) -> Result<Self> {
        let is_current = game.current == Some(player_id.clone());
        let selects = &game.selects[&player_id];

        let trashes = Self::into_deck_view(
            &game
                .fields
                .get(&FieldKey::Trashes)
                .ok_or(anyhow!("trashes not found"))?
                .0,
            &selects,
        );
        let excluded = Self::into_deck_view(
            &game
                .fields
                .get(&FieldKey::Excluded)
                .ok_or(anyhow!("excluded not found"))?
                .0,
            &selects,
        );
        let river = Self::into_deck_view(game.river.last().unwrap_or(&vec![]), &[]);
        let hands = Self::into_deck_view(
            &game
                .fields
                .get(&FieldKey::Hands(player_id.to_string()))
                .ok_or(anyhow!("hands not found"))?
                .0,
            &selects,
        );

        let show_prompt = game
            .prompts
            .last()
            .map(|p| p.player_ids.contains(&player_id) && !game.answers.contains_key(&player_id))
            .unwrap_or(false);

        Ok(Self {
            is_current,
            current: game.current.clone(),
            trashes,
            excluded,
            river,
            hands,
            show_prompt,
            prompt: game.prompts.clone(),
        })
    }

    pub fn render(&self) -> Result<String> {
        let mut context = tera::Context::new();
        context.insert("is_current", &self.is_current);
        context.insert("current", &self.current);
        context.insert("trashes", &self.trashes);
        context.insert("excluded", &self.excluded);
        context.insert("river", &self.river);
        context.insert("hands", &self.hands);
        context.insert("show_prompt", &self.show_prompt);
        context.insert("prompt", &self.prompt);

        let html = Tera::one_off(std::str::from_utf8(APP_HTML)?, &context, false)?;
        Ok(html)
    }
}
