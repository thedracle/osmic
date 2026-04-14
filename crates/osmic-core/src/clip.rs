use geo_types::{Coord, LineString, MultiPolygon, Polygon};

use crate::bbox::BBox;
use crate::geometry::Geometry;

/// Cohen-Sutherland region codes.
const INSIDE: u8 = 0b0000;
const LEFT: u8 = 0b0001;
const RIGHT: u8 = 0b0010;
const BOTTOM: u8 = 0b0100;
const TOP: u8 = 0b1000;

fn region_code(x: f64, y: f64, bbox: &BBox) -> u8 {
    let mut code = INSIDE;
    if x < bbox.min_lon {
        code |= LEFT;
    } else if x > bbox.max_lon {
        code |= RIGHT;
    }
    if y < bbox.min_lat {
        code |= BOTTOM;
    } else if y > bbox.max_lat {
        code |= TOP;
    }
    code
}

/// Clip a line segment to a bbox using Cohen-Sutherland.
/// Returns Some((x0,y0,x1,y1)) if visible, None if fully outside.
fn clip_segment(
    mut x0: f64,
    mut y0: f64,
    mut x1: f64,
    mut y1: f64,
    bbox: &BBox,
) -> Option<(f64, f64, f64, f64)> {
    let mut code0 = region_code(x0, y0, bbox);
    let mut code1 = region_code(x1, y1, bbox);

    loop {
        if (code0 | code1) == 0 {
            return Some((x0, y0, x1, y1));
        }
        if (code0 & code1) != 0 {
            return None;
        }

        let code_out = if code0 != 0 { code0 } else { code1 };
        let (x, y);

        if code_out & TOP != 0 {
            x = x0 + (x1 - x0) * (bbox.max_lat - y0) / (y1 - y0);
            y = bbox.max_lat;
        } else if code_out & BOTTOM != 0 {
            x = x0 + (x1 - x0) * (bbox.min_lat - y0) / (y1 - y0);
            y = bbox.min_lat;
        } else if code_out & RIGHT != 0 {
            y = y0 + (y1 - y0) * (bbox.max_lon - x0) / (x1 - x0);
            x = bbox.max_lon;
        } else {
            y = y0 + (y1 - y0) * (bbox.min_lon - x0) / (x1 - x0);
            x = bbox.min_lon;
        }

        if code_out == code0 {
            x0 = x;
            y0 = y;
            code0 = region_code(x0, y0, bbox);
        } else {
            x1 = x;
            y1 = y;
            code1 = region_code(x1, y1, bbox);
        }
    }
}

/// Clip a polyline to a bbox, returning zero or more clipped segments.
pub fn clip_line(line: &LineString<f64>, bbox: &BBox) -> Vec<LineString<f64>> {
    let coords: Vec<_> = line.coords().collect();
    if coords.len() < 2 {
        return vec![];
    }

    let mut result = Vec::new();
    let mut current_segment: Vec<Coord<f64>> = Vec::new();

    for window in coords.windows(2) {
        let (c0, c1) = (window[0], window[1]);
        if let Some((x0, y0, x1, y1)) = clip_segment(c0.x, c0.y, c1.x, c1.y, bbox) {
            let start = Coord { x: x0, y: y0 };
            let end = Coord { x: x1, y: y1 };

            if current_segment.is_empty() {
                current_segment.push(start);
            } else if (current_segment.last().unwrap().x - start.x).abs() > 1e-10
                || (current_segment.last().unwrap().y - start.y).abs() > 1e-10
            {
                if current_segment.len() >= 2 {
                    result.push(LineString::new(std::mem::take(&mut current_segment)));
                } else {
                    current_segment.clear();
                }
                current_segment.push(start);
            }
            current_segment.push(end);
        } else if current_segment.len() >= 2 {
            result.push(LineString::new(std::mem::take(&mut current_segment)));
        } else {
            current_segment.clear();
        }
    }

    if current_segment.len() >= 2 {
        result.push(LineString::new(current_segment));
    }

    result
}

/// Clip a polygon to a bbox using Sutherland-Hodgman algorithm.
pub fn clip_polygon(poly: &Polygon<f64>, bbox: &BBox) -> Option<Polygon<f64>> {
    let exterior = sutherland_hodgman(&poly.exterior().0, bbox);
    if exterior.len() < 3 {
        return None;
    }

    let interiors: Vec<_> = poly
        .interiors()
        .iter()
        .filter_map(|ring| {
            let clipped = sutherland_hodgman(&ring.0, bbox);
            if clipped.len() >= 3 {
                Some(LineString::new(clipped))
            } else {
                None
            }
        })
        .collect();

    Some(Polygon::new(LineString::new(exterior), interiors))
}

