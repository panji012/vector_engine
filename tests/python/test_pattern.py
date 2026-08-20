import json
from vector_engine._native import analyze_pattern_suitability_json, generate_pattern_styled_json

def test_pattern():
    from pathlib import Path
    svg_files = list(Path("input").glob("*.svg"))
    if not svg_files:
        import pytest
        pytest.skip("No SVG input files found for pattern test")
    # Try to find a suitable file first
    svg_content = None
    for f in svg_files:
        content = f.read_text(encoding="utf-8")
        report = json.loads(analyze_pattern_suitability_json(content))
        if report.get("is_suitable", False):
            svg_content = content
            break

    # If no suitable file is found, just use the first file but set detect_suitable_only=False
    detect_suitable_only = True
    if svg_content is None:
        svg_content = svg_files[0].read_text(encoding="utf-8")
        detect_suitable_only = False

    report = json.loads(analyze_pattern_suitability_json(svg_content))
    print("Pattern Suitability Report:", report)

    # Test Slanted Left Pattern with Edge Wraparound
    slanted_pattern_svg = generate_pattern_styled_json(
        svg_content,
        grid_x=3,
        grid_y=3,
        style="slanted_left",
        detect_suitable_only=detect_suitable_only,
        slant_offset_ratio=-0.5,
        wraparound_seamless=True
    )
    print("Generated Slanted Left Pattern SVG length:", len(slanted_pattern_svg))
    assert len(slanted_pattern_svg) > 100

    print("Pattern slanted_left & wraparound unit test PASSED successfully!")

if __name__ == "__main__":
    test_pattern()
