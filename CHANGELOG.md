# Changelog

## Unreleased

### New

  * `reveal_down`, `reveal_up`, `hide_down`, `hide_up` added to `Group`
  * `stext` tag attributes: `color`, `font-size` / `text-size`, `font`, `font-weight`, `bold`, `italic` are now parsed and applied directly from markup (e.g. `<green>INFO</green>`, `<span color='red' bold>ERROR</span>`)

### Fixes

  * `.color` and `.stroke_color` can now take `None`
  * `stext`: a `<` with no matching `>` is now treated as literal text instead of raising `ValueError`, allowing arbitrary content such as ASCII art and file paths


## 0.1.0

  * Initial release
