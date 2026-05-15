mod bundle;
mod manager;
mod region;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Json;
use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn};

use osmic_core::BBox;

use crate::manager::RegionManager;
use crate::region::RegionRegistry;

/// Trail map tile server — global coverage with on-demand region loading.
#[derive(Parser)]
struct Args {
    /// Directory for all data (feature packs, PBFs, cache, HGT, Geofabrik index)
    #[arg(long, default_value = "data")]
    data_dir: PathBuf,

    /// Bind address
    #[arg(long, default_value = "0.0.0.0:3000")]
    bind: String,

    /// Grid cell size in degrees (0.05 ≈ 5.5km)
    #[arg(long, default_value = "0.05")]
    grid_step: f64,

    /// Chunk size for HTTP delivery (bytes)
    #[arg(long, default_value = "4096")]
    chunk_size: usize,

    /// Maximum expected OSM node ID for PBF processing
    #[arg(long, default_value = "2000000000")]
    max_node_id: i64,

    /// Maximum regions to keep loaded in memory simultaneously
    #[arg(long, default_value = "3")]
    max_regions: usize,

    /// Optional: pre-load a specific PBF file on startup (legacy mode)
    #[arg(long)]
    pbf_file: Option<PathBuf>,
}

struct AppState {
    region_manager: Arc<RegionManager>,
    grid_step: f64,
    chunk_size: usize,
    data_dir: PathBuf,
    tile_cache: RwLock<HashMap<String, Vec<u8>>>,
}

#[derive(Deserialize)]
struct TileParams {
    lat: f64,
    lon: f64,
    chunk: Option<usize>,
}

#[derive(Serialize)]
struct TileMeta {
    size: usize,
    chunks: usize,
    #[serde(rename = "chunkSize")]
    chunk_size: usize,
    bbox: [i32; 4],
    grid: String,
    region: String,
}

#[derive(Serialize)]
struct ChunkResponse {
    d: Vec<u8>,
    i: usize,
}

fn snap_to_grid(val: f64, step: f64) -> f64 {
    (val / step).floor() * step
}

fn grid_key(lat: f64, lon: f64) -> String {
    format!("{:.2}_{:.2}", lat, lon)
}

pub fn hgt_filename(lat: f64, lon: f64) -> String {
    let lat_prefix = if lat >= 0.0 { "N" } else { "S" };
    let lon_prefix = if lon >= 0.0 { "E" } else { "W" };
    format!(
        "{}{:02}{}{:03}.hgt",
        lat_prefix,
        lat.abs().floor() as u32,
        lon_prefix,
        lon.abs().ceil() as u32
    )
}

fn extract_bbox_microdegrees(blob: &[u8]) -> [i32; 4] {
    if blob.len() < 24 {
        return [0; 4];
    }
    let read_i32 = |off: usize| -> i32 {
        i32::from_be_bytes([blob[off], blob[off + 1], blob[off + 2], blob[off + 3]])
    };
    [read_i32(8), read_i32(12), read_i32(16), read_i32(20)]
}

