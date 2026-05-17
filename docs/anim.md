---
icon: lucide/film
---

# Animations

## Timeline control

Three functions advance the animation clock:

| Function | Description |
|---|---|
| `wait(seconds)` | Advance time by the given number of seconds |
| `wait(Frames(n))` | Advance by an exact number of frames |
| `next_frame()` | Shortcut for `wait(Frames(1))` |

The default frame rate is **24 fps**. This can be changed in `fairyflow.toml`.

```ffpy video="mp4"
with Scene():
    r = Rect().size(80, 80).color("steelblue").align_x(0.5).align_y(0.5)
    wait(1)           # pause one second
    r.color("tomato") # instant change at t=1s
    wait(0.5)
    r.color("gold")
```

---

## The `tr` parameter

Every attribute method accepts an optional `tr` keyword argument that specifies the
**transition duration**. Without it, the change is instant. With it, the attribute
animates linearly and the clock advances by the transition duration.

```python
r.xy(20, 30, tr=0.5)      # moves to (20, 30) over 0.5 s; clock advances 0.5 s
r.color("green", tr=1)    # changes colour over 1 s; clock advances 1 s
r.alpha(0, tr=Frames(6))  # fades out over 6 frames; clock advances 6 frames
r.color("red")            # instant colour change; clock does not advance
```

```ffpy video="mp4"
with Scene():
    r = Rect().size(80, 80).color("steelblue").align_x(0.5).align_y(0.5)
    wait(1)
    r.color("tomato")          # instant — no tr
    wait(1)
    r.color("steelblue", tr=1) # animated — tr=1
```

The first change snaps instantly at t = 1 s; the second transitions smoothly over 1 s.

---

## Sequential and parallel composition

Code at the top level is already **sequential** — each line follows the previous one.
`Seq` and `Par` are context managers that control how their contents are composed in time.

**`with Seq():`** — children run one after the other:

```ffpy video="mp4"
with Scene(width=300, height=140):
    with Seq():
        Rect().size(100, 80).xy(15, 30).color("steelblue").fade_out()
        Rect().size(100, 80).xy(165, 30).color("tomato").fade_in()
```

**`with Par():`** — all children start at the same time; the clock advances to the
longest child's end time:

```ffpy video="mp4"
with Scene(width=300, height=140):
    with Par():
        Rect().size(100, 80).xy(15, 30).color("steelblue").fade_out()
        Rect().size(100, 80).xy(165, 30).color("tomato").fade_out()
```

Creating objects before the composition block makes them all visible from the start:

```ffpy video="mp4"
with Scene(width=300, height=140):
    a = Rect().size(100, 80).xy(15, 30).color("steelblue")
    b = Rect().size(100, 80).xy(165, 30).color("tomato")

    with Seq():
        a.fade_out()   # a fades first …
        b.fade_out()   # … then b follows
```

`Par` and `Seq` compose freely. A typical pattern is a **sequence of parallel steps** —
top-level sequential code with `Par` blocks inside, each firing multiple animations at once:

```ffpy video="mp4"
with Scene():
    a = Rect().size(80, 60).color("steelblue").xy(20, 60)
    b = Rect().size(80, 60).color("coral").xy(120, 60)

    with Par():        # step 1: both fade in together
        a.fade_in(0.5)
        b.fade_in(0.5)

    wait(0.3)

    with Par():        # step 2: both animate simultaneously
        a.xy(220, 60, tr=0.8)
        b.color("gold", tr=0.8)
```

Going the other way, nesting `Seq` inside `Par` creates **staggered** parallel animations —
a common pattern for cascading effects:

```ffpy video="mp4"
with Scene():
    with Par():
        for i in range(5):
            with Seq():
                wait(0.15 * i)          # offset each rect
                Rect().size(40, 40).xy(20 + 60 * i, 60).color("orchid").fade_in(0.4)
```

---

## Moving and transforming

Combine `tr` with `Par` to animate position, size, and colour simultaneously:

```ffpy video="mp4"
with Scene():
    r = Rect().size(60, 60).color("tomato").xy(20, 70)
    with Par():
        r.xy(220, 70, tr=1.5)
        r.color("steelblue", tr=1.5)
        r.size(80, 80, tr=1.5)
```

Use `wait()` to insert a pause before starting a transition:

```ffpy video="mp4"
with Scene():
    r = Rect().size(60, 60).color("gold").align_x(0.5).align_y(0.5)
    wait(1)
    with Par():
        r.color("tomato", tr=1)
        r.size(120, 120, tr=1)
    wait(0.5)
    with Par():
        r.color("steelblue", tr=1)
        r.size(60, 60, tr=1)
```

---

## Rotation and scale

`Group` nodes support `.rotate()` and `.scale()` animations. The pivot defaults to the
group's centre (`pivot_x=0.5`, `pivot_y=0.5`).

```ffpy video="mp4"
with Scene():
    with Group().size(80, 80).align_x(0.5).align_y(0.5) as g:
        Rect().size(80, 80).color("steelblue")
        Rect().size(20, 20).color("white").xy(30, 30)
    g.rotate(360, tr=2)
```

```ffpy video="mp4"
with Scene():
    with Group().size(80, 80).align_x(0.5).align_y(0.5) as g:
        Ellipse().size(80, 80).color("coral")
    g.scale(0.2, tr=1)
    g.scale(1, tr=1)
```

---

## Fade in and fade out

`.fade_in(time)` animates alpha from 0 to 1. `.fade_out(time)` animates alpha from its
current value down to 0. Both advance the clock automatically.

```ffpy video="mp4"
with Scene():
    r = Rect().size(120, 80).color("orchid").align_x(0.5).align_y(0.5)
    r.fade_in(0.6)
    wait(0.6)       # hold at full opacity
    r.fade_out(0.6)
```

You can also set alpha directly with `tr`:

```ffpy video="mp4"
with Scene():
    r = Rect().size(120, 80).color("steelblue").align_x(0.5).align_y(0.5)
    r.alpha(0)
    r.alpha(1, tr=1)   # fade in over 1 s
    wait(0.5)
    r.alpha(0, tr=1)   # fade out over 1 s
```

---

## Clipping animations

`Group` nodes have an animatable clipping window. `clip_x`/`clip_w` control the horizontal
extent (as a fraction of the group width), and `clip_y`/`clip_h` control the vertical extent.

```ffpy video="mp4"
with Scene():
    with Group().size(200, 60) as g:
        Rect().size(200, 60).color("cornflowerblue")
        t = Text()
        t.span("Revealed!").font_size(22).bold().color("white")
        t.xy(40, 18)
    g.clip_w(0)               # start fully hidden
    g.clip_w(1, tr=1.2)       # reveal left-to-right over 1.2 s
```

`.hide_right(time)`, `.hide_left(time)`, `.reveal_right(time)`, and `.reveal_left(time)` are
convenience helpers that animate the clip to conceal or reveal the group content. The direction
refers to the sweep direction: `reveal_right` expands the clip window rightward, `reveal_left`
sweeps it leftward. Each call also advances the clock automatically.

```ffpy video="mp4"
with Scene():
    with Group().size(200, 60).align_x(0.5).align_y(0.5) as g:
        Rect().size(200, 60).color("cornflowerblue")
        t = Text()
        t.span("reveal_right / hide_left").font_size(14).bold().color("white")
        t.xy(18, 22)
    g.reveal_right(1)   # expand clip from left to right
    wait(0.4)
    g.hide_left(1)      # shrink clip from right to left
```
