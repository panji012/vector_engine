import sys
import json
from pathlib import Path

# Add project root to sys.path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from vector_engine._native import process_svg_outset, parse_svg_string

# 4 Badges arranged in a 2x2 grid with spatial gaps between them
FOUR_BADGES_SVG = """<svg width="400" height="400" viewBox="0 0 400 400" xmlns="http://www.w3.org/2000/svg">
  <!-- Badge 1: Top-Left (x: 10..50, y: 10..50) -->
  <path id="b1_bg" d="M 10 10 L 50 10 L 50 50 L 10 50 Z" fill="#2B2B2B"/>
  <path id="b1_fg" d="M 20 20 L 40 20 L 40 40 L 20 40 Z" fill="#FFFFFF"/>

  <!-- Badge 2: Top-Right (x: 200..240, y: 10..50) -->
  <path id="b2_bg" d="M 200 10 L 240 10 L 240 50 L 200 50 Z" fill="#1E7B34"/>
  <path id="b2_fg" d="M 210 20 L 230 20 L 230 40 L 210 40 Z" fill="#FFFFFF"/>

  <!-- Badge 3: Bottom-Left (x: 10..50, y: 200..240) -->
  <path id="b3_bg" d="M 10 200 L 50 200 L 50 240 L 10 240 Z" fill="#1E7B34"/>
  <path id="b3_fg" d="M 20 210 L 40 210 L 40 230 L 20 230 Z" fill="#FFFFFF"/>

  <!-- Badge 4: Bottom-Right (x: 200..240, y: 200..240) -->
  <path id="b4_bg" d="M 200 200 L 240 200 L 240 240 L 200 240 Z" fill="#2B2B2B"/>
  <path id="b4_fg" d="M 210 210 L 230 210 L 230 230 L 210 230 Z" fill="#FFFFFF"/>
</svg>"""

def test_four_badge_grouping():
    input_file = Path("output_test/test_4badges.svg")
    input_file.parent.mkdir(parents=True, exist_ok=True)
    input_file.write_text(FOUR_BADGES_SVG)

    output_file = Path("output_test/out_4badges.svg")

    output_svg = process_svg_outset(
        str(input_file),
        str(output_file),
        False, 0.0, False, 0.0
    )

    # 1. Verify that SVG contains <g id="badge_1"> .. <g id="badge_4">
    assert '<g id="badge_1">' in output_svg
    assert '<g id="badge_2">' in output_svg
    assert '<g id="badge_3">' in output_svg
    assert '<g id="badge_4">' in output_svg

    # 2. Verify parsed JSON doc has badge_id attributes set
    doc = json.loads(parse_svg_string(output_svg))
    badge_ids = {p.get("badge_id") for p in doc["paths"]}
    assert badge_ids == {1, 2, 3, 4}

    print("SUCCESS: 4-Badge Spatial Grouping Test Passed!")

def test_badge_svg_forces_single_mode():
    from run_batch import run_batch_queue
    input_file = Path("output_test/badge_input.svg")
    input_file.parent.mkdir(parents=True, exist_ok=True)
    input_file.write_text(FOUR_BADGES_SVG)

    out_dir = Path("output_test/badge_out")
    out_dir.mkdir(parents=True, exist_ok=True)

    # Process file without explicit mode flags (default mode)
    run_batch_queue(
        input_path=input_file,
        output_path=out_dir,
        no_emoji=True
    )

    out_files = [f.name for f in out_dir.glob("*")]
    # Deliverables MUST ONLY be single deliverables (badge_input_single.eps, badge_input_single.jpg, badge_input_single.svg)!
    assert len(out_files) > 0
    for fname in out_files:
        assert "_single" in fname or "single" in fname
        assert "bad_two" not in fname
        assert "pattern" not in fname

    assert any(fname.endswith("_single.svg") for fname in out_files)

    print("SUCCESS: Badge SVG Single Mode Deliverables Test Passed!")

if __name__ == "__main__":
    test_four_badge_grouping()
    test_badge_svg_forces_single_mode()
