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

def test_single_badge_grouping():
    SINGLE_BADGE_SVG = """<svg width="400" height="400" viewBox="0 0 400 400" xmlns="http://www.w3.org/2000/svg">
  <!-- Single Badge in Center -->
  <path id="badge_ring" d="M 100 100 A 100 100 0 1 1 300 300 Z" fill="#1E7B34"/>
  <path id="badge_center" d="M 150 150 L 250 150 L 250 250 L 150 250 Z" fill="#FFFFFF"/>
</svg>"""
    input_file = Path("output_test/test_1badge.svg")
    input_file.parent.mkdir(parents=True, exist_ok=True)
    input_file.write_text(SINGLE_BADGE_SVG)

    output_file = Path("output_test/out_1badge.svg")

    output_svg = process_svg_outset(
        str(input_file),
        str(output_file),
        False, 0.0, False, 0.0
    )

    # 1. Verify that SVG contains <g id="badge_1">
    assert '<g id="badge_1">' in output_svg

    # 2. Verify parsed JSON doc has badge_id attributes set
    doc = json.loads(parse_svg_string(output_svg))
    badge_ids = {p.get("badge_id") for p in doc["paths"]}
    assert 1 in badge_ids

    print("SUCCESS: 1-Badge Spatial Grouping Test Passed!")

def test_sticker_sheet_varying_sizes_grouping():
    # Sticker sheet with 1 large central sticker and 2 small surrounding accessory stickers
    STICKER_SHEET_SVG = """<svg width="1000" height="1000" viewBox="0 0 1000 1000" xmlns="http://www.w3.org/2000/svg">
  <!-- Meditating Girl Sticker (Large 200x200 in center) -->
  <path id="girl_body" d="M 400 400 L 600 400 L 600 600 L 400 600 Z" fill="#D32F2F"/>
  <path id="girl_text" d="M 450 350 L 550 350 L 550 370 L 450 370 Z" fill="#2B2B2B"/>

  <!-- Coffee Cup Sticker (Small 50x50 at top-left) -->
  <path id="cup_body" d="M 50 50 L 100 50 L 100 100 L 50 100 Z" fill="#1E7B34"/>
  
  <!-- Alarm Clock Sticker (Small 50x50 at bottom-right) -->
  <path id="clock_body" d="M 850 850 L 900 850 L 900 900 L 850 900 Z" fill="#FBC02D"/>
</svg>"""
    input_file = Path("output_test/test_sticker_sheet.svg")
    input_file.parent.mkdir(parents=True, exist_ok=True)
    input_file.write_text(STICKER_SHEET_SVG)

    output_file = Path("output_test/out_sticker_sheet.svg")

    output_svg = process_svg_outset(
        str(input_file),
        str(output_file),
        False, 0.0, False, 0.0
    )

    # 1. Verify that SVG contains <g id="badge_1">, <g id="badge_2">, <g id="badge_3">
    assert '<g id="badge_1">' in output_svg
    assert '<g id="badge_2">' in output_svg
    assert '<g id="badge_3">' in output_svg

    # 2. Verify parsed JSON doc has badge_id attributes set
    doc = json.loads(parse_svg_string(output_svg))
    badge_ids = {p.get("badge_id") for p in doc["paths"]}
    assert badge_ids == {1, 2, 3}

    print("SUCCESS: Sticker Sheet (Varying Element Sizes) Grouping Test Passed!")

if __name__ == "__main__":
    test_four_badge_grouping()
    test_single_badge_grouping()
    test_sticker_sheet_varying_sizes_grouping()
    test_badge_svg_forces_single_mode()
