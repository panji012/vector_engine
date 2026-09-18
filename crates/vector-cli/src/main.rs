use clap::Parser;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use rayon::prelude::*;

use vector_core::{VectorDocument, VectorPath, PathCommand, Point2D};
use vector_io::{SvgParser, EpsWriter, JpgPreviewGenerator};
use vector_algo::{
    BackgroundProcessor, ColorCombiner, GeometryOutset, DuoBoardGenerator,
    LuminanceEngine, CanvasScaler, PatternDetector, PatternGenerator
};

/// Command line arguments struct for Vector Geometry Engine CLI
#[derive(Parser, Debug)]
#[command(
    name = "vector-cli",
    author = "Vector Engine Contributors",
    version = env!("CARGO_PKG_VERSION"),
    about = "High-performance 100% Native Rust Vector Geometry Engine CLI"
)]
struct CliArgs {
    /// Input directory or single SVG file path
    #[arg(short, long, default_value = "input")]
    input: PathBuf,

    /// Output directory to save processed SVG files
    #[arg(short, long, default_value = "output")]
    output: PathBuf,

    /// Optional configuration file path (YAML or JSON)
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Worker thread pool size (0 = auto-detect CPU cores)
    #[arg(short = 'w', long, default_value_t = 0)]
    workers: usize,

    /// Outset distance override in pixels
    #[arg(long)]
    distance: Option<f64>,

    /// Node simplification tolerance override
    #[arg(long)]
    tolerance: Option<f64>,

    /// Silhouette threshold override
    #[arg(long)]
    silhouette_threshold: Option<f64>,

    /// Comma-separated list of layer indexes to remove
    #[arg(long)]
    remove_layers: Option<String>,

    /// Minimum nodes override
    #[arg(long)]
    min_nodes: Option<usize>,

    /// Line art override
    #[arg(long)]
    line_art: Option<bool>,

    /// Background color override
    #[arg(long)]
    bg_color: Option<String>,

    /// Foreground color override
    #[arg(long)]
    fg_color: Option<String>,

    /// Invert colors override
    #[arg(long)]
    invert: Option<bool>,

    /// Layout gap override
    #[arg(long)]
    gap: Option<f64>,

    /// Target minimum area override
    #[arg(long)]
    min_area: Option<f64>,

    /// Target maximum area override
    #[arg(long)]
    max_area: Option<f64>,

    /// Export single artwork design ONLY
    #[arg(long, default_value_t = false)]
    single: bool,

    /// Export 2-panel bad board ONLY
    #[arg(long, default_value_t = false)]
    double: bool,

    /// Export 3-panel bad board ONLY
    #[arg(long, default_value_t = false)]
    triple: bool,

    /// Export seamless pattern ONLY
    #[arg(long, default_value_t = false)]
    pattern: bool,

    /// Override pattern layout style
    #[arg(long)]
    style: Option<String>,

    /// Disable emoji logs for clean text output
    #[arg(long, default_value_t = false)]
    clean: bool,
}

#[derive(Debug, Clone)]
struct ProfileConfig {
    max_colors: Option<usize>,
    min_colors: Option<usize>,
    threshold: f64,
    tolerance: f64,
    distance: f64,
    color_merge_threshold: f64,
    remove_layers: Vec<usize>,
    min_nodes: usize,
    line_art: bool,
    bg_color: String,
    fg_color: String,
    invert: bool,
    gap: f64,
    min_area_px: f64,
    max_area_px: f64,
    export_single: bool,
    export_trio: bool,
    pattern_enable: bool,
    pattern_style: String,
    pattern_slant_offset_ratio: f64,
    pattern_grid_x: u32,
    pattern_grid_y: u32,
    pattern_seamless_combine: bool,
    middle_panel_min_lum: f64,
    middle_panel_max_lum: f64,
    right_panel_min_lum: f64,
    right_panel_max_lum: f64,
}

fn color_green(text: &str, clean: bool) -> String {
    if clean { text.to_string() } else { format!("\x1b[32m{}\x1b[0m", text) }
}
fn color_red(text: &str, clean: bool) -> String {
    if clean { text.to_string() } else { format!("\x1b[31m{}\x1b[0m", text) }
}
fn color_yellow(text: &str, clean: bool) -> String {
    if clean { text.to_string() } else { format!("\x1b[33m{}\x1b[0m", text) }
}
fn color_cyan(text: &str, clean: bool) -> String {
    if clean { text.to_string() } else { format!("\x1b[36m{}\x1b[0m", text) }
}
fn color_magenta(text: &str, clean: bool) -> String {
    if clean { text.to_string() } else { format!("\x1b[35m{}\x1b[0m", text) }
}
fn color_bold(text: &str, clean: bool) -> String {
    if clean { text.to_string() } else { format!("\x1b[1m{}\x1b[0m", text) }
}
fn color_gray(text: &str, clean: bool) -> String {
    if clean { text.to_string() } else { format!("\x1b[90m{}\x1b[0m", text) }
}

