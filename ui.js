// Career Poker (Daifugo) custom UI for cdfy_next.
//
// A single self-contained bundle. The cdfy_next host runs it inside a
// sandboxed iframe (CSP default-src 'none', connect-src 'none'), so everything
// here is inline — no imports, no fetch, no external fonts (ui-monospace is a
// system font). Contract: the host injects `cdfy.onView(cb)` (cb gets
// {view, legal, seat}) and `cdfy.sendAction(kind, data)`; we render into
// #cdfy-root and only ever *request* actions — the host authority validates
// turn/seat and legality. The plugin enumerates every legal play in `legal`,
// so this UI never computes rules: it matches the player's selection against it.
//
// View shape: src/convert.rs. Zones: 0..N hands (owner=N), 100 river,
// 101 trushes, 102 excluded, 200 meta (one card whose attrs carry the flags).
// Card attr values are externally tagged: {"Int":n}/{"Str":s}/{"Bool":b}/{"List":[..]}.
(function () {
  var RIVER = 100, META = 200;

  var VIEW = null, LEGAL = [], SEAT = null, PHASE = "";
  var selected = []; // card ids currently picked from the own hand

  // ---- one-time styles (DESIGN.md: mono, hairlines, pastel blocks, pills) ----
  var style = document.createElement("style");
  style.textContent =
    ":root{--ink:#000;--canvas:#fff;--hair:#e6e6e6;--soft:#f7f7f5;" +
    "--lime:#dceeb1;--lilac:#c5b0f4;--pink:#efd4d4;--cream:#f4ecd6;--mint:#c8e6cd;--mag:#ff3d8b}" +
    "*{box-sizing:border-box}" +
    "body{margin:0;font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;color:var(--ink);background:var(--canvas)}" +
    "#cdfy-root{padding:16px;display:flex;flex-direction:column;gap:12px;max-width:760px;margin:0 auto}" +
    ".eyebrow{font-size:11px;letter-spacing:.12em;text-transform:uppercase;opacity:.55}" +
    ".hair{height:1px;background:var(--hair);border:0;margin:2px 0}" +
    ".row{display:flex;align-items:center;gap:8px;flex-wrap:wrap}" +
    ".opp{display:flex;flex-direction:column;gap:2px;border:2px solid transparent;border-radius:8px;padding:6px 10px;background:var(--soft)}" +
    ".opp.turn{border-color:var(--ink)}" +
    ".opp .n{font-size:13px}.opp .c{font-size:12px;opacity:.7}" +
    ".badge{font-size:11px;padding:1px 6px;border-radius:50px;background:var(--lilac)}" +
    ".block{border-radius:24px;padding:14px 16px;background:var(--cream)}" +
    ".flags{display:flex;gap:6px;flex-wrap:wrap}" +
    ".flag{font-size:11px;padding:2px 8px;border-radius:50px;background:var(--mint)}" +
    ".flag.rev{background:var(--mag);color:#fff}" +
    ".chip{display:inline-flex;align-items:center;justify-content:center;min-width:34px;height:46px;" +
    "padding:0 6px;border-radius:6px;border:2px solid var(--hair);background:#fff;font-size:15px;cursor:default;user-select:none}" +
    ".chip.red{color:#c8102e}" +
    ".hand .chip{cursor:pointer}" +
    ".hand .chip.sel{border-color:var(--ink);transform:translateY(-6px)}" +
    ".hand.off .chip{cursor:default;opacity:.75}" +
    ".btn{font-family:inherit;font-size:14px;font-weight:500;border-radius:50px;border:1px solid transparent;padding:9px 18px;cursor:pointer}" +
    ".btn:disabled{opacity:.4;cursor:not-allowed}" +
    ".btn.primary{background:var(--ink);color:#fff}" +
    ".btn.ghost{background:#fff;color:var(--ink);border-color:var(--ink)}" +
    ".muted{opacity:.6;font-size:13px}" +
    ".standings{display:flex;flex-direction:column;gap:4px}" +
    ".standings .r{display:flex;gap:10px;font-size:14px}";
  document.head.appendChild(style);

  var root = document.getElementById("cdfy-root");

  // ---- helpers ----
  function zone(id) { return (VIEW.zones || []).find(function (z) { return z.id === id; }); }
  function metaAttrs() { var z = zone(META); return z && z.cards[0] ? z.cards[0].attrs : {}; }
  function mInt(a, k) { var v = a[k]; return v && "Int" in v ? v.Int : null; }
  function mBool(a, k) { var v = a[k]; return !!(v && v.Bool); }
  function mList(a, k) { var v = a[k]; return v && "List" in v ? v.List : []; }
  function tagStr(v) { return v && "Str" in v ? v.Str : null; }
  function tagInt(v) { return v && "Int" in v ? v.Int : null; }

  var SUIT_SYM = { 1: "♠", 2: "♦", 3: "♥", 4: "♣" }; // ♠♦♥♣ by convert.rs index
  var SUIT_RED = { 2: true, 3: true };
  var LETTER_SYM = { s: "♠", d: "♦", h: "♥", c: "♣" };
  function rank(n) { return ({ 1: "A", 10: "T", 11: "J", 12: "Q", 13: "K" })[n] || String(n); }

  // Display a card from its proto (suit_index*100 + n; 0 or >=9000 = joker).
  function cardFace(proto) {
    if (proto === 0 || proto >= 9000) return { text: "🃏", red: false }; // 🃏
    var si = Math.floor(proto / 100), n = proto % 100;
    return { text: rank(n) + (SUIT_SYM[si] || "?"), red: !!SUIT_RED[si] };
  }

  function ownHandZone() {
    if (SEAT === null) return null;
    return (VIEW.zones || []).find(function (z) { return z.owner === SEAT; });
  }
  function numcmp(a, b) { return a - b; }
  function sameSet(a, b) {
    if (a.length !== b.length) return false;
    var x = a.slice().sort(numcmp), y = b.slice().sort(numcmp);
    return x.every(function (v, i) { return v === y[i]; });
  }
  function legalOf(kind) { return LEGAL.filter(function (a) { return a.kind === kind; }); }
  function selectionMatches(kind) {
    return legalOf(kind).some(function (a) { return sameSet(a.data, selected); });
  }
  function myTurn() { return SEAT !== null && VIEW.active_player === SEAT; }

  function rankLabel(pos, n) {
    // Daifugo tiers. 富豪/貧民 only exist at 4+ players; a 3-player game is just
    // 大富豪 / 平民 / 大貧民.
    if (pos === 0) return "大富豪";              // top
    if (pos === n - 1) return "大貧民";          // bottom
    if (n >= 4 && pos === 1) return "富豪";      // 2nd
    if (n >= 4 && pos === n - 2) return "貧民";  // 2nd-from-bottom
    return "平民";                                 // middle
  }

  // ---- element builders ----
  function el(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text != null) e.textContent = text;
    return e;
  }
  function chip(proto, extraCls) {
    var f = cardFace(proto);
    var c = el("span", "chip" + (f.red ? " red" : "") + (extraCls ? " " + extraCls : ""), f.text);
    return c;
  }

  // ---- render ----
  function render() {
    root.textContent = "";
    if (!VIEW) { root.appendChild(el("p", "muted", "waiting…")); return; }

    var attrs = metaAttrs();
    var names = mList(attrs, "players").map(tagStr);
    var ranks = mList(attrs, "ranks").map(tagInt);
    var n = names.length || (VIEW.players && VIEW.players.length) || 0;

    // opponents (every seated player that is not us; a spectator sees them all)
    var oppRow = el("div", "row");
    (VIEW.zones || []).forEach(function (z) {
      if (z.owner == null || z.owner === SEAT) return;
      var box = el("div", "opp" + (VIEW.active_player === z.owner ? " turn" : ""));
      box.appendChild(el("div", "n", names[z.owner] || ("p" + z.owner)));
      box.appendChild(el("div", "c", "🂠 × " + z.cards.length)); // 🂠 × count
      var pos = ranks.indexOf(z.owner);
      if (pos >= 0) box.appendChild(el("div", "badge", rankLabel(pos, n)));
      oppRow.appendChild(box);
    });
    if (oppRow.children.length) {
      root.appendChild(el("p", "eyebrow", "opponents"));
      root.appendChild(oppRow);
      root.appendChild(el("hr", "hair"));
    }

    // river (the last served group is the play to beat) + flags
    var river = zone(RIVER);
    var groups = mList(attrs, "river_groups").map(tagInt);
    var block = el("div", "block");
    var rlab = el("p", "eyebrow", "場 river");
    block.appendChild(rlab);
    var rrow = el("div", "row");
    if (river && river.cards.length && groups.length) {
      var take = groups[groups.length - 1];
      var start = Math.max(0, river.cards.length - take);
      river.cards.slice(start).forEach(function (c) { rrow.appendChild(chip(c.proto)); });
    } else {
      rrow.appendChild(el("span", "muted", "新しい場 — 自由に出せます")); // fresh field
    }
    block.appendChild(rrow);
    var flags = el("div", "flags");
    if (mBool(attrs, "revoluted")) flags.appendChild(el("span", "flag rev", "革命")); // 革命
    var sl = mList(attrs, "suit_limits").map(tagStr).filter(Boolean);
    if (sl.length) flags.appendChild(el("span", "flag", "縛り " + sl.map(function (s) { return LETTER_SYM[s] || s; }).join(""))); // 縛り
    if (mBool(attrs, "is_step")) flags.appendChild(el("span", "flag", "階段")); // 階段
    var rsize = mInt(attrs, "river_size");
    if (rsize != null && rsize > 0) flags.appendChild(el("span", "flag", rsize + "枚出し")); // n枚出し
    if (flags.children.length) block.appendChild(flags);
    root.appendChild(block);
    root.appendChild(el("hr", "hair"));

    // ended: standings, no actions
    if (PHASE === "ended") {
      root.appendChild(el("p", "eyebrow", "結果")); // 結果
      var st = el("div", "standings");
      ranks.forEach(function (owner, i) {
        var r = el("div", "r");
        r.appendChild(el("span", "badge", rankLabel(i, n)));
        r.appendChild(el("span", null, names[owner] || ("p" + owner)));
        st.appendChild(r);
      });
      root.appendChild(st);
      return;
    }

    // own hand (spectators have none)
    var hand = ownHandZone();
    if (!hand) { root.appendChild(el("p", "muted", "観戦中 — 手札はありません")); return; } // spectating
    root.appendChild(el("p", "eyebrow", "your hand" + (myTurn() ? " — あなたの番" : "")));
    var interactive = myTurn() && (PHASE === "serve" || PHASE.indexOf("select") === 0);
    var handRow = el("div", "hand row" + (interactive ? "" : " off"));
    hand.cards.slice().sort(function (a, b) { return a.proto - b.proto; }).forEach(function (c) {
      var isSel = selected.indexOf(c.id) >= 0;
      var ch = chip(c.proto, isSel ? "sel" : null);
      if (interactive) ch.onclick = function () { toggle(c.id); };
      handRow.appendChild(ch);
    });
    root.appendChild(handRow);

    // action bar
    root.appendChild(actionBar());
  }

  function actionBar() {
    var bar = el("div", "row");
    if (!myTurn()) { bar.appendChild(el("span", "muted", "相手の番です")); return bar; } // opponent's turn

    if (PHASE === "serve") {
      var serve = el("button", "btn primary", "serve");
      serve.disabled = !selectionMatches("serve");
      serve.onclick = function () { send("serve", selected.slice()); };
      bar.appendChild(serve);
      if (legalOf("pass").length) {
        var pass = el("button", "btn ghost", "pass");
        pass.onclick = function () { send("pass", []); };
        bar.appendChild(pass);
      }
      return bar;
    }

    if (PHASE.indexOf("select") === 0) { // select4 / select7 / select13
      bar.appendChild(el("span", "muted", "カードを選んで確定")); // pick cards then confirm
      var ok = el("button", "btn primary", "確定"); // 確定
      ok.disabled = !selectionMatches("select");
      ok.onclick = function () { send("select", selected.slice()); };
      bar.appendChild(ok);
      return bar;
    }

    if (PHASE === "one_chance") {
      legalOf("one_chance").forEach(function (a) {
        if (a.data.length === 0) {
          var skip = el("button", "btn ghost", "スキップ"); // skip
          skip.onclick = function () { send("one_chance", []); };
          bar.appendChild(skip);
        } else {
          var f = cardFace(protoOfId(a.data[0]));
          var b = el("button", "btn primary", "宣言: " + f.text); // declare: <card>
          b.onclick = function () { send("one_chance", a.data.slice()); };
          bar.appendChild(b);
        }
      });
      return bar;
    }
    return bar;
  }

  // proto of a card id, searched across all zones (for one_chance's ace label)
  function protoOfId(id) {
    var found = 0;
    (VIEW.zones || []).forEach(function (z) {
      z.cards.forEach(function (c) { if (c.id === id) found = c.proto; });
    });
    return found;
  }

  function toggle(id) {
    var i = selected.indexOf(id);
    if (i >= 0) selected.splice(i, 1); else selected.push(id);
    render();
  }
  function send(kind, data) {
    selected = [];
    cdfy.sendAction(kind, data);
  }

  cdfy.onView(function (state) {
    VIEW = state.view;
    LEGAL = state.legal || [];
    SEAT = state.seat;
    PHASE = VIEW ? VIEW.phase : "";
    // Drop any selection that is no longer in the own hand (e.g. after a play),
    // and clear it entirely when the hand isn't pickable (off-turn, one_chance,
    // ended, spectator) so stale highlights never linger.
    var hand = VIEW ? ownHandZone() : null;
    var ids = hand ? hand.cards.map(function (c) { return c.id; }) : [];
    var canPick = myTurn() && (PHASE === "serve" || PHASE.indexOf("select") === 0);
    selected = canPick ? selected.filter(function (id) { return ids.indexOf(id) >= 0; }) : [];
    render();
  });
})();
