//! Invariants that hosts depend on, independent of concrete balance numbers.
//!
//! These tests intentionally avoid asserting "seed 1 wins at 800s" style
//! snapshots: balance changes are allowed, but determinism and state bounds are
//! part of the ABI contract between the core, the simulator and the product.

use crown_sim_core::{Config, Input, MetaCfg, Profile, Sim};
use serde_json::Value;
use std::collections::HashMap;

fn assert_same_run(a: &Sim, b: &Sim) {
    let sa: Value = serde_json::from_str(&a.snapshot(false)).unwrap();
    let sb: Value = serde_json::from_str(&b.snapshot(false)).unwrap();
    assert_eq!(sa, sb, "runs diverged");
}

#[test]
fn batching_is_equivalent_to_single_steps() {
    let cfg = Config {
        seed: 99,
        auto: false,
        time_limit: 0.0,
        ..Default::default()
    };
    let mut batched = Sim::new(cfg.clone());
    let mut single = Sim::new(cfg);
    let neutral = Input::default();

    batched.step(120, &neutral);
    for _ in 0..120 {
        single.step(1, &neutral);
    }

    assert_same_run(&batched, &single);
}

#[test]
fn batching_mid_run_matches_single_steps() {
    let cfg = Config {
        seed: 123,
        auto: true,
        ..Default::default()
    };
    let mut batched = Sim::new(cfg.clone());
    let mut single = Sim::new(cfg);
    let input = Input {
        auto: Some(true),
        ..Default::default()
    };

    // Mix batch sizes so a regression in per-call state (dash/wall edge
    // triggering, pending choices, win timers) shows up.
    batched.step(60, &input);
    for _ in 0..60 {
        single.step(1, &input);
    }
    batched.step(240, &input);
    for _ in 0..240 {
        single.step(1, &input);
    }
    batched.step(15, &input);
    for _ in 0..15 {
        single.step(1, &input);
    }

    assert_same_run(&batched, &single);
}

#[test]
fn profiles_are_deterministic() {
    let mut ranks = HashMap::new();
    ranks.insert("w:bolt".to_string(), 1);
    ranks.insert("p:tome".to_string(), 2);
    let profile = Profile {
        name: "test".into(),
        ranks,
        weights: HashMap::new(),
    };
    let cfg = Config {
        seed: 2024,
        auto: true,
        profile: Some(profile),
        ..Default::default()
    };
    let mut a = Sim::new(cfg.clone());
    let mut b = Sim::new(cfg);
    a.run(60 * 240);
    b.run(60 * 240);
    assert_same_run(&a, &b);
}

#[test]
fn spawner_respects_configured_enemy_budget() {
    // `max_enemies` throttles the regular spawner (and boss adds), exactly like
    // the legacy `en.length < 320` check. Bosses themselves are mandatory and
    // may push the total a few entities over the budget, but the world must not
    // grow without bound.
    const CAP: usize = 64;
    let mut sim = Sim::new(Config {
        seed: 5,
        auto: true,
        max_enemies: CAP,
        time_limit: 60.0,
        ..Default::default()
    });
    let input = Input {
        auto: Some(true),
        ..Default::default()
    };
    for _ in 0..60 * 60 {
        // `sim_step` in the wild advances in batches; stepping in chunks also
        // exercises the same path hosts use.
        sim.step(7, &input);
        let live = sim.game().enemies.iter().filter(|e| !e.dead).count();
        assert!(
            live <= CAP + 8,
            "live enemy count {live} exceeded spawn budget at t={}",
            sim.game().t
        );
    }
}

#[test]
fn time_limit_is_exact_for_any_meta() {
    let mut meta = MetaCfg::default();
    meta.revive = 3;
    let mut sim = Sim::new(Config {
        seed: 31,
        auto: true,
        time_limit: 15.0,
        meta,
        ..Default::default()
    });
    sim.run(60 * 120);
    assert!(sim.game().ended);
    if !sim.game().over.as_ref().map(|o| o.win).unwrap_or(false) {
        assert!(sim.game().timed_out);
        assert!((sim.game().t - 15.0).abs() < 1e-9);
    }
}

#[test]
fn status_and_snapshot_agree() {
    let mut sim = Sim::new(Config {
        seed: 77,
        auto: true,
        ..Default::default()
    });
    sim.run(60 * 90);
    let status: Value = serde_json::from_str(&sim.game().status_json()).unwrap();
    let snap: Value = serde_json::from_str(&sim.snapshot(false)).unwrap();
    for key in ["version", "t", "rng", "level", "xp", "kills", "gold"] {
        assert_eq!(status[key], snap[key], "status/snapshot disagree on {key}");
    }
}
