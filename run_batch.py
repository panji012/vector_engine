#!/usr/bin/env python3
"""
Vector Geometry Engine - Standalone Python Batch Queue Processor

End-to-End Pipeline:
1. Outset & Simplify via PyO3 C-Extension
2. Inkscape C++ Script Simplify Action
3. Background Luminance Variants:
   - Design 1: Blue Palette, 80% bg luminance (from 60% base + 20% control boost)
   - Design 2: Brown Palette, 97.5% bg luminance (from 95% base + 2.5% control boost)
4. Canvas Auto-Scaling: Target total pixel area between 19,000,000 px and 22,000,000 px
5. SVG Bad (Two) Dual Variant Board: Side-by-side combined SVG artwork
6. EPS and JPG Exporters for single SVG & Bad Two SVG
7. Circular Pattern Detection ("bulat melingkar") & Generator
"""

import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

# ==============================================================================
# === CONFIGURATION (EASY TO CHANGE) ===========================================
# ==============================================================================

DEFAULT_INPUT_DIR = "input"
DEFAULT_OUTPUT_DIR = "output"
DEFAULT_OUTSET_DISTANCE = 0.5
ENABLE_OUTSET = True
ENABLE_SIMPLIFY = True
SIMPLIFY_TOLERANCE = 0.2

# RAM Disk Temporary Work Directory (/dev/shm)
RAM_TEMP_DIR = Path("/dev/shm/vector_engine_work")

# Canvas Auto-Scale Area Bounds (px)
MIN_CANVAS_AREA_PX = 19_000_000.0
MAX_CANVAS_AREA_PX = 22_000_000.0

# ==============================================================================

# Add python directory to module search path
sys.path.insert(0, str(Path(__file__).parent / "python"))

try:
    from vector_engine._native import (
        process_svg_outset,
        inkscape_cpp_simplify_svg,
        parse_svg_string,
        apply_design1_variant_json,
        apply_design2_variant_json,
        apply_silhouette_variant_json,
        scale_document_area_json,
        create_duo_board_json,
        analyze_pattern_suitability_json,
        generate_pattern_svg_json,
    )
except ImportError:
    try:
        from vector_engine import (
            process_svg_outset,
            inkscape_cpp_simplify_svg,
            parse_svg_string,
            apply_design1_variant_json,
            apply_design2_variant_json,
            apply_silhouette_variant_json,
            scale_document_area_json,
            create_duo_board_json,
            analyze_pattern_suitability_json,
            generate_pattern_svg_json,
        )
    except ImportError:
        process_svg_outset = None

if process_svg_outset is None:
    print("CRITICAL ERROR: Native PyO3 extension could not be loaded.", file=sys.stderr)
    print("Please ensure the project is compiled with 'maturin develop --release'.", file=sys.stderr)
    sys.exit(1)


def run_inkscape_action_simplify(svg_path: Path, out_path: Path, simplify_tolerance: float = 0.2) -> str:
    """Executes native C++ Inkscape Livarot path simplify engine with CLI fallback."""
    try:
        content = svg_path.read_text()
        simplified_svg = inkscape_cpp_simplify_svg(content, simplify_tolerance)
        out_path.write_text(simplified_svg)
        return simplified_svg
    except Exception as e:
        cmd = [
            "inkscape",
            str(svg_path),
            "--actions=select-all;path-simplify",
            f"--export-filename={out_path}",
        ]
        try:
            subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            return out_path.read_text()
        except Exception:
            out_path.write_bytes(svg_path.read_bytes())
            return out_path.read_text()



CPU_WORKER_COUNT = max(4, os.cpu_count() or 4)
PARALLEL_EXECUTOR = ThreadPoolExecutor(max_workers=CPU_WORKER_COUNT)


def _run_inkscape_command(cmd: list[str]) -> None:
    env = os.environ.copy()
    num_threads = str(os.cpu_count() or 12)
    env["OMP_NUM_THREADS"] = num_threads
    env["OPENBLAS_NUM_THREADS"] = num_threads
    env["MKL_NUM_THREADS"] = num_threads
    env["VECLIB_MAXIMUM_THREADS"] = num_threads
    env["NUMEXPR_NUM_THREADS"] = num_threads
    subprocess.run(cmd, check=False, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=env)


def _render_jpg_via_resvg(svg_content: str, jpg_path: Path) -> bool:
    """Renders SVG string to high-resolution crisp JPG preview directly using resvg_py (in-memory) with white background composite."""
    try:
        import resvg_py
        from PIL import Image
        import io
        from vector_engine._native import scale_document_area_json

        try:
            preview_svg, _ = scale_document_area_json(svg_content, 3.0e6, 4.0e6)
        except Exception:
            preview_svg = svg_content

        png_bytes = resvg_py.svg_to_bytes(preview_svg)
        if png_bytes:
            Image.MAX_IMAGE_PIXELS = None
            with Image.open(io.BytesIO(png_bytes)) as img:
                if img.mode in ("RGBA", "LA") or (img.mode == "P" and "transparency" in img.info):
                    img_rgba = img.convert("RGBA")
                    bg = Image.new("RGB", img_rgba.size, (255, 255, 255))
                    bg.paste(img_rgba, mask=img_rgba.split()[3])
                    bg.save(jpg_path, "JPEG", quality=95)
                else:
                    img.convert("RGB").save(jpg_path, "JPEG", quality=95)
            return True
    except Exception as e:
        print(f"WARNING: Native resvg_py rendering failed ({e})", file=sys.stderr)
    return False


def _render_eps_via_native_rust(svg_content: str, eps_path: Path) -> bool:
    """Exports SVG content to EPS using native Rust EpsWriter in vector_engine._native."""
    try:
        from vector_engine._native import export_svg_to_eps_json
        eps_content = export_svg_to_eps_json(svg_content, eps_path.stem)
        if eps_content and len(eps_content) > 50:
            eps_path.write_text(eps_content, encoding="utf-8")
            return True
    except Exception as e:
        print(f"WARNING: Native Rust EPS export failed ({e})", file=sys.stderr)
    return False


def export_eps_and_jpg(scaled_svg_content: str, unscaled_svg_content: str, eps_path: Path, jpg_path: Path) -> tuple[Path, Path]:
    """Exports scaled SVG (20.5 MP) to EPS via Native Rust and unscaled SVG to crisp JPG via resvg_py in parallel."""
    fut_eps = PARALLEL_EXECUTOR.submit(_render_eps_via_native_rust, scaled_svg_content, eps_path)
    fut_jpg = PARALLEL_EXECUTOR.submit(_render_jpg_via_resvg, unscaled_svg_content, jpg_path)

    fut_eps.result()
    fut_jpg.result()

    return eps_path, jpg_path


def discover_svg_files(input_dir: Path) -> list[Path]:
    """Discovers all SVG files in the given directory or single file path."""
    if input_dir.is_file():
        if input_dir.suffix.lower() == ".svg":
            return [input_dir]
        return []
    elif input_dir.is_dir():
        return sorted([p for p in input_dir.glob("*.svg") if p.is_file()])
    return []


def hex_luminance(hex_str: str) -> float:
    """Calculates perceived relative luminance of a HEX color."""
    if not hex_str or not hex_str.startswith("#"):
        return 1.0
    clean = hex_str.lstrip("#")
    if len(clean) == 3:
        clean = "".join([c * 2 for c in clean])
    if len(clean) != 6:
        return 1.0
    r, g, b = int(clean[0:2], 16), int(clean[2:4], 16), int(clean[4:6], 16)
    return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0


def hex_to_rgb(hex_str: str) -> tuple[float, float, float]:
    """Converts a HEX color string to normalized RGB float tuple (0.0 to 1.0)."""
    if not hex_str or not hex_str.startswith("#"):
        return (1.0, 1.0, 1.0)
    clean = hex_str.lstrip("#")
    if len(clean) == 3:
        clean = "".join([c * 2 for c in clean])
    if len(clean) != 6:
        return (1.0, 1.0, 1.0)
    try:
        r = int(clean[0:2], 16) / 255.0
        g = int(clean[2:4], 16) / 255.0
        b = int(clean[4:6], 16) / 255.0
        return (r, g, b)
    except ValueError:
        return (1.0, 1.0, 1.0)


