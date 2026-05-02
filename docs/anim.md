---
icon: lucide/film
---

# Animations

## Timeline and time functions

FairyFlow animations are built by advancing a global **current frame** and setting attribute
values at specific frames. Five functions control the timeline:

| Function | Description |
|---|---|
| `adv_time(seconds)` | Advance time by the given number of seconds |
| `set_time(seconds)` | Jump the timeline to an absolute time |
| `adv_frames(n)` | Advance by an integer number of frames |
| `set_frame(n)` | Jump to a specific frame number |

The default frame rate is **24 fps**. This can be changed in `fairyflow.toml`.

---

## step vs linear

Two **transition modes** control how attribute values change between keyframes:

- `step()` (default) — values change **instantaneously** at the target frame
- `linear()` — values **interpolate linearly** between the previous and next keyframe

Call these functions at any point to switch mode for subsequent attribute changes.

```ffpy video="mp4"
with Scene(width=300, height=140):
    # left rect: step (instant color change at 1.5 s)
    a = Rect().size(120, 80).xy(15, 30).color("steelblue")
    step()
    adv_time(1.5)
    a.color("tomato")

    # right rect: linear (smooth color transition over 1.5 s)
    set_time(0.0)
    b = Rect().size(120, 80).xy(165, 30).color("steelblue")
    linear()
    adv_time(1.5)
    b.color("tomato")
```

The left rect keeps its color and then changes instantly; the right rect transitions smoothly.

---

## Moving and transforming

Combine `linear()` and `adv_time()` to animate position, size, color, or any other attribute:

```ffpy video="mp4"
with Scene():
    r = Rect().size(60, 60).color("tomato").xy(20, 70)
    linear()
    adv_time(1.5)
    r.xy(220, 70).color("steelblue").size(80, 80)
```

Use `.hold()` to freeze all current attribute values before starting a new segment:

```ffpy video="mp4"
with Scene():
    r = Rect().size(60, 60).color("gold").align_x(0.5).align_y(0.5)
    adv_time(1)    
    r.hold()    
    linear()
    adv_time(1)
    r.color("tomato").size(120, 120)
    adv_time(0.5)
    r.hold()
    linear()
    adv_time(1)
    r.color("steelblue").size(60, 60)
```


---

## Rotation and scale

`Group` nodes support `.rotate()` and `.scale()` animations. The pivot defaults to the
group's center (`pivot_x=0.5`, `pivot_y=0.5`).

```ffpy video="mp4"
with Scene():
    with Group().size(80, 80).align_x(0.5).align_y(0.5) as g:
        Rect().size(80, 80).color("steelblue")
        Rect().size(20, 20).color("white").xy(30, 30)
    g.hold()
    linear()
    adv_time(2)
    g.rotate(360)
```

```ffpy video="mp4"
with Scene():
    with Group().size(80, 80).align_x(0.5).align_y(0.5) as g:
        Ellipse().size(80, 80).color("coral")
    linear()
    adv_time(1)
    g.scale(0.2)
    linear()
    adv_time(1)
    g.scale(1)
```

---

## Fade in and fade out

`.fade_in(time)` animates alpha from 0 to 1. `.fade_out(time)` animates alpha from its current
value down to 0. Both advance the timeline automatically.

```ffpy video="mp4"
with Scene():
    r = Rect().size(120, 80).color("orchid").align_x(0.5).align_y(0.5)
    r.fade_in(0.6)
    adv_time(0.6)    # hold at full opacity
    r.fade_out(0.6)
```

You can also set alpha directly:

```ffpy video="mp4"
with Scene():
    r = Rect().size(120, 80).color("steelblue").align_x(0.5).align_y(0.5)
    r.alpha(0)
    linear()
    adv_time(1)
    r.alpha(1)
    adv_time(0.5)
    r.hold()    
    linear()
    adv_time(1)
    r.alpha(0)
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
    g.clip_w(0)           # start fully hidden
    linear()
    adv_time(1.2)
    g.clip_w(1)           # reveal left-to-right
```

`.hide_right(time)`, `.hide_left(time)`, `.reveal_right(time)`, and `.reveal_left(time)` are
convenience helpers that animate the clip to conceal or reveal the group content. The direction
refers to the sweep direction: `reveal_right` expands the clip window rightward, `reveal_left`
sweeps it leftward. Each call also advances the timeline automatically.

```ffpy video="mp4"
with Scene():
    with Group().size(200, 60).align_x(0.5).align_y(0.5) as g:
        Rect().size(200, 60).color("cornflowerblue")
        t = Text()
        t.span("reveal_right / hide_left").font_size(14).bold().color("white")
        t.xy(18, 22)
    g.reveal_right(1)   # expand clip from left to right
    adv_time(0.4)
    g.hide_left(1)      # shrink clip from right to left
```

---

## Build State

`bstate()` captures a snapshot of the current time and transition style. Using it as a context
manager temporarily applies that snapshot — changes inside the block do not affect the outer
timeline, so the clock is restored when the block exits. This is useful for animating multiple
elements in parallel from the same starting point.

```ffpy video="mp4"
with Scene():
    a = Rect().size(80, 80).color("steelblue").xy(20, 60)
    b = Rect().size(80, 80).color("tomato").xy(200, 60)

    with bstate():       # snapshot time = 0
        linear()
        adv_time(1.5)
        a.xy(110, 60)    # a slides right over 1.5 s

    # time is restored to 0 — b animates in parallel
    linear()
    adv_time(1)
    b.color("gold")      # b changes color over 1 s
```