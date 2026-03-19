//! Output formatters for extracted entities.

use std::io::Write;
use std::path::Path;

use omm_core::error::{OmmError, OmmResult};

use crate::entity::Entity;

/// Write entities to a CSV file.
///
/// Columns: name, type, lat, lon, address, phone, website, operator, tags
pub fn write_csv(entities: &[Entity], path: &Path) -> OmmResult<()> {
    let mut file = std::fs::File::create(path)
        .map_err(|e| OmmError::Io(e))?;

    // Header
    writeln!(file, "name,type,lat,lon,address,phone,website,operator,tags")
        .map_err(|e| OmmError::Io(e))?;

    for e in entities {
        writeln!(
            file,
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
        .map_err(|e| OmmError::Io(e))?;
    }

    Ok(())
}

/// Write entities to a JSON file.
pub fn write_json(entities: &[Entity], path: &Path) -> OmmResult<()> {
    let json = serde_json::to_string_pretty(entities)
        .map_err(|e| OmmError::Other(e.to_string()))?;
    std::fs::write(path, json).map_err(|e| OmmError::Io(e))
}

/// Escape a field for CSV output (RFC 4180).
fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
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
}
