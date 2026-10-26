# Crown Survivors Classic — STATS Reference

> **Source of truth**: `public/game.js` (and mirrored in `public/index.html`, `public/game-core.js`)  
> **Version**: 0.1.2  
> **Purpose**: Quick reference for players, modders, and contributors. All values are from game constants.  
> **Keep in sync**: When editing game constants, update this file.

---

## Table of Contents
1. [Characters](#characters)
2. [Weapons (Base & Evolved)](#weapons-base--evolved)
3. [Passives (Level 1–5)](#passives-level-15)
4. [Endless Upgrades (Post-Max)](#endless-upgrades-post-max)
5. [Meta Upgrades (Shop)](#meta-upgrades-shop)
6. [Dash Power-Up](#dash-power-up)
7. [Wall Building](#wall-building)
8. [Enemy Types](#enemy-types)
9. [Bosses](#bosses)
10. [Enemy Scaling Formulas](#enemy-scaling-formulas)
11. [Player Stat Formulas](#player-stat-formulas)
12. [XP / Level Curve](#xp--level-curve)
13. [Curse System](#curse-system)
14. [Revive](#revive)
15. [Gold / Economy](#gold--economy)
16. [Weapon Stats by Level](#weapon-stats-by-level)

---

## Characters

| ID | Name | Base HP | Speed Mult | Start Weapon | Unlock Cost | Perk |
|---|---|---|---|---|---|---|
| 0 | **King Cube** | 100 | 1.00 | Scepter Bolt | 0 | +10% XP |
| 1 | **Ruby Knight** | 140 | 0.90 | Royal Aura | 500 | +1 Armor |
| 2 | **Mint Mage** | 80 | 1.00 | Thunder Block | 1600 | +20% Area |
| 3 | **Shadow Rogue** | 90 | 1.20 | Crown Boomerang | 1000 | +15% Crit |

- **Max HP** = `baseHP + M.hp * 10` (M = meta upgrades)
- **Base move speed** = `110 * char.spd` (modified by boots, meta, endless)

---

## Weapons (Base & Evolved)

| Key | Name | Evolution | Evolves With | Base Color |
|---|---|---|---|---|
| `bolt` | Scepter Bolt | Royal Barrage | Tome | #ffd23f |
| `orb` | Orbit Coins | Golden Halo | Magnet | #e0a526 |
| `rain` | Gold Rain | Midas Flood | Area | #ffb13d |
| `aura` | Royal Aura | Throne Aura | Armor | #ff6b5a |
| `boom` | Crown Boomerang | Crown Storm | Boots | #c9a0ff |
| `zap` | Thunder Block | Storm Block | Luck | #9fd3ff |

**Evolution requirement**: Weapon at level 5 + corresponding passive owned.

---

## Passives (Level 1–5)

| Key | Name | Per Level Effect | Max Level | Color |
|---|---|---|---|---|
| `might` | Might | +12% damage | 5 | #c8452f |
| `tome` | Tome | -8% cooldowns | 5 | #6a5acd |
| `magnet` | Magnet | +35% pickup range | 5 | #5fae7a |
| `area` | Area | +12% area | 5 | #ffb13d |
| `armor` | Armor | -1 damage taken | 5 | #8a8a9a |
| `boots` | Boots | +8% move speed | 5 | #9c6a12 |
| `luck` | Luck | +5% crit, +gold | 5 | #2f9e6b |
| `heart` | Heart | +20 max HP + heal | 5 | #e8473a |

---

## Endless Upgrades (Post-Max)

Stack forever. Past soft cap, each stack = half value and shows up less often.

| Key | Name | Value/Stack | Soft Cap | Linked Passive |
|---|---|---|---|---|
| `dmg` | Royal Edge | +6% damage | 15 | Might |
| `cd` | Swift Decree | -4% cooldowns | 8 | Tome |
| `hp` | Crown Vigor | +12 max HP & heal | 12 | Heart |
| `area` | Wide Reign | +5% area | 10 | Area |
| `crit` | Lucky Star | +3% crit | 10 | Luck |
| `armor` | Iron Mantle | +1 armor | 4 | Armor |
| `spd` | Gale Step | +4% move speed | 6 | Boots |
| `mag` | Royal Pull | +15% pickup range | 5 | Magnet |

**Effect formula** (for `n` stacks):  
`value * (min(n, cap) + max(0, n - cap) * 0.5)`

---

## Meta Upgrades (Shop)

| Key | Name | Effect | Cost | Max |
|---|---|---|---|---|
| `might` | Might | +5% damage | 90 | 10 |
| `hp` | Vitality | +10 max HP | 75 | 10 |
| `armor` | Plating | +1 armor | 180 | 3 |
| `speed` | Swiftness | +4% speed | 90 | 5 |
| `magnet` | Attraction | +15% pickup | 60 | 5 |
| `greed` | Greed | +15% gold | 120 | 5 |
| `reroll` | Reroll | +1 reroll/run | 150 | 3 |
| `choice` | Insight | +1 card choice | 600 | 1 |
| `revive` | Revive | Revive once/run | 750 | 1 |

---

## Dash Power-Up

**Unlock**: Dash mod enabled + appears in level-up pool at level 1.

| Level | Distance | Speed | Cooldown | Duration | Description |
|---|---|---|---|---|---|
| 1 | 660 | 600 | 3.55s | 0.145s | +1 dash! Ready to use now. |
| 2 | 720 | 600 | 3.10s | 0.160s | Longer dash distance |
| 3 | 780 | 600 | 2.65s | 0.175s | Faster dash |
| 4 | 840 | 600 | 2.20s | 0.190s | Shorter cooldown |
| 5 | 900 | 600 | 1.75s | 0.205s | MAX speed blast |

- **Distance** = `600 + 60 * lv` (pixels/sec × duration)
- **Cooldown** = `max(1.2, 4 - 0.45 * lv)` seconds
- **Invulnerability** = dash duration + 0.1s
- **FOV zoom-out** while dashing
- **Input**: `»` HUD button or **Shift (tap <200ms)** — see [Unified Shift Control](#unified-shift-control-v012)

---

## Wall Building

**Unlock**: Walls mod enabled (on by default). Starts at level 1.

### Wall Cap (Max Active Walls)
**v0.1.2 buff: 2× at all levels**

| Wall Level | Base Cap | +Evo Bonus | **Final Cap (×2)** |
|---|---|---|---|
| 1 | 3 | — | **6** |
| 2 | 4 | — | **8** |
| 3 | 5 | — | **10** |
| 4 | 6 | — | **12** |
| 5 | 7 | +2 | **18** |

Formula: `([3,4,5,6,7][lv-1] + (evo?2:0)) * 2`

### Wall Durability (HP)
**v0.1.2 buff: 2× at all levels**

| Wall Level | Regular Wall HP | Radiant Wall (Evo) HP |
|---|---|---|
| 1 | **56** | 440 |
| 2 | **80** | 440 |
| 3 | **104** | 440 |
| 4 | **128** | 440 |
| 5 | **152** | 440 |

Formula: `Regular: (16 + 12 * lv) * 2` | `Radiant: 440` (was 220)

### Wall Lifetime
| Type | Base | Per Level |
|---|---|---|
| Regular | 2.6s | +0.7s |
| Radiant | 7.5s | (fixed) |

### Wall Thickness
| Type | Base | Per Level |
|---|---|---|
| Regular | 10 | +1.0 |
| Radiant | 14 | +1.0 |

### Wall Laser Damage (Radiant Only)
- **Tick rate**: Every 0.4s
- **Damage vs normal**: `(26 + 8 * wallLv) * 0.4` per tick
- **Damage vs boss**: `6 * 0.4 = 2.4` per tick (heavily reduced)
- **Ignores**: Ghosts, Wisps (phasers)

### Enemy Interaction Rules
| Enemy Type | vs Regular Walls | vs Radiant Walls |
|---|---|---|
| Ground (blob, skel, imp, vine, frog) | Blocked, slide/chew | Burned (laser) |
| Fliers (bat, harpy) | **Fly over** | Blocked + burned |
| Phasers (ghost, wisp) | **Phase through** | **Phase through** |
| Bosses | **Crush** (instant destroy) | Take reduced laser dmg |

### Unified Shift Control (v0.1.2)
| Input | Action |
|---|---|
| **Shift tap (<200ms)** | Dash (if dash mod on) |
| **Shift hold (≥200ms)** | Toggle wall draw (if walls mod on) |
| **E key** | Toggle wall draw (legacy) |
| **▬ HUD button** | Toggle wall draw |

---

## Enemy Types

| Type | Base HP | Speed | Radius | Damage | XP | Color | Notes |
|---|---|---|---|---|---|---|---|
| **blob** | 10 | 34 | 9 | 6 | 1 | #7d4a2a | Basic melee |
| **bat** | 6 | 62 | 7 | 4 | 1 | #3a2f55 | **Flier** — flies over regular walls |
| **skel** | 34 | 30 | 10 | 9 | 3 | #e8e0cc | Tanky melee |
| **ghost** | 20 | 50 | 9 | 7 | 2 | #cfe3ff | **Phaser** — phases through all walls |
| **imp** | 26 | 36 | 9 | 8 | 3 | #c8452f | Shoots projectiles |
| **vine** | 40 | 40 | 9 | 10 | 3 | #4f9a3a | Phase 3 |
| **frog** | 55 | 20 | 10 | 12 | 4 | #7fc04a | Phase 3, slow tank |
| **wisp** | 38 | 70 | 8 | 10 | 4 | #e8f4ff | **Phaser** + fast, Phase 4 |
| **harpy** | 70 | 44 | 11 | 12 | 5 | #8fb8e8 | **Flier**, Phase 4 |

### Elite Multipliers
- **HP**: ×8
- **Radius**: ×1.8
- **Speed**: ×0.85
- **Damage**: ×1.5
- **XP**: ×15
- **Gold drop**: Higher chance

---

## Bosses

| # | Name | Time | HP | Radius | Speed | Damage | Color |
|---|---|---|---|---|---|---|---|
| 0 | **BLOB KING** | 120s | 700 | 34 | 38 | 18 | #7d4a2a |
| 1 | **BAT QUEEN** | 270s | 2,600 | 30 | 70 | 24 | #5a4a8a |
| 2 | **LAVA KING** | 420s | 5,200 | 40 | 46 | 30 | #a8321e |
| 3 | **THORN MOTHER** | 600s | 11,000 | 44 | 34 | 36 | #3f8a3a |
| 4 | **STORM SOVEREIGN** | 790s | 11,500 | 38 | 60 | 42 | #cfe6ff |

**Boss HP multiplier**: `1 + 0.4 * curse`  
**Boss damage**: `18 + bossIndex * 6` (base, then scaled by time)  
**Boss XP**: 60  
**Boss kill gold**: ~`5 * greed` × 20 gems + chest

### Boss Special Abilities
- **Blob King**: Splits into 4 projectiles on attack
- **Bat Queen**: Spawns bats, dash attacks
- **Lava King**: Fire trails, area denial
- **Thorn Mother**: Thorn walls, vine spawns
- **Storm Sovereign**: Lightning, harpy spawns

**Rewards**:
- Boss 2 (Lava King): +2 weapon/passive slots (5 each)
- Boss 4 (Storm Sovereign): Victory

---

## Enemy Scaling Formulas

### Health Multiplier (HM)
```
HM = (1 + time/95 + (time/210)²) * (1 + 0.35 * curse)
```
- Applied to **all regular enemies** on spawn
- Elites: ×8 on top

### Speed Scaling
```
Speed = baseSpeed * (1 + time/900) * (1 + 0.08 * curse) * (elite ? 0.85 : 1)
```

### Damage Scaling
```
Damage = baseDamage * (1 + time/400) * (elite ? 1.5 : 1)
```

### Spawn Rate
```
Rate = min(12, 0.9 + time/28 + (time/150)²) * (boss? 0.55 : 1) * (1 + 0.2 * curse)
```
- Max 12/sec (during boss: 6.6/sec)
- Max 320 enemies alive

### Phase Enemy Pools
| Phase | Time | Pool |
|---|---|---|
| 0 | 0–120s | blob ×2 |
| 1 | 25s+ | + bat ×2 |
| 2 | 80s+ | + skel |
| 3 | 140s+ | + ghost, bat |
| 4 | 200s+ | + imp |
| 5 | 300s+ | + skel, imp |
| **Phase 3** | 420s+ | vine×2, frog, skel, bat, imp |
| **Phase 4** | 600s+ | wisp×2, harpy, ghost, bat, frog |

---

## Player Stat Formulas

Computed in `st()` each frame. `ps` = passive levels, `M` = meta levels, `infV` = endless values.

| Stat | Formula |
|---|---|
| **Damage** | `(1 + 0.12*might + 0.05*M.might) * (1 + infV('dmg'))` |
| **Cooldown** | `0.92^tome * max(0.5, 1 - infV('cd'))` |
| **Magnet** | `72 * (1 + 0.35*magnet + 0.15*M.magnet + infV('mag'))` |
| **Area** | `(1 + 0.12*area + infV('area')) * (char==MintMage ? 1.2 : 1)` |
| **Armor** | `armor + M.armor + (char==RubyKnight?1:0) + floor(infV('armor'))` |
| **Speed** | `110 * char.spd * (1 + 0.08*boots + 0.04*M.speed + infV('spd'))` |
| **Crit** | `min(0.75, 0.05*luck + (char==ShadowRogue?0.15:0) + 0.03 + infV('crit'))` |
| **Greed** | `1 + 0.15*M.greed + 0.1*luck + 0.25*curse` |
| **XP Mult** | `char==KingCube ? 1.1 : 1` |

---

## XP / Level Curve

```js
need(lvl) = round(4 + lvl * 3.2 + max(0, lvl-12)*3 + max(0, lvl-25)*6)
```

| Level | XP Needed | Cumulative |
|---|---|---|
| 1 | 7 | 7 |
| 2 | 10 | 17 |
| 3 | 14 | 31 |
| 4 | 17 | 48 |
| 5 | 20 | 68 |
| 6 | 23 | 91 |
| 7 | 27 | 118 |
| 8 | 30 | 148 |
| 9 | 33 | 181 |
| 10 | 36 | 217 |
| 11 | 40 | 257 |
| 12 | 43 | 300 |
| 13 | 49 | 349 |
| 14 | 52 | 401 |
| 15 | 55 | 456 |
| 20 | 73 | ~800 |
| 25 | 94 | ~1300 |
| 30 | 127 | ~2000 |

- **XP per gem**: gem value × `SC.xpm` (1.1 for King Cube, else 1.0)
- **Cards per level**: `3 + M.choice` (base 3, +1 with Insight)

---

## Curse System

| Curse | Unlock Requirement | Effect |
|---|---|---|
| 0 | Default | Baseline |
| 1 | Win at curse 0 | +35% enemy HP, +8% speed, +20% spawn rate, +50% win gold |
| 2 | Win at curse 1 | Stacking |
| 3 | Win at curse 2 | Stacking |
| 4 | Win at curse 3 | Stacking |
| 5 | Win at curse 4 | Max |

**Curse effects** (applied per level):
- Enemy HP: `× (1 + 0.35 * curse)`
- Enemy Speed: `× (1 + 0.08 * curse)`
- Spawn Rate: `× (1 + 0.2 * curse)`
- Boss HP: `× (1 + 0.4 * curse)`
- Win Gold: `× (1 + 0.5 * curse)`
- Player Greed: `+0.25 * curse`

---

## Revive

- **Source**: Meta upgrade "Revive" (750 gold, max 1)
- **Effect**: On death with revive available:
  - HP → 50% max
  - Invulnerability: **2.4s**
  - Kills all non-boss enemies within 160px
  - **Revive Drama mod** (on by default): 2.4s cinematic freeze with dimming overlay, expanding ring, particle burst, dramatic chord
  - Without mod: Simple "REVIVED!" banner

---

## Gold / Economy

### Gold Sources
| Source | Amount |
|---|---|
| Kill (base) | 1 (scaled by Greed) |
| Elite kill | ~8 × Greed |
| Gold Rain pools | 1046Hz + 1568Hz tone |
| Boss kill | ~100 × Greed (20 gems × 5) |
| Chest | 20 × Greed |
| Coin Sack (level-up) | +25 flat |
| End of run | `gold + kills/30 + (win? 220*(1+curse*0.5):0) + (bb≥3&&!win?60:0)` |

### Shop Prices (Meta)
See [Meta Upgrades](#meta-upgrades-shop) table.

---

## Weapon Stats by Level

### Scepter Bolt (bolt)
| Lv | Projectiles | Damage | Cooldown | Pierce | Evolution |
|---|---|---|---|---|---|
| 1 | 1 | 14 | 0.80s | 1 | — |
| 2 | 1 | 18 | 0.75s | 1 | — |
| 3 | 2 | 22 | 0.70s | 1 | — |
| 4 | 2 | 26 | 0.60s | 1 | — |
| 5 | 3 | 30 | 0.55s | 2 | — |
| **Evo** | 5 | 24 | 0.40s | 4 | Royal Barrage |

### Orbit Coins (orb)
| Lv | Coins | Damage | Radius | Evolution |
|---|---|---|---|---|
| 1 | 1 | 9 | 48 | — |
| 2 | 2 | 12 | 48 | — |
| 3 | 2 | 15 | 48 | — |
| 4 | 3 | 18 | 48 | — |
| 5 | 5 | 21 | 48 | — |
| **Evo** | 8 | 20 | 75 | Golden Halo |

### Gold Rain (rain)
| Lv | Pools | Damage | Radius | Cooldown | Evolution |
|---|---|---|---|---|---|
| 1 | 1 | 7 | 29 | 3.0s | — |
| 2 | 1 | 9 | 32 | 3.0s | — |
| 3 | 2 | 11 | 35 | 3.0s | — |
| 4 | 2 | 13 | 38 | 3.0s | — |
| 5 | 3 | 15 | 41 | 3.0s | — |
| **Evo** | 5 | 16 | 46 | 2.0s | Midas Flood (+slow) |

### Royal Aura (aura)
| Lv | Radius | Damage | Cooldown | Evolution |
|---|---|---|---|---|
| 1 | 38 | 4 | 0.5s | — |
| 2 | 44 | 5 | 0.5s | — |
| 3 | 52 | 7 | 0.5s | — |
| 4 | 58 | 8 | 0.5s | — |
| 5 | 68 | 10 | 0.5s | +50 knockback |
| **Evo** | 95 | 16 | 0.5s | +heal 0.6/tick |

### Crown Boomerang (boom)
| Lv | Crowns | Damage | Duration | Cooldown | Evolution |
|---|---|---|---|---|---|
| 1 | 1 | 14 | 1.6s | 1.6s | — |
| 2 | 1 | 18 | 1.6s | 1.6s | — |
| 3 | 2 | 22 | 1.6s | 1.6s | — |
| 4 | 2 | 26 | 1.6s | 1.6s | — |
| 5 | 3 | 30 | 1.6s | 1.6s | +range |
| **Evo** | 6 | 26 | 2.2s | 1.1s | Crown Storm |

### Thunder Block (zap)
| Lv | Strikes | Damage | Cooldown | Evolution |
|---|---|---|---|---|
| 1 | 2 | 19 | 2.2s | — |
| 2 | 3 | 24 | 2.2s | — |
| 3 | 3 | 29 | 2.2s | — |
| 4 | 4 | 34 | 2.2s | — |
| 5 | 6 | 39 | 2.2s | — |
| **Evo** | 10 | 40 | 1.2s | Storm Block (chain 0.5×) |

---

## Quick Reference: Key Constants

```js
// Player base
BASE_SPEED = 110
BASE_MAGNET = 72
XP_FORMULA = l => round(4 + l*3.2 + max(0,l-12)*3 + max(0,l-25)*6)

// Enemy scaling
HP_MULT = (1 + t/95 + (t/210)^2) * (1 + 0.35*curse)
SPD_MULT = (1 + t/900) * (1 + 0.08*curse)
DMG_MULT = (1 + t/400)
SPAWN_RATE = min(12, 0.9 + t/28 + (t/150)^2) * (boss?0.55:1) * (1+0.2*curse)

// Wall (v0.1.2)
WALL_CAP = ([3,4,5,6,7][lv-1] + (evo?2:0)) * 2
WALL_HP_REG = (16 + 12*lv) * 2
WALL_HP_RAD = 440
WALL_LIFE_REG = 2.6 + lv*0.7
WALL_LIFE_RAD = 7.5

// Dash
DASH_SPEED = 600 + 60*lv
DASH_TIME = 0.13 + 0.015*lv
DASH_CD = max(1.2, 4 - 0.45*lv)

// Shift tap threshold
SHIFT_TAP_MS = 200
```

---

## Version History

| Version | Changes |
|---|---|
| 0.1.2 | **New unified Shift control**: Tap=dash, Hold=wall draw. **2× wall cap & HP** at all levels. Accessibility: arrow keys + Enter/Shift for all menus. |
| 0.1.1 | Bugfixes: dash timer, wall button overlap, shop syntax, disableTracking |
| 0.1.0 | Initial release with mods: speed, dash, walls, revive drama, numKeys |

---

## Mod Flags (S.mods)

| Flag | Default | Description |
|---|---|---|
| `speed` | 1 | Proportional touch-drag speed |
| `speedCurve` | 0 | 0=linear, 1=responsive (sqrt) |
| `dash` | 1 | Dash power-up enabled |
| `walls` | 1 | Wall building enabled |
| `revive` | 1 | Revive drama cinematic |
| `leftHand` | 0 | Left-handed UI |
| `numKeys` | 1 | 1-4 keys pick level-up cards |
| `timeFreeze` | 1 | Pause on focus loss |
| `areaClear` | 1 | Clear on level up |
| `qa` | 0 | QA menu |
| `disableTracking` | 1 | Disable analytics |

---

*Generated from game constants. Edit `public/game.js` (and mirrors) then update this file.*