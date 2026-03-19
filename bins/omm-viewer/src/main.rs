use std::path::PathBuf;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use clap::Parser;
use cosmic_text::{Attrs, Buffer as TextBuffer, Family, FontSystem, Metrics, Shaping, SwashCache};
use lyon::math::point;
use lyon::path::Path as LyonPath;
use lyon::tessellation::*;
use tracing::info;
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use omm_core::bbox::BBox;
use omm_core::Color;
use omm_geo::projection::bbox_to_tile_range;
use omm_tiles::mvt_decode::{self, DecodedFeature};

#[derive(Parser)]
#[command(name = "omm-viewer", about = "Interactive map viewer")]
struct Args {
    /// PMTiles file to view
    pmtiles_file: PathBuf,

    /// Initial center longitude
    #[arg(long, default_value = "-98.5", allow_hyphen_values = true)]
    lon: f64,

    /// Initial center latitude
    #[arg(long, default_value = "39.8")]
    lat: f64,

    /// Initial zoom level
    #[arg(long, default_value = "10")]
    zoom: f64,
}

// --- Vertex format ---

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
}

impl Vertex {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: 8,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    };
}

// --- Camera ---

struct Camera {
    center_lon: f64,
    center_lat: f64,
    zoom: f64, // degrees of longitude visible across the window width
}

impl Camera {
    fn new(lon: f64, lat: f64, zoom: f64) -> Self {
        Self {
            center_lon: lon,
            center_lat: lat,
            zoom,
        }
    }

    fn view_bbox(&self, aspect: f64) -> BBox {
        let half_w = self.zoom / 2.0;
        let half_h = half_w / aspect;
        BBox::new(
            self.center_lon - half_w,
            self.center_lat - half_h,
            self.center_lon + half_w,
            self.center_lat + half_h,
        )
    }

    fn projection(&self, aspect: f64) -> [[f32; 4]; 4] {
        let bb = self.view_bbox(aspect);
        orthographic(
            bb.min_lon as f32,
            bb.max_lon as f32,
            bb.min_lat as f32,
            bb.max_lat as f32,
        )
    }

    fn tile_zoom(&self) -> u8 {
        let z = (360.0 / self.zoom).log2();
        (z as u8).clamp(0, 14)
    }
}

fn orthographic(left: f32, right: f32, bottom: f32, top: f32) -> [[f32; 4]; 4] {
    let w = right - left;
    let h = top - bottom;
    [
        [2.0 / w, 0.0, 0.0, 0.0],
        [0.0, 2.0 / h, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [-(right + left) / w, -(top + bottom) / h, 0.0, 1.0],
    ]
}

// --- Tessellation ---

fn tessellate_features(features: &[DecodedFeature]) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Sort by layer z-order
    let mut sorted: Vec<&DecodedFeature> = features.iter().collect();
    sorted.sort_by_key(|f| match f.layer.as_str() {
        "landuse" => 0,
        "natural" => 1,
        "leisure" => 2,
        "water" => 3,
        "building" => 4,
        "boundary" => 5,
        "railway" => 6,
        "highway" => 7,
        _ => 8,
    });

    for feature in &sorted {
        let is_area = matches!(
            feature.layer.as_str(),
            "building" | "landuse" | "natural" | "leisure"
        ) || (feature.layer == "water"
            && feature
                .class
                .as_deref()
                .is_some_and(|c| matches!(c, "lake" | "pond" | "reservoir" | "basin")));

        let is_line = matches!(
            feature.layer.as_str(),
            "highway" | "railway" | "boundary"
        ) || (feature.layer == "water"
            && feature
                .class
                .as_deref()
                .is_some_and(|c| matches!(c, "river" | "stream" | "canal")));

        if is_area {
            if let Some(color) = area_color(&feature.layer, feature.class.as_deref()) {
                tessellate_fill(&feature.geometry, &color, &mut vertices, &mut indices);
            }
        }
        if is_line {
            if let Some((color, width)) = line_style(&feature.layer, feature.class.as_deref()) {
                tessellate_stroke(&feature.geometry, &color, width, &mut vertices, &mut indices);
            }
        }
    }

    (vertices, indices)
}