def hex_to_hsl(hex_str: str) -> tuple[float, float, float]:
    """Converts HEX color to HSL tuple (Hue: 0-360, Saturation: 0-1, Lightness: 0-1)."""
    r, g, b = hex_to_rgb(hex_str)
    max_c = max(r, g, b)
    min_c = min(r, g, b)
    l = (max_c + min_c) / 2.0

    if max_c == min_c:
        h = s = 0.0
    else:
        d = max_c - min_c
        s = d / (2.0 - max_c - min_c) if l > 0.5 else d / (max_c + min_c)
        if max_c == r:
            h = (g - b) / d + (6.0 if g < b else 0.0)
        elif max_c == g:
            h = (b - r) / d + 2.0
        else:
            h = (r - g) / d + 4.0
        h *= 60.0

    return (h, s, l)


def hsl_to_hex(h: float, s: float, l: float) -> str:
    """Converts HSL tuple (Hue: 0-360, Saturation: 0-1, Lightness: 0-1) to HEX string."""
    h = h % 360.0
    c = (1.0 - abs(2.0 * l - 1.0)) * s
    x = c * (1.0 - abs((h / 60.0) % 2 - 1.0))
    m = l - c / 2.0

    if 0 <= h < 60:
        r_p, g_p, b_p = c, x, 0
    elif 60 <= h < 120:
        r_p, g_p, b_p = x, c, 0
    elif 120 <= h < 180:
        r_p, g_p, b_p = 0, c, x
    elif 180 <= h < 240:
        r_p, g_p, b_p = 0, x, c
    elif 240 <= h < 300:
        r_p, g_p, b_p = x, 0, c
    else:
        r_p, g_p, b_p = c, 0, x

    r = max(0, min(255, int(round((r_p + m) * 255.0))))
    g = max(0, min(255, int(round((g_p + m) * 255.0))))
    b = max(0, min(255, int(round((b_p + m) * 255.0))))

    return f"#{r:02X}{g:02X}{b:02X}"


def normalize_hex_color(color_str: str) -> str:
    """Normalizes any color string (#fff, #FFFFFF, #ffffff, rgb(...)) to standard 7-char uppercase HEX (#RRGGBB)."""
    if not color_str or color_str == "none":
        return "none"
    s = color_str.strip().upper()
    if s.startswith("#"):
        clean = s.lstrip("#")
        if len(clean) == 3:
            clean = "".join([c * 2 for c in clean])
        if len(clean) >= 6:
            return f"#{clean[:6]}"
    if s.startswith("RGB"):
        try:
            parts = s.replace("RGB", "").replace("(", "").replace(")", "").split(",")
            r, g, b = int(parts[0].strip()), int(parts[1].strip()), int(parts[2].strip())
            return f"#{r:02X}{g:02X}{b:02X}"
        except Exception:
            pass
    return s


def find_dominant_color(paths: list[dict], cutoff: float = 0.95) -> tuple[float, float, str]:
    """Detects the true dominant colored fill from artwork paths (excluding background and dark neutrals)."""
    colored_counts = {}
    all_fg_counts = {}
    
    for p in paths:
        fill = p.get("fill")
        if fill and fill != "none":
            norm_fill = normalize_hex_color(fill)
            lum = hex_luminance(norm_fill)
            if lum < cutoff:  # Exclude background colors above threshold
                cmd_cnt = len(p.get("commands", [])) + 1
                all_fg_counts[norm_fill] = all_fg_counts.get(norm_fill, 0) + cmd_cnt
                
                h, s, l = hex_to_hsl(norm_fill)
                if s >= 0.15 and l >= 0.12:  # Must be a distinct non-neutral color
                    colored_counts[norm_fill] = colored_counts.get(norm_fill, 0) + cmd_cnt

    if colored_counts:
        dom_hex = max(colored_counts.keys(), key=lambda c: colored_counts[c])
        h, s, _ = hex_to_hsl(dom_hex)
        return (h, max(0.35, s), dom_hex)
        
    if all_fg_counts:
        dom_hex = max(all_fg_counts.keys(), key=lambda c: all_fg_counts[c])
        h, s, _ = hex_to_hsl(dom_hex)
        if s < 0.15:
            return (130.0, 0.55, dom_hex)  # Neutral fallback green hue if pure monochrome
        return (h, max(0.35, s), dom_hex)

    return (130.0, 0.55, "#2E7D32")


def calculate_layer_luminance(
    rank_idx: int,
    total_layers: int,
    min_luminance: float = 0.20,
    max_luminance: float = 0.62,
) -> float:
    """Calculates distinct luminance value for a layer rank spaced linearly between min_luminance (20%) and max_luminance (62%)."""
    if total_layers <= 1:
        return (min_luminance + max_luminance) / 2.0

    t = rank_idx / (total_layers - 1)  # Normalized 0.0 to 1.0
    return min_luminance + t * (max_luminance - min_luminance)


def apply_multi_shade_luminance_recoloring(
    svg_content: str,
    threshold: float = 0.95,
    mode: str = "grey",
    min_luminance: float = 0.20,
    max_luminance: float = 0.62,
) -> str:
    """Applies per-layer multi-shade luminance recoloring while preserving 100% of shapes and layers."""
    try:
        doc = json.loads(parse_svg_string(svg_content))
    except Exception as e:
        print("EXCEPTION IN PARSE_SVG_STRING:", e)
        return svg_content

    paths = doc.get("paths", [])
    if not paths:
        return svg_content

    # Calculate perceived luminance for each unique fill color
    color_lums = {}
    for p in paths:
        fill = p.get("fill")
        if fill and fill != "none":
            norm_fill = normalize_hex_color(fill)
            if norm_fill not in color_lums:
                color_lums[norm_fill] = hex_luminance(norm_fill)

    if not color_lums:
        return svg_content

    cutoff = threshold if 0.0 < threshold <= 1.0 else 0.95
    
    fg_colors = [c for c, lum in color_lums.items() if lum < cutoff]
    fg_colors.sort(key=lambda c: color_lums[c])  # Sort from darkest to lightest

    dom_h, dom_s, _ = find_dominant_color(paths, cutoff) if mode == "colored" else (0.0, 0.0, "")

    color_map = {}
    for hex_color, lum in color_lums.items():
        if lum >= cutoff:
            color_map[hex_color] = "#FFFFFF"

    total_fg = len(fg_colors)
    for idx, hex_color in enumerate(fg_colors):
        target_lum = calculate_layer_luminance(idx, total_fg, min_luminance, max_luminance)
        if mode == "grey":
            v = int(round(target_lum * 255.0))
            v = max(0, min(255, v))
            color_map[hex_color] = f"#{v:02X}{v:02X}{v:02X}"
        else:
            color_map[hex_color] = hsl_to_hex(dom_h, dom_s, target_lum)

    width = doc.get("width") or 1000.0
    height = doc.get("height") or 1000.0
    vb = doc.get("view_box")
    vb_str = f"{vb[0]} {vb[1]} {vb[2]} {vb[3]}" if vb else f"0 0 {width} {height}"

    lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="{vb_str}">',
    ]

    for idx, p in enumerate(paths):
        raw_fill = p.get("fill") or "#000000"
        norm_fill = normalize_hex_color(raw_fill)
        new_fill = color_map.get(norm_fill, norm_fill)
        stroke = p.get("stroke") or "none"
        stroke_w = p.get("stroke_width") or 1.0
        pid = p.get("id") or f"path_{idx}"

        d_parts = []
        for cmd in p.get("commands", []):
            if "MoveTo" in cmd:
                d_parts.append(f"M {cmd['MoveTo']['x']} {cmd['MoveTo']['y']}")
            elif "LineTo" in cmd:
                d_parts.append(f"L {cmd['LineTo']['x']} {cmd['LineTo']['y']}")
            elif "QuadTo" in cmd:
                c = cmd["QuadTo"]["control"]
                e = cmd["QuadTo"]["end"]
                d_parts.append(f"Q {c['x']} {c['y']} {e['x']} {e['y']}")
            elif "CubicTo" in cmd:
                c1 = cmd["CubicTo"]["control1"]
                c2 = cmd["CubicTo"]["control2"]
                e = cmd["CubicTo"]["end"]
                d_parts.append(f"C {c1['x']} {c1['y']} {c2['x']} {c2['y']} {e['x']} {e['y']}")
            elif cmd == "Close" or cmd == "close":
                d_parts.append("Z")

        if d_parts:
            d_str = " ".join(d_parts)
            lines.append(f'  <path id="{pid}" fill="{new_fill}" stroke="{stroke}" stroke-width="{stroke_w}" d="{d_str}"/>')

    lines.append("</svg>")
    return "\n".join(lines)


