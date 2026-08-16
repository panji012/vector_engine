import json
from vector_engine._native import analyze_pattern_suitability_json, generate_pattern_styled_json

def test_pattern():
    from pathlib import Path
    svg_files = list(Path("input").glob("*.svg"))
    if not svg_files:
        import pytest
        pytest.skip("No SVG input files found for pattern test")
    svg_content = svg_files[0].read_text(encoding="utf-8")
    report = json.loads(analyze_pattern_suitability_json(svg_content))
    print("Pattern Suitability Report:", report)

    # Test Slanted Left Pattern with Edge Wraparound
    slanted_pattern_svg = generate_pattern_styled_json(
        svg_content,
        grid_x=3,
        grid_y=3,
        style="slanted_left",
        detect_suitable_only=True,
        slant_offset_ratio=-0.5,
        wraparound_seamless=True
    )
    print("Generated Slanted Left Pattern SVG length:", len(slanted_pattern_svg))
    assert len(slanted_pattern_svg) > 100
    assert "_wrap_x" in slanted_pattern_svg

    print("Pattern slanted_left & wraparound unit test PASSED successfully!")

if __name__ == "__main__":
    test_pattern()
