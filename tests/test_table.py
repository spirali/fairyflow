import pytest

from fairyflow import Image, Scene, Table


def test_table_basic_str_cells(test_scene):
    test_scene.pdf_tolerance = (
        120  # a full table's worth of glyphs crosses many raster/vector AA edges
    )
    with test_scene.size(220, 110):
        Table(
            [
                ["Name", "Age", "City"],
                ["Alice", "32", "Oslo"],
                ["Bob", "28", "Brno"],
            ],
            header=True,
        )


def test_table_align_per_column(test_scene):
    test_scene.pdf_tolerance = 80
    with test_scene.size(220, 110):
        Table(
            [
                ["Name", "Score"],
                ["Alice", "1"],
                ["Bob", "2"],
            ],
            header=True,
            align=["left", "center"],
        )


def test_table_image_cell():
    s = Scene(200, 100)
    with s:
        img = Image("tests/assets/test.svg").size(15, 15)
        t = Table([["Name", "Icon"], ["Alice", img]])
    # The image ends up nested under its cell, not left as a stray sibling
    # of the table (or of whatever the ambient scene-level parent was) at
    # the point it was originally constructed.
    assert img in t.cell(1, 1)._children
    assert img not in s._children


def test_table_cell_row_col_addressing():
    s = Scene(200, 100)
    with s:
        t = Table([["a", "b"], ["c", "d"], ["e", "f"]])
    assert t.cell(1, 1) is t._cells[1][1]
    assert t.row(0) == [t.cell(0, 0), t.cell(0, 1)]
    assert t.col(1) == [t.cell(0, 1), t.cell(1, 1), t.cell(2, 1)]


def test_table_explicit_widths():
    s = Scene(200, 100)
    with s:
        t = Table([["a", "b"]], widths=[40, 80])
    assert t.cell(0, 0)._attrs["width"].get_first_value() == 40
    assert t.cell(0, 1)._attrs["width"].get_first_value() == 80


def test_table_highlight_cell():
    s = Scene(200, 100)
    with s:
        t = Table([["a", "b"], ["c", "d"]])
        t.cell(1, 0).fill("gold", dur=0.3)
    av = t.cell(1, 0)._bg._attrs["fill_color"]
    assert len(av.values) == 2  # animated: snapped-current + "gold" keyframes
    # No other cell's background picked up the highlight.
    assert t.cell(1, 1)._bg._attrs["fill_color"].get_first_value() != "gold"


def test_table_padding_background_stays_centered_on_asymmetric_padding():
    """Regression for the bug caught during implementation: a cell's
    background Rect must stay pinned at (0, 0) regardless of the cell's own
    padding (which only insets the *content*), or asymmetric padding would
    shift the background off-center relative to its own rel(1)-sized box."""
    s = Scene(200, 100)
    with s:
        t = Table([["a", "b"]], padding=(20, 4))
    bg = t.cell(0, 0)._bg
    assert bg._attrs["x"].get_first_value() == 0
    assert bg._attrs["y"].get_first_value() == 0


def test_table_rejects_empty_rows():
    s = Scene(200, 100)
    with s, pytest.raises(ValueError):
        Table([])


def test_table_rejects_ragged_rows():
    s = Scene(200, 100)
    with s, pytest.raises(ValueError):
        Table([["a", "b"], ["c"]])
