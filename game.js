const API =
  window.location.hostname === 'localhost' ||
  window.location.hostname === '127.0.0.1'
    ? 'http://localhost:8000/api'
    : '/api'

// ── セッション状態 ─────────────────────────────────────────────
let gameId = null
let playerIdx = null
let gameState = null
let selectedCards = new Set()
let selectedCount = 4
let selectedDifficulty = 'normal'
let selectedRules = {
  eight_stop: true,
  revolution: true,
  five_skip: false,
  j_back: false,
  suit_lock: false,
  ten_discard: false,
  tochaku: false,
  stairs: false,
  tribute: false,
  spade3_return: false,
}
let pollTimer = null
let lobbyTimer = null
let fetchInProgress = false
let currentMode = 'standard'
let pickupSelected = new Set()

// ══════════════════════════════════════════════════════════════
//  初期化
// ══════════════════════════════════════════════════════════════
window.addEventListener('load', () => {
  const saved = sessionStorage.getItem('daifugo')
  if (saved) {
    try {
      const { gid, pidx } = JSON.parse(saved)
      gameId = gid
      playerIdx = pidx
      fetchAndRoute()
      return
    } catch (_) {}
  }
  showScreenStart()
})

document.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    if (!document.getElementById('screen-create').classList.contains('hidden'))
      createGame()
    if (!document.getElementById('screen-join').classList.contains('hidden'))
      joinGame()
  }
})

// プレイ・パス以外のすべてのボタンに UI クリック音を付与
const NO_UI_SOUND_BTNS = new Set(['play-btn', 'pass-btn'])
document.addEventListener('click', (e) => {
  const btn = e.target.closest('button')
  if (btn && !btn.disabled && !NO_UI_SOUND_BTNS.has(btn.id)) {
    SoundEngine.uiClick()
  }
})

// ══════════════════════════════════════════════════════════════
//  画面切り替え
// ══════════════════════════════════════════════════════════════
function hideAll() {
  ;[
    'screen-start',
    'screen-create',
    'screen-join',
    'screen-lobby',
    'game-board',
  ].forEach((id) => document.getElementById(id).classList.add('hidden'))
}

function showScreenStart() {
  hideAll()
  document.getElementById('screen-start').classList.remove('hidden')
}
function showScreenCreate() {
  hideAll()
  const isZen = currentMode === 'zen'
  document.getElementById('create-screen-title').textContent = isZen
    ? '全役職大富豪 — 新規作成'
    : '新しいゲームを作成'
  document
    .getElementById('standard-rules-section')
    .classList.toggle('hidden', isZen)
  document
    .getElementById('zen-rules-section')
    .classList.toggle('hidden', !isZen)
  document.getElementById('screen-create').classList.remove('hidden')
}
function showScreenJoin() {
  hideAll()
  document.getElementById('screen-join').classList.remove('hidden')
}

function selectMode(mode) {
  currentMode = mode
  document.querySelectorAll('.mode-card').forEach((btn) => {
    btn.classList.toggle('active', btn.id === `mode-card-${mode}`)
  })
}

function showScreenLobby(state) {
  hideAll()
  document.getElementById('screen-lobby').classList.remove('hidden')
  renderLobby(state)
  startLobbyPoll()
}

function showBoard() {
  hideAll()
  document.getElementById('game-board').classList.remove('hidden')
}

// ══════════════════════════════════════════════════════════════
//  プレイヤー数セレクター
// ══════════════════════════════════════════════════════════════
function selectCount(n) {
  selectedCount = n
  document.querySelectorAll('.count-btn').forEach((btn) => {
    btn.classList.toggle('active', parseInt(btn.dataset.n) === n)
  })
}

function selectDifficulty(d) {
  selectedDifficulty = d
  document.querySelectorAll('.diff-btn').forEach((btn) => {
    btn.classList.toggle('active', btn.dataset.d === d)
  })
}

function toggleRule(name) {
  selectedRules[name] = !selectedRules[name]
  document.querySelectorAll('.rule-btn').forEach((btn) => {
    if (btn.dataset.rule === name) {
      btn.classList.toggle('active', selectedRules[name])
    }
  })
}

// ══════════════════════════════════════════════════════════════
//  ゲーム作成 / 参加 / 開始
// ══════════════════════════════════════════════════════════════
async function createGame() {
  const btn = document.getElementById('create-btn')
  btn.disabled = true
  btn.textContent = '作成中…'

  const name =
    document.getElementById('create-name').value.trim() || 'プレイヤー1'
  try {
    const res = await fetch(`${API}/game/new`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        name,
        num_players: selectedCount,
        cpu_difficulty: selectedDifficulty,
        rules: selectedRules,
        mode: currentMode,
      }),
    })
    const data = await res.json()
    if (!res.ok) {
      alert(data.error)
      return
    }

    gameId = data.game_id
    playerIdx = data.player_idx
    saveSession()
    showScreenLobby(data.state)
  } catch (_) {
    alert('バックエンドに接続できません (http://localhost:8000)')
  } finally {
    btn.disabled = false
    btn.textContent = '作成してロビーへ'
  }
}

async function joinGame() {
  const btn = document.getElementById('join-btn')
  btn.disabled = true
  btn.textContent = '参加中…'

  const name = document.getElementById('join-name').value.trim() || 'プレイヤー'
  const code = document.getElementById('join-code').value.trim().toLowerCase()
  if (!code) {
    alert('ゲームコードを入力してください')
    btn.disabled = false
    btn.textContent = '参加する'
    return
  }

  try {
    const res = await fetch(`${API}/game/${code}/join`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name }),
    })
    const data = await res.json()
    if (!res.ok) {
      alert(data.error)
      return
    }

    gameId = data.game_id
    playerIdx = data.player_idx
    saveSession()

    if (data.state.state === 'waiting') {
      showScreenLobby(data.state)
    } else {
      showBoard()
      render(data.state)
    }
  } catch (_) {
    alert('バックエンドに接続できません (http://localhost:8000)')
  } finally {
    btn.disabled = false
    btn.textContent = '参加する'
  }
}

async function startGame() {
  try {
    const res = await fetch(`${API}/game/${gameId}/start`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ player_idx: playerIdx }),
    })
    const data = await res.json()
    if (!res.ok) {
      alert(data.error)
      return
    }

    stopLobbyPoll()
    showBoard()

    const initialState = data.initial_state ?? data.state
    gameState = initialState
    renderAll(initialState)

    const cpuReplay = data.cpu_replay ?? []
    if (cpuReplay.length > 0) {
      await delay(700)
      await replayEvents(cpuReplay, initialState)
    }
    render(data.state)
  } catch (_) {}
}

// ══════════════════════════════════════════════════════════════
//  ロビー
// ══════════════════════════════════════════════════════════════
function renderLobby(state) {
  const codeEl = document.getElementById('lobby-code')
  codeEl.textContent = state.game_id

  const slots = document.getElementById('lobby-slots')
  const total = state.num_players
  const joined = state.joined_count

  slots.innerHTML = Array.from({ length: total }, (_, i) => {
    const p = state.players[i]
    const filled = !!p
    return `
      <div class="lobby-slot ${filled ? 'filled' : ''}">
        <div class="slot-num">${i + 1}</div>
        <div class="slot-name">${filled ? esc(p.name) : '─── 待機中'}</div>
        <div class="slot-status">${filled ? '✓ 参加済み' : '空席'}</div>
      </div>`
  }).join('')
}

