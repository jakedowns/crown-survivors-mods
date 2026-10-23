# MODs (jakedowns/crown-survivors-mods)

This fork adds a lightweight **modding / plugins layer** to Crown Survivors Classic. Mods are self-contained gameplay tweaks and additions that can be toggled from a new **"Mods / Plugins"** option in the GUI — no engine changes needed to ship a new mod. Several mods ship **enabled by default** on this fork (flagged below).

> Status: all core mods implemented (`public/index.html`). Toggles persist in the save; dash & wall levels apply from your next run.

## What this fork is adding

- ✅ **Mod/plugin manager in the GUI** — a single place to enable/disable mods *(title screen + pause menu → MODS)*.
- ✅ **Proportional speed control** *(mod, on by default)* — touch-drag distance from touch start drives player movement speed (0–100%), linear by default with a common responsiveness curve option.
- ✅ **Dash power-up** — a new GUI button + keyboard shortcut (`Shift`) to dash in the current movement direction; levels up via mod-enabled cards in the power-up pool, with a stylish FOV/zoom-out effect while dashing.
- ✅ **Wall building ability** *(mod, on by default on this fork)* — draw short-lived block walls; leveling increases max wall count and wall duration/durability, with enemy-interaction rules (fliers over short walls, laser walls block ground + air, ghost/wisp phase, bosses crush).
- ✅ **Revive drama** *(mod, on by default on this fork)* — a cinematic pause when the player's single revive is used, to make that moment feel special and remind them it's gone.

## Implementation notes

- Mods live in the save (`S.mods`): `speed` (w/ `speedCurve`: linear|responsive), `dash`, `walls`, `revive`. Defaults: speed on, walls on, revive drama on, dash off.
- **Dash**: `»` HUD button or `Shift`; dash along last move direction, brief invuln + cooldown; upgrades = distance/speed/cooldown. Camera zooms out (FOV) while dashing.
- **Walls**: `▬` HUD button or `E`; wall spawns in front of you, perpendicular to movement. Ground enemies are blocked and slide along/chew walls; fliers (bats, harpies) cross short walls; ghosts & wisps phase through everything; bosses crush walls. At Lv5 with the Armor passive, walls evolve into **Radiant Walls** (lasers): burn ground AND air, bosses take heavily reduced laser damage.
- **Revive drama**: action freezes ~2.4s with dimming overlay, expanding ring, particle burst and dramatic chord; `p`-style banner fallback when the mod is off.

## Tasklist

- [x] Add "mods/plugins" option in GUI
- [x] Add a new mod, enabled by default: **"Enable proportional speed control"** — player movement speed is proportional to distance touch-drag from touch start center; linear lerp between 0 and 100% (or apply common default curve for max responsiveness balanced with fine-grain control)
- [x] New power up: **Dash!** — adds a button to the GUI and uses a corresponding shortcut key (like `Shift`) for keyboard input, to allow the player to dash in their current movement direction. The power up levels in strength via mod-enabled cards in the power up pool during player leveling moments (maybe locked behind evolution level, or tied to evolution level in terms of max speed / dash distance). *Bonus: make the camera zoom out stylishly when dashing, increasing FOV temporarily.* — done: dash levels 1–5 via mod cards; camera zoom-out included.
- [x] New mod, enabled by default on my jakedowns/crown-survivors-mods fork: **Wall building ability** — starts with "you can draw one block wall (square) that fades out quickly." Leveling system increases max wall building ability (max total wall count) and their duration (permanency/lifetime/HP/durability). Design notes:
  - [x] Flying creatures can go above short walls; evolutionarily advanced walls (like lasers) block flying enemy types and ground enemies — done: Lv5 + Armor passive evolves walls into Radiant Walls (lasers).
  - [x] Perhaps some enemies are ALWAYS impervious to walls / can phase shift / quantum tunnel past lasers — done: ghosts & wisps phase through all walls; bats/harpies fly over brick walls.
  - [x] Bosses should probably crush walls / take reduced damage from lasers, since they can absorb more than smaller average majority low-level enemies — done.
- [x] New mod, enabled by default on my fork: **Revive drama** — add a cinematic animation when the player uses a REVIVE. Since there is only one revive, it should feel special and important, and pause the action for a moment to remind the player their one revive is now gone.

## Bonus: related mod ideas (grill-worthy list)

Brainstorm of other mods that fit this fork's modding layer:

- [ ] **Auto-pickup radius mod** — tune XP/gem magnet range (a QoL knob, off-by-default for purists)
- [ ] **Pause-on-touch mod** — auto-pause when the game loses focus / mouse leaves the window
- [ ] **Difficulty sliders mod** — enemy spawn rate, HP scaling, XP curve knobs exposed in the GUI
- [ ] **Ghost replay / best-run overlay mod** — translucent "ghost" of your best run for chase-the-record sessions
- [ ] **Screenshot / clip hotkey mod** — one-key capture of gameplay moments, wired into the leaderboard flow
- [ ] **Retro CRT / filter mod** — shader-style visual filters (CRT, scanlines, palette swaps)
- [ ] **Speedrun timer mod** — splits and timers for the `fastest` leaderboard crowd
- [ ] **Colorblind / accessibility mod** — enemy palette remapping and hit-flash intensity options
  - [x] New mod, enabled by default: **Num key card select** — 1,2,3,4 keys instantly pick a level-up card during level up instead of mouse clicking; wraps feature in mod flag in the mod menu, enabled by default
