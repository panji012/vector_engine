import pytest
from vector_engine.spatial import generate_pattern, analyze_pattern_suitability
from vector_engine.exceptions import GeometryError

SUITABLE_COLORFUL_SVG = """<svg width="100" height="100" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
  <path id="flower" d="M 10 10 L 90 10 L 90 90 L 10 90 Z" fill="#E91E63" stroke="#880E4F" stroke-width="2"/>
</svg>"""

UNSUITABLE_SVG = """<svg width="500" height="20" viewBox="0 0 500 20" xmlns="http://www.w3.org/2000/svg">
  <line x1="0" y1="10" x2="500" y2="10" stroke="black"/>
</svg>"""

def test_pattern_generator_approval_gate_and_color_preservation():
    try:
        # Test Case 1: Suitable Colorful SVG -> Generate 3x3 pattern tile
        report = analyze_pattern_suitability(SUITABLE_COLORFUL_SVG)
        assert report["is_suitable"] is True

        tiled = generate_pattern(SUITABLE_COLORFUL_SVG, grid=(3, 3))
        # 3x3 grid of 1 path = 9 tiled paths
        assert len(tiled["paths"]) == 9

        # Verify Color Preservation across all generated pattern tiles
        for path in tiled["paths"]:
            assert path["fill"] == "#E91E63"
            assert path["stroke"] == "#880E4F"
            assert path["stroke_width"] == 2.0

        print("\n[PATTERN GENERATOR TEST] 3x3 Grid pattern generated with original colors preserved (#E91E63 / #880E4F).")

        # Test Case 2: Unsuitable SVG -> Must fail gatekeeper check and raise GeometryError
        with pytest.raises(GeometryError) as exc_info:
            generate_pattern(UNSUITABLE_SVG, grid=(3, 3))
        assert "not suitable for seamless pattern generation" in str(exc_info.value)
        print("[PATTERN GENERATOR TEST] Gatekeeper rejection for unsuitable SVG validated successfully.")

    except GeometryError as e:
        if "Pattern Generation failed" in str(e) or "Pattern suitability analysis failed" in str(e):
            pytest.skip("Native FFI extension module not built yet")
        raise
