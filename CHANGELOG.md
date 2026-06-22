# Changelog

## Unrealeased

### New

  * Improvements in cancelling rendering
  * Shows rendering progress
  * `follow_path` now optionaly takes `start` and `end` that allows to go backwards
  * Updated colors in timeline
  * UI refactoring
  * `reserve` parameter for `.column()` and `.row()` — when `True` (default), inactive children (not yet visible or already removed) still occupy their full size in the layout so that siblings never shift when items appear or disappear; set to `False` to have the group tightly fit only its currently visible children
  

### Fixed

  * Fixed animated colors with null values
  * Fixed double render of arrows
  * Fixed stext and mixed styles and multilines

## 0.3.0

### New

  * Paralel / Sequential operators for animations
  * PgUp/PgDown in player

## 0.2.0

### New

  * `[font-aliases]` table in `fairyflow.toml` — map CSS generic family names (`sans-serif`, `monospace`, …) to specific fonts for reproducible cross-machine rendering
  * `reveal_down`, `reveal_up`, `hide_down`, `hide_up` added to `Group`
  * `stext` tag attributes: `color`, `font-size` / `text-size`, `font`, `font-weight`, `bold`, `italic` are now parsed and applied directly from markup (e.g. `<green>INFO</green>`, `<span color='red' bold>ERROR</span>`)
  * Player prerenders frames in paralel
  * New shortcuts in player: "Home" / "End" 

### Fixes

  * `.color` and `.stroke_color` can now take `None`
  * `stext`: a `<` with no matching `>` is now treated as literal text instead of raising `ValueError`, allowing arbitrary content such as ASCII art and file paths

## 0.1.0

  * Initial release
