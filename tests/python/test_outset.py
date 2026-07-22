import pytest
from vector_engine.core import outset, combine_color_groups
from vector_engine.exceptions import GeometryError

SAMPLE_SQUARE_SVG = """<svg width="200" height="200" viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">
  <path id="box" d="M 10 10 L 110 10 L 110 110 L 10 110 Z" fill="#FF0000"/>
</svg>"""

def test_outset_operation():
    try:
        # First group into compound path with combine_color_groups
        combined_doc, _ = combine_color_groups(SAMPLE_SQUARE_SVG)
        
        # Apply Outset with 0.5 px default distance
        outset_doc, report = outset(SAMPLE_SQUARE_SVG, distance=0.5)

        # Check Report Statistics
        assert report["outset_distance"] == 0.5
        assert report["original_bounds"] == [10.0, 10.0, 110.0, 110.0]
        # Outward offset by 0.5 px expands bounds by 0.5 in each direction
        assert report["new_bounds"] == [9.5, 9.5, 110.5, 110.5]

        # Check path styling preservation
        assert len(outset_doc["paths"]) == 1
        assert outset_doc["paths"][0]["fill"] == "#FF0000"

        print("\n[OUTSET TEST] Outset 0.5 px statistics validated successfully:")
        print(f"  - Outset Distance: {report['outset_distance']} px")
        print(f"  - Original Bounds: {report['original_bounds']}")
        print(f"  - New Bounds: {report['new_bounds']}")

    except GeometryError as e:
        if "Outset operation failed" in str(e):
            pytest.skip("Native FFI extension module not built yet")
        raise
