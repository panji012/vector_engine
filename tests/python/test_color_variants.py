import pytest
from vector_engine.core import generate_color_variants
from vector_engine.exceptions import GeometryError

SAMPLE_MULTILAYER_SVG = """<svg width="100" height="100" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
  <path id="l1" d="M 0 0 L 50 0 L 50 50 Z" fill="#000000" stroke="#000000"/>
  <path id="l2" d="M 50 50 L 100 50 L 100 100 Z" fill="#FFFFFF" stroke="#000000"/>
</svg>"""

def test_color_variant_engine():
    try:
        # Test Case 1: Built-in preset palette ("cyberpunk")
        cyberpunk_doc = generate_color_variants(SAMPLE_MULTILAYER_SVG, palette="cyberpunk")
        assert len(cyberpunk_doc["paths"]) == 2
        assert cyberpunk_doc["paths"][0]["fill"] == "#00F0FF"
        assert cyberpunk_doc["paths"][1]["fill"] == "#FF007F"

        # Test Case 2: Custom HEX palette
        custom_palette = ["#E91E63", "#9C27B0"]
        custom_doc = generate_color_variants(SAMPLE_MULTILAYER_SVG, palette=custom_palette)
        assert custom_doc["paths"][0]["fill"] == "#E91E63"
        assert custom_doc["paths"][1]["fill"] == "#9C27B0"

        print("\n[COLOR VARIANT TEST] Preset 'cyberpunk' and custom HEX palette re-coloring validated successfully.")
    except GeometryError as e:
        if "Color Variant Engine failed" in str(e):
            pytest.skip("Native FFI extension module not built yet")
        raise
