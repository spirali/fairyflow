---
icon: lucide/type
---

# Text

## Text

`Text` is a multi-span text node. Create it, then call `.span(text)` to add lines of text.
Each span can have its own color, font size, weight, and style.

```ffpy frame="0"
with Scene():
    t = Text()
    t.span("Hello, FairyFlow!").font_size(28).color("steelblue")
```

Multiple spans stack vertically as separate lines:

```ffpy frame="0"
with Scene():
    t = Text()
    t.span("First line").font_size(22).color("darkslateblue")
    t.span("Second line").font_size(22).color("steelblue")
    t.span("Third line").font_size(22).color("cornflowerblue")
```

### Inline groups

Use `.group()` on a `Text` or `TextGroup` to place spans side-by-side on the same line.

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

---

## Positioning

`Text` supports `.xy()`, `.align_x()`, `.align_y()`, and `.move()` for placement — see [Positioning](positioning.md) for details.

## stext

`stext` is a helper that builds a `Text` node from a string with optional inline markup.
Tag names become named spans or groups you can look up later to restyle.

```python
# Plain text — equivalent to Text().span("Hello")
t = stext("Hello, world!")

# Named spans — use .find_node(name="title") to restyle later
t = stext("<title>FairyFlow\n<subtitle>Animation for Python")
t.find_node(name="title").font_size(32).bold().color("steelblue")
t.find_node(name="subtitle").font_size(18).color("gray")
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