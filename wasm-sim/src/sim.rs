//! Fixed-step simulation. The only source of randomness is `Game::rng`.

use crate::config::Input;
use crate::data::*;
use crate::game::*;
use std::collections::HashMap;

const TAU: f64 = 6.283_185_307_179_586;
const WALL_GRID: f64 = 24.0;

impl Game {
    pub fn step(&mut self, dt: f64, input: &Input) {
        if let Some(a) = input.auto {
            self.auto = a;
        }
        self.telemetry.steps += 1;

        if self.ended || self.over.is_some() {
            return;
        }
        if self.cfg.time_limit > 0.0 && self.t + 1e-9 >= self.cfg.time_limit {
            self.ended = true;
            self.timed_out = true;
            return;
        }

        // Choices pause game time, exactly like the legacy update loop.
        if self.choice.is_some() {
            if let Some(i) = input.pick {
                self.choose(i);
            } else {
                self.maybe_auto_pick(dt);
            }
            return;
        }

        self.t += dt;
        self.stats = self.compute_stats();

        // ---- movement ----
        let mut mx;
        let mut my;
        if self.auto {
            let (ax, ay) = self.autopilot();
            mx = ax;
            my = ay;
        } else {
            mx = input.mx;
            my = input.my;
            let l = (mx * mx + my * my).sqrt();
            if l > 1.0 {
                mx /= l;
                my /= l;
            }
        }

        // Auto dash / wall behaviour (matches simulator auto-play heuristics).
        if self.auto {
            if self.dash_lv > 0 && self.cfg.mods.dash && self.dash_cd <= 0.0 {
                let mut near = 0;
                for e in &self.enemies {
                    if !e.dead
                        && (e.x - self.p.x).powi(2) + (e.y - self.p.y).powi(2) < 120.0 * 120.0
                    {
                        near += 1;
                    }
                }
                if near >= 3 {
                    self.dash_now();
                }
            }
            if self.wall_lv > 0 && self.cfg.mods.walls && !self.wall_draw {
                self.wall_timer += dt;
                if self.wall_timer >= 8.0 {
                    self.wall_timer = 0.0;
                    self.place_wall();
                }
            }
        } else {
            if input.dash {
                self.dash_now();
            }
            if input.wall {
                self.place_wall();
            }
        }

        if let Some(d) = self.dash_v {
            self.p.x += d.vx * dt;
            self.p.y += d.vy * dt;
            self.dash_v = Some(Dash { t: d.t - dt, ..d });
            if self.dash_v.map(|v| v.t <= 0.0).unwrap_or(false) {
                self.dash_v = None;
            }
        } else {
            self.p.x += mx * self.stats.spd * dt;
            self.p.y += my * self.stats.spd * dt;
        }
        if mx != 0.0 || my != 0.0 {
            self.p.mdx = mx;
            self.p.mdy = my;
        }
        if mx != 0.0 {
            self.p.face = if mx > 0.0 { 1.0 } else { -1.0 };
        }
        self.p.walk += (mx * mx + my * my).sqrt() * dt * 12.0;
        self.p.inv = (self.p.inv - dt).max(0.0);
        self.p.moving = (mx * mx + my * my).sqrt() > 0.1;

        // ---- telemetry ----
        let dx = self.p.x - self.last_pos.0;
        let dy = self.p.y - self.last_pos.1;
        let dist = dx.hypot(dy);
        self.telemetry.total_pixels += dist;
        self.telemetry.max_travel = self.telemetry.max_travel.max(self.p.x.hypot(self.p.y));
        if dt > 0.0 {
            self.telemetry.max_speed = self.telemetry.max_speed.max(dist / dt);
        }
        if dist < 0.5 {
            self.telemetry.frames_stationary += 1;
            self.telemetry.last_stationary += dt;
            self.telemetry.max_linger =
                self.telemetry.max_linger.max(self.telemetry.last_stationary);
        } else {
            self.telemetry.last_stationary = 0.0;
        }
        self.last_pos = (self.p.x, self.p.y);

        // ---- spawning ----
        if !self.won {
            if self.boss_i < BOSS_COUNT
                && self.t >= BOSSES[self.boss_i].time
                && self.boss_id.is_none()
            {
                let i = self.boss_i;
                self.spawn_boss(i);
                self.boss_i += 1;
            }
            let rate = (0.9 + self.t / 28.0 + (self.t / 150.0).powi(2)).min(12.0)
                * if self.boss_id.is_some() { 0.55 } else { 1.0 }
                * (1.0 + 0.2 * self.cfg.curse as f64);
            self.spawn_acc += rate * dt;

            let mut pool: Vec<EType> = vec![EType::Blob, EType::Blob];
            if self.t > 25.0 {
                pool.push(EType::Bat);
                pool.push(EType::Bat);
            }
            if self.t > 80.0 {
                pool.push(EType::Skel);
            }
            if self.t > 140.0 {
                pool.push(EType::Ghost);
                pool.push(EType::Bat);
            }
            if self.t > 200.0 {
                pool.push(EType::Imp);
            }
            if self.t > 300.0 {
                pool.push(EType::Skel);
                pool.push(EType::Imp);
            }
            if self.phase == 3 {
                pool = vec![
                    EType::Vine,
                    EType::Vine,
                    EType::Frog,
                    EType::Skel,
                    EType::Bat,
                    EType::Imp,
                ];
            } else if self.phase >= 4 {
                pool = vec![
                    EType::Wisp,
                    EType::Wisp,
                    EType::Harpy,
                    EType::Ghost,
                    EType::Bat,
                    EType::Frog,
                ];
            }
            while self.spawn_acc >= 1.0 && self.enemies.len() < self.cfg.max_enemies {
                self.spawn_acc -= 1.0;
                let has_bat = pool.contains(&EType::Bat);
                if has_bat && self.rng.chance(0.04) {
                    let t = pool[self.rng.below(pool.len())];
                    for _ in 0..6 {
                        self.spawn_enemy_edge(t, false);
                    }
                } else {
                    let t = pool[self.rng.below(pool.len())];
                    self.spawn_enemy_edge(t, false);
                }
            }
            self.elite_t -= dt;
            if self.elite_t <= 0.0 {
                self.elite_t = 42.0;
                let t = pool[self.rng.below(pool.len())];
                self.spawn_enemy_edge(t, true);
            }
        }

        // ---- weapons / projectiles / hazards ----
        self.fire_weapons(dt);
        self.update_projectiles(dt);
        self.update_pools(dt);
        self.update_walls(dt);
        self.update_enemies(dt);
        self.update_ebullets(dt);
        self.update_tele(dt);
        self.update_gems(dt);

        if self.dash_cd > 0.0 {
            self.dash_cd = (self.dash_cd - dt).max(0.0);
        }

        if self.over.is_none() && self.choice.is_none() {
            if self.pending_open > 0.0 {
                self.pending_open -= dt;
                if self.pending_open <= 0.0 {
                    self.open_choice("lvl");
                }
            } else if self.queue > 0 {
                self.queue -= 1;
                self.open_choice("lvl");
            }
        }

        if self.won && self.win_timer > 0.0 && self.over.is_none() {
            self.win_timer -= dt;
            if self.win_timer <= 0.0 {
                self.end_run(true, "win");
            }
        }

        if self.cfg.time_limit > 0.0 && self.t + 1e-9 >= self.cfg.time_limit {
            self.ended = true;
            self.timed_out = true;
        }
    }

    // ---------------- auto-play ----------------

