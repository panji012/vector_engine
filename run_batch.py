#!/usr/bin/env python3
"""
Vector Geometry Engine - Standalone Python Batch Queue Processor

Reads SVG files sequentially from an input directory, applies Color Grouping
and Outset operations via PyO3 C-Extension bindings (`process_svg_outset`),
and saves the output SVG files to an output directory.

Includes fault-tolerant try/except error handling so individual file errors
or geometry panics are safely caught and logged without breaking the queue.
"""

import argparse
import sys
import time
from pathlib import Path

# Add python directory to module search path
sys.path.insert(0, str(Path(__file__).parent / "python"))

try:
    from vector_engine._native import process_svg_outset
except ImportError:
    try:
        from vector_engine import process_svg_outset
    except ImportError:
        process_svg_outset = None

if process_svg_outset is None:
    print("CRITICAL ERROR: Native PyO3 extension 'process_svg_outset' could not be loaded.", file=sys.stderr)
    print("Please ensure the project is compiled with 'cargo build --release' or 'maturin develop'.", file=sys.stderr)
    sys.exit(1)


def discover_svg_files(input_dir: Path) -> list[Path]:
    """Discovers all SVG files in the given directory or single file path."""
    if input_dir.is_file():
        if input_dir.suffix.lower() == ".svg":
            return [input_dir]
        return []
    elif input_dir.is_dir():
        return sorted([p for p in input_dir.glob("*.svg") if p.is_file()])
    return []


def run_batch_queue(input_path: Path, output_path: Path, distance: float) -> None:
    """Processes SVG files in a sequential fault-tolerant queue."""
    print("==================================================")
    print("Vector Geometry Engine - Python Batch Queue")
    print("==================================================")
    print(f"Input Directory  : {input_path}")
    print(f"Output Directory : {output_path}")
    print(f"Outset Distance  : {distance} px")
    print("==================================================\n")

    output_path.mkdir(parents=True, exist_ok=True)

    svg_files = discover_svg_files(input_path)
    if not svg_files:
        print(f"WARNING: No SVG files found at input path: {input_path}")
        return

    print(f"Found {len(svg_files)} SVG file(s) to process in queue...\n")

    success_count = 0
    failure_count = 0
    total_start_time = time.time()

    for idx, svg_file in enumerate(svg_files, 1):
        rel_name = svg_file.name
        out_file = output_path / rel_name
        print(f"[{idx}/{len(svg_files)}] Processing: {rel_name} ...")

        file_start_time = time.time()
        try:
            # Execute Native Rust processing pipeline via PyO3 FFI
            process_svg_outset(str(svg_file), str(out_file), distance)

            elapsed = (time.time() - file_start_time) * 1000.0
            print(f"  --> SUCCESS: Saved to '{out_file}' ({elapsed:.2f} ms)")
            success_count += 1

        except Exception as e:
            elapsed = (time.time() - file_start_time) * 1000.0
            print(f"  --> ERROR: Failed to process '{rel_name}' ({elapsed:.2f} ms)", file=sys.stderr)
            print(f"      Details: {e}", file=sys.stderr)
            failure_count += 1
            print("  --> Safely proceeding to next file in queue...\n")
            continue

    total_elapsed = time.time() - total_start_time

    print("\n==================================================")
    print("Batch Queue Execution Summary")
    print("==================================================")
    print(f"Total Files Processed : {len(svg_files)}")
    print(f"Successfully Saved    : {success_count}")
    print(f"Failures / Errors     : {failure_count}")
    print(f"Total Queue Time      : {total_elapsed:.2f} s")
    print("==================================================\n")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Vector Geometry Engine - Python Batch Queue Processor"
    )
    parser.add_argument(
        "-i", "--input",
        type=Path,
        default=Path("input"),
        help="Input directory or SVG file path (default: input/)"
    )
    parser.add_argument(
        "-o", "--output",
        type=Path,
        default=Path("output"),
        help="Output directory to save processed SVG files (default: output/)"
    )
    parser.add_argument(
        "-d", "--distance",
        type=float,
        default=0.5,
        help="Outset distance in pixels (default: 0.5 px)"
    )

    args = parser.parse_args()
    run_batch_queue(args.input, args.output, args.distance)


if __name__ == "__main__":
    main()
