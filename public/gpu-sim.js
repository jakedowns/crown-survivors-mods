/*
 * Crown Survivors GPU lab core.
 *
 * Two engines, one workload:
 *
 *   Engine C (GPU):  a fully data-parallel horde kernel in WGSL. No atomics and
 *                    no cross-invocation writes, so a given device is
 *                    run-to-run deterministic.
 *   Engine D (hybrid): partitions independent runs between CPU workers (the
 *                    same kernel, standing in for the WASM core) and the GPU,
 *                    then runs both sides concurrently.
 *
 * The kernel is deliberately shaped like the real game's hot loop:
 * enemies chase the player, separate from each other, projectiles travel and
 * hit the nearest enemy, enemies die and respawn at the edges, and the player
 * takes contact damage. It is *not* the authoritative game core; it is a
 * measurement rig for deciding whether a WGSL port or GPU offload is worth it.
 *
 * Data layout (flat typed arrays; one block of 8 lanes per entity):
 *   runU[run*8 + 0] rng
 *   runU[run*8 + 1] kills
 *   runU[run*8 + 2] live enemies
 *   runU[run*8 + 3] steps
 *   runU[run*8 + 4] over
 *   runU[run*8 + 5] spawned total
 *   runF[run*8 + 0] player hp
 *   runF[run*8 + 1] damage dealt
 *   runF[run*8 + 2] spawn accumulator
 *   runF[run*8 + 3] game time
 *
 *   enemy[base+0..3] x, y, vx, vy
 *   enemy[base+4] hp
 *   enemy[base+5] alive
 *   enemy[base+6] killed this step
 *   enemy[base+7] damage taken this step
 *
 *   projectile[base+0..3] x, y, vx, vy
 *   projectile[base+4] damage
 *   projectile[base+5] alive
 *   projectile[base+6] hit enemy index (-1 = none)
 *   projectile[base+7] pad
 */
