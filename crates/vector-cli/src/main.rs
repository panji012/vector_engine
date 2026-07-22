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

    for file_path in &svg_files {
        process_single_svg(file_path, &args.output, args.outset_distance)?;
    }

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
    outset_dist: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let file_name = input_file.file_name().ok_or("Invalid filename")?;
    debug!("Reading SVG: {}", input_file.display());

    let content = fs::read_to_string(input_file)?;
    let mut doc = SvgParser::parse_str(&content)?;

    // 0. Detect and normalize background
    if vector_algo::BackgroundProcessor::normalize_background(&mut doc) {
        debug!("Background detected and normalized for {}", input_file.display());
    }

    // 1. Execute ColorCombiner
    let (combined_doc, combine_report) = ColorCombiner::combine_document(&doc)?;

    println!("Processing File: {}", file_name.to_str().unwrap_or(""));
    println!("Original Paths: {}", combine_report.original_path_count);
    println!("Color Groups: {}", combine_report.number_of_color_groups);
    println!("Combined Paths: {}", combine_report.combined_path_count);

    // 2. Execute Outset on compound paths
    let (outset_doc, outset_report) = GeometryOutset::outset_document(&combined_doc, outset_dist)?;

    println!("Outset Distance: {} px", outset_report.outset_distance);
    println!("Original Bounds: {:?}", outset_report.original_bounds);
    println!("New Bounds: {:?}", outset_report.new_bounds);

    info!(
        "Outset Distance: {} px | Original Bounds: {:?} | New Bounds: {:?}",
        outset_report.outset_distance, outset_report.original_bounds, outset_report.new_bounds
    );

    // Save processed SVG with outset geometry
    let output_file = output_dir.join(file_name);
    let output_svg_content = outset_doc.to_svg_string();
    fs::write(&output_file, output_svg_content)?;
    info!("Saved processed SVG -> {}", output_file.display());

    Ok(())
}
