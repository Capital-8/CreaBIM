use crate::TriMesh;
use creabim_core::walls::TopProfile;
use creabim_core::{Opening, Wall, WallEnds};
use glam::{DVec2, DVec3};

/// Rectangles in wall-local coordinates: `u` along the wall, `v` up from its base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cut {
    pub u0: f64,
    pub u1: f64,
    pub v0: f64,
    pub v1: f64,
}

impl Cut {
    pub fn of(o: &Opening, wall: &Wall) -> Option<Cut> {
        let c = Cut {
            u0: o.offset.max(0.0),
            u1: (o.offset + o.width).min(wall.length()),
            v0: o.sill.max(0.0),
            v1: (o.sill + o.height).min(wall.height),
        };
        (c.u1 > c.u0 + 1e-9 && c.v1 > c.v0 + 1e-9).then_some(c)
    }

    fn contains(&self, u: f64, v: f64) -> bool {
        u > self.u0 && u < self.u1 && v > self.v0 && v < self.v1
    }
}

/// An arched or raked opening's head in wall coordinates: the points
/// `(u, v)` of its edge from one springing to the other, straight
/// between them. None for a square-headed opening.
pub fn head_of(o: &Opening, wall: &Wall) -> Option<Vec<(f64, f64)>> {
    let c = Cut::of(o, wall)?;
    let outline = o.frame.grid.outline;
    if outline.is_rectangle() {
        return None;
    }
    let (w, h) = (c.u1 - c.u0, c.v1 - c.v0);
    let outline = outline.fitted(w, h);
    if outline.is_rectangle() {
        return None;
    }
    Some(
        outline
            .head(w, h, 0.0)
            .into_iter()
            .map(|(x, z)| (c.u0 + x, c.v0 + z))
            .collect(),
    )
}

/// The part of the rectangle `u0..u1` × `v0..v1` above the straight line
/// through `(u0, a)` and `(u1, b)`, as a convex polygon anticlockwise in
/// (u, v), with each edge marked by which side of the rectangle it lies
/// on (0 left, 1 right, 2 bottom, 3 top) or 4 for the line.
fn above_chord(u0: f64, u1: f64, v0: f64, v1: f64, a: f64, b: f64) -> Vec<((f64, f64), u8)> {
    // the rectangle's corners, each followed by the side its next edge runs along
    let rect = [((u0, v0), 2u8), ((u1, v0), 1), ((u1, v1), 3), ((u0, v1), 0)];
    let line = |u: f64| a + (b - a) * (u - u0) / (u1 - u0);
    let inside = |p: (f64, f64)| p.1 >= line(p.0) - 1e-12;
    let mut out: Vec<((f64, f64), u8)> = vec![];
    for k in 0..4 {
        let (p, side) = rect[k];
        let (q, _) = rect[(k + 1) % 4];
        let (pi, qi) = (inside(p), inside(q));
        // a point the same as the last one says where the edge from there runs
        let mut push = |pt: (f64, f64), m: u8| match out.last_mut() {
            Some(l) if (l.0 .0 - pt.0).abs() < 1e-12 && (l.0 .1 - pt.1).abs() < 1e-12 => l.1 = m,
            _ => out.push((pt, m)),
        };
        if pi {
            push(p, side);
        }
        if pi != qi {
            // where this edge crosses the line
            let (fp, fq) = (p.1 - line(p.0), q.1 - line(q.0));
            let t = fp / (fp - fq);
            let x = (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t);
            // leaving: the next edge runs along the line; entering: along this side
            push(x, if pi { 4 } else { side });
        }
    }
    if out.len() > 1 {
        let (f, l) = (out[0].0, out[out.len() - 1].0);
        if (f.0 - l.0).abs() < 1e-12 && (f.1 - l.1).abs() < 1e-12 {
            out.pop();
        }
    }
    out
}

/// Build a closed solid for a straight wall with rectangular openings.
///
/// The wall face is split into a grid at every opening edge; each grid cell is
/// either solid or void. Solid cells get front and back faces, and every
/// boundary between a solid cell and void (or the wall's outer edge) gets a
/// side face, so ends, top, bottom and opening reveals all come out closed.
/// `ends` mitres or trims the two ends where the wall joins its neighbours
/// and leaves out the gaps where walls cross it; with `top`, the top
/// follows the undersides it is attached to instead of being level.
pub fn wall_mesh(
    wall: &Wall,
    base_z: f64,
    openings: &[&Opening],
    ends: &WallEnds,
    top: Option<&TopProfile>,
) -> TriMesh {
    let t = wall.thickness / 2.0;
    wall_layer_mesh(wall, base_z, openings, ends, top, (-t, t), false, [0.0; 2])
}