async function copyGameCode() {
  if (!gameId) return
  try {
    await navigator.clipboard.writeText(gameId)
    const el = document.getElementById('lobby-code')
    const hint = document.getElementById('lobby-copy-hint')
    el.classList.add('copied')
    const prev = hint.innerHTML
    hint.innerHTML = '✅ コードをコピーしました！'
    setTimeout(() => {
      el.classList.remove('copied')
      hint.innerHTML = prev
    }, 2000)
  } catch (_) {
    alert('コードをコピーできませんでした: ' + gameId)
  }
}

function startLobbyPoll() {
  if (lobbyTimer) return
  lobbyTimer = setInterval(async () => {
    if (!gameId) {
      stopLobbyPoll()
      return
    }
    try {
      const res = await fetch(
        `${API}/game/${gameId}/state?player_idx=${playerIdx}`,
      )
      const state = await res.json()
      if (state.state === 'playing' || state.state === 'finished') {
        stopLobbyPoll()
        showBoard()
        render(state)
      } else if (state.state === 'waiting') {
        renderLobby(state)
      }
    } catch (_) {}
  }, 2000)
}

function stopLobbyPoll() {
  if (lobbyTimer) {
    clearInterval(lobbyTimer)
    lobbyTimer = null
  }
}

// ══════════════════════════════════════════════════════════════
//  状態取得 / ルーティング
// ══════════════════════════════════════════════════════════════
async function fetchAndRoute() {
  if (!gameId) return
  try {
    const res = await fetch(
      `${API}/game/${gameId}/state?player_idx=${playerIdx}`,
    )
    if (!res.ok) {
      resetGame()
      return
    }
    const state = await res.json()
    if (state.state === 'waiting') {
      showScreenLobby(state)
    } else {
      showBoard()
      render(state)
    }
  } catch (_) {
    resetGame()
  }
}

async function fetchState() {
  if (!gameId || fetchInProgress) return
  fetchInProgress = true
  try {
    const res = await fetch(
      `${API}/game/${gameId}/state?player_idx=${playerIdx}`,
    )
    if (!res.ok) return
    const newState = await res.json()
    const effects = detectNewEffects(gameState, newState)
    await maybeAnimateOpponent(newState)
    if (effects.has('eight_stop')) {
      showEffectBanner('8切り！', '#f39c12')
      await delay(500)
      SoundEngine.eightStop()
      await delay(280)
      effects.delete('eight_stop')
    }
    if (effects.has('revolution')) showEffectBanner('🔄 革命！', '#e74c3c')
    playEffectSounds(effects)
    render(newState)
  } catch (_) {
  } finally {
    fetchInProgress = false
  }
}

// ══════════════════════════════════════════════════════════════
//  ゲームアクション（プレイ / パス）
// ══════════════════════════════════════════════════════════════
async function playCards() {
  if (selectedCards.size === 0) return
  stopPolling()

  // アニメーション用に現在の位置を事前キャプチャ（await前に同期で取得）
  const myPlayer = gameState.players.find((p) => p.idx === playerIdx)
  const indices = [...selectedCards].sort((a, b) => a - b)
  const cardData = indices.map((i) => myPlayer.hand[i])

  const handEl = document.getElementById('player-hand')
  const fromRects = indices
    .map((i) => {
      const el = handEl.querySelector(`[data-idx="${i}"]`)
      return el ? el.getBoundingClientRect() : null
    })
    .filter(Boolean)

  const tableRect = document
    .getElementById('table-area')
    .getBoundingClientRect()

  // API リクエスト
  const res = await fetch(`${API}/game/${gameId}/play`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ player_idx: playerIdx, card_indices: indices }),
  })
  const data = await res.json()

  if (!res.ok) {
    showError(data.error)
    return
  }

  // 自分のカードアニメーション（投げる音 + 飛翔）
  SoundEngine.play()
  if (fromRects.length > 0) {
    await animateCardsToTable(fromRects, cardData, tableRect)
  }

  // 人間プレイ直後の状態（CPU ターン前）でエフェクトを判定
  const afterHuman = data.after_human ?? data
  const humanEffects = detectNewEffects(gameState, afterHuman)

  // 8切り / 10捨て: カード着地 → バナー → 効果音
  if (humanEffects.has('eight_stop')) {
    showCardsOnTableTemp(cardData)
    showEffectBanner('8切り！', '#f39c12')
    await delay(500)
    SoundEngine.eightStop()
    await delay(280)
    humanEffects.delete('eight_stop')
  } else if (humanEffects.has('ten_discard_play')) {
    showCardsOnTableTemp(cardData)
    showEffectBanner('10捨て！', '#e67e22')
    await delay(500)
    SoundEngine.uiClick()
    await delay(280)
    humanEffects.delete('ten_discard_play')
  }
  if (humanEffects.has('revolution')) showEffectBanner('🔄 革命！', '#e74c3c')
  showNewEffectBanners(humanEffects, afterHuman)
  playEffectSounds(humanEffects)

  // after_human 状態に更新して手番表示を切り替える
  gameState = afterHuman
  renderAll(afterHuman)

  // CPU 全ターンを順番にアニメーション
  const cpuReplay = data.cpu_replay ?? []
  if (cpuReplay.length > 0) {
    await delay(700)
    await replayEvents(cpuReplay, afterHuman)
  }

  selectedCards.clear()
  render(data)
}

async function passTurn() {
  stopPolling()
  SoundEngine.pass()
  showEffectBanner('パス', '#7f8c8d')
  const res = await fetch(`${API}/game/${gameId}/pass`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ player_idx: playerIdx }),
  })
  const data = await res.json()
  if (!res.ok) {
    showError(data.error)
    return
  }

  const cpuReplay = data.cpu_replay ?? []
  if (cpuReplay.length > 0) {
    await delay(400)
    await replayEvents(cpuReplay, gameState)
  }
  render(data)
}

async function discardCard() {
  if (selectedCards.size !== 1) return
  stopPolling()
  const cardIdx = [...selectedCards][0]
  const res = await fetch(`${API}/game/${gameId}/discard`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ player_idx: playerIdx, card_idx: cardIdx }),
  })
  const data = await res.json()
  if (!res.ok) {
    showError(data.error)
    return
  }
  const cpuReplay = data.cpu_replay ?? []
  if (cpuReplay.length > 0) {
    await delay(400)
    await replayEvents(cpuReplay, gameState)
  }
  selectedCards.clear()
  render(data)
}

async function discardSkip() {
  stopPolling()
  const res = await fetch(`${API}/game/${gameId}/discard`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ player_idx: playerIdx, skip: true }),
  })
  const data = await res.json()
  if (!res.ok) {
    showError(data.error)
    return
  }
  const cpuReplay = data.cpu_replay ?? []
  if (cpuReplay.length > 0) {
    await delay(400)
    await replayEvents(cpuReplay, gameState)
  }
  selectedCards.clear()
  render(data)
}

function togglePickupCard(cardIdx) {
  if (
    !gameState?.pending_pickup ||
    gameState.pending_pickup.player_idx !== playerIdx
  )
    return
  const pp = gameState.pending_pickup
  const pile =
    pp.source === 'discard'
      ? (gameState.discard_pile ?? [])
      : (gameState.removed_cards ?? [])
  const required = Math.min(pp.count, pile.length)

  if (pickupSelected.has(cardIdx)) {
    pickupSelected.delete(cardIdx)
  } else if (pickupSelected.size < required) {
    pickupSelected.add(cardIdx)
  }

  // Re-render pickup pile and button state
  renderPickupArea(gameState)
}