    pub fn autopilot(&self) -> (f64, f64) {
        let p = &self.p;
        let mut vx = 0.0;
        let mut vy = 0.0;
        for e in &self.enemies {
            if e.dead {
                continue;
            }
            let dx = p.x - e.x;
            let dy = p.y - e.y;
            let d = dx * dx + dy * dy;
            if d < 140.0 * 140.0 {
                let w = if e.boss { 6.0 } else { 1.0 } / d.max(25.0);
                vx += dx * w * 300.0;
                vy += dy * w * 300.0;
            }
        }
        for b in &self.ebullets {
            let dx = p.x - b.x;
            let dy = p.y - b.y;
            let d = dx * dx + dy * dy;
            if d < 70.0 * 70.0 && d > 1e-6 {
                vx += dx / d * 600.0;
                vy += dy / d * 600.0;
            }
        }
        for q in &self.tele {
            if q.fired {
                continue;
            }
            if q.kind == TeleKind::Circle {
                let dx = p.x - q.x;
                let dy = p.y - q.y;
                let d = dx.hypot(dy).max(1e-6);
                if d < q.r + 20.0 {
                    vx += dx / d * 4.0;
                    vy += dy / d * 4.0;
                }
            } else {
                let vx2 = q.x2 - q.x;
                let vy2 = q.y2 - q.y;
                let l = vx2.hypot(vy2).max(1e-6);
                let nx = -vy2 / l;
                let ny = vx2 / l;
                let sd = (p.x - q.x) * nx + (p.y - q.y) * ny;
                if sd.abs() < q.w / 2.0 + 24.0 {
                    let sg = if sd >= 0.0 { 1.0 } else { -1.0 };
                    vx += nx * sg * 4.0;
                    vy += ny * sg * 4.0;
                }
            }
        }
        let mut best: Option<&Gem> = None;
        let mut bd = 1e12;
        for q in &self.gems {
            let d = (q.x - p.x).powi(2) + (q.y - p.y).powi(2);
            if d < bd {
                bd = d;
                best = Some(q);
            }
        }
        if let Some(q) = best {
            if bd < 250.0 * 250.0 {
                let l = bd.sqrt().max(1e-6);
                vx += (q.x - p.x) / l * 0.8;
                vy += (q.y - p.y) / l * 0.8;
            }
        }
        vx += (self.t * 0.3).cos() * 0.2;
        vy += (self.t * 0.3).sin() * 0.2;
        let l = vx.hypot(vy);
        if l > 0.05 {
            (vx / l, vy / l)
        } else {
            (0.0, 0.0)
        }
    }

    // ---------------- dash / walls ----------------

    pub fn dash_now(&mut self) {
        if !self.cfg.mods.dash
            || self.dash_lv == 0
            || self.over.is_some()
            || self.choice.is_some()
            || self.dash_v.is_some()
            || self.dash_cd > 0.0
        {
            return;
        }
        let lv = self.dash_lv as f64;
        let dirx = if self.p.mdx != 0.0 { self.p.mdx } else { self.p.face };
        let diry = self.p.mdy;
        let l = dirx.hypot(diry).max(1e-6);
        let t = 0.13 + 0.015 * lv;
        self.dash_v = Some(Dash {
            vx: dirx / l * (600.0 + 60.0 * lv),
            vy: diry / l * (600.0 + 60.0 * lv),
            t,
        });
        self.dash_cd = (4.0 - 0.45 * lv).max(1.2);
        self.p.inv = self.p.inv.max(t + 0.1);
    }

    pub fn wall_cap(&self) -> usize {
        let base = [3, 4, 5, 6, 7][(self.wall_lv.max(1) - 1) as usize];
        (base + if self.wall_evo { 2 } else { 0 }) * 2
    }

    pub fn place_wall(&mut self) {
        if !self.cfg.mods.walls || self.over.is_some() || self.choice.is_some() {
            return;
        }
        let cap = self.wall_cap();
        let n = self.walls.iter().filter(|w| w.life > 0.0).count();
        if !self.wall_draw && n >= cap {
            return;
        }
        self.wall_draw = !self.wall_draw;
        if self.wall_draw {
            self.wall_cell = Some((
                (self.p.x / WALL_GRID).floor() as i32,
                (self.p.y / WALL_GRID).floor() as i32,
            ));
        } else {
            self.wall_cell = None;
        }
    }

    fn emit_wall_seg(&mut self, a: (i32, i32), b: (i32, i32)) -> bool {
        let x1 = (a.0 as f64 + 0.5) * WALL_GRID;
        let y1 = (a.1 as f64 + 0.5) * WALL_GRID;
        let x2 = (b.0 as f64 + 0.5) * WALL_GRID;
        let y2 = (b.1 as f64 + 0.5) * WALL_GRID;
        let cap = self.wall_cap();
        let n = self.walls.iter().filter(|w| w.life > 0.0).count();
        if n >= cap {
            self.wall_draw = false;
            self.wall_cell = None;
            return false;
        }
        let lv = self.wall_lv;
        let hp = if self.wall_evo {
            440.0
        } else {
            (16.0 + 12.0 * lv as f64) * 2.0
        };
        self.walls.push(Wall {
            x1,
            y1,
            x2,
            y2,
            th: if self.wall_evo { 14.0 } else { 10.0 } + lv as f64,
            hp,
            mhp: hp,
            life: if self.wall_evo {
                7.5
            } else {
                2.6 + lv as f64 * 0.7
            },
            tick: 0.0,
            laser: self.wall_evo,
        });
        true
    }

    fn update_walls(&mut self, dt: f64) {
        if self.wall_draw && self.wall_lv > 0 && self.cfg.mods.walls {
            let c = (
                (self.p.x / WALL_GRID).floor() as i32,
                (self.p.y / WALL_GRID).floor() as i32,
            );
            if self.wall_cell.is_none() {
                self.wall_cell = Some(c);
            } else {
                let mut a = self.wall_cell.unwrap();
                let mut guard = 0;
                while (a.0 != c.0 || a.1 != c.1) && guard < 6 {
                    guard += 1;
                    let dx = c.0 - a.0;
                    let dy = c.1 - a.1;
                    let n = if dx != 0 && dx.abs() >= dy.abs() {
                        (a.0 + dx.signum(), a.1)
                    } else if dy != 0 {
                        (a.0, a.1 + dy.signum())
                    } else {
                        break;
                    };
                    if !self.emit_wall_seg(a, n) {
                        break;
                    }
                    a = n;
                }
                self.wall_cell = Some(a);
            }
        }

        for w in self.walls.iter_mut() {
            w.life -= dt;
            if w.life <= 0.0 {
                continue;
            }
            if w.laser {
                w.tick -= dt;
            }
        }

        // Laser walls burn nearby enemies. The legacy browser build applied this
        // to the whole map (no distance check); `wall_laser_global` preserves
        // that quirk for parity, while the default is the fixed behaviour.
        let mut hits: Vec<(usize, f64)> = Vec::new();
        for w in self.walls.iter() {
            if !w.laser || w.life <= 0.0 || w.tick > 0.0 {
                continue;
            }
            for (ei, e) in self.enemies.iter().enumerate() {
                if e.dead || e.kind.phases_through_walls() {
                    continue;
                }
                let near = self.cfg.wall_laser_global
                    || seg_d2(e.x, e.y, w.x1, w.y1, w.x2, w.y2)
                        < (w.th * 0.5 + 40.0).powi(2);
                if near {
                    let dmg = (if e.boss { 6.0 } else { 26.0 + 8.0 * self.wall_lv as f64 }) * 0.4;
                    hits.push((ei, dmg));
                }
            }
        }
        for w in self.walls.iter_mut() {
            if w.laser && w.life > 0.0 && w.tick <= 0.0 {
                w.tick = 0.4;
            }
        }
        for (ei, dmg) in hits {
            self.hurt_enemy(ei, dmg, 0.0, 0.0, 0.0);
        }

        self.walls.retain(|w| w.life > 0.0 && w.hp > 0.0);
    }

