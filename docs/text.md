---
icon: lucide/type
---

# Text

## Text

`Text` is a multi-line text node. The constructor takes an optional string,
split on `\n` into lines:

```ffpy frame="0"
with Scene():
    Text("Hello, FairyFlow!").font(size=28).fill("steelblue")
```

```ffpy frame="0"
with Scene():
    Text("First line\nSecond line\nThird line").font(size=22).fill("steelblue")
```

Use `.line(text="")` to start additional lines and style them individually,
and `.span(text)` to add another inline run to the *current* (last) line:

```ffpy frame="0"
with Scene():
    t = Text("First line").font(size=22).fill("darkslateblue")
    t.line("Second line").fill("steelblue")
    t.line("Third line").fill("cornflowerblue")
```

```ffpy frame="0"
with Scene():
    t = Text("INFO ").font("monospace", 22)
    t.span("server started").fill("gray")
```

### Inline groups

Calling `.span(text)` more than once without an intervening `.line()` joins
the runs onto the same line — `.group()` is only needed when you want a
handle to a nested sub-group of runs (e.g. to name or style them together):

```ffpy frame="0"
with Scene():
    t = Text()
    g = t.group()
    g.span("Bold").font(size=26, bold=True).fill("darkred")
    g.span("  normal  ").font(size=26).fill("gray")
    g.span("Italic").font(size=26, italic=True).fill("darkblue")
```

---

## Font styling

All text nodes share one structured style setter,
`.font(family=None, size=None, *, weight=, italic=, bold=, mono=, dur=, ease=)`,
plus `.fill(c)` for the text fill color:

| Parameter | Effect |
|---|---|
| `family` (positional) | Font family name, e.g. `"serif"`, `"monospace"`, or font name |
| `size` (positional) | Font size in pixels |
| `weight=` | Numeric weight (100–900) |
| `bold=True` | Shortcut for `weight=800` (`bold=False` resets to `weight=400`) |
| `mono=True` | Shortcut for `family="monospace"` (`mono=False` resets to `"sans-serif"`) |
| `italic=` | Enable/disable italic |

`bold=`/`weight=` are mutually exclusive (as are `mono=`/`family`) — passing both raises `TypeError`.

```ffpy frame="0"
with Scene():
    t = Text()
    t.span("Small").font(size=14).fill("gray")
    t.span("Medium").font(size=22).fill("steelblue")
    t.span("Large").font(size=36, bold=True).fill("darkslateblue")
```

To use your own font files, add a `font_directories` key to `fairyflow.toml`. FairyFlow scans each listed directory recursively and loads all fonts it finds (`.ttf`, `.otf`, `.ttc`, `.otc`, `.woff`, `.woff2`). Paths are relative to the project root.

```toml
font_directories = ["fonts"]
```

After adding fonts, refer to them by family name in `.font()`:

```python
t.span("Custom text").font("MyFont", 24)
```

### Font aliases

The `[font-aliases]` table in `fairyflow.toml` maps CSS generic family names to specific fonts. This lets you pin which font backs `"sans-serif"`, `"monospace"`, etc., so renders are consistent across machines regardless of system fonts.

```toml
[font-aliases]
sans-serif = "DejaVu Sans"
monospace  = "DejaVu Sans Mono"
serif      = "DejaVu Serif"
```

Any CSS generic family name is accepted as a key: `serif`, `sans-serif`, `monospace`, `cursive`, `fantasy`, `system-ui`, `ui-serif`, `ui-sans-serif`, `ui-monospace`, `ui-rounded`, `emoji`, `math`, `fangsong`. Generic families not listed here keep their system defaults.

The target font must be loaded — either from `font_directories` or from system fonts — before the alias takes effect.

---

## Sizing

By default a `Text` node's box is the *measured* extent of the laid-out text —
whatever width/height the current font size and line breaks produce. Call
`.size(w=, h=)` to scale the finished block to fit an explicit box; this does not
re-layout the text or move any line breaks, it magnifies the whole block as a unit —
the same fit model [`Image`](images.md) uses.

```ffpy frame="0"
with Scene():
    Text("Hi").font(size=16).size(w=100)
```

When only one axis is given, the other is derived automatically to preserve the aspect
ratio:

```ffpy frame="0"
with Scene():
    Text("Hi").font(size=16).size(h=80)
```

When both axes are given, the block is fit into the box scaled uniformly and centered
(letterboxed) by default (`keep_aspect=True`):

```ffpy frame="0"
with Scene():
    Text("Hi").font(size=16).size(100, 60)
```

Pass `.keep_aspect(False)` to stretch the block to exactly fill the box instead:

