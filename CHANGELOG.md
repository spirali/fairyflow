# Changelog

## Unreleased

### New

  * Invalid/Missing font throws an error instead of silent error
  * Circular arrow

### Fixes

  * Fixed path resolution for font loading
  * Fixed removing heads when arrow is removed

## 0.5.0

A major redesign of the Python scene API. Not backwards compatible with 0.4.

### New

  * `dur=`/`ease=` keyword arguments on every animatable setter, replacing `tr=`; five easing presets (`"linear"`, `"in"`, `"out"`, `"in_out"`, `"out_back"`)
  * `with anim(dur, ease=):` block and `node.anim(dur, ease=)` proxy set a default duration/easing for calls inside them, instead of repeating `dur=` on every call
  * `.rotate()`, `.scale()`, and `.pivot()` now work on every drawable node (`Rect`, `Ellipse`, `Text`, `Image`, `Group`) instead of `Group` only; `Path` is the one exception
  * `.camera` on every `Group` and the `Scene` — `.zoom()`, `.center()`, `.reset()` — an animatable viewpoint onto a container's content, independent of the container's own box/layout
  * `.next_to(node, direction, gap=, align=)` — place a node beside another, across groups, tracking a moving target
  * `rel(f)` — a value marker for size/position slots meaning *`f` × the parent's corresponding dimension*; `.expand()` for the "fill the whole parent" case
  * `Group.grid(cols, gap=, gap_y=)` layout, and `Group.padding(...)` inner spacing
  * `Rect.radius(r)` for rounded corners
  * `.stroke(color=, width=, dash=, offset=)` gains dashed strokes with an animatable offset
  * `Polygon(points)`, `RegularPolygon(n, radius, ...)`, and `Star(points, outer, inner, ...)` shapes
  * `Line(start, end)` and `Arrow(start, end, gap=, head=, style=)` connector nodes; `.arrow()` gains `style=` (`"triangle"`, `"open"`, `"stealth"`, `"bar"`, `"dot"`) beyond the original filled triangle
  * `Table(rows, header=, widths=, row_height=, padding=, stroke=, fill=, align=)` helper — tabular data as a `Group().grid(...)` of cells, addressed with `.cell()`/`.row()`/`.col()`
  * `Text.wrap(width)` and `.text_align(mode)` for automatic line breaking and block alignment
  * `Text.type_on(dur=)` typewriter reveal
  * `.underline()` / `.strike()` on `Text` and its lines/spans — animatable 0→1 progress, not booleans
  * `set_default_font(family, size, fill=)` and `set_default_code(language, theme=, family=, size=)` project-wide defaults in `prologue.py`; `code(source, language=, theme=)` builds a dedented, markup-free `Text` for source snippets, and `.code()` styles an inline run
  * `Par(stagger=)` — each child of a `Par` starts `i × stagger` seconds later, replacing the manual `Seq()` + `wait()` staggering idiom
  * `Path.draw(dur=, ease=)` sugar for animating a path drawing itself in
  * `note(text)` speaker notes, decoupled from `cue()` — attaches to the current segment, shown in the presenter view
  * `gradient(*stops, angle=)` linear gradients, usable anywhere `.fill()` accepts a color
  * `cue()` now marks the frame and arms a pending one-frame advance, so two instant changes around a `cue()` produce two distinct slides instead of colliding on one frame
  * The player now pauses at the end of every scene by default; `Scene(flow=True)` opts a scene out so it runs straight into the next
  * `fill(color, dur=, ease=)` replaces `color()` on shapes and `Text`; `Scene.background(color, dur=, ease=)` replaces `Scene.color()`
  * `.font(family=, size=, weight=, italic=, bold=, mono=, dur=, ease=)` replaces the separate `font_size()`/`font_weight()`/`bold()`/`italic()` methods
  * `xy(x=, y=)`, `size(w=, h=)`, `clip(x=, y=, w=, h=)`, `align(x=, y=)` replace the old per-axis method families (`x_reset`/`y_reset`/`xy_reset`, `rwidth`/`rheight`/`rsize`, `clip_x`/`clip_y`/`clip_w`/`clip_h`); the `DEFAULT` sentinel resets an attribute to its layout default
  * `.reveal(direction=, dur=)` / `.hide(direction=, dur=)` replace the eight `reveal_*`/`hide_*` direction-suffixed methods
  * `.crop(start=, end=)` replaces `crop_start()`/`crop_end()`; `handle.c1(dx, dy)`/`.c2(dx, dy)` replace the six `c1_*`/`c2_*` cubic-handle methods
  * `at()` (renamed from `get_pos()`) — fraction pairs or nine named anchors (`"center"`, `"top"`, `"bottom"`, `"left"`, `"right"`, `"top_left"`, `"top_right"`, `"bottom_left"`, `"bottom_right"`); a bare single fraction now raises `TypeError` instead of silently meaning top-center
  * `z(level, dur=, ease=)` replaces `z_level()`

### Fixed

  * `reveal("left")` no longer duplicates `hide_left`'s effect (it previously animated the wrong clip axis)

## 0.4.0

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
