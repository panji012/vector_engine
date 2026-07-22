"""Seamless pattern detection and generation module."""

import json
from pathlib import Path
from typing import Dict, Any, Union, Tuple, Optional

from vector_engine.exceptions import GeometryError, IOError


def analyze_pattern_suitability(svg_input: Union[str, Path]) -> Dict[str, Any]:
    """Analyze an SVG string or file for seamless pattern suitability.

    Does NOT generate pattern tiles. Returns a detailed suitability analysis report.
    """
    if isinstance(svg_input, Path) or (isinstance(svg_input, str) and not svg_input.strip().startswith("<")):
        path = Path(svg_input)
        if not path.exists():
            raise IOError(f"SVG file not found: {path}")
        try:
            content = path.read_text(encoding="utf-8")
        except Exception as e:
            raise IOError(f"Failed to read SVG file {path}: {e}") from e
    else:
        content = svg_input

    try:
        from vector_engine import _native
        json_str = _native.analyze_pattern_suitability_json(content)
        return json.loads(json_str)
    except Exception as e:
        raise GeometryError(f"Pattern suitability analysis failed: {e}") from e


def generate_pattern(
    svg_input: Union[str, Path],
    grid: Tuple[int, int] = (3, 3),
    output_path: Optional[Union[str, Path]] = None,
) -> Dict[str, Any]:
    """Generate a seamless pattern grid ONLY IF Pattern Detection approves.

    Preserves all original path colors, fills, strokes, and styling.

    :param svg_input: SVG string content or file path.
    :param grid: Tuple of (grid_x, grid_y) repeat counts. Default is (3, 3).
    :param output_path: Optional file path to save the generated pattern.
    :return: Dictionary representing the tiled pattern VectorDocument.
    """
    grid_x, grid_y = grid
    if grid_x <= 0 or grid_y <= 0:
        raise GeometryError("Grid dimensions must be positive integers")

    if isinstance(svg_input, Path) or (isinstance(svg_input, str) and not svg_input.strip().startswith("<")):
        path = Path(svg_input)
        if not path.exists():
            raise IOError(f"SVG file not found: {path}")
        try:
            content = path.read_text(encoding="utf-8")
        except Exception as e:
            raise IOError(f"Failed to read SVG file {path}: {e}") from e
    else:
        content = svg_input

    try:
        from vector_engine import _native
        tiled_json = _native.generate_pattern_svg_json(content, int(grid_x), int(grid_y))
        tiled_doc = json.loads(tiled_json)

        if output_path is not None:
            Path(output_path).write_text(tiled_json, encoding="utf-8")

        return tiled_doc
    except Exception as e:
        raise GeometryError(f"Pattern Generation failed: {e}") from e