```ffpy frame="0"
with Scene():
    Text("Hi").font(size=16).size(100, 60).keep_aspect(False)
```

`.expand()` fills the parent completely — handy for poster-style text:

```ffpy frame="0"
with Scene():
    Text("Hi").font(size=16).expand()
```

`.font(size=)` and `.size()` are not the same thing: `.font(size=)` changes the
*layout* — metrics and where line breaks fall. `.size()` magnifies the already
laid-out result; line breaks never move.

---

## Wrapping and alignment

By default a line never breaks on its own — it's exactly as wide as its text. Call
`.wrap(width)` to break a line automatically at word boundaries once it would exceed
`width` (measured before any `.size()` scaling, in the same unscaled layout units as
`.font(size=)`):

```ffpy frame="0"
with Scene():
    Text("The quick brown fox jumps over the lazy dog").font(size=16).wrap(160)
```

`.text_align(mode)` sets how the block's lines are positioned relative to each other —
`"left"` (default), `"center"`, `"right"`, or `"justify"`:

```ffpy frame="0"
with Scene():
    Text("Hi\nA longer second line").font(size=16).wrap(220).text_align("center")
```

```ffpy frame="0"
with Scene():
    Text("Hi\nA longer second line").font(size=16).wrap(220).text_align("right")
```

`"justify"` stretches inter-word spacing so every line except the last fills the wrap
width exactly:

```ffpy frame="0"
with Scene():
    Text("The quick brown fox jumps over the lazy dog. The quick brown fox jumps over the lazy dog.").font(size=16).wrap(
        220
    ).text_align("justify")
```

