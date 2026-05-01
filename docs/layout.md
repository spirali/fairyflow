---
icon: lucide/layout-grid
---

# Layout

## Default (centering) layout

By default, `Scene` and `Group` use a **centering layout** that places each child at the
center of the container. This makes it easy to add a single centered element without
manually computing coordinates:

```ffpy frame="0"
with Scene():
    Rect().size(160, 90).color("steelblue")
```

## Positioning

When you want precise control over placement, use `.xy(x, y)`, `.align_x()`, and
`.align_y()` to override the default position. These methods work on all node types —
shapes, text, images, and groups.

`.xy(x, y)` sets the absolute position within the parent:

```ffpy frame="0"
with Scene():
    Rect().size(60, 60).color("tomato").xy(20, 20)
    Rect().size(60, 60).color("gold").xy(120, 70)
    Rect().size(60, 60).color("mediumseagreen").xy(220, 120)
```

`.align_x(f)` / `.align_y(f)` place the node relative to the container — `0.0` = left/top edge, `0.5` = center, `1.0` = right/bottom edge:

```ffpy frame="0"
with Scene():
    Rect().size(60, 60).color("tomato").align_x(0).align_y(0)
    Rect().size(60, 60).color("gold").align_x(0.5).align_y(0.5)
    Rect().size(60, 60).color("mediumseagreen").align_x(1).align_y(1)
```

`.move(dx, dy)` shifts a node by a fixed offset relative to its already-computed position, so it composes freely with `.align_x` / `.align_y` or `.xy()`:

```ffpy frame="0"
with Scene():
    Rect().size(60, 60).color("orchid").align_x(0.5).align_y(0.5).move(-80, 0)
    Rect().size(60, 60).color("steelblue").align_x(0.5).align_y(0.5)
    Rect().size(60, 60).color("gold").align_x(0.5).align_y(0.5).move(80, 0)
```

---

## Column layout

Call `.column(gap, align)` on a `Group` to stack its children **vertically**:

- `gap` — vertical spacing between children in pixels (default `0`)
- `align` — horizontal alignment: `0.0` = left, `0.5` = center, `1.0` = right (default `0.5`)

```ffpy frame="0"
with Scene():
    with Group().size(120, 180).column(gap=10, align=0.5):
        Rect().size(100, 40).color("steelblue")
        Rect().size(100, 40).color("coral")
        Rect().size(100, 40).color("mediumseagreen")
```

Left-aligned column with varying widths:

```ffpy frame="0"
with Scene():
    with Group().size(200, 160).column(gap=8, align=0.0):
        Rect().size(180, 30).color("steelblue")
        Rect().size(120, 30).color("cornflowerblue")
        Rect().size(80, 30).color("lightskyblue")
        Rect().size(40, 30).color("aliceblue").stroke_color("steelblue").stroke_width(1)
```

---

## Row layout

Call `.row(gap, align)` on a `Group` to place its children **horizontally**:

- `gap` — horizontal spacing between children in pixels (default `0`)
- `align` — vertical alignment: `0.0` = top, `0.5` = center, `1.0` = bottom (default `0.5`)

```ffpy frame="0"
with Scene():
    with Group().size(260, 90).row(gap=10, align=0.5):
        Rect().size(60, 60).color("tomato")
        Rect().size(60, 60).color("gold")
        Rect().size(60, 60).color("mediumseagreen")
        Rect().size(60, 60).color("steelblue")
```

Bottom-aligned row with varying heights:

```ffpy frame="0"
with Scene():
    with Group().size(240, 120).row(gap=8, align=1.0):
        Rect().size(40, 30).color("lightskyblue")
        Rect().size(40, 60).color("cornflowerblue")
        Rect().size(40, 90).color("steelblue")
        Rect().size(40, 60).color("cornflowerblue")
        Rect().size(40, 30).color("lightskyblue")
```

---

## Nested layouts

Groups can be nested to build complex grids and hierarchies:

```ffpy frame="0"
with Scene(width=300, height=200):
    with Group().size(280, 160).row(gap=10, align=0.5):
        with Group().size(80, 160).column(gap=8, align=0.5):
            Rect().size(80, 48).color("steelblue")
            Rect().size(80, 48).color("cornflowerblue")
            Rect().size(80, 48).color("lightskyblue")
        with Group().size(80, 160).column(gap=8, align=0.5):
            Rect().size(80, 48).color("tomato")
            Rect().size(80, 48).color("coral")
            Rect().size(80, 48).color("lightsalmon")
        with Group().size(80, 160).column(gap=8, align=0.5):
            Rect().size(80, 48).color("mediumseagreen")
            Rect().size(80, 48).color("lightgreen")
            Rect().size(80, 48).color("honeydew").stroke_color("mediumseagreen").stroke_width(1)
```

---

## Layout and animation

Groups with column or row layout can still be animated — position, rotation, scale, alpha,
and the clipping window all work as usual. The layout controls where children are placed;
animation moves or transforms the group as a whole.

```ffpy video="mp4"
with Scene():
    with Group().size(220, 70).align_x(0.5).align_y(0.5).row(gap=8, align=0.5) as g:
        Rect().size(60, 50).color("steelblue")
        Rect().size(60, 50).color("coral")
        Rect().size(60, 50).color("gold")
    g.hold()
    linear()
    adv_time(1.5)
    g.rotate(180)
```