fn format_duration(seconds: f64, clean: bool) -> String {
    let formatted = format!("{:.2} s", seconds);
    if clean {
        formatted
    } else if seconds >= 0.5 {
        format!("\x1b[1;31m{}\x1b[0m", formatted) // Bold Red for heavy phases
    } else if seconds >= 0.1 {
        format!("\x1b[1;33m{}\x1b[0m", formatted) // Bold Yellow for medium phases
    } else {
        format!("\x1b[90m{}\x1b[0m", formatted)    // Gray for fast phases
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse();

    let ic_engine = if args.clean { "[ENGINE]" } else { "⚙️ " };
    let ic_input = if args.clean { "[INPUT]" } else { "📂 " };
    let ic_output = if args.clean { "[OUTPUT]" } else { "📥 " };
    let ic_prof = if args.clean { "[PROFILE]" } else { "🛠️ " };
    let ic_canvas = if args.clean { "[CANVAS]" } else { "📐 " };

    let line_sep = color_gray("================================================================================", args.clean);
    let line_thin = color_gray("────────────────────────────────────────────────────────────────────────────────", args.clean);

    println!("{}", line_sep);
    println!(" {} {}", ic_engine, color_bold("Vector Geometry Engine - Multi-Phase Batch Queue Pipeline (Full Rust)", args.clean));
    println!("{}", line_sep);
    println!(" {} {}        : {}", ic_input, color_cyan("Input Path", args.clean), args.input.display());
    println!(" {} {} : {}", ic_output, color_green("Output Directory", args.clean), args.output.display());

    // Configure Rayon thread pool if requested
    if args.workers > 0 {
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(args.workers)
            .build_global();
    }

    // Load config profiles
    let (low_cfg, multi_cfg, config_path) = load_profiles(args.config.as_ref());
    println!(" {} {} : {}", ic_prof, color_magenta("Loaded Configuration from", args.clean), config_path.display());
    println!(" {} {} : {}", ic_canvas, color_yellow("Canvas Area Target", args.clean), "18,000,000 px to 22,000,000 px");
    println!("{}", line_sep);

    // Discover input files
    let svg_files = discover_svg_files(&args.input)?;
    if svg_files.is_empty() {
        println!(" {} No SVG files found at input path: {}", color_yellow("[WARNING]", args.clean), args.input.display());
        return Ok(());
    }

    let found_msg = format!("Found {} SVG file(s) in queue to process...", svg_files.len());
    println!("\n {}\n", color_cyan(&found_msg, args.clean));

    let mut summary = Vec::new();
    let queue_start = Instant::now();

    for (file_idx, file_path) in svg_files.iter().enumerate() {
        let file_start = Instant::now();
        let file_name = file_path.file_name().unwrap_or_default().to_string_lossy().into_owned();

        println!("{}", line_thin);
        let file_header = format!("📄 [FILE {}/{}] Processing File: {}", file_idx + 1, svg_files.len(), file_name);
        println!(" {}", color_bold(&file_header, args.clean));
        println!("{}", line_thin);

        match process_file(file_path, &args, &low_cfg, &mut multi_cfg.clone()) {
            Ok((_color_count, _prof_name, deliverables)) => {
                let duration = file_start.elapsed().as_secs_f64();
                let success_msg = format!("Finished {} in {:.2} s | Status: [OK]", file_name, duration);
                println!("  ✅ {}", color_green(&success_msg, args.clean));
                summary.push((file_name, duration, deliverables, "SUCCESS".to_string()));
            }
            Err(e) => {
                let duration = file_start.elapsed().as_secs_f64();
                let error_msg = format!("Failed to process {}: {}", file_name, e);
                println!("  ❌ {}", color_red(&error_msg, args.clean));
                summary.push((file_name, duration, "-".to_string(), format!("Error: {}", e)));
            }
        }
    }

    let total_duration = queue_start.elapsed().as_secs_f64();

    // Print summary report
    println!("\n{}", line_sep);
    println!(" 📊 {}", color_bold("BATCH QUEUE SUMMARY REPORT", args.clean));
    println!("{}", line_sep);
    println!(" {:<32} | {:<10} | {:<25} | {:<10}", "File Name", "Time (s)", "Output Deliverables", "Status");
    println!("{}", color_gray("--------------------------------------------------------------------------------", args.clean));
    for (name, time, delivs, status) in &summary {
        let trunc_name = if name.len() > 32 { format!("{}...", &name[..29]) } else { name.clone() };
        let trunc_delivs = if delivs.len() > 25 { format!("{}...", &delivs[..22]) } else { delivs.clone() };
        let status_colored = if status == "SUCCESS" {
            color_green(status, args.clean)
        } else {
            color_red(status, args.clean)
        };
        println!(" {:<32} | {:<10.2} | {:<25} | {}", trunc_name, time, trunc_delivs, status_colored);
    }
    println!("{}", color_gray("--------------------------------------------------------------------------------", args.clean));
    let time_msg = format!("Total Queue Time : {:.2} s", total_duration);
    let processed_msg = format!("Total Processed   : {} File(s)", summary.len());
    println!(" ⏱️  {}", color_cyan(&time_msg, args.clean));
    println!(" 📦 {}", color_magenta(&processed_msg, args.clean));
    println!("{}", line_sep);

    Ok(())
}

#[allow(unused_assignments)]
fn process_file(
    file_path: &Path,
    args: &CliArgs,
    low_cfg: &ProfileConfig,
    multi_cfg: &ProfileConfig,
) -> Result<(usize, String, String), Box<dyn std::error::Error>> {
    let raw_input_svg = fs::read_to_string(file_path)?;
    let mut doc = SvgParser::parse_str(&raw_input_svg)?;

    // 0. Detect and normalize background
    BackgroundProcessor::normalize_background(&mut doc);

    // Count unique colors in document (excluding background)
    let mut colors = HashSet::new();
    for p in &doc.paths {
        if p.group_id != usize::MAX {
            if let Some(ref color) = p.fill {
                colors.insert(color.clone());
            }
        }
    }
    let color_count = colors.len();

    // Select profile
    let (prof_name, prof) = if color_count <= 5 {
        ("low_color_bad_two", low_cfg)
    } else {
        ("multi_color_bad_two", multi_cfg)
    };

    let profile_msg = format!("Selected Profile: '{}' ({} Colors)", prof_name, color_count);
    println!("  🎨 {}", color_magenta(&profile_msg, args.clean));

    // Extract variables with command overrides (Default distance & tolerance to 0.0 in single mode to preserve pristine shapes)
    let is_single_only = args.single && !args.double && !args.triple && !args.pattern;
    let outset_distance = args.distance.unwrap_or(if is_single_only { 0.0 } else { prof.distance });
    let simplify_tolerance = args.tolerance.unwrap_or(if is_single_only { 0.0 } else { prof.tolerance });
    let sil_threshold = args.silhouette_threshold.unwrap_or(prof.threshold);
    let min_nodes = args.min_nodes.unwrap_or(prof.min_nodes);
    let line_art = args.line_art.unwrap_or(prof.line_art);
    let bg_color = args.bg_color.clone().unwrap_or_else(|| prof.bg_color.clone());
    let fg_color = args.fg_color.clone().unwrap_or_else(|| prof.fg_color.clone());
    let invert = args.invert.unwrap_or(prof.invert);
    let gap = args.gap.unwrap_or(prof.gap);
    let min_area = args.min_area.unwrap_or(prof.min_area_px);
    let max_area = args.max_area.unwrap_or(prof.max_area_px);

    let remove_layers = if let Some(ref list_str) = args.remove_layers {
        list_str.split(',')
            .filter_map(|s| s.trim().parse::<usize>().ok())
            .collect::<Vec<usize>>()
    } else {
        prof.remove_layers.clone()
    };

    // Phase 1: Outset & Combine Geometry in RAM
    let p1_start = Instant::now();
    let (combined_doc, _) = ColorCombiner::combine_document_badge_aware(&doc, true)?;
    let p1_sec = p1_start.elapsed().as_secs_f64();
    println!("  🔹 {}  : {}", color_cyan("Phase 1 (Outset & Combine Geometry in RAM)", args.clean), format_duration(p1_sec, args.clean));

    // Check if it's a badge layout
    let is_badge_svg = combined_doc.paths.iter().any(|p| p.badge_id.is_some());

    let mut outset_doc = combined_doc.clone();
    let mut p2_sec = 0.0;
    let mut p3_sec = 0.0;
    let mut p4_sec = 0.0;
    let mut p5_sec = 0.0;
    let mut p6_sec = 0.0;
    let mut p7_sec = 0.0;

    let mut do_single = args.single;
    let mut do_double = args.double;
    let mut do_triple = args.triple;
    let mut do_pattern = args.pattern;

    let mut silhouette_doc = outset_doc.clone();
    let mut panel3_doc: Option<VectorDocument> = None;

    if is_badge_svg {
        println!("  ⚠️  {}", color_yellow("Phase 2 (In-Memory Simplification Bypassed): 0.00 s (Preserving Pristine Fonts)", args.clean));
        println!("  ⚠️  {}", color_yellow("Phase 3 (Color Isolation & Silhouette Bypassed): 0.00 s", args.clean));
        println!("  ℹ️  {}", color_gray("SVG Badge Grid layout detected -> Forcing Single mode export ONLY", args.clean));
        do_single = true;
        do_double = false;
        do_triple = false;
        do_pattern = false;
    } else {
        // Phase 2: In-Memory Node Simplification
        let p2_start = Instant::now();
        let enable_outset = outset_distance > 0.0;
        let enable_simplify = simplify_tolerance > 0.0;
        if enable_outset || enable_simplify {
            let (res_doc, _) = GeometryOutset::outset_document(
                &combined_doc,
                enable_outset,
                outset_distance,
                enable_simplify,
                simplify_tolerance
            )?;
            outset_doc = res_doc;
        }
        p2_sec = p2_start.elapsed().as_secs_f64();
        println!("  🔹 {}  : {}", color_cyan("Phase 2 (In-Memory Node Simplification)", args.clean), format_duration(p2_sec, args.clean));

        // Phase 3: Color Isolation & Silhouette
        let p3_start = Instant::now();
        let mut original_doc = outset_doc.clone();
        let color_merge_thresh = if is_single_only { 0.0 } else { prof.color_merge_threshold };
        if color_merge_thresh > 0.0 {
            merge_similar_colors_in_doc(&mut original_doc, color_merge_thresh);
        }

        let mut sil_input_doc = original_doc.clone();
        if !remove_layers.is_empty() || min_nodes > 0 {
            remove_specific_layers_from_doc(&mut sil_input_doc, &remove_layers, min_nodes);
        }

        if prof_name == "multi_color_bad_two" {
            let mut sil_doc = sil_input_doc.clone();
            apply_multi_shade_luminance_recoloring_doc(
                &mut sil_doc,
                sil_threshold,
                "grey",
                prof.middle_panel_min_lum,
                prof.middle_panel_max_lum
            );
            silhouette_doc = sil_doc;

            let mut pan3 = sil_input_doc.clone();
            apply_multi_shade_luminance_recoloring_doc(
                &mut pan3,
                sil_threshold,
                "colored",
                prof.right_panel_min_lum,
                prof.right_panel_max_lum
            );
            panel3_doc = Some(pan3);
        } else {
            silhouette_doc = LuminanceEngine::apply_silhouette_variant_full(
                &sil_input_doc,
                sil_threshold,
                line_art,
                Some(&bg_color),
                Some(&fg_color),
                invert,
                false
            )?;
        }
        p3_sec = p3_start.elapsed().as_secs_f64();
        println!("  🔹 {}  : {}", color_cyan("Phase 3 (Color Isolation & Silhouette)", args.clean), format_duration(p3_sec, args.clean));

        let mode_specified = args.single || args.double || args.triple || args.pattern;
        if !mode_specified {
            do_single = prof.export_single;
            do_double = prof.export_trio;
            do_pattern = prof.pattern_enable;
        }
    }

    let mut export_queue = Vec::new();

    // Phase 4: Bad Board Generation
    if do_double || do_triple {
        let p4_start = Instant::now();
        let (doc_w, doc_h) = match outset_doc.view_box {
            Some(vb) => ((vb.2 - vb.0).abs(), (vb.3 - vb.1).abs()),
            None => (outset_doc.width.unwrap_or(1000.0), outset_doc.height.unwrap_or(1000.0)),
        };

        let effective_gap = if doc_h > 0.0 && doc_w > doc_h {
            let aspect = doc_w / doc_h;
            (gap / aspect).max(5.0)
        } else {
            gap
        };

        let board_suffix = if (do_triple || panel3_doc.is_some()) && panel3_doc.is_some() {
            let duo_1_2 = DuoBoardGenerator::create_duo_board(&outset_doc, &silhouette_doc, effective_gap)?;
            let triple_board = DuoBoardGenerator::create_duo_board(&duo_1_2, panel3_doc.as_ref().unwrap(), effective_gap)?;
            let (scaled_board, _) = CanvasScaler::scale_document_to_pixel_area(&triple_board, min_area, max_area)?;
            export_queue.push((scaled_board, triple_board, "bad_three".to_string()));
            "bad_three"
        } else {
            let duo_board = DuoBoardGenerator::create_duo_board(&outset_doc, &silhouette_doc, effective_gap)?;
            let (scaled_board, _) = CanvasScaler::scale_document_to_pixel_area(&duo_board, min_area, max_area)?;
            export_queue.push((scaled_board, duo_board, "bad_two".to_string()));
            "bad_two"
        };
        p4_sec = p4_start.elapsed().as_secs_f64();
        let board_msg = format!("Phase 4 (Bad Board Generation: {})", board_suffix);
        println!("  🔹 {}  : {}", color_cyan(&board_msg, args.clean), format_duration(p4_sec, args.clean));
    }

    // Phase 5: Seamless Pattern Generation
    if do_pattern && (prof.pattern_enable || args.pattern) {
        let p5_start = Instant::now();
        if let Ok(report) = PatternDetector::analyze_document(&outset_doc) {
            let allowed_shapes = vec!["circular".to_string(), "square".to_string()];
            let style_used = args.style.as_ref().map(|s| s.as_str()).unwrap_or(&prof.pattern_style);
            
            let is_suitable = !prof.pattern_enable || args.pattern || (
                report.is_suitable && (allowed_shapes.contains(&report.detected_shape) || report.is_circular_shape || report.is_square_shape)
            );

            if is_suitable {
                let grid_x = prof.pattern_grid_x;
                let grid_y = prof.pattern_grid_y;

                let pattern_doc = PatternGenerator::generate_pattern_full_config(
                    &outset_doc,
                    grid_x,
                    grid_y,
                    style_used,
                    prof.pattern_slant_offset_ratio,
                    true,
                    true
                )?;

                let (scaled_pattern, _) = CanvasScaler::scale_document_to_pixel_area(&pattern_doc, min_area, max_area)?;
                export_queue.push((scaled_pattern, pattern_doc, "pattern".to_string()));
                p5_sec = p5_start.elapsed().as_secs_f64();
                let pat_msg = format!("Phase 5 (Seamless Pattern: style='{}')", style_used);
                println!("  🔹 {}  : {}", color_cyan(&pat_msg, args.clean), format_duration(p5_sec, args.clean));
            } else {
                p5_sec = p5_start.elapsed().as_secs_f64();
                let pat_skip_msg = format!("Phase 5 (Pattern Skipped: shape='{}')", report.detected_shape);
                println!("  ⚠️  {}  : {}", color_yellow(&pat_skip_msg, args.clean), format_duration(p5_sec, args.clean));
            }
        }
    }

    // Phase 6: Canvas Auto-Scale
    let p6_start = Instant::now();
    if do_single {
        let svg_for_single = outset_doc.clone();
        let (mut scaled_single, _) = CanvasScaler::scale_document_to_pixel_area(&svg_for_single, min_area, max_area)?;
        if !is_badge_svg && !args.single {
            center_document_artwork(&mut scaled_single, 20.0);
        }
        export_queue.push((scaled_single, svg_for_single, "single".to_string()));
        p6_sec = p6_start.elapsed().as_secs_f64();
        println!("  🔹 {}  : {}", color_cyan("Phase 6 (Canvas Auto-Scale 1x Target)", args.clean), format_duration(p6_sec, args.clean));
    }

    // Phase 7: Parallel Disk Export & Render
    let p7_start = Instant::now();
    let stem = file_path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let output_path_buf = args.output.clone();

    let export_results: Vec<Result<String, String>> = export_queue
        .into_par_iter()
        .map(|(scaled_doc, _raw_doc, suffix)| {
            let final_svg = output_path_buf.join(format!("{}_{}.svg", stem, suffix));
            let final_eps = output_path_buf.join(format!("{}_{}.eps", stem, suffix));
            let final_jpg = output_path_buf.join(format!("{}_{}.jpg", stem, suffix));

            let scaled_svg_str = scaled_doc.to_svg_string();

            if let Some(parent) = final_svg.parent() {
                let _ = fs::create_dir_all(parent);
            }

            // Write SVG
            if let Err(e) = fs::write(&final_svg, &scaled_svg_str) {
                return Err(format!("Failed to write SVG: {}", e));
            }

            // Generate EPS and Render JPG concurrently
            let (eps_res, jpg_res) = rayon::join(
                || -> Result<(), String> {
                    let eps_content = EpsWriter::write_document(&scaled_doc, &stem)
                        .map_err(|e| format!("EPS Write failed: {}", e))?;
                    fs::write(&final_eps, eps_content)
                        .map_err(|e| format!("Failed to write EPS file: {}", e))?;
                    Ok(())
                },
                || -> Result<(), String> {
                    let jpg_bytes = JpgPreviewGenerator::render_svg_str_jpg(&scaled_svg_str, 0, 95)
                        .map_err(|e| format!("JPG Render failed: {}", e))?;
                    fs::write(&final_jpg, jpg_bytes)
                        .map_err(|e| format!("Failed to write JPG file: {}", e))?;
                    Ok(())
                }
            );

            if let Err(e) = eps_res {
                return Err(e);
            }
            if let Err(e) = jpg_res {
                return Err(e);
            }

            Ok(suffix)
        })
        .collect();

    let mut suffix_list = Vec::new();
    for res in export_results {
        match res {
            Ok(suffix) => suffix_list.push(suffix),
            Err(e) => return Err(e.into()),
        }
    }
    p7_sec = p7_start.elapsed().as_secs_f64();
    println!("  🔹 {}  : {}", color_cyan("Phase 7 (Disk Export EPS & JPG Render)", args.clean), format_duration(p7_sec, args.clean));

    let deliverables = suffix_list.join(", ");
    Ok((color_count, prof_name.to_string(), deliverables))
}

fn discover_svg_files(input_path: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    if input_path.is_file() {
        if is_svg_file(input_path) {
            files.push(input_path.to_path_buf());
        }
    } else if input_path.is_dir() {
        for entry in fs::read_dir(input_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && is_svg_file(&path) {
                files.push(path);
            }
        }
    }
    Ok(files)
}

fn is_svg_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.eq_ignore_ascii_case("svg"))
        .unwrap_or(false)
}

