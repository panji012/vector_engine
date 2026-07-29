import sys
import json
from pathlib import Path

# Add project root to sys.path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from calibrate_silhouette import (
    color_distance,
    are_colors_similar_hsl,
    merge_similar_colors_in_svg,
    count_unique_svg_colors,
    parse_svg_string,
)

MIXED_COLORS_SVG = """<svg width="300" height="300" viewBox="0 0 300 300" xmlns="http://www.w3.org/2000/svg">
  <!-- Light Green (2 paths) -->
  <path id="p1" d="M 10 10 L 60 10 L 60 60 Z" fill="#90EE90"/>
  <path id="p2" d="M 30 10 L 80 10 L 80 60 Z" fill="#90EE90"/>
  
  <!-- Slightly Darker Light Green (2 paths) -->
  <path id="p3" d="M 100 10 L 150 10 L 150 60 Z" fill="#76C776"/>
  <path id="p4" d="M 120 10 L 170 10 L 170 60 Z" fill="#76C776"/>

  <!-- Blue (2 paths) - MUST NEVER MERGE WITH GREEN -->
  <path id="p5" d="M 10 100 L 60 100 L 60 150 Z" fill="#579DB6"/>
  <path id="p6" d="M 30 100 L 80 100 L 80 150 Z" fill="#579DB6"/>
</svg>"""

def test_hsl_hue_isolation():
    # Light green vs slightly darker light green: SAME HUE FAMILY -> MERGE TRUE
    assert are_colors_similar_hsl("#90EE90", "#76C776", max_hue_diff=20.0) is True

    # Green vs Blue: DIFFERENT HUE FAMILY -> MERGE FALSE
    assert are_colors_similar_hsl("#90EE90", "#579DB6", max_hue_diff=20.0) is False
    assert are_colors_similar_hsl("#76C776", "#579DB6", max_hue_diff=20.0) is False


def test_merge_colors_and_consolidate_layers():
    orig_colors = count_unique_svg_colors(MIXED_COLORS_SVG)
    assert orig_colors == 3

    # Merge SVG
    merged_svg = merge_similar_colors_in_svg(MIXED_COLORS_SVG, threshold=0.15)
    
    # 1. Colors check: 2 greens merge to 1 green, blue stays separate -> 2 unique colors
    merged_color_count = count_unique_svg_colors(merged_svg)
    assert merged_color_count == 2
    assert "#579DB6" in merged_svg  # Blue is intact!

    # 2. Path count check: 6 original paths consolidated into 2 compound layers!
    doc = json.loads(parse_svg_string(merged_svg))
    assert len(doc["paths"]) == 2  # Exactly 1 layer for Green, 1 layer for Blue!


if __name__ == "__main__":
    test_hsl_hue_isolation()
    test_merge_colors_and_consolidate_layers()
    print("ALL HSL COLOR & LAYER MERGE TESTS PASSED SUCCESSFULLY!")
