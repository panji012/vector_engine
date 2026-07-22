"""Polygon simplification module using Ramer-Douglas-Peucker (RDP) algorithm."""

import json
from typing import Dict, Any

from vector_engine.exceptions import GeometryError


def simplify(polygon: Dict[str, Any], tolerance: float = 0.1) -> Dict[str, Any]:
    """Simplify a 2D polygon with a configurable distance tolerance parameter.

    Preserves visual shape quality while reducing vertex count.
    """
    if tolerance < 0:
        raise GeometryError("Simplification tolerance must be a non-negative number")

    try:
        from vector_engine import _native
        poly_json = json.dumps(polygon)
        res_json = _native.simplify_polygon_json(poly_json, float(tolerance))
        return json.loads(res_json)
    except Exception as e:
        raise GeometryError(f"Polygon Simplification failed: {e}") from e