async function confirmPickup() {
  if (
    !gameState?.pending_pickup ||
    gameState.pending_pickup.player_idx !== playerIdx
  )
    return
  if (pickupSelected.size === 0) return
  stopPolling()

  const pp = gameState.pending_pickup
  const pileEl = document.getElementById(
    pp.source === 'discard' ? 'zen-discard-pile' : 'zen-removed-pile',
  )
  const handEl = document.getElementById('player-hand')
  if (pileEl && handEl)
    await animateCardBetweenElements(pileEl, handEl, pickupSelected.size, true)

  const indices = [...pickupSelected].sort((a, b) => a - b)
  pickupSelected.clear()

  const res = await fetch(`${API}/game/${gameId}/pickup`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ player_idx: playerIdx, card_indices: indices }),
  })
  const data = await res.json()
  if (!res.ok) {
    showError(data.error)
    startPolling()
    return
  }
  const cpuReplay = data.cpu_replay ?? []
  if (cpuReplay.length > 0) {
    await delay(400)
    await replayEvents(cpuReplay, gameState)
  }
  render(data)
}

async function giveCards() {
  const pg = gameState?.pending_give
  if (!pg || pg.player_idx !== playerIdx) return
  if (selectedCards.size !== pg.count) return
  stopPolling()
  const indices = [...selectedCards].sort((a, b) => a - b)

  // Animate: player hand → target opponent element
  const targetEl = document.querySelector(
    `[data-player-idx="${pg.target_idx}"]`,
  )
  const handEl = document.getElementById('player-hand')
  if (handEl && targetEl)
    await animateCardBetweenElements(handEl, targetEl, pg.count, true)

  const res = await fetch(`${API}/game/${gameId}/give`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ player_idx: playerIdx, card_indices: indices }),
  })
  const data = await res.json()
  if (!res.ok) {
    showError(data.error)
    return
  }
  const cpuReplay = data.cpu_replay ?? []
  if (cpuReplay.length > 0) {
    await delay(400)
    await replayEvents(cpuReplay, gameState)
  }
  selectedCards.clear()
  render(data)
}

async function activateOneChance() {
  stopPolling()
  const res = await fetch(`${API}/game/${gameId}/one_chance`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ player_idx: playerIdx }),
  })
  const data = await res.json()
  if (!res.ok) {
    showError(data.error)
    return
  }

  if (data.one_chance_result === 'success') {
    showEffectBanner('ワンチャンス 成功！', '#e74c3c')
    SoundEngine.revolution()
  } else {
    showEffectBanner('ワンチャンス 失敗…', '#7f8c8d')
    SoundEngine.pass()
  }

  const cpuReplay = data.cpu_replay ?? []
  if (cpuReplay.length > 0) {
    await delay(600)
    await replayEvents(cpuReplay, gameState)
  }
  render(data)
}

// ══════════════════════════════════════════════════════════════
//  カードアニメーション
// ══════════════════════════════════════════════════════════════
// ── 相手プレイのアニメーション ─────────────────────────────────
// 新旧状態を比較して、相手がカードを出していれば相手エリアから場へ飛ばす
async function maybeAnimateOpponent(newState) {
  const newTable = newState.table ?? []
  if (newTable.length === 0) return
  if (newState.table_owner === null || newState.table_owner === undefined)
    return
  if (newState.table_owner === playerIdx) return

  // 前の状態と同じ場なら既にアニメーション済み（再描画時のスキップ）
  const prevTable = gameState?.table ?? []
  if (JSON.stringify(newTable) === JSON.stringify(prevTable)) return

  const ownerEl = document.querySelector(
    `[data-player-idx="${newState.table_owner}"]`,
  )
  if (!ownerEl) return

  const fromRect = ownerEl.getBoundingClientRect()
  const tableRect = document
    .getElementById('table-area')
    .getBoundingClientRect()
  const W = 52,
    H = 76
  const cx = fromRect.left + fromRect.width / 2
  const cy = fromRect.top + fromRect.height / 2

  const fromRects = Array.from({ length: newTable.length }, () => ({
    left: cx - W / 2,
    top: cy - H / 2,
    width: W,
    height: H,
  }))

  await animateCardsToTable(fromRects, newTable, tableRect)
}

function animateCardsToTable(fromRects, cardData, tableRect) {
  return new Promise((resolve) => {
    const overlay = document.getElementById('anim-overlay')
    const n = fromRects.length
    const W = 52,
      H = 76
    const cx = tableRect.left + tableRect.width / 2
    const cy = tableRect.top + tableRect.height / 2

    // 元の位置にクローンを配置
    const clones = fromRects.map((rect, i) => {
      const card = cardData[i] || cardData[0]
      const el = document.createElement('div')
      el.className = `card anim-card ${card.color || 'black'}`
      const [rank, suit] = splitDisplay(card.display)
      el.innerHTML = `<span class="card-rank">${rank}</span><span class="card-suit">${suit}</span>`
      el.style.left = `${rect.left}px`
      el.style.top = `${rect.top}px`
      el.style.width = `${rect.width}px`
      el.style.height = `${rect.height}px`
      overlay.appendChild(el)
      return el
    })

    // リフロー強制（transitionが効くよう）
    overlay.getBoundingClientRect()

    // 場の中央へ移動（複数枚は横に並べる）
    const gap = Math.min(W + 6, (tableRect.width * 0.8) / Math.max(n, 1))
    clones.forEach((el, i) => {
      const offsetX = n > 1 ? (i - (n - 1) / 2) * gap : 0
      el.style.transition =
        'left 0.55s cubic-bezier(0.25,0.8,0.25,1), top 0.55s cubic-bezier(0.25,0.8,0.25,1), transform 0.55s ease, box-shadow 0.55s'
      el.style.left = `${cx - W / 2 + offsetX}px`
      el.style.top = `${cy - H / 2}px`
      el.style.transform = 'scale(1.15)'
      el.style.boxShadow = '0 10px 28px rgba(0,0,0,0.55)'
    })

    setTimeout(() => {
      clones.forEach((el) => el.remove())
      resolve()
    }, 580)
  })
}

// カードを要素間でアニメーション (fromEl → toEl), face-down card
function animateCardBetweenElements(fromEl, toEl, count = 1, faceDown = true) {
  return new Promise((resolve) => {
    if (!fromEl || !toEl) {
      resolve()
      return
    }
    const overlay = document.getElementById('anim-overlay')
    const fromRect = fromEl.getBoundingClientRect()
    const toRect = toEl.getBoundingClientRect()
    const W = 36,
      H = 52
    const fx = fromRect.left + fromRect.width / 2 - W / 2
    const fy = fromRect.top + fromRect.height / 2 - H / 2
    const tx = toRect.left + toRect.width / 2 - W / 2
    const ty = toRect.top + toRect.height / 2 - H / 2

    const clones = Array.from({ length: count }, (_, i) => {
      const el = document.createElement('div')
      el.className = faceDown ? 'card-back anim-card' : 'card anim-card black'
      el.style.cssText = `position:fixed;width:${W}px;height:${H}px;left:${fx + i * 3}px;top:${fy + i * 3}px;border-radius:4px;z-index:301;transition:none;`
      overlay.appendChild(el)
      return el
    })

    overlay.getBoundingClientRect()

    clones.forEach((el, i) => {
      el.style.transition =
        'left 0.45s cubic-bezier(0.25,0.8,0.25,1), top 0.45s cubic-bezier(0.25,0.8,0.25,1), transform 0.45s'
      el.style.left = `${tx}px`
      el.style.top = `${ty}px`
      el.style.transform = 'scale(1.1)'
    })

    setTimeout(() => {
      clones.forEach((el) => el.remove())
      resolve()
    }, 480)
  })
}

