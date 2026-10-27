/*
 * Headless simulation worker.
 *
 * One worker = one `crown-sim-core` WASM instance = one independent run. The
 * worker never touches the DOM; it advances the fixed 1/60s simulation in
 * wall-clock-budgeted chunks, so the UI stays responsive and pause/stop/step
 * messages are honoured between chunks.
 *
 * Protocol (main -> worker):
 *   { type: 'init', url? }
 *   { type: 'getMeta' } | { type: 'getState' }
 *   { type: 'reset', config }            // full Config, creates a fresh run
 *   { type: 'start' } | { type: 'pause' } | { type: 'resume' } | { type: 'stop' }
 *   { type: 'step', steps }              // manual advance while paused
 *   { type: 'pick', index }
 *   { type: 'setSpeed', scale }          // 1 = realtime, >=999 = flat out
 *   { type: 'setConfig', patch }         // applies to the next reset
 *   { type: 'snapshot', detail }
 *   { type: 'destroy' }
 *
 * Worker -> main:
 *   { type: 'ready', version }
 *   { type: 'meta', data }
 *   { type: 'state', data }              // legacy simulator state shape
 *   { type: 'done', data }               // final state (death / win / limit)
 *   { type: 'snapshot', data }
 *   { type: 'error', error }
 */
'use strict';

importScripts('sim-core.js');

var DEFAULT_CONFIG = {
  seed: 1,
  character: 0,
  auto: true,
  time_limit: 0,
  curse: 0,
  view_w: 960,
  view_h: 540
};

var core = null;
var handle = 0;
var baseConfig = Object.assign({}, DEFAULT_CONFIG);
var running = false;
var paused = false;
var finished = false;
var speed = 1;
var timer = null;
var lastTick = 0;
var lastPost = 0;
var pendingSteps = 0;
var lastChoiceKey = '';

var POST_INTERVAL_MS = 100;
var MAX_CHUNK_STEPS = 1200;

function post(msg) {
  self.postMessage(msg);
}

function fail(err) {
  post({ type: 'error', error: String(err && err.stack ? err.stack : err) });
}

function ensureCore() {
  if (!core) throw new Error('sim worker not initialised');
  return core;
}

function alive() {
  return !!core && !!handle && !finished;
}

function destroyRun() {
  if (core && handle) {
    try { core.destroy(handle); } catch (e) { /* already gone */ }
  }
  handle = 0;
}

function createRun(config) {
  var coreRef = ensureCore();
  destroyRun();
  baseConfig = Object.assign({}, baseConfig, config || {});
  handle = coreRef.create(baseConfig);
  finished = false;
  paused = false;
  lastPost = 0;
  lastChoiceKey = '';
}

function toState(snap, extra) {
  var state = {
    time: snap.t,
    kills: snap.kills,
    level: snap.level,
    damage: snap.damage,
    hp: snap.hp,
    maxHp: snap.maxHp,
    enemies: snap.enemies,
    gold: snap.gold,
    xp: snap.xp,
    xpNeed: snap.xpNeed,
    bosses: snap.bosses,
    phase: snap.phase,
    curse: snap.curse,
    char: snap.character,
    running: running && !paused && !finished,
    paused: paused,
    over: !!snap.over,
    result: snap.over || null,
    won: !!snap.won,
    timedOut: !!snap.timedOut,
    ended: !!snap.ended,
    weapons: Object.keys(snap.weapons || {}),
    passives: Object.keys(snap.passives || {}),
    evolutions: snap.evolutions || [],
    revives: snap.revive || 0,
    rerolls: snap.rerolls || 0,
    seed: snap.seed,
    core: snap.version,
    choice: snap.choice || null,
    powerUps: {
      dash: snap.dashLv || 0,
      walls: snap.wallLv || 0,
      revive: snap.revive || 0,
      wallEvo: !!snap.wallEvo
    }
  };
  if (extra) Object.assign(state, extra);
  return state;
}

function readState(extra) {
  var snap = ensureCore().snapshot(handle, false);
  return toState(snap, extra);
}

function emitState(force) {
  if (!alive()) return;
  var now = lastTick || Date.now();
  if (!force && now - lastPost < POST_INTERVAL_MS) return;
  lastPost = now;
  post({ type: 'state', data: readState() });
}

function emitDone() {
  if (!alive()) return;
  running = false;
  finished = true;
  var snap = ensureCore().snapshot(handle, false);
  post({ type: 'done', data: toState(snap, { running: false, finished: true }) });
}

function schedule(delay) {
  if (timer !== null) return;
  timer = setTimeout(tick, delay === undefined ? 0 : delay);
}

function readyToStep() {
  return core && handle && !finished && !paused && (running || pendingSteps > 0);
}

