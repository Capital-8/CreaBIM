//! Site services worked out against the ground: a drain's cover and
//! invert levels at each chamber, a tank's levels, the solids of both,
//! how they draw in plan, and their digs. A dig is measured apart from
//! the earthworks, because it is backfilled and leaves the ground as it
//! was.

use crate::plan::{Pen, PlanItem};
use crate::TriMesh;
use creabim_core::services::{Drain, DrainUse, Tank, TankUse};
use creabim_core::site::{Site, SiteForm};
use creabim_core::{members, ElementId, ElementKind, Model};
use glam::{DVec2, DVec3};

/// A pipe's wall, each side, for its outside size.
const PIPE_WALL: f64 = 0.01;
/// A chamber's walls and base.
const CHAMBER_WALL: f64 = 0.1;
/// What a chamber's floor sits under its outgoing invert.
const CHAMBER_SUMP: f64 = 0.15;
/// How many times a long section draws heights over their true scale.
pub const LONG_SECTION_EXAGGERATION: f64 = 5.0;
/// The earth under the ground in a long section.
const EARTH_TINT: [f32; 3] = [0.93, 0.89, 0.80];
/// Level tags and notes, in grey.
const MUTED: [f32; 3] = [0.45, 0.48, 0.46];

pub const FOUL_COLOR: [f32; 3] = [0.62, 0.40, 0.24];
pub const SURFACE_COLOR: [f32; 3] = [0.26, 0.48, 0.78];
pub const COMBINED_COLOR: [f32; 3] = [0.50, 0.38, 0.58];
pub const FIELD_COLOR: [f32; 3] = [0.38, 0.58, 0.36];
pub const CHAMBER_COLOR: [f32; 3] = [0.66, 0.66, 0.64];
pub const TANK_COLOR: [f32; 3] = [0.30, 0.34, 0.30];
pub const GULLY_COLOR: [f32; 3] = [0.22, 0.22, 0.23];
pub const SOAKAWAY_COLOR: [f32; 3] = [0.62, 0.60, 0.55];

/// The finished ground's level at `p`, from the terrains that hold it
/// (with their works), as an elevation. None off every terrain.
pub fn ground_level(model: &Model, p: DVec2) -> Option<f64> {
    for e in model.iter() {
        let ElementKind::Site(s) = &e.kind else {
            continue;
        };
        let SiteForm::Terrain { boundary, top, .. } = &s.form else {
            continue;
        };
        if !creabim_core::polygon::contains(boundary, p) {
            continue;
        }
        let base = members::elevation(model, s.level);
        let z = crate::earth::shaped(model, s)
            .and_then(|sh| sh.ground.height(p))
            .unwrap_or(*top);
        return Some(base + z);
    }
    None
}

/// A drain's levels at each point of its path: the ground over it (the
/// chamber's cover) and its invert, both elevations.
#[derive(Clone, Debug, PartialEq)]
pub struct DrainLevels {
    pub cover: Vec<f64>,
    pub invert: Vec<f64>,
}

impl DrainLevels {
    /// How far the ground is above the pipe's top at the shallowest
    /// point, and where (an index of its path).
    pub fn least_cover(&self, d: &Drain) -> Option<(f64, usize)> {
        let top = d.diameter + 2.0 * PIPE_WALL;
        self.cover
            .iter()
            .zip(&self.invert)
            .map(|(c, i)| c - (i + top))
            .enumerate()
            .map(|(k, v)| (v, k))
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }
}

/// Where the ground is over `p`, or the level `s` is on.
fn cover_at(model: &Model, s: &Site, p: DVec2) -> f64 {
    ground_level(model, p).unwrap_or_else(|| members::elevation(model, s.level))
}

/// `d`'s levels, its head `d.depth` under the ground there.
pub fn drain_levels(model: &Model, s: &Site, d: &Drain) -> DrainLevels {
    let cover: Vec<f64> = d.path.iter().map(|p| cover_at(model, s, *p)).collect();
    let head = cover.first().copied().unwrap_or(0.0) - d.depth;
    DrainLevels {
        invert: d.inverts(head),
        cover,
    }
}

/// A tank's levels: the ground over it, its top and the inside of its
/// floor, elevations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TankLevels {
    pub ground: f64,
    pub top: f64,
    pub floor: f64,
}

pub fn tank_levels(model: &Model, s: &Site, t: &Tank) -> TankLevels {
    let ground = cover_at(model, s, t.centre);
    let top = ground - t.cover;
    TankLevels {
        ground,
        top,
        floor: top - t.wall - t.depth,
    }
}

/// What digging a service in takes, in cubic metres: the hole, what goes
/// in it (pipes and chambers, or the tank), and what is put back.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Dig {
    pub dug: f64,
    pub placed: f64,
    /// The granular bed and surround under and round a pipe.
    pub bed: f64,
    pub backfill: f64,
    /// The deepest the dig goes under the ground.
    pub deepest: f64,
}

