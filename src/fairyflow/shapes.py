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
class CircularArrow(Path):
    """A circular arc with an arrowhead at one or both ends.

    `angle_start`/`angle_end` are in degrees, with 0° pointing up and
    increasing clockwise, matching `rotate()`/`RegularPolygon`. Sweeping
    from a smaller to a larger angle draws clockwise; swap them to sweep
    counterclockwise. The arc is built as a sequence of `cubic_to` segments
    (at most 90° each) approximating the circle.
    """

    def __init__(
        self,
        center: tuple[SupportsFloat, SupportsFloat],
        radius: SupportsFloat,
        angle_start: SupportsFloat,
        angle_end: SupportsFloat,
        head: Literal["end", "start", "both"] = "end",
        style: Literal["triangle", "open", "stealth", "bar", "dot"] = "triangle",
        arrow_length: FloatLike | None = None,
        arrow_width: FloatLike | None = None,
    ):
        super().__init__()
        if float(angle_start) == float(angle_end):
            raise ValueError("CircularArrow needs angle_start != angle_end")

        cx, cy = float(center[0]), float(center[1])
        radius = float(radius)
        a0 = math.radians(float(angle_start) - 90)
        a1 = math.radians(float(angle_end) - 90)

        sweep = a1 - a0
        segments = max(1, math.ceil(abs(sweep) / (math.pi / 2)))
        step = sweep / segments

        self.move_to(cx + radius * math.cos(a0), cy + radius * math.sin(a0))
        for i in range(segments):
            seg_start = a0 + step * i
            seg_end = seg_start + step
            k = (4 / 3) * math.tan(step / 4)
            c1 = (-k * radius * math.sin(seg_start), k * radius * math.cos(seg_start))
            c2 = (k * radius * math.sin(seg_end), -k * radius * math.cos(seg_end))
            self.cubic_to(
                cx + radius * math.cos(seg_end),
                cy + radius * math.sin(seg_end),
                c1=c1,
                c2=c2,
            )

        self.arrowheads = []
        if head in ("start", "both"):
            self.arrowheads.append(
                self.arrow("start", style=style, length=arrow_length, width=arrow_width)
            )
        if head in ("end", "both"):
            self.arrowheads.append(
                self.arrow("end", style=style, length=arrow_length, width=arrow_width)
            )


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
