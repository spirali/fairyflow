---
icon: lucide/folder-tree
---

# Project Structure

## Concepts

FairyFlow animations are built around three layers: the **sequences**, the **scene file**, and the **scene**.
A sequence (`.ffsq` file) a collection of scene files. Each scene file (`.ffpy`) may contain one or more scenes.



### Scene

A `Scene` is the top-level canvas. It defines the canvas dimensions and background color. Everything visible in an animation belongs to a scene.

```python
with Scene(width=400, height=300, color="white"):
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
        Rect().size(60, 60).color("steelblue").xy(10, 30)
        Ellipse().size(60, 60).color("coral").xy(130, 30)
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