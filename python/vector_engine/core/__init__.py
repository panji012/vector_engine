"""Vector Engine Core package exporting boolean, simplification, canvas scaling, color variants, luminance variants, color grouping/combine, and outset."""

from vector_engine.core.boolean import union
from vector_engine.core.simplify import simplify
from vector_engine.core.scale import scale_canvas
from vector_engine.core.color import generate_color_variants
from vector_engine.core.luminance import adjust_luminance
from vector_engine.core.color_combine import combine_color_groups
from vector_engine.core.outset import outset

__all__ = [
    "union",
    "simplify",
    "scale_canvas",
    "generate_color_variants",
    "adjust_luminance",
    "combine_color_groups",
    "outset",
]
