//! Roof windows: windows set in a roof's slope and hosted by the roof.
//!
//! A roof window is an [`Opening`] whose host is a roof and whose
//! [`Opening::roof_at`] is its centre in plan. Its width runs across the
//! slope and its height up it. It is built as a window in a wall would be,
//! in its own coordinates (x across, y into the roof, z up the slope),
//! which [`Seat::at`] turns into the model's.

use glam::{DVec2, DVec3};

use crate::{ElementId, ElementKind, Model, Opening, Roof};

/// Where a roof window sits: the bottom corner of its outline at the start
/// of its width, on the roof's middle plane, and the directions across the
/// slope, out of the roof (square to it) and up the slope.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seat {
    pub origin: DVec3,
    pub across: DVec3,
    pub out: DVec3,
    pub up: DVec3,
    /// The roof's thickness, square to its slope.
    pub thickness: f64,
}

impl Seat {
    /// A point given in the window's own coordinates (x across the slope,
    /// y into the roof, z up the slope), in the model. Like a wall's
    /// window, its outside is towards -y.
    pub fn at(&self, p: DVec3) -> DVec3 {
        self.origin + self.across * p.x - self.out * p.y + self.up * p.z
    }

    /// The slope it sits on, in degrees from level.
    pub fn pitch(&self) -> f64 {
        self.up.z.clamp(-1.0, 1.0).asin().to_degrees()
    }
}

/// The roof hosting a roof window; None for a door or window in a wall.
pub fn roof_of<'a>(model: &'a Model, o: &Opening) -> Option<&'a Roof> {
    o.roof_at?;
    match &model.get(o.host)?.kind {
        ElementKind::Roof(r) => Some(r),
        _ => None,
    }
}

/// Where a roof window sits on its roof.
pub fn seat(model: &Model, o: &Opening) -> Option<Seat> {
    let r = roof_of(model, o)?;
    seat_on(r, r.base(model), o.roof_at?, o.width, o.height)
}

/// Where a window `w` wide and `h` long (up the slope), centred on `at` in
/// plan, sits on roof `r` whose base is at `base`: on the slope over that
/// point.
pub fn seat_on(r: &Roof, base: f64, at: DVec2, w: f64, h: f64) -> Option<Seat> {
    let faces = r.faces();
    // the slope over the point; outside every one (on the overhang), the
    // underside there
    let f = faces
        .iter()
        .find(|f| f.outline.len() >= 3 && crate::polygon::contains(&f.outline, at))
        .or_else(|| faces.iter().min_by(|a, b| a.z(at).total_cmp(&b.z(at))))?;
    let g = f.g;
    let (up, out) = if g.length() < 1e-9 {
        (DVec3::Y, DVec3::Z)
    } else {
        let d = g.normalize();
        (
            DVec3::new(d.x, d.y, g.length()).normalize(),
            DVec3::new(-g.x, -g.y, 1.0).normalize(),
        )
    };
    let across = (-out).cross(up);
    let vertical = r.thickness / out.z.max(1e-6);
    let centre = at.extend(base + f.z(at) + vertical / 2.0);
    Some(Seat {
        origin: centre - across * (w / 2.0) - up * (h / 2.0),
        across,
        out,
        up,
        thickness: r.thickness,
    })
}

/// A roof window's outline in plan, counter-clockwise: the hole it needs
/// in its roof, seen from above.
pub fn footprint(model: &Model, o: &Opening) -> Option<Vec<DVec2>> {
    let s = seat(model, o)?;
    Some(
        [
            (0.0, 0.0),
            (o.width, 0.0),
            (o.width, o.height),
            (0.0, o.height),
        ]
        .iter()
        .map(|&(x, z)| s.at(DVec3::new(x, 0.0, z)).truncate())
        .collect(),
    )
}

