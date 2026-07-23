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
    r = Rect().size(80, 80).color("steelblue").align(0.5, 0.5)
    wait(1)           # pause one second
    r.color("tomato") # instant change at t=1s
    wait(0.5)
    r.color("gold")
```

---

## The `dur` parameter

Every attribute method accepts an optional `dur` keyword argument that specifies the
**transition duration**. Without it, the change is instant. With it, the attribute
animates and the clock advances by the transition duration.

```python
r.xy(20, 30, dur=0.5)      # moves to (20, 30) over 0.5 s; clock advances 0.5 s
r.color("green", dur=1)    # changes colour over 1 s; clock advances 1 s
r.alpha(0, dur=Frames(6))  # fades out over 6 frames; clock advances 6 frames
r.color("red")             # instant colour change; clock does not advance
```

`dur` is always keyword-only.

```ffpy video="mp4"
with Scene():
    r = Rect().size(80, 80).color("steelblue").align(0.5, 0.5)
    wait(1)
    r.color("tomato")           # instant — no dur
    wait(1)
    r.color("steelblue", dur=1) # animated — dur=1
```

The first change snaps instantly at t = 1 s; the second transitions smoothly over 1 s.

---

## The `ease` parameter

Alongside `dur`, every attribute method also accepts an optional `ease` keyword argument
that controls the **rate curve** of the transition — how the value's speed changes between
its start and end, rather than how long it takes. `ease` has no effect without `dur`, since
an instant change has no rate to shape.

Five presets are available, matching the standard CSS easing curves:

| `ease` | Description |
|---|---|
| `"linear"` (default) | Constant speed throughout |
| `"in"` | Starts slow, accelerates towards the end |
| `"out"` | Starts fast, decelerates towards the end |
| `"in_out"` | Slow start and end, faster in the middle |
| `"out_back"` | Overshoots past the target, then settles back |

```python
r.xy(220, 70, dur=0.8, ease="in_out")  # eased transition
r.color("gold", dur=0.8)               # dur alone ⇒ ease="linear"
```

```ffpy video="mp4"
with Scene(width=320, height=260):
    easings = ["linear", "in", "out", "in_out", "out_back"]
    rects = []
    for i, name in enumerate(easings):
        y = 20 + i * 48
        Text().xy(4, y).span(name).font(size=14).color("gray")
        r = Rect().size(18, 18).color("steelblue").xy(70, y - 2)
        rects.append(r)
    wait(0.3)
    with Par():
        for r, name in zip(rects, easings):
            r.x(280, dur=1.5, ease=name)
```

All five boxes travel the same distance over the same 1.5 s duration, started together in
a `Par()` block — the differing curves are what set them apart. Note how `"out_back"` briefly
overshoots the target before settling, while `"in"` lags behind at the start and catches up
at the end.

---

## The `.anim()` proxy

`node.anim(dur, ease=None)` returns a proxy that pre-fills `dur`/`ease` on every chained
setter call and runs them in parallel — a shorthand for animating several attributes of
one node at once without writing out a `Par()` block:

```python
# these two are equivalent:
g.anim(0.8).scale(1.9).xy(550, 400)

with Par():
    g.scale(1.9, dur=0.8)
    g.xy(550, 400, dur=0.8)
```

```ffpy video="mp4"
with Scene():
    with Group().size(80, 80).align(0.5, 0.5) as g:
        Rect().size(80, 80).color("orchid")
    g.anim(1).scale(1.4).rotate(45)
```

A `dur=`/`ease=` passed to one call in the chain still overrides the proxy's.

---

## The `anim()` block

`with anim(dur, ease=None):` sets the *default* `dur`/`ease` for any setter call inside
the block that doesn't specify its own. Plain top-level code is already sequential, so a
bare `anim()` block needs no extra `Seq()` wrapper:

```ffpy video="mp4"
with Scene():
    box = Rect().size(80, 60).color("steelblue").xy(20, 50)
    title = Rect().size(24, 24).color("gold").xy(230, 20)
    with anim(0.6):
        box.xy(180, 55)    # each step takes 0.6 s, one after another
        title.alpha(0)
```

`anim()` says nothing about composition, so it combines freely with `Par`/`Seq` — pair it
with `Par()` to animate several nodes' attributes at once instead of one after another:

```ffpy video="mp4"
with Scene():
    box = Rect().size(80, 60).color("steelblue").xy(20, 50)
    title = Rect().size(24, 24).color("gold").xy(230, 20)
    with Par(), anim(1.2):
        box.xy(180, 55)
        title.alpha(0)
