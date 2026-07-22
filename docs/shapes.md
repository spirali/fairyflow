---
icon: lucide/shapes
---

# Shapes

## Rect

`Rect` draws a filled rectangle. Set its size with `.size(width, height)`, fill color with
`.color()`, and position with `.xy(x, y)`. By default the rect has zero size and is placed
according to the parent's layout (centered for the default layout).

```ffpy frame="0"
with Scene():
    Rect().size(160, 90).color("steelblue")
```

### Stroke

Add an outline with `.stroke_color()` and `.stroke_width()`. Setting a fill color to `None`
gives a hollow shape.

```ffpy frame="0"
with Scene():
    r = Rect().size(160, 90)
    r.color("lightyellow").stroke_color("navy").stroke_width(4)
```

### Relative sizing

`rel(f)` is a value marker for `size()` (and `xy()`, see [Positioning](positioning.md)):
`f` times the parent's corresponding dimension. `1.0` equals the full parent extent on
that axis. The bare "fill the whole parent" case has its own verb, `.expand()`. "Parent"
here is the node's nearest `Group()`, or the `Scene` itself when there is no intermediate
group — no wrapping `Group()` is required just to use `rel()`.

```ffpy frame="0"
with Scene(width=300, height=180):
    with Group().size(300, 180):
        Rect().expand().color("whitesmoke")   # full background
        Rect().size(rel(0.5), rel(0.5)).color("steelblue")  # top-left quadrant
```

```ffpy frame="0"
with Scene(width=300, height=180):
    with Group().size(300, 180):
        Rect().expand().color("whitesmoke")
        Rect().size(w=rel(1), h=rel(0.25)).color("coral")   # full-width banner
```

```ffpy frame="0"
with Scene(width=300, height=180):
    Rect().size(rel(0.5), rel(0.5)).color("steelblue")   # relative to the Scene directly
```

### Positioning

See [Positioning](positioning.md) for `.xy()`, `.align()`, and `.move()`.

---

## Ellipse

`Ellipse` draws an ellipse. When `width == height` it becomes a circle. Its API is identical
to `Rect`.

```ffpy frame="0"
with Scene():
    Ellipse().size(160, 110).color("coral")
```

```ffpy frame="0"
with Scene():
    Ellipse().size(80, 80).color("orchid").xy(30, 60)
    Ellipse().size(80, 40).color("gold").xy(130, 80)
    Ellipse().size(40, 80).color("steelblue").xy(220, 60)
```

---

## Path

`Path` draws an arbitrary vector shape from a sequence of commands. Use `.stroke_color()` to
set the line color and `.stroke_width()` for line thickness. Like `Rect`, it can also be
filled with `.color()`.

### Line segments

```ffpy frame="0"
with Scene():
    p = Path()
    p.stroke_color("darkslateblue").stroke_width(3).color("lavender")
    p.move_to().xy(30, 100)
    p.line_to().xy(150, 40)
    p.line_to().xy(270, 100)
    p.line_to().xy(150, 160)
    p.close()
```

### Cubic Bézier curves

`.cubic_to()` appends a cubic Bézier segment. Use `.c1_xy(dx, dy)` and `.c2_xy(dx, dy)` to
set the two control point offsets (relative to the segment start and end respectively).

```ffpy frame="0"
with Scene():
    p = Path()
    p.stroke_color("darkorange").stroke_width(3)
    p.move_to().xy(40, 150)
    p.cubic_to().xy(260, 150).c1_xy(60, -130).c2_xy(-60, -130)
```

### Arrows

`.arrow()` adds an arrowhead at the end of a path. Pass `"start"` to place it at the
beginning instead. The arrowhead is automatically sized to match the stroke width.

```ffpy frame="0"
with Scene():
    p = Path()
    p.stroke_color("steelblue").stroke_width(3)
    p.move_to().xy(40, 100)
    p.line_to().xy(260, 100)
    p.arrow("end")
    p.arrow("start").color("tomato")
```

The arrowhead inherits the path's stroke color by default. Call `.color()` on the returned arrow object to override it independently — as shown above with the red start arrow.

The arrowhead scales automatically with stroke width. Pass `length` and `width` to `arrow()` to override the size explicitly — `length` is the tip-to-base distance, `width` is the base width (both default to `3 × stroke_width`):

```ffpy frame="0"
with Scene():
    # top: automatic size (stroke_width=8 → arrow 24×24)
    p = Path()
    p.stroke_color("steelblue").stroke_width(8)
    p.move_to().xy(40, 60)
    p.line_to().xy(250, 60)
    p.arrow("end")

    # bottom: same stroke but arrow manually set to length=40, width=20
    q = Path()
    q.stroke_color("steelblue").stroke_width(8)
    q.move_to().xy(40, 140)
    q.line_to().xy(250, 140)
    q.arrow("end", length=40, width=20)
```

### Arrowhead styles

Pass `style=` to choose a different head shape. `"open"` and `"bar"` are stroked only
(no fill); `"dot"` returns an `Ellipse` instead of a `Path`.

| style | shape |
|---|---|
| `"triangle"` | filled triangle *(default)* |
| `"open"` | two stroked lines forming a V |
| `"stealth"` | concave filled head (TikZ-like) |
| `"bar"` | perpendicular stroke — measurement / UML ends |
| `"dot"` | filled circle at the endpoint |

```ffpy frame="0"
with Scene(width=200, height=200):
    for i, style in enumerate(["triangle", "open", "stealth", "bar", "dot"]):
        y = 20 + i * 35
        p = Path()
        p.stroke_color("steelblue").stroke_width(3)
        p.move_to().xy(40, y)
        p.line_to().xy(140, y)
        p.arrow("end", style=style, width=20)
```

### Path cropping

`crop_start` and `crop_end` trim the path from either end. Values are in `[0.0, 1.0]` where
`0.0` is the full extent. This is mainly used to animate paths drawing themselves in.

```ffpy frame="0"
with Scene():
    p = Path()
    p.stroke_color("mediumseagreen").stroke_width(4)
    p.move_to().xy(30, 100)
    p.cubic_to().xy(150, 40).c1_xy(50, -60).c2_xy(-50, -60)
    p.cubic_to().xy(270, 100).c1_xy(50, 60).c2_xy(-50, 60)
    p.crop_end(0.5)
```