// ══════════════════════════════════════════════════════════════
//  ポーリング
// ══════════════════════════════════════════════════════════════
function startPolling() {
  if (pollTimer) return
  pollTimer = setInterval(fetchState, 1000)
}

function stopPolling() {
  if (pollTimer) {
    clearInterval(pollTimer)
    pollTimer = null
  }
}

// ══════════════════════════════════════════════════════════════
//  描画
// ══════════════════════════════════════════════════════════════
function render(state) {
  gameState = state
  stopPolling()

  if (state.state === 'finished') {
    renderAll(state)
    setTimeout(() => showEndModal(state), 500)
    return
  }

  renderAll(state)

  const myTurn = state.current_player_idx === playerIdx
  document.getElementById('my-turn-badge').classList.toggle('hidden', !myTurn)

  if (!myTurn) startPolling()
}

function renderAll(state) {
  renderOpponents(state)
  renderTable(state)
  renderPlayerHand(state)
  renderButtons(state)
  renderLog(state)
  renderBadges(state)
  renderPickupArea(state)
  renderGiveArea(state)
  renderZenPiles(state)
}

function renderZenPiles(state) {
  const pilesEl = document.getElementById('zen-piles')
  if (!pilesEl) return

  const isZen = state.mode === 'zen'
  pilesEl.classList.toggle('hidden', !isZen)
  if (!isZen) return

  const discardCount = (state.discard_pile ?? []).length
  const removedCount = (state.removed_cards ?? []).length

  document.getElementById('zen-discard-count').textContent = discardCount
  document.getElementById('zen-removed-count').textContent = removedCount

  const pp = state.pending_pickup
  const discardActive = pp?.player_idx === playerIdx && pp?.source === 'discard'
  const removedActive = pp?.player_idx === playerIdx && pp?.source === 'removed'

  const discardPileEl = document.getElementById('zen-discard-pile')
  const removedPileEl = document.getElementById('zen-removed-pile')
  discardPileEl.classList.toggle(
    'pile-active',
    discardActive && discardCount > 0,
  )
  removedPileEl.classList.toggle(
    'pile-active',
    removedActive && removedCount > 0,
  )

  // Show/hide the card-back image depending on whether pile has cards
  discardPileEl.querySelector('.zen-pile-card').style.visibility =
    discardCount > 0 ? 'visible' : 'hidden'
  removedPileEl.querySelector('.zen-pile-card').style.visibility =
    removedCount > 0 ? 'visible' : 'hidden'
}

function renderBadges(state) {
  const isZen = state.mode === 'zen'
  document.getElementById('zen-mode-badge').classList.toggle('hidden', !isZen)
  document.getElementById('card-effects-btn').classList.toggle('hidden', !isZen)

  document
    .getElementById('revolution-badge')
    .classList.toggle('hidden', !state.revolution)
  document
    .getElementById('j-back-badge')
    .classList.toggle('hidden', !state.j_back_active)

  const suitLockBadge = document.getElementById('suit-lock-badge')
  if (state.suit_lock_suit) {
    const syms = { SPADES: '♠', HEARTS: '♥', DIAMONDS: '♦', CLUBS: '♣' }
    const sym = syms[state.suit_lock_suit] || ''
    suitLockBadge.textContent = state.suit_lock_stairs
      ? `🔒 摩訶鉢特摩 ${sym}縛り＋階段`
      : state.suit_lock_strict
        ? `🔒 摩訶鉢特摩 ${sym}縛り`
        : `🔒 ${sym}縛り`
    suitLockBadge.classList.remove('hidden')
  } else {
    suitLockBadge.classList.add('hidden')
  }

  document
    .getElementById('six-strongest-badge')
    .classList.toggle('hidden', !state.six_strongest)
  document
    .getElementById('effect-nullified-badge')
    .classList.toggle('hidden', !state.effect_nullified)
  document
    .getElementById('ten-nullified-badge')
    .classList.toggle('hidden', !state.ten_nullified)
}

// 時計回りの座席配置マップ（自分の左隣が最初＝時計回り: 底→左→上→右）
const OPP_POSITIONS = {
  1: ['top'],
  2: ['top-left', 'top-right'],
  3: ['left', 'top', 'right'],
  4: ['left', 'top-left', 'top-right', 'right'],
  5: ['left', 'top-left', 'top', 'top-right', 'right'],
}

// 自分以外のプレイヤー（CPU / 他の人間）を時計回りにスロットへ配置
function renderOpponents(state) {
  document.querySelectorAll('.opp-slot').forEach((el) => {
    el.innerHTML = ''
  })

  const total = state.players.length
  const opponents = []
  for (let i = 1; i < total; i++) {
    const idx = (playerIdx + i) % total
    const p = state.players.find((pl) => pl.idx === idx)
    if (p) opponents.push(p)
  }

  const positions = OPP_POSITIONS[opponents.length] || []

  opponents.forEach((p, i) => {
    const posName = positions[i]
    if (!posName) return
    const slot = document.getElementById(`opp-${posName}`)
    if (!slot) return

    const div = document.createElement('div')
    div.className =
      'cpu-player' +
      (p.is_current ? ' is-current' : '') +
      (p.is_finished ? ' is-finished' : '')
    div.dataset.playerIdx = p.idx

    const rankHtml = p.rank_name
      ? `<div class="cpu-rank-badge">${esc(p.rank_name)}</div>`
      : ''

    const maxBacks = state.players.length <= 2 ? 7 : 13
    const backs = Array.from(
      { length: Math.min(p.hand_count, maxBacks) },
      () => `<div class="card-back"></div>`,
    ).join('')

    div.innerHTML = `
      <div class="cpu-name">${esc(p.name)}${p.is_current ? ' ◀' : ''}${p.is_cpu ? '' : ' 👤'}</div>
      ${rankHtml}
      <div class="cpu-cards">${backs}</div>
      <div style="font-size:0.72rem;color:#9fcf9f;margin-top:3px">${p.hand_count}枚</div>`
    slot.appendChild(div)
  })
}

function renderTable(state) {
  const cardsEl = document.getElementById('table-cards')
  const hint = document.getElementById('table-hint')

  if (!state.table || state.table.length === 0) {
    cardsEl.innerHTML =
      '<span style="color:#5a8a5a;font-size:0.88rem">（空）</span>'
    hint.style.color = ''
    hint.textContent = '最初の1枚（または複数枚）を出してください'
    return
  }

  cardsEl.innerHTML = state.table.map((c) => cardHtml(c, false, true)).join('')
  const maxVal = comboValue(state.table)
  hint.style.color = ''
  const tableCount =
    state.nine_table_count > 0 ? state.nine_table_count : state.table.length
  let hintText = `${tableCount}枚出し ／ 強さ: ${valLabel(maxVal)}`
  if (state.nine_table_count > 0) hintText += '（阿修羅: 3枚縛り）'
  if (state.table_type === 'sequence') hintText += '（階段）'
  if (state.revolution && !state.j_back_active) hintText += '（革命中）'
  if (state.j_back_active) hintText += '（Jバック中）'
  hint.textContent = hintText
}

