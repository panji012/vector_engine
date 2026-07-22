"""SVG parsing module leveraging native Rust SVG parser."""

import json
from pathlib import Path
from typing import Dict, Any, Union

from vector_engine.exceptions import IOError, GeometryError


def parse_svg(svg_input: Union[str, Path]) -> Dict[str, Any]:
    """Parse SVG content (from string or file path) using the Rust SVG engine.

    Returns a structured dictionary representing the vector document.
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
        json_str = _native.parse_svg_string(content)
        return json.loads(json_str)
    except Exception as e:
        raise IOError(f"Failed to parse SVG: {e}") from e


def combine_svg_paths(d1: str, d2: str) -> str:
    """Compute the Union of two SVG path 'd' attribute strings.

    Returns the merged SVG path 'd' string representation.
    """
    try:
        from vector_engine import _native
        return _native.union_svg_path_strings(d1, d2)
    except Exception as e:
        raise GeometryError(f"Failed to combine SVG paths: {e}") from e
