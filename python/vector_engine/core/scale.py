"""Canvas Scaling module with configurable target pixels."""

import json
from typing import Dict, Any, Tuple

from vector_engine.exceptions import GeometryError


def scale_canvas(polygon: Dict[str, Any], target_pixels: float = 1024.0) -> Tuple[Dict[str, Any], float]:
    """Scale a 2D polygon up to target_pixels ONLY if its maximum dimension is below target_pixels.

    Returns a tuple of (scaled_polygon_dict, applied_scale_factor).
    If the polygon is already at or above target_pixels, returns (original_polygon, 1.0).
    """
    if target_pixels <= 0:
        raise GeometryError("target_pixels must be a positive number")

    try:
        from vector_engine import _native
        poly_json = json.dumps(polygon)
        scaled_json, scale_factor = _native.scale_canvas_json(poly_json, float(target_pixels))
        return json.loads(scaled_json), scale_factor
    except Exception as e:
        raise GeometryError(f"Canvas scaling failed: {e}") from e
