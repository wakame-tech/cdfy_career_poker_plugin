# Career Poker → cdfy_next port — Implementation Plan

> **For agentic workers:** Implement task-by-task. Tasks 1–6 are the SEQUENTIAL foundation (do in order, one PR each). Tasks 7–11 are PARALLELIZABLE once the foundation builds. The existing game logic in this repo is the reference — reuse it; the work is swapping the ABI + state representation, not reinventing the rules.

**Goal:** Port this all-role Daifugo plugin from the old cdfy ABI (`init_game`/`handle_event`/`render`, `var::` persistence, HTML) to the **cdfy_next plugin contract** (`setup`/`apply_action`/`legal_actions`/`observe`/`status` over a generic `GameView` JSON, with the `rand_u64` host function), reusing the existing card/deck/effect logic and adding thorough tests.

**Architecture:** The state is the cdfy_next `GameView`. Card fields (hands, river, trushes, excluded) become `GameView` zones; the non-card engine flags (revoluted, effect_limits, prompt, …) are encoded in a reserved "meta" card's `attrs`. `apply_action` decodes the `GameView` into the existing internal `Game`, maps the `Action` to the existing handler logic, runs it, and re-encodes. Randomness (the initial shuffle, the one-chance RPS resolved by probability) comes from the `rand_u64` host function so play is deterministic and replayable.

**Tech Stack:** Rust, `extism-pdk` (the plugin already uses it), `serde`/`serde_json`. Target `wasm32-unknown-unknown`. Existing deps stay; **remove `tera`** (no HTML) and **stop using `rand::thread_rng`** (use the host RNG).

---

## The cdfy_next contract (authoritative)

