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
These files are created:


* ``fairyflow.toml`` - Configuration file
* ``prologue.py`` - Python file included into every scene
* ``scenes/scene1.ffpy`` - A scene file
* ``sequences/sequence1.ffsq`` - A sequence file (a sequence of scenes)


## Start development environment

```bash
$ fairyflow open <project_name>
```

It starts a local web server. Click the printed URL to open the interactive environment.

## First code & render

Open `scenes/scene1.ffpy`, it will contain the following code; 
press ++ctrl+enter++ to evaluate.

```ffpy video="mp4"
with Scene():
    stext("Hello world!").fade_out(dur=1)
```


## Next steps

- [Project Structure](structure.md) — scenes, groups, and files
- [Shapes](shapes.md) — Rect, Ellipse, Path
- [Text](text.md) — Text and font styling
- [Animations](anim.md) — transitions, fades, path following
- [Positioning](positioning.md) — coordinates, alignment, and cross-group positioning
- [Layout](layout.md) — column and row layouts
- [Cues](cues.md) — interactive presentations
