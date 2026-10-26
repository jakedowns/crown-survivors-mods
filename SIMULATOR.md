# Crown Survivors — Simulator Documentation

## Overview

The Crown Survivors Simulator (`public/simulator.html`) is a multi-instance batch testing and balancing tool. It runs the **real game engine** from `public/index.html` inside iframes (single source of truth) and drives it through `postMessage`, so anything you change in the game is immediately reflected in every simulated run.

It supports:

- 1–16 concurrent visual instances with automatic grid layout
- Time scales from 0.5x up to **MAX** (as fast as the frame loop allows)
- Auto-scale profiles that pick a stable speed for the current instance count
- **Build Analyzer** ranking profiles (“preferential voting” for card picks)
- **Evolutionary build search** that breeds ranking profiles and reports the strongest / weakest ones
- Aggregated telemetry, graphs, per-instance details, event log, and JSON export

The goal is to get fast, comparable signal on which builds, card orders, and strategies perform best so balance, content, and art direction can be steered toward the most enjoyable play.

---

## Quick Start

1. Serve the `public/` directory (any static server) and open `simulator.html`.
2. Pick the number of instances (default 4) and a time scale (or an auto-scale mode).
3. Click **Auto All** so the AI plays, then **Start All**.
4. Watch aggregate stats, graphs, and per-instance details.
5. For build/meta work, use the **Build Analyzer** panel (see below).
6. Export JSON when done.

> Local example: `python3 -m http.server 8765 --directory public` then open `http://localhost:8765/simulator.html`.

---

## Controls

### Instances
Select 1, 2, 4, 6, 8, 9 or 16 instances. The grid re-lays-out automatically (1×1, 2×1, 2×2, 3×2, 3×3, 4×N).

Changing the instance count stops an active evolution run, rebuilds the iframes, re-applies the current time scale, and continues with the new count.

### Time Scale
| Value | Behaviour |
| --- | --- |
| 0.5x, 1x, 2x, 4x, 10x | Real-time multiplier |
| 100x | ~100 game-seconds per wall-second |
| MAX (`999`) | As fast as the frame loop allows |

All speeds use a **fixed 1/60 s simulation step** with an accumulator. High speeds run many fixed steps per frame instead of enlarging `dt`, which keeps collisions, projectile movement, and enemy AI accurate enough for balance analysis (`MAX` = 200 steps/frame ≈ 3.3 game-seconds per frame).

### Auto Scale
`Off | Low | Med | High`

Picks a conservative time scale based on the number of instances to avoid melting the machine:

| Mode | ≤4 | ≤6 | ≤9 | >9 |
| --- | --- | --- | --- | --- |
| Low | 2x | 1x | 1x | 1x |
| Med | 4x | 2x | 1x | 1x |
| High | 10x | 4x | 2x | 1x |

Auto-scale is disabled while an evolution run is active (the evolution config controls speed).

### Playback Buttons
- **Start All** — start every instance
- **Pause All** — pause every instance
- **Auto All** — enable AI (movement, dodging, gem pickup, dash, walls, card picks)
- **Reset All** — rebuild the grid with fresh runs
- **Export Data** — download JSON (instances, aggregates, game metadata, evolution history)

---

## Build Analyzer

The Build Analyzer controls how the AI values cards when a level-up choice appears.

### Ranking profiles
Each item has:

- **Rank** — `1` is the highest priority
- **Weight** — a multiplier (layer “opacity/influence”) applied on top of the rank

Card selection works in two stages:

1. `cards()` offers cards using weights derived from rank + weight (low rank ⇒ more likely to be offered).
2. `simPickCardIndex()` chooses which offered card to take using the same rank + weight. Evolutions get a large bonus, then the highest scoring card wins (with tiny jitter to break ties).

Presets:

