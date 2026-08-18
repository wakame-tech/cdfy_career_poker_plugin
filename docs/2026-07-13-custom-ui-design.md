# Career Poker custom UI — design

Date: 2026-07-13
Status: approved, not yet implemented

## Problem

The plugin only ships game logic; the cdfy_next server renders the generic
`GameView` — one button per advertised `legal_action`. For Daifugo every legal
play is a separate `serve` action, so a hand produces dozens of similarly
labelled buttons and the board is an undifferentiated list of zones. It is
playable but unreadable.

cdfy_next now supports **custom UI**: a plugin may ship a single self-contained
JS bundle (`ui.js`, pinned by sha256 in `meta.json`) that the web-ui runs inside
a strictly-sandboxed iframe, replacing the generic renderer. This design is that
bundle for Career Poker.

## Goal

A Daifugo-specific table: opponents with hand counts and ranks, the river with
its active flags (revolution / 縛り / 階段), and the player's own hand as
clickable cards that assemble a legal play. Full rule coverage — the special
prompt phases (Select4/7/13, one-chance) and the end screen included.

## Constraints (the cdfy_next custom-UI contract)

- **One self-contained JS file.** The iframe CSP is `default-src 'none'` /
  `connect-src 'none'`, so no imports, no fetch, no external fonts or scripts —
  everything inline. `ui-monospace` is a system font, so the DESIGN.md look needs
  no web font.
- **API:** the host injects `cdfy.onView(cb)` — `cb({ view, legal, seat })` fires
  on every update — and `cdfy.sendAction(kind, data)`. Render into
  `document.getElementById("cdfy-root")`.
- **`view` is already observed:** other players' hand zones arrive with their
  identity stripped (`proto` redacted, `attrs` cleared, `Face::Down`), so the
  bundle only ever sees counts. The cdfy_next core does this from
  `Zone.visibility`, not the plugin. The meta zone (200) is public game state
  and passes through; zone 201 is `Hidden` and never arrives.
- **Actions are validated by the host authority** (turn/seat + legality); the UI
  only *requests*. `data` is a list of card ids (small integers).

## The view (from `src/convert.rs`)

Zones:
- `0..N` — hand of player N (`owner = N`). Own hand `Face::Up`; others `Down`.
- `100` — river, flattened in serve order (`river_groups` in meta chunks it).
- `101` — trushes / graveyard. `102` — excluded.
- `200` — meta: one `Face::Down` card whose `attrs` carry every flag.

Each card: `attrs.label` = `{"Str": "3♠"}` (ready to render), `proto`, `id` (the
selection/serve key), `face`.

Meta attrs (`view.zone(200).cards[0].attrs`, each a tagged `Value`):
`players: List[Str]` (names), `ranks: List[Int]` (finish order, 大富豪→大貧民),
`revoluted: Bool`, `is_step: Bool`, `suit_limits: List[Str]` (縛り suits),
`river_size: Int` (−1 = none), `last_served: Int` (−1 = none; owns the field),
`river_groups: List[Int]`, `prompt: Str` (JSON of pending prompts).

`phase` ∈ `"serve" | "select4" | "select7" | "select13" | "one_chance" | "ended"`.
`active_player` is the seat to act.

## Layout (DESIGN.md look: mono, hairlines, pastel state blocks, pill buttons)

```
opponents:  p1 🂠×7   p2 🂠×5 [大富豪]   p3 🂠×9 ← turn (black border)
─── hairline ───
river:      7♥ 8♥        [pastel block]   革命 · 縛り♥ · 階段   river_size 2
─── hairline ───
your hand:  3♠ 4♠ [7♦] [7♣] 9♥ J♠ 🃏      ([] = selected, black-bordered pill)
[ serve ] [ pass ]                          (black pills; disabled off-turn)
```

- **Opponents row:** name + `🂠×<count>` (count from the zone's card length) +
  a rank badge when finished (大富豪/富豪/平民/貧民/大貧民 by `ranks` position),
  the active seat outlined.
- **River block:** the last `river_groups` chunk as the play to beat, plus chips
  for `revoluted` / each `suit_limits` suit / `is_step`, and `river_size`.
- **Own hand:** every card in the `owner === seat` zone as a label chip, sorted
  by proto; click toggles selection (selected = black-bordered pill).
- **Action bar:** phase-dependent (below).

## Interaction (legal-match)

The plugin enumerates every legal play as a distinct action in `legal`, so the
UI never computes rules — it matches the current selection against `legal`.

- **`serve` phase:** clicking hand cards builds a selected id-set. `[serve]` is
  enabled iff that set equals the `data` set of some `legal` action with
  `kind === "serve"`; clicking it sends `cdfy.sendAction("serve", selectedIds)`.
  `[pass]` shows iff a `legal` action has `kind === "pass"` → sends `("pass", [])`.
- **`select4 / select7 / select13`:** the prompt (parsed from `meta.prompt`, or
  simply "select cards") drives a pick-from-hand mode; `[confirm]` enabled when
  the selection matches a `legal` `select` action → sends `("select", ids)`.
- **`one_chance`:** render each `legal` `one_chance` action as "declare with
  <label>" (its `data` is `[ace_id]`) plus a "skip" that sends `("one_chance", [])`.
- **`ended`:** a rank table from `ranks` + `players`; no actions.
- Off-turn (`active_player !== seat`) or spectator: hand is read-only, no action
  buttons.

Card ids are assigned `0..` in `to_view` and a Daifugo deck is ≤ ~110 cards, so
ids fit in the plugin's `Vec<u8>` `data`.

## Verification — local preview harness

`dev/preview.html` + `dev/preview.js`: a mock `cdfy` global and a set of sample
`GameView` fixtures (one per phase: serve, select7, one_chance, ended),
hand-built to the `convert.rs` shape. Opening it renders the bundle standalone
(no server, no sandbox — that is cdfy_next's already-tested concern) so each
phase can be eyeballed and screenshotted. Doubles as the author's dev tool;
documented in the README. The fixtures are dev-only (not shipped in the wasm or
the published bundle).

## Publishing

```bash
./build.sh
cdfy push-game target/wasm32-unknown-unknown/release/cdfy_plugin_career_poker.wasm \
  --id career-poker --name "Career Poker" --min-players 2 --max-players 6 --ui ui.js
# then upload wasm + ui.js + meta.json to R2 (like any game)
```

`--ui` (added to the cdfy CLI in the custom-UI feature) records the bundle's
sha256 in `meta.json` and stores `games/career-poker/ui.js`. The README's push
section is updated to the `--ui` form. Live publishing to the deployed registry
is out of scope for this change (career-poker is not currently in the live
registry); it is a separate, operator-run step.

## Files

| File | Change |
|---|---|
| `ui.js` | New — the custom UI bundle (self-contained, inline CSS) |
| `dev/preview.html`, `dev/preview.js` | New — local preview harness + sample views |
| `README.md` | Push section uses `--ui`; add a "local UI preview" note |
| `docs/2026-07-13-custom-ui-design.md` | This design |

The Rust plugin (`src/**`), `build.rs`, `build.sh`, and `Cargo.toml` are
unchanged — the UI is pure client-side and needs no engine change.

## Out of scope

- Any change to the plugin's game logic or wire shape.
- Animations, drag-to-reorder, sound.
- Live publishing to the deployed cdfy_next registry (separate operator step).
- A shared UI SDK; the bundle uses the raw `cdfy` global directly.
