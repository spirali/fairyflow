---
icon: lucide/crosshair
---

# Positioning

Every node has an `x` / `y` position relative to its parent container. Four methods cover
most placement needs: `.xy()` for absolute coordinates, `.align_x()` / `.align_y()` for
proportional placement within the parent, and `.move()` for small relative offsets on top of
any of the above.

---

## Absolute position — `.xy(x, y)`

`.xy(x, y)` sets the node's position in pixels relative to the top-left corner of its parent:

```ffpy frame="0"
with Scene():
    Rect().size(60, 60).color("tomato").xy(20, 20)
    Rect().size(60, 60).color("gold").xy(120, 70)
    Rect().size(60, 60).color("mediumseagreen").xy(220, 120)
```

---

## Proportional alignment — `.align_x()` / `.align_y()`

`.align_x(f)` and `.align_y(f)` place the node relative to the container size:
`0.0` = left / top edge, `0.5` = center, `1.0` = right / bottom edge.

```ffpy frame="0"
with Scene():
    Rect().size(60, 60).color("tomato").align_x(0).align_y(0)
    Rect().size(60, 60).color("gold").align_x(0.5).align_y(0.5)
    Rect().size(60, 60).color("mediumseagreen").align_x(1).align_y(1)
```

The two methods compose freely — you can align on one axis and use `.xy()` on the other:

```ffpy frame="0"
with Scene():
    Rect().size(80, 50).color("steelblue").align_x(0.5).xy(0, 30)
    Rect().size(80, 50).color("coral").align_x(0.5).xy(0, 110)
```

---

## Relative offset — `.move(dx, dy)`

`.move(dx, dy)` shifts a node by a fixed pixel offset on top of its already-computed
position, regardless of how that position was set:

```ffpy frame="0"
with Scene():
    Rect().size(60, 60).color("orchid").align_x(0.5).align_y(0.5).move(-80, 0)
    Rect().size(60, 60).color("steelblue").align_x(0.5).align_y(0.5)
    Rect().size(60, 60).color("gold").align_x(0.5).align_y(0.5).move(80, 0)
```

---

## Cross-group positioning — `get_pos()` and `pos()`

The methods above always work within the coordinate space of a single parent. When you need
to position a node **relative to another node that lives in a different group**, use
`.get_pos()` and `.pos()`.

`.get_pos(align_x, align_y)` returns a `Position` object — a live coordinate reference
attached to the source node's coordinate space:

| call | point returned |
|---|---|
| `node.get_pos()` | top-left corner of the node |
| `node.get_pos(0.5, 0.5)` | center of the node |
| `node.get_pos(1, 0.5)` | right-center edge |
| `node.get_pos(0.5, 0)` | top-center edge |

`.pos(position)` sets the node's position to a `Position` returned by `.get_pos()`.
FairyFlow automatically converts the coordinates into the target node's local space, so
group offsets, scales, and rotations are all accounted for — **no manual maths needed**.

### Connecting nodes across groups

A common use-case is drawing a line between two nodes that live in separate groups:

```ffpy frame="0"
with Scene(width=300, height=160):
    with Group().size(60, 60).xy(30, 50) as a:
        Rect().size(60, 60).color("steelblue")
    with Group().size(60, 60).xy(210, 50) as b:
        Rect().size(60, 60).color("coral")

    connector = Path().stroke_color("#555").stroke_width(2)
    connector.move_to().pos(a.get_pos(1, 0.5))   # right-center of a
    connector.line_to().pos(b.get_pos(0, 0.5))   # left-center of b
```

### Tracking during animation

Because `.get_pos()` returns a live reference to the node's attribute expressions (not a
snapshot of the current value), a node set with `.pos()` **tracks its source at every
frame**. In the example below, one end of the line is fixed on a static anchor while the
other end is permanently bound to the moving box's center:

```ffpy video="mp4"
with Scene(width=300, height=160):
    anchor = Ellipse().size(12, 12).color("tomato").xy(100, 20)    

    with Group().size(50, 50).xy(20, 55) as box:
        Rect().size(50, 50).color("steelblue")

    line = Path().stroke_color("#888").stroke_width(2)
    line.move_to().pos(anchor.get_pos(0.5, 0.5))  # fixed end
    line.line_to().pos(box.get_pos(0.5, 0.5))     # tracks box center

    box.xy(180, 55, tr=1.5)   # move the box — the line stretches automatically
```

### Fine-tuning with `.move()`

`.pos()` returns `self`, so `.move()` can be chained directly to nudge the result by a
fixed offset:

```ffpy frame="0"
with Scene(width=300, height=160):
    with Group().size(60, 60).align_x(0.5).align_y(0.5) as box:
        Rect().size(60, 60).color("steelblue")

    # arrow tip sits 10 px above the top-center of box
    arrow = Path().stroke_color("tomato").stroke_width(3)
    arrow.move_to().pos(box.get_pos(0.5, 0)).move(0, -30)
    arrow.line_to().pos(box.get_pos(0.5, 0)).move(0, -4)
    arrow.triangle_arrow("end")
```

---

## Following a path — `.follow_path()`

`.follow_path(path, tr=1)` animates a node along a `Path` over the given duration. The
node travels from the path's start (parameter 0) to its end (parameter 1), centered on the
curve at every frame. The clock advances automatically by `tr` seconds.

Any `Path` shape works as the track — straight lines, multi-segment paths, or Bézier curves.
Use `cubic_to()` to add a cubic Bézier segment; its two control points are set with
`c1_xy(dx, dy)` and `c2_xy(dx, dy)`, where the offsets are relative to the segment's start
and end points respectively:

```ffpy video="mp4"
with Scene(width=300, height=200):
    # The track: an arch-shaped cubic Bézier
    track = Path().stroke_color("#bbb").stroke_width(2)
    track.move_to().xy(30, 160)
    curve = track.cubic_to()
    curve.xy(270, 160)
    curve.c1_xy(60, -130)   # control point 1: pulls up from the start
    curve.c2_xy(-60, -130)  # control point 2: pulls up into the end

    # A ball that travels along the arch
    ball = Ellipse().size(22, 22).color("steelblue")
    ball.follow_path(track, tr=2)
```

The same path can be used to animate multiple nodes. Nesting `Seq` inside `Par` creates a
staggered procession where each traveller starts slightly after the previous one:

```ffpy video="mp4"
with Scene(width=300, height=200):
    track = Path().stroke_color("#bbb").stroke_width(2)
    track.move_to().xy(30, 160)
    curve = track.cubic_to()
    curve.xy(270, 160)
    curve.c1_xy(60, -130)
    curve.c2_xy(-60, -130)

    colors = ["steelblue", "coral", "gold"]
    with Par():
        for i, color in enumerate(colors):
            with Seq():
                wait(0.4 * i)
                Ellipse().size(22, 22).color(color).follow_path(track, tr=2)
```
