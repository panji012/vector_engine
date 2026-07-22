"""Color Grouping and Combine module."""

import json
from pathlib import Path
from typing import Dict, Any, Union, Tuple, Optional

from vector_engine.exceptions import GeometryError, IOError


def combine_color_groups(
    svg_input: Union[str, Path],
    output_path: Optional[Union[str, Path]] = None,
) -> Tuple[Dict[str, Any], Dict[str, Any]]:
    """Group SVG paths by identical fill color and perform boolean union combine within each color group.

    Preserves fill rules, holes, and compound path topologies. Returns a tuple:
    `(combined_document, report_statistics)`.

    Report Statistics Contains:
    - original_path_count
    - number_of_color_groups
    - combined_path_count
    - group_details

    :param svg_input: SVG string content or file path.
    :param output_path: Optional file path to save the combined SVG document.
    :return: Tuple of (combined_document_dict, report_statistics_dict).
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
        doc_json, report_json = _native.combine_color_groups_json(content)
        combined_doc = json.loads(doc_json)
        report = json.loads(report_json)

        if output_path is not None:
            Path(output_path).write_text(doc_json, encoding="utf-8")

        return combined_doc, report
    except Exception as e:
        raise GeometryError(f"Color Grouping and Combine failed: {e}") from e
