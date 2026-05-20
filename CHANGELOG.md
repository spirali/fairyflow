# Changelog

## Unrealeased

### New

  * `follow_path` now supports option `backwards`

### Fixed

  * Fixed animated colors with null values
  * Fixed double render of arrows

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
