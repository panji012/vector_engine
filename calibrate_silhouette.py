#!/usr/bin/env python3
"""
Vector Geometry Engine - Dedicated Silhouette Calibration & Control Script

Usage:
  # 1. Inspect Color Layers/Buckets in an SVG:
  python3 calibrate_silhouette.py -i input/Design_003.svg --inspect

  # 2. Run Calibration with custom numeric threshold & node tolerance:
  python3 calibrate_silhouette.py -i input/Design_003.svg -t 0.65 --tolerance 0.20

  # 3. Run Calibration removing specific layer indexes (e.g. layer 0 background):
  python3 calibrate_silhouette.py -i input/Design_003.svg -r 0,3 -t 0.65

  # 4. Multi-Threshold Sweep Test (Compares 0.30, 0.50, and 0.70 thresholds):
  python3 calibrate_silhouette.py -i input/Design_003.svg --sweep
"""

import argparse
import json
import shutil
import subprocess
import sys
import time
from pathlib import Path

# RAM Disk Temporary Work Directory (/dev/shm)
RAM_TEMP_DIR = Path("/dev/shm/vector_engine_work")

# Ensure python directory is in search path
sys.path.insert(0, str(Path(__file__).parent / "python"))

try:
    from vector_engine._native import (
        process_svg_outset,
        inkscape_cpp_simplify_svg,
        parse_svg_string,
        apply_silhouette_variant_json,
        scale_document_area_json,
        scale_document_by_factor_json,
        create_duo_board_json,
    )
except ImportError:
    print("CRITICAL ERROR: PyO3 extension module could not be loaded.", file=sys.stderr)
    print("Please run: source .venv/bin/activate && maturin develop --release", file=sys.stderr)
    sys.exit(1)


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
    except Exception:
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

    lines.append('</svg>')
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