function renderPlayerHand(state) {
  const me = state.players.find((p) => p.idx === playerIdx)
  if (!me) return

  document.getElementById('my-name').textContent =
    esc(me.name) + (me.rank_name ? `（${me.rank_name}）` : '')

  const handEl = document.getElementById('player-hand')
  if (!me.hand || me.hand.length === 0) {
    handEl.innerHTML =
      '<span style="color:#9fcf9f;font-size:0.88rem">手札なし</span>'
    return
  }

  handEl.innerHTML = me.hand
    .map((c, i) => cardHtml(c, selectedCards.has(i), false, i))
    .join('')
  handEl.querySelectorAll('.card[data-idx]').forEach((el) => {
    el.addEventListener('click', () => toggleCard(parseInt(el.dataset.idx, 10)))
  })
}

function renderButtons(state) {
  const me = state.players.find((p) => p.idx === playerIdx)
  const myTurn =
    state.current_player_idx === playerIdx && state.state === 'playing'
  const isFinished = me?.is_finished ?? false
  const pendingDiscard =
    state.pending_ten_discard && state.pending_ten_discard_player === playerIdx
  const pendingPickup = state.pending_pickup?.player_idx === playerIdx
  const pendingGive = state.pending_give?.player_idx === playerIdx
  const anyPending = pendingDiscard || pendingPickup || pendingGive

  document.getElementById('play-btn').disabled =
    !myTurn || isFinished || selectedCards.size === 0 || anyPending
  document.getElementById('pass-btn').disabled =
    !myTurn ||
    isFinished ||
    !state.table ||
    state.table.length === 0 ||
    anyPending

  const discardArea = document.getElementById('discard-area')
  const discardBtn = document.getElementById('discard-btn')
  if (pendingDiscard) {
    discardArea.classList.remove('hidden')
    discardBtn.disabled = selectedCards.size !== 1
  } else {
    discardArea.classList.add('hidden')
  }

  const giveBtn = document.getElementById('give-btn')
  giveBtn.disabled =
    !pendingGive || selectedCards.size !== (state.pending_give?.count ?? -1)

  const oneChanceBtn = document.getElementById('one-chance-btn')
  const hasAce = me?.hand?.some((c) => !c.is_joker && c.rank === 'A') ?? false
  const aceEffectActive = !state.effect_nullified && !state.ten_nullified
  const canOneChance =
    state.mode === 'zen' &&
    myTurn &&
    !isFinished &&
    state.table?.length > 0 &&
    state.table_owner !== playerIdx &&
    !anyPending &&
    aceEffectActive &&
    hasAce
  oneChanceBtn.classList.toggle('hidden', !canOneChance)

  const info = document.getElementById('selected-info')
  if (pendingDiscard) {
    info.textContent =
      selectedCards.size === 1
        ? '1枚選択中（捨てる or スキップ）'
        : '捨てるカードを1枚選択してください'
  } else if (pendingGive) {
    const needed = state.pending_give?.count ?? 0
    info.textContent = `${selectedCards.size}/${needed}枚選択中（渡す）`
  } else {
    info.textContent =
      selectedCards.size > 0
        ? `${selectedCards.size}枚選択中`
        : myTurn
          ? 'カードをクリックして選択'
          : ''
  }
}

function renderLog(state) {
  if (!state.log) return
  const list = document.getElementById('log-list')
  list.innerHTML = [...state.log]
    .reverse()
    .map((line) => `<div class="log-item">${esc(line)}</div>`)
    .join('')
}

function renderPickupArea(state) {
  const area = document.getElementById('pickup-area')
  const pp = state.pending_pickup
  if (!pp || pp.player_idx !== playerIdx) {
    area.classList.add('hidden')
    pickupSelected.clear()
    return
  }

  area.classList.remove('hidden')
  const pile =
    pp.source === 'discard'
      ? (state.discard_pile ?? [])
      : (state.removed_cards ?? [])
  const effectName = pp.source === 'discard' ? '死者蘇生' : 'セイバー'
  const sourceName = pp.source === 'discard' ? '捨て札' : '除外カード'

  const required = Math.min(pp.count, pile.length)
  area.querySelector('.pickup-hint').textContent =
    `${effectName}：${sourceName}から${required}枚選んで「決定」を押してください`

  const pileEl = document.getElementById('pickup-pile')
  if (pile.length === 0) {
    pileEl.innerHTML =
      '<span style="color:#bb8fce;font-size:0.82rem">（取得できるカードがありません）</span>'
    document.getElementById('pickup-confirm-btn').disabled = true
    document.getElementById('pickup-selected-info').textContent = ''
    return
  }

  // Clear invalid selections (in case pile changed)
  for (const idx of pickupSelected) {
    if (idx >= pile.length) pickupSelected.delete(idx)
  }

  pileEl.innerHTML = pile
    .map((c, i) => {
      const [rank, suit] = splitDisplay(c.display)
      const sel = pickupSelected.has(i) ? ' selected' : ''
      return `<div class="card ${c.color || 'black'}${sel}" onclick="togglePickupCard(${i})" style="cursor:pointer">
      <span class="card-rank">${rank}</span><span class="card-suit">${suit}</span>
    </div>`
    })
    .join('')

  const confirmBtn = document.getElementById('pickup-confirm-btn')
  const infoEl = document.getElementById('pickup-selected-info')
  confirmBtn.disabled = pickupSelected.size !== required
  infoEl.textContent = `${pickupSelected.size}/${required}枚選択中`
}

function renderGiveArea(state) {
  const area = document.getElementById('give-area')
  const pg = state.pending_give
  if (!pg || pg.player_idx !== playerIdx) {
    area.classList.add('hidden')
    return
  }

  area.classList.remove('hidden')
  area.querySelector('.give-hint').textContent =
    `七渡し：${esc(pg.target_name)} へ ${pg.count}枚選んで渡してください`
}

// ══════════════════════════════════════════════════════════════
//  カード操作
// ══════════════════════════════════════════════════════════════
function toggleCard(idx) {
  if (!gameState || gameState.current_player_idx !== playerIdx) return
  if (selectedCards.has(idx)) {
    selectedCards.delete(idx)
    SoundEngine.deselect()
  } else {
    selectedCards.add(idx)
    SoundEngine.select()
  }
  renderPlayerHand(gameState)
  renderButtons(gameState)
}

// ══════════════════════════════════════════════════════════════
//  終了モーダル / エラー / リセット
// ══════════════════════════════════════════════════════════════
function playEffectSounds(effects) {
  if (effects.has('revolution')) SoundEngine.revolution()
  if (effects.has('eight_stop')) SoundEngine.eightStop()
  if (effects.has('rank_up')) SoundEngine.rankUp()
  if (effects.has('game_end')) SoundEngine.gameEnd()
  if (effects.has('j_back')) SoundEngine.uiClick()
  if (effects.has('suit_lock') || effects.has('mahakamaha'))
    SoundEngine.uiClick()
  if (effects.has('five_skip')) SoundEngine.uiClick()
  if (effects.has('tochaku')) SoundEngine.revolution()
}