```

The most specific duration wins: an explicit `dur=` on a call beats `.anim()`'s, which
beats the innermost enclosing `anim()` block.

---

## Sequential and parallel composition

Code at the top level is already **sequential** — each line follows the previous one.
`Seq` and `Par` are context managers that control how their contents are composed in time.

**`with Seq():`** — children run one after the other:

```ffpy video="mp4"
with Scene(width=300, height=140):
    with Seq():
        Rect().size(100, 80).xy(15, 30).color("steelblue").fade_out(dur=1)
        Rect().size(100, 80).xy(165, 30).color("tomato").fade_in(dur=1)
```

**`with Par():`** — all children start at the same time; the clock advances to the
longest child's end time:

```ffpy video="mp4"
with Scene(width=300, height=140):
    with Par():
        Rect().size(100, 80).xy(15, 30).color("steelblue").fade_out(dur=1)
        Rect().size(100, 80).xy(165, 30).color("tomato").fade_out(dur=1)
```

Creating objects before the composition block makes them all visible from the start:

```ffpy video="mp4"
with Scene(width=300, height=140):
    a = Rect().size(100, 80).xy(15, 30).color("steelblue")
    b = Rect().size(100, 80).xy(165, 30).color("tomato")

    with Seq():
        a.fade_out(dur=1)   # a fades first …
        b.fade_out(dur=1)   # … then b follows
```

`Par` and `Seq` compose freely. A typical pattern is a **sequence of parallel steps** —
top-level sequential code with `Par` blocks inside, each firing multiple animations at once:

```ffpy video="mp4"
with Scene():
    a = Rect().size(80, 60).color("steelblue").xy(20, 60)
    b = Rect().size(80, 60).color("coral").xy(120, 60)

    with Par():        # step 1: both fade in together
        a.fade_in(dur=0.5)
        b.fade_in(dur=0.5)

    wait(0.3)

    with Par():        # step 2: both animate simultaneously
        a.xy(220, 60, dur=0.8)
        b.color("gold", dur=0.8)
```

Going the other way, nesting `Seq` inside `Par` creates **staggered** parallel animations —
a common pattern for cascading effects:

```ffpy video="mp4"
with Scene():
    with Par():
        for i in range(5):
            with Seq():
                wait(0.15 * i)          # offset each rect
                Rect().size(40, 40).xy(20 + 60 * i, 60).color("orchid").fade_in(dur=0.4)
```

`Par(stagger=)` is shorthand for exactly this pattern — each direct child of the block
(a bare call, or a nested `Seq`/`Par`) starts `i × stagger` seconds later than the
previous one, without a manual `wait()` offset:

```ffpy video="mp4"
with Scene():
    with Par(stagger=0.15), anim(0.4):
        for i in range(5):
            Rect().size(40, 40).xy(20 + 60 * i, 60).color("orchid").fade_in()
```

`stagger` composes with an ambient `anim()` block default just like any other call, and
is rejected on `Seq` (a sequence's children never start together, so staggering their
start times has no meaning).

---

## Moving and transforming

Combine `dur` with `Par` to animate position, size, and colour simultaneously:

```ffpy video="mp4"
with Scene():
    r = Rect().size(60, 60).color("tomato").xy(20, 70)
    with Par():
        r.xy(220, 70, dur=1.5)
        r.color("steelblue", dur=1.5)
        r.size(80, 80, dur=1.5)
```

Use `wait()` to insert a pause before starting a transition:

```ffpy video="mp4"
with Scene():
    r = Rect().size(60, 60).color("gold").align(0.5, 0.5)
    wait(1)
    with Par():
        r.color("tomato", dur=1)
        r.size(120, 120, dur=1)
    wait(0.5)
    with Par():
        r.color("steelblue", dur=1)
        r.size(60, 60, dur=1)
```

---

## Rotation and scale

`Group` nodes support `.rotate()` and `.scale()` animations. The pivot defaults to the
group's centre — `pivot_x`/`pivot_y` hold an absolute pixel offset from the group's own
top-left, unset defaulting to `width * 0.5`/`height * 0.5`. Set it with `.pivot()`, e.g.
`g.pivot("top_left")` or `g.pivot(x=rel(0.3), y=rel(0.7))` for an own-box fraction.

```ffpy video="mp4"
with Scene():
    with Group().size(80, 80).align(0.5, 0.5) as g:
        Rect().size(80, 80).color("steelblue")
        Rect().size(20, 20).color("white").xy(30, 30)
    g.rotate(360, dur=2)
```

```ffpy video="mp4"
with Scene():
    with Group().size(80, 80).align(0.5, 0.5) as g:
        Ellipse().size(80, 80).color("coral")
    g.scale(0.2, dur=1)
    g.scale(1, dur=1)
