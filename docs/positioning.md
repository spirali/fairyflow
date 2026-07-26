---
icon: lucide/crosshair
---

# Positioning

Every node has an `x` / `y` position relative to its parent container. Three methods cover
most placement needs: `.xy()` for absolute coordinates, `.align()` for proportional placement
within the parent, and `.move()` for small relative offsets on top of any of the above.

---

## Absolute position — `.xy(x, y)`

`.xy(x, y)` sets the node's position in pixels relative to the top-left corner of its parent:

```ffpy frame="0"
with Scene():
    Rect().size(60, 60).fill("tomato").xy(20, 20)
    Rect().size(60, 60).fill("gold").xy(120, 70)
    Rect().size(60, 60).fill("mediumseagreen").xy(220, 120)
```

---

## Relative position — `rel(f)`

`rel(f)` is a value marker accepted anywhere `.xy()` takes a coordinate (and by `.size()`,
see [Shapes](shapes.md#relative-sizing)): it means *`f` times the parent's corresponding
dimension* — width in the `x` slot, height in the `y` slot. It resolves against the node's
parent — the nearest `Group()`, or the `Scene` itself when the node has no intermediate
group:

```ffpy frame="0"
with Scene(width=300, height=160):
    Rect().size(60, 60).fill("tomato").xy(x=rel(0.5) - 30, y=rel(0.5) - 30)
```

`rel(f)` is an `Expr`, so it composes with arithmetic (`rel(1) - 20`, `rel(0.5) + 10`) —
useful when you need a proportional position offset by a fixed pixel amount. Unlike
`.align()`, whose `0.0`–`1.0` factor already accounts for the node's own size, `rel(f)`
is a raw fraction of the parent's box with no such adjustment.

---

## Proportional alignment — `.align()`

`.align(x=fx, y=fy)` places the node relative to the container size:
`0.0` = left / top edge, `0.5` = center, `1.0` = right / bottom edge. Either axis can be
omitted to leave it untouched.

```ffpy frame="0"
with Scene():
    Rect().size(60, 60).fill("tomato").align(0, 0)
    Rect().size(60, 60).fill("gold").align(0.5, 0.5)
    Rect().size(60, 60).fill("mediumseagreen").align(1, 1)
```

`.align()` composes freely with `.xy()` — you can align on one axis and set the other
directly:

```ffpy frame="0"
with Scene():
    Rect().size(80, 50).fill("steelblue").align(x=0.5).xy(0, 30)
    Rect().size(80, 50).fill("coral").align(x=0.5).xy(0, 110)
```

---

## Relative offset — `.move(dx, dy)`

`.move(dx, dy)` shifts a node by a fixed pixel offset on top of its already-computed
position, regardless of how that position was set:

```ffpy frame="0"
with Scene():
    Rect().size(60, 60).fill("orchid").align(0.5, 0.5).move(-80, 0)
    Rect().size(60, 60).fill("steelblue").align(0.5, 0.5)
    Rect().size(60, 60).fill("gold").align(0.5, 0.5).move(80, 0)
```

---

## Cross-group positioning — `at()` and `pos()`

The methods above always work within the coordinate space of a single parent. When you need
to position a node **relative to another node that lives in a different group**, use
`.at()` and `.pos()`.

`.at(x, y)` returns a `Position` object — a live coordinate reference attached to the
source node's coordinate space. Fractions must be given in pairs; a bare `at(0.5)` raises
`TypeError` rather than silently meaning top-center. Nine named anchors cover the common
cases:

| call | point returned |
|---|---|
| `node.at()` | center of the node (the default — also the only point for a size-less node) |
| `node.at("center")` | center (same as bare `at()`) |
| `node.at("top")` | top-center edge |
| `node.at("bottom")` | bottom-center edge |
| `node.at("left")` | left-center edge |
| `node.at("right")` | right-center edge |
| `node.at("top_left")` | top-left corner |
| `node.at("top_right")` | top-right corner |
| `node.at("bottom_left")` | bottom-left corner |
| `node.at("bottom_right")` | bottom-right corner |
| `node.at(0.2, 0.7)` | 20% across, 70% down |

`.pos(position)` sets the node's position to a `Position` returned by `.at()`.
FairyFlow automatically converts the coordinates into the target node's local space, so
group offsets, scales, and rotations are all accounted for — **no manual maths needed**.
This works between any two nodes, whether they live inside a `Group()` or directly in
the bare `Scene`.

### Connecting nodes across groups

A common use-case is drawing a line between two nodes that live in separate groups:

```ffpy frame="0"
with Scene(width=300, height=160):
    with Group().size(60, 60).xy(30, 50) as a:
        Rect().size(60, 60).fill("steelblue")
    with Group().size(60, 60).xy(210, 50) as b:
        Rect().size(60, 60).fill("coral")

    connector = Path().stroke("#555", 2)
    connector.move_to(a.at("right"))   # right-center of a
    connector.line_to(b.at("left"))    # left-center of b
```

### Tracking during animation

Because `.at()` returns a live reference to the node's attribute expressions (not a
snapshot of the current value), a node set with `.pos()` **tracks its source at every
frame**. In the example below, one end of the line is fixed on a static anchor while the
other end is permanently bound to the moving box's center:

```ffpy video="mp4"
with Scene(width=300, height=160):
    anchor = Ellipse().size(12, 12).fill("tomato").xy(100, 20)    

    with Group().size(50, 50).xy(20, 55) as box:
        Rect().size(50, 50).fill("steelblue")

    line = Path().stroke("#888", 2)
    line.move_to(anchor.at())  # fixed end
    line.line_to(box.at())     # tracks box center (the default anchor)

    box.xy(180, 55, dur=1.5)   # move the box — the line stretches automatically
```

### Fine-tuning with `.move()`

`.pos()` returns `self`, so `.move()` can be chained directly to nudge the result by a
fixed offset:

```ffpy frame="0"
with Scene(width=300, height=160):
    with Group().size(60, 60).align(0.5, 0.5) as box:
        Rect().size(60, 60).fill("steelblue")

    # arrow tip sits 10 px above the top-center of box
    arrow = Path().stroke("tomato", 3)
    arrow.move_to(box.at("top")).move(0, -30)
    arrow.line_to(box.at("top")).move(0, -4)
    arrow.arrow("end")
```

---

## Sibling placement — `.next_to()`

`.at()`/`.pos()` place a node at a *point*; `.next_to(node, direction, gap=0, align=0.5)`
places it **beside another node**, taking both boxes' size into account — the everyday
"label next to box" case that would otherwise need manual offset math:

```ffpy frame="0"
with Scene(width=300, height=120):
    with Group().size(60, 60).xy(40, 30) as box:
        Rect().size(60, 60).fill("steelblue")

    label = Text()
    label.span("label").font(size=16)
    label.next_to(box, "right", gap=12)   # right of box, vertically centered
```

`direction` is one of `"right"`, `"left"`, `"above"`, `"below"` — which side of the target
to place on. `gap` is the pixel distance between the facing edges. `align` places the node
along the perpendicular axis: `0` start-aligned, `0.5` centered (the default), `1`
end-aligned:

```ffpy frame="0"
with Scene(width=300, height=120):
    with Group().size(60, 60).xy(40, 20) as img:
        Rect().size(60, 60).fill("coral")

    caption = Text()
    caption.span("caption").font(size=14)
    caption.next_to(img, "below", gap=8, align=0)   # under img, left edges aligned
```

Like `.pos()`, `next_to()` works **across groups** and is **live** — built on the same
`node_transform` machinery, so the node keeps tracking its target as it moves. It also
accepts `dur=`/`ease=` like any other position change.

---

## Following a path — `.follow_path()`

`.follow_path(path, dur=1, start=0, end=1)` animates a node along a `Path` over the given
duration. The node travels from `start` to `end` (both are path parameters in the range
0–1, where 0 is the path's start and 1 is its end), centered on the curve at every frame.
The clock advances automatically by `dur` seconds.

Any `Path` shape works as the track — straight lines, multi-segment paths, or Bézier curves.
Use `cubic_to(x, y, c1=, c2=)` to add a cubic Bézier segment; `c1`/`c2` are `(dx, dy)`
offsets relative to the segment's start and end points respectively:

```ffpy video="mp4"
with Scene(width=300, height=200):
    # The track: an arch-shaped cubic Bézier
    track = Path().stroke("#bbb", 2)
    track.move_to(30, 160)
    track.cubic_to(270, 160, c1=(60, -130), c2=(-60, -130))
    # c1: pulls up from the start; c2: pulls up into the end

    # A ball that travels along the arch
    ball = Ellipse().size(22, 22).fill("steelblue")
    ball.follow_path(track, dur=2)
```

Pass `start=1, end=0` to travel in the opposite direction — from the end of the path to the
start. This is useful for animating a return trip or for reversing entrance effects:

```ffpy video="mp4"
with Scene(width=300, height=200):
    track = Path().stroke("#bbb", 2)
    track.move_to(30, 160)
    track.cubic_to(270, 160, c1=(60, -130), c2=(-60, -130))

    with Par():
        Ellipse().size(22, 22).fill("steelblue").follow_path(track, dur=2)
        Ellipse().size(22, 22).fill("coral").follow_path(track, dur=2, start=1, end=0)
```

The same path can be used to animate multiple nodes. Nesting `Seq` inside `Par` creates a
staggered procession where each traveller starts slightly after the previous one:

```ffpy video="mp4"
with Scene(width=300, height=200):
    track = Path().stroke("#bbb", 2)
    track.move_to(30, 160)
    track.cubic_to(270, 160, c1=(60, -130), c2=(-60, -130))

    colors = ["steelblue", "coral", "gold"]
    with Par():
        for i, color in enumerate(colors):
            with Seq():
                wait(0.4 * i)
                Ellipse().size(22, 22).fill(color).follow_path(track, dur=2)
```
