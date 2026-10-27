//! Crown Survivors authoritative simulation core.
//!
//! The same crate powers three runtimes:
//!
//! * the browser product (compiled to `wasm32-unknown-unknown`, raw ABI below),
//! * the multi-instance simulator's worker pool (same WASM module),
//! * native batch tools / tests (`examples/batch.rs`, `tests/`).
//!
//! There is no rendering, audio or DOM code in here. Anything visual lives in
//! the host and must never feed randomness back into the core.
//!
//! # ABI
//!
//! The module deliberately avoids `wasm-bindgen`. JSON is exchanged through two
//! shared byte buffers:
//!
//! ```text
//! sim_input_reserve(len) -> *mut u8   // caller writes `len` UTF-8 bytes here
//! sim_create() -> u32                 // handle, 0 = error
//! sim_step(handle, steps) -> u32      // 1 step per 1/60s, returns out length
//! sim_snapshot(handle, detail) -> u32
//! sim_meta() -> u32
//! sim_pick(handle, index) -> u32
//! sim_configure(handle) -> u32
//! sim_destroy(handle) -> u32
//! sim_out_ptr() -> *const u8
//! sim_out_len() -> u32
//! ```
//!
//! Every function that returns `u32` leaves a JSON payload in the output buffer
//! and returns its byte length. Errors are reported as `{"error": "..."}`.
//! `sim_create` returns `0` on error (after writing the error payload).

#![recursion_limit = "512"]

mod config;
mod data;
mod game;
mod rng;
mod sim;

pub use config::{Config, Input, MetaCfg, ModsCfg, Profile};
pub use data::*;
pub use game::{meta_json, Game};
pub use rng::Rng;

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

/// Bumped whenever simulation semantics change in a way that alters results.
/// Hosts use this to invalidate cached runs and to gate parity comparisons.
pub const CORE_VERSION: u32 = 1;

/// Largest supported single `sim_step` batch. 100x realtime for 60 seconds.
pub const MAX_STEPS_PER_CALL: u32 = 6000;

static INPUT_BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static OUTPUT_BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static NEXT_HANDLE: AtomicU32 = AtomicU32::new(1);

fn games() -> &'static Mutex<HashMap<u32, Game>> {
    static GAMES: std::sync::OnceLock<Mutex<HashMap<u32, Game>>> = std::sync::OnceLock::new();
    GAMES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn set_out(value: impl Into<String>) -> u32 {
    let bytes = value.into().into_bytes();
    let len = bytes.len() as u32;
    let mut out = OUTPUT_BUF.lock().unwrap_or_else(|e| e.into_inner());
    *out = bytes;
    len
}

fn set_err(msg: impl std::fmt::Display) -> u32 {
    set_out(json!({ "error": msg.to_string() }).to_string())
}

fn input_json() -> Result<Value, String> {
    let buf = INPUT_BUF.lock().unwrap_or_else(|e| e.into_inner());
    if buf.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_slice(&buf).map_err(|e| format!("invalid input JSON: {e}"))
}

fn with_game<T>(handle: u32, f: impl FnOnce(&mut Game) -> T) -> Result<T, String> {
    let mut games = games().lock().unwrap_or_else(|e| e.into_inner());
    let game = games
        .get_mut(&handle)
        .ok_or_else(|| format!("unknown sim handle {handle}"))?;
    Ok(f(game))
}

// ---------------------------------------------------------------------------
// Buffer ABI
// ---------------------------------------------------------------------------

/// Resize the shared input buffer and return a pointer for the host to fill.
#[no_mangle]
pub extern "C" fn sim_input_reserve(len: u32) -> *mut u8 {
    let mut buf = INPUT_BUF.lock().unwrap_or_else(|e| e.into_inner());
    buf.clear();
    buf.resize(len as usize, 0);
    buf.as_mut_ptr()
}

#[no_mangle]
pub extern "C" fn sim_out_ptr() -> *const u8 {
    let buf = OUTPUT_BUF.lock().unwrap_or_else(|e| e.into_inner());
    buf.as_ptr()
}

#[no_mangle]
pub extern "C" fn sim_out_len() -> u32 {
    let buf = OUTPUT_BUF.lock().unwrap_or_else(|e| e.into_inner());
    buf.len() as u32
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

/// Create a run from a [`Config`] JSON payload in the input buffer.
#[no_mangle]
pub extern "C" fn sim_create() -> u32 {
    let cfg: Config = match input_json().and_then(|v| {
        if v.is_null() {
            Ok(Config::default())
        } else {
            serde_json::from_value(v).map_err(|e| format!("invalid config: {e}"))
        }
    }) {
        Ok(c) => c,
        Err(e) => {
            set_err(e);
            return 0;
        }
    };
    let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
    let mut games = games().lock().unwrap_or_else(|e| e.into_inner());
    games.insert(handle, Game::new(cfg));
    set_out(json!({ "handle": handle, "version": CORE_VERSION }).to_string());
    handle
}

#[no_mangle]
pub extern "C" fn sim_destroy(handle: u32) -> u32 {
    let removed = games()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&handle)
        .is_some();
    set_out(json!({ "destroyed": removed }).to_string())
}