```

---

## Fade in and fade out

`.fade_in(dur=t)` animates alpha from 0 to 1. `.fade_out(dur=t)` animates alpha from its
current value down to 0. Both advance the clock automatically. `dur` behaves like every
other setter's — instant if left unset (unless an enclosing `anim()` block supplies a
default) — and both accept an `ease=` too.

```ffpy video="mp4"
with Scene():
    r = Rect().size(120, 80).color("orchid").align(0.5, 0.5)
    r.fade_in(dur=0.6)
    wait(0.6)       # hold at full opacity
    r.fade_out(dur=0.6)
```

You can also set alpha directly with `dur`:

```ffpy video="mp4"
with Scene():
    r = Rect().size(120, 80).color("steelblue").align(0.5, 0.5)
    r.alpha(0)
    r.alpha(1, dur=1)   # fade in over 1 s
    wait(0.5)
    r.alpha(0, dur=1)   # fade out over 1 s
```

---

## Clipping animations

`Group` nodes have an animatable clipping window, set with `.clip(x=, y=, w=, h=)`.
`x`/`w` control the horizontal extent (as a fraction of the group width), and `y`/`h`
control the vertical extent. Any axis left out is untouched.

```ffpy video="mp4"
with Scene():
    with Group().size(200, 60) as g:
        Rect().size(200, 60).color("cornflowerblue")
        t = Text()
        t.span("Revealed!").font(size=22, bold=True).color("white")
        t.xy(40, 18)
    g.clip(w=0)               # start fully hidden
    g.clip(w=1, dur=1.2)      # reveal left-to-right over 1.2 s
```

`.hide(direction, dur=t)` and `.reveal(direction, dur=t)` are convenience helpers that
animate the clip to conceal or reveal the group content. `direction` is one of `"right"`
(default), `"left"`, `"up"`, `"down"` and refers to the sweep direction: `reveal("right")`
expands the clip window rightward, `reveal("left")` sweeps it leftward. Each call also
advances the clock automatically. Like `fade_in`/`fade_out`, `dur` is instant if left
unset, unless an enclosing `anim()` block supplies a default.

```ffpy video="mp4"
with Scene():
    with Group().size(200, 60).align(0.5, 0.5) as g:
        Rect().size(200, 60).color("cornflowerblue")
        t = Text()
        t.span("reveal / hide").font(size=14, bold=True).color("white")
        t.xy(18, 22)
    g.reveal("right", dur=1)   # expand clip from left to right
    wait(0.4)
    g.hide("left", dur=1)      # shrink clip from right to left
```

---

## Camera

Every `Group` and the `Scene` itself have an animatable `.camera` — a
viewpoint onto that node's **content**, distinct from `.scale()`/`.rotate()`
which transform the node as a widget:

```python
g.scale(2)          # the group grows as a widget — its box, layout, and
                     # hit-testing all move with it
g.camera.zoom(2)     # only the *content* magnifies — the group's own box
                     # stays exactly where layout put it
```

`.camera.zoom(factor)` magnifies content around the current camera center.
`.camera.center(x, y)` sets the content point the camera looks at — or pass a
live `Position` to track a moving target. `.camera.reset()` returns to
`zoom=1` centered on the group's own box.

```ffpy video="mp4"
with Scene():
    with Group().size(200, 140).align(0.5, 0.5) as g:
        Rect().size(200, 140).color("steelblue")
        Ellipse().size(24, 24).color("gold").xy(150, 30)
    with anim(1):
        g.camera.zoom(1.9)
        g.camera.center(160, 40)
    wait(0.4)
    g.camera.reset(dur=0.6)
```

`.camera.center()` also accepts a `Position`, tracking a moving node the same
way `.pivot()` does:

```ffpy video="mp4"
with Scene():
    with Group().size(200, 140).align(0.5, 0.5) as g:
        Rect().size(200, 140).color("steelblue")
        ball = Ellipse().size(20, 20).color("gold").xy(20, 20)
    g.camera.zoom(1.6)
    g.camera.center(ball.at("center"))
    ball.xy(160, 100, dur=1.5)
```

The camera does **not** auto-clip — zoomed content overflows the group's box
unless paired with `.clip()`:

```ffpy frame="0"
with Scene():
    with Group().size(140, 100).align(0.5, 0.5) as g:
        Rect().size(140, 100).color("steelblue")
        Ellipse().size(20, 20).color("gold").xy(60, 40)
    g.clip()          # crop overflow to the group's own (un-zoomed) box
    g.camera.zoom(2)
```
