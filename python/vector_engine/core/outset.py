"""Geometry Outset module."""

import json
from pathlib import Path
from typing import Dict, Any, Union, Tuple, Optional

from vector_engine.exceptions import GeometryError, IOError


def outset(
    svg_input: Union[str, Path],
    distance: float = 0.5,
    output_path: Optional[Union[str, Path]] = None,
) -> Tuple[Dict[str, Any], Dict[str, Any]]:
    """Outsets SVG geometry outward by distance px (default 0.5 px).

    Preserves holes, compound paths, fill color, and winding order without simplifying. Returns a tuple:
    `(outset_document, report_statistics)`.

    Report Statistics Contains:
    - outset_distance
    - original_bounds
    - new_bounds

    :param svg_input: SVG string content or file path.
    :param distance: Configurable outward offset distance in pixels. Default is 0.5.
    :param output_path: Optional file path to save the outset SVG document.
    :return: Tuple of (outset_document_dict, report_statistics_dict).
    """
    if distance < 0.0:
        raise GeometryError("Outset distance must be non-negative")

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
        doc_json, report_json = _native.apply_outset_json(content, True, float(distance), False, 0.0)
        outset_doc = json.loads(doc_json)
        report = json.loads(report_json)

        if output_path is not None:
            Path(output_path).write_text(doc_json, encoding="utf-8")

        return outset_doc, report
    except Exception as e:
        raise GeometryError(f"Outset operation failed: {e}") from e
