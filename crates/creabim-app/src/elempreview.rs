//! The Preview section of Properties (Mockup_Preview_And_Louvres_REV00,
//! approved 2026-11-21, after Blender's material Preview): a picture of
//! what is selected, drawn from it, with a column of ways to look beside
//! it (plan, elevation, section, 3D; for a material a sphere, a cube or a
//! flat swatch), folding as any section does, and a grip under it to drag
//! it taller. One component for every element, type and material.

use super::*;
use creabim_core::types::TypeCategory;
use creabim_geom::opening::{Part, Shaped};
use eframe::egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};
use glam::{DVec2, DVec3};

/// A coloured triangle, in model coordinates.
type Tri = ([DVec3; 3], [f32; 3]);

/// The ways to look, and their buttons' letters and tips.
const ELEMENT_VIEWS: &[(&str, &str)] = &[
    ("P", "Plan: seen from above"),
    ("E", "Elevation: seen from the front"),
    ("S", "Section: cut through its middle"),
    ("3D", "3D: seen from above and to the side"),
];
const MATERIAL_VIEWS: &[(&str, &str)] = &[
    ("●", "A sphere, as the 3D view lights it"),
    ("■", "A cube"),
    ("▬", "A flat swatch"),
];

/// Where [`preview_frame`] put the picture and its view buttons.
#[derive(Debug)]
pub(super) struct Framed {
    #[allow(dead_code)]
    pub stage: Rect,
    #[allow(dead_code)]
    pub buttons: Vec<Rect>,
}

/// The frame of the Preview: the picture across the panel's whole width,
/// its view buttons in a row under it and the grip under them (MU142: the
/// buttons down its right edge took a strip off the picture). `key` keeps
/// each kind of element's view and height apart. `draw` paints the
/// picture for the view picked.
pub(super) fn preview_frame(
    ui: &mut egui::Ui,
    key: &str,
    views: &[(&str, &str)],
    draw: impl FnOnce(&egui::Painter, Rect, usize),
) -> Framed {
    let vid = egui::Id::new(("preview.view", key));
    let hid = egui::Id::new(("preview.height", key));
    let mut view: usize = ui
        .data_mut(|d| d.get_persisted(vid))
        .unwrap_or(if views.len() > 1 { 1 } else { 0 });
    let mut h: f32 = ui.data_mut(|d| d.get_persisted(hid)).unwrap_or(170.0);
    view = view.min(views.len().saturating_sub(1));
    let v = ui.visuals().clone();
    let w = ui.available_width();
    let (stage, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let p = ui.painter_at(stage);
    // a light checker, as Blender's, so pale and see-through things read
    let (a, b) = if v.dark_mode {
        (Color32::from_gray(52), Color32::from_gray(44))
    } else {
        (Color32::from_gray(250), Color32::from_gray(238))
    };
    p.rect_filled(stage, 6.0, a);
    let cell = 10.0;
    let (nx, ny) = (
        (stage.width() / cell).ceil() as i32,
        (stage.height() / cell).ceil() as i32,
    );
    for i in 0..nx {
        for j in 0..ny {
            if (i + j) % 2 == 1 {
                let r = Rect::from_min_size(
                    stage.min + Vec2::new(i as f32 * cell, j as f32 * cell),
                    Vec2::splat(cell),
                )
                .intersect(stage);
                p.rect_filled(r, 0.0, b);
            }
        }
    }
    draw(&p, stage.shrink(10.0), view);
    ui.painter().rect_stroke(
        stage,
        6.0,
        v.widgets.inactive.bg_stroke,
        egui::StrokeKind::Inside,
    );
    // the view buttons in a row under it, in its middle
    let (bw, bh, gap) = (34.0, 22.0, 2.0);
    ui.add_space(2.0);
    let (row, _) = ui.allocate_exact_size(Vec2::new(w, bh), Sense::hover());
    let n = views.len() as f32;
    let x0 = row.center().x - (n * bw + (n - 1.0).max(0.0) * gap) / 2.0;
    let mut buttons = Vec::with_capacity(views.len());
    for (k, (label, tip)) in views.iter().enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(x0 + k as f32 * (bw + gap), row.top()),
            Vec2::new(bw, bh),
        );
        buttons.push(r);
        let resp = ui
            .interact(r, egui::Id::new(("preview.btn", key, k)), Sense::click())
            .on_hover_text(*tip);
        let on = k == view;
        let pp = ui.painter();
        if on {
            pp.rect(
                r,
                5.0,
                v.selection.bg_fill.gamma_multiply(0.35),
                v.selection.stroke,
                egui::StrokeKind::Inside,
            );
        } else if resp.hovered() {
            pp.rect_filled(r, 5.0, v.widgets.hovered.weak_bg_fill);
        }
        let ink = if on {
            v.selection.stroke.color
        } else {
            v.weak_text_color()
        };
        pp.text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            *label,
            egui::FontId::proportional(11.5),
            ink,
        );
        if resp.clicked() {
            view = k;
        }
    }
    // the grip: drag it to make the picture taller or shorter
    let (g, gr) = ui.allocate_exact_size(Vec2::new(w, 8.0), Sense::drag());
    let gr = gr
        .on_hover_cursor(egui::CursorIcon::ResizeVertical)
        .on_hover_text("Drag to make the preview taller or shorter");
    for k in -2..=2 {
        ui.painter().circle_filled(
            Pos2::new(g.center().x + k as f32 * 5.0, g.center().y),
            1.2,
            v.weak_text_color(),
        );
    }
    if gr.dragged() {
        h = (h + gr.drag_delta().y).clamp(90.0, 420.0);
    }
    ui.data_mut(|d| {
        d.insert_persisted(vid, view);
        d.insert_persisted(hid, h);
    });
    Framed { stage, buttons }
}

