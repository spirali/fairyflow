---
icon: lucide/play-circle
---

# Examples

## Sieve of Eratosthenes

The [Sieve of Eratosthenes](https://en.wikipedia.org/wiki/Sieve_of_Eratosthenes) is a classic
algorithm for finding all prime numbers up to a given limit. Starting from 2, each prime's
multiples are crossed out repeatedly until only primes remain.

The animation shows numbers 1–100 on a grid. Each discovered prime is highlighted green; its
multiples are marked red and then hidden, leaving only the primes at the end.


```ffpy video="mp4" position="top"
with Scene(1280, 720):
    with Group().column(40).align(y=0.6):
        with Group() as g2:
            t = Text("Sieve of Eratosthenes").font(size=40, bold=True)
        with Group() as g3:
            Image("docs/ff_logo.png").height(200)
        wait(0.2)
        g2.fade_out(dur=0.5)
        g3.hide(dur=0.5)
        wait(0.2)

with Scene(1280, 720) as s:
    numbers = []
    with Group().grid(cols=20, gap=10) as g:
        with Par(stagger=0.01), anim(0.3):
            for i in range(0, 100):
                with Group() as n:
                    Rect().stroke("black").fill("#ccc").size(40, 40)
                    Text(str(i + 1))
                    n.fade_in()
                numbers.append(n)
        wait(0.2)

        arrow = Arrow((-350, -10), (-350, -50), head="start").stroke("green", 4)

        for step in [2, 3]:
            idx = step - 1
            with Par(), anim(0.8):
                s.camera.zoom(1.9)
                s.camera.center(numbers[idx].at("center").move(120, 60))
            if step == 2:
                numbers[0].fade_out(dur=0.5)
                wait(0.5)

            wait(0.5)
            with Par(), anim(0.5):
                arrow.start.pos(numbers[idx].at("top").move(-5, -5))
                arrow.end.pos(numbers[idx].at("top").move(-5, -40))

            r = numbers[idx].get_child(kind="rect")
            wait(0.3)
            r.fill("green", dur=0.4)
            wait(0.2)

            with Group() as m:
                m.pos(numbers[idx].at("top"))
                p = Path().stroke("red", 2)
                a = p.move_to(0, 0)
                b = p.line_to(0, 0)
                p.move_to(a.at().move(0, -4))
                p.line_to(a.at().move(0, 4))
                p.move_to(b.at().move(0, -4))
                p.line_to(b.at().move(0, 4))
                t = Text(str(step)).fill("red")
                t.pos(numbers[idx].get_child(kind="text").at(0, 0))
                m.fade_in(dur=0.5)

            for i in range(3):
                with Par(), anim(0.5):
                    a.pos(numbers[idx + i * step].at("top").move(0, -6))
                    b.pos(numbers[idx + (i + 1) * step].at("top").move(0, -6))
                    t.pos(numbers[idx + i * step].at("top_left").move(70, -30))
                wait(0.2)
                r = numbers[idx + (i + 1) * step].get_child(kind="rect")
                r.fill("red", dur=0.5)
                wait(0.5)

            with Par(), anim(0.5):
                m.alpha(0)
                s.camera.reset()

            wait(0.5)

            with Par(stagger=0.02 * step), anim(0.3):
                for i in range(idx + (i * step), 100, step):
                    numbers[i].get_child(kind="rect").fill("red")

        for step in [5, 7, 11]:
            idx = step - 1
            wait(0.3)
            with Par(), anim(0.5):
                arrow.start.pos(numbers[idx].at("top").move(-5, -5))
                arrow.end.pos(numbers[idx].at("top").move(-5, -40))
            wait(0.2)
            r = numbers[idx].get_child(kind="rect")
            r.fill("green", dur=0.4)
            wait(0.2)

            if step == 11:
                break

            with Par(stagger=0.01 * step), anim(0.3):
                for i in range(idx + 3 * step, 100, step):
                    numbers[i].get_child(kind="rect").fill("red")

        arrow.alpha(0, dur=0.3)

        PRIMES = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97]
        with Par(stagger=0.02), anim(0.3):
            for p in PRIMES[5:]:
                numbers[p - 1].get_child(kind="rect").fill("green")

        wait(0.5)
        with Par(), anim(0.5):
            for i in range(1, 100):
                if (i + 1) in PRIMES:
                    continue
                numbers[i].alpha(0)

        with Par(), anim(0.5):
            for i, p in enumerate(PRIMES):
                idx = p - 1
                numbers[idx].xy(50 * (i % 20), (i // 20) * 50 + 300)

        wait(0.5)

    with Group().column(40).align(y=0.2) as g:
        with Group() as g2:
            Text("Sieve of Eratosthenes").font(size=40, bold=True)
        with Group() as g3:
            Image("docs/ff_logo.png").height(200)
        g.fade_in(dur=1)
        wait(1)
```

---

## Walk-through

The sections below explain how the animation is built piece by piece.

### Multiple scenes

The animation uses two `Scene` objects. A `Scene` is the top-level canvas; it defines
the resolution and background. Running multiple `Scene` calls in sequence produces a
multi-scene video where the player transitions from one scene to the next automatically.

```python
with Scene(1280, 720):
    ...  # intro

with Scene(1280, 720) as s:
    ...  # main sieve + outro
```

The main scene is captured as `s` — `with Scene(...) as s:` returns the scene itself,
and `s` is needed later to reach `s.camera` (see "Zooming in", below).

### The intro screen

```python
with Scene(1280, 720):
    with Group().column(40).align(y=0.6):
        with Group() as g2:
            Text("Sieve of Eratosthenes").font(size=40, bold=True)
        with Group() as g3:
            Image("docs/ff_logo.png").height(200)
        wait(0.2)
        g2.fade_out(dur=0.5)
        g3.hide(dur=0.5)
        wait(0.2)
```

`Group().column(40)` creates a vertical layout that stacks its children with 40 px of spacing
between them. `.align(y=0.6)` positions the group 60% of the way down the canvas — just below
center.

The `with Group() as g2:` pattern is the core FairyFlow idiom: every node created *inside* the
`with` block becomes a child of that group, and the variable `g2` is a handle you can use to
animate the group as a whole afterward.

Once the children are placed, `wait(0.2)` moves the global clock forward 0.2 seconds, creating
a brief pause where the title and logo are fully visible. `.fade_out(dur=0.5)` and `.hide(dur=0.5)`
then animate the two groups away. Both helpers advance the clock automatically, so the outro
takes 0.2 + 0.5 + 0.5 + 0.2 = 1.4 seconds in total. (Note that `fade_out` and `hide`
run sequentially here — use `Par` to run them simultaneously.)

### Building the number grid

```python
numbers = []
with Group().grid(cols=20, gap=10) as g:
    with Par(stagger=0.01), anim(0.3):
        for i in range(0, 100):
            with Group() as n:
                Rect().stroke("black").fill("#ccc").size(40, 40)
                Text(str(i + 1))
                n.fade_in()
            numbers.append(n)
    wait(0.2)
```

`Group().grid(cols=20, gap=10)` lays out its children automatically in a 20-column grid with
10 px of spacing, sizing every column/row to fit the widest/tallest cell — no manual `.xy()`
arithmetic needed. Each cell is a `Group` containing a grey 40 × 40 `Rect` and a `Text` label
built directly from the constructor (`Text(str(i + 1))`). The `numbers` list keeps a reference
to every cell so the sieve loop can look up any cell by its 0-based index later.

### Staggered fade-in with `Par(stagger=)`

```python
with Par(stagger=0.01), anim(0.3):
    for i in range(0, 100):
        ...
        n.fade_in()
```

`Par(stagger=0.01)` starts all 100 cells at the same time, but offsets each one's start by
`i × 0.01` seconds — cell 0 starts immediately, cell 99 starts 0.99 seconds later — creating
the same cascade effect as manually nesting `Seq(): wait(0.01 * i); ...` inside a `Par()`,
without writing the offset by hand. The enclosing `anim(0.3)` block supplies `dur=0.3` to the
bare `n.fade_in()` call, so no per-call `dur=` is needed either. The `Par` block advances the
outer clock to the longest child's end time (≈ 0.99 + 0.3 = 1.3 s), followed by a short
`wait(0.2)` pause.

### The arrow indicator

```python
arrow = Arrow((-350, -10), (-350, -50), head="start").stroke("green", 4)
```

`Arrow(start, end, head=)` builds a two-point connector with an arrowhead in one call —
`head="start"` puts the head at the first point. `.start`/`.end` expose the underlying
`move_to`/`line_to` handles, so the endpoints animate independently later.

The arrow begins at x = −350 — off the left edge of the canvas so it is invisible at first. It
will be repositioned later by animating `arrow.start` and `arrow.end` to the coordinates of the
target cell.

### Zooming in and pointing to a prime

```python
for step in [2, 3]:
    idx = step - 1

    with Par(), anim(0.8):
        s.camera.zoom(1.9)
        s.camera.center(numbers[idx].at("center").move(120, 60))
```

`s.camera` is the scene's own animatable viewpoint onto its content — distinct from
`.scale()`, which would transform a node's own box as a widget. `.camera.zoom(1.9)` magnifies
the content around the current camera center; `.camera.center(...)` re-points the camera at
the target cell, offset by `.move(120, 60)` from its exact center — this keeps the framing from
being dead-centered on the single cell, leaving room in the zoomed view for the crossing-line
bracket and the multiples it will step across. Running both inside `Par(), anim(0.8)` animates
zoom and re-centering together over 0.8 seconds — the grid zooms in and shifts in one fluid
move, and the scene's own layout is untouched (see [Camera](anim.md#camera) for the full
zoom/center/reset story).

