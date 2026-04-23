
from typing import SupportsFloat
from .avalue import AnimatedValue
from .color import Color
from .exprs import Expr

type FloatLike = SupportsFloat | AnimatedValue["FloatLike"] | Expr
type StringLike = str | AnimatedValue["StringLike"] | Expr 
type ColorLike = str | Color | AnimatedValue["ColorLike"] | Expr
type BoolLike = bool | AnimatedValue[bool] | Expr