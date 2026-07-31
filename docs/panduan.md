# 📖 Panduan Lengkap Eksekusi Terminal - Vector Geometry Engine

Dokumen ini berisi panduan langkah demi langkah untuk menjalankan setiap fitur, script batch queue, mode eksekusi CLI, dan membaca log terminal yang rapi dan mudah diamati.

---

## 🛠️ 1. Lingkungan Kerja & Kompilasi Engine (Setup & Build)

Pastikan Anda berada di direktori utama proyek `/home/panjikali/Projects/vector_engine`.

### a. Menjalankan Python Virtual Environment
```bash
./.venv/bin/python --version
```

### b. Kompilasi Ulang Module Native Rust (Jika Mengubah Kode Rust)
```bash
./.venv/bin/maturin develop
```

### c. Menjalankan Unit Test Rust (27+ Test Geometri)
```bash
cargo test
```

---

## 🚀 2. Mode Eksekusi Fleksibel via CLI Flag (`run_batch.py`)

Anda dapat mengontrol secara langsung fitur/mode mana saja yang ingin diekspor dari terminal tanpa perlu mengubah file konfigurasi JSON.

### 🚩 Daftar Mode Flag CLI:

| Flag | Fungsi | File Output |
| :--- | :--- | :--- |
| **`--single`** | Ekspor desain gambar tunggal berukuran standar | `file_single.eps`, `file_single.jpg` |
| **`--double`** (atau `--bad-two`) | Ekspor Papan 2 Panel (*Asli \| Silhouette*) | `file_bad_two.eps`, `file_bad_two.jpg` |
| **`--triple`** (atau `--bad-three`) | Ekspor Papan 3 Panel (*Asli \| Greyscale \| Dominant*) | `file_bad_three.eps`, `file_bad_three.jpg` |
| **`--pattern`** | Ekspor Seamless Pattern saja | `file_pattern.eps`, `file_pattern.jpg` |
| **`--style <nama_style>`** | Mengganti style pattern langsung dari terminal | *(Digunakan bersama `--pattern`)* |
| **`--clean`** (atau `--no-emoji`) | Mode teks bersih tanpa emoji (*bebas dari kotak glyph `[01F]`*) | Log terminal 100% kompatibel di semua font |

---

## 🖥️ 3. Format Log Terminal Bersih & Rapi (`--clean`)

Jika terminal Anda menggunakan font bawaan yang belum memiliki dukungan font emoji, tambahkan flag `--clean` saat menjalankan script:

```bash
./.venv/bin/python run_batch.py --input input --output output --pattern --style half_drop_vertical --clean
```

Tampilan log di terminal akan menjadi **100% bersih, rapi, tanpa kotak glyph**:

```text
================================================================================
 [ENGINE] Vector Geometry Engine - Multi-Phase Batch Queue Pipeline
================================================================================
 [INPUT] Input Directory  : input
 [OUTPUT] Output Directory : output
 [PROFILE] Adaptive Profiles: 'low_color_bad_two' (<=5 colors) | 'multi_color_bad_two' (>=6 colors)
 [CANVAS] Target Canvas    : 19,000,000 px to 22,000,000 px total area
================================================================================

 Found 5 SVG file(s) in queue to process...

────────────────────────────────────────────────────────────────────────────────
 [FILE] [1/5] Processing File: Design_003.svg (8 Colors | Profile: 'multi_color_bad_two')
────────────────────────────────────────────────────────────────────────────────
  [P1] Phase 1 (Outset & Combine Geometry in RAM)  :   0.12 s
  [P2] Phase 2 (Livarot Node Simplification)        :   0.09 s
  [P3] Phase 3 (Color Isolation & Silhouette)      :   0.17 s
  [P4] Phase 4 (Canvas Auto-Scale & Sync)          :   0.63 s
  [P7] Phase 7 (Seamless Pattern: style='half_drop_vertical')     :   7.40 s
  [P6] Phase 6 (Disk Export EPS & JPG Render)      :   7.40 s

  --------------------------------------------------------------------------------
  [TIME] PHASE TIMING & BOTTLENECK SUMMARY (Design_003.svg)
  --------------------------------------------------------------------------------
   [P1] Phase 1: Outset & Combine Geometry      :   0.12 s  (  1.5%)
   [P2] Phase 2: Livarot Node Simplification    :   0.09 s  (  1.0%)
   [P3] Phase 3: Color Isolation & Silhouette   :   0.17 s  (  2.0%)
   [P4] Phase 4: Canvas Auto-Scale & Sync       :   0.63 s  (  7.4%)
   [P7] Phase 7: Seamless Pattern Generation    :   7.40 s  ( 87.0%)
   [P6] Phase 6: Disk Export EPS & JPG Render   :   7.40 s  ( 87.0%) [*] [PRIMARY BOTTLENECK]
  --------------------------------------------------------------------------------
  Finished Design_003.svg in 8.50 s | Bottleneck: [P6] Phase 6: Disk Export EPS & JPG Render (7.40 s)

================================================================================
 [SUMMARY] BATCH QUEUE SUMMARY REPORT
================================================================================
 File Name                        | Time (s)  | Output Deliverables        | Status
-------------------------------------------------------------------------------------
 Design_003.svg                   |    8.50 s | pattern.eps, pattern.jpg   | [OK] SUCCESS
-------------------------------------------------------------------------------------
 [TIME] Total Queue Time : 8.50 s
 Total Processed   : 1 File(s) (1 Successful, 0 Failed)
================================================================================
```

---

## 🎨 4. Opsi Style Pattern Yang Tersedia (`--style`)

- **`half_drop_vertical`** *(Half-drop per kolom vertikal)*
- **`brick_wall`** *(Susunan bata horizontal)*
- **`diamond_stagger`** *(Matriks belah ketupat)*
- **`checker_pinwheel`** *(Rotasi kincir 90°)*
- **`scale_rhythm`** *(Irama skala ganda 100% & 75%)*
- **`wave_sine_offset`** *(Gelombang sinus meliuk)*
- **`kaleidoscope_mirror`** *(Simetri cermin 4 arah 2x2)*
- **`slanted_left`** *(Miring kiri)*
- **`slanted_right`** *(Miring kanan)*
- **`grid`** *(Grid lurus)*
- **`alternating`** *(Cermin selang-seling)*
