---
icon: lucide/rocket
---

# Get started

## Installation

```bash
$ pip install fairyflow
```

## Create project

```bash
$ fairyflow init <project_name>
```

This creates an initial project layout. See [Project Structure](structure.md) for details.

## Start development environment

```bash
$ fairyflow open <project_name>
```

It starts a local web server. Click the printed URL to open the interactive environment.

## First code & render

Open `scene.py` in the editor and write your first scene. Press ++ctrl+s++ to evaluate:

```ffpy frame=0
with Scene():
    Rect().size(120, 70).color("steelblue").align_x(0.5).align_y(0.5)
```

## Your first animation

Add time-based changes to create motion:

```ffpy video="mp4"
with Scene():
    r = Rect().size(60, 60).color("steelblue").xy(20, 70)
    linear()
    adv_time(1.5)
    r.xy(220, 70).color("tomato")
```

`linear()` switches interpolation to smooth, and `adv_time(1.5)` advances the timeline by
1.5 seconds. Attribute changes after that call are placed at the new time.

## Next steps

- [Project Structure](structure.md) — scenes, groups, and files
- [Shapes](shapes.md) — Rect, Ellipse, Path
- [Text](text.md) — Text and font styling
- [Animations](anim.md) — transitions, fades, path following
- [Layout](layout.md) — column and row layouts
- [Cues](cues.md) — interactive presentations