/// A drain's trench and chamber pits.
pub fn drain_dig(model: &Model, s: &Site, d: &Drain) -> Dig {
    let lv = drain_levels(model, s, d);
    let r = d.diameter / 2.0 + PIPE_WALL;
    let mut dig = Dig::default();
    for i in 0..d.path.len().saturating_sub(1) {
        let len = d.path[i].distance(d.path[i + 1]);
        let deep = |k: usize| (lv.cover[k] - lv.invert[k] + PIPE_WALL + d.bed).max(0.0);
        let (a, b) = (deep(i), deep(i + 1));
        dig.deepest = dig.deepest.max(a).max(b);
        dig.dug += d.trench * (a + b) / 2.0 * len;
        let pipe = std::f64::consts::PI * r * r * len;
        dig.placed += pipe;
        // the bed under it, and a surround to its top
        dig.bed += (d.trench * (d.bed + 2.0 * r) - pipe / len.max(1e-9)) * len;
    }
    if d.chambers {
        let outside = d.chamber + 2.0 * CHAMBER_WALL;
        for k in chamber_points(d) {
            let deep = (lv.cover[k] - lv.invert[k] + CHAMBER_SUMP + CHAMBER_WALL).max(0.0);
            dig.deepest = dig.deepest.max(deep);
            let pit = (outside + 0.6).powi(2) * deep;
            dig.dug += pit;
            dig.placed += outside * outside * deep;
        }
    }
    dig.backfill = (dig.dug - dig.placed - dig.bed).max(0.0);
    dig
}

/// A tank's pit: its outside and its working space each side, down to
/// under its floor.
pub fn tank_dig(model: &Model, s: &Site, t: &Tank) -> Dig {
    let lv = tank_levels(model, s, t);
    let deep = lv.ground - lv.floor + t.wall + 0.1;
    let (l, w) = (t.length + 2.0 * t.wall, t.width + 2.0 * t.wall);
    let dug = (l + 2.0 * t.working) * (w + 2.0 * t.working) * deep;
    let placed = l * w * (t.depth + 2.0 * t.wall);
    // a blinding under it
    let bed = (l + 2.0 * t.working) * (w + 2.0 * t.working) * 0.1;
    Dig {
        dug,
        placed,
        bed,
        backfill: (dug - placed - bed).max(0.0),
        deepest: deep,
    }
}

/// Which points of a drain have a chamber: its head, its bends and its
/// end.
pub fn chamber_points(d: &Drain) -> Vec<usize> {
    if !d.chambers {
        return vec![];
    }
    (0..d.path.len()).collect()
}

/// The colour a drain is drawn in.
pub fn drain_color(d: &Drain) -> [f32; 3] {
    match d.service {
        DrainUse::Foul => FOUL_COLOR,
        DrainUse::Surface => SURFACE_COLOR,
        DrainUse::Combined => COMBINED_COLOR,
        DrainUse::Field => FIELD_COLOR,
    }
}

/// A tube `r` round from `a` to `b`, eight-sided.
fn tube(m: &mut TriMesh, a: DVec3, b: DVec3, r: f64) {
    let along = (b - a).normalize_or_zero();
    if along == DVec3::ZERO {
        return;
    }
    let side = along.cross(DVec3::Z).normalize_or(DVec3::X);
    let up = side.cross(along);
    let ring = |c: DVec3| -> Vec<DVec3> {
        (0..8)
            .map(|k| {
                let t = k as f64 * std::f64::consts::TAU / 8.0;
                c + (side * t.cos() + up * t.sin()) * r
            })
            .collect()
    };
    let (ra, rb) = (ring(a), ring(b));
    for k in 0..8 {
        let n = (k + 1) % 8;
        let mid = (ra[k] + ra[n]) / 2.0 - a;
        m.add_quad([ra[k], ra[n], rb[n], rb[k]], mid.normalize_or(DVec3::Z));
    }
    let ends: Vec<[usize; 3]> = (1..7).map(|k| [0, k, k + 1]).collect();
    m.add_polygon(&ra.iter().rev().copied().collect::<Vec<_>>(), &ends, -along);
    m.add_polygon(&rb, &ends, along);
}

