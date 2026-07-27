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

## Grid layout

Call `.grid(cols, gap, gap_y)` on a `Group` to place its children in a grid,
row-major (row = index // cols, column = index % cols):

- `cols` — number of columns (required)
- `gap` — horizontal spacing between columns in pixels (default `0`); also used
  as the vertical spacing if `gap_y` is left unset
- `gap_y` — vertical spacing between rows in pixels (default: same as `gap`)

Each column is sized to its widest child, each row to its tallest. Children
keep their own natural size and are anchored at their cell's top-left corner —
there is no per-cell alignment or stretching in this layout; size cells
explicitly if you need uniform backgrounds (this is exactly what
[`Table`](table.md) does).

```ffpy frame="0"
with Scene():
    with Group().grid(cols=2, gap=10):
        Rect().size(80, 40).fill("steelblue")
        Rect().size(40, 60).fill("coral")
        Rect().size(60, 30).fill("mediumseagreen")
        Rect().size(50, 50).fill("gold")
```

A separate horizontal/vertical gap:

```ffpy frame="0"
with Scene():
    with Group().grid(cols=3, gap=6, gap_y=24):
        Rect().size(40, 30).fill("steelblue")
        Rect().size(40, 30).fill("coral")
        Rect().size(40, 30).fill("mediumseagreen")
        Rect().size(40, 30).fill("gold")
        Rect().size(40, 30).fill("orchid")
        Rect().size(40, 30).fill("tomato")
```

`reserve` (whether inactive children still occupy their grid cell) works the
same way as for [column](#column-layout)/[row](#row-layout) layout, but has
no dedicated `grid()` keyword yet — it's always `True`.

---

## Padding

Call `.padding(all, *, x=, y=, top=, right=, bottom=, left=)` on a `Group` to
add inner spacing between the group's own box and its laid-out children.
Applies to every layout kind (centering, column, row, grid) alike — `gap`
(spacing *between* siblings) and `padding` (spacing between the box edge and
its content) are independent, composable settings; `gap` alone can't produce
what `padding` does, since it never affects the space between the *outermost*
children and the box edge.

To see that clearly, draw the box itself with a border rect behind the
padded content — the two steelblue/coral rects sit inset from every edge of
the outlined box, not just spaced apart from each other:

```ffpy frame="0"
with Scene(width=200, height=140):
    with Group().size(200, 140):
        Rect().expand().fill(None).stroke("steelblue", 2)
        with Group().expand().column(gap=10).padding(20):
            Rect().size(120, 30).fill("steelblue")
            Rect().size(120, 30).fill("coral")
```

Most specific wins: `all` is applied first, then `x`/`y`, then the individual
`top`/`right`/`bottom`/`left` args, each overriding whatever came before
within the same call. A later call only touches the sides it names:

```python
card.padding(16)  # 16px inset on all four sides
card.padding(top=32)  # widen just the top inset, leave the other three sides alone
```

Padding also participates in auto-sizing: a group with no explicit `size()`
call grows to fit its content plus the padding on each side.

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

Groups with column, row, or grid layout can still be animated — position, rotation, scale, alpha,
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
