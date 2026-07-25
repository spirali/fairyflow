---
icon: lucide/pause-circle
---

# Cues

## What is a cue?

A **cue** is a marker on the timeline where the player pauses and waits for user input
(e.g. a click or key press) before continuing. Cues let you build presentations where each
slide or step is a separate "chapter" in one continuous animation.

Call `cue()` at any point in your scene to mark the current frame as a cue point.
`cue()` also arms a lazy one-frame advance: the very next change — an instant attribute
set, a new node, `remove()`, or a transition — happens one frame later automatically, so
the cue point and the first frame of the following change are never the same frame. An
explicit `wait()` or `next_frame()` right after `cue()` disarms the lazy advance instead
of stacking with it, and calling `cue()` more than once on the same frame is harmless.

```python
with Scene():
    title = Text()
    title.span("Slide 1").font(size=32, bold=True).fill("steelblue")
    title.align(0.5, 0.5)
    cue()                          # player stops here

    # advance to the next state
    title.fill("gray", dur=0.4)
    title.font(size=24, dur=0.4)
    title.align(y=0.2, dur=0.4)
    wait(0.4)

    body = Text()
    body.span("Content appears here").font(size=18).fill("darkslateblue")
    body.align(0.5, 0.5)
    body.fade_in(dur=0.4)
    cue()                          # player stops here again
```

---

## Scene(flow=True)

By default the player pauses at the end of every scene, the same way it pauses at an
explicit cue — this keeps multi-scene decks from running past a scene boundary
unattended. Pass `flow=True` to opt a scene out, so playback runs straight into the next
scene instead of stopping:

```python
with Scene(flow=True):
    r = Rect().size(80, 80).fill("tomato").align(0.5, 0.5)
    r.fade_in(dur=1)
```

---

## Player behavior

In the interactive editor (opened with `fairyflow open`) the canvas auto-plays between cue
points and pauses at the end of every scene, unless that scene uses `flow=True`.

When you export to a **stand-alone player** (see [Exports](exports.md)), the same behavior is
preserved — the exported bundle plays and pauses at every cue point (and at every
non-`flow` scene boundary), making it suitable for use as a self-contained presentation.

---

## Speaker notes

Call `note(text)` to attach a speaker note to the current **segment** — the span from
the previous cue (or scene start) up to the next cue (or scene end). Notes don't move
the clock, and several `note()` calls in one segment stack as paragraphs:

```python
with Scene() as s:
    note("Introduce the problem first.")   # first segment — no cue needed
    ...
    cue()
    note("Now the punchline.")
```

Notes never appear in the rendered animation itself — they're for presenter view only:

- In the **sequence player** (web), toggle the notes panel from the header button next
  to fullscreen. It shows the current segment's notes as you scrub or play through.
- In the **stand-alone player** (`fairyflow play`), press `N` to toggle a notes overlay
  at the bottom of the window. Pass `--twin-view` to open two windows instead — a clean
  one for a projector, and a presenter window with notes on by default (either window's
  notes can still be toggled independently with `N`). See [Exports](exports.md).

---

## Design patterns

### Cue + transition

Combine cues with smooth transitions so content doesn't just pop in:

```python
with Scene():
    r = Rect().size(100, 100).fill("steelblue").align(0.5, 0.5)
    cue()

    r.fill("tomato", dur=0.5)
    r.size(60, 60, dur=0.5)
    wait(0.5)
    cue()

    r.fill("gold", dur=0.5)
    r.size(140, 140, dur=0.5)
    wait(0.5)
```
