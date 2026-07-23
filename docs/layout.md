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
    Rect().size(160, 90).fill("steelblue")
```

## Positioning

For absolute coordinates, proportional alignment, and cross-node positioning
(including across groups), see [Positioning](positioning.md).

---

## Column layout

Call `.column(gap, align, reserve)` on a `Group` to stack its children **vertically**:

- `gap` — vertical spacing between children in pixels (default `0`)
- `align` — horizontal alignment: `0.0` = left, `0.5` = center, `1.0` = right (default `0.5`)
- `reserve` — whether inactive children still occupy space (default `True`; see [below](#the-reserve-parameter))

```ffpy frame="0"
with Scene():
    with Group().size(120, 180).column(gap=10, align=0.5):
        Rect().size(100, 40).fill("steelblue")
        Rect().size(100, 40).fill("coral")
        Rect().size(100, 40).fill("mediumseagreen")
```

Left-aligned column with varying widths:

```ffpy frame="0"
with Scene():
    with Group().size(200, 160).column(gap=8, align=0.0):
        Rect().size(180, 30).fill("steelblue")
        Rect().size(120, 30).fill("cornflowerblue")
        Rect().size(80, 30).fill("lightskyblue")
        Rect().size(40, 30).fill("aliceblue").stroke("steelblue", 1)
```

---

## Row layout

Call `.row(gap, align, reserve)` on a `Group` to place its children **horizontally**:

- `gap` — horizontal spacing between children in pixels (default `0`)
- `align` — vertical alignment: `0.0` = top, `0.5` = center, `1.0` = bottom (default `0.5`)
- `reserve` — whether inactive children still occupy space (default `True`; see [below](#the-reserve-parameter))

```ffpy frame="0"
with Scene():
    with Group().size(260, 90).row(gap=10, align=0.5):
        Rect().size(60, 60).fill("tomato")
        Rect().size(60, 60).fill("gold")
        Rect().size(60, 60).fill("mediumseagreen")
        Rect().size(60, 60).fill("steelblue")
```

Bottom-aligned row with varying heights:

```ffpy frame="0"
with Scene():
    with Group().size(240, 120).row(gap=8, align=1.0):
        Rect().size(40, 30).fill("lightskyblue")
        Rect().size(40, 60).fill("cornflowerblue")
        Rect().size(40, 90).fill("steelblue")
        Rect().size(40, 60).fill("cornflowerblue")
        Rect().size(40, 30).fill("lightskyblue")
```

---

## Nested layouts

Groups can be nested to build complex grids and hierarchies:

```ffpy frame="0"
with Scene(width=300, height=200):
    with Group().size(280, 160).row(gap=10, align=0.5):
        with Group().size(80, 160).column(gap=8, align=0.5):
            Rect().size(80, 48).fill("steelblue")
            Rect().size(80, 48).fill("cornflowerblue")
            Rect().size(80, 48).fill("lightskyblue")
        with Group().size(80, 160).column(gap=8, align=0.5):
            Rect().size(80, 48).fill("tomato")
            Rect().size(80, 48).fill("coral")
            Rect().size(80, 48).fill("lightsalmon")
        with Group().size(80, 160).column(gap=8, align=0.5):
            Rect().size(80, 48).fill("mediumseagreen")
            Rect().size(80, 48).fill("lightgreen")
            Rect().size(80, 48).fill("honeydew").stroke("mediumseagreen", 1)
```

---

## The `reserve` parameter

When items appear or disappear across frames, column and row layouts need to
decide whether absent children still take up space.

**`reserve=True` (default)** — every child always occupies its full size in
the layout, even when it is not yet visible or has already been removed.
This keeps the positions of all siblings stable: nothing shifts when a new
item appears or an old one disappears.

```ffpy frames="0,1"
with Scene(width=200, height=120):
    with Group().column(gap=10):   # reserve=True by default
        Rect().size(160, 30).fill("steelblue")
        next_frame()
        Rect().size(160, 50).fill("coral")
```

**`reserve=False`** — only currently active children occupy space.  The
layout shrinks and grows as items come and go, so siblings recentre or
reflow on each frame.

```ffpy frames="0,1"
with Scene(width=200, height=120):
    with Group().column(gap=10, reserve=False):
        Rect().size(160, 30).fill("steelblue")
        next_frame()
        Rect().size(160, 50).fill("coral")
```

Use `reserve=True` when you want a stable layout where items "drop in" to
their final position without displacing neighbours.  Use `reserve=False`
when you want the group to tightly fit its visible children at every frame.

---

## Layout and animation

Groups with column or row layout can still be animated — position, rotation, scale, alpha,
and the clipping window all work as usual. The layout controls where children are placed;
animation moves or transforms the group as a whole.

```ffpy video="mp4"
with Scene():
    with Group().size(220, 70).align(0.5, 0.5).row(gap=8, align=0.5) as g:
        Rect().size(60, 50).fill("steelblue")
        Rect().size(60, 50).fill("coral")
        Rect().size(60, 50).fill("gold")
    g.rotate(180, dur=1.5)
```
