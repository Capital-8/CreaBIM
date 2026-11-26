//! Trace and reference (MU18, v2 after MU19): something shown faintly
//! under (or over) a drawing to draw over or compare against: another
//! view, a picture (a scanned plan, a photo of a sketch) or a DXF drawing.
//!
//! The reference itself is a project element ([`creabim_core::Reference`]):
//! where it is put, turned and scaled, its colour, strength and the views
//! it shows in are saved with the project. What it shows (a picture's
//! pixels, a drawing's lines, a view's linework) is read here and kept in
//! a cache, as a link is: the file stays where it is.

use creabim_core::reference::{RefSource, Reference, REFERENCE_COLOURS};
use creabim_core::{ElementId, Model};
use creabim_geom::plan::PlanItem;
use creabim_geom::raster::Raster;
use eframe::egui::{self, Color32, Pos2, Stroke};
use glam::DVec2;
use std::collections::HashMap;
use std::sync::Arc;

/// Set each frame: true while a pointer button is down, so drags reuse
/// what the references showed instead of reading their views again.
static HOLD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Did the last DXF read stop at the file's item budget, leaving part of
/// it out? (Told in the status when the reference is added.)
pub static DXF_CAPPED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Hold (or release) the references' read drawings; see [`RefCache::get`].
/// Is a pointer button down (a drag anywhere)?
pub fn held() -> bool {
    HOLD.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn hold(on: bool) {
    HOLD.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// What a reference shows, read: a picture, or lines in its own metres.
#[derive(Clone, Debug)]
pub enum Shown {
    Picture(Arc<Raster>),
    Lines(Arc<Vec<Vec<DVec2>>>),
    /// It couldn't be read; why.
    Missing(String),
}

/// The references' contents, read once per file (or per model revision
/// for a view's linework).
#[derive(Default)]
pub struct RefCache {
    read: HashMap<ElementId, (String, u64, Shown)>,
}

/// The cache key of a source: its file, or its view.
fn key(source: &RefSource) -> String {
    match source {
        RefSource::View(v) => format!("view:{v:?}"),
        RefSource::Image { path, .. } => format!("image:{path}"),
        RefSource::Dxf { path } => format!("dxf:{path}"),
        RefSource::Map { path } => format!("map:{path}"),
    }
}

impl RefCache {
    /// What reference `id` shows, read if not yet (or stale).
    pub fn get(&mut self, model: &Model, id: ElementId, r: &Reference) -> Shown {
        let mut k = key(&r.source);
        // map data follows the project's place on the map
        if let RefSource::Map { .. } = r.source {
            let o = map_offset(model);
            k.push_str(&format!(":{}:{}", o.x, o.y));
        }
        let rev = match r.source {
            RefSource::View(_) => model.revision(),
            _ => 0,
        };
        if let Some((kk, rr, s)) = self.read.get(&id) {
            // while a button is held (a drag), what was read stands: a view's
            // drawing was read again on every frame of a drag, because each
            // frame's move is a change to the model (MU37: references dragged
            // slowly); it is read again once, when the drag ends
            if *kk == k && (*rr == rev || HOLD.load(std::sync::atomic::Ordering::Relaxed)) {
                return s.clone();
            }
        }
        let s = read(model, &r.source);
        self.read.insert(id, (k, rev, s.clone()));
        s
    }

    /// Forget what was read, so it is read again (Reload).
    pub fn forget(&mut self, id: ElementId) {
        self.read.remove(&id);
    }
}

/// Read a source.
pub fn read(model: &Model, source: &RefSource) -> Shown {
    match source {
        RefSource::View(v) => Shown::Lines(Arc::new(
            creabim_geom::section::view_drawing(model, *v)
                .map(|d| linework(&d.items))
                .unwrap_or_default(),
        )),
        RefSource::Image { path, .. } => match picture(path) {
            Ok(r) => Shown::Picture(Arc::new(r)),
            Err(e) => Shown::Missing(e),
        },
        RefSource::Dxf { path } => match dxf_lines(path) {
            Ok(l) => Shown::Lines(Arc::new(l)),
            Err(e) => Shown::Missing(e),
        },
        RefSource::Map { path } => match map_lines(path, map_offset(model)) {
            Ok(l) => Shown::Lines(Arc::new(l)),
            Err(e) => Shown::Missing(e),
        },
    }
}

/// Where the project's origin is on the map; nothing when it isn't placed.
pub fn map_offset(model: &Model) -> DVec2 {
    model
        .project_info()
        .and_then(|(_, p)| p.map.as_ref())
        .map_or(DVec2::ZERO, |m| DVec2::new(m.easting, m.northing))
}

/// A map file's lines, less `offset`: in the project's coordinates when
/// it is the project's place on the map.
pub fn map_lines(path: &str, offset: DVec2) -> Result<Vec<Vec<DVec2>>, String> {
    let d = creabim_exchange::gis::read(std::path::Path::new(path))?;
    let lines: Vec<Vec<DVec2>> = d
        .survey
        .lines
        .iter()
        .map(|l| {
            let mut pts: Vec<DVec2> = l.points.iter().map(|p| p.truncate() - offset).collect();
            if l.closed {
                if let Some(f) = pts.first().copied() {
                    pts.push(f);
                }
            }
            pts
        })
        .collect();
    if lines.is_empty() {
        return Err(format!("{path} holds no lines to show"));
    }
    Ok(lines)
}

/// What a source spans in its own coordinates, once read.
pub fn bounds_of(source: &RefSource, shown: &Shown) -> Option<[DVec2; 2]> {
    match (source, shown) {
        (RefSource::Image { width, .. }, Shown::Picture(r)) => {
            let h = width * r.height as f64 / r.width.max(1) as f64;
            Some([DVec2::ZERO, DVec2::new(*width, h)])
        }
        (_, Shown::Lines(l)) => {
            let mut it = l.iter().flatten();
            let first = *it.next()?;
            let (lo, hi) = it.fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
            Some([lo, hi])
        }
        _ => None,
    }
}

/// A reference's name: the view's, or the file's.
pub fn label(model: &Model, source: &RefSource) -> String {
    let file = |p: &str| {
        std::path::Path::new(p)
            .file_name()
            .map_or(p.to_string(), |n| n.to_string_lossy().into_owned())
    };
    match source {
        RefSource::View(v) => model.get(*v).map_or("a view".into(), |e| e.name.clone()),
        RefSource::Image { path, .. } | RefSource::Dxf { path } | RefSource::Map { path } => {
            file(path)
        }
    }
}

/// The ink a reference is drawn in; none keeps its own colours.
pub fn colour(r: &Reference) -> Option<Color32> {
    r.colour.map(|i| {
        let [cr, cg, cb] = REFERENCE_COLOURS[(i as usize).min(REFERENCE_COLOURS.len() - 1)].1;
        Color32::from_rgb(cr, cg, cb)
    })
}

/// Draw the references shown in `view` that go under the model (or, with
/// `over`, over it); `to_screen` places a model point.
pub fn draw_view(
    painter: &egui::Painter,
    model: &Model,
    cache: &mut RefCache,
    view: ElementId,
    over: bool,
    to_screen: &dyn Fn(DVec2) -> Pos2,
) {
    for (id, r) in creabim_core::reference::in_view(model, view) {
        if r.hidden || r.over != over || r.opacity <= 0.0 {
            continue;
        }
        let shown = cache.get(model, id, r);
        draw(painter, r, &shown, to_screen);
    }
}

/// Draw one reference.
pub fn draw(
    painter: &egui::Painter,
    r: &Reference,
    shown: &Shown,
    to_screen: &dyn Fn(DVec2) -> Pos2,
) {
    let a = r.opacity.clamp(0.0, 1.0);
    match shown {
        Shown::Picture(raster) => {
            let Some([lo, hi]) = r.bounds.or_else(|| bounds_of(&r.source, shown)) else {
                return;
            };
            let corners = [lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)]
                .map(|c| to_screen(r.place(c)));
            let id = crate::textures::texture(painter.ctx(), raster);
            // its own colours, or tinted with the reference colour
            let tint = colour(r).unwrap_or(Color32::WHITE).gamma_multiply(a);
            let mut mesh = egui::Mesh::with_texture(id);
            let uv = [
                egui::pos2(0.0, 1.0),
                egui::pos2(1.0, 1.0),
                egui::pos2(1.0, 0.0),
                egui::pos2(0.0, 0.0),
            ];
            for (c, t) in corners.iter().zip(uv) {
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: *c,
                    uv: t,
                    color: tint,
                });
            }
            mesh.indices.extend([0, 1, 2, 0, 2, 3]);
            painter.add(egui::Shape::mesh(mesh));
        }
        Shown::Lines(lines) => {
            let ink = colour(r)
                .unwrap_or(Color32::from_gray(70))
                .gamma_multiply(a);
            let stroke = Stroke::new(1.0, ink);
            for line in lines.iter() {
                let pts: Vec<Pos2> = line.iter().map(|p| to_screen(r.place(*p))).collect();
                if pts.len() >= 2 {
                    painter.add(egui::Shape::line(pts, stroke));
                }
            }
        }
        Shown::Missing(_) => {}
    }
}

