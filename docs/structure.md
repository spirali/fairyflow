---
icon: lucide/folder-tree
---

# Project Structure

## Concepts

FairyFlow animations are built around three layers: the **sequence**, the **scene file**, and the **scene**.

## Sequence

A sequence is an ordered collection of scene files. 
Final export of the project is done in through sequences.
It is stored in files with suffix `.ffsq`.

## Scene file

A scene file (suffix `.ffpy`) is a Python file that defines one or more scenes.


## Project layout

A project created with `fairyflow init` looks like:

```
my_project/
├── fairyflow.toml       ← project settings (fps, prologue path)
├── prologue.py          ← shared imports and defaults for all scenes
├── scenes/
│   └── scene1.ffpy      ← animation code (add more .ffpy files as needed)
└── sequences/
    └── sequence1.ffsq   ← sequence definition (ordered playlist of scenes)
```

It is just an initial layout; scenes and sequences could be placed in arbitrary directories.
The only fixed path is `fairyflow.toml` that has to be in the root.


### Scene

A `Scene` is the top-level canvas. It defines the canvas dimensions and background color. Everything visible in an animation belongs to a scene.

```python
with Scene(width=400, height=300, background="white"):
    ...
```

The default scene size is **300 × 200** pixels; **white** background.
Note that width and height is a logical size; **not** a target resolution.
All rendering information are stored as vector graphics and rasterized into the final resolution at very last moment.

### Group

A `Group` is a container node. It has its own coordinate system, optional layout (column/row), and an animatable clipping window. Children are positioned relative to the group's origin.

```python
with Scene():
    with Group().size(200, 120).xy(50, 40):
        Rect().size(60, 60).fill("steelblue").xy(10, 30)
        Ellipse().size(60, 60).fill("coral").xy(130, 30)
```

### Nodes

Leaf nodes are the building blocks of a scene:

| Node | Description |
|---|---|
| `Rect` | Rectangle with fill and optional stroke |
| `Ellipse` | Ellipse or circle |
| `Path` | Arbitrary vector path (lines, cubic curves) |
| `Text` | Multi-span text block |
| `Image` | Raster image (PNG, JPEG, SVG, ORA) |

All nodes support animatable position, alpha, and z-ordering. Shape nodes also support color and stroke. See the individual pages for details.