    // ---------------- weapons ----------------

    fn fire_weapons(&mut self, dt: f64) {
        for k in 0..WEAPON_COUNT {
            self.cd[k] -= dt;
        }
        let evo = self.evo;
        let w = self.weapons;
        let s = self.stats;

        if w[W_BOLT] > 0 && self.cd[W_BOLT] <= 0.0 {
            let l = w[W_BOLT];
            let n = if evo[W_BOLT] {
                5
            } else {
                [1, 1, 2, 2, 3][(l - 1) as usize]
            };
            let dmg = if evo[W_BOLT] { 24.0 } else { 10.0 + 4.0 * l as f64 };
            self.cd[W_BOLT] = (if evo[W_BOLT] {
                0.4
            } else {
                [0.8, 0.75, 0.7, 0.6, 0.55][(l - 1) as usize]
            }) * s.cd;
            if let Some(t) = self.nearest(self.p.x, self.p.y, 150.0 * s.area) {
                let (tx, ty) = (self.enemies[t].x, self.enemies[t].y);
                let a0 = (ty - self.p.y).atan2(tx - self.p.x);
                for i in 0..n {
                    let a = a0 + (i as f64 - (n as f64 - 1.0) / 2.0) * 0.16;
                    self.projectiles.push(Projectile {
                        kind: ProjKind::Bolt,
                        key: 0,
                        x: self.p.x,
                        y: self.p.y,
                        vx: a.cos() * 300.0,
                        vy: a.sin() * 300.0,
                        dmg,
                        r: 5.0 * s.area,
                        life: 1.2,
                        age: 0.0,
                        pierce: if evo[W_BOLT] { 4 } else if l >= 5 { 2 } else { 1 },
                        rehit: 0.0,
                        hit: Vec::new(),
                    });
                }
            }
        }

        if w[W_ORB] > 0 {
            let l = w[W_ORB];
            let n = if evo[W_ORB] {
                8
            } else {
                [1, 2, 2, 3, 5][(l - 1) as usize]
            };
            let r = (if evo[W_ORB] { 75.0 } else { 48.0 }) * s.area;
            let dmg = if evo[W_ORB] { 20.0 } else { 6.0 + 3.0 * l as f64 };
            self.orb_a += dt * if evo[W_ORB] { 4.0 } else { 3.0 };
            let mut candidates: Vec<usize> = Vec::new();
            for i in 0..n {
                let a = self.orb_a + i as f64 * TAU / n as f64;
                let ox = self.p.x + a.cos() * r;
                let oy = self.p.y + a.sin() * r;
                for (ei, e) in self.enemies.iter().enumerate() {
                    if e.dead || e.hit_orb > self.t {
                        continue;
                    }
                    if (e.x - ox).powi(2) + (e.y - oy).powi(2)
                        < (e.r + 8.0 * s.area).powi(2)
                        && !candidates.contains(&ei)
                    {
                        candidates.push(ei);
                    }
                }
            }
            for ei in candidates {
                self.enemies[ei].hit_orb = self.t + 0.4;
                let (ex, ey) = (self.enemies[ei].x, self.enemies[ei].y);
                self.hurt_enemy(ei, dmg, 40.0, ex - self.p.x, ey - self.p.y);
            }
        }

        if w[W_RAIN] > 0 && self.cd[W_RAIN] <= 0.0 {
            let l = w[W_RAIN];
            let n = if evo[W_RAIN] {
                5
            } else {
                [1, 1, 2, 2, 3][(l - 1) as usize]
            };
            self.cd[W_RAIN] = (if evo[W_RAIN] { 2.0 } else { 3.0 }) * s.cd;
            for _ in 0..n {
                // Legacy: uniform index over the whole (possibly dead) array.
                let t = if self.enemies.is_empty() {
                    None
                } else {
                    Some(self.rng.below(self.enemies.len()))
                };
                let near = t
                    .map(|i| {
                        (self.enemies[i].x - self.p.x).powi(2)
                            + (self.enemies[i].y - self.p.y).powi(2)
                            < 250.0 * 250.0
                    })
                    .unwrap_or(false);
                let (tx, ty) = if near {
                    let i = t.unwrap();
                    (self.enemies[i].x, self.enemies[i].y)
                } else {
                    (
                        self.p.x + self.rng.range(-100.0, 100.0),
                        self.p.y + self.rng.range(-100.0, 100.0),
                    )
                };
                self.pools.push(Pool {
                    x: tx,
                    y: ty,
                    r: (if evo[W_RAIN] { 46.0 } else { 26.0 + 3.0 * l as f64 }) * s.area,
                    life: if evo[W_RAIN] { 3.5 } else { 2.6 },
                    dmg: if evo[W_RAIN] { 16.0 } else { 5.0 + 2.0 * l as f64 },
                    tick: 0.0,
                    slow: evo[W_RAIN],
                });
            }
        }

        if w[W_AURA] > 0 {
            let l = w[W_AURA];
            let r = (if evo[W_AURA] {
                95.0
            } else {
                [38.0, 44.0, 52.0, 58.0, 68.0][(l - 1) as usize]
            }) * s.area;
            let dmg = if evo[W_AURA] {
                16.0
            } else {
                [4.0, 5.0, 7.0, 8.0, 10.0][(l - 1) as usize]
            };
            self.aura_r = r;
            self.aura_t -= dt;
            if self.aura_t <= 0.0 {
                self.aura_t = 0.5;
                let kb = if l >= 5 || evo[W_AURA] { 50.0 } else { 15.0 };
                let mut hits = Vec::new();
                for (ei, e) in self.enemies.iter().enumerate() {
                    if !e.dead && (e.x - self.p.x).powi(2) + (e.y - self.p.y).powi(2) < (r + e.r).powi(2) {
                        hits.push(ei);
                    }
                }
                for ei in hits {
                    let (ex, ey) = (self.enemies[ei].x, self.enemies[ei].y);
                    self.hurt_enemy(ei, dmg, kb, ex - self.p.x, ey - self.p.y);
                }
                if evo[W_AURA] {
                    self.p.hp = (self.p.hp + 0.6).min(self.p.mhp);
                }
            }
        } else {
            self.aura_r = 0.0;
        }

        if w[W_BOOM] > 0 && self.cd[W_BOOM] <= 0.0 {
            let l = w[W_BOOM];
            let n = if evo[W_BOOM] {
                6
            } else {
                [1, 1, 2, 2, 3][(l - 1) as usize]
            };
            let dmg = if evo[W_BOOM] { 26.0 } else { 10.0 + 4.0 * l as f64 };
            self.cd[W_BOOM] = (if evo[W_BOOM] { 1.1 } else { 1.6 }) * s.cd;
            let a0 = match self.nearest(self.p.x, self.p.y, 150.0 * s.area) {
                Some(i) => (self.enemies[i].y - self.p.y).atan2(self.enemies[i].x - self.p.x),
                None => self.rng.next_f64() * TAU,
            };
            for i in 0..n {
                let a = a0 + i as f64 * TAU / n as f64 * if n > 1 { 1.0 } else { 0.0 };
                self.projectiles.push(Projectile {
                    kind: ProjKind::Boom,
                    key: a.to_bits(),
                    x: self.p.x,
                    y: self.p.y,
                    vx: a.cos() * 240.0,
                    vy: a.sin() * 240.0,
                    dmg,
                    r: 9.0 * s.area,
                    life: if evo[W_BOOM] { 2.2 } else { 1.6 },
                    age: 0.0,
                    pierce: 999,
                    rehit: 0.35,
                    hit: Vec::new(),
                });
            }
        }

        if w[W_ZAP] > 0 && self.cd[W_ZAP] <= 0.0 {
            let l = w[W_ZAP];
            let n = if evo[W_ZAP] {
                10
            } else {
                [2, 3, 3, 4, 6][(l - 1) as usize]
            };
            let dmg = if evo[W_ZAP] { 40.0 } else { 14.0 + 5.0 * l as f64 };
            self.cd[W_ZAP] = (if evo[W_ZAP] { 1.2 } else { 2.2 }) * s.cd;
            let (hw, hh) = self.view_half();
            let mut vis: Vec<usize> = self
                .enemies
                .iter()
                .enumerate()
                .filter(|(_, e)| (e.x - self.p.x).abs() < hw && (e.y - self.p.y).abs() < hh)
                .map(|(i, _)| i)
                .collect();
            for _ in 0..n {
                if vis.is_empty() {
                    break;
                }
                let k = ((self.rng.next_f64() * vis.len() as f64) as usize).min(vis.len() - 1);
                let ei = vis.remove(k);
                self.hurt_enemy(ei, dmg, 0.0, 0.0, 0.0);
                if evo[W_ZAP] {
                    let (ex, ey) = (self.enemies[ei].x, self.enemies[ei].y);
                    let mut chain = Vec::new();
                    for (oi, o) in self.enemies.iter().enumerate() {
                        if !o.dead
                            && oi != ei
                            && (o.x - ex).powi(2) + (o.y - ey).powi(2) < 40.0 * 40.0
                        {
                            chain.push(oi);
                        }
                    }
                    for oi in chain {
                        self.hurt_enemy(oi, dmg * 0.5, 0.0, 0.0, 0.0);
                    }
                }
            }
        }
    }