def are_colors_similar_hsl(
    hex1: str,
    hex2: str,
    max_hue_diff: float = 20.0,
    max_l_diff: float = 0.20,
    max_s_diff: float = 0.40,
) -> bool:
    """Checks if two HEX colors belong to the same color shade family using strict HSL constraints.

    Prevents merging distinct color spectrums (e.g. Green vs Blue or Red vs Yellow).
    """
    h1, s1, l1 = hex_to_hsl(hex1)
    h2, s2, l2 = hex_to_hsl(hex2)

    l_diff = abs(l1 - l2)
    if l_diff > max_l_diff:
        return False

    # Near white (>0.92) or near black (<0.08) immunity
    if (l1 > 0.92 and l2 > 0.92) or (l1 < 0.08 and l2 < 0.08):
        return l_diff <= max_l_diff

    s_diff = abs(s1 - s2)
    if s_diff > max_s_diff:
        return False

    raw_h_diff = abs(h1 - h2)
    hue_diff = min(raw_h_diff, 360.0 - raw_h_diff)
    return hue_diff <= max_hue_diff


def color_distance(hex1: str, hex2: str) -> float:
    """Calculates weighted perceptual RGB color distance (0.0 to 1.0)."""
    r1, g1, b1 = hex_to_rgb(hex1)
    r2, g2, b2 = hex_to_rgb(hex2)
    dist_sq = 0.30 * (r1 - r2) ** 2 + 0.59 * (g1 - g2) ** 2 + 0.11 * (b1 - b2) ** 2
    return dist_sq ** 0.5


def merge_similar_colors_in_svg(svg_content: str, threshold: float = 0.15) -> str:
    """Groups visually similar fill colors (strict HSL hue family matching) AND consolidates
    all sub-paths with identical fills into unified compound layers.
    """
    if threshold <= 0.0:
        return svg_content

    try:
        from vector_engine._native import parse_svg_string
        doc = json.loads(parse_svg_string(svg_content))
    except Exception:
        return svg_content

    paths = doc.get("paths", [])
    if not paths:
        return svg_content

    color_counts = {}
    for p in paths:
        fill = p.get("fill")
        if fill and fill != "none":
            color_counts[fill] = color_counts.get(fill, 0) + 1

    if not color_counts:
        return svg_content

    sorted_colors = sorted(color_counts.keys(), key=lambda c: color_counts[c], reverse=True)

    clusters: list[str] = []
    color_map: dict[str, str] = {}

    for c in sorted_colors:
        assigned = False
        for head in clusters:
            if are_colors_similar_hsl(c, head, max_hue_diff=20.0, max_l_diff=0.20, max_s_diff=0.40):
                color_map[c] = head
                assigned = True
                break
        if not assigned:
            clusters.append(c)
            color_map[c] = c

    badge_layers: dict[tuple, list[str]] = {}
    badge_order: list[tuple] = []

    for p in paths:
        fill = p.get("fill") or "#000000"
        merged_fill = color_map.get(fill, fill)
        b_id = p.get("badge_id")

        d_parts = []
        for cmd in p.get("commands", []):
            if "MoveTo" in cmd:
                d_parts.append(f"M {cmd['MoveTo']['x']} {cmd['MoveTo']['y']}")
            elif "LineTo" in cmd:
                d_parts.append(f"L {cmd['LineTo']['x']} {cmd['LineTo']['y']}")
            elif "QuadTo" in cmd:
                c = cmd["QuadTo"]["control"]
                e = cmd["QuadTo"]["end"]
                d_parts.append(f"Q {c['x']} {c['y']} {e['x']} {e['y']}")
            elif "CubicTo" in cmd:
                c1 = cmd["CubicTo"]["control1"]
                c2 = cmd["CubicTo"]["control2"]
                e = cmd["CubicTo"]["end"]
                d_parts.append(f"C {c1['x']} {c1['y']} {c2['x']} {c2['y']} {e['x']} {e['y']}")
            elif cmd == "Close" or cmd == "close":
                d_parts.append("Z")

        if d_parts:
            key = (b_id, merged_fill)
            if key not in badge_layers:
                badge_layers[key] = []
                badge_order.append(key)
            badge_layers[key].append(" ".join(d_parts))

    width = doc.get("width") or 1000.0
    height = doc.get("height") or 1000.0
    vb = doc.get("view_box")
    vb_str = f"{vb[0]} {vb[1]} {vb[2]} {vb[3]}" if vb else f"0 0 {width} {height}"

    lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="{vb_str}">',
    ]

    current_badge = None
    for idx, (b_id, fill) in enumerate(badge_order):
        if b_id != current_badge:
            if current_badge is not None:
                lines.append('  </g>')
            current_badge = b_id
            if current_badge is not None:
                lines.append(f'  <g id="badge_{current_badge}">')

        indent = '    ' if current_badge is not None else '  '
        combined_d = " ".join(badge_layers[(b_id, fill)])
        lines.append(f'{indent}<path id="layer_{idx}_{fill.lstrip("#")}" fill="{fill}" stroke="none" fill-rule="evenodd" d="{combined_d}"/>')

    if current_badge is not None:
        lines.append('  </g>')

    lines.append('</svg>')
    return "\n".join(lines)


def remove_specific_layers_from_svg(svg_content: str, remove_indexes: list[int], min_nodes: int) -> str:
    """Filters out paths belonging to specified color layer indexes or below min_nodes."""
    if not remove_indexes and min_nodes <= 0:
        return svg_content
    try:
        from vector_engine._native import parse_svg_string
        doc = json.loads(parse_svg_string(svg_content))
    except Exception:
        return svg_content

    paths = doc.get("paths", [])
    if not paths:
        return svg_content

    color_lums = {}
    for p in paths:
        fill = p.get("fill")
        if fill and fill != "none" and fill not in color_lums:
            color_lums[fill] = hex_luminance(fill)

    sorted_fills = sorted(color_lums.keys(), key=lambda c: color_lums[c])
    removed_fills = {
        sorted_fills[idx] 
        for idx in remove_indexes 
        if 0 <= idx < len(sorted_fills)
    }

    filtered_paths = []
    for p in paths:
        fill = p.get("fill")
        cmd_count = len(p.get("commands", []))
        lum = color_lums.get(fill, 1.0)
        if fill in removed_fills:
            continue
        if min_nodes > 0 and cmd_count < min_nodes and lum >= 0.15:
            continue
        filtered_paths.append(p)

    doc["paths"] = filtered_paths

    width = doc.get("width") or 1000.0
    height = doc.get("height") or 1000.0
    vb = doc.get("view_box")
    vb_str = f"{vb[0]} {vb[1]} {vb[2]} {vb[3]}" if vb else f"0 0 {width} {height}"

    lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="{vb_str}">',
    ]
    for p in filtered_paths:
        fill = p.get("fill") or "#000000"
        stroke = p.get("stroke") or "none"
        stroke_w = p.get("stroke_width") or 1.0
        pid = p.get("id") or "path"

        d_parts = []
        for cmd in p.get("commands", []):
            if "MoveTo" in cmd:
                d_parts.append(f"M {cmd['MoveTo']['x']} {cmd['MoveTo']['y']}")
            elif "LineTo" in cmd:
                d_parts.append(f"L {cmd['LineTo']['x']} {cmd['LineTo']['y']}")
            elif "QuadTo" in cmd:
                c = cmd["QuadTo"]["control"]
                e = cmd["QuadTo"]["end"]
                d_parts.append(f"Q {c['x']} {c['y']} {e['x']} {e['y']}")
            elif "CubicTo" in cmd:
                c1 = cmd["CubicTo"]["control1"]
                c2 = cmd["CubicTo"]["control2"]
                e = cmd["CubicTo"]["end"]
                d_parts.append(f"C {c1['x']} {c1['y']} {c2['x']} {c2['y']} {e['x']} {e['y']}")
            elif cmd == "Close" or cmd == "close":
                d_parts.append("Z")

        if d_parts:
            d_str = " ".join(d_parts)
            lines.append(f'  <path id="{pid}" fill="{fill}" stroke="{stroke}" stroke-width="{stroke_w}" d="{d_str}"/>')

    lines.append('</svg>')
    return "\n".join(lines)


