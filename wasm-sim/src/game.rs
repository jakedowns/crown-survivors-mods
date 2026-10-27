//! Game state, content rules (cards/levels/choices) and state serialization.

use crate::config::{Config, Profile};
use crate::data::*;
use crate::rng::Rng;
use serde_json::{json, Map, Value};

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub dmg: f64,
    pub cd: f64,
    pub mag: f64,
    pub area: f64,
    pub armor: f64,
    pub spd: f64,
    pub crit: f64,
    pub greed: f64,
    pub xpm: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Player {
    pub x: f64,
    pub y: f64,
    pub hp: f64,
    pub mhp: f64,
    pub inv: f64,
    pub face: f64,
    pub mdx: f64,
    pub mdy: f64,
    pub moving: bool,
    pub walk: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Dash {
    pub vx: f64,
    pub vy: f64,
    pub t: f64,
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub id: u32,
    pub kind: EType,
    pub x: f64,
    pub y: f64,
    pub hp: f64,
    pub mhp: f64,
    pub r: f64,
    pub spd: f64,
    pub dmg: f64,
    pub xp: f64,
    pub elite: bool,
    pub boss: bool,
    pub bi: usize,
    pub flash: f64,
    pub kx: f64,
    pub ky: f64,
    pub ph: f64,
    pub shoot_t: f64,
    pub slow_t: f64,
    pub dead: bool,
    pub hit_orb: f64,
    /// (angle key, re-hit time) — mirrors legacy `hitT['b'+b.a]`.
    pub hit_boom: Vec<(u64, f64)>,
    // boss-only timers / state
    pub atk: f64,
    pub seq: u32,
    pub bph: u8,
    pub dash: f64,
    pub dvx: f64,
    pub dvy: f64,
    pub burrow: f64,
    pub bx: f64,
    pub by: f64,
    pub spiral: f64,
    pub s_t: f64,
    pub s_a: f64,
    pub hide: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjKind {
    Bolt,
    Boom,
}

#[derive(Clone, Debug)]
pub struct Projectile {
    pub kind: ProjKind,
    /// Angle key for boomerang re-hit tracking (`b.a` in the legacy code).
    pub key: u64,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub dmg: f64,
    pub r: f64,
    pub life: f64,
    pub age: f64,
    pub pierce: i32,
    pub rehit: f64,
    pub hit: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct Pool {
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub life: f64,
    pub dmg: f64,
    pub tick: f64,
    pub slow: bool,
}

#[derive(Clone, Debug)]
pub struct EBullet {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub r: f64,
    pub dmg: f64,
    pub life: f64,
    pub color: &'static str,
    pub tw: f64,
    pub split: Option<f64>,
    pub orb: bool,
    pub ox: f64,
    pub oy: f64,
    pub ang: f64,
    pub dang: f64,
    pub rad: f64,
    pub drad: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GemKind {
    Xp,
    Mid,
    Big,
    Gold,
    Heart,
    Chest,
    Magnet,
}

impl GemKind {
    pub fn key(self) -> &'static str {
        match self {
            GemKind::Xp => "xp",
            GemKind::Mid => "mid",
            GemKind::Big => "big",
            GemKind::Gold => "gold",
            GemKind::Heart => "heart",
            GemKind::Chest => "chest",
            GemKind::Magnet => "magnet",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Gem {
    pub x: f64,
    pub y: f64,
    pub value: f64,
    pub kind: GemKind,
    pub vac: bool,
    pub got: bool,
}

#[derive(Clone, Debug)]
pub struct Wall {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub th: f64,
    pub hp: f64,
    pub mhp: f64,
    pub life: f64,
    pub tick: f64,
    pub laser: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeleKind {
    Circle,
    Line,
}

#[derive(Clone, Debug)]
pub struct Tele {
    pub kind: TeleKind,
    pub x: f64,
    pub y: f64,
    pub x2: f64,
    pub y2: f64,
    pub r: f64,
    pub w: f64,
    pub t: f64,
    pub fire_t: f64,
    pub fired: bool,
    pub post: f64,
    pub dmg: f64,
    pub color: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardKind {
    Weapon { idx: usize, lv: u8 },
    Passive { idx: usize, lv: u8 },
    Mod { idx: usize, lv: u8 },
    EvoWeapon { idx: usize },
    EvoWalls,
    Inf { idx: usize },
    Gold,
    Heal,
}

impl CardKind {
    pub fn kchar(&self) -> char {
        match self {
            CardKind::Weapon { .. } => 'w',
            CardKind::Passive { .. } => 'p',
            CardKind::Mod { .. } => 'm',
            CardKind::EvoWeapon { .. } | CardKind::EvoWalls => 'e',
            CardKind::Inf { .. } => 'i',
            CardKind::Gold | CardKind::Heal => 'x',
        }
    }

    pub fn is_evo(&self) -> bool {
        matches!(self, CardKind::EvoWeapon { .. } | CardKind::EvoWalls)
    }
}

#[derive(Clone, Debug)]
pub struct Card {
    pub kind: CardKind,
    pub wt: f64,
}

impl Card {
    pub fn key(&self) -> &'static str {
        match self.kind {
            CardKind::Weapon { idx, .. } | CardKind::EvoWeapon { idx } => WEAPON_KEYS[idx],
            CardKind::Passive { idx, .. } => PASSIVE_KEYS[idx],
            CardKind::Mod { idx, .. } => {
                if idx == 0 {
                    "dash"
                } else {
                    "walls"
                }
            }
            CardKind::EvoWalls => "walls",
            CardKind::Inf { idx } => INF_KEYS[idx],
            CardKind::Gold => "gold",
            CardKind::Heal => "heal",
        }
    }

    pub fn name(&self) -> &'static str {
        match self.kind {
            CardKind::Weapon { idx, .. } | CardKind::EvoWeapon { idx } => WEAPONS[idx].name,
            CardKind::Passive { idx, .. } => PASSIVES[idx].name,
            CardKind::Mod { idx, .. } => {
                if idx == 0 {
                    "Dash"
                } else {
                    "Walls"
                }
            }
            CardKind::EvoWalls => "Radiant Walls",
            CardKind::Inf { idx } => INF[idx].name,
            CardKind::Gold => "Coin Sack",
            CardKind::Heal => "Royal Feast",
        }
    }

    pub fn level(&self) -> u8 {
        match self.kind {
            CardKind::Weapon { lv, .. }
            | CardKind::Passive { lv, .. }
            | CardKind::Mod { lv, .. } => lv,
            CardKind::EvoWeapon { .. } | CardKind::EvoWalls => 6,
            _ => 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Choice {
    pub cards: Vec<Card>,
    pub t: f64,
    pub source: &'static str,
}

#[derive(Clone, Debug)]
pub struct Over {
    pub win: bool,
    pub gold: f64,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Default)]
pub struct Telemetry {
    pub total_pixels: f64,
    pub max_travel: f64,
    pub max_speed: f64,
    pub max_linger: f64,
    pub frames_stationary: u64,
    pub steps: u64,
    pub last_stationary: f64,
}

pub struct Game {
    pub cfg: Config,
    pub rng: Rng,
    pub t: f64,
    pub p: Player,

    pub enemies: Vec<Enemy>,
    pub projectiles: Vec<Projectile>,
    pub pools: Vec<Pool>,
    pub ebullets: Vec<EBullet>,
    pub gems: Vec<Gem>,
    pub walls: Vec<Wall>,
    pub tele: Vec<Tele>,

    pub weapons: [u8; WEAPON_COUNT],
    pub passives: [u8; PASSIVE_COUNT],
    pub inf: [u32; INF_COUNT],
    pub evo: [bool; WEAPON_COUNT],
    pub w_order: Vec<usize>,
    pub p_order: Vec<usize>,
    pub cd: [f64; WEAPON_COUNT],

    pub orb_a: f64,
    pub aura_t: f64,
    pub aura_r: f64,

    pub lvl: u32,
    pub xp: f64,
    pub kills: u32,
    pub gold: f64,
    pub dmg_dealt: f64,
    pub bb: u32,

    pub spawn_acc: f64,
    pub elite_t: f64,
    pub phase: usize,
    pub boss_i: usize,
    pub boss_id: Option<u32>,

    pub choice: Option<Choice>,
    /// Delay before the next queued level-up card opens (legacy: 150ms).
    pub pending_open: f64,
    pub queue: u32,
    pub slot_w: usize,
    pub slot_p: usize,

    pub dash_lv: u8,
    pub dash_cd: f64,
    pub dash_v: Option<Dash>,
    pub wall_lv: u8,
    pub wall_evo: bool,
    pub wall_draw: bool,
    pub wall_cell: Option<(i32, i32)>,
    pub wall_snd_t: f64,
    pub wall_timer: f64,

    pub revive: u32,
    pub rerolls: u32,
    pub auto: bool,
    pub over: Option<Over>,
    pub won: bool,
    /// Legacy delay between the final boss dying and the run ending (1.8s).
    pub win_timer: f64,
    pub ended: bool,
    pub timed_out: bool,

    pub stats: Stats,
    pub next_id: u32,
    pub telemetry: Telemetry,
    pub last_pos: (f64, f64),
}

impl Game {
    pub fn new(cfg: Config) -> Self {
        let ch = cfg.character.min(CHARACTER_COUNT - 1);
        let c = CHARS[ch];
        let mhp = c.hp + cfg.meta.hp as f64 * 10.0;
        let mut weapons = [0u8; WEAPON_COUNT];
        weapons[c.start_weapon] = 1;
        let dash_lv = if cfg.mods.dash { 1 } else { 0 };
        let wall_lv = if cfg.mods.walls { 1 } else { 0 };
        let auto = cfg.auto;
        let mut g = Self {
            rng: Rng::new(cfg.seed),
            t: 0.0,
            p: Player {
                hp: mhp,
                mhp,
                face: 1.0,
                ..Default::default()
            },
            enemies: Vec::with_capacity(512),
            projectiles: Vec::with_capacity(128),
            pools: Vec::with_capacity(64),
            ebullets: Vec::with_capacity(128),
            gems: Vec::with_capacity(256),
            walls: Vec::with_capacity(64),
            tele: Vec::with_capacity(32),
            weapons,
            passives: [0u8; PASSIVE_COUNT],
            inf: [0u32; INF_COUNT],
            evo: [false; WEAPON_COUNT],
            w_order: vec![c.start_weapon],
            p_order: Vec::new(),
            cd: [0.0; WEAPON_COUNT],
            orb_a: 0.0,
            aura_t: 0.0,
            aura_r: 0.0,
            lvl: 1,
            xp: 0.0,
            kills: 0,
            gold: 0.0,
            dmg_dealt: 0.0,
            bb: 0,
            spawn_acc: 0.0,
            elite_t: 45.0,
            phase: 0,
            boss_i: 0,
            boss_id: None,
            choice: None,
            pending_open: 0.0,
            queue: 0,
            slot_w: 4,
            slot_p: 4,
            dash_lv,
            dash_cd: 0.0,
            dash_v: None,
            wall_lv,
            wall_evo: false,
            wall_draw: false,
            wall_cell: None,
            wall_snd_t: 0.0,
            wall_timer: 0.0,
            revive: cfg.meta.revive,
            rerolls: cfg.meta.reroll,
            auto,
            over: None,
            won: false,
            win_timer: 0.0,
            ended: false,
            timed_out: false,
            stats: Stats::default(),
            next_id: 1,
            telemetry: Telemetry::default(),
            last_pos: (0.0, 0.0),
            cfg,
        };
        g.stats = g.compute_stats();
        g
    }

    pub fn compute_stats(&self) -> Stats {
        let ps = &self.passives;
        let m = &self.cfg.meta;
        let ch = self.cfg.character.min(CHARACTER_COUNT - 1);
        Stats {
            dmg: (1.0 + 0.12 * ps[P_MIGHT] as f64 + 0.05 * m.might as f64)
                * (1.0 + self.inf_value(I_DMG)),
            cd: 0.92f64.powf(ps[P_TOME] as f64) * (1.0 - self.inf_value(I_CD)).max(0.5),
            mag: 72.0
                * (1.0
                    + 0.35 * ps[P_MAGNET] as f64
                    + 0.15 * m.magnet as f64
                    + self.inf_value(I_MAG)),
            area: (1.0 + 0.12 * ps[P_AREA] as f64 + self.inf_value(I_AREA))
                * if ch == 2 { 1.2 } else { 1.0 },
            armor: ps[P_ARMOR] as f64
                + m.armor as f64
                + if ch == 1 { 1.0 } else { 0.0 }
                + self.inf_value(I_ARMOR).floor(),
            spd: 110.0
                * CHARS[ch].spd
                * (1.0
                    + 0.08 * ps[P_BOOTS] as f64
                    + 0.04 * m.speed as f64
                    + self.inf_value(I_SPD)),
            crit: (0.05 * ps[P_LUCK] as f64
                + if ch == 3 { 0.15 } else { 0.0 }
                + 0.03
                + self.inf_value(I_CRIT))
            .min(0.75),
            greed: 1.0 + 0.15 * m.greed as f64 + 0.1 * ps[P_LUCK] as f64 + 0.25 * self.cfg.curse as f64,
            xpm: if ch == 0 { 1.1 } else { 1.0 },
        }
    }

    pub fn inf_value(&self, idx: usize) -> f64 {
        let d = &INF[idx];
        let n = self.inf[idx];
        d.value * (n.min(d.cap) as f64 + n.saturating_sub(d.cap) as f64 * 0.5)
    }

    pub fn view_half(&self) -> (f64, f64) {
        (self.cfg.view_w * 0.5, self.cfg.view_h * 0.5)
    }

    pub fn nearest(&self, x: f64, y: f64, maxd: f64) -> Option<usize> {
        let mut best = None;
        let mut bd = maxd * maxd;
        for (i, e) in self.enemies.iter().enumerate() {
            if e.dead {
                continue;
            }
            let d = (e.x - x).powi(2) + (e.y - y).powi(2);
            if d <= bd {
                bd = d;
                best = Some(i);
            }
        }
        best
    }

    pub fn edge_pt(&mut self, margin: f64) -> (f64, f64) {
        let (hw0, hh0) = self.view_half();
        let hw = hw0 + margin;
        let hh = hh0 + margin;
        let per = 2.0 * (hw + hh);
        let r = self.rng.next_f64() * per;
        let (ex, ey) = if r < 2.0 * hw {
            (-hw + r, if self.rng.chance(0.5) { -hh } else { hh })
        } else {
            (
                if self.rng.chance(0.5) { -hw } else { hw },
                -hh + (r - 2.0 * hw),
            )
        };
        (self.p.x + ex, self.p.y + ey)
    }

    pub fn spawn_enemy(&mut self, kind: EType, elite: bool, x: f64, y: f64) -> usize {
        let def = *kind.def();
        let hm = (1.0 + self.t / 95.0 + (self.t / 210.0).powi(2))
            * (1.0 + 0.35 * self.cfg.curse as f64);
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let e = Enemy {
            id,
            kind,
            x,
            y,
            hp: def.hp * hm * if elite { 8.0 } else { 1.0 },
            mhp: def.hp * hm * if elite { 8.0 } else { 1.0 },
            r: def.r * if elite { 1.8 } else { 1.0 },
            spd: def.spd
                * (1.0 + self.t / 900.0)
                * (1.0 + 0.08 * self.cfg.curse as f64)
                * if elite { 0.85 } else { 1.0 },
            dmg: def.dmg * (1.0 + self.t / 400.0) * if elite { 1.5 } else { 1.0 },
            xp: def.xp * if elite { 15.0 } else { 1.0 },
            elite,
            boss: false,
            bi: 0,
            flash: 0.0,
            kx: 0.0,
            ky: 0.0,
            ph: self.rng.next_f64() * 6.0,
            shoot_t: self.rng.range(2.0, 4.0),
            slow_t: 0.0,
            dead: false,
            hit_orb: 0.0,
            hit_boom: Vec::new(),
            atk: 0.0,
            seq: 0,
            bph: 0,
            dash: 0.0,
            dvx: 0.0,
            dvy: 0.0,
            burrow: 0.0,
            bx: 0.0,
            by: 0.0,
            spiral: 0.0,
            s_t: 0.0,
            s_a: 0.0,
            hide: 0.0,
        };
        self.enemies.push(e);
        self.enemies.len() - 1
    }

    pub fn spawn_enemy_edge(&mut self, kind: EType, elite: bool) -> usize {
        let (x, y) = self.edge_pt(18.0);
        self.spawn_enemy(kind, elite, x, y)
    }

    pub fn spawn_boss(&mut self, i: usize) {
        let b = &BOSSES[i];
        let hm = 1.0 + 0.4 * self.cfg.curse as f64;
        let a = self.rng.next_f64() * 6.28;
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let e = Enemy {
            id,
            kind: EType::Blob,
            x: self.p.x + a.cos() * 220.0,
            y: self.p.y + a.sin() * 220.0,
            hp: b.hp * hm,
            mhp: b.hp * hm,
            r: b.r,
            spd: b.spd,
            dmg: 18.0 + i as f64 * 6.0,
            xp: 60.0,
            elite: false,
            boss: true,
            bi: i,
            flash: 0.0,
            kx: 0.0,
            ky: 0.0,
            ph: 0.0,
            shoot_t: 0.0,
            slow_t: 0.0,
            dead: false,
            hit_orb: 0.0,
            hit_boom: Vec::new(),
            atk: 3.0,
            seq: 0,
            bph: 0,
            dash: 0.0,
            dvx: 0.0,
            dvy: 0.0,
            burrow: 0.0,
            bx: 0.0,
            by: 0.0,
            spiral: 0.0,
            s_t: 0.0,
            s_a: 0.0,
            hide: 0.0,
        };
        self.enemies.push(e);
        self.boss_id = Some(id);
    }

    pub fn hurt_enemy(&mut self, idx: usize, dmg: f64, kb: f64, ax: f64, ay: f64) {
        let dead = match self.enemies.get(idx) {
            Some(e) => e.dead,
            None => return,
        };
        if dead {
            return;
        }
        let crit = self.rng.chance(self.stats.crit);
        let dmg = dmg
            * self.stats.dmg
            * if crit { 2.0 } else { 1.0 }
            * self.rng.range(0.9, 1.1);
        self.dmg_dealt += dmg;
        let e = &mut self.enemies[idx];
        e.hp -= dmg;
        e.flash = 0.1;
        if kb > 0.0 && !e.boss {
            let l = ax.hypot(ay).max(1e-9);
            e.kx += ax / l * kb;
            e.ky += ay / l * kb;
        }
        if self.enemies[idx].hp <= 0.0 {
            self.kill_enemy(idx);
        }
    }

    pub fn kill_enemy(&mut self, idx: usize) {
        if self.enemies.get(idx).map(|e| e.dead).unwrap_or(true) {
            return;
        }
        let (x, y, xp, elite, boss, bi) = {
            let e = &mut self.enemies[idx];
            e.dead = true;
            (e.x, e.y, e.xp, e.elite, e.boss, e.bi)
        };
        self.kills += 1;

        let gem_kind = if xp >= 15.0 {
            GemKind::Big
        } else if xp >= 3.0 {
            GemKind::Mid
        } else {
            GemKind::Xp
        };
        self.gems.push(Gem { x, y, value: xp, kind: gem_kind, vac: false, got: false });

        if self.rng.chance(0.045 * self.stats.greed) || elite {
            let v = if elite {
                (8.0 * self.stats.greed).round()
            } else {
                (self.stats.greed).round().max(1.0)
            };
            self.gems.push(Gem {
                x: x + self.rng.range(-6.0, 6.0),
                y: y + self.rng.range(-6.0, 6.0),
                value: v,
                kind: GemKind::Gold,
                vac: false,
                got: false,
            });
        }
        if self.rng.chance(0.004) || elite {
            self.gems.push(Gem { x: x + 5.0, y, value: 20.0, kind: GemKind::Heart, vac: false, got: false });
        }
        if elite {
            self.gems.push(Gem { x: x - 5.0, y, value: 0.0, kind: GemKind::Chest, vac: false, got: false });
        }

        if boss {
            self.boss_id = None;
            self.bb += 1;
            for _ in 0..20 {
                self.gems.push(Gem {
                    x: x + self.rng.range(-30.0, 30.0),
                    y: y + self.rng.range(-30.0, 30.0),
                    value: (5.0 * self.stats.greed).round(),
                    kind: GemKind::Gold,
                    vac: false,
                    got: false,
                });
            }
            self.tele.clear();
            if bi == 2 {
                self.slot_w = 5;
                self.slot_p = 5;
            }
            if bi == BOSS_COUNT - 1 {
                self.won = true;
                self.win_timer = 1.8;
                for e in self.enemies.iter_mut() {
                    if !e.boss {
                        e.dead = true;
                    }
                }
            } else {
                self.gems.push(Gem { x, y, value: 0.0, kind: GemKind::Chest, vac: false, got: false });
                self.ebullets.clear();
                self.phase = bi + 1;
                if bi >= 2 {
                    self.p.hp = (self.p.hp + self.p.mhp * 0.25).min(self.p.mhp);
                }
            }
        }
    }

    pub fn hurt_player(&mut self, dmg: f64) {
        if self.p.inv > 0.0 || self.cfg.god || self.won || self.over.is_some() {
            return;
        }
        let d = (dmg - self.stats.armor).max(1.0);
        self.p.hp -= d;
        self.p.inv = 0.6;
        if self.p.hp <= 0.0 {
            if self.revive > 0 {
                self.revive -= 1;
                self.p.hp = self.p.mhp * 0.5;
                self.p.inv = 2.4;
                let (px, py) = (self.p.x, self.p.y);
                // Legacy revive pulse *kills* (not just removes) nearby enemies,
                // so they grant XP, gold and kill credit.
                let nearby: Vec<usize> = self
                    .enemies
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| {
                        !e.boss
                            && !e.dead
                            && (e.x - px).powi(2) + (e.y - py).powi(2) < 160.0 * 160.0
                    })
                    .map(|(i, _)| i)
                    .collect();
                for i in nearby {
                    self.kill_enemy(i);
                }
            } else {
                self.p.hp = 0.0;
                self.end_run(false, "death");
            }
        }
    }

    pub fn end_run(&mut self, win: bool, reason: &'static str) {
        if self.over.is_some() {
            return;
        }
        let gold = (self.gold
            + self.kills as f64 / 30.0
            + if win {
                220.0 * (1.0 + self.cfg.curse as f64 * 0.5)
            } else {
                0.0
            }
            + if self.bb >= 3 && !win { 60.0 } else { 0.0 })
        .round();
        self.over = Some(Over { win, gold, reason });
        self.ended = true;
    }

    // ---------------- cards / leveling ----------------

    fn card_weight(&self, kind: &str, key: &str) -> Option<f64> {
        self.cfg
            .profile
            .as_ref()
            .and_then(|p| p.card_weight(kind, key))
    }

    fn push_weighted(&self, cards: &mut Vec<Card>, card: CardKind, prof_kind: &str, key: &str, base: f64) {
        let wt = self
            .card_weight(prof_kind, key)
            .map(|w| if card.is_evo() { w * 10.0 } else { w })
            .unwrap_or(base);
        cards.push(Card { kind: card, wt });
    }

    pub fn offer_cards(&mut self) -> Vec<Card> {
        let mut out: Vec<Card> = Vec::with_capacity(16);
        for &idx in &self.w_order.clone() {
            let lv = self.weapons[idx];
            if lv == 0 || self.evo[idx] {
                continue;
            }
            if lv < 5 {
                self.push_weighted(
                    &mut out,
                    CardKind::Weapon { idx, lv: lv + 1 },
                    "w",
                    WEAPON_KEYS[idx],
                    3.0,
                );
            } else if !self.evo[idx] && self.passives[WEAPONS[idx].evo_passive] > 0 {
                self.push_weighted(
                    &mut out,
                    CardKind::EvoWeapon { idx },
                    "w",
                    WEAPON_KEYS[idx],
                    100.0,
                );
            }
        }
        if self.w_order.len() < self.slot_w {
            for idx in 0..WEAPON_COUNT {
                if self.weapons[idx] == 0 {
                    self.push_weighted(
                        &mut out,
                        CardKind::Weapon { idx, lv: 1 },
                        "w",
                        WEAPON_KEYS[idx],
                        2.0,
                    );
                }
            }
        }
        for &idx in &self.p_order.clone() {
            let lv = self.passives[idx];
            if lv == 0 {
                continue;
            }
            if lv < 5 {
                self.push_weighted(
                    &mut out,
                    CardKind::Passive { idx, lv: lv + 1 },
                    "p",
                    PASSIVE_KEYS[idx],
                    2.0,
                );
            }
        }
        if self.p_order.len() < self.slot_p {
            for idx in 0..PASSIVE_COUNT {
                if self.passives[idx] == 0 {
                    self.push_weighted(
                        &mut out,
                        CardKind::Passive { idx, lv: 1 },
                        "p",
                        PASSIVE_KEYS[idx],
                        1.5,
                    );
                }
            }
        }
        if self.cfg.mods.dash && self.dash_lv > 0 && self.dash_lv < 5 {
            self.push_weighted(
                &mut out,
                CardKind::Mod { idx: 0, lv: self.dash_lv + 1 },
                "m",
                "dash",
                1.1,
            );
        }
        if self.cfg.mods.walls && self.wall_lv > 0 {
            if !self.wall_evo && self.wall_lv < 5 {
                self.push_weighted(
                    &mut out,
                    CardKind::Mod { idx: 1, lv: self.wall_lv + 1 },
                    "m",
                    "walls",
                    1.3,
                );
            }
            if !self.wall_evo && self.wall_lv >= 5 && self.passives[P_ARMOR] > 0 {
                self.push_weighted(
                    &mut out,
                    CardKind::EvoWalls,
                    "m",
                    "walls",
                    100.0,
                );
            }
        }

        let n = 3 + self.cfg.meta.choice as usize;
        let mut pick: Vec<Card> = Vec::with_capacity(n);
        if let Some(pos) = out.iter().position(|c| c.kind.is_evo()) {
            pick.push(out.remove(pos));
        }
        while pick.len() < n && !out.is_empty() {
            let sum: f64 = out.iter().map(|c| c.wt).sum();
            let mut r = self.rng.next_f64() * sum;
            let mut chosen = out.len() - 1;
            for (i, c) in out.iter().enumerate() {
                r -= c.wt;
                if r <= 0.0 {
                    chosen = i;
                    break;
                }
            }
            pick.push(out.remove(chosen));
        }
        if pick.len() < n {
            let hpf = if self.p.mhp > 0.0 { self.p.hp / self.p.mhp } else { 0.0 };
            let fw = if hpf < 0.3 {
                4.0
            } else if hpf < 0.6 {
                2.0
            } else {
                0.7
            };
            let mut mix: Vec<Card> = (0..INF_COUNT)
                .map(|idx| Card {
                    kind: CardKind::Inf { idx },
                    wt: if self.inf[idx] >= INF[idx].cap { 0.35 } else { 1.0 },
                })
                .collect();
            mix.push(Card { kind: CardKind::Gold, wt: 0.8 });
            mix.push(Card { kind: CardKind::Heal, wt: fw });

            if self.rng.chance(0.8) {
                let xs: Vec<usize> = mix
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| matches!(c.kind, CardKind::Gold | CardKind::Heal))
                    .map(|(i, _)| i)
                    .collect();
                if !xs.is_empty() {
                    let sum: f64 = xs.iter().map(|&i| mix[i].wt).sum();
                    let mut r = self.rng.next_f64() * sum;
                    let mut chosen = xs[xs.len() - 1];
                    for &i in &xs {
                        r -= mix[i].wt;
                        if r <= 0.0 {
                            chosen = i;
                            break;
                        }
                    }
                    pick.push(mix.remove(chosen));
                }
            }
            while pick.len() < n && !mix.is_empty() {
                let sum: f64 = mix.iter().map(|c| c.wt).sum();
                let mut r = self.rng.next_f64() * sum;
                let mut chosen = mix.len() - 1;
                for (i, c) in mix.iter().enumerate() {
                    r -= c.wt;
                    if r <= 0.0 {
                        chosen = i;
                        break;
                    }
                }
                pick.push(mix.remove(chosen));
            }
            while pick.len() < n {
                pick.push(Card { kind: CardKind::Heal, wt: 1.0 });
            }
        }
        pick
    }

    pub fn open_choice(&mut self, source: &'static str) {
        let cards = self.offer_cards();
        self.choice = Some(Choice { cards, t: 0.0, source });
    }

    pub fn pick_index(&mut self) -> usize {
        let choice = match &self.choice {
            Some(c) => c,
            None => return 0,
        };
        let len = choice.cards.len();
        if len == 0 {
            return 0;
        }
        let profile: Option<&Profile> = self.cfg.profile.as_ref();
        let profile = match profile {
            Some(p) => p,
            None => return self.rng.below(len),
        };
        let mut best = 0;
        let mut best_score = f64::NEG_INFINITY;
        for (i, c) in choice.cards.iter().enumerate() {
            let mut score = profile.pick_score(c.kind.kchar(), c.key(), c.kind.is_evo());
            score += self.rng.next_f64() * 0.01;
            if score > best_score {
                best_score = score;
                best = i;
            }
        }
        best
    }

    pub fn choose(&mut self, index: usize) {
        let mut choice = match self.choice.take() {
            Some(c) => c,
            None => return,
        };
        if choice.cards.is_empty() {
            return;
        }
        let i = index.min(choice.cards.len() - 1);
        let card = choice.cards.remove(i).kind;
        match card {
            CardKind::Inf { idx } => {
                self.inf[idx] += 1;
                if idx == I_HP {
                    let v = if self.inf[I_HP] > INF[I_HP].cap {
                        6.0
                    } else {
                        12.0
                    };
                    self.p.mhp += v;
                    self.p.hp = (self.p.hp + 20.0 + v).min(self.p.mhp);
                }
            }
            CardKind::Weapon { idx, lv } => {
                if self.weapons[idx] == 0 {
                    self.w_order.push(idx);
                }
                self.weapons[idx] = lv;
            }
            CardKind::Passive { idx, lv } => {
                if self.passives[idx] == 0 {
                    self.p_order.push(idx);
                }
                self.passives[idx] = lv;
                if idx == P_HEART {
                    self.p.mhp += 20.0;
                    self.p.hp = (self.p.hp + 30.0).min(self.p.mhp);
                }
            }
            CardKind::Mod { idx, lv } => {
                if idx == 0 {
                    self.dash_lv = lv;
                } else {
                    self.wall_lv = lv;
                }
            }
            CardKind::EvoWeapon { idx } => {
                self.evo[idx] = true;
            }
            CardKind::EvoWalls => {
                self.wall_evo = true;
            }
            CardKind::Gold => {
                self.gold += 25.0;
            }
            CardKind::Heal => {
                self.p.hp = (self.p.hp + self.p.mhp * 0.3).min(self.p.mhp);
            }
        }
        self.stats = self.compute_stats();
        self.choice = None;
        if self.queue > 0 {
            // Legacy defers by 150ms; the world keeps simulating meanwhile.
            self.queue -= 1;
            self.pending_open = 0.15;
        }
    }

    pub fn maybe_auto_pick(&mut self, dt: f64) {
        let ready = match &mut self.choice {
            Some(c) => {
                c.t += dt;
                c.t > 0.35
            }
            None => false,
        };
        if ready && self.auto {
            let idx = self.pick_index();
            self.choose(idx);
        }
    }

    /// Small per-step result used by the hot loop (`sim_step`/`sim_run`).
    ///
    /// Kept deliberately compact: full entity state is only serialized by
    /// [`Game::snapshot`], which hosts call once per rendered/polled frame.
    pub fn status_json(&self) -> String {
        let choice = match &self.choice {
            Some(c) => json!({
                "source": c.source,
                "cards": c.cards.iter().map(|card| json!({
                    "kind": card.kind.kchar().to_string(),
                    "k": card.key(),
                    "n": card.name(),
                    "lv": card.level(),
                    "wt": card.wt,
                })).collect::<Vec<_>>()
            }),
            None => Value::Null,
        };
        json!({
            "version": crate::CORE_VERSION,
            "t": self.t,
            "rng": self.rng.state,
            "level": self.lvl,
            "xp": self.xp,
            "xpNeed": xp_need(self.lvl),
            "hp": self.p.hp,
            "maxHp": self.p.mhp,
            "kills": self.kills,
            "gold": self.gold,
            "damage": self.dmg_dealt,
            "bosses": self.bb,
            "phase": self.phase,
            "enemies": self.enemies.iter().filter(|e| !e.dead).count(),
            "auto": self.auto,
            "choice": choice,
            "over": self.over.as_ref().map(|o| json!({
                "win": o.win,
                "gold": o.gold,
                "reason": o.reason,
            })),
            "won": self.won,
            "ended": self.ended,
            "timedOut": self.timed_out,
            "timeLimit": self.cfg.time_limit,
            "revive": self.revive,
        })
        .to_string()
    }

    // ---------------- snapshot ----------------

    pub fn snapshot(&self, detail: bool) -> Value {
        let mut weapons = Map::new();
        for i in 0..WEAPON_COUNT {
            if self.weapons[i] > 0 {
                weapons.insert(WEAPON_KEYS[i].to_string(), json!(self.weapons[i]));
            }
        }
        let mut passives = Map::new();
        for i in 0..PASSIVE_COUNT {
            if self.passives[i] > 0 {
                passives.insert(PASSIVE_KEYS[i].to_string(), json!(self.passives[i]));
            }
        }
        let mut inf = Map::new();
        for i in 0..INF_COUNT {
            if self.inf[i] > 0 {
                inf.insert(INF_KEYS[i].to_string(), json!(self.inf[i]));
            }
        }
        let evolving: Vec<&str> = (0..WEAPON_COUNT)
            .filter(|&i| self.evo[i])
            .map(|i| WEAPON_KEYS[i])
            .collect();
        let choice = match &self.choice {
            Some(c) => json!({
                "t": c.t,
                "source": c.source,
                "cards": c.cards.iter().map(|card| json!({
                    "kind": card.kind.kchar().to_string(),
                    "k": card.key(),
                    "n": card.name(),
                    "lv": card.level(),
                    "wt": card.wt,
                })).collect::<Vec<_>>()
            }),
            None => Value::Null,
        };

        let mut v = json!({
            "version": crate::CORE_VERSION,
            "t": self.t,
            "seed": self.cfg.seed,
            "rng": self.rng.state,
            "character": self.cfg.character,
            "curse": self.cfg.curse,
            "auto": self.auto,
            "level": self.lvl,
            "xp": self.xp,
            "xpNeed": xp_need(self.lvl),
            "hp": self.p.hp,
            "maxHp": self.p.mhp,
            "kills": self.kills,
            "gold": self.gold,
            "damage": self.dmg_dealt,
            "bosses": self.bb,
            "phase": self.phase,
            "bossAlive": self.boss_id.is_some(),
            "bossIndex": self.boss_i,
            "enemies": self.enemies.iter().filter(|e| !e.dead).count(),
            "over": self.over.as_ref().map(|o| json!({
                "win": o.win,
                "gold": o.gold,
                "reason": o.reason,
            })),
            "won": self.won,
            "ended": self.ended,
            "timedOut": self.timed_out,
            "weapons": weapons,
            "passives": passives,
            "inf": inf,
            "evolutions": evolving,
            "dashLv": self.dash_lv,
            "wallLv": self.wall_lv,
            "wallEvo": self.wall_evo,
            "slotW": self.slot_w,
            "slotP": self.slot_p,
            "revive": self.revive,
            "rerolls": self.rerolls,
            "choice": choice,
            "stats": {
                "dmg": self.stats.dmg,
                "cd": self.stats.cd,
                "mag": self.stats.mag,
                "area": self.stats.area,
                "armor": self.stats.armor,
                "spd": self.stats.spd,
                "crit": self.stats.crit,
                "greed": self.stats.greed,
                "xpm": self.stats.xpm,
            },
            "telemetry": {
                "totalPixels": self.telemetry.total_pixels,
                "maxTravel": self.telemetry.max_travel,
                "maxSpeed": self.telemetry.max_speed,
                "maxLinger": self.telemetry.max_linger,
                "framesStationary": self.telemetry.frames_stationary,
                "steps": self.telemetry.steps,
            },
            "player": {
                "x": self.p.x,
                "y": self.p.y,
                "hp": self.p.hp,
                "mhp": self.p.mhp,
                "inv": self.p.inv,
                "face": self.p.face,
                "mdx": self.p.mdx,
                "mdy": self.p.mdy,
                "moving": self.p.moving,
            },
        });

        if detail {
            let obj = v.as_object_mut().unwrap();
            obj.insert(
                "entities".to_string(),
                json!({
                    "enemies": self.enemies.iter().filter(|e| !e.dead).map(|e| json!({
                        "id": e.id, "type": e.kind.key(), "x": e.x, "y": e.y,
                        "hp": e.hp, "mhp": e.mhp, "r": e.r, "flash": e.flash,
                        "ph": e.ph, "elite": e.elite, "boss": e.boss, "bi": e.bi,
                        "kx": e.kx, "ky": e.ky, "slow": e.slow_t,
                    })).collect::<Vec<_>>(),
                    "projectiles": self.projectiles.iter().map(|p| json!({
                        "kind": if p.kind == ProjKind::Bolt { "bolt" } else { "boom" },
                        "x": p.x, "y": p.y, "vx": p.vx, "vy": p.vy, "r": p.r, "dmg": p.dmg,
                    })).collect::<Vec<_>>(),
                    "pools": self.pools.iter().map(|q| json!({
                        "x": q.x, "y": q.y, "r": q.r, "dmg": q.dmg, "slow": q.slow,
                    })).collect::<Vec<_>>(),
                    "enemyBullets": self.ebullets.iter().map(|b| json!({
                        "x": b.x, "y": b.y, "r": b.r, "dmg": b.dmg, "c": b.color,
                        "orb": b.orb,
                    })).collect::<Vec<_>>(),
                    "gems": self.gems.iter().map(|g| json!({
                        "x": g.x, "y": g.y, "v": g.value, "k": g.kind.key(),
                    })).collect::<Vec<_>>(),
                    "walls": self.walls.iter().filter(|w| w.life > 0.0).map(|w| json!({
                        "x1": w.x1, "y1": w.y1, "x2": w.x2, "y2": w.y2,
                        "th": w.th, "hp": w.hp, "mhp": w.mhp, "l": w.life, "laser": w.laser,
                    })).collect::<Vec<_>>(),
                    "telegraphs": self.tele.iter().map(|q| json!({
                        "k": if q.kind == TeleKind::Circle { "c" } else { "l" },
                        "x": q.x, "y": q.y, "x2": q.x2, "y2": q.y2,
                        "r": q.r, "w": q.w, "t": q.t, "T": q.fire_t,
                        "fired": q.fired, "c": q.color,
                    })).collect::<Vec<_>>(),
                    "auraR": self.aura_r,
                }),
            );
        }
        v
    }

    pub fn snapshot_json(&self, detail: bool) -> String {
        self.snapshot(detail).to_string()
    }
}

pub fn meta_json() -> String {
    json!({
        "version": crate::CORE_VERSION,
        "engine": "crown-sim-core",
        "characters": CHARS.iter().enumerate().map(|(i, c)| json!({
            "id": i, "name": c.name, "desc": c.desc, "color": c.color,
            "hp": c.hp, "spd": c.spd, "perk": c.perk,
            "weapon": WEAPON_KEYS[c.start_weapon],
        })).collect::<Vec<_>>(),
        "weapons": WEAPONS.iter().enumerate().map(|(i, w)| json!({
            "k": w.key, "n": w.name, "d": w.desc, "c": w.color,
            "evo": w.evo_name, "evoP": PASSIVE_KEYS[w.evo_passive], "id": i,
        })).collect::<Vec<_>>(),
        "passives": PASSIVES.iter().enumerate().map(|(i, p)| json!({
            "k": p.key, "n": p.name, "d": p.desc, "c": p.color, "id": i,
        })).collect::<Vec<_>>(),
        "inf": INF.iter().enumerate().map(|(i, q)| json!({
            "k": q.key, "n": q.name, "d": q.desc, "v": q.value, "cap": q.cap, "id": i,
        })).collect::<Vec<_>>(),
        "meta": [
            {"k":"might","n":"Might","d":"+5% damage"},
            {"k":"hp","n":"Vitality","d":"+10 max HP"},
            {"k":"armor","n":"Plating","d":"+1 armor"},
            {"k":"speed","n":"Swiftness","d":"+4% speed"},
            {"k":"magnet","n":"Attraction","d":"+15% pickup"},
            {"k":"greed","n":"Greed","d":"+15% gold"},
            {"k":"reroll","n":"Reroll","d":"+1 reroll/run"},
            {"k":"choice","n":"Insight","d":"+1 card choice"},
            {"k":"revive","n":"Revive","d":"Revive once/run"}
        ],
        "bosses": BOSSES.iter().enumerate().map(|(i, b)| json!({
            "id": i, "t": b.time, "n": b.name, "hp": b.hp, "r": b.r, "spd": b.spd, "c": b.color,
        })).collect::<Vec<_>>(),
        "enemies": ENEMIES.iter().map(|e| json!({
            "k": e.key, "hp": e.hp, "spd": e.spd, "r": e.r, "dmg": e.dmg, "xp": e.xp, "c": e.color,
        })).collect::<Vec<_>>(),
        "mods": [
            {"id":"dash","n":"Dash"},
            {"id":"walls","n":"Walls"},
            {"id":"revive","n":"Revive"},
            {"id":"speed","n":"Proportional speed"}
        ],
        "profileKeys": {
            "weapons": WEAPON_KEYS,
            "passives": PASSIVE_KEYS,
            "mods": ["dash", "walls"]
        }
    })
    .to_string()
}
