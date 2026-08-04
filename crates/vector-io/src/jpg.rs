use image::codecs::jpeg::JpegEncoder;
use image::{ImageBuffer, Rgb};
use std::io::Cursor;
use vector_core::{Point2D, Polygon2D, VectorCoreError};

pub struct JpgPreviewGenerator;

impl JpgPreviewGenerator {
    /// Renders an SVG XML string into a JPEG byte array natively in Rust with white background composite.
    pub fn render_svg_str_jpg(
        svg_str: &str,
        quality: u8,
    ) -> Result<Vec<u8>, VectorCoreError> {
        use resvg::tiny_skia::{Pixmap, Transform};
        use resvg::usvg::{Options, Tree, TreeParsing};

        let opt = Options::default();
        let tree = Tree::from_str(svg_str, &opt)
            .map_err(|e| VectorCoreError::IoError(format!("USVG parse error: {}", e)))?;

        let size = tree.size.to_int_size();
        let width = size.width();
        let height = size.height();

        if width == 0 || height == 0 {
            return Err(VectorCoreError::IoError("Invalid SVG dimensions for JPG rendering".to_string()));
        }

        let mut pixmap = Pixmap::new(width, height)
            .ok_or_else(|| VectorCoreError::IoError("Failed to allocate Pixmap".to_string()))?;

        resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());

        let mut img = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(width, height);
        let pixels = pixmap.data();
        if pixels.len() >= 20 {
            println!("Pixmap first 20 bytes: {:?}", &pixels[..20]);
        }

        for (i, pixel) in pixels.chunks_exact(4).enumerate() {
            let r = pixel[0];
            let g = pixel[1];
            let b = pixel[2];
            let a = pixel[3] as f32 / 255.0;

            let final_r = (r as f32 * a + 255.0 * (1.0 - a)).round() as u8;
            let final_g = (g as f32 * a + 255.0 * (1.0 - a)).round() as u8;
            let final_b = (b as f32 * a + 255.0 * (1.0 - a)).round() as u8;

            let x = (i as u32) % width;
            let y = (i as u32) / width;
            img.put_pixel(x, y, Rgb([final_r, final_g, final_b]));
        }

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        let mut encoder = JpegEncoder::new_with_quality(&mut cursor, quality.clamp(1, 100));

        encoder
            .encode_image(&img)
            .map_err(|e| VectorCoreError::IoError(format!("JPEG encoding failed: {}", e)))?;

        Ok(buffer)
    }

    /// Renders a `Polygon2D` into a JPG/JPEG preview byte array with configurable resolution.
    pub fn render_polygon_jpg(
        polygon: &Polygon2D,
        width: u32,
        height: u32,
        quality: u8,
    ) -> Result<Vec<u8>, VectorCoreError> {
        if width == 0 || height == 0 {
            return Err(VectorCoreError::IoError("Canvas resolution dimensions must be > 0".to_string()));
        }

        let mut img = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_pixel(width, height, Rgb([255, 255, 255]));

        if !polygon.exterior.is_empty() {
            let (min_x, min_y, max_x, max_y) = polygon.bounding_box();
            let bbox_w = (max_x - min_x).max(1e-5);
            let bbox_h = (max_y - min_y).max(1e-5);

            let margin = 0.1; // 10% margin
            let draw_w = width as f64 * (1.0 - 2.0 * margin);
            let draw_h = height as f64 * (1.0 - 2.0 * margin);

            let scale = (draw_w / bbox_w).min(draw_h / bbox_h);

            let offset_x = (width as f64 - bbox_w * scale) / 2.0 - min_x * scale;
            let offset_y = (height as f64 - bbox_h * scale) / 2.0 - min_y * scale;

            let map_pt = |pt: &Point2D| -> (i32, i32) {
                let px = (pt.x * scale + offset_x).round() as i32;
                let py = (pt.y * scale + offset_y).round() as i32;
                (px, py)
            };

            let poly_pixels: Vec<(i32, i32)> = polygon.exterior.iter().map(map_pt).collect();

            // Scanline fill rasterization
            let foreground = Rgb([30, 136, 229]); // Professional Blue
            fill_polygon_pixels(&mut img, &poly_pixels, foreground);

            // Draw crisp outline
            let outline_color = Rgb([21, 101, 192]);
            draw_polygon_outline(&mut img, &poly_pixels, outline_color);
        }

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        let mut encoder = JpegEncoder::new_with_quality(&mut cursor, quality.clamp(1, 100));

        encoder
            .encode_image(&img)
            .map_err(|e| VectorCoreError::IoError(format!("JPEG encoding failed: {}", e)))?;

        Ok(buffer)
    }
}