The plugin exports these Extism functions; I/O is JSON (see the shapes below, which mirror cdfy_next's `docs/wire-contract.md`):

| export | input | output |
|---|---|---|
| `setup` | `Config` | `GameView` |
| `apply_action` | `[GameView, Action]` | `GameView` |
| `legal_actions` | `[GameView, PlayerId]` | `[Action]` |
| `observe` | `[GameView, PlayerId]` | `GameView` |
| `status` | `GameView` | `Status` |

Host import the plugin declares and calls for randomness:
```rust
#[host_fn]
extern "ExtismHost" { fn rand_u64() -> u64; }
```
The host writes the `u64` as 8 little-endian bytes into Extism memory and returns the offset; read 8 LE bytes back (see cdfy_next's `test-plugin`/`moon-plugin` for the exact read). Shuffle and RPS draw from it.

### Wire types (re-defined here; must JSON-match the contract)

`src/wire.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Value { Int(i64), Str(String), Bool(bool), List(Vec<Value>) }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Face { Up, Down }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZoneKind { Stack, Set }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility { Public, Owner, Hidden }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Card {
    pub id: u64,
    pub proto: u32,
    pub attrs: BTreeMap<String, Value>,
    pub face: Face,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    pub id: u32,
    pub owner: Option<u32>,
    pub kind: ZoneKind,
    pub visibility: Visibility,
    pub cards: Vec<Card>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Player { pub id: u32 }
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GameView {
    pub players: Vec<Player>,
    pub zones: Vec<Zone>,
    pub counters: BTreeMap<String, i64>,
    pub phase: String,
    pub turn: u32,
    pub active_player: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Action { pub kind: String, pub data: Vec<u8> }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Status { Running, Ended { winners: Vec<u32> } }
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Config { pub data: Vec<u8> }
```
A `wire.rs` test must assert the exact JSON for a small `GameView`/`Status` matches the contract examples (external-tagged `Status`, bare-number ids, `face` `"Up"`/`"Down"`, `null` owner). This is the drift guard against `docs/wire-contract.md`.

---

## GameView encoding of the Daifugo state (the crux — read carefully)

Players are cdfy_next `PlayerId(u32)` = the index into the existing `Game.players` (player N ⇒ `players[N]`). Keep a stable mapping `u32 ⇔ String id` (use `format!("p{n}")` for the internal `String`).

**Zones** (fixed ids):
- `0..N` — hand of player N (`owner = Some(N)`, `visibility = Owner`, `ZoneKind::Stack`).
- `100` — `river` (all served cards, in serve order; `Public`).
- `101` — `trushes` / graveyard (`Public`).
- `102` — `excluded` (`Public`).
- `200` — **meta** zone holding exactly one meta `Card` (proto `9999`) whose `attrs` carry all non-card engine flags (`Hidden`).

**Card mapping** (`Card` ⇔ internal `card::Card`):
- `Number(suit, n)` → `proto = suit_index*100 + n` where suit_index: Spade=1, Diamond=2, Heart=3, Clover=4 (matching the existing `Suit`). `attrs` empty.
- `Joker(None)` → `proto = 0`. `Joker(Some((suit,n)))` → `proto = 9000 + suit_index*100 + n` (paired joker carries its mimicked card).
- `id` — a stable per-instance u64 assigned at deal (0..53). Persist ids across moves (a card keeps its id when it changes zones). `face` = `Up` for public zones / `Owner` hands when observing self, `Down` otherwise (the `observe` step sets this; internal logic ignores `face`).

**Meta card attrs** (`attrs: BTreeMap<String, Value>` on the meta card) — encode the rest of `Game`:
- `river_size: Int` (or absent/`Int(-1)` for `None`).
- `revoluted: Bool`, `turn_revoluted: Bool`, `is_step: Bool`.
- `suit_limits: List[Str]` (suit chars), `effect_limits: List[Int]`.
- `last_served: Int` (player index, or `-1`).
- `river_groups: List[Int]` — the sizes of each serve in the river, so `river: Vec<Vec<Card>>` can be reconstructed from the flat zone-100 cards.
- `prompt: List[...]` and `selects`/`answers` — encode the pending-prompt state (`PromptKind`, target player ids, collected selects/answers) as nested `Value`. (Selects are also reconstructable; keep them here so multi-step prompts survive across `apply_action` calls.)

**phase** (string): `"serve"` (normal turn), `"select4"|"select7"|"select13"|"one_chance"` (a prompt is pending), `"ended"`.

**active_player** = `Game.current` index. **turn** = a move counter.

`src/convert.rs` provides `to_view(&Game) -> GameView` and `from_view(&GameView) -> Game` and MUST round-trip: `from_view(to_view(g)) == g` for any reachable `g` (test this on several constructed games).

---

## Action protocol

`Action.kind` + `Action.data` (JSON-encoded payload in `data` bytes):
- `"serve"` — `data` = JSON `[card_id, …]` of the cards to play. (Replaces the old select-then-serve; the plugin sets `selects[current] = those cards` then runs the existing `Serve` handler.)
- `"pass"` — `data` = `[]`.
- `"select"` — `data` = `[card_id, …]` answering a pending `select4/7/13` (cards chosen from trushes/hands/excluded), then runs the existing `Answer`/select-resolution flow.
- `"one_chance"` — `data` = `[ace_card_id]` or `[]` to skip; resolves the `UseOneChance` prompt. The RPS is decided by `rand_u64` (the declarer wins with probability `1/active_players`); on win, the existing one-chance effect runs.

`apply_action` validates the action belongs to the current phase/player and maps to the existing handlers (`Serve`, `Pass`, `Answer`, effect dispatch). On a rules error, return the input `GameView` unchanged with `phase` carrying an error marker, OR (simpler) propagate as an Extism error so cdfy_next surfaces `EngineError::Plugin`. Choose the Extism-error path — the engine + tests treat it as a rejected action.

---

## Tasks

> Reuse the existing `src/card.rs`, `src/deck.rs`, `src/game.rs`, and `src/events/*` logic. The port strips `extism_pdk` `var::`/`ToBytes`/`Event`/`render`/`tera` and the `EventHandler` trait indirection, turning handlers into plain functions on `Game`.

### Task 1 (foundation): wire types + JSON-shape test
- Create `src/wire.rs` as above. Add a test asserting the JSON of a sample `GameView` and `Status::Ended{winners:vec![0]}` matches the contract (`{"Ended":{"winners":[0]}}`, bare-number ids, `"face":"Up"`). Commit `feat: cdfy_next wire types`.

### Task 2 (foundation): internal model, de-extism'd + host RNG
- Keep `src/card.rs` (Card/Suit/ordering) as-is.
- `src/deck.rs`: replace `shuffle(&mut self)` using `rand::thread_rng` with `shuffle_with(&mut self, rng: &mut impl FnMut() -> u64)` doing Fisher-Yates with `rng() % (i+1)` (matches cdfy_next `RngState::shuffle`). Remove the `rand` dependency.
- `src/game.rs`: drop `ToBytes`/`FromBytesOwned` impls; keep the struct + methods. Keep `Prompt`/`FieldKey`/effect flags.
- Add `src/rng.rs`: the `#[host_fn] rand_u64` import + a `fn next_u64() -> u64` reading the 8 LE bytes from the returned offset (copy the read pattern from cdfy_next `moon-plugin`/`test-plugin`; in Rust extism-pdk a plain `-> u64` host fn auto-reads it, so `fn rand_u64() -> u64 = "extism:host/user" "rand_u64"` suffices — confirm).
- Commit `feat: internal model uses host RNG, no extism var`.

### Task 3 (foundation): GameView ↔ Game conversion
- `src/convert.rs`: `to_view`/`from_view` per the encoding above. Round-trip test on (a) a fresh dealt game, (b) a mid-river game with effect flags + a pending prompt. Commit `feat: GameView <-> Game conversion`.

### Task 4 (foundation): port the handlers to plain functions
- `src/rules.rs` (or keep `events/` but strip the trait): `serve(game, player, card_ids)`, `pass(game, player)`, `answer_select(game, player, card_ids)`, `answer_one_chance(game, player, ace_or_skip, rng)`, and `effect(game, player, serves)` — copied from the existing `events/serve.rs`, `pass.rs`, `answer.rs`, `effect_card.rs`, `select.rs`, with `EventHandler::on` bodies inlined and `var::` removed. RPS in one-chance uses `rng`. Unit-test serve legality (same-number, ordering, river-size, step, suit) and pass. Commit `feat: rules as pure functions`.

### Task 5 (foundation): ABI lib.rs
- Rewrite `src/lib.rs`: remove `init_game`/`handle_event`/`render`/`get_state`. Add `#[plugin_fn] setup/apply_action/legal_actions/observe/status` using `wire`, `convert`, `rules`, `rng`. `setup` deals via the host RNG (port `Distribute`). `apply_action` decodes `[GameView, Action]`, dispatches by `kind`, re-encodes. `status` = `Ended{winners}` when `active_player_ids().len() <= 1` (winners = finish order). Remove `tera`, `game_view.rs`, `templates/`, `game.js`.
- `moon.pkg.json`/exports n/a (this is Rust); ensure `Cargo.toml` `crate-type=["cdylib"]` and exports compile.
- Build: `cargo build --target wasm32-unknown-unknown --release`. Commit `feat: cdfy_next ABI (setup/apply_action/legal_actions/observe/status)`.

### Task 6 (foundation): finish-order roles + exchange
- Track finish order (when a hand empties, append the player to a `ranks: List[Int]` meta attr). `status` winners = that order. Add the post-round role card exchange (大富豪↔大貧民 give strongest, 富豪↔貧民 give a chosen/auto card) as a `setup`-of-next-round step or an `exchange` phase. (If the existing repo lacks exchange, implement: 大富豪 gives 大貧民 the 2 weakest? No — 大貧民 gives 大富豪 the 2 strongest, 大富豪 gives back any 2; for v1 auto-give strongest/weakest.) Commit `feat: finish-order roles and card exchange`.

> After Task 6 builds and the foundation tests pass, Tasks 7–11 may run IN PARALLEL (independent files).

### Task 7 (parallel): `legal_actions`
- Enumerate the current player's legal plays as concrete `Action`s: every legal `serve` (singles/pairs/triples/quads of same number from hand that beat the river top under current flags — reuse `ValidateServe` logic), plus `pass` when the river is non-empty, plus the right `select`/`one_chance` actions when a prompt is pending. Each `serve` action's `data` = the chosen card ids. Test: from a known hand + empty river, the singles/pairs are all present; with a top card, only beating plays appear. File `src/legal.rs`. Commit `feat: legal_actions enumeration`.

### Task 8 (parallel): `observe` masking
- `observe(view, player)`: clone the view, set `face = Down` on every card in zones owned by a DIFFERENT player (and the river/trushes stay `Up`; deck/hidden zones of others masked). Test: player 0 sees own hand `Up`, player 1's hand `Down`. File: add to `lib.rs` or `src/observe.rs`. Commit `feat: per-player observe masking`.

### Task 9 (parallel): effect unit tests
- One test per effect (3 三途, 4 死者蘇生/Select4, 5 skip, 6, 7 渡し/Select7, 8 切り, 9 阿修羅, 10 十戒, 11 back, 12 摩訶鉢特摩, 13 ロイヤルレリーフ/Select13, 2 除外, 1 one-chance, revolution on 4 cards) asserting the resulting `Game`/flags. `tests/effects.rs`. Commit `test: per-effect behavior`.

### Task 10 (parallel): deterministic full-game test
- `tests/full_game.rs`: 4 players, fixed seed (drive a mock `rand_u64` sequence or run through the real host in a small harness), play a scripted sequence of `serve`/`pass` to completion, assert `status` becomes `Ended` with a winners order. If running pure-Rust (no host), call the `rules` functions directly with an injected rng closure. Commit `test: deterministic full game`.

### Task 11 (parallel): build the wasm fixture + README
- `build.sh`: `cargo build --target wasm32-unknown-unknown --release` → copy the `.wasm` to a known path. Update `README.md` for the cdfy_next contract + how to push it to a cdfy_next store and play it. Commit `docs: build + usage for cdfy_next`.

---

## Done criteria
- `cargo test` (host-target unit/integration tests) green; `cargo build --target wasm32-unknown-unknown --release` produces the plugin wasm.
- The wasm runs on cdfy_next: `cdfy push-game <wasm> --id career-poker`, create a room with N players, and `legal_actions`/`serve`/`pass` drive a game to `Ended` (manual smoke against the cdfy_next server).
- Determinism: same room seed ⇒ same shuffle + same RPS outcomes (replay reproduces).
- All 13 effects + revolution + roles covered by tests; `from_view(to_view(g)) == g` round-trips.

## Notes
- The generic cdfy_next web-ui renders one button per advertised `legal_action` (label = `kind`); for Daifugo every legal play is a separate `serve` action, so buttons will be many and similarly labeled — acceptable for now (a Daifugo-specific UI is a later, separate concern).
- RPS-as-probability is a deliberate simplification of the real one-chance rock-paper-scissors; documented as such.