/// The axes a view looks along: right, up, and towards the viewer.
fn basis(view: usize, along: DVec3, out: DVec3) -> (DVec3, DVec3, DVec3) {
    let z = DVec3::Z;
    match view {
        0 => (along, out, z),
        1 => (along, z, out),
        2 => (-out, z, along),
        _ => {
            let f = (out * 0.75 - along * 0.55 + z * 0.55).normalize();
            let r = z.cross(f).normalize();
            (r, f.cross(r), f)
        }
    }
}

/// Which triangle is nearest at each pixel of a `w` × `h` picture of
/// `tris` seen along `-f`, fitted and centred as [`paint_tris`] draws it,
/// and the depth there. Per pixel, not per triangle: sorting triangles (or
/// pieces of them) drew a roof's back layers over its front, because thin
/// layers on a slope overlap in depth across any one triangle (owner
/// markup 2026-11-23, Main roof Preview E).
struct Depths {
    w: usize,
    h: usize,
    /// (triangle, depth) per pixel, row by row from the top
    at: Vec<Option<(usize, f64)>>,
    /// the fit: model (right, up) to pixels
    k: f64,
    c: DVec2,
}

impl Depths {
    /// A model point's place in the picture, in pixels from its top left.
    fn pixel(&self, q: DVec3, r: DVec3, u: DVec3) -> DVec2 {
        DVec2::new(
            self.w as f64 / 2.0 + (q.dot(r) - self.c.x) * self.k,
            self.h as f64 / 2.0 - (q.dot(u) - self.c.y) * self.k,
        )
    }
}

fn depths(tris: &[Tri], (r, u, f): (DVec3, DVec3, DVec3), w: usize, h: usize) -> Depths {
    let (mut lo, mut hi) = (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN));
    for (t, _) in tris {
        for q in t {
            let p = DVec2::new(q.dot(r), q.dot(u));
            lo = lo.min(p);
            hi = hi.max(p);
        }
    }
    let span = (hi - lo).max(DVec2::splat(1e-6));
    let k = (w as f64 / span.x).min(h as f64 / span.y);
    let c = (lo + hi) / 2.0;
    let mut out = Depths {
        w,
        h,
        at: vec![None; w * h],
        k,
        c,
    };
    for (i, (t, _)) in tris.iter().enumerate() {
        let px = t.map(|q| out.pixel(q, r, u));
        let z = t.map(|q| q.dot(f));
        let det = (px[1] - px[0]).perp_dot(px[2] - px[0]);
        if det.abs() < 1e-12 {
            continue;
        }
        let x0 = px.iter().map(|p| p.x).fold(f64::MAX, f64::min);
        let x1 = px.iter().map(|p| p.x).fold(f64::MIN, f64::max);
        let y0 = px.iter().map(|p| p.y).fold(f64::MAX, f64::min);
        let y1 = px.iter().map(|p| p.y).fold(f64::MIN, f64::max);
        let (xa, xb) = (
            x0.floor().max(0.0) as usize,
            x1.ceil().clamp(0.0, w as f64) as usize,
        );
        let (ya, yb) = (
            y0.floor().max(0.0) as usize,
            y1.ceil().clamp(0.0, h as f64) as usize,
        );
        for y in ya..yb {
            for x in xa..xb {
                let p = DVec2::new(x as f64 + 0.5, y as f64 + 0.5);
                let b1 = (p - px[0]).perp_dot(px[2] - px[0]) / det;
                let b2 = (px[1] - px[0]).perp_dot(p - px[0]) / det;
                let b0 = 1.0 - b1 - b2;
                if b0 < -1e-9 || b1 < -1e-9 || b2 < -1e-9 {
                    continue;
                }
                let d = z[0] * b0 + z[1] * b1 + z[2] * b2;
                let cell = &mut out.at[y * w + x];
                if cell.is_none_or(|(_, e)| d > e) {
                    *cell = Some((i, d));
                }
            }
        }
    }
    out
}

