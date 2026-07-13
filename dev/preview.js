// Local preview harness for ui.js. Dev-only — never shipped in the wasm or the
// published bundle. Provides a mock `cdfy` global and a sample GameView per
// phase (built to the src/convert.rs shape), plus a bar to switch between them.
// Open dev/preview.html in a browser; click cards / buttons and watch the
// "last action" readout to verify what the bundle would send.
(function () {
  var cb = null, lastAction = "—", current = "serve";

  window.cdfy = {
    onView: function (f) { cb = f; },
    sendAction: function (kind, data) {
      lastAction = kind + " [" + data.join(",") + "]";
      console.log("sendAction", kind, data);
      paintBar();
    },
  };

  // tagged-Value + shape helpers (mirror src/convert.rs)
  function I(n) { return { Int: n }; }
  function S(s) { return { Str: s }; }
  function B(b) { return { Bool: b }; }
  function Lst(a) { return { List: a }; }
  function card(id, proto, face) { return { id: id, proto: proto, attrs: { label: S(String(proto)) }, face: face || "Up" }; }
  function down(base, n) { var a = []; for (var i = 0; i < n; i++) a.push(card(base + i, 0, "Down")); return a; }
  function zone(id, owner, vis, cards) { return { id: id, owner: owner, kind: "Stack", visibility: vis, cards: cards }; }
  function metaZone(attrs) { return { id: 200, owner: null, kind: "Set", visibility: "Hidden", cards: [{ id: 900, proto: 9999, attrs: attrs, face: "Down" }] }; }

  var players3 = Lst([S("you"), S("risa"), S("ken")]);

  // A serve turn with revolution + a heart lock on the field, a 4♥ to beat.
  var handA = [card(0, 103), card(1, 104), card(2, 207), card(3, 407), card(4, 309), card(5, 111), card(6, 0)];
  var serve = {
    view: {
      players: [{ id: 0 }, { id: 1 }, { id: 2 }],
      zones: [
        zone(0, 0, "Owner", handA),
        zone(1, 1, "Owner", down(100, 5)),
        zone(2, 2, "Owner", down(200, 8)),
        zone(100, null, "Public", [card(300, 304)]),
        zone(101, null, "Public", []),
        zone(102, null, "Public", []),
        metaZone({
          players: players3, ranks: Lst([]), revoluted: B(true), is_step: B(false),
          suit_limits: Lst([S("h")]), river_size: I(1), last_served: I(1),
          river_groups: Lst([I(1)]), prompt: S("[]"), selects: S("{}"), answers: S("{}"),
        }),
      ],
      counters: {}, phase: "serve", turn: 5, active_player: 0,
    },
    legal: [
      { kind: "serve", data: [2] }, { kind: "serve", data: [3] },
      { kind: "serve", data: [4] }, { kind: "serve", data: [5] },
      { kind: "serve", data: [6] }, { kind: "pass", data: [] },
    ],
    seat: 0,
  };

  // A 7-give prompt: pick one card from the hand to pass on.
  var select7 = {
    view: {
      players: [{ id: 0 }, { id: 1 }, { id: 2 }],
      zones: [
        zone(0, 0, "Owner", handA),
        zone(1, 1, "Owner", down(100, 4)),
        zone(2, 2, "Owner", down(200, 8)),
        zone(100, null, "Public", [card(300, 307), card(301, 107)]),
        zone(101, null, "Public", []),
        zone(102, null, "Public", []),
        metaZone({
          players: players3, ranks: Lst([]), revoluted: B(false), is_step: B(false),
          suit_limits: Lst([]), river_size: I(2), last_served: I(0),
          river_groups: Lst([I(2)]), prompt: S('[{"kind":"Select7","player_ids":["you"],"question":"give a card","options":["ok"]}]'),
          selects: S("{}"), answers: S("{}"),
        }),
      ],
      counters: {}, phase: "select7", turn: 6, active_player: 0,
    },
    legal: [0, 1, 2, 3, 4, 5, 6].map(function (id) { return { kind: "select", data: [id] }; }),
    seat: 0,
  };

  // One-chance: hold an ace, declare with it or skip.
  var handAce = [card(0, 101), card(1, 205), card(2, 413)];
  var oneChance = {
    view: {
      players: [{ id: 0 }, { id: 1 }, { id: 2 }],
      zones: [
        zone(0, 0, "Owner", handAce),
        zone(1, 1, "Owner", down(100, 3)),
        zone(2, 2, "Owner", down(200, 6)),
        zone(100, null, "Public", [card(300, 208)]),
        zone(101, null, "Public", []),
        zone(102, null, "Public", []),
        metaZone({
          players: players3, ranks: Lst([]), revoluted: B(false), is_step: B(false),
          suit_limits: Lst([]), river_size: I(1), last_served: I(2),
          river_groups: Lst([I(1)]), prompt: S('[{"kind":"UseOneChance","player_ids":["you"],"question":"one chance?","options":["ok"]}]'),
          selects: S("{}"), answers: S("{}"),
        }),
      ],
      counters: {}, phase: "one_chance", turn: 7, active_player: 0,
    },
    legal: [{ kind: "one_chance", data: [0] }, { kind: "one_chance", data: [] }],
    seat: 0,
  };

  // Ended: final standings.
  var ended = {
    view: {
      players: [{ id: 0 }, { id: 1 }, { id: 2 }],
      zones: [
        zone(0, 0, "Owner", []),
        zone(1, 1, "Owner", []),
        zone(2, 2, "Owner", []),
        zone(100, null, "Public", []),
        zone(101, null, "Public", []),
        zone(102, null, "Public", []),
        metaZone({
          players: players3, ranks: Lst([I(0), I(2), I(1)]), revoluted: B(false), is_step: B(false),
          suit_limits: Lst([]), river_size: I(-1), last_served: I(-1),
          river_groups: Lst([]), prompt: S("[]"), selects: S("{}"), answers: S("{}"),
        }),
      ],
      counters: {}, phase: "ended", turn: 30, active_player: null,
    },
    legal: [],
    seat: 0,
  };

  var FIX = { serve: serve, select7: select7, one_chance: oneChance, ended: ended };

  function show(name) { current = name; if (cb) cb(FIX[name]); paintBar(); }

  function paintBar() {
    var bar = document.getElementById("bar");
    if (!bar) return;
    bar.textContent = "";
    Object.keys(FIX).forEach(function (name) {
      var b = document.createElement("button");
      b.textContent = name;
      b.style.background = name === current ? "#dceeb1" : "#fff";
      b.onclick = function () { show(name); };
      bar.appendChild(b);
    });
    var act = document.createElement("span");
    act.className = "act";
    act.textContent = "last action: " + lastAction;
    bar.appendChild(act);
  }

  window.PREVIEW = { init: function () { paintBar(); show("serve"); } };
})();
