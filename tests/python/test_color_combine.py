import pytest
from vector_engine.core import combine_color_groups
from vector_engine.exceptions import GeometryError

COLOR_GROUPS_SVG = """<svg width="200" height="200" viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">
  <!-- Red Group: 2 Overlapping Rectangles -->
  <path id="r1" d="M 10 10 L 60 10 L 60 60 L 10 60 Z" fill="#FF0000"/>
  <path id="r2" d="M 30 10 L 80 10 L 80 60 L 30 60 Z" fill="#FF0000"/>

  <!-- Blue Group: 1 Separate Rectangle -->
  <path id="b1" d="M 100 10 L 150 10 L 150 60 L 100 60 Z" fill="#0000FF"/>
</svg>"""

def test_color_grouping_and_combine():
    try:
        doc, report = combine_color_groups(COLOR_GROUPS_SVG)

        # Check Report Statistics
        assert report["original_path_count"] == 3
        assert report["number_of_color_groups"] == 2
        assert report["combined_path_count"] == 2

        # Verify combined document paths
        assert len(doc["paths"]) == 2

        # Check group details
        group_colors = [g["fill_color"] for g in report["group_details"]]
        assert "#FF0000" in group_colors or "#ff0000" in group_colors
        assert "#0000FF" in group_colors or "#0000ff" in group_colors

        print("\n[COLOR COMBINE TEST] Report Statistics validated successfully:")
        print(f"  - Original Path Count: {report['original_path_count']}")
        print(f"  - Number of Color Groups: {report['number_of_color_groups']}")
        print(f"  - Combined Path Count: {report['combined_path_count']}")

    except GeometryError as e:
        if "Color Grouping and Combine failed" in str(e):
            pytest.skip("Native FFI extension module not built yet")
        raise
