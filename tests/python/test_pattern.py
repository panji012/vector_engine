import json
from vector_engine._native import analyze_pattern_suitability_json, generate_pattern_styled_json

def test_pattern():
    svg_content = open("input/Remix_AI_455_1784485211792.svg").read()
    report = json.loads(analyze_pattern_suitability_json(svg_content))
    print("Pattern Suitability Report:", report)

    pattern_svg = generate_pattern_styled_json(svg_content, grid_x=3, grid_y=3, style="alternating", detect_suitable_only=True)
    print("Generated Alternating Pattern SVG length:", len(pattern_svg))
    assert len(pattern_svg) > 100

    grid_pattern_svg = generate_pattern_styled_json(svg_content, grid_x=3, grid_y=3, style="grid", detect_suitable_only=True)
    print("Generated Grid Pattern SVG length:", len(grid_pattern_svg))
    assert len(grid_pattern_svg) > 100

    print("Pattern unit test PASSED successfully!")

if __name__ == "__main__":
    test_pattern()