function showNewEffectBanners(effects, state) {
  if (effects.has('j_back')) showEffectBanner('Jバック！', '#7d3c98')
  if (effects.has('mahakamaha')) {
    const syms = { SPADES: '♠', HEARTS: '♥', DIAMONDS: '♦', CLUBS: '♣' }
    const sym = state?.suit_lock_suit ? syms[state.suit_lock_suit] || '' : ''
    showEffectBanner(`摩訶鉢特摩！${sym}縛り`, '#1a5276')
  } else if (effects.has('suit_lock')) {
    const syms = { SPADES: '♠', HEARTS: '♥', DIAMONDS: '♦', CLUBS: '♣' }
    const sym = state?.suit_lock_suit ? syms[state.suit_lock_suit] || '' : ''
    showEffectBanner(`${sym}縛り！`, '#1a5276')
  }
  if (effects.has('five_skip')) showEffectBanner('5スキップ！', '#1e8449')
  if (effects.has('tochaku')) showEffectBanner('都落ち！', '#c0392b')
  if (effects.has('effect_nullified')) showEffectBanner('三途の川！', '#9b59b6')
  if (effects.has('ten_nullified')) showEffectBanner('十戒！', '#3498db')
  if (effects.has('dimension_eater'))
    showEffectBanner('ディメンションイーター！', '#8e44ad')
  if (effects.has('shisha_sosei')) showEffectBanner('死者蘇生！', '#6c3483')
  if (effects.has('saber')) showEffectBanner('セイバー！', '#e67e22')
  if (effects.has('nana_watashi')) showEffectBanner('七渡し！', '#1abc9c')
  if (effects.has('ashura')) showEffectBanner('阿修羅！', '#e74c3c')
}

function showEffectBanner(text, color) {
  const banner = document.getElementById('effect-banner')
  const el = document.getElementById('effect-text')
  // アニメーションをリセットしてから再表示
  banner.classList.add('hidden')
  el.textContent = text
  el.style.color = color
  void banner.offsetWidth // リフロー強制
  banner.classList.remove('hidden')
  setTimeout(() => banner.classList.add('hidden'), 2500)
}

// ── CPU ターンを1つずつアニメーションで再生する ────────────────────────
async function replayEvents(replay, prevState) {
  let prev = prevState ?? gameState

  for (let i = 0; i < replay.length; i++) {
    const snap = replay[i]
    const isLast = i === replay.length - 1
    const newTable = snap.table ?? []
    const prevTable = prev.table ?? []
    const tableChanged = JSON.stringify(newTable) !== JSON.stringify(prevTable)
    const effects = detectNewEffects(prev, snap)
    const newLogs = getNewLogEntries(prev, snap)

    // CPU play detection:
    //   Case A: table was updated (normal play)
    //   Case B: table was reset by 8切り/10捨て — detect from log entries
    const isCpuTablePlay =
      newTable.length > 0 &&
      tableChanged &&
      snap.table_owner !== null &&
      snap.table_owner !== playerIdx
    const cpuLogPlay = isCpuTablePlay
      ? null
      : extractCpuPlayFromLog(snap.players, newLogs, playerIdx)
    const isCpuPlay = isCpuTablePlay || cpuLogPlay !== null

    const animOwnerIdx = isCpuTablePlay
      ? snap.table_owner
      : (cpuLogPlay?.ownerIdx ?? null)
    const animCards = newTable.length > 0 ? newTable : (cpuLogPlay?.cards ?? [])

    if (isCpuPlay && animOwnerIdx !== null && animCards.length > 0) {
      const ownerEl = document.querySelector(
        `[data-player-idx="${animOwnerIdx}"]`,
      )
      const tableRect = document
        .getElementById('table-area')
        .getBoundingClientRect()
      if (ownerEl) {
        const r = ownerEl.getBoundingClientRect()
        const cx = r.left + r.width / 2
        const cy = r.top + r.height / 2
        const W = 52,
          H = 76
        const fromRects = Array.from({ length: animCards.length }, () => ({
          left: cx - W / 2,
          top: cy - H / 2,
          width: W,
          height: H,
        }))
        SoundEngine.play()
        await animateCardsToTable(fromRects, animCards, tableRect)
      }

      // 8切り: 着地確認 → バナー → 効果音
      if (effects.has('eight_stop')) {
        showCardsOnTableTemp(animCards)
        showEffectBanner('8切り！', '#f39c12')
        await delay(500)
        SoundEngine.eightStop()
        await delay(280)
        effects.delete('eight_stop')
      } else if (effects.has('ten_discard_play')) {
        // 10捨て: 着地確認 → バナー → 効果音
        showCardsOnTableTemp(animCards)
        showEffectBanner('10捨て！', '#e67e22')
        await delay(500)
        SoundEngine.uiClick()
        await delay(280)
        effects.delete('ten_discard_play')
      }
    } else {
      // CPU pickup: "{name}: {card} を手札に加えました"
      const pickupLog = newLogs.find((l) => l.includes('を手札に加えました'))
      // CPU give: "{name} → {target}: {card}"
      const giveLog = newLogs.find((l) => l.includes(' → ') && l.includes(': '))

      if (pickupLog) {
        // Determine source pile from previous state (discard or removed)
        const prevDiscardLen = (prev.discard_pile ?? []).length
        const snapDiscardLen = (snap.discard_pile ?? []).length
        const pileId =
          snapDiscardLen < prevDiscardLen
            ? 'zen-discard-pile'
            : 'zen-removed-pile'
        const pileEl = document.getElementById(pileId)
        // Find which CPU player did the pickup
        const pickupName = pickupLog.split(':')[0]
        const pickerPlayer = snap.players.find((p) => p.name === pickupName)
        const pickerEl = pickerPlayer
          ? document.querySelector(`[data-player-idx="${pickerPlayer.idx}"]`)
          : null
        if (pileEl && pickerEl) {
          await animateCardBetweenElements(pileEl, pickerEl, 1, true)
        }
      } else if (giveLog) {
        // Format: "giver → target: cards"
        const arrowIdx = giveLog.indexOf(' → ')
        const colonIdx = giveLog.indexOf(': ', arrowIdx)
        if (arrowIdx !== -1 && colonIdx !== -1) {
          const giverName = giveLog.slice(0, arrowIdx)
          const targetName = giveLog.slice(arrowIdx + 3, colonIdx)
          const giverPlayer = snap.players.find((p) => p.name === giverName)
          const targetPlayer = snap.players.find((p) => p.name === targetName)
          const giverEl = giverPlayer
            ? document.querySelector(`[data-player-idx="${giverPlayer.idx}"]`)
            : null
          const targetEl = targetPlayer
            ? document.querySelector(`[data-player-idx="${targetPlayer.idx}"]`)
            : null
          if (giverEl && targetEl) {
            await animateCardBetweenElements(giverEl, targetEl, 1, true)
          }
        }
      }

      // パス / 全員パス / その他（ログから判定）
      if (newLogs.some((l) => l.includes('全員パス'))) {
        SoundEngine.pass()
        showEffectBanner('全員パス', '#5dade2')
        await delay(600)
      } else if (newLogs.some((l) => l.includes(': パス'))) {
        SoundEngine.pass()
        showEffectBanner('パス', '#7f8c8d')
        await delay(420)
      } else {
        await delay(280)
      }
    }

    if (effects.has('revolution')) showEffectBanner('🔄 革命！', '#e74c3c')
    showNewEffectBanners(effects, snap)
    playEffectSounds(effects)
    gameState = snap
    renderAll(snap)

    if (!isLast) await delay(isCpuPlay ? 900 : 280)
    prev = snap
  }
}