/// A drawing's linework as polylines: lines, outlines and arcs, no text
/// or fills.
pub fn linework(items: &[PlanItem]) -> Vec<Vec<DVec2>> {
    let mut out = Vec::new();
    for it in items {
        match it {
            PlanItem::Line { a, b, .. } => out.push(vec![*a, *b]),
            PlanItem::Polygon { points, .. } | PlanItem::Fill { points, .. } => {
                if let Some(first) = points.first() {
                    let mut p = points.clone();
                    p.push(*first);
                    out.push(p);
                }
            }
            PlanItem::Arc {
                center,
                radius,
                start,
                end,
                ..
            } => {
                let sweep = end - start;
                let n = ((sweep.abs() / 0.15).ceil() as usize).clamp(4, 64);
                out.push(
                    (0..=n)
                        .map(|i| {
                            let a = start + sweep * i as f64 / n as f64;
                            *center + DVec2::from_angle(a) * *radius
                        })
                        .collect(),
                );
            }
            _ => {}
        }
    }
    out
}

/// A picture file's pixels.
pub fn picture(path: &str) -> Result<Raster, String> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".tif") || lower.ends_with(".tiff") {
        let b = std::fs::read(path).map_err(|e| format!("couldn't read {path}: {e}"))?;
        let m = creabim_exchange::gis::map_picture(std::io::Cursor::new(b))?;
        return Ok(Raster {
            width: m.width,
            height: m.height,
            rgba: m.rgba,
            key: path_key(path),
        });
    }
    let img = image::open(path).map_err(|e| format!("couldn't read {path}: {e}"))?;
    let rgba = img.to_rgba8();
    Ok(Raster {
        width: rgba.width() as usize,
        height: rgba.height() as usize,
        rgba: rgba.into_raw(),
        key: path_key(path),
    })
}

