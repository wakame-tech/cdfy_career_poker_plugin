use crate::{
    card::Card,
    game::{FieldKey, Game, Prompt},
    plugin::RenderConfig,
};
use anyhow::Result;
use tera::Tera;

static APP_HTML: &[u8] = include_bytes!("templates/app.html");

/// text, data, selected
type DeckView = Vec<(String, String, bool)>;

pub struct Ctx {
    is_current: bool,
    current: Option<String>,
    trushes: DeckView,
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

    pub fn new(game: &Game, config: &RenderConfig) -> Self {
        let is_current = game.current == Some(config.player_id.clone());
        let selects = &game.selects[&config.player_id];

        let trushes =
            Self::into_deck_view(&game.fields.get(&FieldKey::Trushes).unwrap().0, &selects);
        let excluded =
            Self::into_deck_view(&game.fields.get(&FieldKey::Excluded).unwrap().0, &selects);
        let river = Self::into_deck_view(game.river.last().unwrap_or(&vec![]), &[]);
        let hands = Self::into_deck_view(
            &game.fields[&FieldKey::Hands(config.player_id.to_string())].0,
            &selects,
        );

        let show_prompt = game
            .prompt
            .last()
            .map(|p| {
                p.player_ids.contains(&config.player_id)
                    && !game.answers.contains_key(&config.player_id)
            })
            .unwrap_or(false);

        Self {
            is_current,
            current: game.current.clone(),
            trushes,
            excluded,
            river,
            hands,
            show_prompt,
            prompt: game.prompt.clone(),
        }
    }

    pub fn render(&self) -> Result<String> {
        let mut context = tera::Context::new();
        context.insert("is_current", &self.is_current);
        context.insert("current", &self.current);
        context.insert("trushes", &self.trushes);
        context.insert("excluded", &self.excluded);
        context.insert("river", &self.river);
        context.insert("hands", &self.hands);
        context.insert("show_prompt", &self.show_prompt);
        context.insert("prompt", &self.prompt);

        let html = Tera::one_off(std::str::from_utf8(APP_HTML).unwrap(), &context, false)?;
        Ok(html)
    }
}