    fn update_projectiles(&mut self, dt: f64) {
        let mut i = 0;
        while i < self.projectiles.len() {
            let kind = self.projectiles[i].kind;
            self.projectiles[i].life -= dt;
            if kind == ProjKind::Boom {
                self.projectiles[i].age += dt;
                let age = self.projectiles[i].age;
                let life = self.projectiles[i].life;
                if age > life * 0.45 {
                    let dx = self.p.x - self.projectiles[i].x;
                    let dy = self.p.y - self.projectiles[i].y;
                    let l = dx.hypot(dy).max(1e-6);
                    self.projectiles[i].vx += (dx / l * 600.0 - self.projectiles[i].vx) * dt * 4.0;
                    self.projectiles[i].vy += (dy / l * 600.0 - self.projectiles[i].vy) * dt * 4.0;
                    if l < 10.0 && age > 0.4 {
                        self.projectiles[i].life = 0.0;
                    }
                }
            }
            self.projectiles[i].x += self.projectiles[i].vx * dt;
            self.projectiles[i].y += self.projectiles[i].vy * dt;

            if self.projectiles[i].life > 0.0 {
                let px = self.projectiles[i].x;
                let py = self.projectiles[i].y;
                let pr = self.projectiles[i].r;
                let dmg = self.projectiles[i].dmg;
                let vx = self.projectiles[i].vx;
                let vy = self.projectiles[i].vy;
                let rehit = self.projectiles[i].rehit;
                let key = self.projectiles[i].key;
                let mut hits: Vec<usize> = Vec::new();
                for ei in 0..self.enemies.len() {
                    if self.enemies[ei].dead {
                        continue;
                    }
                    if (self.enemies[ei].x - px).powi(2) + (self.enemies[ei].y - py).powi(2)
                        >= (self.enemies[ei].r + pr).powi(2)
                    {
                        continue;
                    }
                    if kind == ProjKind::Boom {
                        let prev = self.enemies[ei]
                            .hit_boom
                            .iter()
                            .find(|(k, _)| *k == key)
                            .map(|(_, t)| *t)
                            .unwrap_or(0.0);
                        if prev > self.t {
                            continue;
                        }
                        if let Some(entry) =
                            self.enemies[ei].hit_boom.iter_mut().find(|(k, _)| *k == key)
                        {
                            entry.1 = self.t + rehit;
                        } else {
                            self.enemies[ei].hit_boom.push((key, self.t + rehit));
                        }
                    } else {
                        let id = self.enemies[ei].id;
                        if self.projectiles[i].hit.contains(&id) {
                            continue;
                        }
                        self.projectiles[i].hit.push(id);
                    }
                    hits.push(ei);
                }
                for ei in hits {
                    self.hurt_enemy(ei, dmg, if kind == ProjKind::Bolt { 25.0 } else { 35.0 }, vx, vy);
                    if kind == ProjKind::Bolt {
                        self.projectiles[i].pierce -= 1;
                        if self.projectiles[i].pierce <= 0 {
                            self.projectiles[i].life = 0.0;
                        }
                    }
                }
            }
            i += 1;
        }
        self.projectiles.retain(|p| p.life > 0.0);
    }

    fn update_pools(&mut self, dt: f64) {
        for q in self.pools.iter_mut() {
            q.life -= dt;
            q.tick -= dt;
        }
        let mut damage: Vec<(usize, f64, bool)> = Vec::new();
        for q in &self.pools {
            if q.life <= 0.0 || q.tick > 0.0 {
                continue;
            }
            for (ei, e) in self.enemies.iter().enumerate() {
                if !e.dead
                    && (e.x - q.x).powi(2) + (e.y - q.y).powi(2) < (q.r + e.r).powi(2)
                {
                    damage.push((ei, q.dmg, q.slow));
                }
            }
        }
        for q in self.pools.iter_mut() {
            if q.life > 0.0 && q.tick <= 0.0 {
                q.tick = 0.4;
            }
        }
        for (ei, dmg, slow) in damage {
            self.hurt_enemy(ei, dmg, 0.0, 0.0, 0.0);
            if slow && ei < self.enemies.len() {
                self.enemies[ei].slow_t = 0.5;
            }
        }
        self.pools.retain(|q| q.life > 0.0);
    }

    // ---------------- enemies ----------------