(function (root) {
  'use strict';

  var RUN_STRIDE = 8;
  var E_STRIDE = 8;
  var P_STRIDE = 8;
  var WG = 64;
  var TAU = Math.PI * 2;

  var DEFAULTS = {
    runs: 8,
    maxEnemies: 128,
    maxProjectiles: 64,
    seed: 12345,
    runOffset: 0,
    spawnRate: 6,
    enemySpeed: 55,
    enemyHp: 20,
    enemyDmg: 0.01,
    projSpeed: 520,
    projDmg: 2,
    projRadius: 6,
    enemyRadius: 8,
    playerRadius: 14,
    worldW: 960,
    worldH: 540,
    dt: 1 / 60,
    sepRadius: 22,
    sepForce: 90
  };

  function withDefaults(cfg) {
    var out = {};
    var k;
    for (k in DEFAULTS) out[k] = DEFAULTS[k];
    if (cfg) for (k in cfg) if (cfg[k] !== undefined) out[k] = cfg[k];
    out.runs = Math.max(0, out.runs | 0);
    out.maxEnemies = Math.max(1, out.maxEnemies | 0);
    out.maxProjectiles = Math.max(1, out.maxProjectiles | 0);
    out.seed = out.seed >>> 0;
    out.runOffset = out.runOffset >>> 0;
    return out;
  }

  function hashU32(x) {
    x = x >>> 0;
    x = Math.imul(x ^ (x >>> 16), 0x7feb352d) >>> 0;
    x = Math.imul(x ^ (x >>> 15), 0x846ca68b) >>> 0;
    return (x ^ (x >>> 16)) >>> 0;
  }

  function rand01(x) {
    return (hashU32(x) >>> 8) / 16777216;
  }

  function createState(cfg) {
    cfg = withDefaults(cfg);
    var runs = cfg.runs;
    return {
      cfg: cfg,
      runU: new Uint32Array(runs * RUN_STRIDE),
      runF: new Float32Array(runs * RUN_STRIDE),
      enemies: new Float32Array(runs * cfg.maxEnemies * E_STRIDE),
      projectiles: new Float32Array(runs * cfg.maxProjectiles * P_STRIDE),
      // Scratch snapshot of enemy positions. The GPU reads the whole horde at
      // once, so the CPU reference must also separate from pre-step positions;
      // otherwise the two implement different algorithms and drift apart.
      sepX: new Float32Array(runs * cfg.maxEnemies),
      sepY: new Float32Array(runs * cfg.maxEnemies)
    };
  }

  function edgePosition(h, worldW, worldH) {
    var edge = h % 4;
    var t = rand01((h + 1) >>> 0);
    if (edge === 0) return [t * worldW, -20];
    if (edge === 1) return [t * worldW, worldH + 20];
    if (edge === 2) return [-20, t * worldH];
    return [worldW + 20, t * worldH];
  }

  function initState(state) {
    var cfg = state.cfg;
    var runU = state.runU;
    var runF = state.runF;
    var enemies = state.enemies;
    var projectiles = state.projectiles;
    var N = cfg.maxEnemies;
    var NP = cfg.maxProjectiles;
    var px = cfg.worldW * 0.5;
    var py = cfg.worldH * 0.5;

    for (var r = 0; r < cfg.runs; r++) {
      var g = (cfg.runOffset + r) >>> 0;
      var rb = r * RUN_STRIDE;
      var runSeed = (cfg.seed ^ Math.imul(g, 0x9e3779b9)) >>> 0;
      runU[rb] = runSeed;
      runU[rb + 1] = 0;
      runU[rb + 2] = 0;
      runU[rb + 3] = 0;
      runU[rb + 4] = 0;
      runU[rb + 5] = 0;
      runF[rb] = 100;
      runF[rb + 1] = 0;
      runF[rb + 2] = 0;
      runF[rb + 3] = 0;

      for (var i = 0; i < N; i++) {
        var eb = (r * N + i) * E_STRIDE;
        var h = hashU32(runSeed ^ Math.imul(i, 0x85ebca6b));
        var pos = edgePosition(h, cfg.worldW, cfg.worldH);
        enemies[eb] = pos[0];
        enemies[eb + 1] = pos[1];
        enemies[eb + 2] = 0;
        enemies[eb + 3] = 0;
        enemies[eb + 4] = cfg.enemyHp;
        enemies[eb + 5] = 1;
        enemies[eb + 6] = 0;
        enemies[eb + 7] = 0;
      }

      for (var p = 0; p < NP; p++) {
        var pb = (r * NP + p) * P_STRIDE;
        var a = rand01(hashU32(runSeed ^ Math.imul(p, 0xc2b2ae35))) * TAU;
        projectiles[pb] = px;
        projectiles[pb + 1] = py;
        projectiles[pb + 2] = Math.cos(a) * cfg.projSpeed;
        projectiles[pb + 3] = Math.sin(a) * cfg.projSpeed;
        projectiles[pb + 4] = cfg.projDmg;
        projectiles[pb + 5] = 1;
        projectiles[pb + 6] = -1;
        projectiles[pb + 7] = 0;
      }
    }
  }

  function nextRand(runU, r) {
    var b = r * RUN_STRIDE;
    var s = (Math.imul(runU[b], 1664525) + 1013904223) >>> 0;
    runU[b] = s;
    return (s >>> 8) / 16777216;
  }

  function stepCpu(state) {
    var cfg = state.cfg;
    var runU = state.runU;
    var runF = state.runF;
    var enemies = state.enemies;
    var projectiles = state.projectiles;
    var R = cfg.runs;
    var N = cfg.maxEnemies;
    var NP = cfg.maxProjectiles;
    var dt = cfg.dt;
    var px = cfg.worldW * 0.5;
    var py = cfg.worldH * 0.5;
    var sepR2 = cfg.sepRadius * cfg.sepRadius;
    var hitR = cfg.projRadius + cfg.enemyRadius;
    var hitR2 = hitR * hitR;
    var playerR2 = cfg.playerRadius * cfg.playerRadius;
    var r, i, j, rb, eb, pb, d, dx, dy, d2, odx, ody, od2, inv, sx, sy, x, y, hp;

    // 1. Respawn spent projectiles at the player.
    for (r = 0; r < R; r++) {
      rb = r * RUN_STRIDE;
      if (runU[rb + 4] !== 0) continue;
      for (i = 0; i < NP; i++) {
        pb = (r * NP + i) * P_STRIDE;
        if (projectiles[pb + 5] !== 0) continue;
        var a = rand01(hashU32((cfg.seed ^ Math.imul(cfg.runOffset + r, 0x9e3779b9) ^
          Math.imul(i, 0x85ebca6b) ^ Math.imul(runU[rb + 3], 0xc2b2ae35)) >>> 0)) * TAU;
        projectiles[pb] = px;
        projectiles[pb + 1] = py;
        projectiles[pb + 2] = Math.cos(a) * cfg.projSpeed;
        projectiles[pb + 3] = Math.sin(a) * cfg.projSpeed;
        projectiles[pb + 4] = cfg.projDmg;
        projectiles[pb + 5] = 1;
        projectiles[pb + 6] = -1;
      }
    }

    // 2. Enemy chase + all-pairs separation.
    //
    // Snapshot positions first: the GPU invoke reads every position from the
    // pre-step buffer, so the CPU reference does the same. Writing in place
    // (neighbor already moved) would be a different algorithm.
    var sepX = state.sepX;
    var sepY = state.sepY;
    for (r = 0; r < R; r++) {
      rb = r * RUN_STRIDE;
      if (runU[rb + 4] !== 0) continue;
      var nbase = r * N;
      for (i = 0; i < N; i++) {
        eb = (nbase + i) * E_STRIDE;
        sepX[nbase + i] = enemies[eb];
        sepY[nbase + i] = enemies[eb + 1];
      }
      for (i = 0; i < N; i++) {
        eb = (nbase + i) * E_STRIDE;
        if (enemies[eb + 5] === 0) continue;
        x = sepX[nbase + i];
        y = sepY[nbase + i];
        dx = px - x;
        dy = py - y;
        d = Math.max(Math.sqrt(dx * dx + dy * dy), 1);
        sx = 0;
        sy = 0;
        for (j = 0; j < N; j++) {
          if (j === i) continue;
          var ob = (nbase + j) * E_STRIDE;
          if (enemies[ob + 5] === 0) continue;
          odx = x - sepX[nbase + j];
          ody = y - sepY[nbase + j];
          od2 = odx * odx + ody * ody;
          if (od2 > 0.0001 && od2 < sepR2) {
            inv = cfg.sepForce / od2;
            sx += odx * inv;
            sy += ody * inv;
          }
        }
        x += (dx / d * cfg.enemySpeed + sx) * dt;
        y += (dy / d * cfg.enemySpeed + sy) * dt;
        enemies[eb] = Math.max(-100, Math.min(cfg.worldW + 100, x));
        enemies[eb + 1] = Math.max(-100, Math.min(cfg.worldH + 100, y));
        enemies[eb + 2] = dx / d * cfg.enemySpeed + sx;
        enemies[eb + 3] = dy / d * cfg.enemySpeed + sy;
      }
    }

    // 3. Projectile travel.
    for (r = 0; r < R; r++) {
      rb = r * RUN_STRIDE;
      if (runU[rb + 4] !== 0) continue;
      for (i = 0; i < NP; i++) {
        pb = (r * NP + i) * P_STRIDE;
        if (projectiles[pb + 5] === 0) continue;
        projectiles[pb] += projectiles[pb + 2] * dt;
        projectiles[pb + 1] += projectiles[pb + 3] * dt;
        if (projectiles[pb] < -20 || projectiles[pb] > cfg.worldW + 20 ||
            projectiles[pb + 1] < -20 || projectiles[pb + 1] > cfg.worldH + 20) {
          projectiles[pb + 5] = 0;
        }
      }
    }

    // 4. First-hit selection (fixed enemy index order => deterministic).
    for (r = 0; r < R; r++) {
      rb = r * RUN_STRIDE;
      if (runU[rb + 4] !== 0) continue;
      for (i = 0; i < NP; i++) {
        pb = (r * NP + i) * P_STRIDE;
        if (projectiles[pb + 5] === 0) continue;
        var hit = -1;
        for (j = 0; j < N; j++) {
          var hb = (r * N + j) * E_STRIDE;
          if (enemies[hb + 5] === 0) continue;
          dx = projectiles[pb] - enemies[hb];
          dy = projectiles[pb + 1] - enemies[hb + 1];
          if (dx * dx + dy * dy < hitR2) {
            hit = j;
            break;
          }
        }
        projectiles[pb + 6] = hit;
      }
    }

    // 5. Damage reduction: each enemy owns its hp and sums hits in index order.
    for (r = 0; r < R; r++) {
      rb = r * RUN_STRIDE;
      if (runU[rb + 4] !== 0) continue;
      for (j = 0; j < N; j++) {
        eb = (r * N + j) * E_STRIDE;
        if (enemies[eb + 5] === 0) continue;
        var sum = 0;
        for (i = 0; i < NP; i++) {
          pb = (r * NP + i) * P_STRIDE;
          if (projectiles[pb + 5] !== 0 && projectiles[pb + 6] === j) sum += projectiles[pb + 4];
        }
        if (sum > 0) {
          hp = enemies[eb + 4] - sum;
          enemies[eb + 7] = sum;
          if (hp <= 0) {
            enemies[eb + 4] = 0;
            enemies[eb + 5] = 0;
            enemies[eb + 6] = 1;
          } else {
            enemies[eb + 4] = hp;
            enemies[eb + 6] = 0;
          }
        } else {
          enemies[eb + 6] = 0;
          enemies[eb + 7] = 0;
        }
      }
    }

    // 6. Consume projectiles that hit.
    for (r = 0; r < R; r++) {
      rb = r * RUN_STRIDE;
      if (runU[rb + 4] !== 0) continue;
      for (i = 0; i < NP; i++) {
        pb = (r * NP + i) * P_STRIDE;
        if (projectiles[pb + 6] >= 0) {
          projectiles[pb + 5] = 0;
          projectiles[pb + 6] = -1;
        }
      }
    }

    // 7. Spawn replacements at the edges.
    for (r = 0; r < R; r++) {
      rb = r * RUN_STRIDE;
      if (runU[rb + 4] !== 0) continue;
      var acc = runF[rb + 2] + cfg.spawnRate * dt;
      var spawned = 0;
      while (acc >= 1 && spawned < 8) {
        var slot = -1;
        for (i = 0; i < N; i++) {
          if (enemies[(r * N + i) * E_STRIDE + 5] === 0) { slot = i; break; }
        }
        if (slot < 0) break;
        acc -= 1;
        var sh = hashU32((cfg.seed ^ Math.imul(cfg.runOffset + r, 0x9e3779b9) ^
          Math.imul(runU[rb + 3], 0x85ebca6b) ^ Math.imul(spawned, 0xc2b2ae35)) >>> 0);
        var spos = edgePosition(sh, cfg.worldW, cfg.worldH);
        eb = (r * N + slot) * E_STRIDE;
        enemies[eb] = spos[0];
        enemies[eb + 1] = spos[1];
        enemies[eb + 2] = 0;
        enemies[eb + 3] = 0;
        enemies[eb + 4] = cfg.enemyHp;
        enemies[eb + 5] = 1;
        enemies[eb + 6] = 0;
        enemies[eb + 7] = 0;
        spawned++;
      }
      runF[rb + 2] = acc;
      runU[rb + 5] += spawned;
    }

    // 8. Run status, contact damage, counters.
    for (r = 0; r < R; r++) {
      rb = r * RUN_STRIDE;
      var alive = 0;
      var kills = 0;
      var dmg = 0;
      hp = runF[rb];
      var over = runU[rb + 4];
      if (over === 0) {
        for (j = 0; j < N; j++) {
          eb = (r * N + j) * E_STRIDE;
          if (enemies[eb + 5] === 0) continue;
          alive++;
          kills += enemies[eb + 6];
          dmg += enemies[eb + 7];
          dx = enemies[eb] - px;
          dy = enemies[eb + 1] - py;
          if (dx * dx + dy * dy < playerR2) hp -= cfg.enemyDmg;
        }
      }
      if (hp <= 0) {
        hp = 0;
        over = 1;
        runU[rb + 4] = 1;
      }
      runU[rb + 1] += kills;
      runF[rb + 1] += dmg;
      runF[rb] = hp;
      runU[rb + 2] = alive;
      runU[rb + 3] += 1;
      runF[rb + 3] += dt;
    }
    return state;
  }

  function runCpu(state, steps) {
    for (var s = 0; s < steps; s++) stepCpu(state);
    return state;
  }

  function metrics(state) {
    var cfg = state.cfg;
    var runU = state.runU;
    var runF = state.runF;
    var runs = [];
    var totalKills = 0;
    var totalDamage = 0;
    var totalAlive = 0;
    var totalSteps = 0;
    var over = 0;
    for (var r = 0; r < cfg.runs; r++) {
      var rb = r * RUN_STRIDE;
      var row = {
        hp: runF[rb],
        kills: runU[rb + 1],
        alive: runU[rb + 2],
        steps: runU[rb + 3],
        over: runU[rb + 4],
        damage: runF[rb + 1],
        spawned: runU[rb + 5]
      };
      runs.push(row);
      totalKills += row.kills;
      totalDamage += row.damage;
      totalAlive += row.alive;
      totalSteps += row.steps;
      over += row.over ? 1 : 0;
    }
    return {
      runs: runs,
      totalKills: totalKills,
      totalDamage: totalDamage,
      totalAlive: totalAlive,
      totalSteps: totalSteps,
      over: over,
      count: cfg.runs
    };
  }

  function hashState(state) {
    var h = 2166136261 >>> 0;
    function mixU32(v) {
      h = (h ^ (v >>> 0)) >>> 0;
      h = Math.imul(h, 16777619) >>> 0;
    }
    function mixF32(v) {
      mixU32(Math.round(v * 1000) >>> 0);
    }
    var i;
    for (i = 0; i < state.runU.length; i++) mixU32(state.runU[i]);
    for (i = 0; i < state.runF.length; i++) mixF32(state.runF[i]);
    for (i = 0; i < state.enemies.length; i++) mixF32(state.enemies[i]);
    for (i = 0; i < state.projectiles.length; i++) mixF32(state.projectiles[i]);
    return h;
  }

  function combineMetrics(parts) {
    var out = {
      runs: [],
      totalKills: 0,
      totalDamage: 0,
      totalAlive: 0,
      totalSteps: 0,
      over: 0,
      count: 0
    };
    parts.forEach(function (m) {
      out.totalKills += m.totalKills || 0;
      out.totalDamage += m.totalDamage || 0;
      out.totalAlive += m.totalAlive || 0;
      out.totalSteps += m.totalSteps || 0;
      out.over += m.over || 0;
      out.count += m.count || 0;
      if (m.runs) out.runs = out.runs.concat(m.runs);
    });
    return out;
  }

  // ---------------------------------------------------------------------------
  // WGSL kernel
  // ---------------------------------------------------------------------------

  var SHADER = /* wgsl */ `
struct Params {
  runs: u32,
  max_enemies: u32,
  max_projectiles: u32,
  seed: u32,
  spawn_rate: f32,
  enemy_speed: f32,
  enemy_hp: f32,
  enemy_dmg: f32,
  proj_speed: f32,
  proj_dmg: f32,
  proj_radius: f32,
  enemy_radius: f32,
  player_radius: f32,
  world_w: f32,
  world_h: f32,
  dt: f32,
  sep_radius: f32,
  sep_force: f32,
  player_x: f32,
  player_y: f32,
  step: u32,
  run_offset: u32,
  pad0: u32,
  pad1: u32,
};

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read_write> run_u: array<u32>;
@group(0) @binding(2) var<storage, read_write> run_f: array<f32>;
@group(0) @binding(3) var<storage, read_write> enemies: array<f32>;
@group(0) @binding(4) var<storage, read_write> projectiles: array<f32>;

fn e_idx(r: u32, i: u32) -> u32 { return (r * P.max_enemies + i) * 8u; }
fn p_idx(r: u32, i: u32) -> u32 { return (r * P.max_projectiles + i) * 8u; }

fn hash_u32(x0: u32) -> u32 {
  var x = x0;
  x = (x ^ (x >> 16u)) * 0x7feb352du;
  x = (x ^ (x >> 15u)) * 0x846ca68bu;
  return x ^ (x >> 16u);
}

fn rand01(x: u32) -> f32 {
  return f32(hash_u32(x) >> 8u) / 16777216.0;
}

fn next_rand(r: u32) -> f32 {
  let b = r * 8u;
  let s = run_u[b] * 1664525u + 1013904223u;
  run_u[b] = s;
  return f32(s >> 8u) / 16777216.0;
}

fn edge_pos(h: u32) -> vec2<f32> {
  let edge = h % 4u;
  let t = rand01(h + 1u);
  if (edge == 0u) { return vec2<f32>(t * P.world_w, -20.0); }
  if (edge == 1u) { return vec2<f32>(t * P.world_w, P.world_h + 20.0); }
  if (edge == 2u) { return vec2<f32>(-20.0, t * P.world_h); }
  return vec2<f32>(P.world_w + 20.0, t * P.world_h);
}

@compute @workgroup_size(64)
fn respawn_projectiles(@builtin(global_invocation_id) gid: vec3<u32>) {
  let n = gid.x;
  if (n >= P.runs * P.max_projectiles) { return; }
  let r = n / P.max_projectiles;
  let i = n % P.max_projectiles;
  let rb = r * 8u;
  if (run_u[rb + 4u] != 0u) { return; }
  let pb = p_idx(r, i);
  if (projectiles[pb + 5u] != 0.0) { return; }
  let h = hash_u32(P.seed ^ ((P.run_offset + r) * 0x9e3779b9u) ^ (i * 0x85ebca6bu) ^ (run_u[rb + 3u] * 0xc2b2ae35u));
  let a = rand01(h) * 6.283185307179586;
  projectiles[pb + 0u] = P.player_x;
  projectiles[pb + 1u] = P.player_y;
  projectiles[pb + 2u] = cos(a) * P.proj_speed;
  projectiles[pb + 3u] = sin(a) * P.proj_speed;
  projectiles[pb + 4u] = P.proj_dmg;
  projectiles[pb + 5u] = 1.0;
  projectiles[pb + 6u] = -1.0;
}

@compute @workgroup_size(64)
fn move_enemies(@builtin(global_invocation_id) gid: vec3<u32>) {
  let n = gid.x;
  if (n >= P.runs * P.max_enemies) { return; }
  let r = n / P.max_enemies;
  let i = n % P.max_enemies;
  let rb = r * 8u;
  if (run_u[rb + 4u] != 0u) { return; }
  let eb = e_idx(r, i);
  if (enemies[eb + 5u] == 0.0) { return; }
  let x = enemies[eb + 0u];
  let y = enemies[eb + 1u];
  let dx = P.player_x - x;
  let dy = P.player_y - y;
  let d = max(sqrt(dx * dx + dy * dy), 1.0);
  var sx = 0.0;
  var sy = 0.0;
  let sep_r2 = P.sep_radius * P.sep_radius;
  for (var j = 0u; j < P.max_enemies; j++) {
    if (j == i) { continue; }
    let ob = e_idx(r, j);
    if (enemies[ob + 5u] == 0.0) { continue; }
    let odx = x - enemies[ob + 0u];
    let ody = y - enemies[ob + 1u];
    let od2 = odx * odx + ody * ody;
    if (od2 > 0.0001 && od2 < sep_r2) {
      let inv = P.sep_force / od2;
      sx += odx * inv;
      sy += ody * inv;
    }
  }
  let nx = x + (dx / d * P.enemy_speed + sx) * P.dt;
  let ny = y + (dy / d * P.enemy_speed + sy) * P.dt;
  enemies[eb + 0u] = clamp(nx, -100.0, P.world_w + 100.0);
  enemies[eb + 1u] = clamp(ny, -100.0, P.world_h + 100.0);
  enemies[eb + 2u] = dx / d * P.enemy_speed + sx;
  enemies[eb + 3u] = dy / d * P.enemy_speed + sy;
}

@compute @workgroup_size(64)
fn move_projectiles(@builtin(global_invocation_id) gid: vec3<u32>) {
  let n = gid.x;
  if (n >= P.runs * P.max_projectiles) { return; }
  let r = n / P.max_projectiles;
  let i = n % P.max_projectiles;
  let rb = r * 8u;
  if (run_u[rb + 4u] != 0u) { return; }
  let pb = p_idx(r, i);
  if (projectiles[pb + 5u] == 0.0) { return; }
  let x = projectiles[pb + 0u] + projectiles[pb + 2u] * P.dt;
  let y = projectiles[pb + 1u] + projectiles[pb + 3u] * P.dt;
  projectiles[pb + 0u] = x;
  projectiles[pb + 1u] = y;
  if (x < -20.0 || x > P.world_w + 20.0 || y < -20.0 || y > P.world_h + 20.0) {
    projectiles[pb + 5u] = 0.0;
  }
}

@compute @workgroup_size(64)
fn projectile_hits(@builtin(global_invocation_id) gid: vec3<u32>) {
  let n = gid.x;
  if (n >= P.runs * P.max_projectiles) { return; }
  let r = n / P.max_projectiles;
  let i = n % P.max_projectiles;
  let rb = r * 8u;
  if (run_u[rb + 4u] != 0u) { return; }
  let pb = p_idx(r, i);
  if (projectiles[pb + 5u] == 0.0) { return; }
  let hit_r = P.proj_radius + P.enemy_radius;
  let hit_r2 = hit_r * hit_r;
  var hit = -1.0;
  for (var j = 0u; j < P.max_enemies; j++) {
    let eb = e_idx(r, j);
    if (enemies[eb + 5u] == 0.0) { continue; }
    let dx = projectiles[pb + 0u] - enemies[eb + 0u];
    let dy = projectiles[pb + 1u] - enemies[eb + 1u];
    if (dx * dx + dy * dy < hit_r2) {
      hit = f32(j);
      break;
    }
  }
  projectiles[pb + 6u] = hit;
}

@compute @workgroup_size(64)
fn apply_hits(@builtin(global_invocation_id) gid: vec3<u32>) {
  let n = gid.x;
  if (n >= P.runs * P.max_enemies) { return; }
  let r = n / P.max_enemies;
  let j = n % P.max_enemies;
  let rb = r * 8u;
  if (run_u[rb + 4u] != 0u) { return; }
  let eb = e_idx(r, j);
  if (enemies[eb + 5u] == 0.0) { return; }
  var sum = 0.0;
  for (var i = 0u; i < P.max_projectiles; i++) {
    let pb = p_idx(r, i);
    if (projectiles[pb + 5u] != 0.0 && projectiles[pb + 6u] == f32(j)) {
      sum += projectiles[pb + 4u];
    }
  }
  if (sum > 0.0) {
    let hp = enemies[eb + 4u] - sum;
    enemies[eb + 7u] = sum;
    if (hp <= 0.0) {
      enemies[eb + 4u] = 0.0;
      enemies[eb + 5u] = 0.0;
      enemies[eb + 6u] = 1.0;
    } else {
      enemies[eb + 4u] = hp;
      enemies[eb + 6u] = 0.0;
    }
  } else {
    enemies[eb + 6u] = 0.0;
    enemies[eb + 7u] = 0.0;
  }
}

@compute @workgroup_size(64)
fn projectile_kill(@builtin(global_invocation_id) gid: vec3<u32>) {
  let n = gid.x;
  if (n >= P.runs * P.max_projectiles) { return; }
  let r = n / P.max_projectiles;
  let i = n % P.max_projectiles;
  let rb = r * 8u;
  if (run_u[rb + 4u] != 0u) { return; }
  let pb = p_idx(r, i);
  if (projectiles[pb + 6u] >= 0.0) {
    projectiles[pb + 5u] = 0.0;
    projectiles[pb + 6u] = -1.0;
  }
}

@compute @workgroup_size(64)
fn spawn(@builtin(global_invocation_id) gid: vec3<u32>) {
  let r = gid.x;
  if (r >= P.runs) { return; }
  let rb = r * 8u;
  if (run_u[rb + 4u] != 0u) { return; }
  var acc = run_f[rb + 2u] + P.spawn_rate * P.dt;
  var spawned = 0u;
  loop {
    if (acc < 1.0 || spawned >= 8u) { break; }
    var slot = 0xffffffffu;
    for (var i = 0u; i < P.max_enemies; i++) {
      if (enemies[e_idx(r, i) + 5u] == 0.0) { slot = i; break; }
    }
    if (slot == 0xffffffffu) { break; }
    acc -= 1.0;
    let h = hash_u32(P.seed ^ ((P.run_offset + r) * 0x9e3779b9u) ^ (run_u[rb + 3u] * 0x85ebca6bu) ^ (spawned * 0xc2b2ae35u));
    let pos = edge_pos(h);
    let eb = e_idx(r, slot);
    enemies[eb + 0u] = pos.x;
    enemies[eb + 1u] = pos.y;
    enemies[eb + 2u] = 0.0;
    enemies[eb + 3u] = 0.0;
    enemies[eb + 4u] = P.enemy_hp;
    enemies[eb + 5u] = 1.0;
    enemies[eb + 6u] = 0.0;
    enemies[eb + 7u] = 0.0;
    spawned += 1u;
  }
  run_f[rb + 2u] = acc;
  run_u[rb + 5u] += spawned;
}

@compute @workgroup_size(64)
fn status(@builtin(global_invocation_id) gid: vec3<u32>) {
  let r = gid.x;
  if (r >= P.runs) { return; }
  let rb = r * 8u;
  var alive = 0u;
  var kills = 0u;
  var dmg = 0u;
  var hp = run_f[rb + 0u];
  var over = run_u[rb + 4u];
  if (over == 0u) {
    for (var j = 0u; j < P.max_enemies; j++) {
      let eb = e_idx(r, j);
      if (enemies[eb + 5u] == 0.0) { continue; }
      alive += 1u;
      kills += u32(enemies[eb + 6u]);
      dmg += u32(enemies[eb + 7u]);
      let dx = enemies[eb + 0u] - P.player_x;
      let dy = enemies[eb + 1u] - P.player_y;
      if (dx * dx + dy * dy < P.player_radius * P.player_radius) {
        hp = hp - P.enemy_dmg;
      }
    }
  }
  if (hp <= 0.0) {
    hp = 0.0;
    over = 1u;
    run_u[rb + 4u] = 1u;
  }
  run_u[rb + 1u] += kills;
  run_f[rb + 1u] += f32(dmg);
  run_f[rb + 0u] = hp;
  run_u[rb + 2u] = alive;
  run_u[rb + 3u] += 1u;
  run_f[rb + 3u] += P.dt;
}
`;

  var PASSES = [
    ['respawn_projectiles', function (cfg) { return cfg.runs * cfg.maxProjectiles; }],
    ['move_enemies', function (cfg) { return cfg.runs * cfg.maxEnemies; }],
    ['move_projectiles', function (cfg) { return cfg.runs * cfg.maxProjectiles; }],
    ['projectile_hits', function (cfg) { return cfg.runs * cfg.maxProjectiles; }],
    ['apply_hits', function (cfg) { return cfg.runs * cfg.maxEnemies; }],
    ['projectile_kill', function (cfg) { return cfg.runs * cfg.maxProjectiles; }],
    ['spawn', function (cfg) { return cfg.runs; }],
    ['status', function (cfg) { return cfg.runs; }]
  ];

  function makeParams(cfg) {
    var ab = new ArrayBuffer(96);
    var u = new Uint32Array(ab);
    var f = new Float32Array(ab);
    u[0] = cfg.runs;
    u[1] = cfg.maxEnemies;
    u[2] = cfg.maxProjectiles;
    u[3] = cfg.seed;
    f[4] = cfg.spawnRate;
    f[5] = cfg.enemySpeed;
    f[6] = cfg.enemyHp;
    f[7] = cfg.enemyDmg;
    f[8] = cfg.projSpeed;
    f[9] = cfg.projDmg;
    f[10] = cfg.projRadius;
    f[11] = cfg.enemyRadius;
    f[12] = cfg.playerRadius;
    f[13] = cfg.worldW;
    f[14] = cfg.worldH;
    f[15] = cfg.dt;
    f[16] = cfg.sepRadius;
    f[17] = cfg.sepForce;
    f[18] = cfg.worldW * 0.5;
    f[19] = cfg.worldH * 0.5;
    u[20] = 0;
    u[21] = cfg.runOffset;
    u[22] = 0;
    u[23] = 0;
    return ab;
  }

  function GpuEngine() {
    this.device = null;
    this.adapter = null;
    this.cfg = null;
    this.state = null;
    this.buffers = null;
    this.bindGroup = null;
    this.pipelines = null;
    this.uniform = null;
    this.destroyed = false;
  }

  GpuEngine.prototype.init = function (cfg, state) {
    var self = this;
    if (!root.navigator || !root.navigator.gpu) {
      return Promise.reject(new Error('WebGPU is not available in this browser'));
    }
    return root.navigator.gpu.requestAdapter({ powerPreference: 'high-performance' })
      .then(function (adapter) {
        if (!adapter) {
          return root.navigator.gpu.requestAdapter({ forceFallbackAdapter: true }).then(function (fallback) {
            if (!fallback) throw new Error('No WebGPU adapter (hardware or fallback)');
            return fallback;
          });
        }
        return adapter;
      })
      .then(function (adapter) {
        self.adapter = adapter;
        return adapter.requestDevice();
      })
      .then(function (device) {
        self.device = device;
        self.cfg = withDefaults(cfg);
        self.state = state || createState(self.cfg);
        if (!state) initState(self.state);
        self._createBuffers();
        self._createPipelines();
        return self;
      });
  };

  GpuEngine.prototype._createBuffers = function () {
    var device = this.device;
    var state = this.state;
    function upload(data) {
      var buf = device.createBuffer({
        size: Math.max(4, data.byteLength),
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST,
        mappedAtCreation: true
      });
      if (data.byteLength) {
        new data.constructor(buf.getMappedRange()).set(data);
      }
      buf.unmap();
      return buf;
    }
    this.buffers = {
      runU: upload(state.runU),
      runF: upload(state.runF),
      enemies: upload(state.enemies),
      projectiles: upload(state.projectiles)
    };
    this.uniform = device.createBuffer({
      size: 96,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST
    });
    device.queue.writeBuffer(this.uniform, 0, makeParams(this.cfg));
  };

  GpuEngine.prototype._createPipelines = function () {
    var device = this.device;
    var module = device.createShaderModule({ code: SHADER });
    var layout = device.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'uniform' } },
        { binding: 1, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'storage' } },
        { binding: 2, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'storage' } },
        { binding: 3, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'storage' } },
        { binding: 4, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'storage' } }
      ]
    });
    var pipelineLayout = device.createPipelineLayout({ bindGroupLayouts: [layout] });
    this.bindGroup = device.createBindGroup({
      layout: layout,
      entries: [
        { binding: 0, resource: { buffer: this.uniform } },
        { binding: 1, resource: { buffer: this.buffers.runU } },
        { binding: 2, resource: { buffer: this.buffers.runF } },
        { binding: 3, resource: { buffer: this.buffers.enemies } },
        { binding: 4, resource: { buffer: this.buffers.projectiles } }
      ]
    });
    this.pipelines = {};
    for (var i = 0; i < PASSES.length; i++) {
      this.pipelines[PASSES[i][0]] = device.createComputePipeline({
        layout: pipelineLayout,
        compute: { module: module, entryPoint: PASSES[i][0] }
      });
    }
  };

  GpuEngine.prototype.encodeStep = function (encoder) {
    var pass = encoder.beginComputePass();
    pass.setBindGroup(0, this.bindGroup);
    for (var i = 0; i < PASSES.length; i++) {
      var name = PASSES[i][0];
      var count = PASSES[i][1](this.cfg);
      if (count <= 0) continue;
      pass.setPipeline(this.pipelines[name]);
      pass.dispatchWorkgroups(Math.ceil(count / WG));
    }
    pass.end();
  };

  GpuEngine.prototype.run = function (steps) {
    var self = this;
    var t0 = performance.now();
    var encoder = this.device.createCommandEncoder();
    for (var s = 0; s < steps; s++) this.encodeStep(encoder);
    this.device.queue.submit([encoder.finish()]);
    return this.device.queue.onSubmittedWorkDone().then(function () {
      return { ms: performance.now() - t0, steps: steps };
    });
  };

  GpuEngine.prototype.readBuffers = function () {
    var device = this.device;
    var b = this.buffers;
    var sizes = [
      this.state.runU.byteLength,
      this.state.runF.byteLength,
      this.state.enemies.byteLength,
      this.state.projectiles.byteLength
    ];
    var total = sizes.reduce(function (a, v) { return a + v; }, 0);
    var staging = device.createBuffer({
      size: total,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ
    });
    var encoder = device.createCommandEncoder();
    var offset = 0;
    var srcs = [b.runU, b.runF, b.enemies, b.projectiles];
    for (var i = 0; i < srcs.length; i++) {
      encoder.copyBufferToBuffer(srcs[i], 0, staging, offset, sizes[i]);
      offset += sizes[i];
    }
    device.queue.submit([encoder.finish()]);
    return staging.mapAsync(GPUMapMode.READ).then(function () {
      var data = new Uint8Array(staging.getMappedRange());
      var out = {
        runU: new Uint32Array(data.buffer.slice(data.byteOffset, data.byteOffset + sizes[0])),
        runF: new Float32Array(data.buffer.slice(data.byteOffset + sizes[0], data.byteOffset + sizes[0] + sizes[1])),
        enemies: new Float32Array(data.buffer.slice(data.byteOffset + sizes[0] + sizes[1], data.byteOffset + sizes[0] + sizes[1] + sizes[2])),
        projectiles: new Float32Array(data.buffer.slice(data.byteOffset + sizes[0] + sizes[1] + sizes[2]))
      };
      staging.unmap();
      staging.destroy();
      return out;
    });
  };

  GpuEngine.prototype.readState = function () {
    var cfg = this.cfg;
    return this.readBuffers().then(function (b) {
      return {
        cfg: cfg,
        runU: b.runU,
        runF: b.runF,
        enemies: b.enemies,
        projectiles: b.projectiles
      };
    });
  };

  GpuEngine.prototype.destroy = function () {
    if (this.destroyed) return;
    this.destroyed = true;
    var b = this.buffers || {};
    Object.keys(b).forEach(function (k) {
      try { b[k].destroy(); } catch (e) {}
    });
    try { if (this.uniform) this.uniform.destroy(); } catch (e) {}
    try { if (this.device) this.device.destroy(); } catch (e) {}
  };

  function createGpuEngine(cfg, state) {
    var engine = new GpuEngine();
    return engine.init(cfg, state);
  }

  // ---------------------------------------------------------------------------
  // CPU worker pool
  // ---------------------------------------------------------------------------

  function runCpuWorkers(cfg, workerCount, steps, workerUrl) {
    cfg = withDefaults(cfg);
    if (!cfg.runs) {
      return Promise.resolve({
        metrics: combineMetrics([]),
        ms: 0,
        workers: 0,
        runs: 0
      });
    }
    var n = Math.max(1, Math.min(workerCount || 1, cfg.runs));
    var chunks = [];
    var base = Math.floor(cfg.runs / n);
    var extra = cfg.runs % n;
    var start = 0;
    for (var i = 0; i < n; i++) {
      var count = base + (i < extra ? 1 : 0);
      if (!count) continue;
      chunks.push({
        cfg: Object.assign({}, cfg, {
          runs: count,
          runOffset: (cfg.runOffset + start) >>> 0
        }),
        runs: count
      });
      start += count;
    }
    var t0 = performance.now();
    var jobs = chunks.map(function (chunk) {
      return new Promise(function (resolve, reject) {
        var worker = new Worker(workerUrl || 'gpu-cpu-worker.js');
        worker.onmessage = function (e) {
          worker.terminate();
          if (e.data && e.data.error) reject(new Error(e.data.error));
          else resolve(e.data.result);
        };
        worker.onerror = function (e) {
          worker.terminate();
          reject(new Error(e.message || 'CPU worker failed'));
        };
        worker.postMessage({ cfg: chunk.cfg, steps: steps });
      });
    });
    return Promise.all(jobs).then(function (parts) {
      var ms = performance.now() - t0;
      var metrics = combineMetrics(parts.map(function (p) { return p.metrics; }));
      return {
        metrics: metrics,
        ms: ms,
        workers: chunks.length,
        runs: cfg.runs,
        hashes: parts.map(function (p) { return p.hash; })
      };
    });
  }

  function runCpuSingle(cfg, steps) {
    var state = createState(cfg);
    initState(state);
    var t0 = performance.now();
    runCpu(state, steps);
    var ms = performance.now() - t0;
    return {
      metrics: metrics(state),
      ms: ms,
      runs: state.cfg.runs,
      hash: hashState(state),
      steps: steps,
      state: state
    };
  }

  // ---------------------------------------------------------------------------
  // Hybrid scheduler
  // ---------------------------------------------------------------------------

  function runHybrid(cfg, opts) {
    cfg = withDefaults(cfg);
    opts = opts || {};
    var steps = opts.steps || 60;
    var gpuRuns = Math.max(0, Math.min(cfg.runs, opts.gpuRuns | 0));
    var cpuRuns = cfg.runs - gpuRuns;
    var t0 = performance.now();

    var gpuCfg = Object.assign({}, cfg, { runs: gpuRuns, runOffset: cfg.runOffset });
    var cpuCfg = Object.assign({}, cfg, {
      runs: cpuRuns,
      runOffset: (cfg.runOffset + gpuRuns) >>> 0
    });

    var gpuJob = gpuRuns > 0
      ? createGpuEngine(gpuCfg).then(function (engine) {
          return engine.run(steps).then(function (timing) {
            return engine.readState().then(function (state) {
              var out = {
            metrics: metrics(state),
            ms: timing.ms,
            runs: gpuRuns,
            hash: hashState(state)
          };
              engine.destroy();
              return out;
            });
          });
        })
      : Promise.resolve({ metrics: combineMetrics([]), ms: 0, runs: 0, hash: 0 });

    var cpuJob = cpuRuns > 0
      ? runCpuWorkers(cpuCfg, opts.workers || 1, steps)
      : Promise.resolve({ metrics: combineMetrics([]), ms: 0, runs: 0, workers: 0, hashes: [] });

    return Promise.all([gpuJob, cpuJob]).then(function (res) {
      var wall = performance.now() - t0;
      var gpu = res[0];
      var cpu = res[1];
      return {
        metrics: combineMetrics([gpu.metrics, cpu.metrics]),
        wallMs: wall,
        gpu: gpu,
        cpu: cpu,
        runs: cfg.runs,
        steps: steps,
        gpuRuns: gpuRuns,
        cpuRuns: cpuRuns
      };
    });
  }

  function agentInfo() {
    return {
      hardwareConcurrency: root.navigator ? (root.navigator.hardwareConcurrency || 0) : 0,
      userAgent: root.navigator ? root.navigator.userAgent : '',
      hasWebGpu: !!(root.navigator && root.navigator.gpu)
    };
  }

  function adapterInfo() {
    if (!root.navigator || !root.navigator.gpu) {
      return Promise.resolve({ hasWebGpu: false });
    }
    return root.navigator.gpu.requestAdapter({ powerPreference: 'high-performance' })
      .then(function (a) {
        if (a) return a;
        return root.navigator.gpu.requestAdapter({ forceFallbackAdapter: true });
      })
      .then(function (a) {
        if (!a) return { hasWebGpu: true, adapter: null };
        return {
          hasWebGpu: true,
          adapter: a.info ? {
            vendor: a.info.vendor,
            architecture: a.info.architecture,
            device: a.info.device,
            description: a.info.description
          } : null,
          features: Array.from(a.features || []),
          limits: {
            maxComputeWorkgroupStorageSize: a.limits.maxComputeWorkgroupStorageSize,
            maxComputeInvocationsPerWorkgroup: a.limits.maxComputeInvocationsPerWorkgroup,
            maxStorageBufferBindingSize: a.limits.maxStorageBufferBindingSize,
            maxBufferSize: a.limits.maxBufferSize
          }
        };
      })
      .catch(function (e) {
        return { hasWebGpu: true, error: String(e && e.message || e) };
      });
  }

  /**
   * Small CPU/GPU equivalence check. Runs one step from an identical initial
   * state and compares per-run counters. f32 kernels are not expected to be
   * bit-identical across vendors, so metrics are compared with a tolerance.
   */
  function validate(cfg) {
    cfg = withDefaults(Object.assign({}, cfg || {}, {
      runs: 2,
      maxEnemies: 32,
      maxProjectiles: 16
    }));
    var state = createState(cfg);
    initState(state);
    var cpuState = {
      cfg: cfg,
      runU: state.runU.slice(),
      runF: state.runF.slice(),
      enemies: state.enemies.slice(),
      projectiles: state.projectiles.slice()
    };
    stepCpu(cpuState);
    var cpuMetrics = metrics(cpuState);
    return createGpuEngine(cfg, state).then(function (engine) {
      return engine.run(1)
        .then(function () { return engine.readState(); })
        .then(function (gpuState) {
          var gpuMetrics = metrics(gpuState);
          var diffs = [];
          for (var r = 0; r < cfg.runs; r++) {
            var a = cpuMetrics.runs[r];
            var b = gpuMetrics.runs[r];
            ['hp', 'kills', 'alive', 'damage', 'steps', 'over'].forEach(function (k) {
              var av = a[k];
              var bv = b[k];
              if (typeof av === 'number' && Math.abs(av - bv) > 0.01) {
                diffs.push({ run: r, field: k, cpu: av, gpu: bv });
              }
            });
          }
          engine.destroy();
          return {
            ok: diffs.length === 0,
            diffs: diffs,
            cpu: cpuMetrics,
            gpu: gpuMetrics
          };
        });
    });
  }

  var api = {
    DEFAULTS: DEFAULTS,
    withDefaults: withDefaults,
    createState: createState,
    initState: initState,
    stepCpu: stepCpu,
    runCpu: runCpu,
    runCpuSingle: runCpuSingle,
    runCpuWorkers: runCpuWorkers,
    metrics: metrics,
    hashState: hashState,
    combineMetrics: combineMetrics,
    createGpuEngine: createGpuEngine,
    runHybrid: runHybrid,
    validate: validate,
    agentInfo: agentInfo,
    adapterInfo: adapterInfo
  };

  root.CrownGpuSim = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
