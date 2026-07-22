use pyo3::prelude::*;
use tracing_subscriber::FmtSubscriber;

/// Initializes tracing logging bridge subscriber for Rust engine.
pub fn init_rust_subscriber(level: &str) {
    let filter = level.parse().unwrap_or(tracing::Level::INFO);
    let subscriber = FmtSubscriber::builder()
        .with_max_level(filter)
        .with_target(false)
        .finish();

    let _ = tracing::subscriber::set_global_default(subscriber);
}

#[pyfunction]
pub fn init_tracing(level: Option<&str>) -> PyResult<()> {
    let lvl = level.unwrap_or("INFO");
    init_rust_subscriber(lvl);
    Ok(())
}
