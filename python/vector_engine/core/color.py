"""Color Variant Engine module with configurable color palettes."""

import json
from pathlib import Path
from typing import Dict, Any, Union, List, Optional

from vector_engine.exceptions import GeometryError, IOError


def generate_color_variants(
    svg_input: Union[str, Path],
    palette: Union[List[str], str] = "cyberpunk",
    output_path: Optional[Union[str, Path]] = None,
) -> Dict[str, Any]:
    """Generate color-themed variant geometry from SVG content using configurable palettes.

    :param svg_input: SVG string content or file path.
    :param palette: List of custom HEX color strings (e.g. ["#FF0000", "#00FF00"]) or preset name
                    ("pastel", "cyberpunk", "monochrome", "earth"). Default is "cyberpunk".
    :param output_path: Optional file path to save the color variant document.
    :return: Dictionary representing the recolored VectorDocument.
    """
    if isinstance(palette, str):
        palette_list = [palette]
    else:
        palette_list = list(palette)

    if not palette_list:
        raise GeometryError("Palette cannot be empty")

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
        variant_json = _native.apply_color_palette_json(content, palette_list)
        variant_doc = json.loads(variant_json)

        if output_path is not None:
            Path(output_path).write_text(variant_json, encoding="utf-8")

        return variant_doc
    except Exception as e:
        raise GeometryError(f"Color Variant Engine failed: {e}") from e