fn fill_polygon_pixels(img: &mut ImageBuffer<Rgb<u8>, Vec<u8>>, ring: &[(i32, i32)], color: Rgb<u8>) {
    let width = img.width() as i32;
    let height = img.height() as i32;
    let n = ring.len();
    if n < 3 {
        return;
    }

    let min_y = ring.iter().map(|p| p.1).min().unwrap_or(0).clamp(0, height - 1);
    let max_y = ring.iter().map(|p| p.1).max().unwrap_or(0).clamp(0, height - 1);

    for y in min_y..=max_y {
        let mut node_x = Vec::new();
        let mut j = n - 1;
        for i in 0..n {
            let (xi, yi) = ring[i];
            let (xj, yj) = ring[j];
            if (yi < y && yj >= y) || (yj < y && yi >= y) {
                let dy = (yj - yi) as f64;
                let dx = (xj - xi) as f64;
                if dy.abs() > 1e-9 {
                    let x = xi + (((y - yi) as f64 * dx) / dy).round() as i32;
                    node_x.push(x);
                }
            }
            j = i;
        }
        node_x.sort_unstable();

        for chunk in node_x.chunks(2) {
            if chunk.len() == 2 {
                let x_start = chunk[0].clamp(0, width - 1);
                let x_end = chunk[1].clamp(0, width - 1);
                for x in x_start..=x_end {
                    img.put_pixel(x as u32, y as u32, color);
                }
            }
        }
    }
}

fn draw_polygon_outline(img: &mut ImageBuffer<Rgb<u8>, Vec<u8>>, ring: &[(i32, i32)], color: Rgb<u8>) {
    let n = ring.len();
    if n < 2 {
        return;
    }
    for i in 0..n {
        let next = (i + 1) % n;
        draw_line_bresenham(img, ring[i], ring[next], color);
    }
}

fn draw_line_bresenham(img: &mut ImageBuffer<Rgb<u8>, Vec<u8>>, p1: (i32, i32), p2: (i32, i32), color: Rgb<u8>) {
    let width = img.width() as i32;
    let height = img.height() as i32;

    let mut x0 = p1.0;
    let mut y0 = p1.1;

    let x1 = p2.0;
    let y1 = p2.1;

    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        if x0 >= 0 && x0 < width && y0 >= 0 && y0 < height {
            img.put_pixel(x0 as u32, y0 as u32, color);
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jpg_preview_encoding() {
        let poly = Polygon2D::new(vec![
            Point2D::new(0.0, 0.0), Point2D::new(100.0, 0.0),
            Point2D::new(100.0, 100.0), Point2D::new(0.0, 100.0)
        ]);

        let jpg_bytes = JpgPreviewGenerator::render_polygon_jpg(&poly, 512, 512, 85).unwrap();

        // Check JPEG magic bytes: 0xFF 0xD8 0xFF
        assert!(jpg_bytes.len() > 100);
        assert_eq!(jpg_bytes[0], 0xFF);
        assert_eq!(jpg_bytes[1], 0xD8);
        assert_eq!(jpg_bytes[2], 0xFF);
    }
}
