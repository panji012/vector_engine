use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn, Level};
use tracing_subscriber::FmtSubscriber;
use vector_algo::{BackgroundProcessor, ColorCombiner, GeometryOutset};
use vector_io::SvgParser;

/// Command line arguments struct for Vector Geometry Engine CLI
#[derive(Parser, Debug)]
#[command(
    name = "vector-cli",
    author = "Vector Engine Contributors",
    version = env!("CARGO_PKG_VERSION"),
    about = "High-performance modular Vector Geometry Engine CLI",
    long_about = "Batch processes SVG files, groups geometry by color, executes compound path generation, and applies outward geometry outset."
)]
struct CliArgs {
    /// Input directory or single SVG file path
    #[arg(short, long)]
    input: PathBuf,

    /// Output directory to save processed SVG files
    #[arg(short, long)]
    output: PathBuf,

    /// Optional configuration file path (YAML)
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Worker thread pool size (0 = auto-detect CPU cores)
    #[arg(short, long, default_value_t = 0)]
    threads: usize,

    /// Outset distance in pixels (default 0.5 px)
    #[arg(short = 'd', long, default_value_t = 0.5)]
    outset_distance: f64,

    /// Disable outset calculation (forces distance to 0.0, performing Boolean Union only)
    #[arg(long, default_value_t = false)]
    no_outset: bool,

    /// Disable RDP node simplification
    #[arg(long, default_value_t = false)]
    no_simplify: bool,

    /// Node simplification tolerance (epsilon) in pixels (default 0.2 px)
    #[arg(short = 's', long, default_value_t = 0.2)]
    simplify_tolerance: f64,

    /// Enable verbose debug log output
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse();

    // Initialize tracing logging subscriber
    let max_level = if args.verbose { Level::DEBUG } else { Level::INFO };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(max_level)
        .with_target(false)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    info!("Initializing Vector Geometry Engine CLI (v{})", env!("CARGO_PKG_VERSION"));

    // Configure Rayon thread pool if requested
    if args.threads > 0 {
        info!("Configuring Rayon worker threadpool with {} threads", args.threads);
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(args.threads)
            .build_global();
    }

    if let Some(ref cfg_path) = args.config {
        info!("Loading configuration file from: {}", cfg_path.display());
    }

    // Ensure output directory exists
    fs::create_dir_all(&args.output)?;

    let svg_files = discover_svg_files(&args.input)?;
    if svg_files.is_empty() {
        warn!("No SVG files found at input path: {}", args.input.display());
        return Ok(());
    }

    info!("Processing {} SVG file(s)...", svg_files.len());

    let mut results_table = Vec::new();
    let start_time = std::time::Instant::now();

    for file_path in &svg_files {
        let file_start = std::time::Instant::now();
        match process_single_svg(file_path, &args.output, !args.no_outset, args.outset_distance, !args.no_simplify, args.simplify_tolerance) {
            Ok((file_name, orig_paths, combined_paths, _outset_dist)) => {
                let duration = file_start.elapsed().as_secs_f64();
                results_table.push((file_name, orig_paths, combined_paths, duration, "Success".to_string()));
            }
            Err(e) => {
                let duration = file_start.elapsed().as_secs_f64();
                let file_name = file_path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                results_table.push((file_name, 0, 0, duration, format!("Error: {}", e)));
                warn!("Failed to process {}: {}", file_path.display(), e);
            }
        }
    }

    let total_duration = start_time.elapsed().as_secs_f64();

    // Print the summary table
    println!("\n=== BATCH PROCESSING SUMMARY ===");
    println!("| {:<30} | {:<14} | {:<14} | {:<10} | {:<20} |", "File Name", "Original Paths", "Combined Paths", "Time (s)", "Status");
    println!("|{:-<32}|{:-<16}|{:-<16}|{:-<12}|{:-<22}|", "", "", "", "", "");
    
    for (name, orig, comb, time, status) in results_table {
        // truncate filename if too long
        let short_name = if name.len() > 30 { format!("{}...", &name[..27]) } else { name };
        println!("| {:<30} | {:<14} | {:<14} | {:<10.3} | {:<20} |", short_name, orig, comb, time, status);
    }
    println!("================================\nTotal Time: {:.3}s\n", total_duration);

    info!("Batch processing completed successfully. Outputs saved to {}", args.output.display());
    Ok(())
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

fn process_single_svg(
    input_file: &Path,
    output_dir: &Path,
    enable_outset: bool,
    outset_dist: f64,
    enable_simplify: bool,
    simplify_tolerance: f64,
) -> Result<(String, usize, usize, f64), Box<dyn std::error::Error>> {
    let file_name = input_file.file_name().ok_or("Invalid filename")?.to_string_lossy().into_owned();
    debug!("Reading SVG: {}", input_file.display());

    let content = fs::read_to_string(input_file)?;
    let mut doc = SvgParser::parse_str(&content)?;

    // 0. Detect and normalize background
    if vector_algo::BackgroundProcessor::normalize_background(&mut doc) {
        debug!("Background detected and normalized for {}", input_file.display());
    }

    // 1. Execute ColorCombiner
    let (combined_doc, combine_report) = ColorCombiner::combine_document(&doc)?;

    // 2. Execute Outset on compound paths
    let (outset_doc, outset_report) = GeometryOutset::outset_document(&combined_doc, enable_outset, outset_dist, enable_simplify, simplify_tolerance)?;

    info!(
        "Processed {} | Outset: {} px | Nodes Reduced: -",
        file_name, outset_report.outset_distance
    );

    // Save processed SVG with outset geometry
    let output_file = output_dir.join(&file_name);
    let output_svg_content = outset_doc.to_svg_string();
    fs::write(&output_file, output_svg_content)?;
    debug!("Saved processed SVG -> {}", output_file.display());

    Ok((file_name, combine_report.original_path_count, combine_report.combined_path_count, outset_dist))
}