/// The edges worth drawing: where faces meet at an angle, or at a mesh's
/// rim (flat-shaded blades all read the same without them). Not the seams
/// inside a flat face: where a face is cut into triangles unevenly (round
/// a window, at a layer's end), a triangle's side meets two smaller ones
/// and is used once, so it read as a rim and drew the face's diagonals
/// (MU138, a wall's Preview E and 3D).
fn drawn_edges(tris: &[Tri]) -> Vec<(DVec3, DVec3, Vec<usize>)> {
    let key = |q: DVec3| {
        (
            (q.x * 1e4).round() as i64,
            (q.y * 1e4).round() as i64,
            (q.z * 1e4).round() as i64,
        )
    };
    type Key = (i64, i64, i64);
    #[allow(clippy::type_complexity)]
    let mut edges: std::collections::HashMap<
        (Key, Key),
        (DVec3, DVec3, Vec<DVec3>, Vec<usize>),
    > = Default::default();
    // the triangles of each flat face: by the plane they lie in
    // (and of one colour: where two layers' faces meet in one plane, the
    // line between their materials is kept)
    type Plane = (i64, i64, i64, i64, [u32; 3]);
    let mut planes: std::collections::HashMap<Plane, Vec<usize>> = Default::default();
    let normal = |t: &[DVec3; 3]| (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
    let plane = |(t, c): &Tri| {
        let n = normal(t);
        (
            (n.x * 1e3).round() as i64,
            (n.y * 1e3).round() as i64,
            (n.z * 1e3).round() as i64,
            (n.dot(t[0]) * 1e4).round() as i64,
            c.map(f32::to_bits),
        )
    };
    for (i, tc) in tris.iter().enumerate() {
        let t = &tc.0;
        let n = normal(t);
        planes.entry(plane(tc)).or_default().push(i);
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let (ka, kb) = (key(a), key(b));
            let e = edges
                .entry(if ka < kb { (ka, kb) } else { (kb, ka) })
                .or_insert((a, b, vec![], vec![]));
            e.2.push(n);
            e.3.push(i);
        }
    }
    // a point of a face's plane inside one of its triangles
    let covered = |q: DVec3, of: &[usize], n: DVec3| {
        of.iter().any(|&i| {
            let t = &tris[i].0;
            (0..3).all(|k| (t[(k + 1) % 3] - t[k]).cross(q - t[k]).dot(n) >= -1e-12)
        })
    };
    let mut out = vec![];
    for (a, b, ns, owners) in edges.values() {
        let bent = ns.iter().any(|n| n.dot(ns[0]) < 0.9);
        if !(ns.len() == 1 || bent) {
            continue;
        }
        if ns.len() == 1 {
            // a seam if its face goes on either side of it all along
            let n = ns[0];
            let of = &planes[&plane(&tris[owners[0]])];
            let len = a.distance(*b);
            let across = n.cross(*b - *a).normalize_or_zero() * (len * 1e-3).max(1e-6);
            let seam = [0.2, 0.5, 0.8].iter().all(|s| {
                let m = *a + (*b - *a) * *s;
                covered(m + across, of, n) && covered(m - across, of, n)
            });
            if seam {
                continue;
            }
        }
        out.push((*a, *b, owners.clone()));
    }
    out
}

/// The picture of `tris`, `w` × `h` pixels, lit flat from the viewer's
/// side, with its feature edges where they are seen.
fn picture(
    tris: &[Tri],
    (r, u, f): (DVec3, DVec3, DVec3),
    w: usize,
    h: usize,
    outline: Color32,
) -> egui::ColorImage {
    let d = depths(tris, (r, u, f), w, h);
    let light = (f * 0.85 + u * 0.45 + r * 0.25).normalize();
    let shade: Vec<Color32> = tris
        .iter()
        .map(|(t, col)| {
            let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
            // every face in light, none dark
            let lit = 0.68 + 0.36 * n.dot(light).abs() as f32;
            Color32::from_rgb(
                (col[0] * lit * 255.0).clamp(0.0, 255.0) as u8,
                (col[1] * lit * 255.0).clamp(0.0, 255.0) as u8,
                (col[2] * lit * 255.0).clamp(0.0, 255.0) as u8,
            )
        })
        .collect();
    let mut img = egui::ColorImage::new([w, h], vec![Color32::TRANSPARENT; w * h]);
    for (px, cell) in img.pixels.iter_mut().zip(&d.at) {
        if let Some((i, _)) = cell {
            *px = shade[*i];
        }
    }
    // an edge shows on its own faces, or in front of what is at that pixel
    // give or take 2 mm. Not a couple of pixels' depth: at a Preview's
    // scale that was 10 cm, and the edges of a wall's layers showed
    // through its 3/4" finish as a mesh of diagonals (MU138)
    let bias = 0.002;
    let ink = outline.gamma_multiply(0.55);
    let blend = |base: Color32| {
        if base == Color32::TRANSPARENT {
            return ink;
        }
        let a = ink.a() as f32 / 255.0;
        let mix = |x: u8, y: u8| (x as f32 * (1.0 - a) + y as f32 * a) as u8;
        Color32::from_rgb(
            mix(base.r(), ink.r()),
            mix(base.g(), ink.g()),
            mix(base.b(), ink.b()),
        )
    };
    for (a, b, owners) in &drawn_edges(tris) {
        let (pa, pb) = (d.pixel(*a, r, u), d.pixel(*b, r, u));
        let (za, zb) = (a.dot(f), b.dot(f));
        let steps = ((pb - pa).length().ceil() as usize).max(1);
        for s in 0..=steps {
            let t = s as f64 / steps as f64;
            let p = pa + (pb - pa) * t;
            let (x, y) = (p.x.floor(), p.y.floor());
            if x < 0.0 || y < 0.0 || x >= w as f64 || y >= h as f64 {
                continue;
            }
            let at = y as usize * w + x as usize;
            let z = za + (zb - za) * t;
            if d.at[at].is_none_or(|(i, e)| owners.contains(&i) || z >= e - bias) {
                img.pixels[at] = blend(img.pixels[at]);
            }
        }
    }
    img
}

