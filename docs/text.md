---
icon: lucide/type
---

# Text

## Text

`Text` is a multi-line text node. The constructor takes an optional string,
split on `\n` into lines:

```ffpy frame="0"
with Scene():
    Text("Hello, FairyFlow!").font_size(28).color("steelblue")
```

```ffpy frame="0"
with Scene():
    Text("First line\nSecond line\nThird line").font_size(22).color("steelblue")
```

Use `.line(text="")` to start additional lines and style them individually,
and `.span(text)` to add another inline run to the *current* (last) line:

```ffpy frame="0"
with Scene():
    t = Text("First line").font_size(22).color("darkslateblue")
    t.line("Second line").color("steelblue")
    t.line("Third line").color("cornflowerblue")
```

```ffpy frame="0"
with Scene():
    t = Text("INFO ").font_size(22).font("monospace")
    t.span("server started").color("gray")
```

### Inline groups

Calling `.span(text)` more than once without an intervening `.line()` joins
the runs onto the same line — `.group()` is only needed when you want a
handle to a nested sub-group of runs (e.g. to name or style them together):

```ffpy frame="0"
with Scene():
    t = Text()
    g = t.group()
    g.span("Bold").bold().font_size(26).color("darkred")
    g.span("  normal  ").font_size(26).color("gray")
    g.span("Italic").italic(True).font_size(26).color("darkblue")
```

---

## Font styling

All text nodes share the same set of style methods:

| Method | Description |
|---|---|
| `.font_size(px)` | Font size in pixels |
| `.font(name)` | Font family name, e.g. `"serif"`, `"monospace"`, or font name |
| `.bold()` | Shortcut for `.font_weight(800)` |
| `.font_weight(w)` | Weight value (100–900) |
| `.italic(True)` | Enable italic |
| `.color(c)` | Text fill color |

```ffpy frame="0"
with Scene():
    t = Text()
    t.span("Small").font_size(14).color("gray")
    t.span("Medium").font_size(22).color("steelblue")
    t.span("Large").font_size(36).bold().color("darkslateblue")
```

To use your own font files, add a `font_directories` key to `fairyflow.toml`. FairyFlow scans each listed directory recursively and loads all fonts it finds (`.ttf`, `.otf`, `.ttc`, `.otc`, `.woff`, `.woff2`). Paths are relative to the project root.

```toml
font_directories = ["fonts"]
```

After adding fonts, refer to them by family name in `.font()`:

```python
t.span("Custom text").font("MyFont").font_size(24)
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

## Positioning

`Text` supports `.xy()`, `.align()`, and `.move()` for placement — see [Positioning](positioning.md) for details.

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
    stext('<info color="green">INFO</info> server started').font_size(22).font("monospace")
```

```ffpy frame="0"
with Scene():
    stext('<span color="#e06c75" bold>ERROR</span> something went wrong').font_size(22).font("monospace")
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
t.find_node(name="title").font_size(32).bold().color("steelblue")
t.find_node(name="subtitle").font_size(18).color("gray")
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
with Scene(color="#2b303b"):
    stext(
"""
x = "world"
print(f"Hello {x}!")
"""
    ).sh("Python", theme="base16-ocean.dark").font("monospace")
```