fn apply_json_profile(val: &serde_json::Value, prof: &mut ProfileConfig) {
    if val.is_null() {
        return;
    }
    if let Some(v) = val.get("threshold").and_then(|v| v.as_f64()) { prof.threshold = v; }
    if let Some(v) = val.get("tolerance").and_then(|v| v.as_f64()) { prof.tolerance = v; }
    if let Some(v) = val.get("distance").and_then(|v| v.as_f64()) { prof.distance = v; }
    if let Some(v) = val.get("color_merge_threshold").and_then(|v| v.as_f64()) { prof.color_merge_threshold = v; }
    if let Some(v) = val.get("line_art").and_then(|v| v.as_bool()) { prof.line_art = v; }
    if let Some(v) = val.get("export_single").and_then(|v| v.as_bool()) { prof.export_single = v; }
    if let Some(v) = val.get("export_trio").and_then(|v| v.as_bool()) { prof.export_trio = v; }
    if let Some(v) = val.get("bg_color").and_then(|v| v.as_str()) { prof.bg_color = v.to_string(); }
    if let Some(v) = val.get("fg_color").and_then(|v| v.as_str()) { prof.fg_color = v.to_string(); }
    if let Some(v) = val.get("invert").and_then(|v| v.as_bool()) { prof.invert = v; }
    if let Some(v) = val.get("gap").and_then(|v| v.as_f64()) { prof.gap = v; }
    if let Some(v) = val.get("min_area_px").and_then(|v| v.as_f64()) { prof.min_area_px = v; }
    if let Some(v) = val.get("max_area_px").and_then(|v| v.as_f64()) { prof.max_area_px = v; }

    if let Some(pat) = val.get("pattern_config") {
        if let Some(v) = pat.get("enable").and_then(|v| v.as_bool()) { prof.pattern_enable = v; }
        if let Some(v) = pat.get("style").and_then(|v| v.as_str()) { prof.pattern_style = v.to_string(); }
        if let Some(v) = pat.get("slant_offset_ratio").and_then(|v| v.as_f64()) { prof.pattern_slant_offset_ratio = v; }
        if let Some(v) = pat.get("grid_x").and_then(|v| v.as_u64()) { prof.pattern_grid_x = v as u32; }
        if let Some(v) = pat.get("grid_y").and_then(|v| v.as_u64()) { prof.pattern_grid_y = v as u32; }
        if let Some(v) = pat.get("seamless_combine").and_then(|v| v.as_bool()) { prof.pattern_seamless_combine = v; }
    }

    if let Some(mid) = val.get("middle_panel") {
        if let Some(v) = mid.get("min_luminance").and_then(|v| v.as_f64()) { prof.middle_panel_min_lum = v; }
        if let Some(v) = mid.get("max_luminance").and_then(|v| v.as_f64()) { prof.middle_panel_max_lum = v; }
    }

    if let Some(right) = val.get("right_panel") {
        if let Some(v) = right.get("min_luminance").and_then(|v| v.as_f64()) { prof.right_panel_min_lum = v; }
        if let Some(v) = right.get("max_luminance").and_then(|v| v.as_f64()) { prof.right_panel_max_lum = v; }
    }
}

