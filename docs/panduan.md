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
| **`--single`** | Ekspor desain gambar tunggal berukuran standar | `file_single.eps`, `file_single.jpg`, `file_single.svg` |
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
 [FILE] [6/6] Processing File: Design_027.svg (3 Colors | Profile: 'low_color_bad_two')
────────────────────────────────────────────────────────────────────────────────
  [P1] Phase 1 (Outset & Combine Geometry in RAM)  :   0.02 s
  [P2] Phase 2 (In-Memory Node Simplification)     :   0.00 s [NATIVE RUST]
  [P3] Phase 3 (Color Isolation & Silhouette)      :   0.07 s
  [P5] Phase 4 (Bad Board Generation: bad_two)     :   0.03 s
  [P7] Phase 5 (Seamless Pattern: style='brick_wall')     :   0.63 s
  [P4] Phase 6 (Canvas Auto-Scale 1x Target)      :   0.00 s
  [P6] Phase 7 (Disk Export EPS & JPG Render)      :   1.51 s

  --------------------------------------------------------------------------------
  [TIME] PHASE TIMING & BOTTLENECK SUMMARY (Design_027.svg)
  --------------------------------------------------------------------------------
   [P1] Phase 1: Outset & Combine Geometry      :   0.02 s  (  0.8%)
   [P2] Phase 2: In-Memory Node Simplification  :   0.00 s  (  0.0%)
   [P3] Phase 3: Color Isolation & Silhouette   :   0.07 s  (  3.2%)
   [P5] Phase 4: Bad Board Generation           :   0.03 s  (  1.6%)
   [P7] Phase 5: Seamless Pattern Generation    :   0.63 s  ( 27.2%)
   [P4] Phase 6: Canvas Auto-Scale 1x Target    :   0.00 s  (  0.0%)
   [P6] Phase 7: Disk Export EPS & JPG Render   :   1.51 s  ( 63.8%) [*] [PRIMARY BOTTLENECK]
  --------------------------------------------------------------------------------
  Finished Design_027.svg in 2.35 s | Bottleneck: [P6] Phase 7: Disk Export EPS & JPG Render (1.51 s)

================================================================================
 [SUMMARY] BATCH QUEUE SUMMARY REPORT
================================================================================
 File Name                        | Time (s)  | Output Deliverables        | Status
-------------------------------------------------------------------------------------
 Design_027.svg                   |    2.35 s | bad_two.eps, pattern.eps   | [OK] SUCCESS
-------------------------------------------------------------------------------------
 [TIME] Total Queue Time : 3.49 s
 Total Processed   : 6 File(s) (6 Successful, 0 Failed)
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

---

## 📁 5. Arsitektur 3-Folder & Sistem Absensi Mandiri per Node

Sistem menggunakan alur 3 folder utama yang ringkas dan mandiri:

1. **`input/` (Folder Bahan Kerja Aktif)**:
   - Tempat pengunggahan JPG mentah (`vector_stock_0001.jpg`).
   - **Node 2** mengunduh file `.svg` langsung ke folder `input/` (`vector_stock_0001.svg`).
   - **Node 3** membaca file `.svg` dari `input/` sebagai bahan olahan geometri.

2. **`output/` (Folder Hasil Olahan Node 3)**:
   - Tempat Node 3 merender berkas luaran EPS & JPG (Low Color, Full Color, Pattern, Single).
   - **Node 4** menginjeksi metadata (Title, Description, Keywords) ke berkas EPS & JPG di folder ini.

3. **`stock_ready/` (Folder Stok Siap Upload - Grouping 30 Stock)**:
   - Setelah Node 4 selesai menginjeksi metadata, berkas dipindahkan ke `stock_ready/`.
   - Berkas dikelompokkan secara otomatis ke dalam sub-folder per 30 stok (`stock_ready/batch_001/`, `stock_ready/batch_002/`, dst.) sehingga siap di-upload langsung ke platform microstock.

### 🛡️ Sistem Absensi Mandiri (Smart Skip):
Tiap Node melakukan pengecekan **Absensi (manifest/file check)** secara mandiri:
- **Node 2**: Jika SVG sudah ada di `input/` atau di `stock_ready/` $\rightarrow$ **SKIP (Absen Pass)**.
- **Node 3**: Jika EPS/JPG sudah ada di `output/` atau di `stock_ready/` $\rightarrow$ **SKIP (Absen Pass)**.
- **Node 4**: Mengelompokkan luaran dari `output/` ke sub-folder 30 stok di `stock_ready/`.