async fn get_tile(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TileParams>,
) -> impl IntoResponse {
    let grid_lat = snap_to_grid(params.lat, state.grid_step);
    let grid_lon = snap_to_grid(params.lon, state.grid_step);
    let key = grid_key(grid_lat, grid_lon);

    // Check tile cache (in-memory)
    {
        let cache = state.tile_cache.read().await;
        if let Some(blob) = cache.get(&key) {
            return serve_blob(blob, &key, "", &params, &state);
        }
    }

    // Check disk cache
    let region_id = state
        .region_manager
        .find_region_id(params.lat, params.lon)
        .unwrap_or_else(|| "unknown".to_string());
    let safe_region = region_id.replace('/', "_");
    let cache_dir = state.data_dir.join("cache").join(&safe_region);
    let disk_path = cache_dir.join(format!("{key}.tmap"));

    if disk_path.exists() {
        if let Ok(blob) = std::fs::read(&disk_path) {
            let mut cache = state.tile_cache.write().await;
            cache.insert(key.clone(), blob.clone());
            return serve_blob(&blob, &key, &region_id, &params, &state);
        }
    }

    // Load region (may download PBF + process — could take minutes for a new region)
    let region_data = match state.region_manager.get_region(params.lat, params.lon).await {
        Some(d) => d,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "no region data available for this location"})),
            )
                .into_response();
        }
    };

    // Generate tile
    let bbox = BBox {
        min_lon: grid_lon,
        min_lat: grid_lat,
        max_lon: grid_lon + state.grid_step,
        max_lat: grid_lat + state.grid_step,
    };
    let hgt_dir = state.data_dir.join("dem");
    let blob = region_data.generate_tile(&bbox, &hgt_dir);

    // Cache to disk
    let _ = std::fs::create_dir_all(&cache_dir);
    let _ = std::fs::write(&disk_path, &blob);

    // Cache in memory
    {
        let mut cache = state.tile_cache.write().await;
        cache.insert(key.clone(), blob.clone());
    }

    serve_blob(&blob, &key, &region_id, &params, &state)
}

fn serve_blob(
    blob: &[u8],
    key: &str,
    region_id: &str,
    params: &TileParams,
    state: &AppState,
) -> axum::response::Response {
    let mut headers = HeaderMap::new();
    headers.insert("Cache-Control", "public, max-age=86400".parse().unwrap());

    match params.chunk {
        None => {
            let chunks = (blob.len() + state.chunk_size - 1) / state.chunk_size;
            let bbox = extract_bbox_microdegrees(blob);
            let meta = TileMeta {
                size: blob.len(),
                chunks,
                chunk_size: state.chunk_size,
                bbox,
                grid: key.to_string(),
                region: region_id.to_string(),
            };
            (StatusCode::OK, headers, Json(serde_json::to_value(meta).unwrap())).into_response()
        }
        Some(i) => {
            let start = i * state.chunk_size;
            if start >= blob.len() {
                return (StatusCode::BAD_REQUEST, "chunk index out of range").into_response();
            }
            let end = (start + state.chunk_size).min(blob.len());
            let chunk_data: Vec<u8> = blob[start..end].to_vec();
            let resp = ChunkResponse { d: chunk_data, i };
            (StatusCode::OK, headers, Json(serde_json::to_value(resp).unwrap())).into_response()
        }
    }
}

async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cache = state.tile_cache.read().await;
    Json(serde_json::json!({
        "status": "ok",
        "cached_tiles": cache.len(),
    }))
}

/// List available bundles with metadata.
async fn list_bundles(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let bundle_dir = state.data_dir.join("bundles");
    let mut bundles = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&bundle_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("tpak") {
                let filename = path.file_name().unwrap().to_string_lossy().to_string();
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                // Read tile count from header
                let tile_count = std::fs::read(&path)
                    .ok()
                    .and_then(|data| {
                        if data.len() >= 8 && &data[0..4] == b"TPAK" {
                            Some(u32::from_le_bytes([data[4], data[5], data[6], data[7]]))
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);

                bundles.push(serde_json::json!({
                    "filename": filename,
                    "size": size,
                    "tiles": tile_count,
                    "url": format!("/bundles/{}", filename),
                }));
            }
        }
    }

    Json(serde_json::json!({ "bundles": bundles }))
}

/// Generate a bundle for a region (POST /bundles/generate).
/// Query params: lat, lon (to identify the Geofabrik region), name (output filename)
#[derive(Deserialize)]
struct GenerateParams {
    lat: f64,
    lon: f64,
    name: String,
    #[serde(default = "default_min_lat")]
    min_lat: Option<f64>,
    #[serde(default)]
    min_lon: Option<f64>,
    #[serde(default)]
    max_lat: Option<f64>,
    #[serde(default)]
    max_lon: Option<f64>,
}
fn default_min_lat() -> Option<f64> { None }

