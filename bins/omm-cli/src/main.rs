use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, Subcommand};
use omm_index::DenseNodeLocationStore;
use omm_osm::pipeline::PbfProcessor;
use omm_tiles::pipeline::{TileGenerator, TileGeneratorConfig};
use omm_tiles::pmtiles::PmTilesArchive;

#[derive(Parser)]
#[command(name = "omm", about = "OpenMapMarketor CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate vector tiles from a PBF file
    GenerateTiles {
        /// Input PBF file
        pbf_file: PathBuf,

        /// Output PMTiles file
        output: PathBuf,

        /// Zoom range (e.g., "0-14")
        #[arg(long, default_value = "0-14")]
        zoom: String,

        /// Path for node location store (temporary mmap file)
        #[arg(long, default_value = "/tmp/omm-nodes.bin")]
        node_store: PathBuf,

        /// Maximum expected node ID
        #[arg(long, default_value = "13000000000")]
        max_node_id: i64,

        /// MVT tile extent
        #[arg(long, default_value = "4096")]
        extent: u32,

        /// Write MapLibre style JSON to this path
        #[arg(long)]
        style: Option<PathBuf>,
    },

    /// Inspect a PBF file and print statistics
    Inspect {
        /// Input PBF file
        pbf_file: PathBuf,

        /// Path for node location store
        #[arg(long, default_value = "/tmp/omm-nodes.bin")]
        node_store: PathBuf,

        /// Maximum expected node ID
        #[arg(long, default_value = "13000000000")]
        max_node_id: i64,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::GenerateTiles {
            pbf_file,
            output,
            zoom,
            node_store,
            max_node_id,
            extent,
            style,
        } => {
            generate_tiles(
                &pbf_file,
                &output,
                &zoom,
                &node_store,
                max_node_id,
                extent,
                style.as_deref(),
            )?;
        }
        Commands::Inspect {
            pbf_file,
            node_store,
            max_node_id,
        } => {
            inspect(&pbf_file, &node_store, max_node_id)?;
        }
    }

    Ok(())
}

fn parse_zoom_range(s: &str) -> Result<(u8, u8), String> {
    if let Some((min, max)) = s.split_once('-') {
        let min: u8 = min.parse().map_err(|e| format!("Invalid min zoom: {e}"))?;
        let max: u8 = max.parse().map_err(|e| format!("Invalid max zoom: {e}"))?;
        Ok((min, max))
    } else {
        let z: u8 = s.parse().map_err(|e| format!("Invalid zoom: {e}"))?;
        Ok((z, z))
    }
}

fn generate_tiles(
    pbf_file: &std::path::Path,
    output: &std::path::Path,
    zoom: &str,
    node_store_path: &std::path::Path,
    max_node_id: i64,
    extent: u32,
    style_path: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (min_zoom, max_zoom) = parse_zoom_range(zoom)?;
    println!("=== OpenMapMarketor - Tile Generator ===");
    println!("Input:  {}", pbf_file.display());
    println!("Output: {}", output.display());
    println!("Zoom:   {}-{}", min_zoom, max_zoom);

    if !pbf_file.exists() {
        eprintln!("Error: PBF file not found: {}", pbf_file.display());
        std::process::exit(1);
    }

    // Step 1: Process PBF
    let total_start = Instant::now();
    let node_store = DenseNodeLocationStore::create(node_store_path, max_node_id)?;
    let processor = PbfProcessor::new();
    let result = processor.process(pbf_file, &node_store)?;

    println!(
        "\nPBF processed: {} features from {} ways in {:.1}s",
        format_number(result.features.len() as u64),
        format_number(result.stats.way_count),
        result.stats.total_duration.as_secs_f64()
    );

    // Step 2: Create PMTiles archive
    let mut archive = PmTilesArchive::create(output, &result.bbox, min_zoom, max_zoom)?;

    // Step 3: Generate tiles
    let config = TileGeneratorConfig {
        min_zoom,
        max_zoom,
        extent,
        batch_size: 10_000,
    };

    let generator = TileGenerator::new(
        &result.features,
        &result.tag_store,
        result.bbox,
        config,
    );

    let tile_count = generator.generate_all(|coord, data| {
        archive.add_tile(coord, data)
    })?;

    // Step 4: Finalize
    archive.finalize()?;

    println!(
        "\nGenerated {} tiles in {:.1}s",
        format_number(tile_count),
        total_start.elapsed().as_secs_f64()
    );
    println!("Output: {}", output.display());

    // Step 5: Write style if requested
    if let Some(style_path) = style_path {
        let pmtiles_url = format!("pmtiles://{}", output.display());
        let style = omm_style::default_style_json(&pmtiles_url);
        std::fs::write(style_path, style.to_json())?;
        println!("Style:  {}", style_path.display());
    }

    // Cleanup temp file
    if node_store_path.starts_with("/tmp") {
        let _ = std::fs::remove_file(node_store_path);
    }

    Ok(())
}

fn inspect(
    pbf_file: &std::path::Path,
    node_store_path: &std::path::Path,
    max_node_id: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OpenMapMarketor - PBF Inspector ===");
    println!("File: {}", pbf_file.display());

    if !pbf_file.exists() {
        eprintln!("Error: PBF file not found: {}", pbf_file.display());
        std::process::exit(1);
    }

    let file_size = std::fs::metadata(pbf_file)?.len();
    println!(
        "Size: {:.2} GB",
        file_size as f64 / (1024.0 * 1024.0 * 1024.0)
    );

    let node_store = DenseNodeLocationStore::create(node_store_path, max_node_id)?;
    let processor = PbfProcessor::new();
    let result = processor.process(pbf_file, &node_store)?;

    println!("\n--- Statistics ---");
    println!(
        "Nodes:      {:>14}",
        format_number(result.stats.node_count)
    );
    println!(
        "Ways:       {:>14}",
        format_number(result.stats.way_count)
    );
    println!(
        "Relations:  {:>14}",
        format_number(result.stats.relation_count)
    );
    println!(
        "Features:   {:>14}",
        format_number(result.features.len() as u64)
    );
    println!(
        "Strings:    {:>14}",
        format_number(result.tag_store.len() as u64)
    );
    println!("Bbox:       {}", result.bbox);
    println!(
        "Total time: {:>11.2}s",
        result.stats.total_duration.as_secs_f64()
    );

    // Feature breakdown
    let mut kind_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for f in &result.features {
        *kind_counts.entry(f.kind.layer_name()).or_default() += 1;
    }
    println!("\n--- Feature Breakdown ---");
    let mut counts: Vec<_> = kind_counts.iter().collect();
    counts.sort_by(|a, b| b.1.cmp(a.1));
    for (kind, count) in counts {
        println!("  {:<12} {:>12}", kind, format_number(*count as u64));
    }

    if node_store_path.starts_with("/tmp") {
        let _ = std::fs::remove_file(node_store_path);
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
