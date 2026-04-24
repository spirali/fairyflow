---
icon: lucide/image
---

# Images

## Image

`Image` loads and displays a raster image file. Supported formats include PNG, JPEG, and SVG.
Pass the file path to the constructor:

```python
with Scene():
    Image("assets/logo.png").align_x(0.5).align_y(0.5)
```

### Size and aspect ratio

By default the image preserves its original aspect ratio (`keep_aspect=True`). You can
override the display size with `.size(width, height)`:

```python
with Scene():
    Image("assets/photo.jpg", keep_aspect=True).size(200, 150).align_x(0.5).align_y(0.5)
```

Pass `keep_aspect=False` to stretch the image to fill the given dimensions exactly:

```python
with Scene():
    Image("assets/photo.jpg", keep_aspect=False).size(200, 150).align_x(0.5).align_y(0.5)
```

### Positioning and alpha

`Image` supports the same `.xy()`, `.align_x()`, `.align_y()`, and `.alpha()` methods as
shape nodes, so images can be positioned, faded, and animated just like any other node.

---

## Layered images (ORA)

FairyFlow supports **OpenRaster** (`.ora`) files — layered image files that you can create
with Krita, GIMP, or other painting apps. Each named layer in the ORA file can be controlled
independently.

### Accessing layers

Call `.layer(name)` on an `Image` node to get a handle to a named layer. The layer node
exposes `.alpha()` so you can show or hide it independently of the others:

```python
with Scene():
    img = Image("assets/diagram.ora")

    # Show only the 'background' and 'labels' layers
    img.layer("background").alpha(1)
    img.layer("labels").alpha(1)
    img.layer("overlay").alpha(0)
```

### Animating layers

Layer visibility can be animated the same way as any other alpha value:

```python
with Scene():
    img = Image("assets/diagram.ora")

    overlay = img.layer("overlay")
    overlay.alpha(0)         # hidden at frame 0

    linear()
    adv_time(1)
    overlay.alpha(1)         # fades in over 1 second
```

---

## SVG images

SVG files are treated as a single-layer image. They scale losslessly to any size and support
`keep_aspect` just like raster images:

```python
with Scene():
    Image("assets/icon.svg").size(80, 80).align_x(0.5).align_y(0.5)
```