async fn generate_bundle_endpoint(
    State(state): State<Arc<AppState>>,
    Query(params): Query<GenerateParams>,
) -> impl IntoResponse {
    // Load the region for this lat/lon
    let region_data = match state.region_manager.get_region(params.lat, params.lon).await {
        Some(d) => d,
        None => return (StatusCode::NOT_FOUND, "no region for this location").into_response(),
    };

    // Use custom bbox if provided, otherwise use region bbox
    let bbox = if let (Some(min_lat), Some(min_lon), Some(max_lat), Some(max_lon)) =
        (params.min_lat, params.min_lon, params.max_lat, params.max_lon) {
        BBox { min_lon, min_lat, max_lon, max_lat }
    } else {
        // Use region's full bbox
        BBox {
            min_lat: params.lat - 0.5,
            max_lat: params.lat + 0.5,
            min_lon: params.lon - 0.5,
            max_lon: params.lon + 0.5,
        }
    };

    let config = bundle::BundleConfig {
        region_bbox: bbox,
        grid_step: state.grid_step,
        display_width: 176,
        display_height: 176,
        contour_interval: 40,
        hgt_dir: state.data_dir.join("dem").to_string_lossy().to_string(),
    };

    info!(name = %params.name, "generating bundle");

    // Generate synchronously (region data is Arc'd, no clone needed)
    let tpak = bundle::generate_bundle(
        &region_data.features,
        &region_data.tag_store,
        &region_data.feature_index,
        &config,
    );

    // Save to disk
    let bundle_path = state.data_dir.join("bundles").join(format!("{}.tpak", params.name));
    if let Err(e) = std::fs::write(&bundle_path, &tpak) {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("write error: {e}")).into_response();
    }

    info!(name = %params.name, tiles = tpak.len(), "bundle saved");

    Json(serde_json::json!({
        "status": "generated",
        "name": params.name,
        "size": tpak.len(),
        "path": format!("/bundles/{}.tpak", params.name),
    })).into_response()
}

/// Generate and return a bundle for a bbox (POST-style via GET for simplicity).
/// The companion app sends the user's selected rectangle.
/// GET /bundles/bbox?min_lat=40.4&min_lon=-112.0&max_lat=41.0&max_lon=-111.5
/// Returns the TPAK binary directly (or from cache if already generated).
#[derive(Deserialize)]
struct BboxBundleParams {
    min_lat: f64,
    min_lon: f64,
    max_lat: f64,
    max_lon: f64,
}

async fn bbox_bundle(
    State(state): State<Arc<AppState>>,
    Query(params): Query<BboxBundleParams>,
) -> impl IntoResponse {
    // Create a cache key from the bbox (snapped to grid)
    let step = state.grid_step;
    let snap = |v: f64| -> f64 { (v / step).floor() * step };
    let cache_name = format!(
        "bbox_{:.2}_{:.2}_{:.2}_{:.2}",
        snap(params.min_lat), snap(params.min_lon),
        snap(params.max_lat), snap(params.max_lon)
    );
    let bundle_path = state.data_dir.join("bundles").join(format!("{cache_name}.tpak"));

    // Serve from cache if exists
    if bundle_path.exists() {
        if let Ok(data) = std::fs::read(&bundle_path) {
            let mut headers = HeaderMap::new();
            headers.insert("Content-Type", "application/octet-stream".parse().unwrap());
            headers.insert("Cache-Control", "public, max-age=86400".parse().unwrap());
            return (StatusCode::OK, headers, data).into_response();
        }
    }

    // Need to load the region — use center point to find it
    let center_lat = (params.min_lat + params.max_lat) / 2.0;
    let center_lon = (params.min_lon + params.max_lon) / 2.0;

    let region_data = match state.region_manager.get_region(center_lat, center_lon).await {
        Some(d) => d,
        None => return (StatusCode::NOT_FOUND, "no region data for this area").into_response(),
    };

    let bbox = BBox {
        min_lat: params.min_lat,
        min_lon: params.min_lon,
        max_lat: params.max_lat,
        max_lon: params.max_lon,
    };

    let config = bundle::BundleConfig {
        region_bbox: bbox,
        grid_step: state.grid_step,
        display_width: 176,
        display_height: 176,
        contour_interval: 40,
        hgt_dir: state.data_dir.join("dem").to_string_lossy().to_string(),
    };

    info!(name = %cache_name, "generating bbox bundle");

    let tpak = bundle::generate_bundle(
        &region_data.features,
        &region_data.tag_store,
        &region_data.feature_index,
        &config,
    );

    // Cache to disk
    let _ = std::fs::write(&bundle_path, &tpak);

    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "application/octet-stream".parse().unwrap());
    headers.insert("Cache-Control", "public, max-age=86400".parse().unwrap());
    (StatusCode::OK, headers, tpak).into_response()
}

