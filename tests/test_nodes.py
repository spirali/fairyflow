from alsie import *
from alsie.composer import create_export
from inline_snapshot import snapshot


def test_simple_scene():

    with scene(100, 100) as s:
        with node():
            rect()
            rect()

    create_export(s)
    assert create_export(s) == snapshot(
        {
            "key_frames": [0],
            "frames": [
                {
                    "kind": "scene", "width": 100, 'fill_color': '#ffffff', "height": 100,
                    "children": [
                        {
                            "kind": "node", 'id': 1, "x": 0, 'y': 0, "width": 0,
                            "height": 0, 'scale': 1, 'rotation': 0, 'alpha': 1, "children": [
                                {
                                    "kind": "rect", 'id': 2, "x": 0, 'y': 0, "width": 0,
                                    "height": 0, 'scale': 1, 'rotation': 0, 'alpha': 1, "fill_color": None, 'stroke_color': None, 'stroke_width': 1},
                                {
                                    "kind": "rect", 'id': 3, "x": 0, 'y': 0, "width": 0,
                                    "height": 0, 'scale': 1, 'rotation': 0, 'alpha': 1, "fill_color": None, 'stroke_color': None, 'stroke_width': 1},
                            ],
                        }
                    ],
                }
            ],
        }
    )
