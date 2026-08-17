# Career Poker — cdfy_next plugin

An all-role Daifugo (大富豪 / Career Poker) game implemented as an
[Extism](https://extism.org/) plugin for the **cdfy_next** engine. The plugin is
pure game logic compiled to `wasm32-unknown-unknown`; the cdfy_next server hosts
it, drives turns, and renders the generic `GameView` state.

## What it is

The engine state is the cdfy_next `GameView` (zones of cards + a meta card that
carries the Daifugo flags). `apply_action` decodes the view into the internal
`Game` model (`src/game.rs`), runs the ported rules (`src/rules.rs`), and
re-encodes. All randomness — the initial shuffle and the one-chance
resolution — is drawn from the host `rand_u64` function, so play is
deterministic and replayable from the room seed.

Supported plays: singles/pairs/triples/quads, plus the role effects 3 (三途),
4 (死者蘇生), 5 (スキップ), 7 (7渡し), 8 (8切り), 9 (阿修羅), 10 (十戒),
11 (Jバック), 12 (摩訶鉢特摩), 13 (ロイヤルレリーフ), 2 (除外), 1 (ワンチャンス),
revolution (4-of-a-kind), and the post-round 大富豪↔大貧民 card exchange.

> The one-chance rock-paper-scissors is approximated as a probability gated by
> `rand_u64` (declarer wins with probability `1/active_players`) — a deliberate
> simplification documented in the port plan.

## Exports (cdfy_next plugin contract)

I/O is JSON over the Extism ABI; the shapes live in `src/wire.rs`.

| export          | input               | output            |
| --------------- | ------------------- | ----------------- |
| `setup`         | `Config`            | `GameView`        |
| `apply_action`  | `[GameView, Action]`| `GameView`        |
| `legal_actions` | `[GameView, PlayerId]` | `[Action]`     |
| `observe`       | `[GameView, PlayerId]` | `GameView`     |
| `status`        | `GameView`          | `Status`          |

`observe` is the identity function. Per-seat masking is the cdfy_next core's
job: it reads `Zone.visibility` and strips what a viewer may not see after
`observe` returns. This plugin's only duty is to label its zones — hands
`Owner`, river and discards `Public`, engine bookkeeping `Hidden`.

Host import the plugin declares and calls for randomness:

```rust
#[host_fn]
extern "ExtismHost" { fn rand_u64() -> u64; }
```

### Actions

`Action { kind, data }`, where `data` is a JSON byte payload:

- `"serve"` — `data = [card_id, …]`: play those cards.
- `"pass"` — `data = []`.
- `"select"` — `data = [card_id, …]`: answer a pending Select4/7/13 prompt.
- `"one_chance"` — `data = [ace_card_id]` to declare, or `[]` to skip.

## Build

Requires the wasm target once: `rustup target add wasm32-unknown-unknown`.

```bash
./build.sh
# => target/wasm32-unknown-unknown/release/cdfy_plugin_career_poker.wasm
```

or directly:

```bash
cargo build --target wasm32-unknown-unknown --release
```

## Test

```bash
cargo test
```

Runs the in-crate unit tests plus the integration suites `tests/effects.rs`
(one assertion per card effect) and `tests/full_game.rs` (a deterministic game
driven through the `rules` functions with an injected counter RNG).

> `build.rs` passes `-Wl,-undefined,dynamic_lookup` (host + test binaries only)
> so the crate links on the host even though `lib.rs`'s `#[plugin_fn]` exports
> reference the extism host ABI. The flag is **not** applied to the
> `wasm32-unknown-unknown` build, where the runtime resolves those imports.

## Push to a cdfy_next store and play

Build the wasm, register it with the server's game store under a stable id, then
create a room for it:

```bash
# 1. build
./build.sh

# 2. push the wasm + the custom UI bundle into the cdfy_next store
cdfy push-game target/wasm32-unknown-unknown/release/cdfy_plugin_career_poker.wasm \
  --id career-poker --name "Career Poker" --min-players 2 --max-players 6 \
  --ui ui.js

# 3. create a room (N players) on the running cdfy_next server, then play.
```

`--ui ui.js` records the bundle's sha256 in `meta.json` and stores it at
`games/career-poker/ui.js`; upload the wasm, `ui.js`, and `meta.json` to R2 like
any game.

## Custom UI

`ui.js` is a self-contained Daifugo table that cdfy_next runs inside a
sandboxed iframe, replacing the generic renderer (which shows one button per
`legal_action` — dozens of near-identical `serve` buttons for Daifugo). It draws
the opponents' hand counts and ranks, the river with its active flags
(革命 / 縛り / 階段), and your own hand as clickable cards; a selection is matched
against the advertised `legal` set, so only legal plays can be served. See
`docs/2026-07-13-custom-ui-design.md`.

### Local UI preview

Open `dev/preview.html` in a browser (no server needed). It loads `ui.js` with a
mock `cdfy` host and sample views for each phase (serve / select / one-chance /
ended); the bar switches between them and shows the action a click would send.
`dev/` is a developer tool and is not part of the published bundle.