def export_jpg(svg_path: Path, jpg_path: Path) -> None:
    """Exports SVG to JPG format via PNG rasterization."""
    png_temp = svg_path.with_suffix(".temp.png")
    subprocess.run(
        ["inkscape", str(svg_path), "--export-area-page", f"--export-filename={png_temp}"],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    if png_temp.exists():
        try:
            sys.path.append("/usr/lib/python3/dist-packages")
            from PIL import Image
            Image.MAX_IMAGE_PIXELS = None
            with Image.open(png_temp) as img:
                img.convert("RGB").save(jpg_path, "JPEG", quality=95)
        except Exception as e:
            print(f"  WARNING: PIL JPG conversion failed: {e}", file=sys.stderr)
        finally:
            if png_temp.exists():
                png_temp.unlink()


def inspect_svg_layers(input_file: Path) -> None:
    """Inspects and prints detailed color layer indexes and telemetry of an SVG file."""
    content = input_file.read_text()
    try:
        doc_json = parse_svg_string(content)
        doc = json.loads(doc_json)
    except Exception as e:
        print(f"ERROR parsing SVG '{input_file.name}': {e}", file=sys.stderr)
        return

    paths = doc.get("paths", [])
    color_stats = {}
    for p in paths:
        fill = p.get("fill") or "none"
        if fill == "none":
            continue
        if fill not in color_stats:
            color_stats[fill] = {
                "path_count": 0,
                "cmd_count": 0,
                "lum": hex_luminance(fill),
            }
        color_stats[fill]["path_count"] += 1
        color_stats[fill]["cmd_count"] += len(p.get("commands", []))

    sorted_fills = sorted(color_stats.keys(), key=lambda c: color_stats[c]["lum"])

    print("\n================================================================================")
    print(f" Color Layer Inspection Report for: {input_file.name}")
    print("================================================================================")
    print(f"{'Layer Index':<12} | {'HEX Color':<12} | {'Luminance':<10} | {'Paths':<10} | {'Cmds':<10}")
    print("-" * 70)

    for idx, hex_code in enumerate(sorted_fills):
        stats = color_stats[hex_code]
        print(
            f"[{idx}]         | {hex_code:<12} | {stats['lum']:<10.3f} | {stats['path_count']:<10} | {stats['cmd_count']:<10}"
        )
    print("-" * 70)
    print(f"Total Unique Color Layers: {len(sorted_fills)}")
    print(f"Total Paths              : {len(paths)}")
    print("Tip: Use '-r 0' or '-r 0,2' to remove specific layer indexes when calibrating.\n")


def count_unique_svg_colors(svg_content: str) -> int:
    """Parses an SVG string and returns the number of unique non-none fill colors."""
    try:
        doc = json.loads(parse_svg_string(svg_content))
    except Exception:
        return 1

    fills = set()
    for p in doc.get("paths", []):
        fill = p.get("fill")
        if fill and fill != "none":
            fills.add(fill)

    return len(fills)


def select_config_profile(config_data: dict, color_count: int) -> tuple[str, dict]:
    """Selects matching config profile ('low_color_bad_two' vs 'multi_color_bad_two') based on color count."""
    if not config_data:
        return "default", {}
    
    if color_count <= 4 and "low_color_bad_two" in config_data:
        return "low_color_bad_two", config_data["low_color_bad_two"]
    elif color_count >= 5 and "multi_color_bad_two" in config_data:
        return "multi_color_bad_two", config_data["multi_color_bad_two"]
    
    if "bad_two" in config_data:
        return "bad_two", config_data["bad_two"]
    if "default" in config_data:
        return "default", config_data["default"]
    
    return "default", {}


def synchronize_svg_dimensions(ref_svg: str, input_svg: str) -> str:
    """Extracts width, height, and viewBox from ref_svg and applies them to input_svg for 100% identical dimensions."""
    try:
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
        ref_doc = json.loads(parse_svg_string(svg_content))
        w = float(ref_doc.get("width") or 1000.0)
        h = float(ref_doc.get("height") or 1000.0)
        vb = ref_doc.get("view_box")
        if vb:
            w = max(w, float(vb[2] - vb[0]))
            h = max(h, float(vb[3] - vb[1]))

        paths = ref_doc.get("paths", [])
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

        for idx, p in enumerate(paths):
            if idx == bg_idx:
                continue
            fill = p.get("fill") or "#000000"
            stroke = p.get("stroke") or "none"
            stroke_w = p.get("stroke_width") or 1.0
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
                lines.append(f'  <path id="{pid}" fill="{fill}" stroke="{stroke}" stroke-width="{stroke_w}" d="{d_str}"/>')

        lines.append('</svg>')
        return "\n".join(lines)
    except Exception:
        return svg_content


def remove_specific_layers_from_svg(svg_content: str, remove_indexes: list[int], min_nodes: int) -> str:
    """Filters out paths belonging to specified color layer indexes or below min_nodes."""
    try:
        doc = json.loads(parse_svg_string(svg_content))
    except Exception:
        return svg_content

    paths = doc.get("paths", [])
    if not paths:
        return svg_content

    # Group colors and determine layer indexes sorted by luminance
    color_lums = {}
    for p in paths:
        fill = p.get("fill")
        if fill and fill != "none" and fill not in color_lums:
            color_lums[fill] = hex_luminance(fill)

    sorted_fills = sorted(color_lums.keys(), key=lambda c: color_lums[c])
    # Line Art Immunity: Dark fill colors (luminance < 0.15) are protected and can NEVER be removed
    removed_fills = {
        sorted_fills[idx] 
        for idx in remove_indexes 
        if 0 <= idx < len(sorted_fills) and color_lums[sorted_fills[idx]] >= 0.15
    }

    filtered_paths = []
    for p in paths:
        fill = p.get("fill")
        cmd_count = len(p.get("commands", []))
        lum = color_lums.get(fill, 1.0)
        if fill in removed_fills and lum >= 0.15:
            continue
        if min_nodes > 0 and cmd_count < min_nodes and lum >= 0.15:
            continue
        filtered_paths.append(p)

    doc["paths"] = filtered_paths

    # Reconstruct SVG string
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


def run_calibration(
    input_file: Path,
    output_dir: Path,
    threshold: float,
    tolerance: float,
    remove_layers: list[int],
    min_nodes: int,
    distance: float = 0.5,
    line_art: bool = False,
    color_merge_threshold: float = 0.0,
    prof_name: str = "default",
    prof_cfg: dict = None,
) -> Path:
    """Runs a single silhouette calibration pass using RAM disk (/dev/shm) and exports SVG Bad Two/Three JPG & EPS preview."""
    if prof_cfg is None:
        prof_cfg = {}
    output_dir.mkdir(parents=True, exist_ok=True)
    RAM_TEMP_DIR.mkdir(parents=True, exist_ok=True)
    stem = input_file.stem
    start_t = time.time()

    rm_str = f"_rm{'_'.join(map(str, remove_layers))}" if remove_layers else ""
    print(
        f"Running Calibration for '{input_file.name}' | Profile: '{prof_name}' | Threshold: {threshold:.2f} | Tolerance: {tolerance:.2f} | Line Art: {line_art} | Color Merge: {color_merge_threshold:.2f} | Removed Layers: {remove_layers or 'None'}"
    )

    try:
        # Step 1: Outset & Combine (in RAM)
        t1 = time.time()
        outset_temp = RAM_TEMP_DIR / f"{stem}_outset_temp.svg"
        process_svg_outset(str(input_file), str(outset_temp), True, distance, True, tolerance)
        sec1 = time.time() - t1

        # Step 2: Inkscape Simplify (in RAM)
        t2 = time.time()
        simplified_temp = RAM_TEMP_DIR / f"{stem}_simp_temp.svg"
        raw_content = inkscape_cpp_simplify_svg(outset_temp.read_text(), tolerance)
        simplified_temp.write_text(raw_content)
        sec2 = time.time() - t2

        # Clean per-step temp files in RAM
        outset_temp.unlink(missing_ok=True)
        # Step 3: Minimal-Layer Original & Silhouette Board
        t3 = time.time()
        orig_input = raw_content
        if color_merge_threshold > 0.0:
            orig_input = merge_similar_colors_in_svg(orig_input, color_merge_threshold)

        sil_input = orig_input
        if remove_layers or min_nodes > 0:
            sil_input = remove_specific_layers_from_svg(sil_input, remove_layers, min_nodes)

        if prof_name == "multi_color_bad_two":
            mid_cfg = prof_cfg.get("middle_panel", {})
            right_cfg = prof_cfg.get("right_panel", {})

            mid_min_lum = float(mid_cfg.get("min_luminance", 0.20))
            mid_max_lum = float(mid_cfg.get("max_luminance", 0.62))

            right_min_lum = float(right_cfg.get("min_luminance", 0.20))
            right_max_lum = float(right_cfg.get("max_luminance", 0.62))

            sil_content = apply_multi_shade_luminance_recoloring(
                sil_input, threshold, "grey", min_luminance=mid_min_lum, max_luminance=mid_max_lum
            )
            panel3_content = apply_multi_shade_luminance_recoloring(
                sil_input, threshold, "colored", min_luminance=right_min_lum, max_luminance=right_max_lum
            )
        else:
            sil_content = apply_silhouette_variant_json(sil_input, threshold, line_art)
            panel3_content = None

        orig_scaled_raw, orig_factor = scale_document_area_json(orig_input, 19_000_000, 22_000_000)
        orig_scaled = center_svg_artwork_content(orig_scaled_raw)
        sil_scaled_raw = scale_document_by_factor_json(sil_content, orig_factor)
        sil_scaled = center_svg_artwork_content(synchronize_svg_dimensions(orig_scaled, sil_scaled_raw))

        # Determine landscape auto gap
        effective_gap = 50.0
        try:
            ref_doc_info = json.loads(parse_svg_string(orig_scaled))
            orig_w = float(ref_doc_info.get("width") or 0.0)
            orig_h = float(ref_doc_info.get("height") or 0.0)
            if orig_w > orig_h and orig_h > 0:
                aspect_ratio = orig_w / orig_h
                effective_gap = max(5.0, 50.0 / aspect_ratio)
        except Exception:
            effective_gap = 50.0

        if panel3_content is not None:
            p3_scaled_raw = scale_document_by_factor_json(panel3_content, orig_factor)
            p3_scaled = center_svg_artwork_content(synchronize_svg_dimensions(orig_scaled, p3_scaled_raw))
            duo_1_2 = create_duo_board_json(orig_scaled, sil_scaled, gap=effective_gap)
            duo_content = create_duo_board_json(duo_1_2, p3_scaled, gap=effective_gap)
        else:
            duo_content = create_duo_board_json(orig_scaled, sil_scaled, gap=effective_gap)
        sec3 = time.time() - t3

        # Step 4: Export Preview JPG & EPS in RAM -> Move Final to Output
        t4 = time.time()
        tag = f"t{threshold:.2f}{rm_str}"
        temp_duo_svg = RAM_TEMP_DIR / f"{stem}_calib_{tag}.svg"
        temp_duo_svg.write_text(duo_content)

        ram_jpg = RAM_TEMP_DIR / f"{stem}_calib_{tag}.jpg"
        ram_eps = RAM_TEMP_DIR / f"{stem}_calib_{tag}.eps"

        final_jpg = output_dir / f"{stem}_calib_{tag}.jpg"
        final_eps = output_dir / f"{stem}_calib_{tag}.eps"

        export_jpg(temp_duo_svg, ram_jpg)
        subprocess.run(
            ["inkscape", str(temp_duo_svg), "--export-area-page", f"--export-filename={ram_eps}"],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        if ram_jpg.exists():
            shutil.move(str(ram_jpg), str(final_jpg))
        if ram_eps.exists():
            shutil.move(str(ram_eps), str(final_eps))

        temp_duo_svg.unlink(missing_ok=True)
        sec4 = time.time() - t4

        total_sec = time.time() - start_t
        print(f"  - Phase 1 (Outset & Combine in RAM) : {sec1:.2f} s")
        print(f"  - Phase 2 (Path Simplify in RAM)    : {sec2:.2f} s")
        print(f"  - Phase 3 (Duo Board Build in RAM)  : {sec3:.2f} s")
        print(f"  - Phase 4 (Export in RAM -> Output) : {sec4:.2f} s")
        print(f"  --> Saved Calibrated Output         : {final_jpg.name} ({total_sec:.2f} s)\n")

        return final_jpg
    finally:
        if RAM_TEMP_DIR.exists():
            try:
                shutil.rmtree(RAM_TEMP_DIR)
            except Exception:
                pass


def load_config_file(config_path: Path) -> dict:
    """Loads calibration parameters from config_silhouette.json if present (supports # comment lines)."""
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
        description="Vector Geometry Engine - Dedicated Silhouette Calibration & Control Script"
    )
    parser.add_argument(
        "-i", "--input",
        type=Path,
        required=True,
        help="Path to input SVG file or directory"
    )
    parser.add_argument(
        "-o", "--output",
        type=Path,
        default=Path("output_calibration"),
        help="Directory to save calibration preview files (default: output_calibration)"
    )
    parser.add_argument(
        "-c", "--config",
        type=Path,
        default=Path("config_silhouette.json"),
        help="Path to calibration JSON config file (default: config_silhouette.json)"
    )
    parser.add_argument(
        "-t", "--threshold",
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
        "-d", "--distance",
        type=float,
        default=None,
        help="Outset distance in pixels (overrides config)"
    )
    parser.add_argument(
        "-cm", "--color-merge",
        type=float,
        default=None,
        help="Color merge similarity threshold (0.0 to 1.0, e.g. 0.15 to group close color shades, overrides config)"
    )
    parser.add_argument(
        "-r", "--remove-layers",
        type=str,
        default=None,
        help="Comma-separated list of layer indexes to remove (e.g. '0' or '0,2', overrides config)"
    )
    parser.add_argument(
        "--min-nodes",
        type=int,
        default=None,
        help="Minimum path node count threshold (overrides config)"
    )
    parser.add_argument(
        "-l", "--inspect",
        action="store_true",
        help="Inspect and print color layer breakdown of input SVG without rendering files"
    )
    parser.add_argument(
        "--sweep",
        action="store_true",
        help="Run multi-threshold sweep test (tests 0.30, 0.50, and 0.70 threshold settings)"
    )

    args = parser.parse_args()

    input_files = []
    if args.input.is_file():
        input_files.append(args.input)
    elif args.input.is_dir():
        input_files = sorted(list(args.input.glob("*.svg")))

    if not input_files:
        print(f"ERROR: No SVG files found at path: {args.input}", file=sys.stderr)
        sys.exit(1)

    if args.inspect:
        for f in input_files:
            inspect_svg_layers(f)
        return

    config_data = load_config_file(args.config)
    if config_data:
        print(f"Loaded JSON Configuration from: {args.config}")

    print("================================================================================")
    print(" Vector Geometry Engine - Silhouette Calibration Tool ")
    print("================================================================================")

    for f in input_files:
        raw_svg = f.read_text()
        color_count = count_unique_svg_colors(raw_svg)
        prof_name, prof_cfg = select_config_profile(config_data, color_count)

        threshold = args.threshold if args.threshold is not None else prof_cfg.get("threshold", 0.50)
        tolerance = args.tolerance if args.tolerance is not None else prof_cfg.get("tolerance", 0.20)
        distance = args.distance if args.distance is not None else prof_cfg.get("distance", 0.5)
        min_nodes = args.min_nodes if args.min_nodes is not None else prof_cfg.get("min_nodes", 0)
        line_art = prof_cfg.get("line_art", False)
        color_merge = args.color_merge if args.color_merge is not None else prof_cfg.get("color_merge_threshold", 0.0)

        remove_list = []
        if args.remove_layers is not None:
            if args.remove_layers.strip():
                try:
                    remove_list = [int(x.strip()) for x in args.remove_layers.split(",") if x.strip().isdigit()]
                except Exception:
                    pass
        else:
            cfg_rm = prof_cfg.get("remove_layers", [])
            if isinstance(cfg_rm, list):
                remove_list = [int(x) for x in cfg_rm if isinstance(x, (int, str)) and str(x).isdigit()]

        print(f"Input File: '{f.name}' ({color_count} colors -> Profile: '{prof_name}', Line Art: {line_art}, Color Merge: {color_merge:.2f})")
        if args.sweep:
            print(f"--- SWEEP TEST FOR: {f.name} ---")
            for t_val in [0.30, 0.50, 0.70]:
                run_calibration(f, args.output, t_val, tolerance, remove_list, min_nodes, distance, line_art, color_merge, prof_name, prof_cfg)
        else:
            run_calibration(f, args.output, threshold, tolerance, remove_list, min_nodes, distance, line_art, color_merge, prof_name, prof_cfg)

    print("Calibration completed! Inspect generated preview files in:", args.output)


if __name__ == "__main__":
    main()
