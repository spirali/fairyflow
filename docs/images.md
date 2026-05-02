---
icon: lucide/image
---

# Images

## Image

`Image` loads and displays image. Supported formats include PNG, JPEG, ORA, and SVG.
Pass the file path to the constructor:

```python
with Scene():
    Image("assets/logo.png")
```

### Size and aspect ratio

By default the image preserves its original aspect ratio (`keep_aspect=True`). You can
override the display size with `.size(width, height)`:

```python
with Scene():
    Image("assets/photo.jpg", keep_aspect=True).size(200, 150)
```

Pass `keep_aspect=False` to stretch the image to fill the given dimensions exactly:

```python
with Scene():
    Image("assets/photo.jpg", keep_aspect=False).size(200, 150)
```

When only one dimension is given, the other is computed automatically to preserve the aspect ratio:

```python
with Scene():
    # Set width only → height is calculated from the image's aspect ratio
    Image("assets/photo.jpg").width(200)

    # Set height only → width is calculated from the image's aspect ratio
    Image("assets/photo.jpg").height(150)
```


### Positioning and alpha

`Image` supports the same `.xy()`, `.align_x()`, `.align_y()`, and `.alpha()` methods as
shape nodes — see [Positioning](positioning.md) for details.

---

## Layered images (ORA, SVG)

FairyFlow allows to work with individual layers of an image with format that supports layers: SVG and ORA.

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