//! Static game/balance data.
//!
//! This module is the single source of truth for content values. The browser
//! build exports this table (see `sim_meta`) so menus, the profile editor and
//! the simulator no longer maintain their own copies.

pub const WEAPON_COUNT: usize = 6;
pub const PASSIVE_COUNT: usize = 8;
pub const INF_COUNT: usize = 8;
pub const CHARACTER_COUNT: usize = 4;
pub const ENEMY_COUNT: usize = 9;
pub const BOSS_COUNT: usize = 5;

pub const W_BOLT: usize = 0;
pub const W_ORB: usize = 1;
pub const W_RAIN: usize = 2;
pub const W_AURA: usize = 3;
pub const W_BOOM: usize = 4;
pub const W_ZAP: usize = 5;

pub const P_MIGHT: usize = 0;
pub const P_TOME: usize = 1;
pub const P_MAGNET: usize = 2;
pub const P_AREA: usize = 3;
pub const P_ARMOR: usize = 4;
pub const P_BOOTS: usize = 5;
pub const P_LUCK: usize = 6;
pub const P_HEART: usize = 7;

pub const I_DMG: usize = 0;
pub const I_CD: usize = 1;
pub const I_HP: usize = 2;
pub const I_AREA: usize = 3;
pub const I_CRIT: usize = 4;
pub const I_ARMOR: usize = 5;
pub const I_SPD: usize = 6;
pub const I_MAG: usize = 7;

pub const WEAPON_KEYS: [&str; WEAPON_COUNT] = ["bolt", "orb", "rain", "aura", "boom", "zap"];
pub const PASSIVE_KEYS: [&str; PASSIVE_COUNT] =
    ["might", "tome", "magnet", "area", "armor", "boots", "luck", "heart"];
pub const INF_KEYS: [&str; INF_COUNT] = ["dmg", "cd", "hp", "area", "crit", "armor", "spd", "mag"];

#[derive(Clone, Copy)]
pub struct WeaponDef {
    pub key: &'static str,
    pub name: &'static str,
    pub evo_name: &'static str,
    /// Passive required for the evolution (index into PASSIVES).
    pub evo_passive: usize,
    pub desc: &'static str,
    pub color: &'static str,
}

pub const WEAPONS: [WeaponDef; WEAPON_COUNT] = [
    WeaponDef {
        key: "bolt",
        name: "Scepter Bolt",
        evo_name: "Royal Barrage",
        evo_passive: P_TOME,
        desc: "Shoots nearest foe",
        color: "#ffd23f",
    },
    WeaponDef {
        key: "orb",
        name: "Orbit Coins",
        evo_name: "Golden Halo",
        evo_passive: P_MAGNET,
        desc: "Coins circle you",
        color: "#e0a526",
    },
    WeaponDef {
        key: "rain",
        name: "Gold Rain",
        evo_name: "Midas Flood",
        evo_passive: P_AREA,
        desc: "Damaging gold pools",
        color: "#ffb13d",
    },
    WeaponDef {
        key: "aura",
        name: "Royal Aura",
        evo_name: "Throne Aura",
        evo_passive: P_ARMOR,
        desc: "Burns nearby foes",
        color: "#ff6b5a",
    },
    WeaponDef {
        key: "boom",
        name: "Crown Boomerang",
        evo_name: "Crown Storm",
        evo_passive: P_BOOTS,
        desc: "Thrown crown returns",
        color: "#c9a0ff",
    },
    WeaponDef {
        key: "zap",
        name: "Thunder Block",
        evo_name: "Storm Block",
        evo_passive: P_LUCK,
        desc: "Lightning strikes foes",
        color: "#9fd3ff",
    },
];

#[derive(Clone, Copy)]
pub struct PassiveDef {
    pub key: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub color: &'static str,
}