/// Triangles drawn nearest-on-top per pixel, lit flat, fitted into
/// `rect`. The picture is kept while what it shows stays the same.
fn paint_tris(
    p: &egui::Painter,
    rect: Rect,
    tris: &[Tri],
    b: (DVec3, DVec3, DVec3),
    outline: Color32,
) {
    if tris.is_empty() || rect.width() < 2.0 || rect.height() < 2.0 {
        return;
    }
    let ppp = p.ctx().pixels_per_point();
    let w = ((rect.width() * ppp).round() as usize).clamp(2, 1600);
    let h = ((rect.height() * ppp).round() as usize).clamp(2, 1200);
    use std::hash::{Hash, Hasher};
    let mut hs = std::collections::hash_map::DefaultHasher::new();
    (w, h, outline.to_array()).hash(&mut hs);
    for v in [b.0, b.1, b.2] {
        v.to_array().map(f64::to_bits).hash(&mut hs);
    }
    for (t, c) in tris {
        for q in t {
            q.to_array().map(f64::to_bits).hash(&mut hs);
        }
        c.map(f32::to_bits).hash(&mut hs);
    }
    let sig = hs.finish();
    let slot = egui::Id::new("preview.pictures");
    let ctx = p.ctx().clone();
    let kept = ctx.data_mut(|dm| {
        dm.get_temp_mut_or_default::<Vec<(u64, egui::TextureHandle)>>(slot)
            .iter()
            .find(|(k, _)| *k == sig)
            .map(|(_, t)| t.clone())
    });
    let tex = kept.unwrap_or_else(|| {
        let t = ctx.load_texture(
            format!("preview-{sig:x}"),
            picture(tris, b, w, h, outline),
            egui::TextureOptions::LINEAR,
        );
        ctx.data_mut(|dm| {
            let cache = dm.get_temp_mut_or_default::<Vec<(u64, egui::TextureHandle)>>(slot);
            cache.push((sig, t.clone()));
            // a few views of a few elements, no more
            if cache.len() > 12 {
                drop(cache.remove(0));
            }
        });
        t
    });
    p.image(
        tex.id(),
        rect,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
}

fn mesh_tris(m: &creabim_geom::TriMesh, col: [f32; 3], out: &mut Vec<Tri>) {
    let v = |i: u32| {
        let q = m.positions[i as usize];
        DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64)
    };
    for t in m.indices.chunks(3) {
        if t.len() == 3 {
            out.push(([v(t[0]), v(t[1]), v(t[2])], col));
        }
    }
}

fn part_colour(part: Part) -> [f32; 3] {
    match part {
        Part::Glass => [0.72, 0.85, 0.92],
        Part::Frame => [0.86, 0.85, 0.82],
        Part::Leaf => [0.80, 0.66, 0.50],
        Part::Handle => [0.45, 0.45, 0.47],
        _ => [0.80, 0.78, 0.74],
    }
}

/// A door's or window's plan, cut half way up as the plans cut it: the
/// wall either side, and the frame, sashes, glass and blades cut there.
fn opening_plan(
    p: &egui::Painter,
    rect: Rect,
    o: &creabim_core::Opening,
    wall: &creabim_core::Wall,
    ink: Color32,
) {
    let parts = creabim_geom::opening::opening_parts(o, wall);
    let Some(f) = creabim_geom::opening::Fitted::of(o, wall) else {
        return;
    };
    let (w, h) = (f.width(), f.height());
    let zm = h * 0.55;
    let t = wall.thickness / 2.0;
    let cutw = [0.55, 0.57, 0.55];
    let mut rects: Vec<(DVec2, DVec2, [f32; 3])> = vec![
        (DVec2::new(-0.3, -t), DVec2::new(0.0, t), cutw),
        (DVec2::new(w, -t), DVec2::new(w + 0.3, t), cutw),
    ];
    // a corner window's parts cut on the mitre (part 9)
    let mut mitres: Vec<(Vec<DVec2>, [f32; 3])> = vec![];
    for s in &parts {
        match s {
            Shaped::Box(b) if b.lo.z <= zm && zm <= b.hi.z => {
                rects.push((
                    DVec2::new(b.lo.x, b.lo.y),
                    DVec2::new(b.hi.x, b.hi.y),
                    part_colour(b.part),
                ));
            }
            Shaped::Plan(pp) if pp.z0 <= zm && zm <= pp.z1 => {
                mitres.push((pp.outline.clone(), part_colour(pp.part)))
            }
            Shaped::Slat(sl) => {
                // where the blade crosses the cut: its outline's span there
                let n = sl.outline.len();
                let mut ys = vec![];
                for k in 0..n {
                    let (a, b) = (sl.outline[k], sl.outline[(k + 1) % n]);
                    if (a.y - zm) * (b.y - zm) <= 0.0 && (a.y - b.y).abs() > 1e-12 {
                        ys.push(a.x + (b.x - a.x) * (zm - a.y) / (b.y - a.y));
                    }
                }
                if ys.len() >= 2 {
                    let (y0, y1) = ys
                        .iter()
                        .fold((f64::MAX, f64::MIN), |(l, h), y| (l.min(*y), h.max(*y)));
                    rects.push((
                        DVec2::new(sl.x0, y0),
                        DVec2::new(sl.x1, y1),
                        part_colour(sl.part),
                    ));
                }
            }
            _ => {}
        }
    }
    let (mut lo, mut hi) = (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN));
    for (a, b, _) in &rects {
        lo = lo.min(*a);
        hi = hi.max(*b);
    }
    for q in mitres.iter().flat_map(|(pts, _)| pts) {
        lo = lo.min(*q);
        hi = hi.max(*q);
    }
    let span = (hi - lo).max(DVec2::splat(1e-6));
    let k = ((rect.width() as f64) / span.x).min((rect.height() as f64) / span.y);
    let c = (lo + hi) / 2.0;
    // outside (+y) down, as a plan is read with the outside at the foot
    let at = |q: DVec2| {
        Pos2::new(
            rect.center().x + ((q.x - c.x) * k) as f32,
            rect.center().y + ((q.y - c.y) * k) as f32,
        )
    };
    for (a, b, col) in &rects {
        let fill = Color32::from_rgb(
            (col[0] * 255.0) as u8,
            (col[1] * 255.0) as u8,
            (col[2] * 255.0) as u8,
        );
        let r = Rect::from_two_pos(at(*a), at(*b));
        p.rect(
            r,
            0.0,
            fill,
            Stroke::new(0.8, ink),
            egui::StrokeKind::Middle,
        );
    }
    for (pts, col) in &mitres {
        let fill = Color32::from_rgb(
            (col[0] * 255.0) as u8,
            (col[1] * 255.0) as u8,
            (col[2] * 255.0) as u8,
        );
        p.add(egui::Shape::convex_polygon(
            pts.iter().map(|q| at(*q)).collect(),
            fill,
            Stroke::new(0.8, ink),
        ));
    }
}