/// A DXF file's lines, in metres.
pub fn dxf_lines(path: &str) -> Result<Vec<Vec<DVec2>>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("couldn't read {path}: {e}"))?;
    let text = creabim_exchange::titleblock::dxf_text(&bytes);
    let items = creabim_exchange::titleblock::dxf_items(&text)?;
    DXF_CAPPED.store(
        creabim_exchange::titleblock::dxf_capped(&items),
        std::sync::atomic::Ordering::Relaxed,
    );
    Ok(items
        .into_iter()
        .filter_map(|it| match it {
            creabim_core::titleblock::TitleItem::Line { points, .. } => {
                Some(points.into_iter().map(|p| p / 1000.0).collect())
            }
            _ => None,
        })
        .collect())
}

fn path_key(path: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    h.finish() | 1 << 63
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_lines_come_in_less_the_project_place_and_rings_close() {
        let dir = std::env::temp_dir().join(format!("creabim-map-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lots.geojson");
        std::fs::write(
            &path,
            r#"{"type":"FeatureCollection","crs":{"type":"name","properties":{"name":"EPSG:32613"}},"features":[
              {"type":"Feature","properties":{"kind":"property"},"geometry":{"type":"Polygon","coordinates":[[[512350,4362130],[512370,4362130],[512370,4362150],[512350,4362150],[512350,4362130]]]}},
              {"type":"Feature","properties":{"kind":"kerb"},"geometry":{"type":"LineString","coordinates":[[512340,4362120],[512380,4362120]]}}]}"#,
        )
        .unwrap();
        let l = map_lines(path.to_str().unwrap(), DVec2::new(512360.0, 4362138.0)).unwrap();
        assert_eq!(l.len(), 2);
        let ring = l.iter().find(|r| r.len() >= 5).expect("the lot");
        assert_eq!(ring.first(), ring.last(), "a closed lot is drawn closed");
        assert!(ring
            .iter()
            .all(|p| p.x.abs() <= 10.0 + 1e-6 && (-8.0 - 1e-6..=12.0 + 1e-6).contains(&p.y)));
        let kerb = l.iter().find(|r| r.len() == 2).unwrap();
        assert!((kerb[0] - DVec2::new(-20.0, -18.0)).length() < 1e-6);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_view_reference_reads_its_linework_and_spans_it() {
        let m = creabim_core::sample::sample_house();
        let v = m
            .iter()
            .find(|e| matches!(&e.kind, creabim_core::ElementKind::View(v) if matches!(v.kind, creabim_core::ViewKind::Plan { .. })))
            .unwrap()
            .id;
        let src = RefSource::View(v);
        let shown = read(&m, &src);
        let Shown::Lines(l) = &shown else { panic!() };
        assert!(!l.is_empty(), "a plan has linework");
        let [lo, hi] = bounds_of(&src, &shown).unwrap();
        assert!(hi.x > lo.x && hi.y > lo.y);
    }

    #[test]
    fn a_dxf_reference_reads_its_lines_in_metres() {
        let dir = std::env::temp_dir().join("creabim-trace-test.dxf");
        let text = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n5000\n21\n0\n0\nENDSEC\n0\nEOF\n";
        std::fs::write(&dir, text).unwrap();
        let lines = dxf_lines(&dir.to_string_lossy()).unwrap();
        assert_eq!(lines.len(), 1);
        assert!((lines[0][1].x - 5.0).abs() < 1e-9, "{:?}", lines[0]);
    }
}