/// Clip a geometry to a bounding box with an optional buffer.
///
/// Returns `None` if the geometry is entirely outside the bbox.
/// For lines, may return multiple segments if the line crosses the bbox boundary.
pub fn clip_geometry(geom: &Geometry, bbox: &BBox, buffer_fraction: f64) -> Option<Geometry> {
    let buffered = if buffer_fraction > 0.0 {
        let bw = bbox.width() * buffer_fraction;
        let bh = bbox.height() * buffer_fraction;
        BBox::new(
            bbox.min_lon - bw,
            bbox.min_lat - bh,
            bbox.max_lon + bw,
            bbox.max_lat + bh,
        )
    } else {
        *bbox
    };

    match geom {
        Geometry::Point(p) => {
            if buffered.contains_point(p.x(), p.y()) {
                Some(Geometry::Point(*p))
            } else {
                None
            }
        }
        Geometry::Line(ls) => {
            let segments = clip_line(ls, &buffered);
            if segments.is_empty() {
                None
            } else if segments.len() == 1 {
                Some(Geometry::Line(segments.into_iter().next().unwrap()))
            } else {
                // Return the longest segment to keep the label position meaningful
                let longest = segments
                    .into_iter()
                    .max_by_key(|s| s.coords().count())
                    .unwrap();
                Some(Geometry::Line(longest))
            }
        }
        Geometry::Polygon(poly) => clip_polygon(poly, &buffered).map(Geometry::Polygon),
        Geometry::MultiPolygon(mp) => {
            let clipped: Vec<Polygon<f64>> = mp
                .iter()
                .filter_map(|poly| clip_polygon(poly, &buffered))
                .collect();
            if clipped.is_empty() {
                None
            } else {
                Some(Geometry::MultiPolygon(MultiPolygon::new(clipped)))
            }
        }
    }
}

fn sutherland_hodgman(vertices: &[Coord<f64>], bbox: &BBox) -> Vec<Coord<f64>> {
    if vertices.is_empty() {
        return vec![];
    }

    let mut output = vertices.to_vec();

    type InsideFn = fn(&Coord<f64>, &BBox) -> bool;
    type IntersectFn = fn(&Coord<f64>, &Coord<f64>, &BBox) -> Coord<f64>;

    // Clip against each edge: left, right, bottom, top
    let edges: [(InsideFn, IntersectFn); 4] = [
        (
            |p, b| p.x >= b.min_lon,
            |s, e, b| intersect_x(s, e, b.min_lon),
        ),
        (
            |p, b| p.x <= b.max_lon,
            |s, e, b| intersect_x(s, e, b.max_lon),
        ),
        (
            |p, b| p.y >= b.min_lat,
            |s, e, b| intersect_y(s, e, b.min_lat),
        ),
        (
            |p, b| p.y <= b.max_lat,
            |s, e, b| intersect_y(s, e, b.max_lat),
        ),
    ];

    for (inside, intersect) in &edges {
        if output.is_empty() {
            break;
        }

        let input = std::mem::take(&mut output);
        let len = input.len();

        for i in 0..len {
            let current = &input[i];
            let previous = &input[(i + len - 1) % len];

            if inside(current, bbox) {
                if !inside(previous, bbox) {
                    output.push(intersect(previous, current, bbox));
                }
                output.push(*current);
            } else if inside(previous, bbox) {
                output.push(intersect(previous, current, bbox));
            }
        }
    }

    output
}

fn intersect_x(a: &Coord<f64>, b: &Coord<f64>, x: f64) -> Coord<f64> {
    let t = (x - a.x) / (b.x - a.x);
    Coord {
        x,
        y: a.y + t * (b.y - a.y),
    }
}

fn intersect_y(a: &Coord<f64>, b: &Coord<f64>, y: f64) -> Coord<f64> {
    let t = (y - a.y) / (b.y - a.y);
    Coord {
        x: a.x + t * (b.x - a.x),
        y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clip_line_inside() {
        let bbox = BBox::new(-1.0, -1.0, 1.0, 1.0);
        let line = LineString::new(vec![Coord { x: -0.5, y: -0.5 }, Coord { x: 0.5, y: 0.5 }]);
        let result = clip_line(&line, &bbox);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_clip_line_outside() {
        let bbox = BBox::new(-1.0, -1.0, 1.0, 1.0);
        let line = LineString::new(vec![Coord { x: 2.0, y: 2.0 }, Coord { x: 3.0, y: 3.0 }]);
        let result = clip_line(&line, &bbox);
        assert!(result.is_empty());
    }

    #[test]
    fn test_clip_polygon_partial() {
        let bbox = BBox::new(0.0, 0.0, 2.0, 2.0);
        let poly = Polygon::new(
            LineString::new(vec![
                Coord { x: -1.0, y: -1.0 },
                Coord { x: 3.0, y: -1.0 },
                Coord { x: 3.0, y: 3.0 },
                Coord { x: -1.0, y: 3.0 },
                Coord { x: -1.0, y: -1.0 },
            ]),
            vec![],
        );
        let result = clip_polygon(&poly, &bbox);
        assert!(result.is_some());
    }
}