/// A door's or window's section, cut through its middle, drawn exactly
/// from its parts: the wall cut above and below, frame, sashes, glass and
/// a louvre's tilted blades as they are built (MU104).
fn opening_section(
    p: &egui::Painter,
    rect: Rect,
    o: &creabim_core::Opening,
    wall: &creabim_core::Wall,
    ink: Color32,
) {
    let parts = creabim_geom::opening::opening_parts(o, wall);
    let Some(f) = creabim_geom::opening::Fitted::of(o, wall) else {
        return;
    };
    let (w, h) = (f.width(), f.height());
    let xm = w / 2.0;
    let t = wall.thickness / 2.0;
    let mut polys: Vec<(Vec<DVec2>, [f32; 3])> = vec![];
    let cutw = [0.55, 0.57, 0.55];
    // the wall above the head and below the sill
    polys.push((
        vec![
            DVec2::new(-t, h),
            DVec2::new(t, h),
            DVec2::new(t, h + 0.25),
            DVec2::new(-t, h + 0.25),
        ],
        cutw,
    ));
    if o.sill > 1e-6 {
        let b = -o.sill.min(0.25);
        polys.push((
            vec![
                DVec2::new(-t, b),
                DVec2::new(t, b),
                DVec2::new(t, 0.0),
                DVec2::new(-t, 0.0),
            ],
            cutw,
        ));
    }
    for s in &parts {
        match s {
            Shaped::Box(b) if b.lo.x <= xm && xm <= b.hi.x => {
                polys.push((
                    vec![
                        DVec2::new(b.lo.y, b.lo.z),
                        DVec2::new(b.hi.y, b.lo.z),
                        DVec2::new(b.hi.y, b.hi.z),
                        DVec2::new(b.lo.y, b.hi.z),
                    ],
                    part_colour(b.part),
                ));
            }
            Shaped::Slat(sl) if sl.x0 <= xm && xm <= sl.x1 => {
                polys.push((sl.outline.clone(), part_colour(sl.part)))
            }
            // mitred at a corner: its depth where the cut crosses it
            Shaped::Plan(pp) => {
                let n = pp.outline.len();
                let mut ys = vec![];
                for k in 0..n {
                    let (a, b) = (pp.outline[k], pp.outline[(k + 1) % n]);
                    if (a.x - xm) * (b.x - xm) <= 0.0 && (a.x - b.x).abs() > 1e-12 {
                        ys.push(a.y + (b.y - a.y) * (xm - a.x) / (b.x - a.x));
                    }
                }
                if ys.len() >= 2 {
                    let (y0, y1) = ys
                        .iter()
                        .fold((f64::MAX, f64::MIN), |(l, h), y| (l.min(*y), h.max(*y)));
                    polys.push((
                        vec![
                            DVec2::new(y0, pp.z0),
                            DVec2::new(y1, pp.z0),
                            DVec2::new(y1, pp.z1),
                            DVec2::new(y0, pp.z1),
                        ],
                        part_colour(pp.part),
                    ));
                }
            }
            _ => {}
        }
    }
    let (mut lo, mut hi) = (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN));
    for (pts, _) in &polys {
        for q in pts {
            lo = lo.min(*q);
            hi = hi.max(*q);
        }
    }
    let span = (hi - lo).max(DVec2::splat(1e-6));
    let k = ((rect.width() as f64) / span.x).min((rect.height() as f64) / span.y);
    let c = (lo + hi) / 2.0;
    // outside (+y) to the left, as a section is usually read from inside
    let at = |q: DVec2| {
        Pos2::new(
            rect.center().x - ((q.x - c.x) * k) as f32,
            rect.center().y - ((q.y - c.y) * k) as f32,
        )
    };
    for (pts, col) in &polys {
        let v: Vec<Pos2> = pts.iter().map(|q| at(*q)).collect();
        let fill = Color32::from_rgb(
            (col[0] * 255.0) as u8,
            (col[1] * 255.0) as u8,
            (col[2] * 255.0) as u8,
        );
        p.add(egui::Shape::convex_polygon(v, fill, Stroke::new(0.8, ink)));
    }
}

