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

### Positioning

`.xy(x, y)` sets the absolute position within the parent. `.align_x(f)` and `.align_y(f)`
align the node using a factor: `0.0` = left/top, `0.5` = center, `1.0` = right/bottom.
`.move(dx, dy)` shifts the node relative to its current position.

```ffpy frame="0"
with Scene():
    Rect().size(60, 60).color("tomato").xy(20, 20)
    Rect().size(60, 60).color("gold").xy(120, 70)
    Rect().size(60, 60).color("mediumseagreen").xy(220, 120)
```

TODO: Demo for using `.align`
TODO: Demo for using `.move`

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

`.triangle_arrow()` adds a filled arrowhead at the end of a path. Pass `"start"` to place it
at the beginning instead. The arrowhead is automatically sized to match the stroke width.

```ffpy frame="0"
with Scene():
    p = Path()
    p.stroke_color("steelblue").stroke_width(3)
    p.move_to().xy(40, 100)
    p.line_to().xy(260, 100)
    p.triangle_arrow()
    p.triangle_arrow("start").color("tomato")
```

TODO: Mention that color is inherited but can be overriden
TODO: Demo for larger arrow

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
