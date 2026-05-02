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

```ffpy video="mp4"
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
        # Build a 5-row × 20-column grid of number cells, initially invisible
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
