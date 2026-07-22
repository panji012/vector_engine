"""Luminance Variant Engine module with configurable scaling factor."""

import json
from pathlib import Path
from typing import Dict, Any, Union, Optional

from vector_engine.exceptions import GeometryError, IOError


def adjust_luminance(
    svg_input: Union[str, Path],
    factor: float = 1.2,
    output_path: Optional[Union[str, Path]] = None,
) -> Dict[str, Any]:
    """Adjust the luminance / brightness of colors in an SVG geometry using a configurable factor.

    - factor > 1.0 brightens colors (e.g. 1.5 increases lightness by 50%).
    - factor < 1.0 darkens colors (e.g. 0.5 reduces lightness by 50%).

    :param svg_input: SVG string content or file path.
    :param factor: Configurable luminance multiplier factor (non-negative float). Default is 1.2.
    :param output_path: Optional file path to save the luminance adjusted document.
    :return: Dictionary representing the adjusted VectorDocument.
    """
    if factor < 0:
        raise GeometryError("Luminance factor must be a non-negative number")

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
        adjusted_json = _native.adjust_luminance_json(content, float(factor))
        adjusted_doc = json.loads(adjusted_json)

        if output_path is not None:
            Path(output_path).write_text(adjusted_json, encoding="utf-8")

        return adjusted_doc
    except Exception as e:
        raise GeometryError(f"Luminance Variant Engine failed: {e}") from e
