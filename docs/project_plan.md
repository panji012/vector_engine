# Rencana Proyek: Vector Geometry Engine
*(Dokumen Perencanaan Lengkap End-to-End)*

## 1. Pendahuluan
Membangun sebuah *Vector Geometry Engine* berbasis Rust yang sangat optimal untuk memanipulasi, menyederhanakan, dan mengekspor grafis vektor. Target utamanya adalah untuk memproses geometri SVG yang kompleks secara *batch* menggunakan skrip Python (`run_batch.py`) dan menyiapkan *binding* untuk integrasi ekosistem eksternal seperti C/C++ (GUI) dan WebAssembly.

## 2. Fase 1: Fondasi Geometri & Stabilitas Inti (Selesai)
- [x] **Parsing SVG & Ekstraksi Node**: Membaca file SVG kompleks dan mengekstrak perintah path (`vector_core`).
- [x] **Transformasi Hirarki (Transform Stack)**: Membaca matriks transformasi (Affine) dari `<g transform="...">` dan menerapkannya langsung ke path dasar (mencegah *Phantom Node*).
- [x] **Konversi Objek ke Path**: Secara otomatis mengubah `<rect>`, `<circle>`, dan `<ellipse>` menjadi perintah `<path>` murni.
- [x] **Color Combiner**: Menggabungkan path dengan warna yang sama untuk mengurangi jumlah *node* dan mempercepat *rendering*.
- [x] **Background Normalization**: Mendeteksi layar *background* penuh dan menggantinya dengan poligon kotak bersih tanpa memicu cacat Boolean Z-Index.

## 3. Fase 2: Operasi Boolean & Outset/Inset Poligon (Selesai/Stabil)
- [x] **Konversi Path ke Poligon Poligonal (Flattening)**: Mengubah kurva Bezier menjadi himpunan titik-titik rapat.
- [x] **Konstruksi Topologi Per-Warna**: Mendeteksi objek di luar (*Exterior*) dan lubang (*Hole*) menggunakan *ray-casting point-in-polygon* khusus untuk objek berwarna identik.
- [x] **Algoritma Offset/Outset Presisi**: Menghitung *normal vector* dan merelokasi titik-titik untuk ekspansi luar atau kontraksi dalam.
- [x] **Penyederhanaan Node RDP**: Memanfaatkan Ramer-Douglas-Peucker untuk membuang titik-titik collinear yang berlebihan (mengurangi 4%-13% node).
- [x] **Resolusi Self-Intersection**: Mencegah *bowtie artifacts* jika garis outset bertabrakan dengan dirinya sendiri melalui *boolean trimming*.
- [x] **Binding FFI Python (Maturin/PyO3)**: Integrasi pustaka Rust murni (`vector_algo`) agar bisa dieksekusi secara *native* lewat Python `run_batch.py`.

## 4. Fase Lanjutan: Multi-Phase Engine Expansion (Selesai/Stabil)
- [x] **FFI Simplify C++ Inkscape**: Integrasi eksekusi aksi penyederhanaan path (*Simplify*) menggunakan script & FFI bridge C++ Inkscape CLI.
- [x] **Pemisahan Background & Modulasi Luminansi**:
  - **Desain 1 (Biru)**: Formula luminansi $60\% + (100\% - 60\%) \times 50\% = 80\%$ untuk background dengan palet warna utama biru.
  - **Desain 2 (Coklat)**: Formula luminansi $95\% + (100\% - 95\%) \times 50\% = 97.5\%$ untuk background dengan palet warna utama coklat.
- [x] **Canvas Auto-Scale Presisi**: Modul pengatur skala kanvas otomatis yang menjamin total luas piksel $W \times H$ berada pada rentang **19.000.000 px hingga 22.000.000 px** (nominal 20.500.000 px).
- [x] **SVG Bad (Two) Dual Variant Board**: Menggabungkan Desain 1 (kiri) dan Desain 2 (kanan) secara berdampingan dalam satu kanvas SVG ganda.
- [x] **Output EPS & JPG Multi-Format**: Sistem ekspor otomatis untuk menghasilkan file EPS dan JPG dari berkas SVG tunggal dan SVG Bad Two.
- [x] **Deteksi & Generator Circular Pattern ("Bulat Melingkar")**: Memeriksa kriteria bentuk bulat/melingkar dan mengonversinya menjadi *seamless 3x3 repeating pattern SVG/EPS/JPG*.

## 5. Fase Optimasi Performa & Paralelisasi (Multi-threading)
- [ ] **Integrasi Penuh Rayon**: Memperluas eksekusi *multi-threading* (paralel) pada tahap bottleneck komputasi: *Flattening*, *Boolean Resolving*, dan *Bezier Fitting*.
- [ ] **Optimasi Alokasi Memori**: Menghapus alokasi duplikat (`clone` dan relokasi heap/`Vec`) pada tahapan iterasi warna dan ekstraksi koordinat.
- [ ] **Instruksi SIMD (Single Instruction Multiple Data)**: Menerapkan fitur prosesor modern (AVX2/NEON) pada *loop* kalkulasi vektor/matriks yang intensif untuk mempercepat operasi trigonometri secara eksponensial.

## 6. Fase Portabilitas Platform & Publikasi FFI (C/C++ & WASM)
- [ ] **Export C/C++ Headers**: Menyempurnakan direktori `crates/vector-ffi` agar dapat diekspor secara dinamis/statis (`.so` / `.dll` / `.a`) untuk dipanggil via standar C ABI di aplikasi Desktop/GUI pihak ketiga.
- [ ] **Kompilasi WebAssembly (WASM)**: Mengadaptasi pustaka `vector_algo` menggunakan `wasm-bindgen` agar *engine* ini bisa berjalan secara penuh langsung di dalam *browser* (dengan JavaScript/TypeScript) tanpa server *backend*.
- [ ] **Rilis CLI Penuh (`vector-cli`)**: Memfinalisasi fitur baris perintah (CLI) murni (di luar *environment* Python) agar *engine* bisa dipaketkan sebagai *binary* mandiri yang super ringan.

## 7. Aturan Main Pengembangan (Development Workflow Rules)
1. **Kompilasi Wajib**: Setiap pengubahan logika di dalam *crate* Rust mana pun (`crates/vector-offset`, `crates/vector-algo`) wajib dieksekusi dengan perintah `maturin develop --release` (di dalam virtual environment `.venv`) sebelum melakukan pengujian Python.
2. **Validasi Rendering**: Setiap fitur yang memodifikasi hasil keluaran koordinat wajib diverifikasi secara visual melalui penampil SVG standar menggunakan berkas vektor yang sangat kompleks (misal: himpunan ilustrasi `Remix_AI_*.svg`).
3. **Pencatatan Status Berkesinambungan**: Selalu perbarui dokumen `docs/project_status.md` pada akhir pengerjaan setiap sub-fase untuk menjaga kelancaran alur serah-terima (handover) antar agen/pengembang AI di sesi mendatang.

---
**Status Revisi Dokumen**: Juli 2026

