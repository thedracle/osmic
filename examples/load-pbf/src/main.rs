use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use tracing::info;

use osmic_index::DenseNodeLocationStore;
use osmic_osm::pipeline::PbfProcessor;

#[derive(Parser)]
#[command(name = "load-pbf", about = "Load an OSM PBF file and print statistics")]
struct Args {
    /// Path to the .osm.pbf file
    pbf_file: PathBuf,

    /// Path for the node location store (temporary mmap file)
    #[arg(long, default_value = "/tmp/osmic-nodes.bin")]
    node_store: PathBuf,

    /// Maximum expected node ID
    #[arg(long, default_value = "13000000000")]
    max_node_id: i64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    println!("=== Osmic - PBF Loader ===");
    println!("File: {}", args.pbf_file.display());

    // Verify file exists
    if !args.pbf_file.exists() {
        eprintln!("Error: PBF file not found: {}", args.pbf_file.display());
        std::process::exit(1);
    }

    let file_size = std::fs::metadata(&args.pbf_file)?.len();
    println!(
        "Size: {:.2} GB",
        file_size as f64 / (1024.0 * 1024.0 * 1024.0)
    );

    // Create node location store
    info!(
        "Creating node location store at {}",
        args.node_store.display()
    );
    let node_store = DenseNodeLocationStore::create(&args.node_store, args.max_node_id)?;

    // Process PBF
    let start = Instant::now();
    let processor = PbfProcessor::new();
    let result = processor.process(&args.pbf_file, &node_store, &osmic_osm::LayerSet::all())?;

    // Build spatial index
    info!("Building spatial index...");
    let index_start = Instant::now();
    let index = osmic_index::FeatureIndex::build(&result.features);
    let index_duration = index_start.elapsed();
    info!(
        "Spatial index built: {} entries in {:.2}s",
        index.len(),
        index_duration.as_secs_f64()
    );

    // Print statistics
    println!("\n=== Statistics ===");
    println!(
        "Nodes processed:       {:>14}",
        format_number(result.stats.node_count)
    );
    println!(
        "Ways processed:        {:>14}",
        format_number(result.stats.way_count)
    );
    println!(
        "Relations found:       {:>14}",
        format_number(result.stats.relation_count)
    );
    println!(
        "Features created:      {:>14}",
        format_number(result.features.len() as u64)
    );
    println!(
        "R-tree entries:        {:>14}",
        format_number(index.len() as u64)
    );
    println!("Bounding box:          {}", result.bbox);
    println!(
        "Interned strings:      {:>14}",
        format_number(result.tag_store.len() as u64)
    );
    println!(
        "\nPass 1 (nodes):        {:>11.2}s",
        result.stats.pass1_duration.as_secs_f64()
    );
    println!(
        "Pass 2 (ways):         {:>11.2}s",
        result.stats.pass2_duration.as_secs_f64()
    );
    println!(
        "Spatial index build:   {:>11.2}s",
        index_duration.as_secs_f64()
    );
    println!(
        "Total time:            {:>11.2}s",
        start.elapsed().as_secs_f64()
    );

    // Sample query: center of bounding box
    let center = result.bbox.center();
    let query_bbox = osmic_core::BBox::new(
        center.lon - 0.01,
        center.lat - 0.01,
        center.lon + 0.01,
        center.lat + 0.01,
    );
    let query_results = index.query_bbox(&query_bbox);
    println!(
        "\nSample query at center ({:.4}, {:.4}):",
        center.lon, center.lat
    );
    println!("  Features in 0.02x0.02 deg box: {}", query_results.len());

    // Cleanup temp file
    if args.node_store.starts_with("/tmp") {
        info!("Cleaning up node store: {}", args.node_store.display());
        let _ = std::fs::remove_file(&args.node_store);
    }

    Ok(())
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}
