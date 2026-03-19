use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse};

use crate::SharedState;

/// Serve a vector tile from the PMTiles archive.
pub async fn get_tile(
    State(state): State<SharedState>,
    Path((z, x, y)): Path<(u8, u32, u32)>,
) -> impl IntoResponse {
    let coord = match pmtiles::TileCoord::new(z, x, y) {
        Ok(c) => c,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, "Invalid tile coordinates").into_response()
        }
    };

    match state.reader.get_tile_decompressed(coord).await {
        Ok(Some(data)) => {
            tracing::info!(z, x, y, bytes = data.len(), "tile hit");
            let h = state.reader.get_header();
            // Serve decompressed MVT; let tower-http CompressionLayer handle encoding
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, h.tile_type.content_type())],
                data,
            )
                .into_response()
        }
        Ok(None) => {
            tracing::info!(z, x, y, "tile miss (no data)");
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            tracing::error!(z, x, y, error = %e, "Tile read error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response()
        }
    }
}

/// Serve PMTiles metadata as JSON.
pub async fn get_metadata(State(state): State<SharedState>) -> impl IntoResponse {
    match state.reader.get_metadata().await {
        Ok(json) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            json,
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Metadata read error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response()
        }
    }
}

/// Serve the MapLibre-compatible style JSON (no-cache to avoid stale URLs).
pub async fn get_style(State(state): State<SharedState>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    (StatusCode::OK, headers, state.style_json.clone())
}

/// Serve an embedded MapLibre GL JS viewer page.
pub async fn get_viewer(State(state): State<SharedState>) -> impl IntoResponse {
    let h = state.reader.get_header();
    let _ = (h.min_longitude, h.min_latitude, h.max_longitude, h.max_latitude);

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>OpenMapMarketor Viewer</title>
    <script src="https://unpkg.com/maplibre-gl@5.3.0/dist/maplibre-gl.js"></script>
    <link href="https://unpkg.com/maplibre-gl@5.3.0/dist/maplibre-gl.css" rel="stylesheet">
    <style>
        body {{ margin: 0; }}
        #map {{ width: 100%; height: 100vh; }}
        .info {{
            position: absolute; top: 10px; left: 10px; z-index: 1;
            background: rgba(255,255,255,0.9); padding: 8px 12px;
            border-radius: 4px; font-family: sans-serif; font-size: 13px;
            box-shadow: 0 1px 3px rgba(0,0,0,0.2);
        }}
    </style>
</head>
<body>
    <div id="map"></div>
    <div class="info">OpenMapMarketor | <a href="/style.json">Style</a> | <a href="/metadata">Metadata</a></div>
    <script>
        const map = new maplibregl.Map({{
            container: 'map',
            style: '/style.json?v=' + Date.now(),
            center: [-98.5, 39.8],
            zoom: 7,
            maxZoom: {max_zoom},
        }});
        map.addControl(new maplibregl.NavigationControl());
    </script>
</body>
</html>"#,
        max_zoom = h.max_zoom,
    );

    Html(html)
}