def count_unique_svg_colors(svg_content: str) -> int:
    """Counts the number of unique fill colors in an SVG document."""
    try:
        from vector_engine._native import parse_svg_string
        doc = json.loads(parse_svg_string(svg_content))
        paths = doc.get("paths", [])
        unique_colors = {
            p.get("fill") for p in paths if p.get("fill") and p.get("fill") != "none"
        }
        return len(unique_colors)
    except Exception:
        import re
        fills = set(re.findall(r'fill=["\'](#?[a-zA-Z0-9]+)["\']', svg_content))
        fills.discard("none")
        return len(fills)


def synchronize_svg_dimensions(ref_svg: str, input_svg: str) -> str:
    """Extracts width, height, and viewBox from ref_svg and applies them to input_svg for 100% identical dimensions."""
    try:
        from vector_engine._native import parse_svg_string
        ref_doc = json.loads(parse_svg_string(ref_svg))
        ref_w = ref_doc.get("width") or 1000.0
        ref_h = ref_doc.get("height") or 1000.0
        ref_vb = ref_doc.get("view_box")
        ref_vb_str = f"{ref_vb[0]} {ref_vb[1]} {ref_vb[2]} {ref_vb[3]}" if ref_vb else f"0 0 {ref_w} {ref_h}"

        doc = json.loads(parse_svg_string(input_svg))
        paths = doc.get("paths", [])

        lines = [
            '<?xml version="1.0" encoding="UTF-8"?>',
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{ref_w}" height="{ref_h}" viewBox="{ref_vb_str}">',
        ]
        for p in paths:
            fill = p.get("fill") or "#000000"
            stroke = p.get("stroke") or "none"
            stroke_w = p.get("stroke_width") or 1.0
            pid = p.get("id") or "path"

            d_parts = []
            for cmd in p.get("commands", []):
                if "MoveTo" in cmd:
                    d_parts.append(f"M {cmd['MoveTo']['x']} {cmd['MoveTo']['y']}")
                elif "LineTo" in cmd:
                    d_parts.append(f"L {cmd['LineTo']['x']} {cmd['LineTo']['y']}")
                elif "QuadTo" in cmd:
                    c = cmd["QuadTo"]["control"]
                    e = cmd["QuadTo"]["end"]
                    d_parts.append(f"Q {c['x']} {c['y']} {e['x']} {e['y']}")
                elif "CubicTo" in cmd:
                    c1 = cmd["CubicTo"]["control1"]
                    c2 = cmd["CubicTo"]["control2"]
                    e = cmd["CubicTo"]["end"]
                    d_parts.append(f"C {c1['x']} {c1['y']} {c2['x']} {c2['y']} {e['x']} {e['y']}")
                elif cmd == "Close" or cmd == "close":
                    d_parts.append("Z")

            if d_parts:
                d_str = " ".join(d_parts)
                lines.append(f'  <path id="{pid}" fill="{fill}" stroke="{stroke}" stroke-width="{stroke_w}" d="{d_str}"/>')

        lines.append('</svg>')
        return "\n".join(lines)
    except Exception:
        return input_svg


def center_svg_artwork_content(svg_content: str, padding: float = 20.0) -> str:
    """Ensures panel canvas (W, H) fits artwork bounding box plus padding, centers artwork inside canvas, and fills background rectangle to full canvas bounds so NO geometry is clipped."""
    try:
        from vector_engine._native import parse_svg_string
        doc = json.loads(parse_svg_string(svg_content))
        w = float(doc.get("width") or 1000.0)
        h = float(doc.get("height") or 1000.0)
        vb = doc.get("view_box")
        if vb:
            w = max(w, float(vb[2] - vb[0]))
            h = max(h, float(vb[3] - vb[1]))

        paths = doc.get("paths", [])
        if not paths:
            return svg_content

        bg_idx = None
        fg_paths = []

        for idx, p in enumerate(paths):
            pid = (p.get("id") or "").lower()
            gid = p.get("group_id")
            if gid == 9223372036854775807 or "background" in pid or "canvas_bg" in pid or (idx == 0 and len(p.get("commands", [])) <= 5):
                if bg_idx is None:
                    bg_idx = idx
                else:
                    fg_paths.append(p)
            else:
                fg_paths.append(p)

        if not fg_paths:
            fg_paths = paths
            bg_idx = None

        min_x, min_y, max_x, max_y = float("inf"), float("inf"), float("-inf"), float("-inf")
        for p in fg_paths:
            for cmd in p.get("commands", []):
                pt = None
                if "MoveTo" in cmd: pt = cmd["MoveTo"]
                elif "LineTo" in cmd: pt = cmd["LineTo"]
                elif "QuadTo" in cmd: pt = cmd["QuadTo"]["end"]
                elif "CubicTo" in cmd: pt = cmd["CubicTo"]["end"]
                if pt:
                    min_x = min(min_x, float(pt["x"]))
                    min_y = min(min_y, float(pt["y"]))
                    max_x = max(max_x, float(pt["x"]))
                    max_y = max(max_y, float(pt["y"]))

        if min_x == float("inf") or max_x <= min_x or max_y <= min_y:
            return svg_content

        art_w = max_x - min_x
        art_h = max_y - min_y

        # Auto-expand panel canvas if artwork bounds exceed canvas dimensions
        final_w = max(w, art_w + 2.0 * padding)
        final_h = max(h, art_h + 2.0 * padding)

        dx = ((final_w - art_w) / 2.0) - min_x
        dy = ((final_h - art_h) / 2.0) - min_y

        vb_str = f"0 0 {final_w} {final_h}"
        lines = [
            '<?xml version="1.0" encoding="UTF-8"?>',
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{final_w}" height="{final_h}" viewBox="{vb_str}">',
        ]

        if bg_idx is not None:
            bg_path = paths[bg_idx]
            bg_fill = bg_path.get("fill") or "#FFFFFF"
            bg_pid = bg_path.get("id") or "normalized_background"
            lines.append(f'  <path id="{bg_pid}" fill="{bg_fill}" d="M 0 0 L {final_w} 0 L {final_w} {final_h} L 0 {final_h} Z"/>')

        current_badge = None
        for idx, p in enumerate(paths):
            if idx == bg_idx:
                continue
            b_id = p.get("badge_id")
            if b_id != current_badge:
                if current_badge is not None:
                    lines.append('  </g>')
                current_badge = b_id
                if current_badge is not None:
                    lines.append(f'  <g id="badge_{current_badge}">')

            fill_attr = f' fill="{p["fill"]}"' if p.get("fill") else ''
            stroke_attr = f' stroke="{p["stroke"]}"' if p.get("stroke") else ''
            stroke_w_attr = f' stroke-width="{p["stroke_width"]}"' if p.get("stroke_width") else ''
            pid = p.get("id") or "path"

            d_parts = []
            for cmd in p.get("commands", []):
                if "MoveTo" in cmd:
                    d_parts.append(f"M {cmd['MoveTo']['x'] + dx} {cmd['MoveTo']['y'] + dy}")
                elif "LineTo" in cmd:
                    d_parts.append(f"L {cmd['LineTo']['x'] + dx} {cmd['LineTo']['y'] + dy}")
                elif "QuadTo" in cmd:
                    c = cmd["QuadTo"]["control"]
                    e = cmd["QuadTo"]["end"]
                    d_parts.append(f"Q {c['x'] + dx} {c['y'] + dy} {e['x'] + dx} {e['y'] + dy}")
                elif "CubicTo" in cmd:
                    c1 = cmd["CubicTo"]["control1"]
                    c2 = cmd["CubicTo"]["control2"]
                    e = cmd["CubicTo"]["end"]
                    d_parts.append(f"C {c1['x'] + dx} {c1['y'] + dy} {c2['x'] + dx} {c2['y'] + dy} {e['x'] + dx} {e['y'] + dy}")
                elif cmd == "Close" or cmd == "close":
                    d_parts.append("Z")

            if d_parts:
                d_str = " ".join(d_parts)
                indent = "    " if current_badge is not None else "  "
                lines.append(f'{indent}<path id="{pid}"{fill_attr}{stroke_attr}{stroke_w_attr} d="{d_str}"/>')

        if current_badge is not None:
            lines.append('  </g>')

        lines.append('</svg>')
        return "\n".join(lines)
    except Exception:
        return svg_content


