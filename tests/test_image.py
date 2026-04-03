from fairyflow import *


def test_svg_image(test_scene):
    with test_scene:
        image("assets/test.svg").size(50, 40)


def test_svg_image_width_dtd(test_scene):
    with test_scene:
        image("assets/knight_with_dtd.svg").size(50, 40)


def test_png_image(test_scene):
    with test_scene:
        image("assets/testimg.png").size(50, 40)


def test_jpeg_image(test_scene):
    with test_scene:
        image("assets/testimg.jpeg").size(50, 40)


def test_ora_image(test_scene):
    with test_scene:
        image("assets/test.ora").size(50, 40)


def test_svg_image_layer_remove(test_scene):
    with test_scene:
        image("assets/test.svg").size(50, 40).layer("Ball").remove()

def test_ora_image_layer_remove(test_scene):
    with test_scene:
        image("assets/test.ora").size(50, 40).layer("B").remove()
