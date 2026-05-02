from pathlib import Path
from fairyflow import Image

ASSETS = Path(__file__).parent / "assets"


def test_svg_image(test_scene):
    with test_scene:
        Image(ASSETS / "test.svg").size(50, 40)


def test_svg_image_width_dtd(test_scene):
    with test_scene:
        Image(ASSETS / "knight_with_dtd.svg").size(50, 40)


def test_png_image(test_scene):
    with test_scene:
        Image(ASSETS / "testimg.png").size(50, 40)


def test_jpeg_image(test_scene):
    with test_scene:
        Image(ASSETS / "testimg.jpeg").size(50, 40)


def test_ora_image(test_scene):
    with test_scene:
        Image(ASSETS / "test.ora").size(50, 40)


def test_svg_image_layer_remove(test_scene):
    with test_scene:
        Image(ASSETS / "test.svg").size(50, 40).layer("Ball").remove()

def test_ora_image_layer_remove(test_scene):
    with test_scene:
        Image(ASSETS / "test.ora").size(50, 40).layer("B").remove()
