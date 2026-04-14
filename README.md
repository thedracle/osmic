# osmic

A Rust workspace for working with OpenStreetMap data end-to-end: parsing PBF,
building spatial indexes, generating vector tiles, styling, rendering, and
serving.

**Status:** early — `0.1.0`, APIs will change.

## Installation

Use the umbrella crate for the full SDK:

```toml
[dependencies]
osmic = "0.1"
```

Or pull in individual crates as needed:

```toml
[dependencies]
osmic-core = "0.1"
osmic-osm = "0.1"
osmic-tiles = "0.1"
```

## Workspace layout

| Crate | Description |
| --- | --- |
| [`osmic`](crates/osmic) | Umbrella: re-exports, prelude, default plugin groups |
| [`osmic-core`](crates/osmic-core) | Shared types, errors, coordinate primitives |
| [`osmic-osm`](crates/osmic-osm) | OSM data model, PBF parsing, tag system, classification |
| [`osmic-geo`](crates/osmic-geo) | Projection, simplification, clipping, validation |
| [`osmic-index`](crates/osmic-index) | Node location store (mmap) and spatial R-tree index |
| [`osmic-style`](crates/osmic-style) | MapLibre-compatible style system |
| [`osmic-tiles`](crates/osmic-tiles) | MVT/MLT encoding, PMTiles archives, tile math |
| [`osmic-render`](crates/osmic-render) | Backend abstraction, scene graph, tessellation |
| [`osmic-text`](crates/osmic-text) | Text shaping, label collision, glyph atlas |
| [`osmic-app`](crates/osmic-app) | Plugin system, App builder, event bus, lifecycle |
| [`osmic-serve`](crates/osmic-serve) | HTTP tile server with Tower middleware pipeline |
| [`osmic-extract`](crates/osmic-extract) | Business entity extraction from OSM PBF |
| [`osmic-accel`](crates/osmic-accel) | GPU-accelerated geometry processing (Apple Silicon Metal) |
| [`osmic-repl`](crates/osmic-repl) | OSM replication and incremental tile updates |
| [`osmic-cli`](bins/osmic-cli) | CLI for tile generation, PBF inspection, render-to-image |
| [`osmic-viewer`](bins/osmic-viewer) | Interactive wgpu windowed map viewer |

## Quickstart

```rust
use osmic::prelude::*;

fn main() -> OsmicResult<()> {
    let app = App::new()
        .add_plugins(osmic::HeadlessPlugins)
        .run();
    Ok(())
}
```

See [`examples/`](examples/) for full runnable programs:

- `load-pbf` — parse a PBF file and index features
- `render-static` — render a static PNG from a PBF
- `tile-server` — serve vector tiles from a PMTiles archive
- `custom-plugin` — define and register a plugin

## Building

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
```

Minimum supported Rust version: **1.94**.

## License

MIT — see [LICENSE](LICENSE).