def select_config_profile(config_data: dict, color_count: int) -> tuple[str, dict]:
    """Selects matching config profile ('low_color_bad_two' vs 'multi_color_bad_two') based on color count."""
    if not config_data:
        return "default", {}
    
    low_cfg = config_data.get("low_color_bad_two", {})
    multi_cfg = config_data.get("multi_color_bad_two", {})

    max_colors = low_cfg.get("max_colors", 5)
    min_colors = multi_cfg.get("min_colors", 6)

    if low_cfg and color_count <= max_colors:
        return "low_color_bad_two", low_cfg
    elif multi_cfg and color_count >= min_colors:
        return "multi_color_bad_two", multi_cfg
    
    if "bad_two" in config_data:
        return "bad_two", config_data["bad_two"]
    if "default" in config_data:
        return "default", config_data["default"]
    
    if low_cfg:
        return "low_color_bad_two", low_cfg
    if multi_cfg:
        return "multi_color_bad_two", multi_cfg

    return "default", {}


DEFAULT_SILHOUETTE_THRESHOLD = 0.50

def run_batch_queue(
    input_path: Path, 
    output_path: Path, 
    distance_override: float = None,
    enable_outset: bool = ENABLE_OUTSET,
    enable_simplify: bool = ENABLE_SIMPLIFY,
    simplify_tolerance_override: float = None,
    silhouette_threshold_override: float = None,
    remove_layers_override: list[int] = None,
    min_nodes_override: int = None,
    line_art_override: bool = None,
    bg_color_override: str = None,
    fg_color_override: str = None,
    invert_override: bool = None,
    stroke_width_override: float = None,
    line_art_style_override: str = None,
    gap_override: float = None,
    min_area_override: float = None,
    max_area_override: float = None,
    config_data: dict = None,
    mode_single: bool = False,
    mode_double: bool = False,
    mode_triple: bool = False,
    mode_pattern: bool = False,
    style_override: str = None,
    no_emoji: bool = False,
) -> None:
    """Processes SVG files in a sequential fault-tolerant multi-phase pipeline with adaptive profile selection."""
    if config_data is None:
        config_data = {}

    ic_engine = "🚀" if not no_emoji else "[ENGINE]"
    ic_input = "📁" if not no_emoji else "[INPUT]"
    ic_output = "📂" if not no_emoji else "[OUTPUT]"
    ic_prof = "⚙️" if not no_emoji else "[PROFILE]"
    ic_canvas = "📏" if not no_emoji else "[CANVAS]"
    ic_file = "📄" if not no_emoji else "[FILE]"
    ic_p1 = "🌀" if not no_emoji else "[P1]"
    ic_p2 = "⚡" if not no_emoji else "[P2]"
    ic_p3 = "🎨" if not no_emoji else "[P3]"
    ic_p4 = "📐" if not no_emoji else "[P4]"
    ic_p5 = "🖼️" if not no_emoji else "[P5]"
    ic_p7 = "🧩" if not no_emoji else "[P7]"
    ic_p6 = "💾" if not no_emoji else "[P6]"
    ic_time = "⏱️" if not no_emoji else "[TIME]"
    ic_summary = "📊" if not no_emoji else "[SUMMARY]"
    ic_bottleneck_tag = "🔴 [PRIMARY BOTTLENECK]" if not no_emoji else "[*] [PRIMARY BOTTLENECK]"
    ic_success = "✅ SUCCESS" if not no_emoji else "[OK] SUCCESS"
    ic_failed = "❌ FAILED" if not no_emoji else "[FAIL] FAILED"

    print("================================================================================")
    print(f" {ic_engine} Vector Geometry Engine - Multi-Phase Batch Queue Pipeline")
    print("================================================================================")
    print(f" {ic_input} Input Directory  : {input_path}")
    print(f" {ic_output} Output Directory : {output_path}")
    low_cfg = config_data.get("low_color_bad_two", {})
    multi_cfg = config_data.get("multi_color_bad_two", {})
    max_c = low_cfg.get("max_colors", 5)
    min_c = multi_cfg.get("min_colors", 6)
    print(f" {ic_prof} Adaptive Profiles: 'low_color_bad_two' (<={max_c} colors) | 'multi_color_bad_two' (>={min_c} colors)")
    print(f" {ic_canvas} Target Canvas    : {MIN_CANVAS_AREA_PX:,.0f} px to {MAX_CANVAS_AREA_PX:,.0f} px total area")
    print("================================================================================\n")

    output_path.mkdir(parents=True, exist_ok=True)
    RAM_TEMP_DIR.mkdir(parents=True, exist_ok=True)

    svg_files = discover_svg_files(input_path)
    if not svg_files:
        print(f"⚠️ WARNING: No SVG files found at input path: {input_path}")
        return

    print(f" Found {len(svg_files)} SVG file(s) in queue to process...\n")

    success_count = 0
    failure_count = 0
    total_start_time = time.time()
    results_table = []

    try:
        for idx, svg_file in enumerate(svg_files, 1):
            rel_name = svg_file.name
            stem = svg_file.stem
            file_start_time = time.time()

            # Count unique fill colors for adaptive profile selection
            raw_input_svg = svg_file.read_text()
            color_count = count_unique_svg_colors(raw_input_svg)
            prof_name, prof_cfg = select_config_profile(config_data, color_count)

            # Resolve parameters for this specific file
            distance = distance_override if distance_override is not None else prof_cfg.get("distance", 0.5)
            simplify_tolerance = simplify_tolerance_override if simplify_tolerance_override is not None else prof_cfg.get("tolerance", 0.20)
            silhouette_threshold = silhouette_threshold_override if silhouette_threshold_override is not None else prof_cfg.get("threshold", 0.50)
            color_merge_threshold = prof_cfg.get("color_merge_threshold", 0.0)
            remove_layers = remove_layers_override if remove_layers_override is not None else prof_cfg.get("remove_layers", [])
            min_nodes = min_nodes_override if min_nodes_override is not None else prof_cfg.get("min_nodes", 0)
            line_art = line_art_override if line_art_override is not None else prof_cfg.get("line_art", False)
            bg_color = bg_color_override if bg_color_override is not None else prof_cfg.get("bg_color", "#FFFFFF")
            fg_color = fg_color_override if fg_color_override is not None else prof_cfg.get("fg_color", "#000000")
            bg_color_3 = prof_cfg.get("bg_color_3", "#D8F3DC" if prof_name == "multi_color_bad_two" else None)
            fg_color_3 = prof_cfg.get("fg_color_3", "#1E7B34" if prof_name == "multi_color_bad_two" else None)
            invert = invert_override if invert_override is not None else prof_cfg.get("invert", False)
            gap = gap_override if gap_override is not None else prof_cfg.get("gap", 50.0)
            min_area = min_area_override if min_area_override is not None else prof_cfg.get("min_area_px", MIN_CANVAS_AREA_PX)
            max_area = max_area_override if max_area_override is not None else prof_cfg.get("max_area_px", MAX_CANVAS_AREA_PX)
            bg_already_removed = bool(remove_layers)

            print(f"────────────────────────────────────────────────────────────────────────────────")
            print(f" {ic_file} [{idx}/{len(svg_files)}] Processing File: {rel_name} ({color_count} Colors | Profile: '{prof_name}')")
            print(f"────────────────────────────────────────────────────────────────────────────────")

            deliverables = []

            try:
                # ------------------------------------------------------------------
                # Phase 1: Geometry Outset & Color Combiner (in RAM)
                # ------------------------------------------------------------------
                p1_start = time.time()
                pristine_input_svg = svg_file.read_text(encoding="utf-8")
                raw_outset_path = RAM_TEMP_DIR / f"{stem}_outset_{idx}.svg"
                raw_outset_svg_content = process_svg_outset(
                    str(svg_file), 
                    str(raw_outset_path), 
                    False, 
                    0.0, 
                    False, 
                    0.0
                )
                p1_sec = time.time() - p1_start
                print(f"  {ic_p1} Phase 1 (Outset & Combine Geometry in RAM)  : {p1_sec:>6.2f} s")

                # Detect if SVG is a Badge Grid
                is_badge_svg = '<g id="badge_' in raw_outset_svg_content or 'badge_1' in raw_outset_svg_content or '<g id="badge_' in pristine_input_svg or 'badge_1' in pristine_input_svg

                if is_badge_svg:
                    # 100% Pure Rust Pipeline for Badge SVGs: Bypass Inkscape CLI simplify to preserve pristine fonts & colors
                    p2_sec = 0.0
                    print(f"  {ic_p2} Phase 2 (Livarot Simplification Bypassed) :   0.00 s (Preserving Pristine Fonts)")
                    if raw_outset_path.exists():
                        raw_outset_path.unlink()

                    p3_start = time.time()
                    original_svg = raw_outset_svg_content
                    sil_input_svg = original_svg
                    silhouette_svg = original_svg
                    panel3_svg = None
                    p3_sec = time.time() - p3_start
                    print(f"  {ic_p3} Phase 3 (Color Isolation & Silhouette)      : {p3_sec:>6.2f} s")

                    print(f"  [INFO] SVG Badge Grid layout detected -> Forcing Single mode export ONLY (_single.eps, _single.jpg & _single.svg)")
                    do_single = True
                    do_double = False
                    do_triple = False
                    do_pattern = False
                else:
                    # Standard Multi-Phase Pipeline for Non-Badge SVGs
                    if enable_outset or enable_simplify:
                        process_svg_outset(
                            str(svg_file), 
                            str(raw_outset_path), 
                            enable_outset, 
                            distance, 
                            enable_simplify, 
                            simplify_tolerance
                        )

                    p2_start = time.time()
                    simplified_path = RAM_TEMP_DIR / f"{stem}_simplified_{idx}.svg"
                    raw_outset_svg_content = run_inkscape_action_simplify(raw_outset_path, simplified_path, simplify_tolerance)
                    p2_sec = time.time() - p2_start
                    print(f"  {ic_p2} Phase 2 (Livarot Node Simplification)        : {p2_sec:>6.2f} s")

                    if raw_outset_path.exists():
                        raw_outset_path.unlink()
                    if simplified_path.exists():
                        simplified_path.unlink()

                    p3_start = time.time()
                    original_svg = raw_outset_svg_content
                    if color_merge_threshold > 0.0:
                        original_svg = merge_similar_colors_in_svg(original_svg, color_merge_threshold)

                    sil_input_svg = original_svg
                    if remove_layers or min_nodes > 0:
                        sil_input_svg = remove_specific_layers_from_svg(sil_input_svg, remove_layers, min_nodes)

                    if prof_name == "multi_color_bad_two":
                        mid_cfg = prof_cfg.get("middle_panel", {})
                        right_cfg = prof_cfg.get("right_panel", {})

                        mid_min_lum = float(mid_cfg.get("min_luminance", 0.20))
                        mid_max_lum = float(mid_cfg.get("max_luminance", 0.62))

                        right_min_lum = float(right_cfg.get("min_luminance", 0.20))
                        right_max_lum = float(right_cfg.get("max_luminance", 0.62))

                        silhouette_svg = apply_multi_shade_luminance_recoloring(
                            sil_input_svg,
                            threshold=silhouette_threshold,
                            mode="grey",
                            min_luminance=mid_min_lum,
                            max_luminance=mid_max_lum,
                        )
                        panel3_svg = apply_multi_shade_luminance_recoloring(
                            sil_input_svg,
                            threshold=silhouette_threshold,
                            mode="colored",
                            min_luminance=right_min_lum,
                            max_luminance=right_max_lum,
                        )
                    else:
                        silhouette_svg = apply_silhouette_variant_json(
                            sil_input_svg,
                            silhouette_threshold,
                            line_art,
                            bg_color,
                            fg_color,
                            invert,
                            bg_already_removed
                        )
                        panel3_svg = None
                        if bg_color_3 and fg_color_3:
                            panel3_svg = apply_silhouette_variant_json(
                                sil_input_svg,
                                silhouette_threshold,
                                line_art,
                                bg_color_3,
                                fg_color_3,
                                invert,
                                bg_already_removed
                            )
                    p3_sec = time.time() - p3_start
                    print(f"  {ic_p3} Phase 3 (Color Isolation & Silhouette)      : {p3_sec:>6.2f} s")

                    mode_specified = mode_single or mode_double or mode_triple or mode_pattern
                    do_single = mode_single
                    do_double = mode_double or (not mode_specified)
                    do_triple = mode_triple
                    do_pattern = mode_pattern or (not mode_specified)

                export_queue = []  # List of (svg_content_str, suffix_name)

                # ------------------------------------------------------------------
                # Phase 4: Bad Board Generation (--double / --triple)
                # ------------------------------------------------------------------
                p4_sec = 0.0
                if do_double or do_triple:
                    p4_start = time.time()

                    effective_gap = gap
                    if gap_override is None:
                        try:
                            ref_doc_info = json.loads(parse_svg_string(original_svg))
                            orig_w = float(ref_doc_info.get("width") or 0.0)
                            orig_h = float(ref_doc_info.get("height") or 0.0)
                            if orig_w > orig_h and orig_h > 0:
                                aspect_ratio = orig_w / orig_h
                                effective_gap = max(5.0, gap / aspect_ratio)
                        except Exception:
                            effective_gap = gap

                    if (do_triple or panel3_svg is not None) and panel3_svg is not None:
                        duo_1_2 = create_duo_board_json(original_svg, silhouette_svg, gap=effective_gap)
                        board_raw_svg = create_duo_board_json(duo_1_2, panel3_svg, gap=effective_gap)
                        board_suffix = "bad_three"
                    else:
                        board_raw_svg = create_duo_board_json(original_svg, silhouette_svg, gap=effective_gap)
                        board_suffix = "bad_two"

                    # Scale combined board document 1x at the end
                    board_scaled_svg, _ = scale_document_area_json(board_raw_svg, min_area, max_area)

                    p4_sec = time.time() - p4_start
                    print(f"  {ic_p5} Phase 4 (Bad Board Generation: {board_suffix})   : {p4_sec:>6.2f} s")
                    export_queue.append((board_scaled_svg, board_raw_svg, board_suffix))

                # ------------------------------------------------------------------
                # Phase 5: Seamless Pattern Generation & Export (--pattern)
                # ------------------------------------------------------------------
                pattern_config = prof_cfg.get("pattern_config", {})
                pattern_enabled_config = pattern_config.get("enable", True)
                style_used = "none"
                p5_sec = 0.0
                if do_pattern and (pattern_enabled_config or mode_pattern):
                    p5_start = time.time()
                    try:
                        from vector_engine._native import analyze_pattern_suitability_json, generate_pattern_styled_json
                        report_raw = analyze_pattern_suitability_json(original_svg)
                        report = json.loads(report_raw)
                        detected_shape = report.get("detected_shape", "other")
                        allowed_shapes = pattern_config.get("allowed_shapes", ["circular", "square"])
                        detect_suitable_only = pattern_config.get("detect_suitable_only", True)
                        if mode_pattern:
                            detect_suitable_only = False

                        is_approved = not detect_suitable_only or (report.get("is_suitable", False) and (detected_shape in allowed_shapes or report.get("is_circular_shape") or report.get("is_square_shape")))

                        if is_approved:
                            grid_x = pattern_config.get("grid_x", 3)
                            grid_y = pattern_config.get("grid_y", 3)
                            style_used = style_override or pattern_config.get("style", "slanted_left")
                            slant_ratio = pattern_config.get("slant_offset_ratio", -0.5)
                            wraparound = pattern_config.get("wraparound_seamless", True)

                            total_tiles = max(1, grid_x * grid_y)
                            target_tile_min_area = min_area / float(total_tiles)
                            target_tile_max_area = max_area / float(total_tiles)

                            pattern_base_scaled_svg, _ = scale_document_area_json(
                                original_svg, target_tile_min_area, target_tile_max_area
                            )
                            pattern_base_scaled_svg = center_svg_artwork_content(pattern_base_scaled_svg)

                            pattern_svg = generate_pattern_styled_json(
                                pattern_base_scaled_svg,
                                grid_x=grid_x,
                                grid_y=grid_y,
                                style=style_used,
                                detect_suitable_only=False,
                                slant_offset_ratio=slant_ratio,
                                wraparound_seamless=wraparound
                            )

                            # Auto-fit scale combined pattern canvas to target stock pixel area [min_area, max_area]
                            pattern_scaled_svg, _ = scale_document_area_json(pattern_svg, min_area, max_area)

                            p5_sec = time.time() - p5_start
                            print(f"  {ic_p7} Phase 5 (Seamless Pattern: style='{style_used}')     : {p5_sec:>6.2f} s")
                            export_queue.append((pattern_scaled_svg, pattern_svg, "pattern"))
                        else:
                            p5_sec = time.time() - p5_start
                            print(f"  {ic_p7} Phase 5 (Pattern Skipped: shape='{detected_shape}') : {p5_sec:>6.2f} s")
                    except Exception as pe:
                        p5_sec = time.time() - p5_start
                        print(f"  {ic_p7} Phase 5 Warning (Pattern skipped: {pe})")

                # ------------------------------------------------------------------
                # Phase 6: Canvas Auto-Scale 1x Target Standardisation (at the end)
                # ------------------------------------------------------------------
                p6_start = time.time()
                if do_single:
                    svg_for_single = raw_outset_svg_content if is_badge_svg else (pristine_input_svg if mode_single else original_svg)
                    original_scaled_raw, _ = scale_document_area_json(svg_for_single, min_area, max_area)
                    if is_badge_svg or mode_single:
                        export_queue.append((original_scaled_raw, svg_for_single, "single"))
                    else:
                        original_scaled_svg = center_svg_artwork_content(original_scaled_raw)
                        export_queue.append((original_scaled_svg, svg_for_single, "single"))
                p6_sec = time.time() - p6_start
                print(f"  {ic_p4} Phase 6 (Canvas Auto-Scale 1x Target)      : {p6_sec:>6.2f} s")

                # ------------------------------------------------------------------
                # Phase 7: Disk Export EPS & JPG Render
                # ------------------------------------------------------------------
                p7_start = time.time()
                for scaled_svg, unscaled_svg, suffix in export_queue:
                    final_eps = output_path / f"{stem}_{suffix}.eps"
                    final_jpg = output_path / f"{stem}_{suffix}.jpg"
                    if suffix == "single":
                        final_svg = output_path / f"{stem}_{suffix}.svg"
                        final_svg.write_text(unscaled_svg, encoding="utf-8")
                        export_eps_and_jpg(scaled_svg, unscaled_svg, final_eps, final_jpg)
                        deliverables.append(f"{suffix}.eps, {suffix}.jpg, {suffix}.svg")
                    else:
                        export_eps_and_jpg(scaled_svg, unscaled_svg, final_eps, final_jpg)
                        deliverables.append(f"{suffix}.eps, {suffix}.jpg")

                p7_sec = time.time() - p7_start
                print(f"  {ic_p6} Phase 7 (Disk Export EPS & JPG Render)      : {p7_sec:>6.2f} s")
                file_elapsed_sec = time.time() - file_start_time

                # Detailed Telemetry & Bottleneck Analysis Logging
                phase_times = [
                    (f"{ic_p1} Phase 1: Outset & Combine Geometry", p1_sec),
                    (f"{ic_p2} Phase 2: Livarot Node Simplification", p2_sec),
                    (f"{ic_p3} Phase 3: Color Isolation & Silhouette", p3_sec),
                ]
                if do_double or do_triple:
                    phase_times.append((f"{ic_p5} Phase 4: Bad Board Generation", p4_sec))
                if do_pattern and p5_sec > 0:
                    phase_times.append((f"{ic_p7} Phase 5: Seamless Pattern Generation", p5_sec))
                phase_times.append((f"{ic_p4} Phase 6: Canvas Auto-Scale 1x Target", p6_sec))
                phase_times.append((f"{ic_p6} Phase 7: Disk Export EPS & JPG Render", p7_sec))

                max_phase_name, max_phase_time = max(phase_times, key=lambda x: x[1])

                print(f"\n  --------------------------------------------------------------------------------")
                print(f"  {ic_time} PHASE TIMING & BOTTLENECK SUMMARY ({rel_name})")
                print(f"  --------------------------------------------------------------------------------")
                for name, t_sec in phase_times:
                    pct = (t_sec / file_elapsed_sec * 100.0) if file_elapsed_sec > 0 else 0.0
                    bottleneck_tag = f" {ic_bottleneck_tag}" if name == max_phase_name else ""
                    print(f"   {name:<44} : {t_sec:>6.2f} s  ({pct:>5.1f}%){bottleneck_tag}")
                print(f"  --------------------------------------------------------------------------------")
                deliv_str = ", ".join(deliverables) if deliverables else "None"
                print(f"  Finished {rel_name} in {file_elapsed_sec:.2f} s | Bottleneck: {max_phase_name} ({max_phase_time:.2f} s)\n")
                results_table.append((rel_name, f"{file_elapsed_sec:.2f} s", deliv_str, ic_success))
                success_count += 1

            except Exception as e:
                file_elapsed_sec = time.time() - file_start_time
                print(f"  {ic_failed}: Failed to process '{rel_name}' ({file_elapsed_sec:.2f} s)", file=sys.stderr)
                print(f"     Details: {e}", file=sys.stderr)
                results_table.append((rel_name, f"{file_elapsed_sec:.2f} s", "None", ic_failed))
                failure_count += 1
                print("  --> Safely proceeding to next file in queue...\n")
                continue
    finally:
        # Automatic cleanup of RAM disk work directory
        if RAM_TEMP_DIR.exists():
            try:
                shutil.rmtree(RAM_TEMP_DIR)
            except Exception as e:
                pass

    total_elapsed = time.time() - total_start_time

    print("\n================================================================================")
    print(f" {ic_summary} BATCH QUEUE SUMMARY REPORT")
    print("================================================================================")
    print(f" {'File Name':<32} | {'Time (s)':<9} | {'Output Deliverables':<26} | {'Status'}")
    print("-" * 85)
    for fname, ftime, fouts, fstatus in results_table:
        print(f" {fname:<32} | {ftime:>9} | {fouts:<26} | {fstatus}")
    print("-" * 85)
    print(f" {ic_time} Total Queue Time : {total_elapsed:.2f} s")
    print(f" Total Processed   : {len(svg_files)} File(s) ({success_count} Successful, {failure_count} Failed)")
    print("================================================================================\n")


