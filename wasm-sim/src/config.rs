//! Configuration and input types shared by the browser, simulator workers and
//! native batch tools.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct MetaCfg {
    pub might: u32,
    pub tome: u32,
    pub magnet: u32,
    pub area: u32,
    pub armor: u32,
    pub boots: u32,
    pub speed: u32,
    pub luck: u32,
    pub greed: u32,
    pub dmg: u32,
    pub cd: u32,
    pub mag: u32,
    pub hp: u32,
    pub regen: u32,
    pub choice: u32,
    pub reroll: u32,
    pub revive: u32,
}

impl Default for MetaCfg {
    fn default() -> Self {
        Self {
            might: 0,
            tome: 0,
            magnet: 0,
            area: 0,
            armor: 0,
            boots: 0,
            speed: 0,
            luck: 0,
            greed: 0,
            dmg: 0,
            cd: 0,
            mag: 0,
            hp: 0,
            regen: 0,
            choice: 0,
            reroll: 0,
            revive: 0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ModsCfg {
    pub speed: bool,
    pub speed_curve: bool,
    pub dash: bool,
    pub walls: bool,
    pub revive: bool,
    pub left_hand: bool,
    pub num_keys: bool,
    pub time_freeze: bool,
    pub area_clear: bool,
    pub qa: bool,
    pub disable_tracking: bool,
}

impl Default for ModsCfg {
    fn default() -> Self {
        Self {
            speed: true,
            speed_curve: false,
            dash: true,
            walls: true,
            revive: true,
            left_hand: false,
            num_keys: true,
            time_freeze: true,
            area_clear: true,
            qa: false,
            disable_tracking: true,
        }
    }
}

/// A build profile: `ranks` is 1 = highest priority. `weights` is the
/// Photoshop-style layer influence multiplier (clamped positive by the core).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    pub ranks: HashMap<String, i32>,
    pub weights: HashMap<String, f64>,
}

impl Profile {
    pub fn rank(&self, kind: &str, key: &str) -> Option<i32> {
        let prefixed = format!("{kind}:{key}");
        self.ranks
            .get(&prefixed)
            .copied()
            .or_else(|| self.ranks.get(key).copied())
    }

    pub fn weight(&self, kind: &str, key: &str) -> f64 {
        let prefixed = format!("{kind}:{key}");
        self.weights
            .get(&prefixed)
            .copied()
            .or_else(|| self.weights.get(key).copied())
            .unwrap_or(1.0)
    }

    /// Card weight used by the offer roller. `None` means "no profile".
    pub fn card_weight(&self, kind: &str, key: &str) -> Option<f64> {
        self.rank(kind, key)
            .map(|rk| (11.0 - rk as f64).max(0.5) * self.weight(kind, key))
    }

    /// Card score used by the auto-picker.
    pub fn pick_score(&self, card_kind: char, key: &str, is_evo: bool) -> f64 {
        let prof_kind = match card_kind {
            'p' => "p",
            'm' => "m",
            _ => "w",
        };
        let rk = self.rank(prof_kind, key).unwrap_or(8);
        let mut score = (11.0 - rk as f64).max(0.5) * self.weight(prof_kind, key);
        if is_evo {
            score += 50.0;
        }
        if card_kind == 'i' || card_kind == 'x' {
            score += 2.0;
        }
        score
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub seed: u32,
    pub character: usize,
    pub meta: MetaCfg,
    pub mods: ModsCfg,
    pub profile: Option<Profile>,
    /// 0 = unlimited. Runs stop exactly when `t` reaches this value.
    pub time_limit: f64,
    pub auto: bool,
    pub god: bool,
    pub curse: u32,
    /// Logical viewport used for spawn distance, despawn and zap targeting.
    /// Defaults match a 16:9 desktop game (see `rs()` in index.html).
    pub view_w: f64,
    pub view_h: f64,
    /// Fidelity switch for the legacy evolved-wall bug that damaged every enemy
    /// on the map. Default `false` = fixed (distance-checked) behaviour.
    pub wall_laser_global: bool,
    /// Spawn budget. The regular spawner stops adding enemies at this count
    /// (matching the legacy `en.length < 320` check); bosses and their adds can
    /// briefly exceed it. Hosts can lower this to bound memory per instance.
    pub max_enemies: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            seed: 1,
            character: 0,
            meta: MetaCfg::default(),
            mods: ModsCfg::default(),
            profile: None,
            time_limit: 0.0,
            auto: true,
            god: false,
            curse: 0,
            view_w: 960.0,
            view_h: 540.0,
            wall_laser_global: false,
            max_enemies: 320,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Input {
    pub mx: f64,
    pub my: f64,
    pub dash: bool,
    pub wall: bool,
    /// Override auto-play for this (and subsequent) steps.
    pub auto: Option<bool>,
    /// Pick a card by index while a choice is open.
    pub pick: Option<usize>,
}