```python
    with Par(), anim(0.5):
        arrow.start.pos(numbers[idx].at("top").move(-5, -5))
        arrow.end.pos(numbers[idx].at("top").move(-5, -40))
```

`.at("top")` returns the top-center point of a cell as a `Position` object (see
[Positioning](positioning.md#cross-group-positioning-at-and-pos) for the full list of named
anchors). Passing that `Position` to `.pos()` animates the arrow endpoint to the cell's top
edge. The `.move(-5, -40)` call applies a small relative offset, nudging the arrowhead above
the cell.

```python
    r = numbers[idx].get_child(kind="rect")
    r.fill("green", dur=0.4)
```

`get_child(kind="rect")` searches the cell's children for a `Rect` node and returns it.
`.fill("green", dur=0.4)` creates a smooth colour transition from grey to green over 0.4 seconds.

### The crossing-line animation

```python
with Group() as m:
    m.pos(numbers[idx].at("top"))
    p = Path().stroke("red", 2)
    a = p.move_to(0, 0)
    b = p.line_to(0, 0)
    p.move_to(a.at().move(0, -4))
    p.line_to(a.at().move(0, 4))
    p.move_to(b.at().move(0, -4))
    p.line_to(b.at().move(0, 4))
    t = Text(str(step)).fill("red")
    t.pos(numbers[idx].get_child(kind="text").at(0, 0))
    m.fade_in(dur=0.5)
```

This builds a red bracket: a horizontal line from `a` to `b` with a short vertical tick at each
end. Unlike the simple two-point arrow above, this shape needs two ticks plus a main line — not
a case `Arrow`/`Line` covers directly — so it's still built command-by-command from `Path`. The
path has 6 commands in total:

1. The main line: `move_to(0, 0)` → `a`, `line_to(0, 0)` → `b`
2. Left tick: `move_to(a.at().move(0, -4))`, `line_to(a.at().move(0, 4))`
3. Right tick: same pattern at `b.at()`

The key insight is that the tick endpoints are defined *relative to `a` and `b`* using
`move_to(a.at().move(0, ±4))`. When `a` and `b` are animated to new positions the ticks move
with them automatically — you never have to update them separately.

The text label `t` sits next to the starting cell and shows the prime value in red. `.at(0, 0)`
anchors on the target text node's top-left corner rather than its center (a bare `.at()` — used
below — defaults to the center; `(0, 0)` is the same point as the named anchor `"top_left"`).

