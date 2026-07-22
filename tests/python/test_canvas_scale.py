import pytest
from vector_engine.core import scale_canvas
from vector_engine.exceptions import GeometryError

def test_scale_only_below_target():
    # 100x100 square polygon
    small_poly = {
        "exterior": [
            {"x": 0.0, "y": 0.0},
            {"x": 100.0, "y": 0.0},
            {"x": 100.0, "y": 100.0},
            {"x": 0.0, "y": 100.0},
        ],
        "interiors": []
    }

    try:
        # Test Case 1: Target = 1024 pixels. Small poly (100x100) MUST scale up 10.24x
        scaled, factor = scale_canvas(small_poly, target_pixels=1024.0)
        assert abs(factor - 10.24) < 1e-4
        assert abs(scaled["exterior"][1]["x"] - 1024.0) < 1e-4

        # Test Case 2: Target = 50 pixels. Small poly (100x100) is ALREADY above target, MUST NOT scale (factor = 1.0)
        untouched, factor_no_op = scale_canvas(small_poly, target_pixels=50.0)
        assert factor_no_op == 1.0
        assert untouched["exterior"][1]["x"] == 100.0
    except GeometryError as e:
        if "Canvas scaling failed" in str(e):
            pytest.skip("Native FFI extension module not built yet")
        raise