/// Apply a partial config update to a live run.
///
/// Only the fields that are safe to change mid-run are honoured; seeding is
/// fixed at creation so that a handle always maps to one reproducible run.
#[no_mangle]
pub extern "C" fn sim_configure(handle: u32) -> u32 {
    let patch = match input_json() {
        Ok(v) => v,
        Err(e) => return set_err(e),
    };
    let result = with_game(handle, |g| {
        if let Some(v) = patch.get("time_limit").and_then(Value::as_f64) {
            g.cfg.time_limit = v.max(0.0);
        }
        if let Some(v) = patch.get("auto").and_then(Value::as_bool) {
            g.auto = v;
            g.cfg.auto = v;
        }
        if let Some(v) = patch.get("god").and_then(Value::as_bool) {
            g.cfg.god = v;
        }
        if let Some(v) = patch.get("curse").and_then(Value::as_u64) {
            g.cfg.curse = v as u32;
        }
        if let Some(v) = patch.get("view_w").and_then(Value::as_f64) {
            g.cfg.view_w = v.max(1.0);
        }
        if let Some(v) = patch.get("view_h").and_then(Value::as_f64) {
            g.cfg.view_h = v.max(1.0);
        }
        if let Some(v) = patch.get("max_enemies").and_then(Value::as_u64) {
            g.cfg.max_enemies = (v as usize).max(1);
        }
        if let Some(v) = patch.get("profile") {
            g.cfg.profile = if v.is_null() {
                None
            } else {
                serde_json::from_value::<Profile>(v.clone()).ok()
            };
        }
        g.stats = g.compute_stats();
        json!({
            "t": g.t,
            "timeLimit": g.cfg.time_limit,
            "auto": g.auto,
            "ended": g.ended,
        })
        .to_string()
    });
    match result {
        Ok(s) => set_out(s),
        Err(e) => set_err(e),
    }
}

// ---------------------------------------------------------------------------
// Simulation
// ---------------------------------------------------------------------------

/// Advance `steps` fixed 1/60s steps.
///
/// Input JSON is an [`Input`]. `dash`, `wall` and `pick` are edge-triggered and
/// therefore only applied on the first step of the batch; `mx`/`my`/`auto` stay
/// active for every step.
#[no_mangle]
pub extern "C" fn sim_step(handle: u32, steps: u32) -> u32 {
    let input: Input = match input_json().and_then(|v| {
        if v.is_null() {
            Ok(Input::default())
        } else {
            serde_json::from_value(v).map_err(|e| format!("invalid input: {e}"))
        }
    }) {
        Ok(i) => i,
        Err(e) => return set_err(e),
    };
    let steps = steps.min(MAX_STEPS_PER_CALL);
    let result = with_game(handle, |g| {
        const DT: f64 = 1.0 / 60.0;
        for i in 0..steps {
            let step_input = if i == 0 {
                input.clone()
            } else {
                Input {
                    mx: input.mx,
                    my: input.my,
                    auto: input.auto,
                    ..Default::default()
                }
            };
            g.step(DT, &step_input);
        }
        g.status_json()
    });
    match result {
        Ok(s) => set_out(s),
        Err(e) => set_err(e),
    }
}

/// Convenience: run until the configured time limit / death / win.
///
/// `input` JSON may contain `{ "max_steps": n }`; defaults to 200k steps
/// (about 55 minutes of game time). Returns the final status JSON.
#[no_mangle]
pub extern "C" fn sim_run(handle: u32) -> u32 {
    let opts = match input_json() {
        Ok(v) => v,
        Err(e) => return set_err(e),
    };
    let max_steps = opts
        .get("max_steps")
        .and_then(Value::as_u64)
        .unwrap_or(200_000)
        .min(u64::MAX);
    let result = with_game(handle, |g| {
        const DT: f64 = 1.0 / 60.0;
        let input = Input {
            auto: Some(true),
            ..Default::default()
        };
        let mut n = 0u64;
        while n < max_steps && !g.ended && g.over.is_none() {
            g.step(DT, &input);
            n += 1;
        }
        g.status_json()
    });
    match result {
        Ok(s) => set_out(s),
        Err(e) => set_err(e),
    }
}

/// Full (or summary) state snapshot. `detail != 0` includes entities for rendering.
#[no_mangle]
pub extern "C" fn sim_snapshot(handle: u32, detail: u32) -> u32 {
    let result = with_game(handle, |g| g.snapshot_json(detail != 0));
    match result {
        Ok(s) => set_out(s),
        Err(e) => set_err(e),
    }
}

/// Pick a card while a choice is open.
#[no_mangle]
pub extern "C" fn sim_pick(handle: u32, index: u32) -> u32 {
    let result = with_game(handle, |g| {
        if g.choice.is_some() {
            g.choose(index as usize);
        }
        g.status_json()
    });
    match result {
        Ok(s) => set_out(s),
        Err(e) => set_err(e),
    }
}