fn load_profiles(config_path: Option<&PathBuf>) -> (ProfileConfig, ProfileConfig, PathBuf) {
    let json_default = PathBuf::from("config_silhouette.json");
    let yaml_default = PathBuf::from("config/config.yaml");

    let default_path = if json_default.exists() {
        json_default
    } else {
        yaml_default
    };

    let path_to_load = config_path.unwrap_or(&default_path);

    let mut low = ProfileConfig {
        max_colors: Some(5),
        min_colors: None,
        threshold: 0.50,
        tolerance: 0.20,
        distance: 0.5,
        color_merge_threshold: 0.15,
        remove_layers: Vec::new(),
        min_nodes: 0,
        line_art: false,
        bg_color: "#FFFFFF".to_string(),
        fg_color: "#000000".to_string(),
        invert: false,
        gap: 50.0,
        min_area_px: 19000000.0,
        max_area_px: 22000000.0,
        export_single: true,
        export_trio: true,
        pattern_enable: true,
        pattern_style: "brick_wall".to_string(),
        pattern_slant_offset_ratio: -0.5,
        pattern_grid_x: 3,
        pattern_grid_y: 3,
        pattern_seamless_combine: true,
        middle_panel_min_lum: 0.20,
        middle_panel_max_lum: 0.62,
        right_panel_min_lum: 0.20,
        right_panel_max_lum: 0.62,
    };
    
    let mut multi = ProfileConfig {
        max_colors: None,
        min_colors: Some(6),
        threshold: 0.95,
        tolerance: 0.30,
        distance: 0.75,
        color_merge_threshold: 0.15,
        remove_layers: Vec::new(),
        min_nodes: 0,
        line_art: false,
        bg_color: "#FFFFFF".to_string(),
        fg_color: "#000000".to_string(),
        invert: false,
        gap: 50.0,
        min_area_px: 18000000.0,
        max_area_px: 22000000.0,
        export_single: true,
        export_trio: true,
        pattern_enable: false,
        pattern_style: "slanted_left".to_string(),
        pattern_slant_offset_ratio: -0.5,
        pattern_grid_x: 3,
        pattern_grid_y: 3,
        pattern_seamless_combine: true,
        middle_panel_min_lum: 0.20,
        middle_panel_max_lum: 0.62,
        right_panel_min_lum: 0.20,
        right_panel_max_lum: 0.62,
    };

    if let Ok(content) = fs::read_to_string(path_to_load) {
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&content) {
            apply_json_profile(&json_val["low_color_bad_two"], &mut low);
            apply_json_profile(&json_val["multi_color_bad_two"], &mut multi);
        } else {
            let mut current_profile = "";
            let mut in_pattern_config = false;
            let mut in_middle_panel = false;
            let mut in_right_panel = false;

            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') || trimmed.is_empty() {
                    continue;
                }

                let indent = line.len() - line.trim_start().len();
                if indent <= 4 {
                    in_pattern_config = false;
                    in_middle_panel = false;
                    in_right_panel = false;
                }

                if trimmed.starts_with("low_color_bad_two:") {
                    current_profile = "low";
                    continue;
                } else if trimmed.starts_with("multi_color_bad_two:") {
                    current_profile = "multi";
                    continue;
                } else if trimmed.starts_with("pattern_config:") {
                    in_pattern_config = true;
                    continue;
                } else if trimmed.starts_with("middle_panel:") {
                    in_middle_panel = true;
                    continue;
                } else if trimmed.starts_with("right_panel:") {
                    in_right_panel = true;
                    continue;
                }

                let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim();
                    let val = parts[1].trim().trim_matches('"').trim_matches('\'');
                    let prof = if current_profile == "low" { &mut low } else { &mut multi };

                    if in_pattern_config {
                        match key {
                            "enable" => prof.pattern_enable = val.parse::<bool>().unwrap_or(prof.pattern_enable),
                            "style" => prof.pattern_style = val.to_string(),
                            "slant_offset_ratio" => prof.pattern_slant_offset_ratio = val.parse::<f64>().unwrap_or(prof.pattern_slant_offset_ratio),
                            "grid_x" => prof.pattern_grid_x = val.parse::<u32>().unwrap_or(prof.pattern_grid_x),
                            "grid_y" => prof.pattern_grid_y = val.parse::<u32>().unwrap_or(prof.pattern_grid_y),
                            "seamless_combine" => prof.pattern_seamless_combine = val.parse::<bool>().unwrap_or(prof.pattern_seamless_combine),
                            _ => {}
                        }
                    } else if in_middle_panel {
                        match key {
                            "min_luminance" => prof.middle_panel_min_lum = val.parse::<f64>().unwrap_or(prof.middle_panel_min_lum),
                            "max_luminance" => prof.middle_panel_max_lum = val.parse::<f64>().unwrap_or(prof.middle_panel_max_lum),
                            _ => {}
                        }
                    } else if in_right_panel {
                        match key {
                            "min_luminance" => prof.right_panel_min_lum = val.parse::<f64>().unwrap_or(prof.right_panel_min_lum),
                            "max_luminance" => prof.right_panel_max_lum = val.parse::<f64>().unwrap_or(prof.right_panel_max_lum),
                            _ => {}
                        }
                    } else {
                        match key {
                            "max_colors" => prof.max_colors = val.parse::<usize>().ok(),
                            "min_colors" => prof.min_colors = val.parse::<usize>().ok(),
                            "threshold" => prof.threshold = val.parse::<f64>().unwrap_or(prof.threshold),
                            "tolerance" => prof.tolerance = val.parse::<f64>().unwrap_or(prof.tolerance),
                            "distance" => prof.distance = val.parse::<f64>().unwrap_or(prof.distance),
                            "color_merge_threshold" => prof.color_merge_threshold = val.parse::<f64>().unwrap_or(prof.color_merge_threshold),
                            "min_nodes" => prof.min_nodes = val.parse::<usize>().unwrap_or(prof.min_nodes),
                            "line_art" => prof.line_art = val.parse::<bool>().unwrap_or(prof.line_art),
                            "export_single" => prof.export_single = val.parse::<bool>().unwrap_or(prof.export_single),
                            "export_trio" => prof.export_trio = val.parse::<bool>().unwrap_or(prof.export_trio),
                            "bg_color" => prof.bg_color = val.to_string(),
                            "fg_color" => prof.fg_color = val.to_string(),
                            "invert" => prof.invert = val.parse::<bool>().unwrap_or(prof.invert),
                            "gap" => prof.gap = val.parse::<f64>().unwrap_or(prof.gap),
                            "min_area_px" => prof.min_area_px = val.parse::<f64>().unwrap_or(prof.min_area_px),
                            "max_area_px" => prof.max_area_px = val.parse::<f64>().unwrap_or(prof.max_area_px),
                            "remove_layers" => {
                                let clean_val = val.trim_matches('[').trim_matches(']');
                                if !clean_val.is_empty() {
                                    prof.remove_layers = clean_val.split(',')
                                        .filter_map(|x| x.trim().parse::<usize>().ok())
                                        .collect();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    (low, multi, path_to_load.to_path_buf())
}

fn hex_to_rgb(hex_str: &str) -> (f64, f64, f64) {
    let clean = hex_str.trim().trim_start_matches('#');
    if clean.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&clean[0..2], 16),
            u8::from_str_radix(&clean[2..4], 16),
            u8::from_str_radix(&clean[4..6], 16),
        ) {
            return (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
        }
    }
    (1.0, 1.0, 1.0)
}

fn rgb_to_hex(r: f64, g: f64, b: f64) -> String {
    let r_u8 = (r * 255.0).round().clamp(0.0, 255.0) as u8;
    let g_u8 = (g * 255.0).round().clamp(0.0, 255.0) as u8;
    let b_u8 = (b * 255.0).round().clamp(0.0, 255.0) as u8;
    format!("#{:02X}{:02X}{:02X}", r_u8, g_u8, b_u8)
}

fn hex_to_hsl(hex_str: &str) -> (f64, f64, f64) {
    let (r, g, b) = hex_to_rgb(hex_str);
    let max_c = r.max(g).max(b);
    let min_c = r.min(g).min(b);
    let l = (max_c + min_c) / 2.0;

    let (h, s) = if max_c == min_c {
        (0.0, 0.0)
    } else {
        let d = max_c - min_c;
        let s = if l > 0.5 { d / (2.0 - max_c - min_c) } else { d / (max_c + min_c) };
        let mut h = if max_c == r {
            (g - b) / d + (if g < b { 6.0 } else { 0.0 })
        } else if max_c == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        h *= 60.0;
        (h, s)
    };
    (h, s, l)
}

fn hsl_to_hex(h: f64, s: f64, l: f64) -> String {
    let h = h % 360.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r_p, g_p, b_p) = if h >= 0.0 && h < 60.0 {
        (c, x, 0.0)
    } else if h >= 60.0 && h < 120.0 {
        (x, c, 0.0)
    } else if h >= 120.0 && h < 180.0 {
        (0.0, c, x)
    } else if h >= 180.0 && h < 240.0 {
        (0.0, x, c)
    } else if h >= 240.0 && h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    rgb_to_hex(r_p + m, g_p + m, b_p + m)
}

fn hex_luminance(hex_str: &str) -> f64 {
    let (r, g, b) = hex_to_rgb(hex_str);
    0.299 * r + 0.587 * g + 0.114 * b
}

fn are_colors_similar_hsl(
    hex1: &str,
    hex2: &str,
    max_hue_diff: f64,
    max_l_diff: f64,
    max_s_diff: f64,
) -> bool {
    let (h1, s1, l1) = hex_to_hsl(hex1);
    let (h2, s2, l2) = hex_to_hsl(hex2);

    let l_diff = (l1 - l2).abs();
    if l_diff > max_l_diff {
        return false;
    }

    if (l1 > 0.92 && l2 > 0.92) || (l1 < 0.08 && l2 < 0.08) {
        return l_diff <= max_l_diff;
    }

    let s_diff = (s1 - s2).abs();
    if s_diff > max_s_diff {
        return false;
    }

    let raw_h_diff = (h1 - h2).abs();
    let hue_diff = raw_h_diff.min(360.0 - raw_h_diff);
    hue_diff <= max_hue_diff
}

fn merge_similar_colors_in_doc(doc: &mut VectorDocument, threshold: f64) {
    if threshold <= 0.0 {
        return;
    }

    let mut color_areas: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for p in &doc.paths {
        if let Some(ref fill) = p.fill {
            if fill != "none" {
                let bbox = p.bounding_box();
                let w = (bbox.2 - bbox.0).abs();
                let h = (bbox.3 - bbox.1).abs();
                let area = (w * h).max(1.0);
                *color_areas.entry(fill.clone()).or_insert(0.0) += area;
            }
        }
    }

    if color_areas.is_empty() {
        return;
    }

    let mut sorted_colors: Vec<String> = color_areas.keys().cloned().collect();
    sorted_colors.sort_by(|a, b| {
        color_areas[b]
            .partial_cmp(&color_areas[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let max_h = 5.0 * (threshold / 0.15).clamp(0.2, 1.0);
    let max_l = 0.05 * (threshold / 0.15).clamp(0.2, 1.0);
    let max_s = 0.10 * (threshold / 0.15).clamp(0.2, 1.0);

    let mut clusters: Vec<String> = Vec::new();
    let mut color_map = std::collections::HashMap::new();

    for c in sorted_colors {
        let mut assigned = false;
        for head in &clusters {
            if are_colors_similar_hsl(&c, head, max_h, max_l, max_s) {
                color_map.insert(c.clone(), head.clone());
                assigned = true;
                break;
            }
        }
        if !assigned {
            clusters.push(c.clone());
            color_map.insert(c.clone(), c.clone());
        }
    }

    for p in &mut doc.paths {
        if let Some(ref fill) = p.fill {
            if let Some(merged) = color_map.get(fill) {
                p.fill = Some(merged.clone());
            }
        }
    }
}

fn remove_specific_layers_from_doc(doc: &mut VectorDocument, remove_indexes: &[usize], min_nodes: usize) {
    if remove_indexes.is_empty() && min_nodes == 0 {
        return;
    }

    let mut color_lums = std::collections::HashMap::new();
    for p in &doc.paths {
        if let Some(ref fill) = p.fill {
            if fill != "none" && !color_lums.contains_key(fill) {
                color_lums.insert(fill.clone(), hex_luminance(fill));
            }
        }
    }

    let mut sorted_fills: Vec<String> = color_lums.keys().cloned().collect();
    sorted_fills.sort_by(|a, b| color_lums[a].partial_cmp(&color_lums[b]).unwrap());

    let mut removed_fills = HashSet::new();
    for &idx in remove_indexes {
        if idx < sorted_fills.len() {
            removed_fills.insert(sorted_fills[idx].clone());
        }
    }

    let (canvas_w, canvas_h) = match doc.view_box {
        Some(vb) => ((vb.2 - vb.0).abs(), (vb.3 - vb.1).abs()),
        None => (doc.width.unwrap_or(1000.0), doc.height.unwrap_or(1000.0)),
    };

    let mut filtered_paths = Vec::new();
    let mut detected_bg_color = None;

    for p in &doc.paths {
        let fill = p.fill.as_deref().unwrap_or("none");
        let cmd_count = p.commands.len();
        let lum = p.fill.as_ref().and_then(|f| color_lums.get(f)).cloned().unwrap_or(1.0);

        if removed_fills.contains(fill) {
            let bbox = p.bounding_box();
            let w = (bbox.2 - bbox.0).abs();
            let h = (bbox.3 - bbox.1).abs();
            if w >= 0.95 * canvas_w && h >= 0.95 * canvas_h {
                if detected_bg_color.is_none() {
                    detected_bg_color = Some(fill.to_string());
                }
                continue;
            }
        }

        if min_nodes > 0 && cmd_count < min_nodes && lum >= 0.15 {
            continue;
        }

        filtered_paths.push(p.clone());
    }

    if let Some(bg_color) = detected_bg_color {
        let mut bg_path = VectorPath::new(Some("bg_canvas_restored".to_string()));
        bg_path.fill = Some(bg_color);
        bg_path.stroke = Some("none".to_string());
        bg_path.stroke_width = Some(1.0);
        bg_path.group_id = usize::MAX;
        bg_path.add_command(PathCommand::MoveTo(Point2D::new(0.0, 0.0)));
        bg_path.add_command(PathCommand::LineTo(Point2D::new(canvas_w, 0.0)));
        bg_path.add_command(PathCommand::LineTo(Point2D::new(canvas_w, canvas_h)));
        bg_path.add_command(PathCommand::LineTo(Point2D::new(0.0, canvas_h)));
        bg_path.add_command(PathCommand::Close);

        filtered_paths.insert(0, bg_path);
    }

    doc.paths = filtered_paths;
}

fn apply_multi_shade_luminance_recoloring_doc(
    doc: &mut VectorDocument,
    threshold: f64,
    mode: &str,
    min_luminance: f64,
    max_luminance: f64,
) {
    if doc.paths.is_empty() {
        return;
    }

    let mut color_lums = std::collections::HashMap::new();
    for p in &doc.paths {
        if let Some(ref fill) = p.fill {
            if fill != "none" && !color_lums.contains_key(fill) {
                color_lums.insert(fill.clone(), hex_luminance(fill));
            }
        }
    }

    if color_lums.is_empty() {
        return;
    }

    let cutoff = if threshold > 0.0 && threshold <= 1.0 { threshold } else { 0.95 };

    let mut fg_colors: Vec<String> = color_lums
        .keys()
        .filter(|c| color_lums[*c] < cutoff)
        .cloned()
        .collect();
    fg_colors.sort_by(|a, b| color_lums[a].partial_cmp(&color_lums[b]).unwrap());

    let (dom_h, dom_s) = if mode == "colored" {
        find_dominant_color_doc(doc, cutoff)
    } else {
        (0.0, 0.0)
    };

    let mut color_map = std::collections::HashMap::new();
    for (hex_color, &lum) in &color_lums {
        if lum >= cutoff {
            color_map.insert(hex_color.clone(), "#FFFFFF".to_string());
        }
    }

    let total_fg = fg_colors.len();
    for (idx, hex_color) in fg_colors.iter().enumerate() {
        let target_lum = if total_fg <= 1 {
            (min_luminance + max_luminance) / 2.0
        } else {
            let t = idx as f64 / (total_fg - 1) as f64;
            min_luminance + t * (max_luminance - min_luminance)
        };

        if mode == "grey" {
            let v = (target_lum * 255.0).round().clamp(0.0, 255.0) as u8;
            color_map.insert(hex_color.clone(), format!("#{:02X}{:02X}{:02X}", v, v, v));
        } else {
            color_map.insert(hex_color.clone(), hsl_to_hex(dom_h, dom_s, target_lum));
        }
    }

    for p in &mut doc.paths {
        if let Some(ref fill) = p.fill {
            if let Some(new_fill) = color_map.get(fill) {
                p.fill = Some(new_fill.clone());
            }
        }
    }
}

fn find_dominant_color_doc(doc: &VectorDocument, cutoff: f64) -> (f64, f64) {
    let mut colored_counts = std::collections::HashMap::new();
    let mut all_fg_counts = std::collections::HashMap::new();

    for p in &doc.paths {
        if let Some(ref fill) = p.fill {
            if fill != "none" {
                let lum = hex_luminance(fill);
                if lum < cutoff {
                    let weight = p.commands.len() + 1;
                    *all_fg_counts.entry(fill.clone()).or_insert(0) += weight;

                    let (_h, s, l) = hex_to_hsl(fill);
                    if s >= 0.15 && l >= 0.12 {
                        *colored_counts.entry(fill.clone()).or_insert(0) += weight;
                    }
                }
            }
        }
    }

    if !colored_counts.is_empty() {
        let dom_hex = colored_counts
            .keys()
            .max_by_key(|c| colored_counts[*c])
            .unwrap();
        let (h, s, _) = hex_to_hsl(dom_hex);
        return (h, s.max(0.35));
    }

    if !all_fg_counts.is_empty() {
        let dom_hex = all_fg_counts
            .keys()
            .max_by_key(|c| all_fg_counts[*c])
            .unwrap();
        let (h, s, _) = hex_to_hsl(dom_hex);
        if s < 0.15 {
            return (130.0, 0.55);
        }
        return (h, s.max(0.35));
    }

    (130.0, 0.55)
}

fn center_document_artwork(doc: &mut VectorDocument, padding: f64) {
    let w = doc.width.unwrap_or(1000.0);
    let h = doc.height.unwrap_or(1000.0);
    let (canvas_w, canvas_h) = match doc.view_box {
        Some(vb) => ((vb.2 - vb.0).abs().max(w), (vb.3 - vb.1).abs().max(h)),
        None => (w, h),
    };

    if doc.paths.is_empty() {
        return;
    }

    let bg_idx_opt = BackgroundProcessor::find_background(doc, canvas_w, canvas_h).map(|(idx, _)| idx);

    let mut bg_indices = Vec::new();
    let mut fg_indices = Vec::new();

    for (idx, p) in doc.paths.iter().enumerate() {
        let pid = p.id.as_deref().unwrap_or("").to_lowercase();
        let is_bg_shape = p.group_id == usize::MAX
            || pid.contains("background")
            || pid.contains("canvas_bg")
            || Some(idx) == bg_idx_opt;

        if is_bg_shape {
            bg_indices.push(idx);
        } else {
            fg_indices.push(idx);
        }
    }

    if fg_indices.is_empty() {
        return;
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for &idx in &fg_indices {
        let bbox = doc.paths[idx].bounding_box();
        if bbox.0 < min_x { min_x = bbox.0; }
        if bbox.1 < min_y { min_y = bbox.1; }
        if bbox.2 > max_x { max_x = bbox.2; }
        if bbox.3 > max_y { max_y = bbox.3; }
    }

    if min_x.is_infinite() || max_x <= min_x || max_y <= min_y {
        return;
    }

    let art_w = max_x - min_x;
    let art_h = max_y - min_y;

    let final_w = canvas_w.max(art_w + 2.0 * padding);
    let final_h = canvas_h.max(art_h + 2.0 * padding);

    let dx = ((final_w - art_w) / 2.0) - min_x;
    let dy = ((final_h - art_h) / 2.0) - min_y;

    for &idx in &fg_indices {
        let path = &mut doc.paths[idx];
        for cmd in &mut path.commands {
            match cmd {
                PathCommand::MoveTo(ref mut pt) => {
                    pt.x += dx;
                    pt.y += dy;
                }
                PathCommand::LineTo(ref mut pt) => {
                    pt.x += dx;
                    pt.y += dy;
                }
                PathCommand::QuadTo { ref mut control, ref mut end } => {
                    control.x += dx;
                    control.y += dy;
                    end.x += dx;
                    end.y += dy;
                }
                PathCommand::CubicTo { ref mut control1, ref mut control2, ref mut end } => {
                    control1.x += dx;
                    control1.y += dy;
                    control2.x += dx;
                    control2.y += dy;
                    end.x += dx;
                    end.y += dy;
                }
                _ => {}
            }
        }
    }

    for &idx in &bg_indices {
        let bg_path = &mut doc.paths[idx];
        bg_path.commands.clear();
        bg_path.add_command(PathCommand::MoveTo(Point2D::new(0.0, 0.0)));
        bg_path.add_command(PathCommand::LineTo(Point2D::new(final_w, 0.0)));
        bg_path.add_command(PathCommand::LineTo(Point2D::new(final_w, final_h)));
        bg_path.add_command(PathCommand::LineTo(Point2D::new(0.0, final_h)));
        bg_path.add_command(PathCommand::Close);
    }

    doc.width = Some(final_w);
    doc.height = Some(final_h);
    doc.view_box = Some((0.0, 0.0, final_w, final_h));
}
