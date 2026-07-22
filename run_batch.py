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

# ==============================================================================
# === CONFIGURATION (EASY TO CHANGE) ===========================================
# ==============================================================================

# 1. Folder untuk mengambil file SVG asli
DEFAULT_INPUT_DIR = "input"

# 2. Folder untuk menyimpan hasil proses SVG
DEFAULT_OUTPUT_DIR = "output"

# 3. Jarak ketebalan Outset (dalam satuan pixel). Default: 0.5
DEFAULT_OUTSET_DISTANCE = 0.5

# 4. Aktifkan fitur Outset (Pelebaran path)? Jika False, hanya akan melakukan penggabungan grup (Boolean Union)
ENABLE_OUTSET = True

# 5. Aktifkan fitur Penyederhanaan (Simplify/Node Reduction) untuk mengoptimalkan jumlah titik?
ENABLE_SIMPLIFY = True

# 6. Toleransi akurasi Simplify (Semakin besar semakin tidak akurat, namun titik semakin sedikit). Default: 0.2
SIMPLIFY_TOLERANCE = 0.2

# ==============================================================================

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


def run_batch_queue(
    input_path: Path, 
    output_path: Path, 
    distance: float,
    enable_outset: bool = ENABLE_OUTSET,
    enable_simplify: bool = ENABLE_SIMPLIFY,
    simplify_tolerance: float = SIMPLIFY_TOLERANCE
) -> None:
    """Processes SVG files in a sequential fault-tolerant queue."""
    print("==================================================")
    print("Vector Geometry Engine - Python Batch Queue")
    print("==================================================")
    print(f"Input Directory  : {input_path}")
    print(f"Output Directory : {output_path}")
    print(f"Outset Distance  : {distance} px")
    print(f"Enable Outset    : {enable_outset}")
    print(f"Enable Simplify  : {enable_simplify} (Tol: {simplify_tolerance})")
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
    
    results_table = []

    for idx, svg_file in enumerate(svg_files, 1):
        rel_name = svg_file.name
        out_file = output_path / rel_name
        print(f"[{idx}/{len(svg_files)}] Processing: {rel_name} ...")

        file_start_time = time.time()
        try:
            # Execute Native Rust processing pipeline via PyO3 FFI
            process_svg_outset(
                str(svg_file), 
                str(out_file), 
                enable_outset, 
                distance, 
                enable_simplify, 
                simplify_tolerance
            )

            elapsed = (time.time() - file_start_time) * 1000.0
            print(f"  --> SUCCESS: Saved to '{out_file}' ({elapsed:.2f} ms)")
            results_table.append((rel_name, f"{elapsed:.2f} ms", "SUCCESS"))
            success_count += 1

        except Exception as e:
            elapsed = (time.time() - file_start_time) * 1000.0
            print(f"  --> ERROR: Failed to process '{rel_name}' ({elapsed:.2f} ms)", file=sys.stderr)
            print(f"      Details: {e}", file=sys.stderr)
            results_table.append((rel_name, f"{elapsed:.2f} ms", "FAILED"))
            failure_count += 1
            print("  --> Safely proceeding to next file in queue...\n")
            continue

    total_elapsed = time.time() - total_start_time

    print("\n================================================================================")
    print(" Batch Queue Execution Summary (Results Table) ")
    print("================================================================================")
    print(f"{'File Name':<45} | {'Time (ms)':<15} | {'Status':<10}")
    print("-" * 80)
    for name, time_ms, status in results_table:
        print(f"{name:<45} | {time_ms:<15} | {status:<10}")
    print("-" * 80)
    print(f"Total Files Processed : {len(svg_files)}")
    print(f"Successfully Saved    : {success_count}")
    print(f"Failures / Errors     : {failure_count}")
    print(f"Total Queue Time      : {total_elapsed:.2f} s")
    print("================================================================================\n")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Vector Geometry Engine - Python Batch Queue Processor"
    )
    parser.add_argument(
        "-i", "--input",
        type=Path,
        default=Path(DEFAULT_INPUT_DIR),
        help=f"Input directory or SVG file path (default: {DEFAULT_INPUT_DIR})"
    )
    parser.add_argument(
        "-o", "--output",
        type=Path,
        default=Path(DEFAULT_OUTPUT_DIR),
        help=f"Output directory to save processed SVG files (default: {DEFAULT_OUTPUT_DIR})"
    )
    parser.add_argument(
        "-d", "--distance",
        type=float,
        default=DEFAULT_OUTSET_DISTANCE,
        help=f"Outset distance in pixels (default: {DEFAULT_OUTSET_DISTANCE} px)"
    )

    args = parser.parse_args()
    run_batch_queue(args.input, args.output, args.distance)


if __name__ == "__main__":
    main()