/// Static content/balance metadata. Authoritative for menus and editors.
#[no_mangle]
pub extern "C" fn sim_meta() -> u32 {
    set_out(meta_json())
}

/// Run a self-contained batch of independent runs from the input buffer:
///
/// ```json
/// { "configs": [ {seed: 1, ...}, {seed: 2, ...} ] }
/// ```
///
/// Returns `{ "runs": [status, ...] }`. Handy for native-style parity checks
/// from a single WASM instance, though the browser simulator prefers one
/// worker per run.
#[no_mangle]
pub extern "C" fn sim_batch() -> u32 {
    let value = match input_json() {
        Ok(v) => v,
        Err(e) => return set_err(e),
    };
    let configs = match value.get("configs").and_then(Value::as_array) {
        Some(c) => c.clone(),
        None => return set_err("missing configs array"),
    };
    let mut runs = Vec::with_capacity(configs.len());
    for cfg_value in configs {
        let cfg: Config = match serde_json::from_value(cfg_value) {
            Ok(c) => c,
            Err(e) => {
                runs.push(json!({ "error": e.to_string() }));
                continue;
            }
        };
        let mut game = Game::new(cfg);
        const DT: f64 = 1.0 / 60.0;
        let input = Input {
            auto: Some(true),
            ..Default::default()
        };
        let max_steps = 200_000u64;
        let mut n = 0u64;
        while n < max_steps && !game.ended && game.over.is_none() {
            game.step(DT, &input);
            n += 1;
        }
        runs.push(game.snapshot(false));
    }
    set_out(json!({ "runs": runs, "version": CORE_VERSION }).to_string())
}

// ---------------------------------------------------------------------------
// Native helper (not exported to WASM)
// ---------------------------------------------------------------------------

/// Minimal native wrapper so batch tools and tests do not need the C ABI.
pub struct Sim {
    game: Game,
}

impl Sim {
    pub fn new(cfg: Config) -> Self {
        Self {
            game: Game::new(cfg),
        }
    }

    pub fn game(&self) -> &Game {
        &self.game
    }

    pub fn game_mut(&mut self) -> &mut Game {
        &mut self.game
    }

    pub fn step(&mut self, steps: u32, input: &Input) {
        const DT: f64 = 1.0 / 60.0;
        for i in 0..steps {
            let step_input = if i == 0 {
                input.clone()
            } else {
                Input {
                    mx: input.mx,
                    my: input.my,
                    auto: input.auto,
                    ..Default::default()
                }
            };
            self.game.step(DT, &step_input);
        }
    }

    pub fn run(&mut self, max_steps: u64) -> &Game {
        const DT: f64 = 1.0 / 60.0;
        let input = Input {
            auto: Some(true),
            ..Default::default()
        };
        let mut n = 0;
        while n < max_steps && !self.game.ended && self.game.over.is_none() {
            self.game.step(DT, &input);
            n += 1;
        }
        &self.game
    }

    pub fn snapshot(&self, detail: bool) -> String {
        self.game.snapshot_json(detail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_version_is_set() {
        assert!(CORE_VERSION >= 1);
        let meta = meta_json();
        assert!(meta.contains("crown-sim-core"));
    }

    #[test]
    fn same_seed_same_result() {
        let mut a = Sim::new(Config {
            seed: 4242,
            ..Default::default()
        });
        let mut b = Sim::new(Config {
            seed: 4242,
            ..Default::default()
        });
        a.run(60 * 180);
        b.run(60 * 180);
        let sa = a.game();
        let sb = b.game();
        assert_eq!(sa.kills, sb.kills);
        assert_eq!(sa.gold, sb.gold);
        assert_eq!(sa.lvl, sb.lvl);
        assert_eq!(sa.rng.state, sb.rng.state);
        assert!((sa.t - sb.t).abs() < 1e-9);
    }

    #[test]
    fn different_seed_diverges() {
        let mut a = Sim::new(Config {
            seed: 1,
            ..Default::default()
        });
        let mut b = Sim::new(Config {
            seed: 2,
            ..Default::default()
        });
        a.run(60 * 120);
        b.run(60 * 120);
        assert!(a.game().rng.state != b.game().rng.state);
    }

    #[test]
    fn time_limit_stops_exactly() {
        let mut sim = Sim::new(Config {
            seed: 7,
            time_limit: 10.0,
            ..Default::default()
        });
        sim.run(60 * 60);
        assert!(sim.game().ended);
        assert!(sim.game().timed_out);
        assert!((sim.game().t - 10.0).abs() < 1e-9);
    }

    #[test]
    fn snapshots_are_valid_json() {
        let mut sim = Sim::new(Config {
            seed: 11,
            ..Default::default()
        });
        sim.run(60 * 30);
        let s = sim.snapshot(true);
        let v: Value = serde_json::from_str(&s).expect("snapshot json");
        assert_eq!(v["version"], CORE_VERSION);
        assert!(v["entities"]["enemies"].is_array());
    }
}
