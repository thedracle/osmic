//! Output formatters for extracted entities.

use std::io::{BufWriter, Write};
use std::path::Path;

use osmic_core::error::{OsmicError, OsmicResult};

use crate::entity::Entity;

/// Write entities to a CSV file.
///
/// Columns: name, type, lat, lon, address, phone, website, operator, tags
pub fn write_csv(entities: &[Entity], path: &Path) -> OsmicResult<()> {
    let file = std::fs::File::create(path).map_err(OsmicError::Io)?;
    let mut writer = BufWriter::new(file);

    // Header
    writeln!(
        writer,
        "name,type,lat,lon,address,phone,website,operator,tags"
    )
    .map_err(OsmicError::Io)?;

    for e in entities {
        writeln!(
            writer,
            "{},{},{},{},{},{},{},{},{}",
            csv_escape(&e.name),
            e.osm_type,
            e.lat.map(|v| format!("{v:.6}")).unwrap_or_default(),
            e.lon.map(|v| format!("{v:.6}")).unwrap_or_default(),
            csv_escape(&e.address),
            csv_escape(&e.phone),
            csv_escape(&e.website),
            csv_escape(&e.operator),
            csv_escape(&e.tags),
        )
        .map_err(OsmicError::Io)?;
    }

    writer.flush().map_err(OsmicError::Io)?;
    Ok(())
}

/// Write entities to a JSON file.
pub fn write_json(entities: &[Entity], path: &Path) -> OsmicResult<()> {
    let json =
        serde_json::to_string_pretty(entities).map_err(|e| OsmicError::Other(e.to_string()))?;
    std::fs::write(path, json).map_err(OsmicError::Io)
}

/// Write entities to a GeoJSON FeatureCollection file.
///
/// Each entity becomes a GeoJSON Feature with a Point geometry (if lat/lon
/// are available) and all metadata as properties.  Entities without
/// coordinates are included with a `null` geometry per the GeoJSON spec.
pub fn write_geojson(entities: &[Entity], path: &Path) -> OsmicResult<()> {
    let features: Vec<serde_json::Value> = entities
        .iter()
        .map(|e| {
            let geometry = match (e.lon, e.lat) {
                (Some(lon), Some(lat)) => serde_json::json!({
                    "type": "Point",
                    "coordinates": [lon, lat]
                }),
                _ => serde_json::Value::Null,
            };

            let mut props = serde_json::Map::new();
            props.insert("name".into(), e.name.clone().into());
            props.insert("osm_type".into(), e.osm_type.clone().into());
            props.insert("osm_id".into(), e.osm_id.into());
            if !e.address.is_empty() {
                props.insert("address".into(), e.address.clone().into());
            }
            if !e.phone.is_empty() {
                props.insert("phone".into(), e.phone.clone().into());
            }
            if !e.website.is_empty() {
                props.insert("website".into(), e.website.clone().into());
            }
            if !e.operator.is_empty() {
                props.insert("operator".into(), e.operator.clone().into());
            }
            if !e.tags.is_empty() {
                props.insert("tags".into(), e.tags.clone().into());
            }

            serde_json::json!({
                "type": "Feature",
                "geometry": geometry,
                "properties": props
            })
        })
        .collect();

    let collection = serde_json::json!({
        "type": "FeatureCollection",
        "features": features
    });

    let json =
        serde_json::to_string_pretty(&collection).map_err(|e| OsmicError::Other(e.to_string()))?;
    std::fs::write(path, json).map_err(OsmicError::Io)
}

/// Escape a field for CSV output (RFC 4180).
fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_escape_plain() {
        assert_eq!(csv_escape("hello"), "hello");
    }

    #[test]
    fn test_csv_escape_comma() {
        assert_eq!(csv_escape("hello, world"), "\"hello, world\"");
    }

    #[test]
    fn test_csv_escape_quotes() {
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn test_csv_escape_newline() {
        assert_eq!(csv_escape("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn test_csv_escape_carriage_return() {
        assert_eq!(csv_escape("line1\rline2"), "\"line1\rline2\"");
    }
}
