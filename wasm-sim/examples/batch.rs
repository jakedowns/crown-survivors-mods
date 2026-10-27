//! Native batch runner: the headless, multi-threaded path used for balance work.
//!
//! ```bash
//! cargo run --release --example batch -- --runs 32 --threads 8 --time 900
//! cargo run --release --example batch -- --runs 8 --char 2 --curse 1 --json
//! ```
//!
//! Each run is an independent `Game`, so this scales linearly with cores. The
//! browser simulator uses the exact same core through WASM workers.

use crown_sim_core::{Config, Game, Input};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

#[derive(Clone, Debug)]
struct Opts {
    runs: u32,
    threads: usize,
    time: f64,
    char: usize,
    curse: u32,
    seed0: u32,
    json: bool,
    profile: Option<String>,
    meta: u32,
    revives: u32,
    choices: u32,
}

fn parse_args() -> Opts {
    let mut o = Opts {
        runs: 8,
        threads: std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4),
        time: 900.0,
        char: 0,
        curse: 0,
        seed0: 1,
        json: false,
        profile: None,
        meta: 0,
        revives: 0,
        choices: 0,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().expect("missing value");
        match a.as_str() {
            "--runs" | "-n" => o.runs = val().parse().expect("runs"),
            "--threads" | "-t" => o.threads = val().parse().expect("threads"),
            "--time" => o.time = val().parse().expect("time"),
            "--char" => o.char = val().parse().expect("char"),
            "--curse" => o.curse = val().parse().expect("curse"),
            "--seed" => o.seed0 = val().parse().expect("seed"),
            "--profile" => o.profile = Some(val()),
            "--meta" => o.meta = val().parse().expect("meta"),
            "--revives" => o.revives = val().parse().expect("revives"),
            "--choices" => o.choices = val().parse().expect("choices"),
            "--json" => o.json = true,
            "--help" | "-h" => {
                println!(
                    "batch: --runs N --threads N --time SECONDS --char IDX --curse N --seed N --meta LVL --revives N --choices N --profile FILE --json"
                );
                std::process::exit(0);
            }
            other => panic!("unknown argument {other}"),
        }
    }
    o.threads = o.threads.max(1);
    o
}

struct RunResult {
    seed: u32,
    time: f64,
    kills: u32,
    level: u32,
    gold: f64,
    damage: f64,
    bosses: u32,
    won: bool,
    reason: &'static str,
    steps: u64,
    core_ms: f64,
}

fn profile_from_arg(path: &str) -> Option<crown_sim_core::Profile> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn main() {
    let opts = parse_args();
    let profile = opts.profile.as_deref().and_then(profile_from_arg);
    let started = Instant::now();
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<RunResult>> = Mutex::new(Vec::new());
    let cores = opts.threads.min(opts.runs.max(1) as usize);

    std::thread::scope(|scope| {
        for _ in 0..cores {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= opts.runs as usize {
                    break;
                }
                let seed = opts.seed0.wrapping_add(i as u32 * 2_654_435_761);
                let mut meta = crown_sim_core::MetaCfg::default();
                if opts.meta > 0 {
                    meta.might = opts.meta;
                    meta.tome = opts.meta;
                    meta.magnet = opts.meta;
                    meta.area = opts.meta;
                    meta.armor = opts.meta;
                    meta.boots = opts.meta;
                    meta.speed = opts.meta;
                    meta.luck = opts.meta;
                    meta.greed = opts.meta;
                    meta.hp = opts.meta;
                    meta.reroll = opts.meta;
                }
                meta.revive = opts.revives;
                meta.choice = opts.choices;
                let cfg = Config {
                    seed,
                    character: opts.char,
                    curse: opts.curse,
                    time_limit: opts.time,
                    auto: true,
                    meta,
                    profile: profile.clone(),
                    ..Default::default()
                };
                let t0 = Instant::now();
                let mut game = Game::new(cfg);
                let input = Input {
                    auto: Some(true),
                    ..Default::default()
                };
                const DT: f64 = 1.0 / 60.0;
                let mut steps = 0u64;
                while steps < 200_000 && !game.ended && game.over.is_none() {
                    game.step(DT, &input);
                    steps += 1;
                }
                let core_ms = t0.elapsed().as_secs_f64() * 1000.0;
                let (won, reason) = match &game.over {
                    Some(o) => (o.win, o.reason),
                    None => (game.won, if game.timed_out { "limit" } else { "running" }),
                };
                results.lock().unwrap().push(RunResult {
                    seed,
                    time: game.t,
                    kills: game.kills,
                    level: game.lvl,
                    gold: game.gold,
                    damage: game.dmg_dealt,
                    bosses: game.bb,
                    won,
                    reason,
                    steps,
                    core_ms,
                });
            });
        }
    });

    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|r| r.seed);
    let wall = started.elapsed().as_secs_f64();

    if opts.json {
        let runs: Vec<_> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "seed": r.seed,
                    "time": r.time,
                    "kills": r.kills,
                    "level": r.level,
                    "gold": r.gold,
                    "damage": r.damage,
                    "bosses": r.bosses,
                    "won": r.won,
                    "reason": r.reason,
                    "steps": r.steps,
                    "coreMs": r.core_ms,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "runs": runs,
                "wallSeconds": wall,
                "threads": cores,
                "gameSeconds": results.iter().map(|r| r.time).sum::<f64>(),
            })
        );
        return;
    }

    println!(
        "{:>10}  {:>7}  {:>7}  {:>5}  {:>6}  {:>9}  {:>6}  {}",
        "seed", "time", "kills", "lvl", "bosses", "damage", "gold", "result"
    );
    for r in &results {
        println!(
            "{:>10}  {:>7.1}  {:>7}  {:>5}  {:>6}  {:>9.0}  {:>6.0}  {}",
            r.seed, r.time, r.kills, r.level, r.bosses, r.damage, r.gold, r.reason
        );
    }
    let n = results.len().max(1) as f64;
    let wins = results.iter().filter(|r| r.won).count();
    let game_seconds: f64 = results.iter().map(|r| r.time).sum();
    let core_seconds: f64 = results.iter().map(|r| r.core_ms).sum::<f64>() / 1000.0;
    println!(
        "\n{} runs on {} thread(s) in {:.2}s wall ({:.1}s game-time, {:.0}x realtime single-core)",
        results.len(),
        cores,
        wall,
        game_seconds,
        game_seconds / core_seconds.max(1e-9),
    );
    println!(
        "avg time {:.0}s | avg kills {:.0} | avg level {:.1} | wins {} ({:.0}%)",
        results.iter().map(|r| r.time).sum::<f64>() / n,
        results.iter().map(|r| r.kills as f64).sum::<f64>() / n,
        results.iter().map(|r| r.level as f64).sum::<f64>() / n,
        wins,
        100.0 * wins as f64 / n,
    );
}