Call `.text_align()` on an individual `TextGroup` line to override the block's default
for just that line. `.line()` with no text starts an empty group — keep a reference to
it (rather than to `.span()`'s return value) to call `.text_align()` on it:

```ffpy frame="0"
with Scene():
    t = Text().font(size=16).wrap(220).text_align("left")
    t.line("This is just a very long line")
    right_line = t.line()
    right_line.span("Right aligned")
    right_line.text_align("right")
    right_line = t.line()
    right_line.span("Center aligned")
    right_line.text_align("center")
```

`.wrap()` and `.size()` compose the same way `.font(size=)` and `.size()` do: `.size()`
fits the box to the *wrapped* measured extent, not the wrap width itself.

`.wrap()` resets with `DEFAULT` (there is no "auto wrap" default to fall back to — it's
either on with an explicit width, or off entirely):

```python
t.wrap(DEFAULT)  # turns wrapping back off
```

---

## Transforms

`Text` supports `.rotate()`, `.scale()`/`.scale_x()`/`.scale_y()`, and `.pivot()`,
same as `Rect`/`Ellipse`/`Image` — the whole resolved box (after any `.size()` fit)
rotates/scales around its pivot, default the box center:

```ffpy frame="0"
with Scene():
    Text("Rotated").font(size=20).xy(20, 60).fill("darkred").rotate(20)
```

```ffpy frame="0"
with Scene():
    t = Text("Grow").font(size=16).xy(20, 60).fill("darkslateblue")
    t.pivot("top_left")
    t.scale(1.8)
```

---

## Positioning

`Text` supports `.xy()`, `.align()`, and `.move()` for placement — see [Positioning](positioning.md) for details.

### Placing individual lines and runs

A `TextGroup` line (from `.line()`) or a `TextSpan` run (from `.span()`) is placeable
too: `.xy()`/`.pos()`/`.move()`/`.next_to()` override where that one run draws,
*without* reflowing its siblings — the rest of the paragraph keeps its normal layout,
leaving a gap where the overridden run used to sit. `DEFAULT` restores the paragraph
position. This is the "word flies out of the sentence" primitive:

```ffpy frame="0"
with Scene():
    t = Text().font(size=16).xy(4, 20).fill("black")
    t.span("The quick brown ")
    fox = t.span("fox")
    fox.fill("darkred")
    fox.move(15, 60)
    t.span(" jumps")
```

Overriding a `TextGroup` line moves every descendant span that doesn't have its own
override, as a rigid unit — a nested span's own override always wins over its parent
line's (nearest self-or-ancestor, independently per axis):

```ffpy frame="0"
with Scene():
    t = Text().font(size=16).xy(4, 20).fill("black")
    t.line("Untouched line")
    line2 = t.line()
    line2.span("Moved ").fill("darkblue")
    line2.span("line")
    line2.xy(60, 100)
```

A line or run also supports `.rotate()`/`.scale()`/`.scale_x()`/`.scale_y()`/
`.pivot()` — spinning or growing it about its own measured box, in place:

```ffpy frame="0"
with Scene():
    t = Text().font(size=20).xy(20, 40).fill("black")
    t.span("spin ")
    word = t.span("me")
    word.fill("darkred")
    word.rotate(30)
```

```ffpy frame="0"
with Scene():
    t = Text().font(size=16).xy(20, 40).fill("black")
    t.span("grow ")
    word = t.span("me")
    word.fill("darkblue")
    word.pivot("top_left")
    word.scale(2.0)
```

Rotate/scale/pivot resolve independently of `.xy()`/`.move()` (their own nearest-
self-or-ancestor cascade), so a run can spin in place *and* fly elsewhere at the same
time — the run spins about its own natural position first, then the position override
carries it to its new spot:

```ffpy frame="0"
with Scene():
    t = Text().font(size=20).xy(20, 40).fill("black")
    t.span("spin and move ")
    word = t.span("me")
    word.fill("darkred")
    word.rotate(45)
    word.xy(180, 30)
```

Like position, a `TextGroup` line's transform applies to every descendant run that
doesn't have its own closer override, as one rigid unit:

```ffpy frame="0"
with Scene():
    t = Text().font(size=16).xy(60, 20).fill("black")
    t.line("Untouched line")
    line2 = t.line()
    line2.span("Rotated ").fill("darkblue")
    line2.span("line")
    line2.rotate(15)
```

## stext

`stext` is a helper that builds a `Text` node from a string with optional inline markup.
Tags can carry style attributes that are applied immediately, and the tag name is also
preserved as a `.name()` on the span or group so you can look it up later.

```python
# Plain text — equivalent to Text().span("Hello")
t = stext("Hello, world!")
```

### Inline color and style

Use a `color` attribute on any named tag to set the text color:

```ffpy frame="0"
with Scene():
    stext('<info color="green">INFO</info> server started').font("monospace", 22)
```

```ffpy frame="0"
with Scene():
    stext('<span color="#e06c75" bold>ERROR</span> something went wrong').font("monospace", 22)
```

### Supported attributes

| Attribute | Effect |
|---|---|
| `color='...'` | Text fill color (any CSS color) |
| `font-size='N'` | Font size in pixels (also accepted: `text-size`) |
| `font='...'` | Font family |
| `font-weight='N'` | Numeric weight (100–900) |
| `bold` | Shorthand for `font-weight='800'` |
| `italic` | Enable italic |

Multiple attributes on one tag are all applied:

```python
stext("<span color='orange' font-size='20' bold>warning</span> check the logs")
```

### Named spans

The tag name is still recorded via `.name()`, so you can look the span up and restyle it later:

```python
t = stext("<title>FairyFlow\n<subtitle>Animation for Python")
t.find_node(name="title").font(size=32, bold=True).fill("steelblue")
t.find_node(name="subtitle").font(size=18).fill("gray")
```

### Literal `<` characters

A `<` that has no matching `>` is treated as literal text, so you can safely pass
arbitrary content (terminal output, ASCII art, file paths) without escaping:

```python
stext("a < b")           # → "a < b"
stext("path/to/<file>")  # → tag named "file" — wrap in a real tag name only when intended
```

By default `stext` uses `<tag>...</tag>` syntax. Pass `delimiters="[]"` to use `[tag]...[/tag]` instead.

---

## Syntax highlighting

Enable source-code syntax highlighting on a `Text` node with `.sh(language)`.

```ffpy frame="0"
with Scene():
    stext(
"""
x = "world"
print(f"Hello {x}!")
"""
    ).sh("Python").font("monospace")
```

An optional `.sh(language, theme=...)` argument selects the color theme.
Preinstalled themes: 

* "InspiredGitHub" (default)
* "base16-ocean.dark"
* "base16-eighties.dark"
* "base16-mocha.dark"
* "base16-ocean.light"
* "Solarized (dark)"
* "Solarized (light)"

```ffpy frame="0"
with Scene(background="#2b303b"):
    stext(
"""
x = "world"
print(f"Hello {x}!")
"""
    ).sh("Python", theme="base16-ocean.dark").font("monospace")
```

---

## Typewriter reveal

`.type_on(dur=)` animates glyphs appearing progressively, in reading order:

```ffpy video="mp4"
with Scene():
    Text("fairyflow renders this\nletter by letter").type_on(dur=1.5)
```

Glyphs pop in at their already-laid-out final position — the text never reflows as
more of it becomes visible, and `dur=`/`ease=` follow the usual conventions
(`ease="linear"` by default). Revealing is a whole-glyph cutoff, not a character-exact
or fractional-glyph reveal, so timing is a close visual approximation rather than
precise per-character pacing.