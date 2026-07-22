import time
import pytest
from vector_engine.core import union

def area_of_polygon(poly):
    ext = poly["exterior"]
    if len(ext) < 3:
        return 0.0
    area = 0.0
    n = len(ext)
    for i in range(n):
        j = (i + 1) % n
        area += ext[i]["x"] * ext[j]["y"]
        area -= ext[j]["x"] * ext[i]["y"]
    return abs(area) / 2.0

def test_benchmark_and_inkscape_comparison():
    # Setup test shapes matching Inkscape SVG test primitives
    # Shape A: Rect 0..10, 0..10 (Area = 100)
    rect_a = {
        "exterior": [
            {"x": 0.0, "y": 0.0},
            {"x": 10.0, "y": 0.0},
            {"x": 10.0, "y": 10.0},
            {"x": 0.0, "y": 10.0},
        ],
        "interiors": []
    }

    # Shape B: Rect 5..15, 0..10 (Area = 100, Overlap = 50, Expected Union Area = 150)
    rect_b = {
        "exterior": [
            {"x": 5.0, "y": 0.0},
            {"x": 15.0, "y": 0.0},
            {"x": 15.0, "y": 10.0},
            {"x": 5.0, "y": 10.0},
        ],
        "interiors": []
    }

    # Benchmark execution speed over 1,000 iterations
    iterations = 1000
    start_time = time.perf_counter()
    for _ in range(iterations):
        res = union(rect_a, rect_b)
    end_time = time.perf_counter()

    avg_time_us = ((end_time - start_time) / iterations) * 1_000_000
    print(f"\n[BENCHMARK] Average Polygon Union Execution Time: {avg_time_us:.2f} µs per operation")

    # Verify geometry results against Inkscape standard boolean union
    assert len(res) == 1
    union_poly = res[0]
    total_area = area_of_polygon(union_poly)

    print(f"[INKSCAPE COMPARISON] Computed Union Area: {total_area:.2f} (Expected Inkscape Union: 150.00)")
    assert abs(total_area - 150.0) < 1e-3

if __name__ == "__main__":
    test_benchmark_and_inkscape_comparison()
