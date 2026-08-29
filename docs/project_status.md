# 📋 Master Project Status & Features Log - Vector Geometry Engine

Dokumen ini memuat catatan lengkap progress pengembangan, pencapaian fitur, pengujian performa, dan arsitektur engine geometri **Vector Geometry Engine**.

---

## 📅 Status Proyek & Ringkasan Pencapaian (Update Terbaru)

| Tanggal | Fitur / Komponen | Status | Hasil & Kecepatan |
|---|---|---|---|
| **2026-08-29** | **Penyederhanaan Node Vektor Rust Murni** | ✅ **Selesai** | Migrasi Phase 2 murni ke Rust. Menghapus dependensi FFI C++ Livarot/Inkscape. Menggunakan RDP & Bezier curve fitting murni Rust. |
| **2026-08-29** | **Baking Matriks Transformasi SVG** | ✅ **Selesai** | Mewariskan & membakar (*baking*) properti `transform` SVG langsung ke koordinat absolut geometry selama parsing awal (`vector-io`). |
| **2026-08-29** | **Deteksi & Klasterisasi Spasial Badge** | ✅ **Selesai** | Menggunakan algoritma Union-Find dan Multi-Factor Confidence Scoring untuk mendeteksi layout grid badge dan menyesuaikan pembatasan fase ekspor. |
| **2026-08-29** | **Optimasi FFI & Reduksi Oversubscription** | ✅ **Selesai** | Fungsi `get_svg_dimensions` memotong overhead JSON, dan `init_rayon_threads` mengunci batas thread pool Rayon per proses untuk mencegah lag. |
| **2026-07-31** | **Presisi Fit Seamless Pattern (Zero Gap)** | ✅ **Selesai** | Sub-pixel edge snapping ($0.05\text{ px}$) menyambungkan batas ubin presisi tanpa sela/celah. |
| **2026-07-31** | **Ekstraksi Background Dinamis 100% Solid** | ✅ **Selesai** | Deteksi warna background otomatis dari area terluas dan diganti dengan persegi panjang solid full-canvas di layer 0. |
| **2026-07-31** | **Multi-Core CPU Parallel Processing (12 Thread)** | ✅ **Selesai** | Rayon `par_iter` untuk klipping geometri & `ThreadPoolExecutor` paralel untuk Inkscape EPS/PNG render pada 12 core CPU. |
| **2026-07-31** | **Clipping Sutherland-Hodgman Canvas EPS** | ✅ **Selesai** | Seluruh elemen vektor terpotong rapi di batas canvas. |

---

## 🚀 Detail Fitur Utama

### 1. Ekstraksi Dynamic Solid Background & Seamless Pres-Fit
- **Dynamic Background Colors**: Engine mendeteksi warna dasar artwork secara otomatis dengan memindai elemen path yang menutupi $\ge 70\%$ dari total area kanvas.
- **Solid Full-Canvas Replacement**: Background asli yang berlubang/terpotong dihapus dan digantikan secara otomatis dengan persegi panjang solid yang membentang $100\%$ menutupi kanvas pada Layer 0. Hal ini menjamin background tetap utuh (tidak bolong) saat di-drag keluar di Adobe Illustrator.
- **Sub-Pixel Edge Snapping ($0.05\text{ px}$)**: Vertex vektor pada tepi ubin pattern dipasang presisi (*snapped*) sehingga sambungan ubin pattern bebas dari celah pixel putih.

### 2. Baking Matriks Transformasi SVG (Transform Matrix Baking)
- **Transform Inheritance**: Mengumpulkan properti matriks `transform` (seperti `scale`, `translate`, `rotate`, `matrix`) saat menelusuri elemen SVG secara rekursif (`traverse_node` di `crates/vector-io/src/svg.rs`).
- **Absolute Coordinate Baking**: Nilai matriks tersebut langsung dikalikan dan dibakar ke koordinat titik absolut (`Point2D`) saat pembuatan geometry path. Hal ini memecahkan masalah posisi/skala yang bergeser saat gambar SVG diimpor.

### 3. Deteksi & Pengelompokan Spasial Badge (Badge Clustering & Scoring)
- **Spatial Union-Find Clustering**: Mengelompokkan elemen geometry yang terpisah secara spasial menjadi beberapa klaster lencana/badge kustom menggunakan algoritma Disjoint Set.
- **Multi-Factor Scoring**: Menghitung skor keyakinan klaster badge berdasarkan rasio aspek, simetri kiri-kanan, kepadatan konsentris, margin kanvas, kontur pembungkus, dan kata kunci nama ID layer.
- **Automatic Optimization Bypass**: Jika dokumen terdeteksi sebagai Grid Badge:
  - Fase penyederhanaan kurva (Phase 2) dilewati secara otomatis untuk mencegah rusaknya elemen teks/font lencana.
  - Fase siluet dinonaktifkan.
  - Mengekspor berkas dalam mode tunggal saja (`_single.eps`, `_single.jpg`, `_single.svg`).

### 4. Manajemen Threadpool & Kecepatan FFI (Oversubscription Prevention)
- **FFI Dimension Scanner**: Menggunakan `get_svg_dimensions` untuk mem-parse ukuran kanvas langsung di memori Rust, menghindari beban alokasi dan pembacaan string JSON geometry raksasa di Python.
- **Rayon Child Thread Limiting**: Membatasi jumlah thread Rayon (`init_rayon_threads`) pada setiap proses paralel Python secara dinamis (`max(1, cpu_count // num_workers)`) sehingga CPU core bekerja seimbang tanpa memicu lag sistem operasi.

---

## 🛠️ Cara Penggunaan CLI Terminal

```bash
# Menjalankan Batch Queue Pattern dengan Pres-Fit & Dynamic Background utuh:
python run_batch.py --input input --output output --pattern --style half_drop_vertical

# Menggunakan mode tampilan log clean ASCII (bebas glitch icon di XFCE):
python run_batch.py --input input --output output --pattern --style half_drop_vertical --clean
```