/// A drain's solids: its pipe, and each chamber.
pub fn drain_meshes(model: &Model, s: &Site, d: &Drain) -> Vec<(TriMesh, [f32; 3], bool)> {
    let lv = drain_levels(model, s, d);
    let r = d.diameter / 2.0 + PIPE_WALL;
    let mut pipe = TriMesh::default();
    for i in 0..d.path.len().saturating_sub(1) {
        let lift = DVec3::Z * (r - PIPE_WALL);
        tube(
            &mut pipe,
            d.path[i].extend(lv.invert[i]) + lift,
            d.path[i + 1].extend(lv.invert[i + 1]) + lift,
            r,
        );
    }
    let mut out = vec![(pipe, drain_color(d), true)];
    let mut chambers = TriMesh::default();
    let half = d.chamber / 2.0 + CHAMBER_WALL;
    for k in chamber_points(d) {
        let p = d.path[k];
        // square to the run out of it (or into the last)
        let dir = if k + 1 < d.path.len() {
            d.path[k + 1] - p
        } else {
            p - d.path[k - 1]
        }
        .normalize_or(DVec2::X);
        let ring: Vec<DVec2> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .iter()
            .map(|(x, y)| p + dir * (x * half) + dir.perp() * (y * half))
            .collect();
        let top = lv.cover[k];
        let floor = lv.invert[k] - CHAMBER_SUMP - CHAMBER_WALL;
        let m = crate::scene::slab_mesh(&ring, top, (top - floor).max(0.05));
        chambers.positions.extend(m.positions.iter().copied());
        let base = chambers.normals.len() as u32;
        chambers.normals.extend(m.normals.iter().copied());
        chambers.indices.extend(m.indices.iter().map(|i| i + base));
    }
    if !chambers.is_empty() {
        out.push((chambers, CHAMBER_COLOR, false));
    }
    out
}

/// A tank's solid.
pub fn tank_meshes(model: &Model, s: &Site, t: &Tank) -> Vec<(TriMesh, [f32; 3], bool)> {
    let lv = tank_levels(model, s, t);
    let color = if t.service == TankUse::Soakaway {
        SOAKAWAY_COLOR
    } else {
        TANK_COLOR
    };
    vec![(
        crate::scene::slab_mesh(&t.outline(), lv.top, lv.top - lv.floor + t.wall),
        color,
        true,
    )]
}

fn line(out: &mut Vec<PlanItem>, id: ElementId, pen: Pen, a: DVec2, b: DVec2) {
    out.push(PlanItem::Line { id, pen, a, b });
}

fn ring(out: &mut Vec<PlanItem>, id: ElementId, pen: Pen, pts: &[DVec2]) {
    for i in 0..pts.len() {
        line(out, id, pen, pts[i], pts[(i + 1) % pts.len()]);
    }
}

/// A drain in plan: its pipe, a dash for each metre of a surface water
/// run, an arrow the way it flows on each length with its size and
/// gradient, and each chamber a square with its name, its cover level
/// and its invert (with `levels`).
pub fn drain_plan(
    model: &Model,
    out: &mut Vec<PlanItem>,
    id: ElementId,
    s: &Site,
    d: &Drain,
    levels: bool,
) {
    if d.path.len() < 2 {
        return;
    }
    let pen = match d.service {
        DrainUse::Surface | DrainUse::Field => Pen::Hidden,
        _ => Pen::Projection,
    };
    let half = d.chamber / 2.0 + CHAMBER_WALL;
    let chambers = chamber_points(d);
    for i in 0..d.path.len() - 1 {
        let (a, b) = (d.path[i], d.path[i + 1]);
        let len = a.distance(b);
        if len < 1e-6 {
            continue;
        }
        let u = (b - a) / len;
        // stopped at the chambers' walls
        let gap = |k: usize| if chambers.contains(&k) { half } else { 0.0 };
        let (p, q) = (a + u * gap(i), b - u * gap(i + 1));
        if (q - p).dot(u) > 0.0 {
            line(out, id, pen, p, q);
        }
        // a drainage field's gravel trench, its edges either side
        if d.service == DrainUse::Field {
            let side = u.perp() * (d.trench / 2.0);
            for s in [side, -side] {
                line(out, id, Pen::Fine, a + s, b + s);
            }
            if i == 0 {
                line(out, id, Pen::Fine, a + side, a - side);
            }
            if i + 2 == d.path.len() {
                line(out, id, Pen::Fine, b + side, b - side);
            }
        }
        // the way it flows, mid-length
        if len > 2.0 * half + 1.0 {
            let mid = (a + b) / 2.0;
            let w = u.perp() * 0.2;
            line(out, id, Pen::Symbol, mid + u * 0.3, mid - u * 0.3 + w);
            line(out, id, Pen::Symbol, mid + u * 0.3, mid - u * 0.3 - w);
            if levels {
                let side = u.perp() * 0.6;
                out.push(crate::parts::ground_text(
                    id,
                    mid + side,
                    format!(
                        "{} Ø{:.0} {}",
                        d.service.letter(),
                        d.diameter * 1000.0,
                        d.gradient_text()
                    ),
                ));
            }
        }
    }
    let lv = drain_levels(model, s, d);
    for (n, k) in chambers.iter().enumerate() {
        let p = d.path[*k];
        let dir = if k + 1 < d.path.len() {
            d.path[k + 1] - p
        } else {
            p - d.path[k - 1]
        }
        .normalize_or(DVec2::X);
        let sq: Vec<DVec2> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .iter()
            .map(|(x, y)| p + dir * (x * half) + dir.perp() * (y * half))
            .collect();
        ring(out, id, Pen::Projection, &sq);
        // a cover: its inner square
        let inner: Vec<DVec2> = sq.iter().map(|c| p + (*c - p) * 0.6).collect();
        ring(out, id, Pen::Fine, &inner);
        if levels {
            let at = p + DVec2::new(half + 0.3, -half);
            out.push(PlanItem::Text {
                angle: 0.0,
                id,
                at: at + DVec2::Y * 0.9,
                text: format!("{}{}", d.service.letter(), n + 1),
                size: crate::parts::GROUND_TEXT_MM,
                align: crate::paper::Anchor::Start,
                mm: 0.1,
                rgb: None,
                bold: true,
            });
            out.push(PlanItem::Text {
                angle: 0.0,
                id,
                at: at + DVec2::Y * 0.45,
                text: format!("CL {}", creabim_core::site_level(model, lv.cover[*k])),
                size: crate::parts::GROUND_TEXT_MM,
                align: crate::paper::Anchor::Start,
                mm: 0.1,
                rgb: None,
                bold: false,
            });
            // red where the pipe would come out of the ground
            let out_of_ground = lv.cover[*k] < lv.invert[*k] + d.diameter + 2.0 * PIPE_WALL;
            out.push(PlanItem::Text {
                angle: 0.0,
                id,
                at,
                text: format!("IL {}", creabim_core::site_level(model, lv.invert[*k])),
                size: crate::parts::GROUND_TEXT_MM,
                align: crate::paper::Anchor::Start,
                mm: 0.1,
                rgb: out_of_ground.then_some([0.85, 0.15, 0.10]),
                bold: out_of_ground,
            });
        }
    }
}