/// One layer of a layered wall as a closed solid: the part of the wall
/// between `band.0` and `band.1` across it (left positive), its ends on
/// the wall's end lines, or past them for a `core` layer that runs on,
/// and `ext` past them (start, end) where its ends were moved by hand.
#[allow(clippy::too_many_arguments)]
pub fn wall_layer_mesh(
    wall: &Wall,
    base_z: f64,
    openings: &[&Opening],
    ends: &WallEnds,
    top: Option<&TopProfile>,
    band: (f64, f64),
    core: bool,
    ext: [f64; 2],
) -> TriMesh {
    wall_layer_mesh_v(wall, base_z, openings, ends, top, band, core, ext, [0.0; 2])
}

/// [`wall_layer_mesh`], its bottom run down `vext[0]` past the wall's base
/// and its top up `vext[1]` past the wall's top (A7-10; negative stops
/// short).
#[allow(clippy::too_many_arguments)]
pub fn wall_layer_mesh_v(
    wall: &Wall,
    base_z: f64,
    openings: &[&Opening],
    ends: &WallEnds,
    top: Option<&TopProfile>,
    band: (f64, f64),
    core: bool,
    ext: [f64; 2],
    vext: [f64; 2],
) -> TriMesh {
    let len = wall.length();
    let h = wall.height;
    // an arched or raked opening cuts a rectangle up to its springing,
    // and its head above in straight pieces (`heads`)
    let heads: Vec<(Cut, Vec<(f64, f64)>)> = openings
        .iter()
        .filter_map(|o| Some((Cut::of(o, wall)?, head_of(o, wall)?)))
        .collect();
    let cuts: Vec<Cut> = openings
        .iter()
        .filter_map(|o| {
            let c = Cut::of(o, wall)?;
            Some(match head_of(o, wall) {
                Some(pts) => Cut {
                    v1: pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min),
                    ..c
                },
                None => c,
            })
        })
        .collect();
    // pocket doors' cavities, hollowing the wall beside their openings
    // between two skins
    let pockets: Vec<(Cut, (f64, f64))> = openings
        .iter()
        .flat_map(|o| crate::opening::pockets(o, wall))
        .collect();

    let mut us = vec![0.0, len];
    // a curved wall bends at even steps round its arc (MU40)
    us.extend(wall.bends());
    let mut vs = vec![0.0, h];
    for c in &cuts {
        us.extend([c.u0, c.u1]);
        vs.extend([c.v0, c.v1]);
    }
    for (c, _) in &pockets {
        us.extend([c.u0, c.u1]);
        vs.extend([c.v0, c.v1]);
    }
    for (c, pts) in &heads {
        us.extend(pts.iter().map(|p| p.0));
        vs.extend(pts.iter().map(|p| p.1));
        vs.push(c.v1);
    }
    for g in &ends.gaps {
        us.extend([g.lo_mid(), g.hi_mid()]);
    }
    if let Some(top) = top {
        // where the top bends over a ridge or steps at an eave
        // each break twice, a millimetre apart: where the top changes
        // height it steps, not slopes (MU20)
        us.extend(
            top.breaks(wall.start, wall.end)
                .into_iter()
                .flat_map(|k| [k * len - 0.001, k * len + 0.001])
                .filter(|u| *u > 0.0 && *u < len),
        );
    }
    let sort = |xs: &mut Vec<f64>| {
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    };
    sort(&mut us);
    sort(&mut vs);
    // the end lines are the wall's ends exactly: an opening run to an end
    // a rounding error short of it must not stand in for it, or the end
    // is drawn square instead of joined (owner MU108)
    if let Some(first) = us.first_mut() {
        *first = 0.0;
    }
    if let Some(last) = us.last_mut() {
        *last = len;
    }

    let (ni, nj) = (us.len() - 1, vs.len() - 1);
    let (lo, hi) = band;
    // the head over cell (i, j), if the cell is in an arched or raked
    // head: which head, and its edge's heights at the cell's sides
    let in_head = |i: isize, j: isize| -> Option<(usize, f64, f64)> {
        if i < 0 || j < 0 || i as usize >= ni || j as usize >= nj {
            return None;
        }
        let (i, j) = (i as usize, j as usize);
        let (u, v) = ((us[i] + us[i + 1]) / 2.0, (vs[j] + vs[j + 1]) / 2.0);
        heads.iter().enumerate().find_map(|(k, (c, pts))| {
            let lowest = pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
            (u > c.u0 && u < c.u1 && v > lowest && v < c.v1).then(|| {
                (
                    k,
                    creabim_core::cells::head_z_on(pts, us[i]),
                    creabim_core::cells::head_z_on(pts, us[i + 1]),
                )
            })
        })
    };
    // each cell's solid stretches across the band: all of it, none where
    // an opening cuts through, or the skins either side of a pocket
    let solid = |i: isize, j: isize| -> Vec<(f64, f64)> {
        if i < 0 || j < 0 || i as usize >= ni || j as usize >= nj {
            return vec![];
        }
        let (i, j) = (i as usize, j as usize);
        let (u, v) = ((us[i] + us[i + 1]) / 2.0, (vs[j] + vs[j + 1]) / 2.0);
        if ends.in_gap(u) || cuts.iter().any(|c| c.contains(u, v)) {
            return vec![];
        }
        let mut out = vec![(lo, hi)];
        for (c, (w0, w1)) in &pockets {
            if !c.contains(u, v) {
                continue;
            }
            out = out
                .into_iter()
                .flat_map(|(a, b)| [(a, b.min(*w0)), (a.max(*w1), b)])
                .filter(|(a, b)| b - a > 1e-9)
                .collect();
        }
        out
    };
    // the stretches of `a` not in `b`
    let minus = |a: (f64, f64), b: &[(f64, f64)]| -> Vec<(f64, f64)> {
        let mut out = vec![a];
        for &(b0, b1) in b {
            out = out
                .into_iter()
                .flat_map(|(x0, x1)| [(x0, x1.min(b0)), (x0.max(b1), x1)])
                .filter(|(x0, x1)| x1 - x0 > 1e-9)
                .collect();
        }
        out
    };

    let d = wall.direction();
    let n2 = DVec2::new(-d.y, d.x);
    let z0 = base_z + wall.base_offset;
    // an attached top comes down no lower than the heads of openings
    let floor = vs[vs.len().saturating_sub(2)].max(0.0) + 0.01;
    let p = |u: f64, v: f64, w: f64| -> DVec3 {
        // the end grid lines and gap sides follow the joins; a curved
        // wall is laid round its arc, its ends square
        let xy = if wall.is_arc() {
            // its ends as its joins put them, along its tangent there
            if u <= 1e-9 || u >= len - 1e-9 {
                wall.place_end(u > 1e-9, ends.place_layer(wall, u, w, core, ext), w)
            } else {
                wall.place(u, w)
            }
        } else {
            let u = ends.place_layer(wall, u, w, core, ext);
            wall.start + d * u + n2 * w
        };
        let z = match top {
            Some(t) if v >= h - 1e-9 => {
                // an end running into a joined wall, just past the ceiling
                // or roof it is held to, keeps that height rather than
                // rising for a few centimetres (MU20)
                let near = [0.0, 0.05, -0.05, 0.1, -0.1, 0.2, -0.2, 0.3, -0.3]
                    .iter()
                    .filter_map(|o| t.z_at(xy + d * *o))
                    .next();
                near.map_or(z0 + h, |z| z.max(z0 + floor.min(h)))
            }
            _ => z0 + v,
        };
        // a layer run on past the base or the top, or stopped short
        let z = if v <= 1e-9 {
            z - vext[0]
        } else if v >= h - 1e-9 {
            z + vext[1]
        } else {
            z
        };
        DVec3::new(xy.x, xy.y, z)
    };
    let along = DVec3::new(d.x, d.y, 0.0);
    let across = DVec3::new(n2.x, n2.y, 0.0);

    let mut m = TriMesh::default();
    for i in 0..ni as isize {
        for j in 0..nj as isize {
            let cell = solid(i, j);
            if cell.is_empty() {
                continue;
            }
            let (ua, ub) = (us[i as usize], us[i as usize + 1]);
            let (va, vb) = (vs[j as usize], vs[j as usize + 1]);
            // on a curve, the faces turn with it
            let (along, across) = if wall.is_arc() {
                let t = wall.tangent_at((ua + ub) / 2.0);
                (DVec3::new(t.x, t.y, 0.0), DVec3::new(-t.y, t.x, 0.0))
            } else {
                (along, across)
            };
            // in an arched or raked head: solid above its edge only
            if let Some((k, a, b)) = in_head(i, j) {
                let poly = above_chord(ua, ub, va, vb, a, b);
                if poly.len() < 3 {
                    continue;
                }
                let (wa, wb) = (lo, hi);
                let tris: Vec<[usize; 3]> = (1..poly.len() - 1).map(|n| [0, n, n + 1]).collect();
                for (w, n) in [(wb, across), (wa, -across)] {
                    let pts: Vec<DVec3> = poly.iter().map(|((u, v), _)| p(*u, *v, w)).collect();
                    m.add_polygon(&pts, &tris, n);
                }
                // a side shows where nothing solid stands beside it
                let open = |i2: isize, j2: isize| {
                    in_head(i2, j2).is_none_or(|(k2, _, _)| k2 != k) && solid(i2, j2).is_empty()
                };
                for e in 0..poly.len() {
                    let ((u0, v0), side) = poly[e];
                    let ((u1, v1), _) = poly[(e + 1) % poly.len()];
                    let shows = match side {
                        0 => open(i - 1, j),
                        1 => open(i + 1, j),
                        2 => open(i, j - 1),
                        3 => open(i, j + 1),
                        _ => true,
                    };
                    if !shows || ((u1 - u0).abs() < 1e-12 && (v1 - v0).abs() < 1e-12) {
                        continue;
                    }
                    let q = [p(u0, v0, wa), p(u1, v1, wa), p(u1, v1, wb), p(u0, v0, wb)];
                    // facing out of the polygon (it runs anticlockwise in u, v)
                    let (du, dv) = (u1 - u0, v1 - v0);
                    let len = du.hypot(dv).max(1e-12);
                    let n = along * (dv / len) - DVec3::Z * (du / len);
                    m.add_quad(q, n);
                }
                continue;
            }
            let (left, right) = (solid(i - 1, j), solid(i + 1, j));
            let (below, above) = (solid(i, j - 1), solid(i, j + 1));
            for &(wa, wb) in &cell {
                m.add_quad(
                    [p(ua, va, wb), p(ub, va, wb), p(ub, vb, wb), p(ua, vb, wb)],
                    across,
                );
                m.add_quad(
                    [p(ua, va, wa), p(ub, va, wa), p(ub, vb, wa), p(ua, vb, wa)],
                    -across,
                );
                // beside an arched or raked head: open below its edge
                let head_left = in_head(i - 1, j).map(|(_, _, b)| b);
                let head_right = in_head(i + 1, j).map(|(_, a, _)| a);
                for (lo, hi) in minus((wa, wb), &left) {
                    m.add_quad(
                        [p(ua, va, lo), p(ua, va, hi), p(ua, vb, hi), p(ua, vb, lo)],
                        end_normal(p(ua, va, lo), p(ua, va, hi), -along),
                    );
                }
                if let Some(top) = head_left.map(|e| e.min(vb)).filter(|e| *e > va + 1e-12) {
                    m.add_quad(
                        [p(ua, va, wa), p(ua, va, wb), p(ua, top, wb), p(ua, top, wa)],
                        end_normal(p(ua, va, wa), p(ua, va, wb), -along),
                    );
                }
                for (lo, hi) in minus((wa, wb), &right) {
                    m.add_quad(
                        [p(ub, va, lo), p(ub, va, hi), p(ub, vb, hi), p(ub, vb, lo)],
                        end_normal(p(ub, va, lo), p(ub, va, hi), along),
                    );
                }
                if let Some(top) = head_right.map(|e| e.min(vb)).filter(|e| *e > va + 1e-12) {
                    m.add_quad(
                        [p(ub, va, wa), p(ub, va, wb), p(ub, top, wb), p(ub, top, wa)],
                        end_normal(p(ub, va, wa), p(ub, va, wb), along),
                    );
                }
                for (lo, hi) in minus((wa, wb), &below) {
                    m.add_quad(
                        [p(ua, va, lo), p(ub, va, lo), p(ub, va, hi), p(ua, va, hi)],
                        -DVec3::Z,
                    );
                }
                for (lo, hi) in minus((wa, wb), &above) {
                    let q = [p(ua, vb, lo), p(ub, vb, lo), p(ub, vb, hi), p(ua, vb, hi)];
                    // an attached top slopes: shade it as it lies
                    let n = (q[2] - q[0]).cross(q[3] - q[1]).normalize_or(DVec3::Z);
                    m.add_quad(q, if n.z < 0.0 { -n } else { n });
                }
            }
        }
    }
    m
}