def json_to_svg_string(doc_struct: dict) -> str:
    """Converts a JSON-serialized VectorDocument struct back into SVG string format."""
    width = doc_struct.get("width") or 1000.0
    height = doc_struct.get("height") or 1000.0
    vb = doc_struct.get("view_box")
    vb_str = f"{vb[0]} {vb[1]} {vb[2]} {vb[3]}" if vb else f"0 0 {width} {height}"

    lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="{vb_str}">',
    ]

    current_badge = None
    for path in doc_struct.get("paths", []):
        b_id = path.get("badge_id")
        if b_id != current_badge:
            if current_badge is not None:
                lines.append('  </g>')
            current_badge = b_id
            if current_badge is not None:
                lines.append(f'  <g id="badge_{current_badge}">')

        fill = path.get("fill") or "none"
        stroke = path.get("stroke") or "none"
        stroke_width = path.get("stroke_width") or 1.0
        path_id = path.get("id") or "path"

        # Build path d command string
        d_parts = []
        for cmd in path.get("commands", []):
            if "MoveTo" in cmd:
                p = cmd["MoveTo"]
                d_parts.append(f"M {p['x']} {p['y']}")
            elif "LineTo" in cmd:
                p = cmd["LineTo"]
                d_parts.append(f"L {p['x']} {p['y']}")
            elif "QuadTo" in cmd:
                c = cmd["QuadTo"]["control"]
                e = cmd["QuadTo"]["end"]
                d_parts.append(f"Q {c['x']} {c['y']} {e['x']} {e['y']}")
            elif "CubicTo" in cmd:
                c1 = cmd["CubicTo"]["control1"]
                c2 = cmd["CubicTo"]["control2"]
                e = cmd["CubicTo"]["end"]
                d_parts.append(f"C {c1['x']} {c1['y']} {c2['x']} {c2['y']} {e['x']} {e['y']}")
            elif cmd == "Close" or cmd == "close":
                d_parts.append("Z")

        d_str = " ".join(d_parts)
        indent = "    " if current_badge is not None else "  "
        lines.append(f'{indent}<path id="{path_id}" fill="{fill}" stroke="{stroke}" stroke-width="{stroke_width}" d="{d_str}"/>')

    if current_badge is not None:
        lines.append('  </g>')

    lines.append('</svg>')
    return "\n".join(lines)


