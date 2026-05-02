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
    with Group().column(40).align_y(0.6):
        with Group() as g2:
            Text().span("Sieve of Eratosthenes").font_size(40).bold()
        with Group() as g3:
            Image("docs/ff_logo.png").height(200)
        adv_time(0.2)
        g2.fade_out(0.5)
        g3.hide_right(0.5)
        adv_time(0.2)

with Scene(1280, 720):
    numbers = []
    with Group().size(1000, 400) as g:
        # Build a 20-column × 5-row grid of number cells, initially invisible
        for i in range(0, 100):
            with Group().xy(50 * (i % 20), (i // 20) * 50) as n:
                n.alpha(0)
                Rect().stroke_color("black").color("#ccc").size(40, 40)
                Text().span(str(i + 1))
            with bstate():
                adv_time(0.01 * i)
                n.fade_in(0.3)
            numbers.append(n)
        adv_time(2.2)

        # Arrow indicator that points to the current prime
        arrow = Path().stroke_color("green").stroke_width(4)
        arrow_start = arrow.move_to().xy(-150, -10)
        arrow_end = arrow.line_to().xy(-150, -50)
        arrow_head = arrow.triangle_arrow("start")

        # Full zoom-in pass for 2 and 3: show the crossing-line animation
        for step in [2, 3]:
            idx = step - 1
            g.hold()
            adv_time(0.8)
            linear()
            g.scale(1.90)
            g.xy(550, 400)
            if step == 2:
                numbers[0].fade_out(0.5)   # 1 is not prime
                adv_time(0.5)

            arrow.hold()
            adv_time(0.5)
            arrow_start.pos(numbers[idx].get_pos(0.5)).move(-5, -5)
            arrow_end.pos(numbers[idx].get_pos(0.5)).move(-5, -40)

            r = numbers[idx].get_child(kind="rect")
            adv_time(0.3)
            r.hold()
            adv_time(0.4)
            r.color("green")
            adv_time(0.2)

            # Crossing line sweeps from the prime to its first few multiples
            with Group() as m:
                m.pos(numbers[idx].get_pos(0.5))
                p = Path().stroke_color("red").stroke_width(2)
                a = p.move_to()
                b = p.line_to()
                p.move_to().pos(a.get_pos()).move(0, -4)
                p.line_to().pos(a.get_pos()).move(0, 4)
                p.move_to().pos(b.get_pos()).move(0, -4)
                p.line_to().pos(b.get_pos()).move(0, 4)
                t = Text().pos(numbers[idx].get_child(kind="text").get_pos())
                t.span(str(step)).color("red")
                m.fade_in(0.5)

            for i in range(3):
                m.hold()
                adv_time(0.5)
                a.pos(numbers[idx + i * step].get_pos(0.5)).move(0, -6)
                b.pos(numbers[idx + (i + 1) * step].get_pos(0.5)).move(0, -6)
                t.pos(numbers[idx + i * step].get_pos()).move(70, -30)
                r = numbers[idx + (i + 1) * step].get_child(kind="rect")
                r.hold()
                adv_time(0.5)
                r.color("red")
                adv_time(0.5)

            m.hold()
            g.hold()
            adv_time(0.5)
            g.scale(1)
            g.xy_reset()
            m.alpha(0)
            adv_time(0.5)

            # Mark remaining multiples red (quickly, no zoom)
            for i in range(idx + (i * step), 100, step):
                adv_time(0.05)
                numbers[i].hold()
                with bstate():
                    adv_time(0.3)
                    numbers[i].get_child(kind="rect").color("red")

        # Quick pass for 5, 7, 11: move arrow and mark composites
        for step in [5, 7, 11]:
            idx = step - 1
            adv_time(0.3)
            arrow.hold()
            adv_time(0.5)
            arrow_start.pos(numbers[idx].get_pos(0.5)).move(-5, -5)
            arrow_end.pos(numbers[idx].get_pos(0.5)).move(-5, -40)
            adv_time(0.2)
            r = numbers[idx].get_child(kind="rect")
            r.hold()
            adv_time(0.4)
            r.color("green")
            adv_time(0.2)

            for i in range(idx + 3 * step, 100, step):
                adv_time(0.05)
                numbers[i].hold()
                with bstate():
                    adv_time(0.3)
                    numbers[i].get_child(kind="rect").color("red")

        arrow.hold()
        arrow_head.hold()
        adv_time(0.3)
        arrow.alpha(0)
        arrow_head.alpha(0)

        # Mark all remaining primes (13 and above) green
        PRIMES = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47,
                  53, 59, 61, 67, 71, 73, 79, 83, 89, 97]
        for p in PRIMES[5:]:
            idx = p - 1
            adv_time(0.05)
            numbers[idx].hold()
            with bstate():
                adv_time(0.3)
                numbers[idx].get_child(kind="rect").color("green")

        # Hide all composites, leaving only primes visible
        adv_time(0.5)
        for i in range(1, 100):
            numbers[i].hold()
        adv_time(0.6)
        for i in range(1, 100):
            if (i + 1) in PRIMES:
                continue
            numbers[i].alpha(0)
        adv_time(0.5)

        # Rearrange primes into a compact grid
        for i in range(1, 100):
            numbers[i].hold()
        adv_time(0.5)
        for i, p in enumerate(PRIMES):
            idx = p - 1
            numbers[idx].xy(50 * (i % 20), (i // 20) * 50 + 300)
        adv_time(0.5)

    # Outro: title and logo fade back in
    with Group().column(40).align_y(0.2) as g:
        with Group() as g2:
            Text().span("Sieve of Eratosthenes").font_size(40).bold()
        with Group() as g3:
            Image("docs/ff_logo.png").height(200)
        g.fade_in()
        adv_time(1.0)
```

---

## Walk-through

The sections below explain how the animation is built piece by piece. If you are new to
FairyFlow, the key idea is simple: you create nodes, then advance a global clock with
`adv_time()`, then set new attribute values — FairyFlow records those as keyframes and
interpolates between them automatically.

### Multiple scenes

The animation uses two `Scene` objects. A `Scene` is the top-level canvas; it defines
the resolution and background. Running multiple `Scene` calls in sequence produces a
multi-scene video where the player transitions from one scene to the next automatically.

```python
with Scene(1280, 720):
    ...   # intro

with Scene(1280, 720):
    ...   # main sieve + outro
```

### The intro screen

```python
with Scene(1280, 720):
    with Group().column(40).align_y(0.6):
        with Group() as g2:
            Text().span("Sieve of Eratosthenes").font_size(40).bold()
        with Group() as g3:
            Image("docs/ff_logo.png").height(200)
        adv_time(0.2)
        g2.fade_out(0.5)
        g3.hide_right(0.5)
        adv_time(0.2)
```

`Group().column(40)` creates a vertical layout that stacks its children with 40 px of spacing
between them. `.align_y(0.6)` positions the group 60% of the way down the canvas — just below
center.

The `with Group() as g2:` pattern is the core FairyFlow idiom: every node created *inside* the
`with` block becomes a child of that group, and the variable `g2` is a handle you can use to
animate the group as a whole afterward.

Once the children are placed, `adv_time(0.2)` moves the global clock forward 0.2 seconds,
creating a brief pause where the title and logo are fully visible. `.fade_out(0.5)` and
`.hide_right(0.5)` then animate the two groups away. Both helpers advance the clock
automatically by their duration, so the outro takes 0.2 + 0.5 + 0.5 + 0.2 = 1.4 seconds
in total.

### Building the number grid

```python
numbers = []
with Group().size(1000, 400) as g:
    for i in range(0, 100):
        with Group().xy(50 * (i % 20), (i // 20) * 50) as n:
            n.alpha(0)
            Rect().stroke_color("black").color("#ccc").size(40, 40)
            Text().span(str(i + 1))
        numbers.append(n)
```

The outer `Group` with an explicit `size(1000, 400)` acts as the stage for the whole sieve
animation. Its 100 children are positioned manually with `.xy()` using simple integer arithmetic:

- **x** = `50 * (i % 20)` — 20 columns spaced 50 px apart (0, 50, 100 … 950)
- **y** = `(i // 20) * 50` — a new row every 20 numbers (0, 50, 100, 150, 200)

This gives a 20-column × 5-row grid. Each cell is a `Group` containing a grey 40 × 40 `Rect`
and a `Text` label. All cells start invisible (`n.alpha(0)`) so they can be faded in with a
stagger. The `numbers` list keeps a reference to every cell so the sieve loop can look up any
cell by its 0-based index later.

### Staggered fade-in with `bstate()`

```python
for i in range(0, 100):
    with Group().xy(...) as n:
        n.alpha(0)
        ...
    with bstate():
        adv_time(0.01 * i)
        n.fade_in(0.3)
    numbers.append(n)
adv_time(2.2)
```

`bstate()` captures a *snapshot* of the current build state — the current time and transition
mode. Using it as a context manager means any clock changes inside the block are discarded when
the block exits; the outer clock is unaffected.

This is the standard FairyFlow pattern for **parallel animations starting at different offsets**.
The outer loop runs at a fixed time (e.g. t = 0). For each cell the snapshot is taken at t = 0,
the clock is advanced inside the block to `0.01 * i` seconds, `.fade_in(0.3)` schedules a
0.3-second fade starting there, and then the snapshot is discarded and the outer clock returns
to 0. Cell 0 starts fading immediately; cell 99 starts 0.99 seconds later — a cascade effect.

After the loop, `adv_time(2.2)` advances the outer clock past the end of all the fades
(last fade finishes at ≈ 0.99 + 0.3 = 1.3 s) plus a short pause.

### The arrow indicator

```python
arrow = Path().stroke_color("green").stroke_width(4)
arrow_start = arrow.move_to().xy(-150, -10)
arrow_end   = arrow.line_to().xy(-150, -50)
arrow_head  = arrow.triangle_arrow("start")
```

`Path` builds a vector path from a sequence of commands. Each command (`move_to()`, `line_to()`)
returns a node whose position can be animated independently. `triangle_arrow("start")` attaches
a filled arrowhead at the start endpoint.

The arrow begins at x = −150 — off the left edge of the canvas so it is invisible at first. It
will be repositioned later by animating `arrow_start` and `arrow_end` to the coordinates of the
target cell.

### Zooming in and pointing to a prime

```python
for step in [2, 3]:
    idx = step - 1

    g.hold()
    adv_time(0.8)
    linear()
    g.scale(1.90)
    g.xy(550, 400)
```

`.hold()` locks all of the group's current attribute values as a keyframe at the current frame.
This is necessary before a `linear()` transition: the engine needs a "from" keyframe to
interpolate from. Switching to `linear()` and then setting `.scale(1.90)` and `.xy(550, 400)`
creates a smooth animated zoom-in over 0.8 seconds.

```python
    arrow.hold()
    adv_time(0.5)
    arrow_start.pos(numbers[idx].get_pos(0.5)).move(-5, -5)
    arrow_end.pos(numbers[idx].get_pos(0.5)).move(-5, -40)
```

`get_pos(0.5)` returns the center of a cell as a `Position` object (the argument `0.5` is the
horizontal alignment — 0.0 is the left edge, 1.0 is the right edge, 0.5 is the center). Passing
that `Position` to `.pos()` on a path command animates the endpoint to the cell's center. The
`.move(-5, -40)` call applies a small relative offset on top, nudging the arrowhead upward so
it sits above the cell rather than on top of it.

```python
    r = numbers[idx].get_child(kind="rect")
    r.hold()
    adv_time(0.4)
    r.color("green")
```

`get_child(kind="rect")` searches the cell's children for a `Rect` node and returns it. Holding
it and then setting `.color("green")` with `linear()` still active creates a smooth colour
transition from grey to green over 0.4 seconds.

### The crossing-line animation

```python
with Group() as m:
    m.pos(numbers[idx].get_pos(0.5))
    p = Path().stroke_color("red").stroke_width(2)
    a = p.move_to()
    b = p.line_to()
    p.move_to().pos(a.get_pos()).move(0, -4)
    p.line_to().pos(a.get_pos()).move(0, 4)
    p.move_to().pos(b.get_pos()).move(0, -4)
    p.line_to().pos(b.get_pos()).move(0, 4)
    t = Text().pos(numbers[idx].get_child(kind="text").get_pos())
    t.span(str(step)).color("red")
    m.fade_in(0.5)
```

This builds a red bracket: a horizontal line from `a` to `b` with a short vertical tick at each
end. The path has 6 commands in total:

1. The main line: `move_to()` → `a`, `line_to()` → `b`
2. Left tick: `move_to()` at `a.get_pos()` offset by (0, −4), `line_to()` at `a.get_pos()`
   offset by (0, +4)
3. Right tick: same pattern at `b.get_pos()`

The key insight is that the tick endpoints are defined *relative to `a` and `b`* using
`pos(a.get_pos()).move(0, ±4)`. When `a` and `b` are animated to new positions the ticks move
with them automatically — you never have to update them separately.

The text label `t` sits next to the starting cell and shows the prime value in red.

```python
for i in range(3):
    m.hold()
    adv_time(0.5)
    a.pos(numbers[idx + i * step].get_pos(0.5)).move(0, -6)
    b.pos(numbers[idx + (i + 1) * step].get_pos(0.5)).move(0, -6)
    t.pos(numbers[idx + i * step].get_pos()).move(70, -30)
    r = numbers[idx + (i + 1) * step].get_child(kind="rect")
    r.hold()
    adv_time(0.5)
    r.color("red")
    adv_time(0.5)
```

Each iteration advances the bracket one step: `a` jumps to cell `idx + i*step` and `b` jumps
to the next multiple `idx + (i+1)*step`. At the same time the target cell's rect fades to red.
The bracket hops across the first three multiples of the prime with colour changes in sync.

After 3 hops the bracket group `m` and the grid `g` are held, the grid zooms back out to
normal size, and `m` is hidden by setting `alpha(0)`.

### Marking all remaining multiples

```python
for i in range(idx + (i * step), 100, step):
    adv_time(0.05)
    numbers[i].hold()
    with bstate():
        adv_time(0.3)
        numbers[i].get_child(kind="rect").color("red")
```

After the animated demonstration the rest of the multiples are coloured red in a rapid sweep.
The outer `adv_time(0.05)` staggers each cell by 50 ms; the actual colour change is scheduled
inside a `bstate()` so it takes 0.3 seconds and runs in parallel with the next cell's stagger.
This is the same parallel-animation pattern used for the initial cascade fade-in.

### Quick pass for 5, 7, 11

```python
for step in [5, 7, 11]:
    idx = step - 1
    # move arrow to the prime, highlight it green
    ...
    for i in range(idx + 3 * step, 100, step):
        adv_time(0.05)
        numbers[i].hold()
        with bstate():
            adv_time(0.3)
            numbers[i].get_child(kind="rect").color("red")
```

Primes 5, 7, and 11 get a simpler treatment: the arrow moves to each prime and its background
turns green, but there is no zoom-in or crossing-line animation. The multiples loop starts at
`idx + 3 * step` because all smaller multiples of these primes were already marked red during
the 2 and 3 passes.

### Revealing the primes

```python
adv_time(0.5)
for i in range(1, 100):
    numbers[i].hold()
adv_time(0.6)
for i in range(1, 100):
    if (i + 1) in PRIMES:
        continue
    numbers[i].alpha(0)
adv_time(0.5)
```

Before hiding anything, `hold()` is called on *every* cell at the current frame. This ensures
that each cell has an explicit keyframe at this point in time, giving the engine a clean
"from" value for the upcoming fade-out. Without the hold, cells that have not been changed
recently might interpolate from a stale keyframe.

After the hold, a 0.6-second pause is added and then all non-prime cells have their alpha set to
0. Because `linear()` is still active they all fade out simultaneously.

```python
for i in range(1, 100):
    numbers[i].hold()
adv_time(0.5)
for i, p in enumerate(PRIMES):
    idx = p - 1
    numbers[idx].xy(50 * (i % 20), (i // 20) * 50 + 300)
adv_time(0.5)
```

Another hold locks the surviving primes before they are repositioned. The `.xy()` calls reassign
each prime to a compact sub-grid (same column/row formula, but shifted 300 px down so it fits
within the canvas). Since the default transition mode at this point is `step()`, the cells jump
instantly to their new positions.

### The outro

```python
with Group().column(40).align_y(0.2) as g:
    with Group() as g2:
        Text().span("Sieve of Eratosthenes").font_size(40).bold()
    with Group() as g3:
        Image("docs/ff_logo.png").height(200)
    g.fade_in()
    adv_time(1.0)
```

This outro lives in the *same* `Scene` as the grid — it is a sibling group placed at
`align_y(0.2)` (top fifth of the canvas). `g.fade_in()` is called on the outer column group
rather than on `g2` and `g3` individually, so the title and logo fade in together as one unit.
`adv_time(1.0)` extends the scene for one more second so the final frame is not cut off
abruptly.
