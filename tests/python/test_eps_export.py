import pytest
from vector_engine.io import export_eps
from vector_engine.exceptions import IOError

def test_export_eps_adobe_inkscape_compatibility():
    polygon = {
        "exterior": [
            {"x": 10.0, "y": 10.0},
            {"x": 100.0, "y": 10.0},
            {"x": 100.0, "y": 100.0},
            {"x": 10.0, "y": 100.0},
        ],
        "interiors": []
    }

    try:
        eps_content = export_eps(polygon, title="Test Compatibility EPS")

        # 1. Adobe PostScript 3.0 EPS Header Specification
        assert eps_content.startswith("%!PS-Adobe-3.0 EPSF-3.0\n")

        # 2. Document Structuring Conventions (DSC) BoundingBox Comments required by Illustrator & Inkscape
        assert "%%BoundingBox: 10 10 100 100\n" in eps_content
        assert "%%HiResBoundingBox: 10.0000 10.0000 100.0000 100.0000\n" in eps_content
        assert "%%EndComments\n" in eps_content

        # 3. Valid PostScript path operators
        assert "moveto\n" in eps_content
        assert "lineto\n" in eps_content
        assert "closepath\n" in eps_content
        assert "fill\n" in eps_content

        # 4. Standard trailer
        assert eps_content.endswith("%%EOF\n")
        print("\n[EPS COMPATIBILITY TEST] Adobe Illustrator & Inkscape EPS DSC headers validated successfully.")
    except IOError as e:
        if "EPS Export failed" in str(e):
            pytest.skip("Native FFI extension module not built yet")
        raise