```python
for i in range(3):
    with Par(), anim(0.5):
        a.pos(numbers[idx + i * step].at("top").move(0, -6))
        b.pos(numbers[idx + (i + 1) * step].at("top").move(0, -6))
        t.pos(numbers[idx + i * step].at("top_left").move(70, -30))
    wait(0.2)
    r = numbers[idx + (i + 1) * step].get_child(kind="rect")
    r.fill("red", dur=0.5)
    wait(0.5)
```

Each iteration advances the bracket one step: `a` and `b` slide to the next multiple in
parallel (`Par(), anim(0.5)` supplies the shared 0.5 s duration to all three calls), and then
the target cell's rect transitions to red. The bracket hops across the first three multiples
of the prime with colour changes in sync. `t`'s offset is now taken from the cell's top-left
corner (`.at("top_left")`) instead of its center, so `.move(70, -30)` lands the label in the
same place relative to the cell's corner on every hop.

### Marking all remaining multiples

```python
with Par(stagger=0.02 * step), anim(0.3):
    for i in range(idx + (i * step), 100, step):
        numbers[i].get_child(kind="rect").fill("red")
```

After the animated demonstration the rest of the multiples are coloured red in a rapid sweep.
`Par(stagger=0.02 * step)` starts each cell's transition `0.02 * step` seconds after the
previous one, and `anim(0.3)` supplies the 0.3 s fill duration — the same staggered-parallel
pattern used for the initial cascade fade-in, without a manual `wait()`/`Seq()` per cell.