fn tessellate_fill(
    geom: &omm_core::Geometry,
    color: &Color,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
) {
    let rgba = [color.r, color.g, color.b, color.a];

    let coords = match geom {
        omm_core::Geometry::Polygon(poly) => {
            let mut rings = vec![];
            rings.push(poly.exterior().coords().collect::<Vec<_>>());
            for hole in poly.interiors() {
                rings.push(hole.coords().collect::<Vec<_>>());
            }
            rings
        }
        _ => return,
    };

    let mut builder = LyonPath::builder();
    for ring in &coords {
        if ring.len() < 3 {
            continue;
        }
        builder.begin(point(ring[0].x as f32, ring[0].y as f32));
        for c in &ring[1..] {
            builder.line_to(point(c.x as f32, c.y as f32));
        }
        builder.end(true);
    }
    let path = builder.build();

    let base = vertices.len() as u32;
    let mut geometry: VertexBuffers<Vertex, u32> = VertexBuffers::new();
    let mut tessellator = FillTessellator::new();
    let result = tessellator.tessellate_path(
        &path,
        &FillOptions::tolerance(0.0001),
        &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| Vertex {
            position: vertex.position().to_array(),
            color: rgba,
        }),
    );
    if result.is_ok() {
        vertices.extend_from_slice(&geometry.vertices);
        indices.extend(geometry.indices.iter().map(|i| i + base));
    }
}

fn tessellate_stroke(
    geom: &omm_core::Geometry,
    color: &Color,
    width: f32,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
) {
    let rgba = [color.r, color.g, color.b, color.a];

    let line_coords = match geom {
        omm_core::Geometry::Line(ls) => ls.coords().collect::<Vec<_>>(),
        _ => return,
    };
    if line_coords.len() < 2 {
        return;
    }

    let mut builder = LyonPath::builder();
    builder.begin(point(line_coords[0].x as f32, line_coords[0].y as f32));
    for c in &line_coords[1..] {
        builder.line_to(point(c.x as f32, c.y as f32));
    }
    builder.end(false);
    let path = builder.build();

    let base = vertices.len() as u32;
    let mut geometry: VertexBuffers<Vertex, u32> = VertexBuffers::new();
    let mut tessellator = StrokeTessellator::new();
    let result = tessellator.tessellate_path(
        &path,
        &StrokeOptions::tolerance(0.0001)
            .with_line_width(width)
            .with_line_cap(lyon::tessellation::LineCap::Round)
            .with_line_join(lyon::tessellation::LineJoin::Round),
        &mut BuffersBuilder::new(&mut geometry, |vertex: StrokeVertex| Vertex {
            position: vertex.position().to_array(),
            color: rgba,
        }),
    );
    if result.is_ok() {
        vertices.extend_from_slice(&geometry.vertices);
        indices.extend(geometry.indices.iter().map(|i| i + base));
    }
}

fn area_color(layer: &str, class: Option<&str>) -> Option<Color> {
    match layer {
        "building" => Color::from_hex("#dfdbd7"),
        "landuse" => match class.unwrap_or("") {
            "forest" => Color::from_hex("#add19e"),
            "grass" | "meadow" => Color::from_hex("#cdebb0"),
            "farmland" => Color::from_hex("#d5e29e"),
            "residential" => Color::from_hex("#e0d6d0"),
            _ => Color::from_hex("#d5cfc8"),
        },
        "natural" => match class.unwrap_or("") {
            "wood" => Color::from_hex("#add19e"),
            "water" => Color::from_hex("#aad3df"),
            "grassland" => Color::from_hex("#cdebb0"),
            _ => None,
        },
        "leisure" => Color::from_hex("#c8facc"),
        "water" => Color::from_hex("#aad3df"),
        _ => None,
    }
}