- **Default (Random)** — clears the profile; uniform random picks
- **Damage Focus** — Might/Tome/Area and damaging weapons first
- **Survival Focus** — Armor/Heart/Area/Boots first
- **Speed Focus** — Boots/Magnet/Area, mobile weapons
- **Balanced Meta** — moderate all-round order
- **Evolution Chaser** — passives that unlock evolutions first
- **Custom Profile** — edit every weapon/passive rank and weight

Profiles apply to all instances via `setBuildProfile`. Use **Apply to All Instances** after editing a custom profile.

### Evolutionary search
The analyzer can breed ranking profiles automatically.

| Input | Meaning |
| --- | --- |
| **Pop** | Population size (2–64) |
| **Gens** | Number of generations |
| **Runs** | Runs per candidate per generation (averaged) |
| **Limit s** | Game-time limit per run (a clean cutoff, exact to the step) |
| **Speed** | Evaluation time scale (10x / 100x / MAX) |
| **Fitness weights** | Time, Kills, Level, Damage |

**Algorithm**

1. Seed a population of random ranking permutations + random weights.
2. Assign candidates to idle instances; each run resets the game, applies the candidate profile, enables auto-play, and stops exactly at the time limit (or at death).
3. Fitness = weighted average over runs:
   `wTime·time + wKills·kills + wLevel·level·10 + wDmg·damage + 500·win`
4. Sort by fitness. Keep the top 25% as elites. Fill the rest with tournament-selected parents, uniform rank/weight crossover, then mutate (rank swaps, random rank jumps, weight nudges) and re-normalize ranks to a permutation.
5. Repeat until the generation budget is exhausted.

Results are shown as a leaderboard per generation:

- Fitness
- Top-5 priority items (with non-default weights)
- Average time / kills / level
- **Use build** — loads that genome into the custom profile editor and applies it to all instances

Evolution history (including per-run results) is included in the JSON export.

### Auto-play card choice
Runs use `simPickCardIndex()`, so the active profile meaningfully changes which upgrades are taken. Without a profile, picks are uniform random.

---

## Telemetry & Analytics

### Aggregate stats
Average time, kills, level, damage; active runs; total kills across all instances.

### Graphs
DPS-equivalent (damage), kills, and level over time (last 60 samples).

### Instance details
Per-instance time, kills, level, HP, death state.

### Event log
Timestamped events (loads, config changes, evolution progress, errors).

### Export
`crown-survivors-sim-<timestamp>.json` contains:

```json
{
  "timestamp": "...",
  "instances": [ ... ],
  "meta": { "weapons": [...], "passives": [...], "mods": [...] },
  "evolution": { "active": false, "generation": 3, "config": {...}, "history": [...] },
  "aggregates": { "avgKills": "...", "avgLevel": "...", "avgDamage": "..." }
}
```

---

## Architecture

### Communication
Same-origin `postMessage` between the simulator and each iframe.

**Simulator → Game**
| Message | Purpose |
| --- | --- |
| `start` | Start a run if not already playing |
| `reset` | Start a fresh run (used by evolution); accepts `auto` |
| `pause` / `resume` | Pause/resume |
| `auto` | Enable/disable AI |
| `step` | Advance one fixed step |
| `setTimeScale` | Set speed multiplier |
| `setTimeLimit` | Stop the run at a game-time limit (`0` disables) |
| `setBuildProfile` | Apply `{ name, ranks, weights }` (or `null` to clear) |
| `getState` | Request a state snapshot |
| `getMeta` | Request weapon/passive/mod metadata |

**Game → Simulator**
| Message | Purpose |
| --- | --- |
| `state` | Snapshot: time, level, HP, kills, damage, weapons, passives, `over`, `won`, … (throttled to ~12/s wall time) |
| `telemetry` | Movement metrics (throttled to ~2/s wall time) |
| `meta` | Weapon/passive/mod lists |
| `ready` | Game script initialized |

### Shared codebase
No duplicated game logic: every instance loads `index.html`. Balance changes, bug fixes, weapons, enemies, and RNG are identical to live play.

