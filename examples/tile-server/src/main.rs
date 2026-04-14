use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;

use osmic_serve::{TileServer, TileServerConfig};

#[derive(Parser)]
#[command(name = "tile-server", about = "Serve vector tiles from a PMTiles archive")]
struct Args {
    /// Path to the PMTiles file
    pmtiles_file: PathBuf,

    /// Address to bind to
    #[arg(long, default_value = "127.0.0.1:3000")]
    bind: SocketAddr,

    /// Cache-Control max-age in seconds
    #[arg(long, default_value = "3600")]
    cache_max_age: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    let config = TileServerConfig {
        bind_addr: args.bind,
        pmtiles_path: args.pmtiles_file,
        cache_max_age: args.cache_max_age,
    };

    let server = TileServer::new(config);
    server.serve().await?;

    Ok(())
}