pub const PASSIVES: [PassiveDef; PASSIVE_COUNT] = [
    PassiveDef { key: "might", name: "Might", desc: "+12% damage", color: "#c8452f" },
    PassiveDef { key: "tome", name: "Tome", desc: "-8% cooldowns", color: "#6a5acd" },
    PassiveDef { key: "magnet", name: "Magnet", desc: "+35% pickup range", color: "#5fae7a" },
    PassiveDef { key: "area", name: "Area", desc: "+12% area", color: "#ffb13d" },
    PassiveDef { key: "armor", name: "Armor", desc: "-1 damage taken", color: "#8a8a9a" },
    PassiveDef { key: "boots", name: "Boots", desc: "+8% move speed", color: "#9c6a12" },
    PassiveDef { key: "luck", name: "Luck", desc: "+5% crit, +gold", color: "#2f9e6b" },
    PassiveDef { key: "heart", name: "Heart", desc: "+20 max HP, heal", color: "#e8473a" },
];

#[derive(Clone, Copy)]
pub struct InfDef {
    pub key: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub value: f64,
    pub cap: u32,
}

pub const INF: [InfDef; INF_COUNT] = [
    InfDef { key: "dmg", name: "Royal Edge", desc: "+6% damage", value: 0.06, cap: 15 },
    InfDef { key: "cd", name: "Swift Decree", desc: "-4% cooldowns", value: 0.04, cap: 8 },
    InfDef { key: "hp", name: "Crown Vigor", desc: "+12 max HP & heal", value: 12.0, cap: 12 },
    InfDef { key: "area", name: "Wide Reign", desc: "+5% area", value: 0.05, cap: 10 },
    InfDef { key: "crit", name: "Lucky Star", desc: "+3% crit", value: 0.03, cap: 10 },
    InfDef { key: "armor", name: "Iron Mantle", desc: "+1 armor", value: 1.0, cap: 4 },
    InfDef { key: "spd", name: "Gale Step", desc: "+4% move speed", value: 0.04, cap: 6 },
    InfDef { key: "mag", name: "Royal Pull", desc: "+15% pickup range", value: 0.15, cap: 5 },
];

#[derive(Clone, Copy)]
pub struct CharDef {
    pub name: &'static str,
    pub desc: &'static str,
    pub color: &'static str,
    pub start_weapon: usize,
    pub hp: f64,
    pub spd: f64,
    pub perk: &'static str,
}

pub const CHARS: [CharDef; CHARACTER_COUNT] = [
    CharDef {
        name: "King Cube",
        desc: "Balanced royal. Starts with Scepter Bolt.",
        color: "#e0a526",
        start_weapon: W_BOLT,
        hp: 100.0,
        spd: 1.0,
        perk: "+10% XP",
    },
    CharDef {
        name: "Ruby Knight",
        desc: "Tanky brawler. Starts with Royal Aura.",
        color: "#c8452f",
        start_weapon: W_AURA,
        hp: 140.0,
        spd: 0.9,
        perk: "+1 Armor",
    },
    CharDef {
        name: "Mint Mage",
        desc: "Glass cannon. Starts with Thunder Block.",
        color: "#5fae7a",
        start_weapon: W_ZAP,
        hp: 80.0,
        spd: 1.0,
        perk: "+20% Area",
    },
    CharDef {
        name: "Shadow Rogue",
        desc: "Fast & lucky. Starts with Crown Boomerang.",
        color: "#4a3a5a",
        start_weapon: W_BOOM,
        hp: 90.0,
        spd: 1.2,
        perk: "+15% Crit",
    },
];

#[derive(Clone, Copy)]
pub struct BossDef {
    pub time: f64,
    pub name: &'static str,
    pub hp: f64,
    pub r: f64,
    pub spd: f64,
    pub color: &'static str,
}