impl CreaBimApp {
    /// The Preview section, under Geometry (owner MU104): for an element
    /// built in 3D, a door or window, an element type or a material.
    pub(super) fn element_preview(&mut self, ui: &mut egui::Ui, id: ElementId) {
        if self.shell.classic {
            return;
        }
        let model = self.doc.model();
        let Some(e) = model.get(id).cloned() else {
            return;
        };
        let ink = ui.visuals().text_color();
        #[allow(clippy::large_enum_variant)]
        enum What {
            /// The window or door, its wall, and the windows round the
            /// corner with it (part 9), each with its wall.
            Opening(
                creabim_core::Opening,
                creabim_core::Wall,
                Vec<(creabim_core::Opening, creabim_core::Wall)>,
            ),
            Material(creabim_core::Material),
            Tris(Vec<Tri>, DVec3, DVec3),
        }
        let what = match &e.kind {
            ElementKind::Opening(o) if o.roof_at.is_none() => {
                match model.get(o.host).map(|h| &h.kind) {
                    Some(ElementKind::Wall(w)) => {
                        // a corner window is seen with the one round the corner
                        let others = if o
                            .arrangement
                            .is_some_and(|a| a.corner_start || a.corner_end)
                        {
                            creabim_core::arrangement::group_of(model, e.id)
                                .into_iter()
                                .filter(|g| *g != e.id)
                                .filter_map(|g| match model.get(g).map(|x| &x.kind) {
                                    Some(ElementKind::Opening(p)) => {
                                        match model.get(p.host).map(|h| &h.kind) {
                                            Some(ElementKind::Wall(pw)) => {
                                                Some((p.clone(), pw.clone()))
                                            }
                                            _ => None,
                                        }
                                    }
                                    _ => None,
                                })
                                .collect()
                        } else {
                            vec![]
                        };
                        What::Opening(o.clone(), w.clone(), others)
                    }
                    _ => return,
                }
            }
            ElementKind::Material(m) => What::Material(m.clone()),
            ElementKind::ElementType(t) => {
                if !matches!(
                    t.category(),
                    TypeCategory::Wall
                        | TypeCategory::Slab
                        | TypeCategory::Door
                        | TypeCategory::Window
                        | TypeCategory::Column
                        | TypeCategory::Beam
                        | TypeCategory::Roof
                        | TypeCategory::Footing
                ) {
                    return;
                }
                let Some((m, _, _)) = super::typepreview::sample_model(model, id, 35.0, 28.0)
                else {
                    return;
                };
                let mut tris = vec![];
                for it in creabim_geom::scene::build_scene(&m) {
                    mesh_tris(&it.mesh, it.color, &mut tris);
                }
                What::Tris(tris, DVec3::X, -DVec3::Y)
            }
            _ => {
                // while the scene is rebuilt after a change, the last
                // picture stays: without it the section vanished for those
                // frames and the panel jumped (MU142: a Turn click)
                let kept = egui::Id::new("elempreview.last").with(id);
                let tris: Vec<Tri> = match creabim_geom::scene::shared_scene_ready(model) {
                    Some(scene) => {
                        let mut tris = vec![];
                        for it in scene.iter().filter(|it| it.id == id && !it.ghost) {
                            mesh_tris(&it.mesh, it.color, &mut tris);
                        }
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(kept, std::sync::Arc::new(tris.clone())));
                        tris
                    }
                    None => match ui
                        .ctx()
                        .data(|d| d.get_temp::<std::sync::Arc<Vec<Tri>>>(kept))
                    {
                        Some(t) => (*t).clone(),
                        None => return,
                    },
                };
                if tris.is_empty() || tris.len() > 60_000 {
                    return;
                }
                // along its length where it has one
                let along = match &e.kind {
                    ElementKind::Wall(w) => (w.end - w.start).normalize_or_zero().extend(0.0),
                    _ => DVec3::X,
                };
                let along = if along.length() < 0.5 {
                    DVec3::X
                } else {
                    along
                };
                What::Tris(tris, along, DVec3::Z.cross(along))
            }
        };
        let open = self.prop_header(ui, "preview", "Preview", true, None);
        let key = match &e.kind {
            ElementKind::Opening(o) => format!("{:?}", o.kind),
            k => format!("{:?}", std::mem::discriminant(k)),
        };
        crate::props::fold(ui, open, |ui| match what {
            What::Material(m) => preview_frame(ui, "material", MATERIAL_VIEWS, |p, r, view| {
                let s = r.width().min(r.height());
                let sq = Rect::from_center_size(r.center(), Vec2::splat(s));
                match view {
                    0 => {
                        let mut child = ui_painter_sphere(&m, sq);
                        for sh in child.drain(..) {
                            p.add(sh);
                        }
                    }
                    1 => {
                        let mut tris = vec![];
                        let c = cube();
                        for t in c {
                            tris.push((t, [m.color[0], m.color[1], m.color[2]]));
                        }
                        paint_tris(
                            p,
                            sq.shrink(s * 0.12),
                            &tris,
                            basis(3, DVec3::X, -DVec3::Y),
                            ink,
                        );
                    }
                    _ => {
                        let col = Color32::from_rgb(
                            (m.color[0] * 255.0) as u8,
                            (m.color[1] * 255.0) as u8,
                            (m.color[2] * 255.0) as u8,
                        );
                        p.rect_filled(r.shrink(6.0), 4.0, col);
                    }
                }
            }),
            What::Opening(o, wall, others) => {
                preview_frame(ui, &key, ELEMENT_VIEWS, |p, r, view| {
                    if view == 2 {
                        opening_section(p, r, &o, &wall, ink);
                        return;
                    }
                    if view == 0 {
                        opening_plan(p, r, &o, &wall, ink);
                        return;
                    }
                    let Some(at) = creabim_geom::opening::to_model(&o, &wall, 0.0) else {
                        return;
                    };
                    let o0 = at(DVec3::ZERO);
                    let (ex, ey) = (at(DVec3::X) - o0, at(DVec3::Y) - o0);
                    let mut tris = vec![];
                    for (part, m) in creabim_geom::opening::opening_meshes(&o, &wall, 0.0) {
                        mesh_tris(&m, part_colour(part), &mut tris);
                    }
                    for (q, qw) in &others {
                        for (part, m) in creabim_geom::opening::opening_meshes(q, qw, 0.0) {
                            mesh_tris(&m, part_colour(part), &mut tris);
                        }
                    }
                    paint_tris(p, r, &tris, basis(view, ex, ey), ink);
                })
            }
            What::Tris(tris, along, out) => preview_frame(ui, &key, ELEMENT_VIEWS, |p, r, view| {
                paint_tris(p, r, &tris, basis(view, along, out), ink);
            }),
        });
    }
}