/// A tank in plan: its outline outside its walls, dashed (it is under
/// the ground), its inside, crossed for a soakaway, and its name and
/// size with its levels.
pub fn tank_plan(
    model: &Model,
    out: &mut Vec<PlanItem>,
    id: ElementId,
    s: &Site,
    t: &Tank,
    levels: bool,
) {
    let outside = t.outline();
    ring(out, id, Pen::Hidden, &outside);
    let inside = t.rect(0.0);
    ring(out, id, Pen::Fine, &inside);
    if t.service == TankUse::Soakaway {
        line(out, id, Pen::Fine, inside[0], inside[2]);
        line(out, id, Pen::Fine, inside[1], inside[3]);
    }
    if !levels {
        return;
    }
    let lv = tank_levels(model, s, t);
    let text = |at: DVec2, text: String, bold: bool| PlanItem::Text {
        angle: 0.0,
        id,
        at,
        text,
        size: crate::parts::GROUND_TEXT_MM,
        align: crate::paper::Anchor::Middle,
        mm: 0.1,
        rgb: None,
        bold,
    };
    out.push(text(
        t.centre + DVec2::Y * 0.3,
        t.service.label().into(),
        true,
    ));
    out.push(text(
        t.centre - DVec2::Y * 0.15,
        format!("{:.1} m³", t.capacity()),
        false,
    ));
    out.push(text(
        t.centre - DVec2::Y * 0.6,
        format!("base {}", creabim_core::site_level(model, lv.floor)),
        false,
    ));
}

/// The ground at `p` before and after the works, as elevations, from the
/// terrain that holds it.
pub(crate) fn grounds_at(model: &Model, p: DVec2) -> Option<(f64, f64)> {
    for e in model.iter() {
        let ElementKind::Site(s) = &e.kind else {
            continue;
        };
        let SiteForm::Terrain { boundary, top, .. } = &s.form else {
            continue;
        };
        if !creabim_core::polygon::contains(boundary, p) {
            continue;
        }
        let base = members::elevation(model, s.level);
        return Some(match crate::earth::shaped(model, s) {
            Some(sh) => {
                let now = sh.ground.height(p).unwrap_or(*top);
                (base + sh.existing.height(p).unwrap_or(now), base + now)
            }
            None => (base + top, base + top),
        });
    }
    None
}

/// Where along a drain's run each of its points is.
pub fn chainages(d: &Drain) -> Vec<f64> {
    let mut gone = 0.0;
    let mut out = vec![0.0];
    for w in d.path.windows(2) {
        gone += w[0].distance(w[1]);
        out.push(gone);
    }
    out
}