### Quick pass for 5, 7, 11

```python
for step in [5, 7, 11]:
    idx = step - 1
    # move arrow to the prime, highlight it green
    ...
    with Par(stagger=0.01 * step), anim(0.3):
        for i in range(idx + 3 * step, 100, step):
            numbers[i].get_child(kind="rect").fill("red")
```

Primes 5, 7, and 11 get a simpler treatment: the arrow moves to each prime and its background
turns green, but there is no zoom-in or crossing-line animation. The multiples loop starts at
`idx + 3 * step` because all smaller multiples of these primes were already marked red during
the 2 and 3 passes.

### Revealing the primes

```python
wait(0.5)
with Par(), anim(0.5):
    for i in range(1, 100):
        if (i + 1) in PRIMES:
            continue
        numbers[i].alpha(0)
```

All non-prime cells fade out simultaneously — `anim(0.5)` supplies `dur=0.5` to every bare
`alpha(0)` call, and since they are all inside `Par` they all start at the same moment.

```python
with Par(), anim(0.5):
    for i, p in enumerate(PRIMES):
        idx = p - 1
        numbers[idx].xy(50 * (i % 20), (i // 20) * 50 + 300)
```

The surviving primes are then repositioned together. Each `.xy()` call picks up the same
0.5 s duration from the enclosing `anim()` block, so all cells slide smoothly to their new
positions simultaneously inside `Par`. The new coordinates use the same column/row formula but
shifted 300 px down to keep them within the canvas.

### The outro

```python
with Group().column(40).align(y=0.2) as g:
    with Group() as g2:
        Text("Sieve of Eratosthenes").font(size=40, bold=True)
    with Group() as g3:
        Image("docs/ff_logo.png").height(200)
    g.fade_in(dur=1)
    wait(1)
```

This outro lives in the *same* `Scene` as the grid — it is a sibling group placed at
`align(y=0.2)` (top fifth of the canvas). `g.fade_in(dur=1)` is called on the outer column group
rather than on `g2` and `g3` individually, so the title and logo fade in together as one unit.
`wait(1)` extends the scene for one more second so the final frame is not cut off abruptly.
