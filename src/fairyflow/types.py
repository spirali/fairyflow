from typing import SupportsFloat

from .avalue import AnimatedValue
from .color import Color, Gradient
from .exprs import Expr

type FloatLike = SupportsFloat | AnimatedValue["FloatLike"] | Expr
type StringLike = str | AnimatedValue["StringLike"] | Expr
type ColorLike = None | str | Color | AnimatedValue["ColorLike"] | Expr
type FillLike = ColorLike | Gradient
type BoolLike = bool | AnimatedValue[bool] | Expr
