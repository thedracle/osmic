# Changelog

All notable changes to this project are documented here.

## 0.1.1 - 2026-05-07

### Added

- Added README badges for crates.io, docs.rs, license, and MSRV.
- Added a runnable `custom-plugin` example that registers a plugin and resource.
- Added regression coverage for multiple external tile sorters sharing a temp directory.

### Changed

- Bumped workspace crates, internal path dependency versions, and examples to `0.1.1`.
- Moved `NodeLocationStore` into `osmic-core` and re-exported it from the native OSM pipeline.
- Made `osmic-osm/native` opt-in at the workspace dependency level.
- Updated README quickstart and examples to match the current app lifecycle and PMTiles renderer.

### Fixed

- Fixed external sort chunk-file collisions when independent sorters use the same temp directory.
- Fixed `osmic-osm` and `osmic-tiles` `--no-default-features` builds.
- Fixed Clippy warnings across OSM assembly, extraction, and viewer input handling.
