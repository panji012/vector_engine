# 📖 Panduan Lengkap Konfigurasi `config_silhouette.json`

Dokumen ini berisi panduan praktis untuk mengubah nilai kontrol parameter pada file konfigurasi `config_silhouette.json`.

---

## 🎨 Opsi Nilai untuk `"style"` (Seamless Pattern Layout)

Ganti nilai `"style"` pada `pattern_config` dengan salah satu string di bawah ini:

| Nama Style | Deskripsi & Perilaku Layout | Rekomendasi Penggunaan |
| :--- | :--- | :--- |
| **`"slanted_left"`** *(Default)* | Pergeseran miring ke kiri bertahap per baris. | Sangat cocok untuk ombak, gambar horizontal, emblem logo. |
| **`"slanted_right"`** | Pergeseran miring ke kanan bertahap per baris. | Cocok untuk garis diagonal dari kiri-bawah ke kanan-atas. |
| **`"half_drop_vertical"`** | **Half-Drop Vertikal**: Kolom selang-seling digeser 50% ke bawah. | Standar industri tekstil/wallpaper, menghilangkan garis horizontal. |
| **`"brick_wall"`** | **Brick Wall**: Baris selang-seling digeser 50% ke kanan (seperti susunan bata). | Sangat cocok untuk gambar berelemen teks mendatar. |
| **`"diamond_stagger"`** | **Diamond Matrix**: Pergeseran 50% horizontal & 50% vertikal simultan. | Memberikan kesan mewah (*luxury pattern*), menyamarkan garis grid. |
| **`"checker_pinwheel"`** | **Rotasi Kincir**: Ubin diputar 90° berurutan ($0^\circ, 90^\circ, 180^\circ, 270^\circ$). | Efek dinamis seperti baling-baling atau pusaran air. |
| **`"scale_rhythm"`** | **Skala Ganda**: Baris selang-seling berukuran 75% dari skala utama. | Menciptakan kedalaman visual (motif utama vs motif pendamping). |
| **`"wave_sine_offset"`** | **Gelombang Sinus**: Pergeseran baris mengikuti gelombang sinus meliuk. | Artistik, mengalir dinamis. |
| **`"kaleidoscope_mirror"`** | **Simetri 4 Arah**: Pencerminan $2 \times 2$ Quad Mirror (Flip X & Flip Y). | Simetris 100%, efek kristal/kaleidoskop. |
| **`"grid"`** | **Grid Standar**: Baris dan kolom lurus sejajar. | Pola klasik sederhana. |
| **`"alternating"`** | **Flipped Alternating**: Pencerminan cermin selang-seling (Flip X+Y). | Pola cermin berulang. |

---

## 🎛️ Penjelasan Setiap Kontrol Parameter pada `pattern_config`

```json
"pattern_config": {
    "enable": true,
    "detect_suitable_only": true,
    "allowed_shapes": ["circular", "square"],
    "style": "slanted_left",
    "slant_offset_ratio": -0.5,
    "wraparound_seamless": true,
    "grid_x": 3,
    "grid_y": 3,
    "seamless_combine": true
}
```

### 1. `"enable"`
- **Tipe**: `boolean` (`true` atau `false`)
- **Fungsi**: Mengaktifkan atau mematikan modul pembuatan seamless pattern.
- **Nilai**:
  - `true`: Ekspor file `.svg`, `.eps`, dan `.jpg` untuk pattern.
  - `false`: Lewati ekspor pattern (hanya buat bad_two/silhouette).

### 2. `"detect_suitable_only"`
- **Tipe**: `boolean` (`true` atau `false`)
- **Fungsi**: Memfilter apakah hanya gambar rasio sesuai yang dibuatkan pattern.
- **Nilai**:
  - `true`: Hanya buat pattern jika bentuk gambar terdeteksi bulat (*circular*) atau kotak (*square* 1:1).
  - `false`: **Selalu buat pattern untuk SEMUA gambar** tanpa memandang rasio bentuknya.

### 3. `"allowed_shapes"`
- **Tipe**: `Array of String`
- **Fungsi**: Daftar bentuk gambar yang diizinkan untuk dibuatkan pattern saat `detect_suitable_only: true`.
- **Nilai**: `["circular", "square"]`

### 4. `"style"`
- **Tipe**: `String`
- **Fungsi**: Menentukan gaya tata letak ubin pattern (lihat tabel opsi di atas).

### 5. `"slant_offset_ratio"`
- **Tipe**: `Float` (Contoh: `-0.5`, `0.5`, `-0.33`, `0.33`)
- **Fungsi**: Menentukan besarnya rasio pergeseran miring untuk style `slanted_left` atau `slanted_right`.
- **Nilai**:
  - `-0.5`: Geser 50% lebar gambar ke kiri per baris (Default).
  - `0.5`: Geser 50% lebar gambar ke kanan per baris.

### 6. `"wraparound_seamless"`
- **Tipe**: `boolean` (`true` atau `false`)
- **Fungsi**: Membuat duplikat geometri tepi melingkar (*toroidal wraparound*) di batas kiri, kanan, atas, dan bawah.
- **Nilai**:
  - `true` (Direkomendasikan): Menjamin pattern menyatu 100% *seamless* di batas canvas.

### 7. `"grid_x"` & `"grid_y"`
- **Tipe**: `Integer` (Contoh: `3`, `4`, `5`)
- **Fungsi**: Jumlah pengulangan ubin ke arah horizontal (`grid_x`) dan vertikal (`grid_y`).
- **Nilai**:
  - `3`: Grid $3 \times 3$ ubin.
  - `4`: Grid $4 \times 4$ ubin.

---

## 🔧 Parameter Utama Lainnya (Vector Processing)

- **`"gap"`**: Jarak penggabungan kontur (default `50.0`).
- **`"min_area_px"` & `"max_area_px"`**: Target luas total area canvas halaman saat ekspor otomatis (default `19,000,000` px s/d `22,000,000` px).
- **`"color_merge_threshold"`**: Ambang batas toleransi penggabungan warna yang mirip (default `0.15`).
