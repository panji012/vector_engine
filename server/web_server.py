#!/usr/bin/env python3
"""
Vector Engine - Integrated 3-Station Modular Web Dashboard & API Server
Serves a 3-Box Station UI for Vectorizer Login (Box 1), Vectorizer Headless (Box 2), and Vector Engine Phase 1-7 (Box 3).
"""

import re
import os
import sys
import json
import time
import queue
import shutil
import threading
import subprocess
from http.server import HTTPServer, ThreadingHTTPServer, BaseHTTPRequestHandler
from urllib.parse import parse_qs, urlparse

# Directories
PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
STATIC_DIR = os.path.join(PROJECT_ROOT, "server", "static")
SCRIPTS_DIR = os.path.join(PROJECT_ROOT, "scripts")
INPUT_DIR = os.path.join(PROJECT_ROOT, "input")
OUTPUT_DIR = os.path.join(PROJECT_ROOT, "output")
STOCK_READY_DIR = os.path.join(PROJECT_ROOT, "stock_ready")
MANIFEST_PATH = os.path.join(PROJECT_ROOT, "manifest.json")
VECTORIZER_INPUT = os.path.join(PROJECT_ROOT, "input")
VECTORIZER_PROFILE = "/home/panjikali/snap/chromium/common/chromium"

os.makedirs(INPUT_DIR, exist_ok=True)
os.makedirs(OUTPUT_DIR, exist_ok=True)
os.makedirs(STOCK_READY_DIR, exist_ok=True)
os.makedirs(VECTORIZER_INPUT, exist_ok=True)
os.makedirs(VECTORIZER_PROFILE, exist_ok=True)

# Global State
sse_subscribers = []
sse_lock = threading.Lock()
manifest_lock = threading.Lock()
manifest_mode_enabled = False  # False = Mode Uji Coba / Trial (File JPG tetap aman di input/, tanpa skip)

current_job = {
    "status": "idle",
    "station_0": {"running": False, "queue": 0, "success": 0, "failed": 0, "log": "Stasiun 0 (AI Remixer Pro) Siap."},
    "station_2": {"running": False, "queue": 0, "success": 0, "failed": 0, "log": "Stasiun 2 Siap."},
    "station_3": {"running": False, "queue": 0, "success": 0, "failed": 0, "progress": 0, "log": "Stasiun 3 Siap."},
    "station_4": {"running": False, "queue": 0, "success": 0, "failed": 0, "progress": 0, "log": "Stasiun 4 Siap."}
}

# Persistent counts
stock_stats = {
    "station_0_success": 0,
    "station_0_failed": 0,
    "station_2_success": 0,
    "station_2_failed": 0,
    "station_3_success": 0,
    "station_3_failed": 0,
    "station_4_success": 0,
    "station_4_failed": 0,
    "failed_files": []
}

def load_manifest():
    with manifest_lock:
        if os.path.exists(MANIFEST_PATH):
            try:
                with open(MANIFEST_PATH, "r", encoding="utf-8") as f:
                    return json.load(f)
            except Exception:
                pass
        return {"updated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"), "files": {}}

def save_manifest(data):
    with manifest_lock:
        data["updated_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ")
        with open(MANIFEST_PATH, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2)

def update_manifest_file(filename, status, station=2, error=None, outputs=None):
    data = load_manifest()
    files = data.get("files", {})
    if filename not in files:
        files[filename] = {"added_at": time.strftime("%Y-%m-%dT%H:%M:%SZ")}
    
    files[filename]["updated_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ")
    files[filename]["status"] = status
    files[filename][f"station_{station}"] = status
    if error:
        files[filename]["error"] = error
    if outputs:
        files[filename]["outputs"] = outputs
    data["files"] = files
    save_manifest(data)

def auto_rename_sequential(target_dir, prefix="vector_stock"):
    """
    Auto-renames all raw input files in target_dir to sequential format: prefix_0001.ext, prefix_0002.ext.
    Stores original_name -> renamed_name mapping in manifest.json.
    """
    data = load_manifest()
    files_manifest = data.get("files", {})

    highest_num = 0
    pattern = re.compile(rf"^{re.escape(prefix)}_(\d+)\.", re.IGNORECASE)

    # Check highest existing number across manifest keys and target folder
    for fn in list(files_manifest.keys()) + (os.listdir(target_dir) if os.path.exists(target_dir) else []):
        m = pattern.match(fn)
        if m:
            val = int(m.group(1))
            if val > highest_num:
                highest_num = val

    if not os.path.exists(target_dir):
        return []

    renamed_results = []
    dir_files = sorted(os.listdir(target_dir))

    for fn in dir_files:
        ext = os.path.splitext(fn)[1].lower()
        if ext not in [".jpg", ".jpeg", ".png", ".webp", ".svg"]:
            continue

        if pattern.match(fn):
            continue

        highest_num += 1
        new_name = f"{prefix}_{highest_num:04d}{ext}"
        src_path = os.path.join(target_dir, fn)
        dst_path = os.path.join(target_dir, new_name)

        while os.path.exists(dst_path):
            highest_num += 1
            new_name = f"{prefix}_{highest_num:04d}{ext}"
            dst_path = os.path.join(target_dir, new_name)

        shutil.move(src_path, dst_path)

        files_manifest[new_name] = {
            "original_name": fn,
            "renamed_name": new_name,
            "added_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
            "status": "pending",
            "station_2": "pending",
            "station_3": "pending"
        }
        renamed_results.append({"from": fn, "to": new_name})

    data["files"] = files_manifest
    save_manifest(data)
    return renamed_results

