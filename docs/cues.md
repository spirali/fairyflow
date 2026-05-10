---
icon: lucide/pause-circle
---

# Cues

## What is a cue?

A **cue** is a marker on the timeline where the player pauses and waits for user input
(e.g. a click or key press) before continuing. Cues let you build presentations where each
slide or step is a separate "chapter" in one continuous animation.

Call `cue()` at any point in your scene to mark the current frame as a cue point:

```python
with Scene():
    title = Text()
    title.span("Slide 1").font_size(32).bold().color("steelblue")
    title.align_x(0.5).align_y(0.5)
    cue()                          # player stops here

    # advance to the next state
    linear()
    adv_time(0.4)
    title.color("gray").font_size(24).align_y(0.2)

    body = Text()
    body.span("Content appears here").font_size(18).color("darkslateblue")
    body.align_x(0.5).align_y(0.5)
    body.fade_in(0.4)
    cue()                          # player stops here again
```

---

## cue_at_start

By default every scene adds a cue at frame 0 (`cue_at_start=True`). This means the player
waits for input before the animation begins. Pass `cue_at_start=False` to start playing
immediately:

```python
with Scene(cue_at_start=False):
    r = Rect().size(80, 80).color("tomato").align_x(0.5).align_y(0.5)
    r.fade_in(1)
```

---

## Player behavior

In the interactive editor (opened with `fairyflow open`) the canvas auto-plays between cue
points.

When you export to a **stand-alone player** (see [Exports](exports.md)), the same behavior is
preserved — the exported bundle plays and pauses at every cue point, making it suitable for
use as a self-contained presentation.

---

## Design patterns

### Cue + transition

Combine cues with smooth transitions so content doesn't just pop in:

```python
with Scene():
    r = Rect().size(100, 100).color("steelblue").align_x(0.5).align_y(0.5)
    cue()

    linear()
    adv_time(0.5)
    r.color("tomato").size(60, 60)
    cue()

    linear()
    adv_time(0.5)
    r.color("gold").size(140, 140)
```