/// The long section of drain `id` drawn at 1:`scale`: the run unfolded
/// along its length, the ground over it (the ground as it was dashed
/// where the works moved it), the pipe falling from chamber to chamber,
/// each chamber, and under a datum line a table of each chamber's cover
/// level, invert, depth and chainage, with each length's pipe.
pub fn long_section(model: &Model, id: ElementId, scale: f64) -> crate::section::Drawing {
    let mut items = vec![];
    let Some(e) = model.get(id) else {
        return crate::section::Drawing { items };
    };
    let ElementKind::Site(s) = &e.kind else {
        return crate::section::Drawing { items };
    };
    let SiteForm::Drain(d) = &s.form else {
        return crate::section::Drawing { items };
    };
    if d.path.len() < 2 {
        return crate::section::Drawing { items };
    }
    let mm = scale / 1000.0;
    let units = model.units();
    let lv = drain_levels(model, s, d);
    let ch = chainages(d);
    let length = *ch.last().unwrap_or(&0.0);
    let metres = |l: f64| {
        if model.units().is_imperial() {
            model.units().length(l)
        } else {
            format!("{l:.2} m")
        }
    };
    let text = |at: DVec2, t: String, align, bold: bool, rgb: Option<[f32; 3]>| PlanItem::Text {
        angle: 0.0,
        id,
        at,
        text: t,
        size: 2.5,
        align,
        mm,
        rgb,
        bold,
    };
    // the ground over it, every quarter metre
    let n = ((length / 0.25).ceil() as usize).max(1);
    let mut was = vec![];
    let mut now = vec![];
    for k in 0..=n {
        let t = length * k as f64 / n as f64;
        // the point that far along
        let seg = ch.windows(2).position(|w| t <= w[1] + 1e-9).unwrap_or(0);
        let f = ((t - ch[seg]) / (ch[seg + 1] - ch[seg]).max(1e-9)).clamp(0.0, 1.0);
        let p = d.path[seg].lerp(d.path[seg + 1], f);
        let (a, b) = grounds_at(model, p).unwrap_or_else(|| {
            let z = members::elevation(model, s.level);
            (z, z)
        });
        was.push((t, a));
        now.push((t, b));
    }
    // the datum: a little under the lowest floor, on a round half metre
    let lowest = lv
        .invert
        .iter()
        .map(|z| z - CHAMBER_SUMP)
        .chain(now.iter().map(|q| q.1))
        .fold(f64::INFINITY, f64::min);
    let datum = ((lowest - 0.3) * 2.0).floor() / 2.0;
    let highest = now
        .iter()
        .map(|q| q.1)
        .chain(lv.cover.iter().copied())
        .fold(f64::NEG_INFINITY, f64::max);
    // heights drawn LONG_SECTION_EXAGGERATION times their true scale, as
    // long sections are
    let ex = LONG_SECTION_EXAGGERATION;
    let at = |x: f64, z: f64| DVec2::new(x, datum + (z - datum) * ex);
    let left = -48.0 * mm;
    let right = length + 6.0 * mm;
    // the earth under the ground, tinted, with a hatched band under its line
    let mut earth: Vec<DVec2> = now.iter().map(|q| at(q.0, q.1)).collect();
    earth.push(at(length, datum));
    earth.push(at(0.0, datum));
    items.push(PlanItem::Tint {
        id,
        points: earth,
        rgb: EARTH_TINT,
    });
    let band = 2.5 * mm;
    let mut strip: Vec<DVec2> = now.iter().map(|q| at(q.0, q.1)).collect();
    strip.extend(
        now.iter()
            .rev()
            .map(|q| at(q.0, q.1) - DVec2::Y * band)
            .map(|p| DVec2::new(p.x, p.y.max(datum))),
    );
    items.push(PlanItem::Fill {
        id,
        points: strip,
        hatch: creabim_core::Hatch::Diagonal,
        rgb: Some(EARTH_TINT),
        material: None,
    });
    // a light line every half metre of level, tagged at the left
    let mut z = datum + 0.5;
    let mut grid_top = datum;
    while z <= highest + 0.3 {
        grid_top = z;
        line(
            &mut items,
            id,
            Pen::Contour,
            at(left + 14.0 * mm, z),
            at(right, z),
        );
        items.push(text(
            at(left + 13.0 * mm, z) + DVec2::Y * 0.4 * mm,
            creabim_core::site_level(model, z),
            crate::paper::Anchor::End,
            false,
            Some(MUTED),
        ));
        z += 0.5;
    }
    // the ground as it was, dashed, where the works moved it
    for k in 0..n {
        let moved = |q: usize| (was[q].1 - now[q].1).abs() > 0.02;
        if moved(k) && moved(k + 1) {
            line(
                &mut items,
                id,
                Pen::Existing,
                at(was[k].0, was[k].1),
                at(was[k + 1].0, was[k + 1].1),
            );
        }
    }
    // the ground as it is to be, heavy
    for k in 0..n {
        line(
            &mut items,
            id,
            Pen::Cut,
            at(now[k].0, now[k].1),
            at(now[k + 1].0, now[k + 1].1),
        );
    }
    // the pipe, its invert and its soffit
    let top = d.diameter;
    let half = if d.chambers {
        d.chamber / 2.0 + CHAMBER_WALL
    } else {
        0.0
    };
    let chambers = chamber_points(d);
    for i in 0..d.path.len() - 1 {
        let gap = |k: usize| if chambers.contains(&k) { half } else { 0.0 };
        let (x0, x1) = (ch[i] + gap(i), ch[i + 1] - gap(i + 1));
        if x1 <= x0 {
            continue;
        }
        let inv = |x: f64| {
            let f = (x - ch[i]) / (ch[i + 1] - ch[i]).max(1e-9);
            lv.invert[i] + (lv.invert[i + 1] - lv.invert[i]) * f
        };
        // the bore drawn at its true size, not stretched with the heights
        let up = DVec2::Y * top;
        let pipe = vec![
            at(x0, inv(x0)),
            at(x1, inv(x1)),
            at(x1, inv(x1)) + up,
            at(x0, inv(x0)) + up,
        ];
        items.push(PlanItem::Tint {
            id,
            points: pipe.clone(),
            rgb: drain_color(d),
        });
        line(&mut items, id, Pen::Cut, pipe[0], pipe[1]);
        line(&mut items, id, Pen::Projection, pipe[3], pipe[2]);
    }
    // each chamber, from its cover down to its floor, named over its cover
    for (j, k) in chambers.iter().enumerate() {
        let (x, c, i) = (ch[*k], lv.cover[*k], lv.invert[*k]);
        let g = now[((x / length.max(1e-9)) * n as f64).round() as usize].1;
        let floor = i - CHAMBER_SUMP;
        let pts = [
            at(x - half, c),
            at(x + half, c),
            at(x + half, floor),
            at(x - half, floor),
        ];
        items.push(PlanItem::Tint {
            id,
            points: pts.to_vec(),
            rgb: [1.0, 1.0, 1.0],
        });
        ring(&mut items, id, Pen::Projection, &pts);
        let inside = d.chamber / 2.0;
        for sx in [-inside, inside] {
            line(&mut items, id, Pen::Fine, at(x + sx, c), at(x + sx, floor));
        }
        items.push(text(
            at(x, c.max(g)) + DVec2::Y * 3.0 * mm,
            format!("{}{}", d.service.letter(), j + 1),
            crate::paper::Anchor::Middle,
            true,
            None,
        ));
        // a light line down from its floor to the datum
        line(&mut items, id, Pen::Contour, at(x, floor), at(x, datum));
    }
    // the datum
    line(
        &mut items,
        id,
        Pen::Projection,
        at(left, datum),
        at(right, datum),
    );
    items.push(text(
        at(left, datum) + DVec2::new(1.5 * mm, 1.5 * mm),
        format!("Datum {}", creabim_core::site_level(model, datum)),
        crate::paper::Anchor::Start,
        false,
        None,
    ));
    // the table under it, its heads in a column of their own
    let row = 7.0 * mm;
    let heads = [
        "Chamber",
        "Cover level",
        "Invert level",
        "Depth",
        "Chainage",
        "Pipe",
    ];
    let base = at(0.0, datum).y;
    let bottom = base - row * heads.len() as f64;
    for (r, h) in heads.iter().enumerate() {
        let y = base - row * (r as f64 + 1.0);
        line(
            &mut items,
            id,
            if r + 1 == heads.len() {
                Pen::Projection
            } else {
                Pen::Fine
            },
            DVec2::new(left, y),
            DVec2::new(right, y),
        );
        items.push(text(
            DVec2::new(left + 1.5 * mm, y + 2.4 * mm),
            h.to_string(),
            crate::paper::Anchor::Start,
            true,
            None,
        ));
    }
    for x in [left, left + 22.0 * mm, right] {
        line(
            &mut items,
            id,
            Pen::Projection,
            DVec2::new(x, base),
            DVec2::new(x, bottom),
        );
    }
    let red = Some([0.8, 0.12, 0.08]);
    for (k, x) in ch.iter().enumerate() {
        let name = match chambers.iter().position(|c| *c == k) {
            Some(n) => format!("{}{}", d.service.letter(), n + 1),
            None => "-".into(),
        };
        let deep = lv.cover[k] - lv.invert[k];
        let values = [
            (name, None),
            (creabim_core::site_level(model, lv.cover[k]), None),
            (creabim_core::site_level(model, lv.invert[k]), None),
            if deep < d.diameter {
                ("out of ground".to_string(), red)
            } else {
                (units.length(deep), None)
            },
            (format!("{x:.2}"), None),
        ];
        for (r, (v, rgb)) in values.into_iter().enumerate() {
            let y = base - row * (r as f64 + 1.0) + 2.4 * mm;
            items.push(text(
                DVec2::new(*x, y),
                v,
                crate::paper::Anchor::Middle,
                r == 0,
                rgb,
            ));
        }
    }
    for i in 0..d.path.len() - 1 {
        items.push(text(
            DVec2::new((ch[i] + ch[i + 1]) / 2.0, bottom + 2.4 * mm),
            format!(
                "Ø{:.0} at {}, {}",
                d.diameter * 1000.0,
                d.gradient_text(),
                metres(ch[i + 1] - ch[i])
            ),
            crate::paper::Anchor::Middle,
            false,
            None,
        ));
    }
    let title_y = at(0.0, highest.max(grid_top)).y + 14.0 * mm;
    items.push(PlanItem::Text {
        angle: 0.0,
        id,
        at: DVec2::new(left, title_y),
        text: format!("{}: long section", e.name),
        size: 3.5,
        align: crate::paper::Anchor::Start,
        mm,
        rgb: None,
        bold: true,
    });
    items.push(text(
        DVec2::new(left, title_y - 5.0 * mm),
        format!(
            "{}, {} long, falling {}. Heights drawn {}× their scale.",
            d.service.label(),
            metres(length),
            d.gradient_text(),
            ex
        ),
        crate::paper::Anchor::Start,
        false,
        Some(MUTED),
    ));
    crate::section::Drawing { items }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::{Element, Level};

    fn level() -> Element {
        Element::new(
            "Ground",
            ElementKind::Level(Level {
                elevation: 100.0,
                height: 3.0,
            }),
        )
    }

    #[test]
    fn a_drain_falls_from_its_head_under_the_ground() {
        let lv = level();
        let mut s = Site::drain(
            lv.id,
            vec![DVec2::ZERO, DVec2::new(16.0, 0.0)],
            DrainUse::Foul,
        );
        if let SiteForm::Drain(d) = &mut s.form {
            d.fall = 2.5;
        }
        let d = s.form.clone();
        let SiteForm::Drain(d) = d else {
            unreachable!()
        };
        let m = Model::from_elements([lv, Element::new("Drain", ElementKind::Site(s.clone()))])
            .unwrap();
        // no terrain: the ground is the level, 100
        let l = drain_levels(&m, &s, &d);
        assert!((l.invert[0] - 99.4).abs() < 1e-9);
        assert!((l.invert[1] - 99.0).abs() < 1e-9);
        let (c, at) = l.least_cover(&d).unwrap();
        assert_eq!(at, 0);
        assert!((c - 0.48).abs() < 1e-9, "{c}");
        // its trench: 0.6 wide, from 0.71 to 1.11 deep, 16 long
        let dig = drain_dig(&m, &s, &d);
        assert!(dig.dug > 0.6 * 0.91 * 16.0, "{dig:?}");
        assert!(dig.backfill > 0.0 && dig.backfill < dig.dug);
        assert!((dig.deepest - (1.0 + 0.15 + 0.1)).abs() < 1e-9, "{dig:?}");
        let meshes = drain_meshes(&m, &s, &d);
        assert_eq!(meshes.len(), 2);
        let lowest = meshes[0]
            .0
            .positions
            .iter()
            .map(|p| p[2])
            .fold(f32::MAX, f32::min);
        assert!((lowest as f64 - 98.99).abs() < 1e-3, "{lowest}");
    }

    #[test]
    fn a_tank_is_dug_in_with_room_round_it() {
        let lv = level();
        let s = Site::tank(lv.id, DVec2::new(5.0, 5.0), TankUse::Septic);
        let SiteForm::Tank(t) = s.form.clone() else {
            unreachable!()
        };
        let m =
            Model::from_elements([lv, Element::new("Tank", ElementKind::Site(s.clone()))]).unwrap();
        let l = tank_levels(&m, &s, &t);
        assert!((l.top - 99.7).abs() < 1e-9);
        assert!((l.floor - (99.7 - 0.15 - 1.5)).abs() < 1e-9);
        let dig = tank_dig(&m, &s, &t);
        // 3.7 by 2.5, down 2.2 to under its floor and blinding
        assert!((dig.dug - 3.7 * 2.5 * 2.2).abs() < 1e-9, "{dig:?}");
        assert!(dig.backfill > 0.0);
        let mut out = vec![];
        tank_plan(&m, &mut out, ElementId::new(), &s, &t, true);
        assert!(out
            .iter()
            .any(|i| matches!(i, PlanItem::Text { text, .. } if text == "Septic tank")));
    }

    #[test]
    fn a_long_section_tables_each_chamber_and_a_schedule_lists_them() {
        let lv = level();
        let s = Site::drain(
            lv.id,
            vec![DVec2::ZERO, DVec2::new(8.0, 0.0), DVec2::new(8.0, 4.0)],
            DrainUse::Foul,
        );
        let de = Element::new("Run A", ElementKind::Site(s));
        let id = de.id;
        let t = Element::new(
            "Tank",
            ElementKind::Site(Site::tank(lv.id, DVec2::new(8.0, 6.0), TankUse::Septic)),
        );
        let m = Model::from_elements([lv, de, t]).unwrap();
        let d = long_section(&m, id, 100.0);
        let texts: Vec<&str> = d
            .items
            .iter()
            .filter_map(|i| match i {
                PlanItem::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        for want in ["F1", "F3", "Invert level", "12.00"] {
            assert!(texts.contains(&want), "{want} in {texts:?}");
        }
        let sch = crate::schedule::schedule_by(&m, creabim_core::ScheduleCategory::Drainage, false);
        // three chambers and the tank
        assert_eq!(sch.rows.len(), 4, "{:?}", sch.rows);
        assert_eq!(sch.rows[0].cells[0], "F1");
        assert!(sch.rows[3].cells[2].starts_with("Septic tank"));
    }
}

/// A road gully: where it is, the way the road runs there, the level of
/// its grating, and the surface water chamber it drains to (the drain,
/// the chamber's number and where it is), if one is near.
#[derive(Clone, Debug, PartialEq)]
pub struct Gully {
    pub at: DVec2,
    pub dir: DVec2,
    pub cover: f64,
    pub to: Option<(ElementId, usize, DVec2)>,
}

/// How far a gully reaches for a chamber to drain to, metres.
pub const GULLY_REACH: f64 = 12.0;

/// A road's gullies, each joined to the nearest chamber of a surface
/// water or combined drain within [`GULLY_REACH`].
pub fn gullies(model: &Model, s: &Site, r: &creabim_core::road::Road) -> Vec<Gully> {
    let base = members::elevation(model, s.level);
    let plan = r.plan();
    // the chambers water off a road can go to
    let mut chambers: Vec<(ElementId, usize, DVec2)> = vec![];
    for e in model.iter() {
        let ElementKind::Site(o) = &e.kind else {
            continue;
        };
        let SiteForm::Drain(d) = &o.form else {
            continue;
        };
        if !matches!(d.service, DrainUse::Surface | DrainUse::Combined) {
            continue;
        }
        for (n, k) in chamber_points(d).iter().enumerate() {
            chambers.push((e.id, n, d.path[*k]));
        }
    }
    r.gully_points()
        .into_iter()
        .map(|g| {
            let at = g.truncate();
            // the way the road runs at its nearest stretch
            let dir = plan
                .windows(2)
                .min_by(|a, b| {
                    seg_distance(at, a[0], a[1]).total_cmp(&seg_distance(at, b[0], b[1]))
                })
                .map_or(DVec2::X, |w| (w[1] - w[0]).normalize_or(DVec2::X));
            let to = chambers
                .iter()
                .filter(|c| c.2.distance(at) <= GULLY_REACH)
                .min_by(|a, b| a.2.distance(at).total_cmp(&b.2.distance(at)))
                .copied();
            Gully {
                at,
                dir,
                cover: base + g.z,
                to,
            }
        })
        .collect()
}

fn seg_distance(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// A road's channels and gullies in plan: the channel by each low kerb,
/// each gully a grating with its bars, and a dashed connection from it
/// to the chamber it drains to; with `levels`, each gully's name.
pub fn gullies_plan(
    model: &Model,
    out: &mut Vec<PlanItem>,
    id: ElementId,
    s: &Site,
    r: &creabim_core::road::Road,
    levels: bool,
) {
    for ch in r.channels() {
        for w in ch.windows(2) {
            line(out, id, Pen::Fine, w[0], w[1]);
        }
    }
    for (n, g) in gullies(model, s, r).iter().enumerate() {
        let (u, v) = (g.dir * 0.225, g.dir.perp() * 0.15);
        let sq = [g.at - u - v, g.at + u - v, g.at + u + v, g.at - u + v];
        out.push(PlanItem::Tint {
            id,
            points: sq.to_vec(),
            rgb: [0.25, 0.25, 0.26],
        });
        ring(out, id, Pen::Projection, &sq);
        for k in 1..4 {
            let t = k as f64 / 4.0;
            line(
                out,
                id,
                Pen::Fine,
                sq[0].lerp(sq[1], t),
                sq[3].lerp(sq[2], t),
            );
        }
        if let Some((_, _, c)) = g.to {
            line(out, id, Pen::Hidden, g.at, c);
        }
        if levels {
            out.push(crate::parts::ground_text(
                id,
                g.at + g.dir.perp() * 0.5 * if g.dir.perp().y < 0.0 { -1.0 } else { 1.0 },
                format!("G{}", n + 1),
            ));
        }
    }
}

/// A road's gully gratings in 3D, on its surface.
pub fn gully_meshes(model: &Model, s: &Site, r: &creabim_core::road::Road) -> Option<TriMesh> {
    let gs = gullies(model, s, r);
    if gs.is_empty() {
        return None;
    }
    let mut m = TriMesh::default();
    for g in gs {
        let (u, v) = (g.dir * 0.225, g.dir.perp() * 0.15);
        let sq = [g.at - u - v, g.at + u - v, g.at + u + v, g.at - u + v];
        m.append(&crate::scene::slab_mesh(&sq, g.cover + 0.01, 0.03));
    }
    Some(m)
}