def sync_input_queues():
    """
    Auto-rename urut semua file di input/.
    """
    auto_rename_sequential(INPUT_DIR)

def get_manifest_summary():
    sync_input_queues()
    data = load_manifest()
    files = data.get("files", {})
    
    pending_v_files = [f for f in os.listdir(INPUT_DIR) if f.lower().endswith((".png", ".jpg", ".jpeg", ".webp"))] if os.path.exists(INPUT_DIR) else []
    pending_i_files = [f for f in os.listdir(INPUT_DIR) if f.lower().endswith(".svg")] if os.path.exists(INPUT_DIR) else []
    ready_stock = os.listdir(STOCK_READY_DIR) if os.path.exists(STOCK_READY_DIR) else []

    total_pending = len(pending_v_files) + len(pending_i_files)
    total_processed = len([f for f in ready_stock if f.lower().endswith((".eps", ".jpg", ".jpeg", ".svg"))])
    total_all = total_pending + total_processed

    progress_pct = round((total_processed / total_all * 100), 1) if total_all > 0 else (100.0 if total_processed > 0 else 0.0)
    eta_sec = total_pending * 15

    return {
        "total": total_all,
        "processed": total_processed,
        "failed": 0,
        "pending": total_pending,
        "progress_percent": progress_pct,
        "eta_seconds": eta_sec,
        "eta_formatted": f"{eta_sec // 60} min {eta_sec % 60} sec" if eta_sec >= 60 else f"{eta_sec} sec",
        "failed_list": [],
        "manifest_files": files
    }

def get_pending_queues():
    sync_input_queues()
    temp_remix_dir = os.path.join(PROJECT_ROOT, "temp_remix")
    st0_pending = [f for f in os.listdir(temp_remix_dir) if f.lower().endswith((".png", ".jpg", ".jpeg", ".webp"))] if os.path.exists(temp_remix_dir) else []
    
    rasters = [f for f in os.listdir(INPUT_DIR) if f.lower().endswith((".png", ".jpg", ".jpeg", ".webp"))] if os.path.exists(INPUT_DIR) else []
    svgs = [f for f in os.listdir(INPUT_DIR) if f.lower().endswith(".svg")] if os.path.exists(INPUT_DIR) else []
    stock_files = os.listdir(STOCK_READY_DIR) if os.path.exists(STOCK_READY_DIR) else []
    stock_stems = set([os.path.splitext(f)[0].replace("_bad_two", "").replace("_bad_three", "").replace("_pattern", "").replace("_single", "") for f in stock_files])
    all_svg_stems = set([os.path.splitext(f)[0] for f in svgs]).union(stock_stems)

    outputs = os.listdir(OUTPUT_DIR) if os.path.exists(OUTPUT_DIR) else []
    output_stems = set([os.path.splitext(f)[0].replace("_bad_two", "").replace("_bad_three", "").replace("_pattern", "").replace("_single", "") for f in outputs]).union(stock_stems)

    # Pending for Stasiun 2: raster files without corresponding SVG or Stock Ready
    st2_pending = [r for r in rasters if os.path.splitext(r)[0] not in all_svg_stems]

    # Pending for Stasiun 3: SVG files in input/ without corresponding deliverables in output/ or stock_ready/
    st3_pending = [s for s in svgs if os.path.splitext(s)[0] not in output_stems]

    # Pending for Stasiun 4: Deliverables in output/ ready to be cataloged into stock_ready/
    st4_pending = [o for o in outputs if o.lower().endswith((".eps", ".jpg"))]

    return {
        "st0_pending": sorted(st0_pending),
        "st2_pending": sorted(st2_pending),
        "st3_pending": sorted(st3_pending),
        "st4_pending": sorted(st4_pending),
        "all_outputs": sorted(outputs + stock_files)
    }


def broadcast_sse(event_type, data):
    payload = f"event: {event_type}\ndata: {json.dumps(data)}\n\n"
    with sse_lock:
        dead = []
        for q in sse_subscribers:
            try:
                q.put_nowait(payload)
            except Exception:
                dead.append(q)
        for q in dead:
            sse_subscribers.remove(q)

def clean_profile_locks():
    subprocess.run(["pkill", "-9", "-f", "vectorizer_profile"], stderr=subprocess.DEVNULL)
    time.sleep(0.3)
    for fname in ["SingletonLock", "SingletonSocket", "SingletonCookie"]:
        fpath = os.path.join(VECTORIZER_PROFILE, fname)
        try:
            if os.path.lexists(fpath):
                os.unlink(fpath)
        except Exception:
            pass

class VectorEngineHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass

    def send_json(self, data, status=200):
        body = json.dumps(data).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.end_headers()

    def do_GET(self):
        parsed = urlparse(self.path)
        path = parsed.path

        if path == "/":
            return self.serve_static("index.html")

        if path.startswith("/static/"):
            rel_path = path[8:]
            return self.serve_static(rel_path)

        if path in ["/captcha_screenshot.png", "/captcha_screenshot"]:
            return self.serve_static("captcha_screenshot.png")

        if path == "/api/manifest":
            return self.send_json(get_manifest_summary())

        if path == "/api/config_silhouette":
            config_path = os.path.join(PROJECT_ROOT, "config_silhouette.json")
            if os.path.exists(config_path):
                with open(config_path, "r", encoding="utf-8") as f:
                    return self.send_json(json.load(f))
            return self.send_json({"error": "Config file not found"}, 404)

        if path == "/api/status":
            queues = get_pending_queues()
            has_profile = os.path.exists(VECTORIZER_PROFILE) and len(os.listdir(VECTORIZER_PROFILE)) > 0
            manifest_summary = get_manifest_summary()

            return self.send_json({
                "status": "online",
                "job": current_job,
                "stats": stock_stats,
                "input_files": queues["st3_pending"],
                "output_files": queues["all_outputs"],
                "vec_input_files": queues["st2_pending"],
                "st4_pending_files": queues["st4_pending"],
                "has_login_session": has_profile,
                "manifest_mode_enabled": manifest_mode_enabled,
                "manifest": manifest_summary
            })

        if path == "/api/progress":
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-cache")
            self.send_header("Connection", "keep-alive")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()

            q = queue.Queue()
            with sse_lock:
                sse_subscribers.append(q)

            try:
                q.put_nowait(f"event: init\ndata: {json.dumps({'job': current_job, 'stats': stock_stats})}\n\n")
                while True:
                    msg = q.get(timeout=30)
                    self.wfile.write(msg.encode("utf-8"))
                    self.wfile.flush()
            except Exception:
                with sse_lock:
                    if q in sse_subscribers:
                        sse_subscribers.remove(q)
            return

        if path == "/api/download":
            params = parse_qs(parsed.query)
            filename = params.get("file", [""])[0]
            file_path = os.path.join(OUTPUT_DIR, filename)
            if not filename or not os.path.exists(file_path):
                return self.send_json({"error": "File not found"}, 404)

            ext = os.path.splitext(filename)[1].lower()
            mime = "image/jpeg" if ext in [".jpg", ".jpeg"] else "image/svg+xml" if ext == ".svg" else "application/postscript"
            
            with open(file_path, "rb") as f:
                data = f.read()

            self.send_response(200)
            self.send_header("Content-Type", mime)
            self.send_header("Content-Disposition", f'attachment; filename="{filename}"')
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            self.wfile.write(data)
            return

        if path == "/api/preview":
            params = parse_qs(parsed.query)
            filename = params.get("file", [""])[0]
            file_path = os.path.join(OUTPUT_DIR, filename)
            if not filename or not os.path.exists(file_path):
                return self.send_json({"error": "File not found"}, 404)

            ext = os.path.splitext(filename)[1].lower()
            mime = "image/jpeg" if ext in [".jpg", ".jpeg"] else "image/png" if ext == ".png" else "image/svg+xml"
            with open(file_path, "rb") as f:
                data = f.read()

            self.send_response(200)
            self.send_header("Content-Type", mime)
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            self.wfile.write(data)
            return

        return self.send_json({"error": "Not found"}, 404)

    def serve_static(self, rel_path):
        target = os.path.join(STATIC_DIR, rel_path)
        if not os.path.exists(target):
            return self.send_json({"error": "File not found"}, 404)

        ext = os.path.splitext(target)[1].lower()
        mime = "text/html" if ext == ".html" else "text/css" if ext == ".css" else "application/javascript" if ext == ".js" else "image/png" if ext == ".png" else "image/jpeg" if ext in [".jpg", ".jpeg"] else "application/octet-stream"

        with open(target, "rb") as f:
            content = f.read()

        self.send_response(200)
        self.send_header("Content-Type", mime)
        self.send_header("Content-Length", str(len(content)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(content)

    def do_POST(self):
        parsed = urlparse(self.path)
        path = parsed.path

        if path == "/api/login-session":
            def run_login():
                try:
                    clean_profile_locks()

                    env = os.environ.copy()
                    if "DISPLAY" not in env:
                        env["DISPLAY"] = ":0"

                    subprocess.run(["node", os.path.join(SCRIPTS_DIR, "vector_login.js")], env=env, check=True)
                    broadcast_sse("login_status", {"status": "success", "message": "Login session active"})
                except Exception as e:
                    broadcast_sse("login_status", {"status": "error", "message": str(e)})

            threading.Thread(target=run_login, daemon=True).start()
        if path == "/api/reset_profile":
            clean_profile_locks()
            return self.send_json({"message": "File pengunci Chromium (SingletonLock) berhasil dibersihkan."})

        if path == "/api/upload_station_0":
            temp_remix_dir = os.path.join(PROJECT_ROOT, "temp_remix")
            os.makedirs(temp_remix_dir, exist_ok=True)
            return self.handle_upload(temp_remix_dir)

        if path == "/api/run_station_0":
            length = int(self.headers.get("Content-Length", 0))
            raw = self.rfile.read(length).decode("utf-8") if length > 0 else "{}"
            payload = json.loads(raw) if raw else {}

            threading.Thread(target=execute_station_0, args=(payload,), daemon=True).start()
            return self.send_json({"message": "Stasiun 0 (AI Concept Remixer Pro) started"})

        if path == "/api/upload_station_3":
            return self.handle_upload(INPUT_DIR)

        if path == "/api/retry_failed":
            retried = []
            if os.path.exists(FAILED_DIR):
                for f in os.listdir(FAILED_DIR):
                    src = os.path.join(FAILED_DIR, f)
                    dst = os.path.join(VECTORIZER_INPUT, f)
                    if os.path.exists(src):
                        shutil.move(src, dst)
                        retried.append(f)
                        update_manifest_file(f, "pending", station=2)

            manifest_data = load_manifest()
            for fn, info in manifest_data.get("files", {}).items():
                if info.get("status") == "failed":
                    update_manifest_file(fn, "pending", station=2)
                    if fn not in retried:
                        retried.append(fn)

            return self.send_json({"message": f"Berhasil mengembalikan {len(retried)} file gagal ke antrean utama.", "files": retried})

        if path == "/api/toggle_manifest":
            global manifest_mode_enabled
            manifest_mode_enabled = not manifest_mode_enabled
            mode_label = "AKTIF (Mode Batch Ribuan Gambar)" if manifest_mode_enabled else "MATI (Mode Uji Coba / Trial)"
            return self.send_json({"manifest_mode_enabled": manifest_mode_enabled, "message": f"Absensi Batch berhasil diubah ke: {mode_label}"})

        if path == "/api/run_station_2":
            threading.Thread(target=execute_station_2, daemon=True).start()
            return self.send_json({"message": "Stasiun 2 (Vectorizer Headless) started"})

        if path == "/api/config_silhouette":
            length = int(self.headers.get("Content-Length", 0))
            raw = self.rfile.read(length).decode("utf-8") if length > 0 else "{}"
            payload = json.loads(raw) if raw else {}
            config_path = os.path.join(PROJECT_ROOT, "config_silhouette.json")
            with open(config_path, "w", encoding="utf-8") as f:
                json.dump(payload, f, indent=4)
            return self.send_json({"message": "config_silhouette.json berhasil diperbarui!"})

        if path == "/api/run_station_3":
            length = int(self.headers.get("Content-Length", 0))
            raw = self.rfile.read(length).decode("utf-8") if length > 0 else "{}"
            payload = json.loads(raw) if raw else {}

            threading.Thread(target=execute_station_3, args=(payload,), daemon=True).start()
            return self.send_json({"message": "Stasiun 3 (Vector Engine) started"})

        if path == "/api/run_station_4":
            length = int(self.headers.get("Content-Length", 0))
            raw = self.rfile.read(length).decode("utf-8") if length > 0 else "{}"
            payload = json.loads(raw) if raw else {}
            gemini_url = payload.get("gemini_url", "").strip() or None
            batch_size = int(payload.get("batch_size", 5))

            threading.Thread(target=execute_station_4, args=(gemini_url, batch_size), daemon=True).start()
            return self.send_json({"message": "Stasiun 4 (AI Metadata & ExifTool) started"})

        if path == "/api/download_master_csv":
            csv_path = os.path.join(PROJECT_ROOT, "master_katalog.csv")
            if not os.path.exists(csv_path):
                return self.send_json({"error": "master_katalog.csv belum dibuat"}, 404)
            return self.serve_static_file(csv_path, "text/csv")

        if path == "/api/auto_rename":
            length = int(self.headers.get("Content-Length", 0))
            raw = self.rfile.read(length).decode("utf-8") if length > 0 else "{}"
            payload = json.loads(raw) if raw else {}
            prefix = payload.get("prefix", "vector_stock").strip() or "vector_stock"

            r1 = auto_rename_sequential(VECTORIZER_INPUT, prefix)
            r2 = auto_rename_sequential(INPUT_DIR, prefix)
            total = len(r1) + len(r2)

            return self.send_json({"message": f"Berhasil merename {total} file ke format urut ({prefix}_XXXX)", "renamed": r1 + r2})

        if path == "/api/run_chained":
            length = int(self.headers.get("Content-Length", 0))
            raw = self.rfile.read(length).decode("utf-8") if length > 0 else "{}"
            payload = json.loads(raw) if raw else {}

            threading.Thread(target=execute_chained_pipeline, args=(payload,), daemon=True).start()
            return self.send_json({"message": "Full Automated Chained Pipeline (Stasiun 2 ➔ 3 ➔ 4) started"})

        if path == "/api/stop":
            stop_all_pipelines()
            return self.send_json({"message": "🛑 Pipeline berhasil dihentikan oleh user."})

        return self.send_json({"error": "Not found"}, 404)

    def handle_upload(self, target_dir):
        content_length = int(self.headers.get("Content-Length", 0))
        content_type = self.headers.get("Content-Type", "")

        if "multipart/form-data" in content_type:
            boundary = content_type.split("boundary=")[1].encode("utf-8")
            raw_data = self.rfile.read(content_length)
            parts = raw_data.split(b"--" + boundary)

            saved_files = []
            for part in parts:
                if b'filename="' in part:
                    header, body = part.split(b"\r\n\r\n", 1)
                    body = body.rsplit(b"\r\n", 1)[0]
                    
                    fn_line = [l for l in header.decode("utf-8", errors="ignore").split("\r\n") if "filename=" in l][0]
                    filename = fn_line.split('filename="')[1].split('"')[0]
                    
                    if filename:
                        dest = os.path.join(target_dir, filename)
                        with open(dest, "wb") as f:
                            f.write(body)
                        saved_files.append(filename)

            # Auto-rename uploaded files immediately to sequential format
            renamed = auto_rename_sequential(target_dir)
            return self.send_json({"message": f"Berhasil mengunggah {len(saved_files)} file (di-rename otomatis secara urut).", "files": saved_files, "renamed": renamed})
        else:
            return self.send_json({"error": "Invalid Content-Type"}, 400)

def update_engine_config(config_params):
    if not config_params or not isinstance(config_params, dict):
        return
    config_path = os.path.join(PROJECT_ROOT, "config_silhouette.json")
    if not os.path.exists(config_path):
        return
    try:
        with open(config_path, "r", encoding="utf-8") as f:
            data = json.load(f)

        if "config_json" in config_params and isinstance(config_params["config_json"], dict):
            data = config_params["config_json"]
        else:
            style = config_params.get("pattern_style", "brick_wall")
            enable = config_params.get("pattern_enable", True)
            grid_x = int(config_params.get("grid_x", 3))
            grid_y = int(config_params.get("grid_y", 3))
            wraparound = config_params.get("wraparound", True)
            gap = float(config_params.get("gap", 50.0))
            bg_color = config_params.get("bg_color", "#FFFFFF")
            line_art = config_params.get("line_art", False)

            # Profile 1: low_color_bad_two
            if "low_color_bad_two" in data:
                if "low_max_colors" in config_params:
                    data["low_color_bad_two"]["max_colors"] = int(config_params["low_max_colors"])
                if "low_threshold" in config_params:
                    data["low_color_bad_two"]["threshold"] = float(config_params["low_threshold"])
                data["low_color_bad_two"]["gap"] = gap
                data["low_color_bad_two"]["bg_color"] = bg_color
                data["low_color_bad_two"]["line_art"] = line_art
                if "pattern_config" in data["low_color_bad_two"]:
                    data["low_color_bad_two"]["pattern_config"]["enable"] = config_params.get("low_pattern_enable", enable)
                    data["low_color_bad_two"]["pattern_config"]["style"] = config_params.get("low_pattern_style", style)
                    data["low_color_bad_two"]["pattern_config"]["grid_x"] = grid_x
                    data["low_color_bad_two"]["pattern_config"]["grid_y"] = grid_y
                    data["low_color_bad_two"]["pattern_config"]["wraparound_seamless"] = wraparound

            # Profile 2: multi_color_bad_two
            if "multi_color_bad_two" in data:
                if "multi_min_colors" in config_params:
                    data["multi_color_bad_two"]["min_colors"] = int(config_params["multi_min_colors"])
                if "multi_threshold" in config_params:
                    data["multi_color_bad_two"]["threshold"] = float(config_params["multi_threshold"])
                data["multi_color_bad_two"]["gap"] = gap
                if "pattern_config" in data["multi_color_bad_two"]:
                    data["multi_color_bad_two"]["pattern_config"]["enable"] = config_params.get("multi_pattern_enable", False)
                    data["multi_color_bad_two"]["pattern_config"]["style"] = config_params.get("multi_pattern_style", style)
                    data["multi_color_bad_two"]["pattern_config"]["grid_x"] = grid_x
                    data["multi_color_bad_two"]["pattern_config"]["grid_y"] = grid_y
                    data["multi_color_bad_two"]["pattern_config"]["wraparound_seamless"] = wraparound

                if "middle_panel" in data["multi_color_bad_two"]:
                    if "mid_min_lum" in config_params:
                        data["multi_color_bad_two"]["middle_panel"]["min_luminance"] = float(config_params["mid_min_lum"])
                    if "mid_max_lum" in config_params:
                        data["multi_color_bad_two"]["middle_panel"]["max_luminance"] = float(config_params["mid_max_lum"])

                if "right_panel" in data["multi_color_bad_two"]:
                    if "right_min_lum" in config_params:
                        data["multi_color_bad_two"]["right_panel"]["min_luminance"] = float(config_params["right_min_lum"])
                    if "right_max_lum" in config_params:
                        data["multi_color_bad_two"]["right_panel"]["max_luminance"] = float(config_params["right_max_lum"])
                    if "right_exclude_bg" in config_params:
                        data["multi_color_bad_two"]["right_panel"]["exclude_background"] = bool(config_params["right_exclude_bg"])

        with open(config_path, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=4)
        print(f"[CONFIG] Updated config_silhouette.json successfully!")
    except Exception as e:
        print(f"[CONFIG_UPDATE_ERROR] Failed to update config_silhouette.json: {e}")

def execute_station_3(config_params=None):
    global current_job, stock_stats, current_proc_st3
    if isinstance(config_params, str):
        config_params = {"pattern_style": config_params, "export_mode": "auto"}
    elif config_params is None:
        config_params = {"pattern_style": "brick_wall", "export_mode": "auto"}

    update_engine_config(config_params)

    export_mode = config_params.get("export_mode", "auto")
    pattern_style = config_params.get("pattern_style", "brick_wall")

    auto_rename_sequential(INPUT_DIR)
    current_job["station_3"]["running"] = True
    current_job["station_3"]["progress"] = 10
    current_job["station_3"]["log"] = f"Stasiun 3: Memulai Rust Vector Engine (Mode: {export_mode}, Style: {pattern_style})..."
    broadcast_sse("update", {"job": current_job, "stats": stock_stats})

    svg_files = [f for f in os.listdir(INPUT_DIR) if f.lower().endswith(".svg")]
    current_job["station_3"]["queue"] = len(svg_files)

    python_exe = os.path.join(PROJECT_ROOT, ".venv", "bin", "python")
    if not os.path.exists(python_exe):
        python_exe = sys.executable

    env = os.environ.copy()
    env["PYTHONUNBUFFERED"] = "1"

    cmd = [
        python_exe,
        "-u",
        os.path.join(PROJECT_ROOT, "run_batch.py"),
        "--input", INPUT_DIR,
        "--output", OUTPUT_DIR,
        "--clean"
    ]

    if export_mode == "pattern":
        cmd.extend(["--pattern", "--style", pattern_style])
    elif export_mode == "double":
        cmd.extend(["--double", "--style", pattern_style])
    elif export_mode == "single":
        cmd.extend(["--single"])
    else:  # "auto" -> Uses config_silhouette.json rules (low_color = pattern + bad_two, multi_color = 3-panel bad_two/bad_three)!
        if pattern_style:
            cmd.extend(["--style", pattern_style])

    current_proc_st3 = subprocess.Popen(
        cmd,
        cwd=PROJECT_ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        env=env
    )

    if current_proc_st3 and current_proc_st3.stdout:
        for line in current_proc_st3.stdout:
            line_clean = line.strip()
            if line_clean:
                current_job["station_3"]["log"] = line_clean
                if "Phase 1" in line_clean: current_job["station_3"]["progress"] = 25
                elif "Phase 2" in line_clean: current_job["station_3"]["progress"] = 40
                elif "Phase 3" in line_clean: current_job["station_3"]["progress"] = 55
                elif "Phase 4" in line_clean: current_job["station_3"]["progress"] = 70
                elif "Phase 5" in line_clean: current_job["station_3"]["progress"] = 85
                elif "Phase 7" in line_clean: current_job["station_3"]["progress"] = 95
                elif "Finished" in line_clean and "Status: [OK]" in line_clean:
                    stock_stats["station_3_success"] += 1
                elif "FAILED" in line_clean:
                    stock_stats["station_3_failed"] += 1
                broadcast_sse("update", {"job": current_job, "stats": stock_stats})

    if current_proc_st3:
        current_proc_st3.wait()
        current_proc_st3 = None
    current_job["station_3"]["running"] = False
    current_job["station_3"]["progress"] = 100
    current_job["station_3"]["log"] = "Stasiun 3: Selesai! Paket EPS & JPG Deliverables Siap."
    broadcast_sse("update", {"job": current_job, "stats": stock_stats})

def execute_station_4(gemini_url=None, batch_size=5):
    global current_job, stock_stats, current_proc_st4

    clean_profile_locks()

    current_job["station_4"]["running"] = True
    current_job["station_4"]["log"] = "Stasiun 4: Memulai AI Metadata Generator & ExifTool Injector..."
    broadcast_sse("update", {"job": current_job, "stats": stock_stats})

    env = os.environ.copy()
    if gemini_url:
        env["TARGET_LINK"] = gemini_url
    if batch_size:
        env["BATCH_SIZE"] = str(batch_size)

    current_proc_st4 = subprocess.Popen(
        ["node", os.path.join(SCRIPTS_DIR, "metadata_engine.js")],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        env=env
    )

    if current_proc_st4 and current_proc_st4.stdout:
        for line in current_proc_st4.stdout:
            line_clean = line.strip()
            if line_clean:
                current_job["station_4"]["log"] = f"[Metadata AI] {line_clean}"
                if "Downloaded:" in line_clean or "Injected Successfully" in line_clean or "sukses:" in line_clean:
                    stock_stats["station_4_success"] += 1
                elif "Failed:" in line_clean or "Gagal" in line_clean:
                    stock_stats["station_4_failed"] += 1
                broadcast_sse("update", {"job": current_job, "stats": stock_stats})

    if current_proc_st4:
        current_proc_st4.wait()
        current_proc_st4 = None

    organize_stock_ready_batches(items_per_batch=30)

    current_job["station_4"]["running"] = False
    current_job["station_4"]["log"] = "Stasiun 4: Selesai! Metadata terinjeksi & Berkas dikelompokkan ke stock_ready/ (Grup 30 Stock)."
    broadcast_sse("update", {"job": current_job, "stats": stock_stats})

def organize_stock_ready_batches(items_per_batch=30):
    """
    Moves deliverables from output/ to stock_ready/batch_XXX/ (grouped into 30 stock items per subfolder).
    """
    if not os.path.exists(OUTPUT_DIR):
        return

    output_files = [f for f in os.listdir(OUTPUT_DIR) if f.lower().endswith((".eps", ".jpg", ".jpeg", ".svg"))]
    if not output_files:
        return

    stem_groups = {}
    for f in output_files:
        stem = os.path.splitext(f)[0].replace("_bad_two", "").replace("_bad_three", "").replace("_pattern", "").replace("_single", "")
        if stem not in stem_groups:
            stem_groups[stem] = []
        stem_groups[stem].append(f)

    os.makedirs(STOCK_READY_DIR, exist_ok=True)
    existing_batches = sorted([d for d in os.listdir(STOCK_READY_DIR) if os.path.isdir(os.path.join(STOCK_READY_DIR, d)) and d.startswith("batch_")])

    batch_idx = 1
    if existing_batches:
        last_batch = existing_batches[-1]
        try:
            batch_idx = int(last_batch.split("batch_")[1])
        except ValueError:
            batch_idx = len(existing_batches)

    def count_stems_in_batch(batch_path):
        if not os.path.exists(batch_path): return 0
        bfiles = os.listdir(batch_path)
        stems = set([os.path.splitext(bf)[0].replace("_bad_two", "").replace("_bad_three", "").replace("_pattern", "").replace("_single", "") for bf in bfiles])
        return len(stems)

    current_batch_dir = os.path.join(STOCK_READY_DIR, f"batch_{batch_idx:03d}")
    os.makedirs(current_batch_dir, exist_ok=True)

    sorted_stems = sorted(stem_groups.keys(), key=lambda s: s.lower())

    for stem in sorted_stems:
        if count_stems_in_batch(current_batch_dir) >= items_per_batch:
            batch_idx += 1
            current_batch_dir = os.path.join(STOCK_READY_DIR, f"batch_{batch_idx:03d}")
            os.makedirs(current_batch_dir, exist_ok=True)

        for fname in stem_groups[stem]:
            src = os.path.join(OUTPUT_DIR, fname)
            dst = os.path.join(current_batch_dir, fname)
            if os.path.isfile(src):
                if os.path.exists(dst): os.remove(dst)
                shutil.move(src, dst)

current_proc_st0 = None
current_proc_st2 = None
current_proc_st3 = None
current_proc_st4 = None

def execute_station_0(payload=None):
    global current_job, stock_stats, current_proc_st0
    if payload is None: payload = {}

    # Kill duplicate Node 0 processes to prevent concurrent instances
    subprocess.run(["pkill", "-9", "-f", "remixer_engine.js"], stderr=subprocess.DEVNULL)

    # Clean old leftover temporary files from temp_remix/
    temp_remix_dir = os.path.join(PROJECT_ROOT, "temp_remix")
    if os.path.exists(temp_remix_dir):
        for f in os.listdir(temp_remix_dir):
            try: os.remove(os.path.join(temp_remix_dir, f))
            except Exception: pass

    clean_profile_locks()

    current_job["station_0"]["running"] = True
    current_job["station_0"]["log"] = "Stasiun 0: Memulai AI Concept Remixer Engine..."
    broadcast_sse("update", {"job": current_job, "stats": stock_stats})

    env = os.environ.copy()
    if "http_urls" in payload: env["HTTP_URLS"] = payload["http_urls"]
    if "prompt" in payload: env["CUSTOM_PROMPT"] = payload["prompt"]
    if "style_level" in payload: env["STYLE_LEVEL"] = payload["style_level"]
    if "color_match" in payload: env["COLOR_MATCH"] = payload["color_match"]
    if "canvas_ratio" in payload: env["CANVAS_RATIO"] = payload["canvas_ratio"]
    if "tool1_anti_gelap" in payload: env["TOOL_ANTI_GELAP"] = "true" if payload["tool1_anti_gelap"] else "false"
    if "tool2_bg_bersih" in payload: env["TOOL_BG_BERSIH"] = "true" if payload["tool2_bg_bersih"] else "false"
    if "color_theme" in payload: env["COLOR_THEME"] = payload["color_theme"]
    if "gemini_url" in payload and payload["gemini_url"]: env["TARGET_LINK"] = payload["gemini_url"]

    current_proc_st0 = subprocess.Popen(
        ["node", os.path.join(SCRIPTS_DIR, "remixer_engine.js")],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        env=env
    )

    if current_proc_st0 and current_proc_st0.stdout:
        for line in current_proc_st0.stdout:
            line_clean = line.strip()
            if line_clean:
                current_job["station_0"]["log"] = f"[Remixer AI] {line_clean}"
                if "[ITEM_REMIX_SUCCESS]" in line_clean:
                    stock_stats["station_0_success"] += 1
                elif "[ITEM_REMIX_FAILED]" in line_clean:
                    stock_stats["station_0_failed"] += 1
                broadcast_sse("update", {"job": current_job, "stats": stock_stats})

    if current_proc_st0:
        current_proc_st0.wait()
        current_proc_st0 = None

    current_job["station_0"]["running"] = False
    current_job["station_0"]["log"] = f"Stasiun 0: Selesai! {stock_stats.get('station_0_success', 0)} Gambar Selesai terverifikasi & tersimpan ke Stasiun 2."
    broadcast_sse("update", {"job": current_job, "stats": stock_stats})

def execute_chained_pipeline(config_params=None):
    execute_station_0(config_params)
    execute_station_2()
    execute_station_3(config_params)
    gemini_url = config_params.get("gemini_url") if isinstance(config_params, dict) else None
    batch_size = int(config_params.get("batch_size", 5)) if isinstance(config_params, dict) else 5
    execute_station_4(gemini_url, batch_size)

def stop_all_pipelines():
    global current_proc_st0, current_proc_st2, current_proc_st3, current_proc_st4, current_job, stock_stats
    if current_proc_st0 and current_proc_st0.poll() is None:
        try:
            current_proc_st0.terminate()
            current_proc_st0.kill()
        except Exception:
            pass
    if current_proc_st2 and current_proc_st2.poll() is None:
        try:
            current_proc_st2.terminate()
            current_proc_st2.kill()
        except Exception:
            pass
    if current_proc_st3 and current_proc_st3.poll() is None:
        try:
            current_proc_st3.terminate()
            current_proc_st3.kill()
        except Exception:
            pass
    if current_proc_st4 and current_proc_st4.poll() is None:
        try:
            current_proc_st4.terminate()
            current_proc_st4.kill()
        except Exception:
            pass

    subprocess.run(["pkill", "-f", "remixer_engine.js"], stderr=subprocess.DEVNULL)
    subprocess.run(["pkill", "-f", "vectorizer_headless.js"], stderr=subprocess.DEVNULL)
    subprocess.run(["pkill", "-f", "metadata_engine.js"], stderr=subprocess.DEVNULL)

    current_job["station_0"]["running"] = False
    current_job["station_0"]["log"] = "🛑 Stasiun 0: Dihentikan oleh user."
    current_job["station_2"]["running"] = False
    current_job["station_2"]["log"] = "🛑 Stasiun 2: Dihentikan oleh user."
    current_job["station_3"]["running"] = False
    current_job["station_3"]["log"] = "🛑 Stasiun 3: Dihentikan oleh user."
    current_job["station_4"]["running"] = False
    current_job["station_4"]["log"] = "🛑 Stasiun 4: Dihentikan oleh user."
    broadcast_sse("update", {"job": current_job, "stats": stock_stats})

def execute_station_2():
    global current_job, stock_stats, current_proc_st2
    auto_rename_sequential(VECTORIZER_INPUT)

    clean_profile_locks()

    current_job["station_2"]["running"] = True
    current_job["station_2"]["log"] = "Stasiun 2: Memulai Puppeteer Headless Vectorizer..."
    broadcast_sse("update", {"job": current_job, "stats": stock_stats})

    files = [f for f in os.listdir(VECTORIZER_INPUT) if f.lower().endswith((".png", ".jpg", ".jpeg", ".webp"))]
    current_job["station_2"]["queue"] = len(files)

    if not files:
        current_job["station_2"]["running"] = False
        current_job["station_2"]["log"] = "Stasiun 2: Antrean gambar kosong."
        broadcast_sse("update", {"job": current_job, "stats": stock_stats})
        return

    env = os.environ.copy()
    env["DISPLAY"] = os.environ.get("DISPLAY", ":0.0")
    if not manifest_mode_enabled:
        env["DISABLE_MANIFEST"] = "true"

    current_proc_st2 = subprocess.Popen(
        ["node", os.path.join(SCRIPTS_DIR, "vectorizer_headless.js")],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        env=env
    )

    if current_proc_st2 and current_proc_st2.stdout:
        for line in current_proc_st2.stdout:
            line_clean = line.strip()
            if line_clean:
                current_job["station_2"]["log"] = line_clean
                if "Downloaded" in line_clean:
                    stock_stats["station_2_success"] += 1
                elif "Failed:" in line_clean:
                    stock_stats["station_2_failed"] += 1
                    stock_stats["failed_files"].append(line_clean)
                broadcast_sse("update", {"job": current_job, "stats": stock_stats})

    if current_proc_st2:
        current_proc_st2.wait()
        current_proc_st2 = None

    # SVG hasil download akan otomatis dipindah ke input/ oleh sync_input_queues()
    sync_input_queues()

    current_job["station_2"]["running"] = False
    current_job["station_2"]["log"] = "Stasiun 2: Selesai! SVG tersimpan ke Stasiun 3."
    broadcast_sse("update", {"job": current_job, "stats": stock_stats})

def main():
    port = 8000
    if len(sys.argv) > 1:
        try:
            port = int(sys.argv[1])
        except ValueError:
            pass

    server = ThreadingHTTPServer(("0.0.0.0", port), VectorEngineHandler)
    print(f"================================================================================")
    print(f" 🌐 Vector Engine - 3-Station Modular Dashboard Server")
    print(f" ------------------------------------------------------------------------------")
    print(f"  Local Access    : http://localhost:{port}")
    print(f"  Network Access  : http://0.0.0.0:{port}")
    print(f"  Cloudflare/Tail : Map http://localhost:{port} to Cloudflare Tunnel / Tailscale")
    print(f"================================================================================")
    
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down server...")
        server.server_close()

if __name__ == "__main__":
    main()
