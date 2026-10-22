# MODs (jakedowns/crown-survivors-mods)

This fork adds a lightweight **modding / plugins layer** to Crown Survivors Classic. Mods are self-contained gameplay tweaks and additions that can be toggled from a new **"Mods / Plugins"** option in the GUI — no engine changes needed to ship a new mod. Several mods ship **enabled by default** on this fork (flagged below).

## What this fork is adding

- **Mod/plugin manager in the GUI** — a single place to enable/disable mods.
- **Proportional speed control** *(mod, on by default)* — touch-drag distance from touch start drives player movement speed (0–100%), linear by default with a common responsiveness curve option.
- **Dash power-up** — a new GUI button + keyboard shortcut (e.g. `Shift`) to dash in the current movement direction; levels up via mod-enabled cards in the power-up pool, with an optional stylish FOV/zoom-out effect while dashing.
- **Wall building ability** *(mod, on by default on this fork)* — draw short-lived block walls; leveling increases max wall count and wall duration/durability, with enemy-interaction rules (fliers over short walls, laser walls block ground + air, phasing enemies, boss interactions).
- **Revive drama** *(mod, on by default on this fork)* — a cinematic pause when the player's single revive is used, to make that moment feel special and remind them it's gone.

## Tasklist

- [ ] Add "mods/plugins" option in GUI
- [ ] Add a new mod, enabled by default: **"Enable proportional speed control"** — player movement speed is proportional to distance touch-drag from touch start center; linear lerp between 0 and 100% (or apply common default curve for max responsiveness balanced with fine-grain control)
- [ ] New power up: **Dash!** — adds a button to the GUI and uses a corresponding shortcut key (like `Shift`) for keyboard input, to allow the player to dash in their current movement direction. The power up levels in strength via mod-enabled cards in the power up pool during player leveling moments (maybe locked behind evolution level, or tied to evolution level in terms of max speed / dash distance). *Bonus: make the camera zoom out stylishly when dashing, increasing FOV temporarily.*
- [ ] New mod, enabled by default on my jakedowns/crown-survivors-mods fork: **Wall building ability** — starts with "you can draw one block wall (square) that fades out quickly." Leveling system increases max wall building ability (max total wall count) and their duration (permanency/lifetime/HP/durability). Design notes:
  - Flying creatures can go above short walls; evolutionarily advanced walls (like lasers) block flying enemy types and ground enemies
  - Perhaps some enemies are ALWAYS impervious to walls / can phase shift / quantum tunnel past lasers
  - Bosses should probably crush walls / take reduced damage from lasers, since they can absorb more than smaller average majority low-level enemies
- [ ] New mod, enabled by default on my fork: **Revive drama** — add a cinematic animation when the player uses a REVIVE. Since there is only one revive, it should feel special and important, and pause the action for a moment to remind the player their one revive is now gone.

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
