from typing import Literal, Union

from beartype import beartype

from ..types import ColorLike, FloatLike
from ..sentinels import rel
from ..nodes import Group, Node, Rect, _effective_width, _effective_height
from ..text import Text
from ..exprs import Call

_ALIGN_FACTOR = {"left": 0.0, "center": 0.5, "right": 1.0}
_HEADER_FILL = "#e0e0e0"


@beartype
class _TableCell(Group):
    def __init__(self):
        super().__init__()
        self._bg: Rect | None = None

    def fill(self, value: ColorLike, *, dur=None, ease=None) -> "_TableCell":
        self._bg.fill(value, dur=dur, ease=ease)
        return self


def _adopt(node: Node, new_parent: Group) -> Node:
    """Reparent an already-constructed node (e.g. an `Image` built just
    before `Table(...)`, hence already a child of whatever was the ambient
    parent then) under `new_parent` instead - mirrors `TextSpan._promote_to_group()`'s
    reparenting mechanics (`text.py`): `Node._id` is assigned once at
    construction and never revisited, and `serializer.py` builds its
    id -> wire-index map by DFS over `_children` at serialize time, not by
    `_id` order, so moving a node between `_children` lists is safe."""
    if node._parent is not None and node in node._parent._children:
        node._parent._children.remove(node)
    node._parent = new_parent
    new_parent._children.append(node)
    return node


@beartype
class Table(Group):
    """A grid of cells built from `rows`, each a `Group` (`Rect` background +
    content) exposing `.fill(color)` directly (forwarded to its own
    background `Rect`), addressed 0-based including the header.

    A cell value is a `str` (becomes a `Text`) or any other node (e.g. an
    `Image`, adopted from wherever it was originally constructed). Not a
    full table renderer: cell spanning, per-cell border control, and
    scrolling are deliberate non-goals - build the grid manually with
    `Group().grid(...)` for those.

    `Table` extends `Group` and locks its layout to `grid()` in the
    constructor, so re-switching layout afterwards doesn't make sense; the
    inherited `row()` (normally `Group`'s row-layout switcher) is
    deliberately shadowed here to mean "the cells in row r" instead - the
    layout-switching meaning has no legitimate use on a `Table`.
    """

    def __init__(
        self,
        rows: list[list[Union[str, Node]]],
        *,
        header: bool = False,
        widths: Union[Literal["auto"], FloatLike, list[FloatLike]] = "auto",
        row_height: Union[Literal["auto"], FloatLike, list[FloatLike]] = "auto",
        padding: tuple[FloatLike, FloatLike] = (12, 6),
        stroke: ColorLike | None = "#888",
        fill: ColorLike | None = None,
        align: Union[
            Literal["left", "center", "right"],
            list[Literal["left", "center", "right"]],
        ] = "left",
    ):
        if not rows:
            raise ValueError("Table needs at least one row")
        n_cols = len(rows[0])
        if any(len(r) != n_cols for r in rows):
            raise ValueError("Table: every row must have the same number of cells")

        super().__init__()
        self.grid(cols=n_cols, gap=0)

        pad_x, pad_y = padding
        aligns = align if isinstance(align, list) else [align] * n_cols

        self._cells: list[list[_TableCell]] = []
        self._content: list[list[Node]] = []

        with self:
            for r, row_values in enumerate(rows):
                cell_row: list[_TableCell] = []
                content_row: list[Node] = []
                is_header_row = header and r == 0
                for c, value in enumerate(row_values):
                    with _TableCell() as cell:
                        cell.padding(x=pad_x, y=pad_y)
                        # Pinned at (0, 0), bypassing auto positioning: the
                        # background must span the *full* cell regardless of
                        # the cell's own padding (which only insets its
                        # auto-positioned content), or asymmetric padding
                        # would shift it off-center relative to its own
                        # rel(1)-sized box.
                        bg = Rect().xy(0, 0).size(rel(1), rel(1))
                        cell._bg = bg
                        if stroke is not None:
                            bg.stroke(stroke)
                        if is_header_row:
                            bg.fill(_HEADER_FILL)
                        elif fill is not None:
                            bg.fill(fill)
                        else:
                            bg.fill(None)

                        content: Node = (
                            Text(value)
                            if isinstance(value, str)
                            else _adopt(value, cell)
                        )
                        content.align(_ALIGN_FACTOR[aligns[c]], 0.5)
                        if is_header_row and isinstance(content, Text):
                            content.font(bold=True)

                    cell_row.append(cell)
                    content_row.append(content)
                self._cells.append(cell_row)
                self._content.append(content_row)

        n_rows = len(rows)
        self._apply_widths(widths, pad_x, n_cols, n_rows)
        self._apply_row_heights(row_height, pad_y, n_cols, n_rows)

    def _apply_widths(self, widths, pad_x, n_cols, n_rows):
        if widths == "auto":
            for c in range(n_cols):
                expr = _effective_width(self._content[0][c])
                for r in range(1, n_rows):
                    expr = Call.max(expr, _effective_width(self._content[r][c]))
                col_w = expr + 2 * pad_x
                for r in range(n_rows):
                    self._cells[r][c].width(col_w)
        elif isinstance(widths, list):
            for c, w in enumerate(widths):
                for r in range(n_rows):
                    self._cells[r][c].width(w)
        else:
            for r in range(n_rows):
                for c in range(n_cols):
                    self._cells[r][c].width(widths)

    def _apply_row_heights(self, row_height, pad_y, n_cols, n_rows):
        if row_height == "auto":
            for r in range(n_rows):
                expr = _effective_height(self._content[r][0])
                for c in range(1, n_cols):
                    expr = Call.max(expr, _effective_height(self._content[r][c]))
                row_h = expr + 2 * pad_y
                for c in range(n_cols):
                    self._cells[r][c].height(row_h)
        elif isinstance(row_height, list):
            for r, h in enumerate(row_height):
                for c in range(n_cols):
                    self._cells[r][c].height(h)
        else:
            for r in range(n_rows):
                for c in range(n_cols):
                    self._cells[r][c].height(row_height)

    def cell(self, r: int, c: int) -> "_TableCell":
        """The cell at row `r`, column `c` (0-based, header included)."""
        return self._cells[r][c]

    def row(self, r: int) -> list["_TableCell"]:
        """All cells in row `r`, left to right."""
        return list(self._cells[r])

    def col(self, c: int) -> list["_TableCell"]:
        """All cells in column `c`, top to bottom."""
        return [row[c] for row in self._cells]