/// Where a new roof window goes on roof `r`: the middle of its largest
/// slope (of its largest face, on a flat roof).
pub fn first_spot(r: &Roof) -> Option<DVec2> {
    let faces = r.faces();
    let area = |f: &crate::members::RoofFace| crate::polygon::signed_area2(&f.outline).abs();
    let f = faces
        .iter()
        .filter(|f| f.g.length() > 1e-6 && f.outline.len() >= 3)
        .max_by(|a, b| area(a).total_cmp(&area(b)))
        .or_else(|| faces.iter().max_by(|a, b| area(a).total_cmp(&area(b))))?;
    // its centroid
    let p = &f.outline;
    let (mut c, mut a) = (DVec2::ZERO, 0.0);
    for i in 0..p.len() {
        let (u, v) = (p[i], p[(i + 1) % p.len()]);
        let k = u.perp_dot(v);
        a += k;
        c += (u + v) * k;
    }
    Some(if a.abs() < 1e-12 {
        p.iter().copied().sum::<DVec2>() / p.len() as f64
    } else {
        c / (3.0 * a)
    })
}

/// Where a window `w` × `h` can go on `roof` clear of the windows it
/// already has: the middle of its largest slope, or failing that the
/// nearest spot along the slope's width, stepping a window and a gap at a
/// time either side; None when the slope is full.
pub fn free_spot(model: &Model, roof: ElementId, w: f64, h: f64) -> Option<DVec2> {
    let ElementKind::Roof(r) = &model.get(roof)?.kind else {
        return None;
    };
    let base = r.base(model);
    let start = first_spot(r)?;
    let taken = footprints(model, roof);
    let outline = |at: DVec2| -> Option<Vec<DVec2>> {
        let s = seat_on(r, base, at, w, h)?;
        Some(
            [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)]
                .iter()
                .map(|&(x, z)| s.at(DVec3::new(x, 0.0, z)).truncate())
                .collect(),
        )
    };
    let s = seat_on(r, base, start, w, h)?;
    let across = s.across.truncate().normalize_or_zero();
    // the slope it starts on, which it must stay wholly within
    let face = r
        .faces()
        .into_iter()
        .find(|f| f.outline.len() >= 3 && crate::polygon::contains(&f.outline, start))?;
    let step = w + 0.4;
    for k in 0..40 {
        let n = ((k + 1) / 2) as f64 * if k % 2 == 1 { 1.0 } else { -1.0 };
        let at = start + across * step * n;
        let Some(p) = outline(at) else { continue };
        let inside = p
            .iter()
            .all(|q| crate::polygon::contains(&face.outline, *q));
        if inside && taken.iter().all(|t| !crate::polygon::convex_overlap(&p, t)) {
            return Some(at);
        }
    }
    None
}

/// A new roof window on `roof`: 780 × 1400, turning on centre pivots,
/// in the middle of its largest slope, or beside the windows already
/// there.
pub fn new_on(model: &Model, roof: ElementId) -> Option<Opening> {
    if !matches!(model.get(roof)?.kind, ElementKind::Roof(_)) {
        return None;
    }
    let mut frame = crate::OpeningFrame::for_kind(crate::OpeningKind::Window);
    frame.grid = crate::cells::WindowStyle::HorizontalPivot.grid(false, [1, 1]);
    Some(Opening {
        host: roof,
        kind: crate::OpeningKind::Window,
        offset: 0.0,
        sill: 0.0,
        width: 0.78,
        height: 1.4,
        material: None,
        frame,
        flip_hand: false,
        flip_facing: false,
        roof_at: Some(free_spot(model, roof, 0.78, 1.4)?),
        roof_build: Default::default(),
        arrangement: None,
    })
}

/// The roof windows a roof hosts.
pub fn windows_in(model: &Model, roof: ElementId) -> Vec<(ElementId, &Opening)> {
    model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Opening(o) if o.host == roof && o.roof_at.is_some() => Some((e.id, o)),
            _ => None,
        })
        .collect()
}