/// Normal of a vertical end face running from `a` to `b` in plan, facing `towards`.
fn end_normal(a: DVec3, b: DVec3, towards: DVec3) -> DVec3 {
    let e = b - a;
    let n = DVec3::new(-e.y, e.x, 0.0).normalize_or_zero();
    if n == DVec3::ZERO {
        towards
    } else if n.dot(towards) < 0.0 {
        -n
    } else {
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Owner MU108: a window run to a wall's end that lands a rounding
    /// error short of it (a wall left 1.7999999999999998 long by a bay,
    /// its corner window ending at 1.7999999999999994) still leaves the
    /// end mitred: the end grid line is the wall's end, not the window's.
    #[test]
    fn a_window_a_hair_short_of_the_end_keeps_the_end_mitred() {
        use creabim_core::{Element, ElementKind, Level, Model, Transaction};
        let mut m = Model::default();
        let l = Element::new(
            "L",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let mk = |a: DVec2, b: DVec2| {
            let mut w = wall();
            w.level = l.id;
            w.start = a;
            w.end = b;
            w.thickness = 0.3;
            w.height = 3.0;
            Element::new("W", ElementKind::Wall(w))
        };
        let west = mk(DVec2::new(0.0, 1.7999999999999998), DVec2::ZERO);
        let front = mk(DVec2::ZERO, DVec2::new(10.0, 0.0));
        let wid = west.id;
        Transaction::new("s")
            .insert(l.clone())
            .insert(west)
            .insert(front)
            .apply(&mut m)
            .unwrap();
        let ElementKind::Wall(w) = &m.get(wid).unwrap().kind else {
            unreachable!()
        };
        let o = Opening {
            host: wid,
            kind: creabim_core::OpeningKind::Window,
            offset: 0.39999999999999947,
            sill: 0.9,
            width: 1.4,
            height: 1.4,
            material: None,
            frame: creabim_core::OpeningFrame::for_kind(creabim_core::OpeningKind::Window),
            flip_hand: false,
            flip_facing: false,
            roof_at: None,
            roof_build: Default::default(),
            arrangement: None,
        };
        let ends = creabim_core::joins::wall_joins(&m).remove(&wid).unwrap();
        let mesh = wall_mesh(w, 0.0, &[&o], &ends, None);
        // its outside face runs on to the outer corner, top and bottom
        for z in [0.0f32, 3.0] {
            assert!(
                mesh.positions.iter().any(|p| (p[0] + 0.15).abs() < 1e-4
                    && (p[1] + 0.15).abs() < 1e-4
                    && (p[2] - z).abs() < 1e-4),
                "no outer corner at z {z}"
            );
        }
    }
    use creabim_core::walls::TopFace;
    use creabim_core::{ElementId, Gap, OpeningKind};

    fn wall() -> Wall {
        Wall::new(
            ElementId::new(),
            DVec2::new(1.0, 1.0),
            DVec2::new(1.0, 6.0),
            0.2,
            3.0,
        )
    }

    fn opening(offset: f64, sill: f64, w: f64, h: f64) -> Opening {
        Opening {
            host: ElementId::new(),
            kind: OpeningKind::Window,
            offset,
            sill,
            width: w,
            height: h,
            material: None,
            frame: creabim_core::OpeningFrame::for_kind(OpeningKind::Window),
            flip_hand: false,
            flip_facing: false,
            roof_at: None,
            roof_build: Default::default(),
            arrangement: None,
        }
    }

    #[test]
    fn a_crossing_wall_leaves_a_gap() {
        let ends = WallEnds {
            gaps: vec![Gap {
                lo: [2.0, 2.0],
                hi: [2.3, 2.3],
                core: [0.0; 2],
            }],
            ..Default::default()
        };
        let m = wall_mesh(&wall(), 0.0, &[], &ends, None);
        assert!(
            (m.volume() - 4.7 * 3.0 * 0.2).abs() < 1e-5,
            "{}",
            m.volume()
        );
    }

    #[test]
    fn an_attached_top_follows_the_slope() {
        // an underside over the whole wall, rising along it from 2.5 m at
        // its start to 3.5 m at its end
        let top = TopProfile {
            faces: vec![TopFace {
                outline: vec![
                    DVec2::new(0.0, 0.0),
                    DVec2::new(2.0, 0.0),
                    DVec2::new(2.0, 7.0),
                    DVec2::new(0.0, 7.0),
                ],
                g: DVec2::new(0.0, 0.2),
                c: 2.3,
                lift: vec![],
            }],
        };
        let m = wall_mesh(&wall(), 0.0, &[], &WallEnds::default(), Some(&top));
        assert!(
            (m.volume() - 5.0 * 3.0 * 0.2).abs() < 1e-5,
            "{}",
            m.volume()
        );
        let (_, hi) = m.bounds().unwrap();
        assert!((hi.z - 3.5).abs() < 1e-5);
        // where nothing is overhead it keeps its own height
        let mut away = top.clone();
        away.faces[0].outline = vec![
            DVec2::new(10.0, 0.0),
            DVec2::new(12.0, 0.0),
            DVec2::new(12.0, 2.0),
        ];
        let m = wall_mesh(&wall(), 0.0, &[], &WallEnds::default(), Some(&away));
        assert!((m.volume() - 5.0 * 3.0 * 0.2).abs() < 1e-5);
    }

    #[test]
    fn a_layer_run_on_by_hand_grows_by_its_run() {
        // the middle 100 mm of the wall, 20 mm past its end
        let band = (-0.05, 0.05);
        let m = wall_layer_mesh(
            &wall(),
            0.0,
            &[],
            &WallEnds::default(),
            None,
            band,
            true,
            [0.0, 0.02],
        );
        assert!(
            (m.volume() - 5.02 * 3.0 * 0.1).abs() < 1e-6,
            "{}",
            m.volume()
        );
        let (_, hi) = m.bounds().unwrap();
        assert!((hi.y - 6.02).abs() < 1e-9);
        // held back 50 mm at its start
        let m = wall_layer_mesh(
            &wall(),
            0.0,
            &[],
            &WallEnds::default(),
            None,
            band,
            false,
            [-0.05, 0.0],
        );
        assert!((m.volume() - 4.95 * 3.0 * 0.1).abs() < 1e-6);
    }

    #[test]
    fn plain_wall_is_a_closed_box() {
        let m = wall_mesh(&wall(), 0.0, &[], &WallEnds::default(), None);
        assert_eq!(m.triangle_count(), 12);
        assert!((m.volume() - 5.0 * 3.0 * 0.2).abs() < 1e-5);
    }

    #[test]
    fn openings_remove_their_volume() {
        let win = opening(1.0, 0.9, 1.2, 1.2);
        let door = opening(3.0, 0.0, 0.9, 2.1);
        let m = wall_mesh(&wall(), 3.0, &[&win, &door], &WallEnds::default(), None);
        let expected = (15.0 - 1.44 - 0.9 * 2.1) * 0.2;
        assert!((m.volume() - expected).abs() < 1e-4, "{}", m.volume());
        let (lo, hi) = m.bounds().unwrap();
        assert!((lo.z - 3.0).abs() < 1e-6 && (hi.z - 6.0).abs() < 1e-6);
    }

    #[test]
    fn a_pocket_door_hollows_its_wall_beside_the_opening() {
        let mut door = opening(3.0, 0.0, 0.9, 2.1);
        door.kind = OpeningKind::Door;
        door.frame
            .set_operation(door.kind, creabim_core::Operation::Pocket);
        let w = wall();
        let pockets = crate::opening::pockets(&door, &w);
        assert_eq!(pockets.len(), 1);
        let (c, (h0, h1)) = pockets[0];
        let m = wall_mesh(&w, 0.0, &[&door], &WallEnds::default(), None);
        // the opening, and less the cavity beside it (it overlaps the
        // opening by a millimetre)
        let cavity = (c.u1 - c.u0 - 0.001) * (c.v1 - c.v0) * (h1 - h0);
        let expected = (15.0 - 0.9 * 2.1) * 0.2 - cavity;
        assert!(
            (m.volume() - expected).abs() < 1e-4,
            "{} {}",
            m.volume(),
            expected
        );
        // a plain door's wall is as it was
        door.frame
            .set_operation(door.kind, creabim_core::Operation::Standard);
        let m = wall_mesh(&w, 0.0, &[&door], &WallEnds::default(), None);
        assert!((m.volume() - (15.0 - 0.9 * 2.1) * 0.2).abs() < 1e-4);
    }

    #[test]
    fn an_arched_door_cuts_its_arch_and_a_raked_window_its_slope() {
        use creabim_core::cells::Outline;
        let mut door = opening(1.0, 0.0, 1.2, 2.4);
        door.kind = OpeningKind::Door;
        door.frame.grid.outline = Outline::Round;
        let m = wall_mesh(&wall(), 0.0, &[&door], &WallEnds::default(), None);
        // the upright part and a half circle of radius 0.6, in straight pieces
        let n = creabim_core::cells::ARCH_PIECES as f64;
        let half = 0.5 * n * 0.36 * (std::f64::consts::PI / n).sin();
        let hole = 1.2 * 1.8 + half;
        assert!(
            (m.volume() - (15.0 - hole) * 0.2).abs() < 1e-6,
            "{} {}",
            m.volume(),
            (15.0 - hole) * 0.2
        );
        assert!(m.is_closed());
        // a window whose head falls 0.4 along it: a rectangle and a triangle
        let mut win = opening(3.0, 0.9, 1.0, 1.4);
        win.frame.grid.outline = Outline::Raked { drop: 0.4 };
        let m = wall_mesh(&wall(), 0.0, &[&win], &WallEnds::default(), None);
        let hole = 1.0 * 1.0 + 0.5 * 1.0 * 0.4;
        assert!(
            (m.volume() - (15.0 - hole) * 0.2).abs() < 1e-6,
            "{}",
            m.volume()
        );
        assert!(m.is_closed());
        // both at once, the window's lines crossing the door's head
        let m = wall_mesh(&wall(), 0.0, &[&door, &win], &WallEnds::default(), None);
        let hole = 1.2 * 1.8 + half + 1.2;
        assert!(
            (m.volume() - (15.0 - hole) * 0.2).abs() < 1e-6,
            "{}",
            m.volume()
        );
    }

    #[test]
    fn opening_past_the_wall_end_is_clipped() {
        let o = opening(4.5, 1.0, 2.0, 1.0);
        let m = wall_mesh(&wall(), 0.0, &[&o], &WallEnds::default(), None);
        assert!((m.volume() - (15.0 - 0.5) * 0.2).abs() < 1e-4);
    }

    #[test]
    fn a_curved_wall_follows_its_arc_with_its_thickness_across_it() {
        // a half circle of radius 3 about the origin, 0.2 thick
        let mut w = Wall::new(
            ElementId::new(),
            DVec2::new(3.0, 0.0),
            DVec2::new(-3.0, 0.0),
            0.2,
            3.0,
        );
        w.bulge = -1.0;
        assert!((w.length() - std::f64::consts::PI * 3.0).abs() < 1e-9);
        let m = wall_mesh(&w, 0.0, &[], &WallEnds::default(), None);
        // every corner lies 2.9 or 3.1 from the centre: on its faces
        for p in &m.positions {
            let r = (p[0] as f64).hypot(p[1] as f64);
            assert!((r - 2.9).abs() < 1e-4 || (r - 3.1).abs() < 1e-4, "{r}");
        }
        // and it goes round through the far side
        assert!(m.positions.iter().any(|p| (p[1] as f64).abs() > 2.8));
        // a window in it sits on the arc too
        let win = opening(4.0, 0.9, 1.0, 1.2);
        let m = wall_mesh(&w, 0.0, &[&win], &WallEnds::default(), None);
        assert!(m.positions.iter().all(|p| {
            let r = (p[0] as f64).hypot(p[1] as f64);
            (r - 2.9).abs() < 1e-4 || (r - 3.1).abs() < 1e-4
        }));
    }
}