function tick() {
  timer = null;
  if (!readyToStep()) return;
  var now = performance.now();
  var dtWall = lastTick ? Math.min(0.25, (now - lastTick) / 1000) : 1 / 60;
  lastTick = now;

  var steps;
  if (pendingSteps > 0) {
    steps = Math.min(pendingSteps, MAX_CHUNK_STEPS);
    pendingSteps -= steps;
  } else if (speed >= 999) {
    // Flat out: no wall-clock pacing. 1200 steps = 20 game seconds per chunk
    // keeps pause/step messages responsive without flooding the main thread.
    steps = MAX_CHUNK_STEPS;
  } else {
    // `speed` is the game-time multiplier versus wall time.
    var want = speed * 60 * dtWall;
    steps = Math.floor(want + (tick.carry || 0));
    tick.carry = (want + (tick.carry || 0)) - steps;
    if (steps > MAX_CHUNK_STEPS) { steps = MAX_CHUNK_STEPS; tick.carry = 0; }
    if (steps < 1 && pendingSteps === 0) {
      schedule(4);
      return;
    }
  }

  try {
    var status = ensureCore().step(handle, steps, { auto: true });
  } catch (e) {
    fail(e);
    running = false;
    return;
  }

  if (status.choice) {
    // Auto-play picks cards inside the core; surface the choice for hosts that
    // want to display or override it.
    var key = status.choice.source + ':' + status.choice.cards.map(function (c) { return c.k; }).join(',');
    if (key !== lastChoiceKey) {
      lastChoiceKey = key;
      post({ type: 'choice', data: status.choice });
    }
  }

  if (status.over || status.ended) {
    emitDone();
    return;
  }

  emitState(paused);
  if (pendingSteps > 0 || running) schedule(0);
}

function mergeConfig(patch) {
  baseConfig = Object.assign({}, baseConfig, patch || {});
}

self.onmessage = function (event) {
  var msg = event.data;
  if (!msg || !msg.type) return;
  try {
    switch (msg.type) {
      case 'init':
        CrownSimCore.load({ url: msg.url, module: msg.module, buffer: msg.buffer }).then(function (loaded) {
          core = loaded;
          if (handle) destroyRun();
          if (msg.config) mergeConfig(msg.config);
          post({ type: 'ready', version: core.version, engine: core.engine });
        }).catch(fail);
        break;

      case 'getMeta':
        post({ type: 'meta', data: ensureCore().meta() });
        break;

      case 'setConfig':
        mergeConfig(msg.config || msg.patch || {});
        break;

      case 'reset':
        if (!core) { post({ type: 'error', error: 'not initialised' }); break; }
        running = false;
        if (msg.auto !== undefined) baseConfig.auto = msg.auto !== false;
        createRun(msg.config || {});
        lastTick = 0;
        tick.carry = 0;
        // Legacy `reset {auto:true}` starts the run immediately. Hosts can pass
        // `start:false` to stage a run and start it explicitly.
        if (msg.start !== false) {
          running = true;
          post({ type: 'state', data: readState() });
          schedule(0);
        } else {
          post({ type: 'state', data: readState() });
        }
        break;

      case 'start':
        if (!alive()) {
          if (core) createRun({});
          else { post({ type: 'error', error: 'not initialised' }); break; }
        }
        running = true;
        paused = false;
        lastTick = 0;
        schedule(0);
        break;

      case 'pause':
        paused = true;
        post({ type: 'state', data: readState({ paused: true, running: false }) });
        break;

      case 'resume':
        paused = false;
        lastTick = 0;
        schedule(0);
        break;

      case 'stop':
        running = false;
        paused = false;
        post({ type: 'state', data: readState({ running: false }) });
        break;

      case 'step':
        paused = true;
        pendingSteps += Math.max(1, (msg.steps || 1) >>> 0);
        schedule(0);
        break;

      case 'setSpeed':
        speed = Math.max(0.01, Number(msg.scale) || 1);
        lastTick = 0;
        tick.carry = 0;
        break;

      // Legacy simulator naming.
      case 'setTimeScale':
        speed = Math.max(0.01, Number(msg.scale) || 1);
        lastTick = 0;
        tick.carry = 0;
        break;

      case 'setSeed':
        baseConfig.seed = msg.seed >>> 0;
        break;

      case 'setTimeLimit':
        baseConfig.time_limit = Math.max(0, Number(msg.limit) || 0);
        break;

      case 'setBuildProfile':
        baseConfig.profile = msg.profile || null;
        break;

      case 'auto':
      case 'setAuto':
        baseConfig.auto = msg.enabled !== false;
        if (handle) {
          ensureCore().configure(handle, { auto: baseConfig.auto });
          post({ type: 'state', data: readState() });
        }
        break;

      case 'getState':
        if (alive()) post({ type: 'state', data: readState() });
        break;

      case 'snapshot':
        if (handle) post({ type: 'snapshot', data: ensureCore().snapshot(handle, !!msg.detail) });
        break;

      case 'pick':
        if (handle) {
          ensureCore().pick(handle, msg.index >>> 0);
          post({ type: 'state', data: readState() });
        }
        break;

      case 'destroy':
        running = false;
        destroyRun();
        break;

      default:
        post({ type: 'error', error: 'unknown message type ' + msg.type });
    }
  } catch (e) {
    fail(e);
  }
};
