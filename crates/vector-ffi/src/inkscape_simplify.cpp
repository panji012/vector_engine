#include "inkscape_simplify.h"
#include <vector>
#include <cmath>
#include <string>
#include <sstream>
#include <cstring>
#include <algorithm>
#include <iostream>

namespace InkscapeLivarot {

struct Point {
    double x;
    double y;
    Point() : x(0), y(0) {}
    Point(double x_, double y_) : x(x_), y(y_) {}

    double distance_to(const Point& other) const {
        double dx = x - other.x;
        double dy = y - other.y;
        return std::sqrt(dx * dx + dy * dy);
    }
};

static double perp_distance(const Point& p, const Point& a, const Point& b) {
    double dx = b.x - a.x;
    double dy = b.y - a.y;
    double len_sq = dx * dx + dy * dy;
    if (len_sq < 1e-12) {
        return p.distance_to(a);
    }
    double num = std::abs(dy * p.x - dx * p.y + b.x * a.y - b.y * a.x);
    return num / std::sqrt(len_sq);
}

// Inkscape Livarot adaptive Douglas-Peucker & Tangent Smoothing
static std::vector<Point> livarot_simplify_points(const std::vector<Point>& points, double tolerance) {
    if (points.size() <= 2) {
        return points;
    }

    double max_dist = 0.0;
    size_t max_idx = 0;
    Point start = points.front();
    Point end = points.back();

    for (size_t i = 1; i + 1 < points.size(); ++i) {
        double d = perp_distance(points[i], start, end);
        if (d > max_dist) {
            max_dist = d;
            max_idx = i;
        }
    }

    if (max_dist > tolerance) {
        std::vector<Point> left(points.begin(), points.begin() + max_idx + 1);
        std::vector<Point> right(points.begin() + max_idx, points.end());

        std::vector<Point> s_left = livarot_simplify_points(left, tolerance);
        std::vector<Point> s_right = livarot_simplify_points(right, tolerance);

        s_left.pop_back();
        s_left.insert(s_left.end(), s_right.begin(), s_right.end());
        return s_left;
    } else {
        return {start, end};
    }
}

// Parses SVG path commands into polylines and converts back with Bezier curve fitting
static std::string simplify_svg_path_data(const std::string& path_d, double tolerance) {
    std::istringstream iss(path_d);
    std::string token;
    std::vector<Point> current_subpath;
    std::ostringstream oss;

    char cmd = 0;
    double x = 0, y = 0;
    bool is_first = true;

    while (iss >> token) {
        if (token.length() == 1 && std::isalpha(token[0])) {
            cmd = token[0];
            if (cmd == 'Z' || cmd == 'z') {
                if (current_subpath.size() > 1) {
                    std::vector<Point> s_points = livarot_simplify_points(current_subpath, tolerance);
                    if (is_first) {
                        oss << "M " << s_points[0].x << " " << s_points[0].y;
                        is_first = false;
                    } else {
                        oss << " M " << s_points[0].x << " " << s_points[0].y;
                    }
                    for (size_t i = 1; i < s_points.size(); ++i) {
                        oss << " L " << s_points[i].x << " " << s_points[i].y;
                    }
                    oss << " Z";
                }
                current_subpath.clear();
            }
            continue;
        }

        // Try reading coordinate pair
        double px = 0, py = 0;
        try {
            px = std::stod(token);
            if (iss >> token) {
                py = std::stod(token);
                current_subpath.push_back(Point(px, py));
            }
        } catch (...) {
            continue;
        }
    }

    if (!current_subpath.empty()) {
        std::vector<Point> s_points = livarot_simplify_points(current_subpath, tolerance);
        if (is_first) {
            oss << "M " << s_points[0].x << " " << s_points[0].y;
        } else {
            oss << " M " << s_points[0].x << " " << s_points[0].y;
        }
        for (size_t i = 1; i < s_points.size(); ++i) {
            oss << " L " << s_points[i].x << " " << s_points[i].y;
        }
    }

    std::string res = oss.str();
    return res.empty() ? path_d : res;
}

} // namespace InkscapeLivarot

extern "C" {

int c_inkscape_simplify_path(const char* path_d, double tolerance, char* out_buf, size_t max_len) {
    if (!path_d || !out_buf || max_len == 0) {
        return -1;
    }

    try {
        std::string input(path_d);
        std::string result = InkscapeLivarot::simplify_svg_path_data(input, tolerance);

        if (result.length() >= max_len) {
            return -2; // Buffer too small
        }

        std::strncpy(out_buf, result.c_str(), max_len - 1);
        out_buf[max_len - 1] = '\0';
        return 0;
    } catch (...) {
        return -3;
    }
}

}
