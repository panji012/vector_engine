"""JPG preview image generation module with configurable resolution."""

import json
from pathlib import Path
from typing import Dict, Any, Union, Tuple, Optional

from vector_engine.exceptions import IOError


def generate_jpg_preview(
    polygon: Dict[str, Any],
    output_path: Optional[Union[str, Path]] = None,
    resolution: Tuple[int, int] = (1024, 1024),
    quality: int = 85,
) -> bytes:
    """Render a polygon geometry to a JPEG preview image at a configurable resolution.

    :param polygon: Dictionary representing polygon geometry.
    :param output_path: Optional path to save the JPEG image file.
    :param resolution: Tuple of (width, height) resolution in pixels. Default is (1024, 1024).
    :param quality: JPEG quality parameter (1 to 100). Default is 85.
    :return: Raw JPEG image bytes.
    """
    width, height = resolution
    if width <= 0 or height <= 0:
        raise IOError("Resolution width and height must be positive integers")

    try:
        from vector_engine import _native
        poly_json = json.dumps(polygon)
        jpg_bytes = _native.render_jpg_preview_json(poly_json, int(width), int(height), int(quality))

        if output_path is not None:
            path = Path(output_path)
            path.write_bytes(jpg_bytes)

        return jpg_bytes
    except Exception as e:
        raise IOError(f"JPG Preview generation failed: {e}") from e