fn line_style(layer: &str, class: Option<&str>) -> Option<(Color, f32)> {
    match layer {
        "highway" => {
            let (c, w) = match class.unwrap_or("") {
                "motorway" | "motorway_link" => ("#e892a2", 0.002),
                "trunk" | "trunk_link" => ("#f9b29c", 0.0015),
                "primary" | "primary_link" => ("#fcd6a4", 0.001),
                "secondary" | "secondary_link" => ("#f7fabf", 0.0008),
                "tertiary" | "tertiary_link" => ("#ffffff", 0.0006),
                "residential" => ("#ffffff", 0.0004),
                _ => ("#cccccc", 0.0003),
            };
            Some((Color::from_hex(c).unwrap(), w))
        }
        "railway" => Some((Color::from_hex("#bfbfbf").unwrap(), 0.0005)),
        "boundary" => Some((Color::from_hex("#9e9cab").unwrap(), 0.0005)),
        "water" => Some((Color::from_hex("#aad3df").unwrap(), 0.0008)),
        _ => None,
    }
}

// --- Tile loading ---

fn load_tiles_blocking(
    path: &std::path::Path,
    bbox: &BBox,
    zoom: u8,
) -> Vec<DecodedFeature> {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let backend = pmtiles::MmapBackend::try_from(path).await.unwrap();
        let reader: pmtiles::AsyncPmTilesReader<pmtiles::MmapBackend> =
            pmtiles::AsyncPmTilesReader::try_from_source(backend)
                .await
                .unwrap();

        let (min_x, min_y, max_x, max_y) = bbox_to_tile_range(bbox, zoom);
        let n = (1u64 << zoom) as f64;
        let mut features = Vec::new();

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                if let Ok(coord) = pmtiles::TileCoord::new(zoom, x, y) {
                    if let Ok(Some(data)) = reader.get_tile_decompressed(coord).await {
                        features.extend(mvt_decode::decode_tile(&data, zoom, x, y, n));
                    }
                }
            }
        }
        features
    })
}

// --- Labels ---

struct MapLabel {
    lon: f64,
    lat: f64,
    text: String,
    font_size: f32,
    color: [u8; 4],
    /// Maximum camera zoom (degrees visible) at which this label appears.
    max_view_deg: f64,
    /// Feature bounding box for re-centering when label scrolls off-screen.
    feature_bbox: BBox,
}

fn collect_labels(features: &[DecodedFeature]) -> Vec<MapLabel> {
    let mut labels = Vec::new();
    for f in features {
        let name = match &f.name {
            Some(n) if !n.is_empty() => n,
            _ => continue,
        };

        // (font_size, color, min_zoom_degrees) - min_zoom_degrees = max degrees visible for label to appear
        let (font_size, color, max_view_deg) = match f.layer.as_str() {
            "highway" => match f.class.as_deref().unwrap_or("") {
                "motorway" | "trunk" => (12.0, [0x55, 0x55, 0x55, 0xFF], 2.0),
                "primary" => (11.0, [0x55, 0x55, 0x55, 0xFF], 0.5),
                "secondary" | "tertiary" => (10.0, [0x66, 0x66, 0x66, 0xFF], 0.2),
                "residential" | "unclassified" => (9.0, [0x77, 0x77, 0x77, 0xFF], 0.05),
                _ => (9.0, [0x77, 0x77, 0x77, 0xFF], 0.03),
            },
            "water" => (11.0, [0x6b, 0x9d, 0xaf, 0xFF], 1.0),
            "place" => match f.class.as_deref().unwrap_or("") {
                "city" => (20.0, [0x33, 0x33, 0x33, 0xFF], 50.0),
                "town" => (15.0, [0x44, 0x44, 0x44, 0xFF], 5.0),
                "village" => (12.0, [0x55, 0x55, 0x55, 0xFF], 1.0),
                _ => (10.0, [0x66, 0x66, 0x66, 0xFF], 0.5),
            },
            "leisure" => (10.0, [0x3a, 0x7a, 0x3a, 0xFF], 0.3),
            _ => continue,
        };

        let bb = f.geometry.bbox();
        let center = bb.center();

        labels.push(MapLabel {
            lon: center.lon,
            lat: center.lat,
            text: name.clone(),
            font_size,
            color,
            max_view_deg,
            feature_bbox: bb,
        });
    }
    labels
}

