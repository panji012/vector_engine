#ifndef INKSCAPE_SIMPLIFY_H
#define INKSCAPE_SIMPLIFY_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Inkscape Livarot-style C++ Path Simplification ABI Function.
 *
 * Takes SVG path command data string `path_d` and error tolerance parameter.
 * Simplifies polyline & Bezier node clusters using Douglas-Peucker & Livarot curve fitting.
 * Writes result into `out_buf` (up to `max_len` bytes).
 *
 * @return 0 on success, non-zero on failure.
 */
int c_inkscape_simplify_path(const char* path_d, double tolerance, char* out_buf, size_t max_len);

#ifdef __cplusplus
}
#endif

#endif // INKSCAPE_SIMPLIFY_H