/// The material sphere, as the materials window draws it, as shapes.
fn ui_painter_sphere(m: &creabim_core::Material, sq: Rect) -> Vec<egui::Shape> {
    let steps = 40;
    let mut mesh = egui::Mesh::default();
    let radius = sq.width() * 0.45;
    let center = sq.center();
    for j in 0..=steps {
        for i in 0..=steps {
            let x = i as f32 / steps as f32 * 2.0 - 1.0;
            let y = j as f32 / steps as f32 * 2.0 - 1.0;
            let d2 = x * x + y * y;
            let (nx, ny) = if d2 > 1.0 {
                let d = d2.sqrt();
                (x / d, y / d)
            } else {
                (x, y)
            };
            let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
            let c = super::materials::shade_srgb(m, [nx, -ny, nz]);
            let a = (m.opacity.clamp(0.05, 1.0) * 255.0) as u8;
            mesh.colored_vertex(
                center + Vec2::new(nx, ny) * radius,
                Color32::from_rgba_unmultiplied(c[0], c[1], c[2], a),
            );
        }
    }
    let row = steps as u32 + 1;
    for j in 0..steps as u32 {
        for i in 0..steps as u32 {
            let k = j * row + i;
            mesh.add_triangle(k, k + 1, k + row);
            mesh.add_triangle(k + 1, k + row + 1, k + row);
        }
    }
    vec![egui::Shape::mesh(mesh)]
}