pub const BOSSES: [BossDef; BOSS_COUNT] = [
    BossDef { time: 120.0, name: "BLOB KING", hp: 700.0, r: 34.0, spd: 38.0, color: "#7d4a2a" },
    BossDef { time: 270.0, name: "BAT QUEEN", hp: 2600.0, r: 30.0, spd: 70.0, color: "#5a4a8a" },
    BossDef { time: 420.0, name: "LAVA KING", hp: 5200.0, r: 40.0, spd: 46.0, color: "#a8321e" },
    BossDef { time: 600.0, name: "THORN MOTHER", hp: 11000.0, r: 44.0, spd: 34.0, color: "#3f8a3a" },
    BossDef { time: 790.0, name: "STORM SOVEREIGN", hp: 11500.0, r: 38.0, spd: 60.0, color: "#cfe6ff" },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EType {
    Blob,
    Bat,
    Skel,
    Ghost,
    Imp,
    Vine,
    Frog,
    Wisp,
    Harpy,
}

impl EType {
    pub fn index(self) -> usize {
        match self {
            EType::Blob => 0,
            EType::Bat => 1,
            EType::Skel => 2,
            EType::Ghost => 3,
            EType::Imp => 4,
            EType::Vine => 5,
            EType::Frog => 6,
            EType::Wisp => 7,
            EType::Harpy => 8,
        }
    }

    pub fn key(self) -> &'static str {
        ENEMY_KEYS[self.index()]
    }

    pub fn def(self) -> &'static EnemyDef {
        &ENEMIES[self.index()]
    }

    pub fn phases_through_walls(self) -> bool {
        matches!(self, EType::Ghost | EType::Wisp)
    }

    pub fn flies(self) -> bool {
        matches!(self, EType::Bat | EType::Harpy)
    }
}

pub const ENEMY_KEYS: [&str; ENEMY_COUNT] =
    ["blob", "bat", "skel", "ghost", "imp", "vine", "frog", "wisp", "harpy"];

#[derive(Clone, Copy)]
pub struct EnemyDef {
    pub key: &'static str,
    pub hp: f64,
    pub spd: f64,
    pub r: f64,
    pub dmg: f64,
    pub xp: f64,
    pub color: &'static str,
}

pub const ENEMIES: [EnemyDef; ENEMY_COUNT] = [
    EnemyDef { key: "blob", hp: 10.0, spd: 34.0, r: 9.0, dmg: 6.0, xp: 1.0, color: "#7d4a2a" },
    EnemyDef { key: "bat", hp: 6.0, spd: 62.0, r: 7.0, dmg: 4.0, xp: 1.0, color: "#3a2f55" },
    EnemyDef { key: "skel", hp: 34.0, spd: 30.0, r: 10.0, dmg: 9.0, xp: 3.0, color: "#e8e0cc" },
    EnemyDef { key: "ghost", hp: 20.0, spd: 50.0, r: 9.0, dmg: 7.0, xp: 2.0, color: "#cfe3ff" },
    EnemyDef { key: "imp", hp: 26.0, spd: 36.0, r: 9.0, dmg: 8.0, xp: 3.0, color: "#c8452f" },
    EnemyDef { key: "vine", hp: 40.0, spd: 40.0, r: 9.0, dmg: 10.0, xp: 3.0, color: "#4f9a3a" },
    EnemyDef { key: "frog", hp: 55.0, spd: 20.0, r: 10.0, dmg: 12.0, xp: 4.0, color: "#7fc04a" },
    EnemyDef { key: "wisp", hp: 38.0, spd: 70.0, r: 8.0, dmg: 10.0, xp: 4.0, color: "#e8f4ff" },
    EnemyDef { key: "harpy", hp: 70.0, spd: 44.0, r: 11.0, dmg: 12.0, xp: 5.0, color: "#8fb8e8" },
];

pub fn weapon_index(key: &str) -> Option<usize> {
    WEAPON_KEYS.iter().position(|k| *k == key)
}

pub fn passive_index(key: &str) -> Option<usize> {
    PASSIVE_KEYS.iter().position(|k| *k == key)
}

pub fn inf_index(key: &str) -> Option<usize> {
    INF_KEYS.iter().position(|k| *k == key)
}

/// XP required to advance from level `lvl`.
pub fn xp_need(lvl: u32) -> f64 {
    let l = lvl as f64;
    (4.0 + l * 3.2 + (l - 12.0).max(0.0) * 3.0 + (l - 25.0).max(0.0) * 6.0).round()
}