fn render_labels_to_rgba(
    labels: &[MapLabel],
    camera: &Camera,
    width: u32,
    height: u32,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
) -> Vec<u8> {
    let aspect = width as f64 / height as f64;
    let bb = camera.view_bbox(aspect);
    let w = width as f64;
    let h = height as f64;

    let mut pixels = vec![0u8; (width * height * 4) as usize];

    // Grid-based collision detection: divide screen into cells
    let cell_w = 120.0f32; // pixels per cell
    let cell_h = 30.0f32;
    let grid_cols = (width as f32 / cell_w).ceil() as usize + 1;
    let grid_rows = (height as f32 / cell_h).ceil() as usize + 1;
    let mut occupied = vec![false; grid_cols * grid_rows];

    // Sort labels by priority (larger font = higher priority)
    let mut sorted_indices: Vec<usize> = (0..labels.len()).collect();
    sorted_indices.sort_by(|&a, &b| {
        labels[b]
            .font_size
            .partial_cmp(&labels[a].font_size)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut placed = 0;
    let max_labels = 200; // cap for performance

    for &idx in &sorted_indices {
        if placed >= max_labels {
            break;
        }
        let label = &labels[idx];

        // Skip labels not visible at current zoom
        if camera.zoom > label.max_view_deg {
            continue;
        }

        // Convert lon/lat to pixel
        let mut px = ((label.lon - bb.min_lon) / bb.width() * w) as f32;
        let mut py = ((bb.max_lat - label.lat) / bb.height() * h) as f32;

        // If label center is off-screen but feature is still visible, re-center
        let margin = 50.0f32;
        let on_screen = px >= -margin
            && py >= -margin
            && px <= w as f32 + margin
            && py <= h as f32 + margin;

        if !on_screen {
            // Check if the feature bbox overlaps the viewport
            if !label.feature_bbox.intersects(&bb) {
                continue;
            }
            // Clamp the label to the visible portion of the feature
            let vis_lon = label
                .lon
                .clamp(bb.min_lon + bb.width() * 0.1, bb.max_lon - bb.width() * 0.1);
            let vis_lat = label
                .lat
                .clamp(bb.min_lat + bb.height() * 0.1, bb.max_lat - bb.height() * 0.1);
            px = ((vis_lon - bb.min_lon) / bb.width() * w) as f32;
            py = ((bb.max_lat - vis_lat) / bb.height() * h) as f32;
        }

        // Estimate text bounds for collision
        let est_w = label.text.len() as f32 * label.font_size * 0.55;
        let est_h = label.font_size * 1.3;
        let x0 = px - est_w / 2.0;
        let y0 = py - est_h / 2.0;

        // Check collision grid
        let col_min = ((x0 / cell_w).floor() as isize).max(0) as usize;
        let col_max = (((x0 + est_w) / cell_w).ceil() as usize).min(grid_cols - 1);
        let row_min = ((y0 / cell_h).floor() as isize).max(0) as usize;
        let row_max = (((y0 + est_h) / cell_h).ceil() as usize).min(grid_rows - 1);

        let mut collides = false;
        for row in row_min..=row_max {
            for col in col_min..=col_max {
                if occupied[row * grid_cols + col] {
                    collides = true;
                    break;
                }
            }
            if collides {
                break;
            }
        }
        if collides {
            continue;
        }

        // Mark cells as occupied
        for row in row_min..=row_max {
            for col in col_min..=col_max {
                occupied[row * grid_cols + col] = true;
            }
        }

        // Shape and render this label
        let metrics = Metrics::new(label.font_size, label.font_size * 1.2);
        let mut buffer = TextBuffer::new(font_system, metrics);
        let attrs = Attrs::new().family(Family::SansSerif);
        buffer.set_text(font_system, &label.text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(font_system, false);

        let mut text_w: f32 = 0.0;
        for run in buffer.layout_runs() {
            text_w = text_w.max(run.line_w);
        }
        let offset_x = px - text_w / 2.0;
        let offset_y = py - label.font_size / 2.0;

        // Halo
        let halo = cosmic_text::Color::rgba(255, 255, 255, 220);
        for &dy in &[-1.0f32, 0.0, 1.0] {
            for &dx in &[-1.0f32, 0.0, 1.0] {
                if dx == 0.0 && dy == 0.0 {
                    continue;
                }
                draw_text_to_buf(
                    &buffer, font_system, swash_cache, &mut pixels, width, height,
                    offset_x + dx, offset_y + dy, halo,
                );
            }
        }

        // Text
        let [r, g, b, a] = label.color;
        draw_text_to_buf(
            &buffer, font_system, swash_cache, &mut pixels, width, height,
            offset_x, offset_y, cosmic_text::Color::rgba(r, g, b, a),
        );

        placed += 1;
    }

    pixels
}

fn draw_text_to_buf(
    buffer: &TextBuffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    pixels: &mut [u8],
    buf_w: u32,
    buf_h: u32,
    offset_x: f32,
    offset_y: f32,
    color: cosmic_text::Color,
) {
    buffer.draw(font_system, swash_cache, color, |x, y, w, h, c| {
        let alpha = c.a();
        if alpha == 0 {
            return;
        }
        for dy in 0..h as i32 {
            for dx in 0..w as i32 {
                let px = (x + dx) + offset_x as i32;
                let py = (y + dy) + offset_y as i32;
                if px >= 0 && py >= 0 && (px as u32) < buf_w && (py as u32) < buf_h {
                    let idx = ((py as u32 * buf_w + px as u32) * 4) as usize;
                    let sa = alpha as f32 / 255.0;
                    pixels[idx] =
                        ((c.r() as f32 * sa + pixels[idx] as f32 * (1.0 - sa)).min(255.0)) as u8;
                    pixels[idx + 1] = ((c.g() as f32 * sa
                        + pixels[idx + 1] as f32 * (1.0 - sa))
                        .min(255.0)) as u8;
                    pixels[idx + 2] = ((c.b() as f32 * sa
                        + pixels[idx + 2] as f32 * (1.0 - sa))
                        .min(255.0)) as u8;
                    pixels[idx + 3] =
                        ((sa + pixels[idx + 3] as f32 / 255.0 * (1.0 - sa)) * 255.0).min(255.0)
                            as u8;
                }
            }
        }
    });
}

// --- App ---

struct MapApp {
    pmtiles_path: PathBuf,
    camera: Camera,
    font_system: FontSystem,
    swash_cache: SwashCache,
    labels: Vec<MapLabel>,
    // wgpu state (initialized on resume)
    gpu: Option<GpuState>,
    // input state
    dragging: bool,
    last_cursor: PhysicalPosition<f64>,
    needs_retessellate: bool,
    current_tile_zoom: u8,
}

struct GpuState {
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    // Label overlay
    label_pipeline: wgpu::RenderPipeline,
    label_texture: wgpu::Texture,
    label_bind_group: wgpu::BindGroup,
    label_uniform_buffer: wgpu::Buffer,
    /// Camera lon/lat when labels were last rendered
    label_camera_lon: f64,
    label_camera_lat: f64,
    label_camera_zoom: f64,
}

impl MapApp {
    fn new(args: &Args) -> Self {
        let degrees = 360.0 / 2.0f64.powf(args.zoom);
        Self {
            pmtiles_path: args.pmtiles_file.clone(),
            camera: Camera::new(args.lon, args.lat, degrees),
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            labels: Vec::new(),
            gpu: None,
            dragging: false,
            last_cursor: PhysicalPosition::new(0.0, 0.0),
            needs_retessellate: true,
            current_tile_zoom: 0,
        }
    }

    fn load_and_tessellate(&mut self) {
        let gpu = self.gpu.as_ref().unwrap();
        let aspect = gpu.config.width as f64 / gpu.config.height as f64;
        let bbox = self.camera.view_bbox(aspect);
        let tile_zoom = self.camera.tile_zoom();

        info!(zoom = tile_zoom, bbox = %bbox, "Loading tiles");
        let features = load_tiles_blocking(&self.pmtiles_path, &bbox, tile_zoom);
        info!(features = features.len(), "Tessellating");

        self.labels = collect_labels(&features);
        info!(labels = self.labels.len(), "Labels collected");

        let (vertices, indices) = tessellate_features(&features);
        info!(
            vertices = vertices.len(),
            indices = indices.len(),
            "GPU buffers ready"
        );

        // Recreate GPU buffers
        let gpu = self.gpu.as_mut().unwrap();

        if vertices.is_empty() {
            gpu.num_indices = 0;
            return;
        }

        gpu.vertex_buffer =
            gpu.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
        gpu.index_buffer =
            gpu.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Index Buffer"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
        gpu.num_indices = indices.len() as u32;
        self.current_tile_zoom = tile_zoom;
        self.needs_retessellate = false;
        self.update_label_texture();
    }

    fn update_label_texture(&mut self) {
        let (w, h) = match &self.gpu {
            Some(g) => (g.config.width, g.config.height),
            None => return,
        };

        let label_pixels = render_labels_to_rgba(
            &self.labels,
            &self.camera,
            w,
            h,
            &mut self.font_system,
            &mut self.swash_cache,
        );

        let gpu = self.gpu.as_mut().unwrap();
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &gpu.label_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &label_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        // Store camera state for pan tracking
        gpu.label_camera_lon = self.camera.center_lon;
        gpu.label_camera_lat = self.camera.center_lat;
        gpu.label_camera_zoom = self.camera.zoom;

        // Reset offset
        gpu.queue.write_buffer(
            &gpu.label_uniform_buffer,
            0,
            bytemuck::cast_slice(&[0.0f32, 0.0f32]),
        );
    }

    fn update_label_offset(&self) {
        if let Some(gpu) = &self.gpu {
            // Compute UV offset: how much camera moved since labels were rendered
            let dx_deg = self.camera.center_lon - gpu.label_camera_lon;
            let dy_deg = self.camera.center_lat - gpu.label_camera_lat;
            let zoom = gpu.label_camera_zoom;
            if zoom > 0.0 {
                let aspect = gpu.config.width as f64 / gpu.config.height as f64;
                let u_offset = (dx_deg / zoom) as f32;
                let v_offset = -(dy_deg / (zoom / aspect)) as f32;
                gpu.queue.write_buffer(
                    &gpu.label_uniform_buffer,
                    0,
                    bytemuck::cast_slice(&[u_offset, v_offset]),
                );
            }
        }
    }

    fn update_projection(&self) {
        if let Some(gpu) = &self.gpu {
            let aspect = gpu.config.width as f64 / gpu.config.height as f64;
            let proj = self.camera.projection(aspect);
            gpu.queue
                .write_buffer(&gpu.uniform_buffer, 0, bytemuck::cast_slice(&proj));
        }
    }

    fn render(&self) {
        let gpu = match &self.gpu {
            Some(g) => g,
            None => return,
        };

        let frame = match gpu.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Map Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.973,
                            g: 0.957,
                            b: 0.941,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&gpu.pipeline);
            pass.set_bind_group(0, &gpu.uniform_bind_group, &[]);
            if gpu.num_indices > 0 {
                pass.set_vertex_buffer(0, gpu.vertex_buffer.slice(..));
                pass.set_index_buffer(gpu.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..gpu.num_indices, 0, 0..1);
            }

            // Draw label overlay
            pass.set_pipeline(&gpu.label_pipeline);
            pass.set_bind_group(0, &gpu.label_bind_group, &[]);
            pass.draw(0..4, 0..1);
        }
        gpu.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}

impl ApplicationHandler for MapApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_title("OpenMapMarketor Viewer")
                        .with_inner_size(LogicalSize::new(1280, 960)),
                )
                .unwrap(),
        );

        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .unwrap();

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Map Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            },
        ))
        .unwrap();

        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .unwrap();
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Map Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[[0.0f32; 4]; 4]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Uniform BGL"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform BG"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Map Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Map Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::LAYOUT],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // Placeholder buffers
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: &[0u8; 24],
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: &[0u8; 4],
            usage: wgpu::BufferUsages::INDEX,
        });

        // --- Label overlay pipeline ---
        let label_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Label Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("label_shader.wgsl").into()),
        });

        let label_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Label Texture"),
            size: wgpu::Extent3d {
                width: size.width.max(1),
                height: size.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let label_texture_view = label_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let label_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let label_uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Label Uniform Buffer"),
                contents: bytemuck::cast_slice(&[0.0f32, 0.0f32]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let label_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Label BGL"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let label_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Label BG"),
            layout: &label_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&label_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&label_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: label_uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let label_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Label Pipeline Layout"),
                bind_group_layouts: &[&label_bind_group_layout],
                immediate_size: 0,
            });

        let label_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Label Pipeline"),
            layout: Some(&label_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &label_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &label_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        self.gpu = Some(GpuState {
            window,
            device,
            queue,
            surface,
            config,
            pipeline,
            uniform_buffer,
            uniform_bind_group,
            vertex_buffer,
            index_buffer,
            num_indices: 0,
            label_pipeline,
            label_texture,
            label_bind_group,
            label_uniform_buffer,
            label_camera_lon: 0.0,
            label_camera_lat: 0.0,
            label_camera_zoom: 1.0,
        });

        self.load_and_tessellate();
        self.update_projection();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.config.width = size.width.max(1);
                    gpu.config.height = size.height.max(1);
                    gpu.surface.configure(&gpu.device, &gpu.config);
                }
                self.update_projection();
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left {
                    let was_dragging = self.dragging;
                    self.dragging = state == ElementState::Pressed;
                    // Re-render labels when drag ends
                    if was_dragging && !self.dragging {
                        self.update_label_texture();
                        if let Some(gpu) = &self.gpu {
                            gpu.window.request_redraw();
                        }
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                if self.dragging {
                    let (w, h) = self
                        .gpu
                        .as_ref()
                        .map(|g| (g.config.width as f64, g.config.height as f64))
                        .unwrap_or((1.0, 1.0));

                    let dx = position.x - self.last_cursor.x;
                    let dy = position.y - self.last_cursor.y;

                    self.camera.center_lon -= dx / w * self.camera.zoom;
                    self.camera.center_lat += dy / h * self.camera.zoom * (h / w);

                    self.update_projection();
                    self.update_label_offset();

                    let new_tz = self.camera.tile_zoom();
                    if new_tz != self.current_tile_zoom {
                        self.needs_retessellate = true;
                    }

                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                }
                self.last_cursor = position;
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(p) => p.y / 50.0,
                };
                let factor = 1.0 - scroll * 0.1;
                self.camera.zoom = (self.camera.zoom * factor).clamp(0.001, 360.0);
                self.update_projection();

                let new_tz = self.camera.tile_zoom();
                if new_tz != self.current_tile_zoom {
                    self.needs_retessellate = true;
                }

                // Re-render labels at new zoom
                self.update_label_texture();

                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                if self.needs_retessellate {
                    self.load_and_tessellate();
                    self.update_projection();
                }
                self.render();
            }

            _ => {}
        }
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,wgpu=warn")),
        )
        .init();

    let args = Args::parse();
    println!("=== OpenMapMarketor Viewer ===");
    println!("File: {}", args.pmtiles_file.display());
    println!("Controls: drag=pan, scroll=zoom");

    let event_loop = EventLoop::new().unwrap();
    let mut app = MapApp::new(&args);
    event_loop.run_app(&mut app).unwrap();
}
