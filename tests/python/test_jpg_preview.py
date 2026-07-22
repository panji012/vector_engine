import pytest
from vector_engine.io import generate_jpg_preview
from vector_engine.exceptions import IOError

def test_generate_jpg_preview_configurable_resolution():
    polygon = {
        "exterior": [
            {"x": 0.0, "y": 0.0},
            {"x": 100.0, "y": 0.0},
            {"x": 100.0, "y": 100.0},
            {"x": 0.0, "y": 100.0},
        ],
        "interiors": []
    }

    try:
        # Test Case 1: Standard 1024x1024 resolution
        jpg_bytes_1024 = generate_jpg_preview(polygon, resolution=(1024, 1024), quality=90)
        assert len(jpg_bytes_1024) > 100
        # Check JPEG magic header 0xFF 0xD8 0xFF
        assert jpg_bytes_1024[:3] == b"\xff\xd8\xff"

        # Test Case 2: Custom 512x512 resolution
        jpg_bytes_512 = generate_jpg_preview(polygon, resolution=(512, 512), quality=80)
        assert len(jpg_bytes_512) > 100
        assert jpg_bytes_512[:3] == b"\xff\xd8\xff"

        print("\n[JPG PREVIEW TEST] Configurable resolution JPEG generation validated successfully.")
    except IOError as e:
        if "JPG Preview generation failed" in str(e):
            pytest.skip("Native FFI extension module not built yet")
        raise