def load_config_file(config_path: Path) -> dict:
    """Loads calibration parameters dictionary from config_silhouette.json if present (supports # comment lines)."""
    if not config_path or not config_path.exists():
        return {}
    try:
        content = config_path.read_text()
        clean_lines = [
            line for line in content.splitlines()
            if not line.strip().startswith("#") and not line.strip().startswith("//")
        ]
        return json.loads("\n".join(clean_lines))
    except Exception as e:
        print(f"WARNING: Could not parse config JSON file '{config_path}': {e}", file=sys.stderr)
        return {}


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Vector Geometry Engine - Multi-Phase Python Batch Queue Processor"
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
        "-c", "--config",
        type=Path,
        default=Path("config_silhouette.json"),
        help="Path to calibration JSON config file (default: config_silhouette.json)"
    )
    parser.add_argument(
        "-d", "--distance",
        type=float,
        default=None,
        help="Outset distance in pixels (overrides config)"
    )
    parser.add_argument(
        "-t", "--silhouette-threshold",
        type=float,
        default=None,
        help="Silhouette lightness threshold (0.0 to 1.0, overrides config)"
    )
    parser.add_argument(
        "--tolerance",
        type=float,
        default=None,
        help="Path simplify node tolerance (overrides config)"
    )
    parser.add_argument(
        "-r", "--remove-layers",
        type=str,
        default=None,
        help="Comma-separated list of color layer indexes to remove (e.g. '0' or '0,2', overrides config)"
    )
    parser.add_argument(
        "--min-nodes",
        type=int,
        default=None,
        help="Minimum path node count threshold (overrides config)"
    )
    parser.add_argument(
        "--line-art",
        action="store_true",
        default=None,
        help="Enable solid line art mode (all artwork paths solid fg color)"
    )
    parser.add_argument(
        "--no-line-art",
        action="store_false",
        dest="line_art",
        help="Disable line art mode (use adaptive thresholding)"
    )
    parser.add_argument(
        "--bg-color",
        type=str,
        default=None,
        help="Background HEX color for silhouette (e.g. #FFFFFF)"
    )
    parser.add_argument(
        "--fg-color",
        type=str,
        default=None,
        help="Foreground HEX color for silhouette/line art (e.g. #000000)"
    )
    parser.add_argument(
        "--invert",
        action="store_true",
        default=None,
        help="Invert silhouette background and foreground colors"
    )
    parser.add_argument(
        "--gap",
        type=float,
        default=None,
        help="Duo board gap distance in pixels (default: 50.0)"
    )
    parser.add_argument(
        "--min-area",
        type=float,
        default=None,
        help="Target canvas minimum area in pixels (default: 19000000)"
    )
    parser.add_argument(
        "--max-area",
        type=float,
        default=None,
        help="Target canvas maximum area in pixels (default: 22000000)"
    )

    parser.add_argument(
        "--single",
        action="store_true",
        default=False,
        help="Export single artwork design ONLY (single.eps, single.jpg, single.svg)"
    )
    parser.add_argument(
        "--double", "--bad-two",
        action="store_true",
        dest="double",
        default=False,
        help="Export 2-panel bad board ONLY (bad_two.eps, bad_two.jpg)"
    )
    parser.add_argument(
        "--triple", "--bad-three",
        action="store_true",
        dest="triple",
        default=False,
        help="Export 3-panel bad board ONLY (bad_three.eps, bad_three.jpg)"
    )
    parser.add_argument(
        "--pattern",
        action="store_true",
        default=False,
        help="Export seamless pattern ONLY (pattern.eps, pattern.jpg)"
    )
    parser.add_argument(
        "--style",
        type=str,
        default=None,
        help="Override pattern layout style (e.g. half_drop_vertical, brick_wall, diamond_stagger, checker_pinwheel, scale_rhythm, wave_sine_offset, kaleidoscope_mirror, slanted_left)"
    )
    parser.add_argument(
        "--no-emoji", "--clean",
        action="store_true",
        dest="no_emoji",
        default=False,
        help="Use clean ASCII/Text labels ([P1], [P2], [OK], [*]) instead of Unicode emoji icons for 100% terminal compatibility"
    )

    args = parser.parse_args()

    # Load parameters from JSON configuration file
    full_cfg = load_config_file(args.config)
    if full_cfg:
        print(f"Loaded Adaptive JSON Configuration from: {args.config}")

    remove_list_override = None
    if args.remove_layers is not None and args.remove_layers.strip():
        try:
            remove_list_override = [int(x.strip()) for x in args.remove_layers.split(",") if x.strip().isdigit()]
        except Exception:
            pass

    run_batch_queue(
        input_path=args.input,
        output_path=args.output,
        distance_override=args.distance,
        simplify_tolerance_override=args.tolerance,
        silhouette_threshold_override=args.silhouette_threshold,
        remove_layers_override=remove_list_override,
        min_nodes_override=args.min_nodes,
        line_art_override=args.line_art,
        bg_color_override=args.bg_color,
        fg_color_override=args.fg_color,
        invert_override=args.invert,
        gap_override=args.gap,
        min_area_override=args.min_area,
        max_area_override=args.max_area,
        config_data=full_cfg,
        mode_single=args.single,
        mode_double=args.double,
        mode_triple=args.triple,
        mode_pattern=args.pattern,
        style_override=args.style,
        no_emoji=args.no_emoji,
    )


if __name__ == "__main__":
    main()