### Determinism caveat
The browser path still uses the game’s `Math.random()` and is therefore **not reproducible by seed**. Use many runs and averages for comparisons. A seeded headless backend is planned (see below).

---

## Known Fixes / Notes

- `M.choice` (the “Insight” upgrade) was missing from the default meta save, which made `cards()` return an empty list and soft-lock level-ups. It now defaults to `0`, and `cards()` also guards with `(M.choice || 0)`.
- State updates are wall-clock throttled so high speeds do not flood the simulator with `postMessage` traffic.
- A run that hits the analyzer’s time limit is paused in-game and reports its final state; the next assignment resets it.

---

## Performance Guidance

- 4 instances at 10x is comfortable on a laptop; 16 at MAX is heavy (each iframe runs 200 fixed updates/frame).
- Prefer 100x or MAX with fewer instances for evolution; prefer more instances at 4x–10x for visual comparisons.
- Keep `Runs` at 1 while exploring, then raise it to average out `Math.random()` noise.
- Evolution cost ≈ `gens × pop × runs × limit / speed`.

---

## Proposed: Faster-Than-Renderer (WASM / WebGPU)

The current MAX mode still pays for canvas drawing, DOM, and per-entity JS objects. A headless backend would remove that entirely.

### Plan
1. **WASM core (first)** — extract a render-agnostic `SimCore` (fixed-step update, spawn, weapons, collisions, XP, auto-play, card profiles) into a Rust crate (`wasm-sim/`), with a seeded PRNG (`xorshift32`/PCG), SoA typed arrays, and no DOM/audio/canvas calls. Expose `run_batch(seeds, limit, profile) -> telemetry`.
2. **Parity tests** — run the same seed/profile in JS and WASM, compare time, kills, level, damage, weapon/passive composition.
3. **Simulator integration** — “Compute Backend” selector: `JS (visual)` | `WASM (headless)`. Headless batches feed the same leaderboard/graphs.
4. **WebGPU (second)** — port the hottest kernels (enemy movement/separation, projectile hits, gem magnet) to WGSL compute; one run per workgroup lane; read back aggregate telemetry only. Target 256–4096 concurrent runs for evolution.
5. **Evolution at scale** — population 64–512, 16–64 seeds per candidate, using WASM/WebGPU for evaluation and the visual iframe mode as the oracle for spot checks.

Expected rough throughput: JS MAX ≈ 10–30 runs/s, WASM ≈ 150–500 runs/s, WebGPU ≈ 2,000–10,000+ runs/s depending on hardware and run length.

---

## Changelog

### v1.2.0 — Build Analyzer & Evolutionary Search
- Instance count selector (1–16) with dynamic grid layouts
- Auto-scale modes (Low/Med/High) keyed to instance count
- 100x and MAX time scales using fixed 1/60 steps
- Build ranking profiles with rank + weight (layer influence)
- Presets + fully custom profile editor built from live game metadata
- Profile-aware card offering (`cards()`) and card picking (`simPickCardIndex`)
- Evolutionary search: population, generations, runs/candidate, fitness weights
- Per-generation leaderboard with avg time/kills/level and “Use build”
- Exact game-time limit for evaluation runs (`setTimeLimit`)
- Wall-clock throttling for state/telemetry messages
- JSON export includes game metadata and evolution history
- Fix: missing `choice` meta default that emptied the card pool
- Fix: instance iframes now wire `onload` before `src` so readiness is reliable

### v1.1.0 — Speed & Instances
- 100x and MAX speed modes
- Instance count selector and grid layout
- Auto-scale placeholder/controls

### v1.0.0 — Initial Multi-Instance Release
- 4-instance grid, shared engine via iframes
- Aggregate telemetry, graphs, per-instance details, event log
- Time scale controls and global controls
- Auto-play AI (dash/walls/gem seeking)
- JSON export
