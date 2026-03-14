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
                    "kind": "scene",
                    "x": 0,
                    "width": 100,
                    "height": 100,
                    "children": [
                        {
                            "kind": "node",
                            "x": 0,
                            "width": 0,
                            "height": 0,
                            "children": [
                                {
                                    "kind": "rect",
                                    "x": 0,
                                    "width": 0,
                                    "height": 0,
                                    "fill_color": None,
                                },
                                {
                                    "kind": "rect",
                                    "x": 0,
                                    "width": 0,
                                    "height": 0,
                                    "fill_color": None,
                                },
                            ],
                        }
                    ],
                }
            ],
        }
    )
