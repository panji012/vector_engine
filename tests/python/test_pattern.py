import json
from vector_engine._native import analyze_pattern_suitability_json, generate_pattern_styled_json

def test_pattern():
    svg_content = open("input/Remix_AI_455_1784485211792.svg").read()
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
