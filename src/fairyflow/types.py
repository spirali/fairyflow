

from .avalue import AnimatedValue
from .color import Color
from .exprs import Expr

type FloatLike = float | int | AnimatedValue["FloatLike"] | Expr
type StringLike = str | AnimatedValue["StringLike"] | Expr 
type ColorLike = str | Color | AnimatedValue["ColorLike"] | Expr