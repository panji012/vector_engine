"""Boolean geometry operations module (Union)."""

import json
from typing import Dict, Any, List, Union

from vector_engine.exceptions import GeometryError


def union(poly1: Dict[str, Any], poly2: Dict[str, Any]) -> List[Dict[str, Any]]:
    """Compute the Boolean Union of two 2D polygons.

    Each polygon should be represented as a dict with 'exterior': [{'x': float, 'y': float}, ...]
    and optional 'interiors'.
    """
    try:
        from vector_engine import _native
        json1 = json.dumps(poly1)
        json2 = json.dumps(poly2)
        res_json = _native.union_polygons_json(json1, json2)
        return json.loads(res_json)
    except Exception as e:
        raise GeometryError(f"Polygon Union failed: {e}") from e