    fn update_enemies(&mut self, dt: f64) {
        let cs = 24.0;
        let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        // Legacy builds the separation grid over the whole array, including
        // enemies killed earlier in the same frame.
        for (i, e) in self.enemies.iter().enumerate() {
            grid.entry(((e.x / cs) as i32, (e.y / cs) as i32))
                .or_default()
                .push(i);
        }

        let mut wall_dmg: Vec<(usize, f64)> = Vec::new();

        for i in 0..self.enemies.len() {
            if self.enemies[i].dead {
                continue;
            }
            self.enemies[i].flash = (self.enemies[i].flash - dt).max(0.0);
            self.enemies[i].ph += dt;

            let ex = self.enemies[i].x;
            let ey = self.enemies[i].y;
            let dx = self.p.x - ex;
            let dy = self.p.y - ey;
            let l = dx.hypot(dy).max(1e-6);
            let kind = self.enemies[i].kind;
            let boss = self.enemies[i].boss;
            let mut sp = self.enemies[i].spd * if self.enemies[i].slow_t > 0.0 { 0.5 } else { 1.0 };
            self.enemies[i].slow_t -= dt;

            let mut ebullet: Option<(f64, f64, f64, f64, f64, &'static str, f64)> = None;

            match kind {
                EType::Imp if !boss && l < 150.0 => {
                    sp *= -0.3;
                    self.enemies[i].shoot_t -= dt;
                    if self.enemies[i].shoot_t <= 0.0 {
                        self.enemies[i].shoot_t = self.rng.range(2.5, 4.0);
                        ebullet = Some((ex, ey, dx / l * 110.0, dy / l * 110.0, 5.0, "#c8452f", 0.0));
                    }
                }
                EType::Bat if !boss => {
                    sp *= 1.0 + 0.3 * (self.enemies[i].ph * 5.0).sin();
                }
                EType::Vine if !boss => {
                    let s3 = (self.enemies[i].ph * 6.0).sin() * 0.9;
                    let ox = -dy / l * s3;
                    let oy = dx / l * s3;
                    self.enemies[i].x += ox * sp * dt;
                    self.enemies[i].y += oy * sp * dt;
                }
                EType::Frog if !boss => {
                    let cyc = self.enemies[i].ph % 1.6;
                    if cyc < 1.1 {
                        sp = 0.0;
                    } else {
                        sp *= 7.0;
                    }
                }
                EType::Wisp if !boss => {
                    let a = dy.atan2(dx) + if l < 120.0 { 1.3 } else { 0.5 };
                    self.enemies[i].x += a.cos() * sp * dt - dx / l * sp * dt;
                    self.enemies[i].y += a.sin() * sp * dt - dy / l * sp * dt;
                }
                EType::Harpy if !boss && l < 190.0 => {
                    sp *= -0.2;
                    let st = self.enemies[i].shoot_t;
                    self.enemies[i].shoot_t = (if st == 0.0 { self.rng.range(1.0, 3.0) } else { st }) - dt;
                    if self.enemies[i].shoot_t <= 0.0 {
                        self.enemies[i].shoot_t = self.rng.range(3.0, 4.5);
                        let a0 = dy.atan2(dx);
                        for k in -1..=1 {
                            let a = a0 + k as f64 * 0.3;
                            self.ebullets.push(EBullet {
                                x: ex,
                                y: ey,
                                vx: a.cos() * 120.0,
                                vy: a.sin() * 120.0,
                                r: 4.0,
                                dmg: self.enemies[i].dmg,
                                life: 3.0,
                                color: "#eaf4ff",
                                tw: 0.0,
                                split: None,
                                orb: false,
                                ox: 0.0,
                                oy: 0.0,
                                ang: 0.0,
                                dang: 0.0,
                                rad: 0.0,
                                drad: 0.0,
                            });
                        }
                    }
                }
                _ => {}
            }

            if boss {
                self.boss_ai(i, dx, dy, l, dt);
            } else {
                self.enemies[i].x += dx / l * sp * dt;
                self.enemies[i].y += dy / l * sp * dt;
            }

            self.enemies[i].x += self.enemies[i].kx * dt;
            self.enemies[i].y += self.enemies[i].ky * dt;
            let decay = 0.02f64.powf(dt);
            self.enemies[i].kx *= decay;
            self.enemies[i].ky *= decay;

            if let Some((x, y, vx, vy, r, c, tw)) = ebullet {
                self.ebullets.push(EBullet {
                    x,
                    y,
                    vx,
                    vy,
                    r,
                    dmg: self.enemies[i].dmg,
                    life: 4.0,
                    color: c,
                    tw,
                    split: None,
                    orb: false,
                    ox: 0.0,
                    oy: 0.0,
                    ang: 0.0,
                    dang: 0.0,
                    rad: 0.0,
                    drad: 0.0,
                });
            }

            // walls
            if !self.walls.is_empty() {
                let kind = self.enemies[i].kind;
                if !kind.phases_through_walls() {
                    for wi in 0..self.walls.len() {
                        let w = &self.walls[wi];
                        if w.life <= 0.0 {
                            continue;
                        }
                        if !w.laser && kind.flies() {
                            continue;
                        }
                        let dseg = seg_d2(self.enemies[i].x, self.enemies[i].y, w.x1, w.y1, w.x2, w.y2);
                        let rr = self.enemies[i].r + w.th / 2.0;
                        if dseg < rr * rr {
                            if self.enemies[i].boss {
                                wall_dmg.push((wi, self.enemies[i].dmg * 16.0 * dt));
                            } else {
                                let vx2 = w.x2 - w.x1;
                                let vy2 = w.y2 - w.y1;
                                let l2 = (vx2 * vx2 + vy2 * vy2).max(1e-9);
                                let mut t2 = ((self.enemies[i].x - w.x1) * vx2
                                    + (self.enemies[i].y - w.y1) * vy2)
                                    / l2;
                                t2 = t2.clamp(0.0, 1.0);
                                let qx = w.x1 + vx2 * t2;
                                let qy = w.y1 + vy2 * t2;
                                let mut ox = self.enemies[i].x - qx;
                                let mut oy = self.enemies[i].y - qy;
                                let d = ox.hypot(oy).max(0.01);
                                let push = rr - d + 0.5;
                                ox /= d;
                                oy /= d;
                                self.enemies[i].x += ox * push;
                                self.enemies[i].y += oy * push;
                            }
                        } else if dseg < (rr + 6.0).powi(2) {
                            wall_dmg.push((
                                wi,
                                self.enemies[i].dmg
                                    * 1.2
                                    * dt
                                    * if self.enemies[i].elite { 1.6 } else { 1.0 },
                            ));
                        }
                    }
                }
            }

            // separation
            if !boss && kind != EType::Ghost {
                let (cx, cy) = (
                    (self.enemies[i].x / cs) as i32,
                    (self.enemies[i].y / cs) as i32,
                );
                let mut px = 0.0;
                let mut py = 0.0;
                for oi in -1..=1 {
                    for oj in -1..=1 {
                        if let Some(list) = grid.get(&(cx + oi, cy + oj)) {
                            for &j in list {
                                if j == i {
                                    continue;
                                }
                                let ox = self.enemies[i].x - self.enemies[j].x;
                                let oy = self.enemies[i].y - self.enemies[j].y;
                                let dd = ox * ox + oy * oy;
                                let rr2 = (self.enemies[i].r + self.enemies[j].r) * 0.9;
                                if dd < rr2 * rr2 && dd > 0.01 {
                                    let d = dd.sqrt();
                                    let push = (rr2 - d) * 0.5;
                                    px += ox / d * push;
                                    py += oy / d * push;
                                }
                            }
                        }
                    }
                }
                self.enemies[i].x += px;
                self.enemies[i].y += py;
            }

            let hit = l < self.enemies[i].r + 8.0;
            let hit_dmg = self.enemies[i].dmg;
            if hit {
                self.hurt_player(hit_dmg);
            }

            if !boss {
                let (hw, hh) = self.view_half();
                if (self.enemies[i].x - self.p.x).abs() > hw + 140.0
                    || (self.enemies[i].y - self.p.y).abs() > hh + 140.0
                {
                    let (nx, ny) = self.edge_pt(18.0);
                    self.enemies[i].x = nx;
                    self.enemies[i].y = ny;
                }
            }
        }

        for (wi, dmg) in wall_dmg {
            if wi < self.walls.len() {
                self.walls[wi].hp -= dmg;
            }
        }
        self.enemies.retain(|e| !e.dead);
    }

    fn boss_phase(&mut self, i: usize) -> u8 {
        let f = if self.enemies[i].mhp > 0.0 {
            self.enemies[i].hp / self.enemies[i].mhp
        } else {
            0.0
        };
        let ph = if f > 0.66 {
            1
        } else if f > 0.33 {
            2
        } else {
            3
        };
        if ph != self.enemies[i].bph {
            let first = self.enemies[i].bph == 0;
            self.enemies[i].bph = ph;
            self.enemies[i].seq = 0;
            if !first {
                self.ebullets.retain(|b| !b.orb);
                self.enemies[i].atk = 1.4;
            }
        }
        ph
    }

    fn boss_ai(&mut self, i: usize, dx: f64, dy: f64, l: f64, dt: f64) {
        let bi = self.enemies[i].bi;
        // Legacy decrements the attack timer before dispatching to any boss AI.
        self.enemies[i].atk -= dt;
        if bi == 3 {
            self.thorn_ai(i, dx, dy, l, dt);
            return;
        }
        if bi == 4 {
            self.storm_ai(i, dx, dy, dt);
            return;
        }
        if self.enemies[i].dash > 0.0 {
            let e = &mut self.enemies[i];
            e.dash -= dt;
            e.x += e.dvx * dt;
            e.y += e.dvy * dt;
            return;
        }
        let bo = self.t - BOSSES[bi].time;
        let enr = if bo > 150.0 {
            3.0
        } else if bo > 90.0 {
            1.8
        } else {
            1.0
        };
        {
            let e = &mut self.enemies[i];
            e.x += dx / l * e.spd * enr * dt;
            e.y += dy / l * e.spd * enr * dt;
        }
        if self.enemies[i].atk > 0.0 {
            return;
        }
        match bi {
            0 => {
                self.enemies[i].atk = 3.0;
                let (bx, by) = (self.enemies[i].x, self.enemies[i].y);
                for _ in 0..4 {
                    self.spawn_minion(EType::Blob, bx, by, 30.0);
                }
            }
            1 => {
                {
                    let e = &mut self.enemies[i];
                    e.atk = 2.6;
                    e.dash = 0.6;
                    e.dvx = dx / l * 280.0;
                    e.dvy = dy / l * 280.0;
                }
                let (bx, by) = (self.enemies[i].x, self.enemies[i].y);
                for _ in 0..4 {
                    self.spawn_minion(EType::Bat, bx, by, 0.0);
                }
            }
            _ => {
                self.enemies[i].atk = 2.2;
                let (bx, by) = (self.enemies[i].x, self.enemies[i].y);
                let o = self.rng.next_f64();
                let n = 14;
                for k in 0..n {
                    let a = o + k as f64 * TAU / n as f64;
                    self.ebullets.push(EBullet {
                        x: bx,
                        y: by,
                        vx: a.cos() * 95.0,
                        vy: a.sin() * 95.0,
                        r: 6.0,
                        dmg: 12.0,
                        life: 5.0,
                        color: "#c8452f",
                        tw: 0.0,
                        split: None,
                        orb: false,
                        ox: 0.0,
                        oy: 0.0,
                        ang: 0.0,
                        dang: 0.0,
                        rad: 0.0,
                        drad: 0.0,
                    });
                }
            }
        }
    }

    /// Boss minions: spawn at the edge first (legacy RNG order), then snap to
    /// the boss with a random offset.
    fn spawn_minion(&mut self, kind: EType, cx: f64, cy: f64, spread: f64) {
        let idx = self.spawn_enemy_edge(kind, false);
        let (ox, oy) = if spread > 0.0 {
            (
                self.rng.range(-spread, spread),
                self.rng.range(-spread, spread),
            )
        } else {
            (0.0, 0.0)
        };
        self.enemies[idx].x = cx + ox;
        self.enemies[idx].y = cy + oy;
    }

    fn thorn_ai(&mut self, i: usize, dx: f64, dy: f64, l: f64, dt: f64) {
        let ph = self.boss_phase(i);
        if self.enemies[i].burrow > 0.0 {
            self.enemies[i].burrow -= dt;
            if self.enemies[i].burrow <= 0.0 {
                let (bx, by) = (self.enemies[i].bx, self.enemies[i].by);
                self.enemies[i].x = bx;
                self.enemies[i].y = by;
                for k in 0..12 {
                    let a = k as f64 * 0.5236;
                    self.ebullets.push(EBullet {
                        x: bx,
                        y: by,
                        vx: a.cos() * 100.0,
                        vy: a.sin() * 100.0,
                        r: 5.0,
                        dmg: 12.0,
                        life: 3.0,
                        color: "#9be86a",
                        tw: 0.0,
                        split: None,
                        orb: false,
                        ox: 0.0,
                        oy: 0.0,
                        ang: 0.0,
                        dang: 0.0,
                        rad: 0.0,
                        drad: 0.0,
                    });
                }
            }
            return;
        }
        if l > 270.0 && self.enemies[i].atk > 0.5 {
            let a = self.rng.next_f64() * TAU;
            let bx = self.p.x + a.cos() * 90.0;
            let by = self.p.y + a.sin() * 90.0;
            self.enemies[i].bx = bx;
            self.enemies[i].by = by;
            self.enemies[i].burrow = 1.1;
            self.tele.push(Tele {
                kind: TeleKind::Circle,
                x: bx,
                y: by,
                x2: 0.0,
                y2: 0.0,
                r: 48.0,
                w: 0.0,
                t: 0.0,
                fire_t: 1.1,
                fired: false,
                post: 0.0,
                dmg: 20.0,
                color: "#8a5a2a",
            });
            return;
        }
        let sp = if ph == 2 {
            0.0
        } else {
            self.enemies[i].spd * if ph == 3 { 1.35 } else { 1.0 }
        };
        self.enemies[i].x += dx / l * sp * dt;
        self.enemies[i].y += dy / l * sp * dt;

        if self.enemies[i].spiral > 0.0 {
            self.enemies[i].spiral -= dt;
            self.enemies[i].s_t -= dt;
            if self.enemies[i].s_t <= 0.0 {
                self.enemies[i].s_t = if ph == 3 { 0.07 } else { 0.09 };
                self.enemies[i].s_a += 0.32;
                let arms = if ph == 3 { 3 } else { 2 };
                let (bx, by, sa) = (
                    self.enemies[i].x,
                    self.enemies[i].y,
                    self.enemies[i].s_a,
                );
                for k in 0..arms {
                    let a = sa + k as f64 * TAU / arms as f64;
                    self.ebullets.push(EBullet {
                        x: bx,
                        y: by,
                        vx: a.cos() * 85.0,
                        vy: a.sin() * 85.0,
                        r: 5.0,
                        dmg: 12.0,
                        life: 5.0,
                        color: "#9be86a",
                        tw: 0.0,
                        split: None,
                        orb: false,
                        ox: 0.0,
                        oy: 0.0,
                        ang: 0.0,
                        dang: 0.0,
                        rad: 0.0,
                        drad: 0.0,
                    });
                }
            }
        }
        if self.enemies[i].atk > 0.0 {
            return;
        }
        let pats: &[&str] = if ph == 1 {
            &["spiral", "snare"]
        } else if ph == 2 {
            &["lash", "pods", "snare", "lash"]
        } else {
            &["spiral", "lash", "snare", "pods"]
        };
        let k = pats[self.enemies[i].seq as usize % pats.len()];
        self.enemies[i].seq += 1;
        self.enemies[i].atk = if ph == 1 {
            2.8
        } else if ph == 2 {
            2.3
        } else {
            1.7
        };
        match k {
            "spiral" => self.enemies[i].spiral = 1.8,
            "snare" => {
                let n = if ph == 3 { 7 } else { 5 };
                for k in 0..n {
                    let a = self.rng.next_f64() * TAU;
                    let d = if k > 0 { self.rng.range(40.0, 130.0) } else { 0.0 };
                    self.tele.push(Tele {
                        kind: TeleKind::Circle,
                        x: self.p.x + a.cos() * d,
                        y: self.p.y + a.sin() * d,
                        x2: 0.0,
                        y2: 0.0,
                        r: 30.0,
                        w: 0.0,
                        t: 0.0,
                        fire_t: 0.95,
                        fired: false,
                        post: 0.0,
                        dmg: 16.0,
                        color: "#e85a9a",
                    });
                }
                if ph == 3 {
                    let (bx, by) = (self.enemies[i].x, self.enemies[i].y);
                    for _ in 0..3 {
                        self.spawn_minion(EType::Frog, bx, by, 40.0);
                    }
                }
            }
            "lash" => {
                let n = if ph == 3 { 5 } else { 3 };
                let a0 = dy.atan2(dx);
                let (bx, by) = (self.enemies[i].x, self.enemies[i].y);
                for k in 0..n {
                    let a = a0 + (k as f64 - (n as f64 - 1.0) / 2.0) * 0.38;
                    self.tele.push(Tele {
                        kind: TeleKind::Line,
                        x: bx,
                        y: by,
                        x2: bx + a.cos() * 560.0,
                        y2: by + a.sin() * 560.0,
                        r: 0.0,
                        w: 22.0,
                        t: 0.0,
                        fire_t: 0.85,
                        fired: false,
                        post: 0.0,
                        dmg: 18.0,
                        color: "#7dff6a",
                    });
                }
            }
            _ => {
                let n = if ph == 3 { 8 } else { 6 };
                let (bx, by) = (self.enemies[i].x, self.enemies[i].y);
                for k in 0..n {
                    let a = k as f64 * TAU / n as f64 + self.rng.next_f64() * 0.3;
                    self.ebullets.push(EBullet {
                        x: bx,
                        y: by,
                        vx: a.cos() * 70.0,
                        vy: a.sin() * 70.0,
                        r: 8.0,
                        dmg: 14.0,
                        life: 3.0,
                        color: "#5ac83a",
                        tw: 0.0,
                        split: Some(1.3),
                        orb: false,
                        ox: 0.0,
                        oy: 0.0,
                        ang: 0.0,
                        dang: 0.0,
                        rad: 0.0,
                        drad: 0.0,
                    });
                }
            }
        }
    }

    fn storm_ai(&mut self, i: usize, dx: f64, dy: f64, dt: f64) {
        let ph = self.boss_phase(i);
        if self.enemies[i].hide > 0.0 {
            self.enemies[i].hide -= dt;
            if self.enemies[i].hide <= 0.0 {
                let (bx, by) = (self.enemies[i].bx, self.enemies[i].by);
                self.enemies[i].x = bx;
                self.enemies[i].y = by;
                let n = if ph == 3 { 20 } else { 16 };
                for k in 0..n {
                    let a = k as f64 * TAU / n as f64;
                    self.ebullets.push(EBullet {
                        x: bx,
                        y: by,
                        vx: a.cos() * 120.0,
                        vy: a.sin() * 120.0,
                        r: 5.0,
                        dmg: 14.0,
                        life: 4.0,
                        color: "#cfe6ff",
                        tw: 0.0,
                        split: None,
                        orb: false,
                        ox: 0.0,
                        oy: 0.0,
                        ang: 0.0,
                        dang: 0.0,
                        rad: 0.0,
                        drad: 0.0,
                    });
                }
            }
            return;
        }
        let want = if ph == 3 { 190.0 } else { 135.0 };
        let ang0 = (-dy).atan2(-dx) + dt * if ph == 1 { 0.6 } else { 0.9 };
        let tx = self.p.x + ang0.cos() * want;
        let ty = self.p.y + ang0.sin() * want;
        let mx = tx - self.enemies[i].x;
        let my = ty - self.enemies[i].y;
        let ml = mx.hypot(my).max(1e-6);
        let v = (ml / dt).min(self.enemies[i].spd * if ph == 3 { 1.5 } else { 1.2 });
        self.enemies[i].x += mx / ml * v * dt;
        self.enemies[i].y += my / ml * v * dt;

        if self.enemies[i].atk > 0.0 {
            return;
        }
        let pats: &[&str] = if ph == 1 {
            &["grid", "gale"]
        } else if ph == 2 {
            &["blink", "grid", "gale"]
        } else {
            &["eye", "blink", "grid", "gale"]
        };
        let k = pats[self.enemies[i].seq as usize % pats.len()];
        self.enemies[i].seq += 1;
        self.enemies[i].atk = if ph == 1 {
            2.9
        } else if ph == 2 {
            2.5
        } else {
            2.2
        };
        let (vw, vh) = self.view_half();
        let (bx, by) = (self.enemies[i].x, self.enemies[i].y);
        match k {
            "grid" => {
                let vert = self.enemies[i].seq % 2 == 0;
                let o = self.rng.range(-30.0, 30.0);
                for j in -2i32..=2 {
                    if vert {
                        self.tele.push(Tele {
                            kind: TeleKind::Line,
                            x: self.p.x + j as f64 * 80.0 + o,
                            y: self.p.y - (vh + 20.0),
                            x2: self.p.x + j as f64 * 80.0 + o,
                            y2: self.p.y + vh + 20.0,
                            r: 0.0,
                            w: 26.0,
                            t: 0.0,
                            fire_t: 1.05 + j.abs() as f64 * 0.12,
                            fired: false,
                            post: 0.0,
                            dmg: 20.0,
                            color: "#ff9a1a",
                        });
                    } else {
                        self.tele.push(Tele {
                            kind: TeleKind::Line,
                            x: self.p.x - (vw + 20.0),
                            y: self.p.y + j as f64 * 80.0 + o,
                            x2: self.p.x + vw + 20.0,
                            y2: self.p.y + j as f64 * 80.0 + o,
                            r: 0.0,
                            w: 26.0,
                            t: 0.0,
                            fire_t: 1.05 + j.abs() as f64 * 0.12,
                            fired: false,
                            post: 0.0,
                            dmg: 20.0,
                            color: "#ff9a1a",
                        });
                    }
                }
                if ph >= 2 {
                    self.enemies[i].atk += 0.6;
                }
            }
            "gale" => {
                let n = if ph == 1 { 8 } else { 12 };
                let a0 = dy.atan2(dx);
                for k in 0..n {
                    let a = a0 + (k as f64 - (n as f64 - 1.0) / 2.0) * 0.14;
                    self.ebullets.push(EBullet {
                        x: bx,
                        y: by,
                        vx: a.cos() * 115.0,
                        vy: a.sin() * 115.0,
                        r: 4.5,
                        dmg: 12.0,
                        life: 4.5,
                        color: "#ffffff",
                        tw: if k % 2 == 0 { -0.9 } else { 0.9 },
                        split: None,
                        orb: false,
                        ox: 0.0,
                        oy: 0.0,
                        ang: 0.0,
                        dang: 0.0,
                        rad: 0.0,
                        drad: 0.0,
                    });
                }
            }
            "blink" => {
                let a = (self.p.y - self.enemies[i].y).atan2(self.p.x - self.enemies[i].x)
                    + std::f64::consts::PI
                    + self.rng.range(-0.6, 0.6);
                let bx2 = self.p.x + a.cos() * 110.0;
                let by2 = self.p.y + a.sin() * 110.0;
                self.enemies[i].bx = bx2;
                self.enemies[i].by = by2;
                self.enemies[i].hide = 0.8;
                self.tele.push(Tele {
                    kind: TeleKind::Circle,
                    x: bx2,
                    y: by2,
                    x2: 0.0,
                    y2: 0.0,
                    r: 40.0,
                    w: 0.0,
                    t: 0.0,
                    fire_t: 0.8,
                    fired: false,
                    post: 0.0,
                    dmg: 22.0,
                    color: "#9fd3ff",
                });
            }
            _ => {
                // eye
                let n = 30usize;
                let gap = self.rng.below(n);
                let ox = self.p.x;
                let oy = self.p.y;
                let dir = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
                self.tele.push(Tele {
                    kind: TeleKind::Circle,
                    x: ox,
                    y: oy,
                    x2: 0.0,
                    y2: 0.0,
                    r: 230.0,
                    w: 0.0,
                    t: 0.0,
                    fire_t: 0.7,
                    fired: false,
                    post: 0.0,
                    dmg: 0.0,
                    color: "#9fd3ff",
                });
                for k in 0..n {
                    let d = (k as i32 - gap as i32).abs();
                    if d <= 2 || d >= n as i32 - 2 {
                        continue;
                    }
                    self.ebullets.push(EBullet {
                        x: ox,
                        y: oy,
                        vx: 0.0,
                        vy: 0.0,
                        r: 6.0,
                        dmg: 18.0,
                        life: 7.0,
                        color: "#9fd3ff",
                        tw: 0.0,
                        split: None,
                        orb: true,
                        ox,
                        oy,
                        ang: k as f64 * TAU / n as f64,
                        dang: 0.55 * dir,
                        rad: 240.0,
                        drad: 42.0,
                    });
                }
                self.enemies[i].atk = 4.2;
            }
        }
    }

    // ---------------- enemy bullets / telegraphs ----------------

    fn update_ebullets(&mut self, dt: f64) {
        let mut spawned: Vec<EBullet> = Vec::new();
        let mut hits: Vec<f64> = Vec::new();
        for b in self.ebullets.iter_mut() {
            b.life -= dt;
            if b.orb {
                b.ang += b.dang * dt;
                b.rad = (b.rad - b.drad * dt).max(0.0);
                b.x = b.ox + b.ang.cos() * b.rad;
                b.y = b.oy + b.ang.sin() * b.rad;
                if b.rad <= 12.0 {
                    b.life = 0.0;
                }
            } else {
                if b.tw != 0.0 {
                    let c = (b.tw * dt).cos();
                    let s = (b.tw * dt).sin();
                    let vx = b.vx;
                    b.vx = vx * c - b.vy * s;
                    b.vy = vx * s + b.vy * c;
                }
                b.x += b.vx * dt;
                b.y += b.vy * dt;
            }
            if let Some(s) = b.split.as_mut() {
                *s -= dt;
                if *s <= 0.0 {
                    b.life = 0.0;
                    let dmg = b.dmg;
                    for k in 0..5 {
                        let a = k as f64 * TAU / 5.0 + self.rng.next_f64();
                        spawned.push(EBullet {
                            x: b.x,
                            y: b.y,
                            vx: a.cos() * 130.0,
                            vy: a.sin() * 130.0,
                            r: 4.0,
                            dmg: dmg * 0.7,
                            life: 2.5,
                            color: "#b8ff6a",
                            tw: 0.0,
                            split: None,
                            orb: false,
                            ox: 0.0,
                            oy: 0.0,
                            ang: 0.0,
                            dang: 0.0,
                            rad: 0.0,
                            drad: 0.0,
                        });
                    }
                }
            }
            if b.life > 0.0
                && (b.x - self.p.x).powi(2) + (b.y - self.p.y).powi(2) < (b.r + 7.0).powi(2)
            {
                hits.push(b.dmg);
                b.life = 0.0;
            }
        }
        for dmg in hits {
            self.hurt_player(dmg);
        }
        self.ebullets.extend(spawned);
        self.ebullets.retain(|b| b.life > 0.0);
    }

    fn update_tele(&mut self, dt: f64) {
        let mut hits: Vec<f64> = Vec::new();
        for q in self.tele.iter_mut() {
            q.t += dt;
            if !q.fired && q.t >= q.fire_t {
                q.fired = true;
                q.post = 0.25;
                let hit = if q.kind == TeleKind::Circle {
                    (q.x - self.p.x).powi(2) + (q.y - self.p.y).powi(2) < (q.r + 6.0).powi(2)
                } else {
                    seg_d2(self.p.x, self.p.y, q.x, q.y, q.x2, q.y2) < (q.w / 2.0 + 6.0).powi(2)
                };
                if hit && q.dmg > 0.0 {
                    hits.push(q.dmg);
                }
            }
            if q.fired {
                q.post -= dt;
            }
        }
        for dmg in hits {
            self.hurt_player(dmg);
        }
        self.tele.retain(|q| !q.fired || q.post > 0.0);
    }

    // ---------------- gems / leveling ----------------

    fn update_gems(&mut self, dt: f64) {
        let mg = self.stats.mag;
        let mut chests = 0u32;
        for i in 0..self.gems.len() {
            if self.gems[i].got {
                continue;
            }
            let dx = self.gems[i].x - self.p.x;
            let dy = self.gems[i].y - self.p.y;
            let dd = dx * dx + dy * dy;
            if self.gems[i].vac || dd < mg * mg {
                let l = dd.sqrt().max(1e-6);
                let s = if self.gems[i].vac { 520.0 } else { 300.0 };
                self.gems[i].x += (self.p.x - self.gems[i].x) / l * s * dt;
                self.gems[i].y += (self.p.y - self.gems[i].y) / l * s * dt;
                if dd < 14.0 * 14.0 {
                    self.gems[i].got = true;
                    let kind = self.gems[i].kind;
                    let value = self.gems[i].value;
                    match kind {
                        GemKind::Gold => self.gold += value,
                        GemKind::Heart => {
                            self.p.hp = (self.p.hp + value).min(self.p.mhp);
                        }
                        GemKind::Chest => chests += 1,
                        GemKind::Magnet => {
                            for g in self.gems.iter_mut() {
                                if g.kind != GemKind::Chest {
                                    g.vac = true;
                                }
                            }
                        }
                        GemKind::Xp | GemKind::Mid | GemKind::Big => {
                            self.xp += value * self.stats.xpm;
                            while self.xp >= xp_need(self.lvl) {
                                self.xp -= xp_need(self.lvl);
                                self.lvl += 1;
                                self.queue += 1;
                            }
                        }
                    }
                }
            }
        }
        self.gems.retain(|g| !g.got);

        for _ in 0..chests {
            self.chest();
        }

        if self.gems.len() > 450 {
            let old: Vec<Gem> = self.gems.drain(0..100).collect();
            let s: f64 = old
                .iter()
                .filter(|g| matches!(g.kind, GemKind::Xp | GemKind::Mid))
                .map(|g| g.value)
                .sum();
            if s > 0.0 {
                let x = self.p.x + self.rng.range(-150.0, 150.0);
                let y = self.p.y + self.rng.range(-150.0, 150.0);
                self.gems.push(Gem { x, y, value: s, kind: GemKind::Big, vac: false, got: false });
            }
        }
        if self.rng.chance(dt / 40.0) {
            let x = self.p.x + self.rng.range(-180.0, 180.0);
            let y = self.p.y + self.rng.range(-260.0, 260.0);
            self.gems.push(Gem {
                x,
                y,
                value: 0.0,
                kind: GemKind::Magnet,
                vac: false,
                got: false,
            });
        }

        if self.queue > 0 && self.choice.is_none() && self.over.is_none() {
            // The caller (step) opens queued choices after the step, so both
            // gem pickups and chests funnel through the same path.
        }
    }

    fn chest(&mut self) {
        let n = if self.rng.chance(0.2) {
            3
        } else if self.rng.chance(0.4) {
            2
        } else {
            1
        };
        self.gold += (20.0 * self.stats.greed).round();
        self.queue += n - 1;
        self.open_choice("chest");
    }
}

pub fn seg_d2(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let vx = bx - ax;
    let vy = by - ay;
    let l = vx * vx + vy * vy;
    if l <= 1e-12 {
        return (px - ax).powi(2) + (py - ay).powi(2);
    }
    let t = (((px - ax) * vx + (py - ay) * vy) / l).clamp(0.0, 1.0);
    let qx = ax + vx * t - px;
    let qy = ay + vy * t - py;
    qx * qx + qy * qy
}
