use geo_types::{LineString, MultiPolygon, Point, Polygon};
use serde::{Deserialize, Serialize};

use crate::bbox::BBox;

/// Unified geometry enum for OSM features.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Geometry {
    Point(Point<f64>),
    Line(LineString<f64>),
    Polygon(Polygon<f64>),
    MultiPolygon(MultiPolygon<f64>),
}

impl Geometry {
    /// Compute the axis-aligned bounding box of this geometry.
    pub fn bbox(&self) -> BBox {
        let mut bb = BBox::empty();
        match self {
            Geometry::Point(p) => {
                bb.expand(p.x(), p.y());
            }
            Geometry::Line(ls) => {
                for c in ls.coords() {
                    bb.expand(c.x, c.y);
                }
            }
            Geometry::Polygon(poly) => {
                for c in poly.exterior().coords() {
                    bb.expand(c.x, c.y);
                }
            }
            Geometry::MultiPolygon(mp) => {
                for poly in mp.iter() {
                    for c in poly.exterior().coords() {
                        bb.expand(c.x, c.y);
                    }
                }
            }
        }
        bb
    }
}
