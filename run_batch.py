#!/usr/bin/env python3
"""
Vector Geometry Engine - 100% Native Rust CLI Wrapper
Disables the old hybrid Python pipeline and delegates all processing to `vector-cli`.
"""

import sys
import subprocess
from pathlib import Path

def main():
    # 1. Ensure the Rust CLI binary is built in release mode
    root_dir = Path(__file__).parent.resolve()
    cli_bin = root_dir / "target" / "release" / "vector-cli"

    if not cli_bin.exists():
        print("[WRAPPER] Native Rust CLI binary not found. Building `vector-cli` in release mode...", flush=True)
        try:
            subprocess.run(
                ["cargo", "build", "--bin", "vector-cli", "--release"],
                cwd=str(root_dir),
                check=True
            )
            print("[WRAPPER] Build successful!", flush=True)
        except subprocess.CalledProcessError as e:
            print(f"[WRAPPER ERROR] Failed to compile native Rust CLI: {e}", file=sys.stderr, flush=True)
            sys.exit(1)

    # 2. Map command line arguments to Rust CLI arguments
    # Translate python's parser arguments directly to rust CLI arguments.
    args = sys.argv[1:]
    rust_args = [str(cli_bin)]

    # We map `--no-emoji` or `--clean` directly to `--clean` in Rust
    # We map `-w` / `--workers` / `--file-workers` to `--workers` or `-w`
    idx = 0
    while idx < len(args):
        arg = args[idx]
        if arg in ("--no-emoji", "--clean"):
            rust_args.append("--clean")
        elif arg in ("-w", "--workers", "--file-workers"):
            rust_args.append("--workers")
            if idx + 1 < len(args):
                rust_args.append(args[idx + 1])
                idx += 1
        elif arg == "--tolerance":
            rust_args.append("--tolerance")
            if idx + 1 < len(args):
                rust_args.append(args[idx + 1])
                idx += 1
        elif arg == "--distance":
            rust_args.append("--distance")
            if idx + 1 < len(args):
                rust_args.append(args[idx + 1])
                idx += 1
        elif arg == "--silhouette-threshold":
            rust_args.append("--silhouette-threshold")
            if idx + 1 < len(args):
                rust_args.append(args[idx + 1])
                idx += 1
        elif arg == "--remove-layers":
            rust_args.append("--remove-layers")
            if idx + 1 < len(args):
                rust_args.append(args[idx + 1])
                idx += 1
        elif arg == "--min-nodes":
            rust_args.append("--min-nodes")
            if idx + 1 < len(args):
                rust_args.append(args[idx + 1])
                idx += 1
        elif arg == "--line-art":
            rust_args.append("--line-art")
            if idx + 1 < len(args):
                rust_args.append(args[idx + 1])
                idx += 1
        elif arg == "--bg-color":
            rust_args.append("--bg-color")
            if idx + 1 < len(args):
                rust_args.append(args[idx + 1])
                idx += 1
        elif arg == "--fg-color":
            rust_args.append("--fg-color")
            if idx + 1 < len(args):
                rust_args.append(args[idx + 1])
                idx += 1
        elif arg == "--invert":
            rust_args.append("--invert")
            if idx + 1 < len(args):
                rust_args.append(args[idx + 1])
                idx += 1
        elif arg == "--gap":
            rust_args.append("--gap")
            if idx + 1 < len(args):
                rust_args.append(args[idx + 1])
                idx += 1
        elif arg == "--min-area":
            rust_args.append("--min-area")
            if idx + 1 < len(args):
                rust_args.append(args[idx + 1])
                idx += 1
        elif arg == "--max-area":
            rust_args.append("--max-area")
            if idx + 1 < len(args):
                rust_args.append(args[idx + 1])
                idx += 1
        else:
            rust_args.append(arg)
        idx += 1

    # 3. Execute the native Rust CLI
    try:
        res = subprocess.run(rust_args, check=True)
        sys.exit(res.returncode)
    except subprocess.CalledProcessError as e:
        sys.exit(e.returncode)

if __name__ == "__main__":
    main()