/// Serve a bundle file for download.
async fn serve_bundle(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(filename): axum::extract::Path<String>,
) -> impl IntoResponse {
    // Sanitize filename
    if filename.contains("..") || filename.contains('/') {
        return (StatusCode::BAD_REQUEST, "invalid filename").into_response();
    }

    let path = state.data_dir.join("bundles").join(&filename);
    if !path.exists() {
        return (StatusCode::NOT_FOUND, "bundle not found").into_response();
    }

    match std::fs::read(&path) {
        Ok(data) => {
            let mut headers = HeaderMap::new();
            headers.insert("Content-Type", "application/octet-stream".parse().unwrap());
            headers.insert(
                "Content-Disposition",
                format!("attachment; filename=\"{}\"", filename).parse().unwrap(),
            );
            headers.insert("Cache-Control", "public, max-age=86400".parse().unwrap());
            (StatusCode::OK, headers, data).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "read error").into_response(),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();
    let data_dir = args.data_dir.clone();
    let _ = std::fs::create_dir_all(data_dir.join("bundles"));

    // Load Geofabrik region index
    let registry = RegionRegistry::load(&args.data_dir)?;

    let region_manager = Arc::new(RegionManager::new(
        registry,
        args.data_dir.clone(),
        args.max_node_id,
        args.max_regions,
    ));

    // Legacy: if a PBF file was specified, pre-load it as a region
    if let Some(pbf_path) = &args.pbf_file {
        info!(pbf = %pbf_path.display(), "pre-loading PBF file");
        // Create a synthetic region entry and load it
        // The region manager will detect the .fpack on disk for future restarts
        let fpack_path = pbf_path.with_extension("fpack");
        if !fpack_path.exists() {
            info!("processing PBF to create feature pack (first run)");
            use osmic_compact::feature_pack::FeaturePack;
            use osmic_index::RamNodeLocationStore;
            use osmic_osm::pipeline::PbfProcessor;
            use osmic_osm::LayerSet;

            let node_store = RamNodeLocationStore::create(args.max_node_id)?;
            let processor = PbfProcessor::new();
            let result = processor.process(pbf_path, &node_store, &LayerSet::all())?;

            let mut bbox = osmic_core::BBox::empty();
            for f in &result.features {
                let fb = f.bbox();
                bbox.expand(fb.min_lon, fb.min_lat);
                bbox.expand(fb.max_lon, fb.max_lat);
            }
            let pack = FeaturePack::from_processed(&result.features, &result.tag_store, &bbox);
            pack.write_to(&fpack_path)?;
            info!("feature pack saved");
        }
    }

    let state = Arc::new(AppState {
        region_manager,
        grid_step: args.grid_step,
        chunk_size: args.chunk_size,
        data_dir,
        tile_cache: RwLock::new(HashMap::new()),
    });


    let app = axum::Router::new()
        .route("/tile", get(get_tile))
        .route("/health", get(health))
        .route("/bundles/index.json", get(list_bundles))
        .route("/bundles/generate", get(generate_bundle_endpoint))
        .route("/bundles/bbox", get(bbox_bundle))
        .route("/bundles/{filename}", get(serve_bundle))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(state);

    // -- Bundle endpoint handlers are defined below --

    let listener = tokio::net::TcpListener::bind(&args.bind).await?;
    info!(bind = %args.bind, "server listening");
    axum::serve(listener, app).await?;

    Ok(())
}