function showEndModal(state) {
  SoundEngine.gameEnd()
  const rankColors = [
    '#f1c40f',
    '#95a5a6',
    '#d0d0d0',
    '#e67e22',
    '#c0392b',
    '#8e44ad',
  ]
  document.getElementById('end-results').innerHTML = state.finished_order
    .map((pidx, i) => {
      const p = state.players[pidx]
      const color = rankColors[Math.min(i, rankColors.length - 1)]
      return `
        <div class="result-row">
          <span class="result-rank" style="color:${color}">${esc(p.rank_name || `${i + 1}位`)}</span>
          <span class="result-name">${esc(p.name)}${p.is_cpu ? '' : ' 👤'}</span>
        </div>`
    })
    .join('')
  document.getElementById('end-overlay').classList.remove('hidden')
}

function showError(msg) {
  const hint = document.getElementById('table-hint')
  hint.style.color = '#e74c3c'
  hint.textContent = '❌ ' + msg
  setTimeout(() => {
    hint.style.color = ''
    if (gameState) renderTable(gameState)
  }, 2500)
}

function showLeaveConfirm() {
  // ゲームが進行中のときのみ警告を表示、それ以外はそのままリセット
  if (gameState && gameState.state === 'playing') {
    document.getElementById('confirm-overlay').classList.remove('hidden')
  } else {
    resetGame()
  }
}

function cancelLeave() {
  document.getElementById('confirm-overlay').classList.add('hidden')
}

async function confirmLeaveGame() {
  document.getElementById('confirm-overlay').classList.add('hidden')
  if (gameId !== null && playerIdx !== null) {
    try {
      await fetch(`${API}/game/${gameId}/leave`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ player_idx: playerIdx }),
      })
    } catch (_) {}
  }
  resetGame()
}

async function restartGame() {
  try {
    const res = await fetch(`${API}/game/${gameId}/restart`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ player_idx: playerIdx }),
    })
    const data = await res.json()
    if (!res.ok) {
      alert(data.error)
      return
    }

    document.getElementById('end-overlay').classList.add('hidden')
    selectedCards.clear()
    showBoard()

    const initialState = data.initial_state ?? data.state
    gameState = initialState
    renderAll(initialState)

    const cpuReplay = data.cpu_replay ?? []
    if (cpuReplay.length > 0) {
      await delay(700)
      await replayEvents(cpuReplay, initialState)
    }
    render(data.state)
  } catch (_) {
    alert('バックエンドに接続できません (http://localhost:8000)')
  }
}

function resetGame() {
  stopPolling()
  stopLobbyPoll()
  gameId = playerIdx = gameState = null
  selectedCards.clear()
  sessionStorage.removeItem('daifugo')
  document.getElementById('end-overlay').classList.add('hidden')
  document.getElementById('create-name').value = ''
  document.getElementById('join-name').value = ''
  document.getElementById('join-code').value = ''
  showScreenStart()
}

function saveSession() {
  sessionStorage.setItem(
    'daifugo',
    JSON.stringify({ gid: gameId, pidx: playerIdx }),
  )
}

// ══════════════════════════════════════════════════════════════
//  ユーティリティ
// ══════════════════════════════════════════════════════════════
function cardHtml(card, selected, isTable, idx = null) {
  const cls = [
    'card',
    card.color || 'black',
    selected ? 'selected' : '',
    isTable ? 'table-card' : '',
  ]
    .filter(Boolean)
    .join(' ')
  const attr = idx !== null ? ` data-idx="${idx}"` : ''
  const [rank, suit] = splitDisplay(card.display)
  return `<div class="${cls}"${attr}><span class="card-rank">${rank}</span><span class="card-suit">${suit}</span></div>`
}

function splitDisplay(display) {
  if (!display || display === 'JK') return ['JK', '🃏']
  const suits = new Set(['♠', '♥', '♦', '♣'])
  const last = display.slice(-1)
  return suits.has(last) ? [display.slice(0, -1), last] : [display, '']
}

const delay = (ms) => new Promise((r) => setTimeout(r, ms))

// 8切り演出用: 実際の render より先に場のカードを一時表示する
function showCardsOnTableTemp(cards) {
  document.getElementById('table-cards').innerHTML = cards
    .map((c) => cardHtml(c, false, true))
    .join('')
}

// 非ジョーカーが含まれる場合はジョーカーを同格扱い（backend の _combo_value と同じ仕様）
function comboValue(cards) {
  const nonJokers = cards.filter((c) => c.display !== 'JK')
  if (nonJokers.length > 0) return Math.max(...nonJokers.map((c) => c.value))
  return Math.max(...cards.map((c) => c.value))
}

function valLabel(v) {
  return (
    ['3', '4', '5', '6', '7', '8', '9', '10', 'J', 'Q', 'K', 'A', '2', 'JK'][
      v
    ] ?? String(v)
  )
}

// 旧状態と新状態のログを比較して新規エントリを返す
function getNewLogEntries(oldState, newState) {
  const oldLog = oldState?.log ?? []
  const newLog = newState?.log ?? []
  let cutIdx = 0
  if (oldLog.length > 0) {
    const last = oldLog[oldLog.length - 1]
    const pos = newLog.lastIndexOf(last)
    if (pos >= 0) cutIdx = pos + 1
    else cutIdx = newLog.length - Math.max(0, newLog.length - oldLog.length)
  }
  return newLog.slice(cutIdx)
}

function detectNewEffects(oldState, newState) {
  const effects = new Set()
  for (const entry of getNewLogEntries(oldState, newState)) {
    if (entry.includes('8切り') || entry.includes('八切'))
      effects.add('eight_stop')
    if (entry.includes('革命！') || entry.includes('無限革命'))
      effects.add('revolution')
    if (entry.includes('上がり')) effects.add('rank_up')
    if (entry.includes('ゲーム終了')) effects.add('game_end')
    if (entry.includes('Jバック') || entry.includes('イレブンバック'))
      effects.add('j_back')
    if (entry.includes('スートしばり')) effects.add('suit_lock')
    if (entry.includes('摩訶鉢特摩')) effects.add('mahakamaha')
    if (entry.includes('5スキップ') || entry.includes('五飛ばし'))
      effects.add('five_skip')
    if (entry.includes('都落ち')) effects.add('tochaku')
    if (entry.includes('10捨て')) effects.add('ten_discard_play')
    if (entry.includes('三途の川')) effects.add('effect_nullified')
    if (entry.includes('十戒')) effects.add('ten_nullified')
    if (entry.includes('ディメンションイーター')) effects.add('dimension_eater')
    if (entry.includes('死者蘇生')) effects.add('shisha_sosei')
    if (entry.includes('セイバー')) effects.add('saber')
    if (entry.includes('七渡し')) effects.add('nana_watashi')
    if (entry.includes('阿修羅')) effects.add('ashura')
    if (entry.includes('ワンチャンス')) effects.add('one_chance')
  }
  return effects
}

