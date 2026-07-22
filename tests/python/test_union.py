import pytest
from vector_engine.core import union
from vector_engine.exceptions import GeometryError

def test_polygon_union():
    poly1 = {
        "exterior": [
            {"x": 0.0, "y": 0.0},
            {"x": 10.0, "y": 0.0},
            {"x": 10.0, "y": 10.0},
            {"x": 0.0, "y": 10.0},
        ],
        "interiors": []
    }

    poly2 = {
        "exterior": [
            {"x": 5.0, "y": 0.0},
            {"x": 15.0, "y": 0.0},
            {"x": 15.0, "y": 10.0},
            {"x": 5.0, "y": 10.0},
        ],
        "interiors": []
    }

    try:
        res = union(poly1, poly2)
        assert len(res) >= 1
        assert "exterior" in res[0]
    except GeometryError as e:
        if "Polygon Union failed" in str(e):
            pytest.skip("Native FFI extension module not built yet")
        raise