/// A unit cube's triangles.
fn cube() -> Vec<[DVec3; 3]> {
    let v = |x: f64, y: f64, z: f64| DVec3::new(x, y, z);
    let faces = [
        [v(0., 0., 0.), v(1., 0., 0.), v(1., 1., 0.), v(0., 1., 0.)],
        [v(0., 0., 1.), v(1., 0., 1.), v(1., 1., 1.), v(0., 1., 1.)],
        [v(0., 0., 0.), v(1., 0., 0.), v(1., 0., 1.), v(0., 0., 1.)],
        [v(0., 1., 0.), v(1., 1., 0.), v(1., 1., 1.), v(0., 1., 1.)],
        [v(0., 0., 0.), v(0., 1., 0.), v(0., 1., 1.), v(0., 0., 1.)],
        [v(1., 0., 0.), v(1., 1., 0.), v(1., 1., 1.), v(1., 0., 1.)],
    ];
    faces
        .iter()
        .flat_map(|f| [[f[0], f[1], f[2]], [f[0], f[2], f[3]]])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MU142 (owner): the picture spans the panel's whole width and the
    /// view buttons sit in a row under it, inside the panel at its
    /// narrowest as at a wide one.
    #[test]
    fn the_preview_spans_the_panel_with_its_buttons_under_it() {
        for width in [200.0, 320.0, 520.0] {
            let ctx = egui::Context::default();
            let mut got: Option<(Rect, Framed)> = None;
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 700.0))),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                let room = ui.available_rect_before_wrap();
                got = Some((room, preview_frame(ui, "test", ELEMENT_VIEWS, |_, _, _| {})));
            });
            out.textures_delta.clear();
            let (room, f) = got.expect("drawn");
            assert!(
                (f.stage.width() - room.width()).abs() < 0.5,
                "{width}: {:?} in {room:?}",
                f.stage
            );
            assert_eq!(f.buttons.len(), ELEMENT_VIEWS.len());
            for b in &f.buttons {
                assert!(
                    b.top() >= f.stage.bottom(),
                    "{width}: {b:?} not under {:?}",
                    f.stage
                );
                assert!(
                    b.left() >= room.left() && b.right() <= room.right(),
                    "{width}: {b:?} out of {room:?}"
                );
            }
            // in one row, in its middle
            assert!(f
                .buttons
                .windows(2)
                .all(|p| p[0].top() == p[1].top() && p[0].right() < p[1].left()));
            let mid = (f.buttons[0].left() + f.buttons[f.buttons.len() - 1].right()) / 2.0;
            assert!((mid - f.stage.center().x).abs() < 0.5);
        }
    }

    /// A flat square cut into triangles unevenly (one half whole, the other
    /// split at the diagonal's middle, as round an opening): only its four
    /// sides are drawn, not the diagonal seams (MU138).
    #[test]
    fn a_flat_face_cut_unevenly_draws_no_seams() {
        let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
        let c = [0.8, 0.8, 0.8];
        let tris: Vec<Tri> = vec![
            ([p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)], c),
            ([p(0.0, 0.0), p(0.5, 0.5), p(0.0, 1.0)], c),
            ([p(0.5, 0.5), p(1.0, 1.0), p(0.0, 1.0)], c),
        ];
        let edges = drawn_edges(&tris);
        assert_eq!(edges.len(), 4, "{edges:?}");
        for (a, b, _) in &edges {
            // each on the square's rim: x or y the same at both ends
            assert!(
                (a.x - b.x).abs() < 1e-9 || (a.y - b.y).abs() < 1e-9,
                "{a} {b}"
            );
        }
        // and a fold still draws: the same square bent along its diagonal
        let bent: Vec<Tri> = vec![
            ([p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)], c),
            ([p(0.0, 0.0), p(1.0, 1.0), DVec3::new(0.0, 1.0, 0.5)], c),
        ];
        assert_eq!(drawn_edges(&bent).len(), 5);
    }

    /// A thin layer in front of others hides their edges: at a Preview's
    /// scale a "couple of pixels" of depth was 10 cm, and a wall's inner
    /// layers drew through its 3/4" finish as a mesh of lines (MU138).
    #[test]
    fn edges_behind_a_thin_finish_are_hidden() {
        let q = |x: f64, y: f64, z: f64| DVec3::new(x, y, z);
        let quad = |y: f64, x0: f64, x1: f64, z0: f64, z1: f64, c: [f32; 3]| -> Vec<Tri> {
            vec![
                ([q(x0, y, z0), q(x1, y, z0), q(x1, y, z1)], c),
                ([q(x0, y, z0), q(x1, y, z1), q(x0, y, z1)], c),
            ]
        };
        // the finish, 19 mm in front of a smaller board behind it
        let mut tris = quad(0.019, 0.0, 6.0, 0.0, 3.0, [0.9, 0.9, 0.9]);
        tris.extend(quad(0.0, 2.0, 4.0, 1.0, 2.0, [0.5, 0.5, 0.5]));
        let (w, h) = (200, 100);
        let img = picture(&tris, basis(1, DVec3::X, DVec3::Y), w, h, Color32::BLACK);
        // the finish's own colour, at its middle
        let face = img.pixels[(h / 2) * w + w / 2];
        // inside the finish, two pixels clear of its rim: nothing but it
        let d = depths(&tris, basis(1, DVec3::X, DVec3::Y), w, h);
        let (lo, hi) = (
            d.pixel(q(0.0, 0.0, 3.0), DVec3::X, DVec3::Z),
            d.pixel(q(6.0, 0.0, 0.0), DVec3::X, DVec3::Z),
        );
        let mut lines = 0;
        for y in (lo.y as usize + 3)..(hi.y as usize).saturating_sub(3) {
            for x in (lo.x as usize + 3)..(hi.x as usize).saturating_sub(3) {
                lines += usize::from(img.pixels[y * w + x] != face);
            }
        }
        assert_eq!(lines, 0, "pixels of the board's edges through the finish");
    }

    /// Of the pixels sampled that show something, (how many show a face
    /// farther by more than 1 mm than the nearest one there, how many).
    /// The nearest is found triangle by triangle, apart from `depths`.
    fn wrong_on_top(tris: &[Tri], (r, u, f): (DVec3, DVec3, DVec3), d: &Depths) -> (usize, usize) {
        let (mut wrong, mut seen) = (0, 0);
        for y in (0..d.h).step_by(3) {
            for x in (0..d.w).step_by(3) {
                let p = DVec2::new(x as f64 + 0.5, y as f64 + 0.5);
                let mut near: Option<f64> = None;
                for (t, _) in tris {
                    let px = t.map(|q| d.pixel(q, r, u));
                    let det = (px[1] - px[0]).perp_dot(px[2] - px[0]);
                    if det.abs() < 1e-12 {
                        continue;
                    }
                    let b1 = (p - px[0]).perp_dot(px[2] - px[0]) / det;
                    let b2 = (px[1] - px[0]).perp_dot(p - px[0]) / det;
                    let b0 = 1.0 - b1 - b2;
                    if b0 < -1e-9 || b1 < -1e-9 || b2 < -1e-9 {
                        continue;
                    }
                    let z = t[0].dot(f) * b0 + t[1].dot(f) * b1 + t[2].dot(f) * b2;
                    near = Some(near.map_or(z, |n: f64| n.max(z)));
                }
                if let (Some(n), Some((_, z))) = (near, d.at[y * d.w + x]) {
                    seen += 1;
                    if n - z > 1e-3 {
                        wrong += 1;
                    }
                }
            }
        }
        (wrong, seen)
    }

    #[test]
    fn the_starter_houses_roof_previews_its_nearest_face_everywhere() {
        // owner markup 2026-11-23: Main roof, Preview E, its back layers
        // drawn across its front. Painting whole triangles sorted by their
        // middles put a farther face on top at 3115 of 3481 sampled points
        // of this view; sorting pieces of them still at 2767.
        let m = creabim_core::starter::starter_house();
        let roof = m.iter().find(|e| e.name == "Main roof").expect("Main roof");
        let mut tris = vec![];
        for it in creabim_geom::scene::build_scene(&m)
            .iter()
            .filter(|it| it.id == roof.id && !it.ghost)
        {
            mesh_tris(&it.mesh, it.color, &mut tris);
        }
        assert!(!tris.is_empty());
        for view in 0..4 {
            let b = basis(view, DVec3::X, DVec3::Y);
            let d = depths(&tris, b, 240, 90);
            let (wrong, seen) = wrong_on_top(&tris, b, &d);
            assert!(seen > 100, "view {view}: {seen}");
            assert_eq!(wrong, 0, "view {view}: {wrong} of {seen}");
        }
    }
}