// ログエントリからCPUのプレイ情報を抽出（8切り/10捨てでtableがリセットされた場合に使用）
function extractCpuPlayFromLog(players, logs, humanPlayerIdx) {
  for (const p of players) {
    if (p.idx === humanPlayerIdx) continue
    const playLog = logs.find(
      (l) =>
        l.startsWith(p.name + ': ') &&
        !l.includes('を捨てました') &&
        !l.includes('パス') &&
        !l.includes('スキップ') &&
        !l.includes('退出') &&
        !l.includes('を手札に加えました') &&
        !l.includes('ワンチャンス発動'),
    )
    if (playLog) {
      const cardsStr = playLog.slice(p.name.length + 2).trim()
      const displays = cardsStr.split(' ').filter((s) => s.length > 0)
      if (displays.length > 0) {
        const cards = displays.map((d) => ({
          display: d,
          color: d.includes('♥') || d.includes('♦') ? 'red' : 'black',
        }))
        return { ownerIdx: p.idx, cards }
      }
    }
  }
  return null
}

// ══════════════════════════════════════════════════════════════
//  ルール確認モーダル
// ══════════════════════════════════════════════════════════════
const RULE_INFO = [
  {
    key: 'eight_stop',
    name: '8切り',
    desc: '8を出すと場をリセット。8を出したプレイヤーが続けて出す',
  },
  {
    key: 'revolution',
    name: '革命',
    desc: '4枚同時出しでカードの強弱が完全に逆転。ジョーカーは常に最強',
  },
  {
    key: 'five_skip',
    name: '5スキップ',
    desc: '5を出すと次のプレイヤーの手番をスキップ',
  },
  {
    key: 'j_back',
    name: 'Jバック',
    desc: 'Jを出すと強弱が逆転。場が流れるまで（全員パス・8切り・10捨て）持続する。革命中は一時的に通常に戻る',
  },
  {
    key: 'suit_lock',
    name: 'スートしばり',
    desc: '複数枚を同じスートで出すと、次以降は同スートを含む手しか出せない。ジョーカーはどのスートとしても扱える',
  },
  {
    key: 'ten_discard',
    name: '10捨て',
    desc: '10を出すと場をリセット。さらに手札から1枚を任意で捨てられる（スキップも可）',
  },
  {
    key: 'tochaku',
    name: '都落ち',
    desc: '前の大富豪が最初の手番でパスすると最下位に転落',
  },
  {
    key: 'stairs',
    name: '階段',
    desc: '3枚以上の連続するランクをまとめて出せる（スート不問、ジョーカー不可）',
  },
  {
    key: 'tribute',
    name: 'カード交換',
    desc: '再戦時に大富豪・大貧民間で自動カード交換。上位→下位は最弱カード、下位→上位は最強カード',
  },
  {
    key: 'spade3_return',
    name: 'スペード3返し',
    desc: 'カード交換時、大貧民がスペード3を持つと献上カードの最弱を3♠に差し替えて最強カードを手元に残せる',
  },
]

function showRulesModal() {
  if (!gameState || !gameState.rules) return
  const rules = gameState.rules
  document.getElementById('rules-list').innerHTML = RULE_INFO.map((r) => {
    const on = rules[r.key] ?? false
    return `
      <div class="rules-item ${on ? '' : 'rules-item-off'}">
        <div class="rules-item-header">
          <span class="rules-item-name">${esc(r.name)}</span>
          <span class="rules-item-status ${on ? 'status-on' : 'status-off'}">${on ? 'ON' : 'OFF'}</span>
        </div>
        <div class="rules-item-desc">${esc(r.desc)}</div>
      </div>`
  }).join('')
  document.getElementById('rules-overlay').classList.remove('hidden')
}

function closeRulesModal() {
  document.getElementById('rules-overlay').classList.add('hidden')
}

// ══════════════════════════════════════════════════════════════
//  カード効果モーダル（全役職大富豪モード）
// ══════════════════════════════════════════════════════════════
const CARD_RULES_ZEN = [
  {
    rank: '3',
    name: '三途の川',
    desc: '3を出すと、場が流れるまで以降に出されたカードの固有効果を完全に無効化する。',
  },
  {
    rank: '4',
    name: '死者蘇生',
    desc: '4を出した枚数分、捨て札（流れたカード）から任意のカードを手札に加えなければならない。手札なしで上がった場合は発動されない。捨て札がなければ無効。',
  },
  {
    rank: '5',
    name: '五飛ばし',
    desc: '5を出した枚数分、次のプレイヤーをスキップする。',
  },
  {
    rank: '6',
    name: '無限革命',
    desc: '単体では効果なし。ジョーカーを含まず4枚の6を同時に出すと無限革命が発動。全員で再スタートし、6が最強カード（Joker除く）となり全カード固有効果が失われる。',
  },
  {
    rank: '7',
    name: '七渡し',
    desc: '7を出した枚数分、右隣（1つ前の手番のプレイヤー）に手札から任意のカードを渡せる。これによって手札を0枚にして上がることも可能。',
  },
  {
    rank: '8',
    name: '八切',
    desc: '8を出すと場をリセットし、自分のターンを続行する。',
  },
  {
    rank: '9',
    name: '阿修羅',
    desc: '9を1枚出すと強制的に3枚出したとみなされる。3枚出しの場（または九の効果で3枚縛りの場）にのみ出せる。この3枚縛りはその後も維持される。複数枚の9は通常通り扱う。',
  },
  {
    rank: '10',
    name: '十戒',
    desc: '10を出すと、場が流れるまでA・2・3〜10の固有効果を完全に無効化する。J・Q・K・Jokerの効果は有効。',
  },
  {
    rank: 'J',
    name: 'イレブンバック',
    desc: 'Jを出すと場が流れるまで強弱が逆転する（革命中は一時的に通常に戻る）。',
  },
  {
    rank: 'Q',
    name: '摩訶鉢特摩',
    desc: 'Qを出すと、場が流れるまでQのスートでスートしばりが発動する。',
  },
  {
    rank: 'K',
    name: 'セイバー',
    desc: 'Kを出した枚数分、2（ディメンションイーター）で除外されたカードから任意のカードを手札に加えなければならない（除外カードがない場合は無効）。手札なしで上がった場合は発動されない。',
  },
  {
    rank: 'A',
    name: 'ワンチャンス',
    desc: '他のプレイヤーのカードが場にある自分のターンに発動できる。参加人数分の1の確率で場を捨て札にして自分のターンとなる。失敗するとAを失い次のプレイヤーへ。通常のAとして出すことも可能。',
  },
  {
    rank: '2',
    name: 'ディメンションイーター',
    desc: '2を出すと場のカードを全て永久除外（ゲームから取り除く）し、自分のターンを続行する。除外カードはKの効果で回収できる。',
  },
  {
    rank: 'JK',
    name: 'ジョーカー',
    desc: '最強カードとして機能する。数字カードと一緒に出した場合、その数字カードの固有効果が適用される。単体では効果なし。',
  },
]

function showCardRulesModal() {
  document.getElementById('card-rules-list').innerHTML = CARD_RULES_ZEN.map(
    (r) => `
    <div class="card-rule-item">
      <div class="card-rule-header">
        <span class="card-rule-rank">${esc(r.rank)}</span>
        <span class="card-rule-name">${esc(r.name)}</span>
      </div>
      <div class="card-rule-desc">${esc(r.desc)}</div>
    </div>`,
  ).join('')
  document.getElementById('card-rules-overlay').classList.remove('hidden')
}

function closeCardRulesModal() {
  document.getElementById('card-rules-overlay').classList.add('hidden')
}

function esc(str) {
  return String(str ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}
