import math
from typing import Literal, SupportsFloat

from beartype import beartype

from .exprs import Call, to_expr
from .nodes import Node, Path
from .position import Position
from .types import FloatLike


def _point_xy(node: Node, point: tuple[FloatLike, FloatLike] | Position):
    """Resolve a point-like value (a live `Position`, tracked via the usual
    `map_x`/`map_y` machinery, or a plain (x, y) pair) into an (x, y)
    expression pair already in `node`'s parent frame. Always returns `Expr`s
    (even for a plain numeric pair) so the result composes with `+`/`-`/`*`
    regardless of which kind of point was passed in."""
    if isinstance(point, Position):
        resolved = point.into_node(node._parent)
        return resolved.x, resolved.y
    x, y = point
    return to_expr(x), to_expr(y)


@beartype
class Line(Path):
    """A straight line between two points, built as a two-command `Path`.

    Endpoints accept either a plain `(x, y)` pair or a live `Position`
    (`a.at("right")`), tracked for free via the usual `map_x`/`map_y`
    machinery. `.start`/`.end` expose the underlying `PathMove`/`PathLine`
    handles for further animation.
    """

    def __init__(
        self,
        start: tuple[FloatLike, FloatLike] | Position,
        end: tuple[FloatLike, FloatLike] | Position,
    ):
        super().__init__()
        sx, sy = _point_xy(self, start)
        ex, ey = _point_xy(self, end)
        self.start = self.move_to(sx, sy)
        self.end = self.line_to(ex, ey)


@beartype
class Arrow(Path):
    """A straight connector with an arrowhead at one or both ends.

    Endpoints accept either a plain `(x, y)` pair or a live `Position`,
    same as `Line`. `gap` shifts whichever endpoint(s) receive an
    arrowhead inward, toward the other endpoint, by `gap` pixels — so the
    head doesn't touch whatever it's pointing at. The tail end (no
    arrowhead) stays exactly at its given point.

    Re-targeting `.start`/`.end` after construction (`.pos()`/`.xy()`) does
    not retroactively reapply `gap` — it overwrites the endpoint outright,
    same as every other placeable node's setters.
    """

    def __init__(
        self,
        start: tuple[FloatLike, FloatLike] | Position,
        end: tuple[FloatLike, FloatLike] | Position,
        gap: SupportsFloat = 0,
        head: Literal["end", "start", "both"] = "end",
        style: Literal["triangle", "open", "stealth", "bar", "dot"] = "triangle",
    ):
        super().__init__()
        sx, sy = _point_xy(self, start)
        ex, ey = _point_xy(self, end)

        if gap:
            ux = Call.norm(sx - ex, sy - ey)  # unit vector end -> start
            uy = Call.norm(sy - ey, sx - ex)
            if head in ("start", "both"):
                sx, sy = sx - ux * gap, sy - uy * gap
            if head in ("end", "both"):
                ex, ey = ex + ux * gap, ey + uy * gap

        self.start = self.move_to(sx, sy)
        self.end = self.line_to(ex, ey)

        self.arrowheads = []
        if head in ("start", "both"):
            self.arrowheads.append(self.arrow("start", style=style))
        if head in ("end", "both"):
            self.arrowheads.append(self.arrow("end", style=style))


@beartype
class Polygon(Path):
    """A closed vector shape from a list of (x, y) vertices."""

    def __init__(self, points: list[tuple[SupportsFloat, SupportsFloat]]):
        super().__init__()
        if len(points) < 2:
            raise ValueError("Polygon needs at least 2 points")
        x0, y0 = points[0]
        self.move_to(x0, y0)
        for x, y in points[1:]:
            self.line_to(x, y)
        self.close()


@beartype
class RegularPolygon(Polygon):
    """A regular n-gon centered at `center`, first vertex pointing up by default."""

    def __init__(
        self,
        n: int,
        radius: SupportsFloat,
        *,
        center: tuple[SupportsFloat, SupportsFloat] = (0, 0),
        rotation: SupportsFloat = 0,
    ):
        if n < 3:
            raise ValueError("RegularPolygon needs at least 3 sides")
        radius = float(radius)
        cx, cy = float(center[0]), float(center[1])
        start = math.radians(float(rotation) - 90)
        points = [
            (
                cx + radius * math.cos(start + 2 * math.pi * i / n),
                cy + radius * math.sin(start + 2 * math.pi * i / n),
            )
            for i in range(n)
        ]
        super().__init__(points)


@beartype
class Star(Polygon):
    """A `points`-pointed star, alternating between `outer` and `inner` radius."""

    def __init__(
        self,
        points: int,
        outer: SupportsFloat,
        inner: SupportsFloat,
        *,
        center: tuple[SupportsFloat, SupportsFloat] = (0, 0),
        rotation: SupportsFloat = 0,
    ):
        if points < 2:
            raise ValueError("Star needs at least 2 points")
        outer = float(outer)
        inner = float(inner)
        cx, cy = float(center[0]), float(center[1])
        start = math.radians(float(rotation) - 90)
        n = points * 2
        verts = [
            (
                cx
                + (outer if i % 2 == 0 else inner)
                * math.cos(start + math.pi * i / points),
                cy
                + (outer if i % 2 == 0 else inner)
                * math.sin(start + math.pi * i / points),
            )
            for i in range(n)
        ]
        super().__init__(verts)