/// The outlines in plan of the roof windows a roof hosts.
pub fn footprints(model: &Model, roof: ElementId) -> Vec<Vec<DVec2>> {
    windows_in(model, roof)
        .into_iter()
        .filter_map(|(_, o)| footprint(model, o))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Element, Level, OpeningFrame, OpeningKind, RoofShape, Transaction};

    /// A 10 × 6 gable roof at 30°, its ridge along x, with a window on its
    /// south slope.
    fn roofed() -> (Model, ElementId, ElementId) {
        let mut m = Model::new();
        let l = Element::new(
            "Level",
            ElementKind::Level(Level {
                elevation: 3.0,
                height: 3.0,
            }),
        );
        let mut r = Roof::new(
            l.id,
            crate::polygon::rectangle(DVec2::ZERO, DVec2::new(10.0, 6.0)),
            RoofShape::Gable,
        );
        r.pitch = 30.0;
        r.overhang = 0.0;
        r.thickness = 0.3;
        r.edge = 0;
        let roof = Element::new("Roof", ElementKind::Roof(r));
        let w = Element::new(
            "Roof window",
            ElementKind::Opening(Opening {
                host: roof.id,
                kind: OpeningKind::Window,
                offset: 0.0,
                sill: 0.0,
                width: 0.78,
                height: 1.4,
                material: None,
                frame: OpeningFrame::for_kind(OpeningKind::Window),
                flip_hand: false,
                flip_facing: false,
                roof_at: Some(DVec2::new(5.0, 1.5)),
                roof_build: Default::default(),
                arrangement: None,
            }),
        );
        let (rid, wid) = (roof.id, w.id);
        Transaction::new("t")
            .insert(l)
            .insert(roof)
            .insert(w)
            .apply(&mut m)
            .unwrap();
        (m, rid, wid)
    }

    #[test]
    fn a_roof_window_sits_in_its_slope_square_to_it() {
        let (m, roof, win) = roofed();
        let ElementKind::Opening(o) = &m.get(win).unwrap().kind else {
            panic!()
        };
        let s = seat(&m, o).unwrap();
        // on the south slope it runs up towards the ridge, at the pitch
        assert!((s.pitch() - 30.0).abs() < 1e-6, "{}", s.pitch());
        assert!(s.up.y > 0.0 && s.out.y < 0.0 && s.out.z > 0.0);
        assert!(s.across.dot(s.up).abs() < 1e-9 && s.across.dot(s.out).abs() < 1e-9);
        assert!((s.across.x - 1.0).abs() < 1e-9);
        // its centre is over the point it was placed at
        let c = s.at(DVec3::new(o.width / 2.0, 0.0, o.height / 2.0));
        assert!(c.truncate().distance(DVec2::new(5.0, 1.5)) < 1e-9);
        // its outline in plan is 0.78 across and 1.4 × cos 30° up the slope
        let f = footprint(&m, o).unwrap();
        assert!((f[1].x - f[0].x - 0.78).abs() < 1e-9);
        let run = 1.4 * 30f64.to_radians().cos();
        assert!((f[3].y - f[0].y - run).abs() < 1e-9);
        assert!(crate::polygon::signed_area2(&f) > 0.0);
        assert_eq!(footprints(&m, roof).len(), 1);
    }

    #[test]
    fn a_new_roof_window_goes_beside_those_already_there() {
        let (mut m, roof, _) = roofed();
        for n in 2..=4 {
            let o = new_on(&m, roof).unwrap();
            Transaction::new("add")
                .insert(Element::new("Roof window", ElementKind::Opening(o)))
                .apply(&mut m)
                .unwrap();
            let f = footprints(&m, roof);
            assert_eq!(f.len(), n);
            for i in 0..f.len() {
                for j in 0..i {
                    assert!(!crate::polygon::convex_overlap(&f[i], &f[j]), "{i} {j}");
                }
            }
        }
    }
}
