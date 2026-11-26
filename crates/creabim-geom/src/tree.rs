//! Growing a tree from its shape ([`creabim_core::tree::TreeShape`]): a
//! trunk and a few limbs, a crown of foliage clumps fitted to the size
//! typed, and from those one 3D body and one plan symbol, so the plan,
//! the elevation and the 3D view always agree.
//!
//! Growing is cached by shape, canopy, height and seed: the scene is built
//! again after every edit (on a worker, see `scene::shared_scene`), and a
//! site of a hundred trees mustn't be grown a hundred times each time a
//! wall moves. A tree whose shape changed is grown once and kept.
//!
//! Everything here is CreaBIM's own: a seeded generator, value noise and
//! an icosphere, no outside code.

use crate::mesh::TriMesh;
use crate::plan::{Pen, PlanItem};
use creabim_core::tree::{ClumpForm, TreeDetail, TreeElevation, TreePlan, TreeShape};
use creabim_core::ElementId;
use glam::{DVec2, DVec3};
use std::collections::HashMap;
use std::f64::consts::{PI, TAU};
use std::sync::{Arc, Mutex, OnceLock};

/// The [`crate::scene::SceneItem::finish`] of foliage: drawings show its
/// outline, not its facets.
pub const FOLIAGE: &str = "foliage";

/// The [`crate::scene::SceneItem::finish`] of a plant's trunk and limbs:
/// rounded too, so drawings show their outline, not their facets.
pub const WOOD: &str = "wood";

/// [`WOOD`] left hollow: a section draws its cut's outline, unfilled.
pub const HOLLOW_WOOD: &str = "hollow wood";

/// [`FOLIAGE`] whose drawings draw it themselves: it still hides what is
/// behind it, but its own outline comes from its [`Stroke`]s.
pub const DRAWN_FOLIAGE: &str = "drawn foliage";

/// Drawn as rounded bodies: by their outline, not their facets.
pub fn rounded(finish: Option<&str>) -> bool {
    matches!(
        finish,
        Some(FOLIAGE) | Some(WOOD) | Some(HOLLOW_WOOD) | Some(DRAWN_FOLIAGE)
    )
}

/// A line a section or elevation draws over its scene's, in the view's
/// own terms: along it, up, and how far in front of its plane (so what
/// stands in front still hides it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub id: ElementId,
    pub pen: Pen,
    pub a: DVec3,
    pub b: DVec3,
}

/// What a section or elevation draws of the generated trees in its scene:
/// the scene with each crown swapped for the one drawings use, the lines
/// drawn over it, and the washes under those lines.
pub struct ForDrawings<'a> {
    pub scene: std::borrow::Cow<'a, [crate::scene::SceneItem]>,
    pub strokes: Vec<Stroke>,
    /// Light washes under a cut crown, in the drawing's own terms.
    pub washes: Vec<PlanItem>,
}

/// `scene` as a section (`cut`) or elevation along `frame` draws it.
///
/// Drawings always show a tree's whole crown, whatever month 3D shows, so
/// each crown here is its full foliage. A crown drawn as clumps and
/// standing clear of the cut is drawn from its 3D outline. Every other
/// crown, and every crown the section cuts through, is drawn as linework
/// of its own from the grown clumps: its body still hides what is behind
/// it, but draws no edges. Where a section cuts a crown it draws the cut
/// in the crown's style, heavy, over the wash its shape asks for, and the
/// crown beyond the cut light.
pub fn for_drawings<'a>(
    model: &creabim_core::Model,
    scene: &'a [crate::scene::SceneItem],
    frame: &crate::section::SectionFrame,
    cut: bool,
) -> ForDrawings<'a> {
    use std::borrow::Cow;
    let mut strokes = vec![];
    let mut washes = vec![];
    // the trees this drawing shows, by their wood
    let mut trees: Vec<(usize, ElementId)> = vec![];
    for (i, item) in scene.iter().enumerate() {
        if matches!(item.finish, Some(WOOD) | Some(HOLLOW_WOOD))
            && !trees.iter().any(|(_, id)| *id == item.id)
        {
            trees.push((i, item.id));
        }
    }
    let mut crowns: Vec<(ElementId, crate::scene::SceneItem)> = vec![];
    let mut woods: Vec<(usize, TriMesh)> = vec![];
    for (wood, id) in trees {
        let Some(creabim_core::Element {
            kind: creabim_core::ElementKind::Site(s),
            ..
        }) = model.get(id)
        else {
            continue;
        };
        let Some(shape) = s.tree_shape() else {
            continue;
        };
        let creabim_core::SiteForm::Plant {
            position,
            base,
            canopy,
            height,
            ..
        } = &s.form
        else {
            continue;
        };
        let g = grow(shape, *canopy, *height);
        let _ = base;
        let z0 = crate::earth::plant_z(model, s);
        let at = position.extend(z0);
        let o = frame.project(at);
        let reach = g.outline.iter().copied().fold(0.0, f64::max);
        let crossed = cut && o.z - reach < 0.0 && o.z + reach > 0.0;
        let seen = o.z + reach > 0.0 && o.z - reach <= frame.depth;
        let drawn = shape.elevation != TreeElevation::Clumps || crossed;
        // its whole crown, in its own colour, from the scene's look
        let colour = scene
            .iter()
            .find(|i| i.id == id && i.finish == Some(FOLIAGE))
            .map_or(shape.leaf_colour, |i| i.color);
        // its trunk and main limbs: the rest ends inside the crown
        let mut wood_mesh = g.drawing_wood().clone();
        for p in &mut wood_mesh.positions {
            *p = (glam::Vec3::from(*p) + at.as_vec3()).to_array();
        }
        woods.push((wood, wood_mesh));
        // a crown drawn as linework hides what is behind it with a card:
        // its drawn outline, standing in the crown facing the viewer, so
        // the drawing hides exactly what its lines enclose (filled below)
        let mut mesh = TriMesh::default();
        if !drawn {
            mesh = g.foliage.clone();
            for p in &mut mesh.positions {
                *p = (glam::Vec3::from(*p) + at.as_vec3()).to_array();
            }
        }
        let crown = crowns.len();
        crowns.push((
            id,
            crate::scene::SceneItem {
                mesh,
                color: colour,
                finish: Some(if drawn { DRAWN_FOLIAGE } else { FOLIAGE }),
                ..scene[wood].clone()
            },
        ));
        if !seen {
            continue;
        }
        let along = frame.along;
        let card = |outlines: &[Vec<DVec2>], depth: f64| {
            let mut m = TriMesh::default();
            let to = |p: DVec2| {
                let w = frame.origin + frame.along * (o.x + p.x) + frame.look * depth;
                [w.x as f32, w.y as f32, (z0 + p.y) as f32]
            };
            let n = (-frame.look).extend(0.0).as_vec3().to_array();
            for poly in outlines.iter().filter(|p| p.len() > 2) {
                // polar outlines: a fan from their middle covers them
                let mid = poly.iter().copied().sum::<DVec2>() / poly.len() as f64;
                let base = m.positions.len() as u32;
                m.positions.push(to(mid));
                m.positions.extend(poly.iter().map(|p| to(*p)));
                m.normals.extend(std::iter::repeat_n(n, poly.len() + 1));
                let k = poly.len() as u32;
                for i in 0..k {
                    m.indices
                        .extend([base, base + 1 + i, base + 1 + (i + 1) % k]);
                }
            }
            m
        };
        let mut push = |pen: Pen, a: DVec2, b: DVec2, depth: f64| {
            strokes.push(Stroke {
                id,
                pen,
                a: DVec3::new(o.x + a.x, z0 + a.y, depth),
                b: DVec3::new(o.x + b.x, z0 + b.y, depth),
            });
        };
        // in front of the whole crown, so only what stands before it hides it
        let front = (o.z - reach).max(0.0) + 1e-3;
        if crossed {
            // the clumps where the section's plane passes through them
            let d0 = -o.z;
            let slices: Vec<Ellipse> = g
                .clumps
                .iter()
                .filter_map(|k| {
                    let d = k.center.truncate().dot(along.perp()) - d0;
                    let f = 1.0 - (d / k.radius).powi(2);
                    (f > 0.01).then(|| Ellipse {
                        c: DVec2::new(k.center.truncate().dot(along), k.center.z),
                        rx: k.radius * f.sqrt(),
                        ry: k.half_height * f.sqrt(),
                        depth: 0.0,
                    })
                })
                .collect();
            let cut_crown = crown_lines(&slices, shape, Pen::Cut, Pen::Fine, true);
            let rgb = match shape.cut {
                creabim_core::tree::TreeCut::Tinted => Some(CANOPY_TINT),
                creabim_core::tree::TreeCut::Filled => Some([0.74, 0.74, 0.74]),
                creabim_core::tree::TreeCut::Outline => None,
            };
            if let Some(rgb) = rgb {
                for outline in &cut_crown.outlines {
                    washes.push(PlanItem::Shade {
                        id,
                        points: outline
                            .iter()
                            .map(|p| DVec2::new(o.x + p.x, z0 + p.y))
                            .collect(),
                        rgb,
                    });
                }
            }
            for (pen, a, b) in &cut_crown.lines {
                push(*pen, *a, *b, 1e-3);
            }
            // the crown beyond the cut, light, where it shows past it
            let beyond = crown_lines(
                &side_view(&g, along),
                shape,
                Pen::Projection,
                Pen::Fine,
                false,
            );
            crowns[crown].1.mesh = card(&beyond.outlines, o.z.max(0.0) + 2e-3);
            let inside_cut = |p: DVec2| cut_crown.outlines.iter().any(|o| inside_polygon(o, p));
            for (pen, a, b) in beyond.lines {
                if pen == Pen::Projection && !inside_cut((a + b) / 2.0) {
                    push(pen, a, b, 2e-3);
                }
            }
            continue;
        }
        if drawn {
            let lines = crown_lines(
                &side_view(&g, along),
                shape,
                Pen::Projection,
                Pen::Fine,
                false,
            );
            // halfway into the crown: its own wood there hides, what
            // stands in front of the crown still draws
            crowns[crown].1.mesh = card(&lines.outlines, (o.z - reach * 0.5).max(1e-3));
            for (pen, a, b) in lines.lines {
                push(pen, a, b, front);
            }
        }
        if shape.elevation_limbs {
            for (pen, a, b) in limb_lines(&g, shape, along) {
                push(pen, a, b, front);
            }
        }
    }
    if crowns.is_empty() {
        return ForDrawings {
            scene: Cow::Borrowed(scene),
            strokes,
            washes,
        };
    }
    let mut out: Vec<crate::scene::SceneItem> = scene
        .iter()
        .enumerate()
        .filter(|(_, i)| !(i.finish == Some(FOLIAGE) && crowns.iter().any(|(id, _)| *id == i.id)))
        .map(|(j, i)| match woods.iter().position(|(w, _)| *w == j) {
            Some(k) => crate::scene::SceneItem {
                mesh: std::mem::take(&mut woods[k].1),
                ..i.clone()
            },
            None => i.clone(),
        })
        .collect();
    out.extend(crowns.into_iter().map(|(_, c)| c));
    ForDrawings {
        scene: Cow::Owned(out),
        strokes,
        washes,
    }
}

/// A clump seen from the side or cut: an ellipse about `c`, `rx` across
/// and `ry` up, `depth` from the viewer (nearer ones hide farther ones).
#[derive(Clone, Copy, Debug)]
struct Ellipse {
    c: DVec2,
    rx: f64,
    ry: f64,
    depth: f64,
}

impl Ellipse {
    fn inside(&self, p: DVec2, f: f64) -> bool {
        ((p - self.c) / DVec2::new(self.rx, self.ry)).length() < f
    }
}

/// A grown crown seen from the side, looking across `along`.
fn side_view(g: &Grown, along: DVec2) -> Vec<Ellipse> {
    g.clumps
        .iter()
        .filter(|k| !k.core)
        .map(|k| Ellipse {
            c: DVec2::new(k.center.truncate().dot(along), k.center.z),
            rx: k.radius,
            ry: k.half_height,
            depth: k.center.truncate().dot(along.perp()),
        })
        .collect()
}

/// Is `p` inside the closed outline `poly`?
fn inside_polygon(poly: &[DVec2], p: DVec2) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + n - 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

/// The outline round `ellipses` from `m`, in `n` directions: how far out
/// it reaches each way.
fn polar(ellipses: &[Ellipse], m: DVec2, n: usize) -> Vec<f64> {
    let mut r: Vec<f64> = (0..n)
        .map(|i| {
            let d = DVec2::from_angle(TAU * i as f64 / n as f64);
            ellipses
                .iter()
                .filter_map(|e| {
                    // the far crossing of the ray with the ellipse
                    let s = DVec2::new(1.0 / e.rx.max(1e-6), 1.0 / e.ry.max(1e-6));
                    let p = (m - e.c) * s;
                    let q = d * s;
                    let (a, b, cc) = (q.dot(q), 2.0 * p.dot(q), p.dot(p) - 1.0);
                    let disc = b * b - 4.0 * a * cc;
                    (disc >= 0.0).then(|| (-b + disc.sqrt()) / (2.0 * a))
                })
                .filter(|t| *t > 0.0)
                .fold(f64::NAN, f64::max)
        })
        .collect();
    for i in 0..n {
        if r[i].is_nan() {
            r[i] = (1..n)
                .flat_map(|k| [r[(i + k) % n], r[(i + n - k) % n]])
                .find(|v| !v.is_nan())
                .unwrap_or(1.0);
        }
    }
    (0..n)
        .map(|i| (0..5).map(|k| r[(i + n + k - 2) % n]).sum::<f64>() / 5.0)
        .collect()
}

/// A crown's lines in a drawing, and the outline of each part of it.
struct CrownLines {
    lines: Vec<(Pen, DVec2, DVec2)>,
    outlines: Vec<Vec<DVec2>>,
}

/// The ellipses that touch one another, each group apart: a cut through
/// a crown's edge can meet two clumps with a gap between.
fn groups(ellipses: &[Ellipse]) -> Vec<Vec<Ellipse>> {
    let n = ellipses.len();
    let mut group: Vec<usize> = (0..n).collect();
    fn root(g: &mut [usize], i: usize) -> usize {
        let mut i = i;
        while g[i] != i {
            g[i] = g[g[i]];
            i = g[i];
        }
        i
    }
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (&ellipses[i], &ellipses[j]);
            // near enough to touch
            let d = ((a.c - b.c) / DVec2::new(a.rx + b.rx, a.ry + b.ry)).length();
            if d < 1.0 {
                let (ri, rj) = (root(&mut group, i), root(&mut group, j));
                group[ri] = rj;
            }
        }
    }
    let mut out: Vec<(usize, Vec<Ellipse>)> = vec![];
    #[allow(clippy::needless_range_loop)]
    for i in 0..n {
        let r = root(&mut group, i);
        match out.iter_mut().find(|(k, _)| *k == r) {
            Some((_, v)) => v.push(ellipses[i]),
            None => out.push((r, vec![ellipses[i]])),
        }
    }
    out.into_iter().map(|(_, v)| v).collect()
}

/// A crown's lines in a section or elevation, in the style its shape
/// asks, from its clumps seen from the side or cut (`cut`): `edge` its
/// outline's pen, `inner` the pen of what is inside it. In drawing terms:
/// along from the trunk, up from its base.
fn crown_lines(
    ellipses: &[Ellipse],
    shape: &TreeShape,
    edge: Pen,
    inner: Pen,
    cut: bool,
) -> CrownLines {
    let mut out = CrownLines {
        lines: vec![],
        outlines: vec![],
    };
    let mut rng = Rng::new(shape.seed ^ if cut { 0xC07_C07 } else { 0xE1E7_A710 });
    for part in groups(ellipses) {
        crown_part(&part, shape, edge, inner, &mut rng, &mut out);
    }
    out
}

fn crown_part(
    ellipses: &[Ellipse],
    shape: &TreeShape,
    edge: Pen,
    inner: Pen,
    rng: &mut Rng,
    out: &mut CrownLines,
) {
    let m = ellipses.iter().map(|e| e.c).sum::<DVec2>() / ellipses.len() as f64;
    const N: usize = 144;
    let r = polar(ellipses, m, N);
    let reach = |a: f64| {
        let f = a.rem_euclid(TAU) / TAU * N as f64;
        let i = f.floor() as usize % N;
        let u = f - f.floor();
        r[i] * (1.0 - u) + r[(i + 1) % N] * u
    };
    let out_at = |a: f64, u: f64| m + DVec2::from_angle(a) * reach(a) * u;
    let per: f64 = (0..N)
        .map(|i| {
            out_at(TAU * i as f64 / N as f64, 1.0)
                .distance(out_at(TAU * (i + 1) as f64 / N as f64, 1.0))
        })
        .sum();
    let rm = per / TAU;
    let unit = (rm / 3.0).clamp(0.25, 1.6);
    let area = PI * rm * rm;
    let mut items: Vec<PlanItem> = vec![];
    let id = ElementId::NIL;
    let closed = |items: &mut Vec<PlanItem>, pen: Pen, pts: &[DVec2]| {
        for i in 0..pts.len() {
            items.push(PlanItem::Line {
                id,
                pen,
                a: pts[i],
                b: pts[(i + 1) % pts.len()],
            });
        }
    };
    let scatter = |rng: &mut Rng, u0: f64, u1: f64| {
        let a = rng.next() * TAU;
        let u = (u0 * u0 + (u1 * u1 - u0 * u0) * rng.next()).sqrt();
        out_at(a, u)
    };
    let smooth: Vec<DVec2> = (0..N)
        .map(|i| out_at(TAU * i as f64 / N as f64, 1.0))
        .collect();
    // each clump as far as it shows past nearer ones and inside the outline
    let billows = |items: &mut Vec<PlanItem>, from: f64, to: f64, keep: f64| {
        for (i, e) in ellipses.iter().enumerate() {
            let k = 32;
            let pts: Vec<DVec2> = (0..=k)
                .map(|j| {
                    let a = from + (to - from) * j as f64 / k as f64;
                    e.c + DVec2::new(a.cos() * e.rx, a.sin() * e.ry)
                })
                .collect();
            let seen = |p: DVec2| {
                (p - m).length() <= reach((p - m).to_angle()) * keep
                    && !ellipses
                        .iter()
                        .enumerate()
                        .any(|(j, o)| j != i && o.depth < e.depth - 1e-9 && o.inside(p, 0.98))
            };
            for w in pts.windows(2) {
                if seen(w[0]) && seen(w[1]) {
                    items.push(PlanItem::Line {
                        id,
                        pen: inner,
                        a: w[0],
                        b: w[1],
                    });
                }
            }
        }
    };
    let outline: Vec<DVec2> = match shape.elevation {
        TreeElevation::Clumps => {
            // the clumps' own outline: each clump's rim where no other
            // covers it
            let mut rim = vec![];
            for (i, e) in ellipses.iter().enumerate() {
                let k = 48;
                let pts: Vec<DVec2> = (0..=k)
                    .map(|j| {
                        let a = TAU * j as f64 / k as f64;
                        e.c + DVec2::new(a.cos() * e.rx, a.sin() * e.ry)
                    })
                    .collect();
                let free = |p: DVec2| {
                    !ellipses
                        .iter()
                        .enumerate()
                        .any(|(j, o)| j != i && o.inside(p, 0.999))
                };
                for w in pts.windows(2) {
                    if free(w[0]) && free(w[1]) {
                        rim.push((w[0], w[1]));
                    }
                }
            }
            for (a, b) in rim {
                items.push(PlanItem::Line {
                    id,
                    pen: edge,
                    a,
                    b,
                });
            }
            billows(&mut items, 0.0, TAU, 0.97);
            smooth
        }
        TreeElevation::Outline => {
            closed(&mut items, edge, &smooth);
            smooth
        }
        TreeElevation::Wavy => {
            let n = ((per / (0.75 * unit)).round() as usize).clamp(10, 80);
            let amp = (0.24 * unit).min(rm * 0.1);
            let ph = rng.next() * TAU;
            let k = n * 8;
            let pts: Vec<DVec2> = (0..k)
                .map(|i| {
                    let a = TAU * i as f64 / k as f64;
                    let w = 0.5 - 0.5 * (n as f64 * a + ph).cos();
                    let j = 0.6 + 0.8 * noise(DVec3::new(a.cos() * 2.5, a.sin() * 2.5, ph));
                    m + DVec2::from_angle(a) * (reach(a) - amp * w * j)
                })
                .collect();
            closed(&mut items, edge, &pts);
            // the gaps between the leaves, where the sky shows through
            let gaps = ((area / (2.4 * unit * unit)) * (0.6 + shape.openness)).round() as usize;
            for _ in 0..gaps.clamp(2, 18) {
                let p = scatter(rng, 0.12, 0.7);
                let r = (0.15 + 0.2 * rng.next()) * unit;
                blob(&mut items, id, inner, p, r, rng);
            }
            pts
        }
        TreeElevation::Scalloped => {
            let n = ((per / (0.9 * unit)).round() as usize).clamp(8, 48);
            let pts: Vec<DVec2> = (0..n)
                .map(|i| {
                    let a = TAU * i as f64 / n as f64;
                    let rr = reach(a);
                    m + DVec2::from_angle(a) * (rr - 0.2 * TAU * rr / n as f64).max(rr * 0.6)
                })
                .collect();
            for i in 0..n {
                scallop(&mut items, id, edge, pts[i], pts[(i + 1) % n]);
            }
            // the top of each billow, where the light catches it
            billows(&mut items, 0.3, PI - 0.3, 0.9);
            // the outline as drawn: each scallop's arc, so what it hides
            // ends at its line (the trunk meets the lowest scallop)
            (0..n)
                .flat_map(|i| {
                    let (p, q) = (pts[i], pts[(i + 1) % n]);
                    let d = q - p;
                    let bulge = DVec2::new(d.y, -d.x) * 0.22;
                    // the arc, near enough: a parabola through its ends and top
                    (0..6).map(move |j| {
                        let t = j as f64 / 6.0;
                        p + d * t + bulge * 4.0 * t * (1.0 - t)
                    })
                })
                .collect()
        }
        TreeElevation::Leafy => {
            let n = ((per / (0.55 * unit)).round() as usize).clamp(8, 90);
            let pts: Vec<DVec2> = (0..n)
                .map(|i| {
                    let a = TAU * (i as f64 + 0.3 * rng.centred()) / n as f64;
                    out_at(a, 0.93 + 0.07 * rng.next())
                })
                .collect();
            closed(&mut items, edge, &pts);
            leaves(&mut items, id, inner, rng, area, unit, &scatter);
            pts
        }
    };
    out.lines.extend(flatten(items));
    out.outlines.push(outline);
}

/// A crown's lines in an elevation looking across `along`, in the style
/// its shape asks: (pen, along from the trunk, up from its base). Drawn
/// from the same grown clumps as 3D, so they agree.
pub fn elevation_lines(g: &Grown, shape: &TreeShape, along: DVec2) -> Vec<(Pen, DVec2, DVec2)> {
    crown_lines(
        &side_view(g, along),
        shape,
        Pen::Projection,
        Pen::Fine,
        false,
    )
    .lines
}

/// The trunk and main limbs through a crown seen looking across `along`:
/// each a pair of lines tapering out along its path, as a hand draws a
/// tree's frame.
pub fn limb_lines(g: &Grown, _shape: &TreeShape, along: DVec2) -> Vec<(Pen, DVec2, DVec2)> {
    let low = g
        .clumps
        .iter()
        .map(|c| c.center.z - c.half_height)
        .fold(f64::INFINITY, f64::min);
    let side = |p: DVec3| DVec2::new(p.truncate().dot(along), p.z);
    let mut out = vec![];
    for b in g.branches.iter().filter(|b| b.rank <= 1) {
        let pts: Vec<DVec2> = b.points.iter().map(|p| side(*p)).collect();
        for s in [-1.0, 1.0] {
            for i in 0..pts.len() - 1 {
                if b.points[i].z.max(b.points[i + 1].z) < low {
                    continue;
                }
                let n = |k: usize| {
                    let (a, c) = (pts[k.saturating_sub(1)], pts[(k + 1).min(pts.len() - 1)]);
                    (c - a).perp().normalize_or_zero()
                };
                let a = pts[i] + n(i) * b.radii[i] * s;
                let c = pts[i + 1] + n(i + 1) * b.radii[i + 1] * s;
                out.push((Pen::Symbol, a, c));
            }
        }
    }
    out
}

/// Plan items drawn by the symbol's helpers, back as lines: an arc as
/// short chords.
fn flatten(items: Vec<PlanItem>) -> Vec<(Pen, DVec2, DVec2)> {
    let mut out = vec![];
    for it in items {
        match it {
            PlanItem::Line { pen, a, b, .. } => out.push((pen, a, b)),
            PlanItem::Arc {
                pen,
                center,
                radius,
                start,
                end,
                ..
            } => {
                let n = (((end - start) / 0.2).ceil() as usize).max(2);
                let at = |i: usize| {
                    center + DVec2::from_angle(start + (end - start) * i as f64 / n as f64) * radius
                };
                for i in 0..n {
                    out.push((pen, at(i), at(i + 1)));
                }
            }
            _ => {}
        }
    }
    out
}

/// A crown's light tint in plans (sRGB).
pub const CANOPY_TINT: [f32; 3] = [0.84, 0.91, 0.80];

/// How many directions the plan outline is sampled in.
const SAMPLES: usize = 144;

/// A mass of foliage: an ellipsoid `radius` across in plan, `half_height`
/// up and down from its centre.
#[derive(Clone, Debug, PartialEq)]
pub struct Clump {
    pub center: DVec3,
    pub radius: f64,
    pub half_height: f64,
    /// Inside the crown, filling it out in 3D; left out of the outline.
    pub core: bool,
}

/// A length of trunk or limb, tapering from `r0` at `a` to `r1` at `b`.
#[derive(Clone, Debug, PartialEq)]
pub struct Limb {
    pub a: DVec3,
    pub b: DVec3,
    pub r0: f64,
    pub r1: f64,
    /// The trunk and the main limbs, which plans can draw; not a twig.
    pub main: bool,
}

/// A grown tree, about its trunk's foot: x and y the model's, z up from
/// the tree's base.
#[derive(Debug)]
pub struct Grown {
    pub clumps: Vec<Clump>,
    pub limbs: Vec<Limb>,
    pub wood: TriMesh,
    /// The twigs inside the clumps: shown in 3D only as the crown thins.
    pub twigs: TriMesh,
    pub foliage: TriMesh,
    /// Each clump's share of `foliage`: (first corner, corners, first
    /// index, indices), so a crown part in leaf is a few of them, shrunk.
    /// Empty when the crown is one fused body.
    pub bodies: Vec<(usize, usize, usize, usize)>,
    /// The trunk and every branch, each a smooth path.
    pub branches: Vec<Branch>,
    /// The crown's reach from the trunk seen from above, in [`SAMPLES`]
    /// directions counter-clockwise from the model's x.
    pub outline: Vec<f64>,
    /// The trunk and main limbs alone, for drawings, made when first asked.
    drawing_wood: OnceLock<TriMesh>,
    solid: bool,
}

impl Grown {
    /// The wood a drawing of the tree in leaf needs: its trunk and main
    /// limbs, a little coarser. The lesser branches and twigs end inside
    /// the crown, which hides them, so a drawing doesn't test their many
    /// edges against everything in front of them.
    pub fn drawing_wood(&self) -> &TriMesh {
        self.drawing_wood.get_or_init(|| {
            let mut m = TriMesh::default();
            for b in self.branches.iter().filter(|b| b.rank <= 1) {
                sweep(&mut m, b, [8, 6][b.rank as usize], self.solid);
            }
            m
        })
    }

    /// The crown's reach from the trunk along `a` radians (model frame).
    pub fn reach(&self, a: f64) -> f64 {
        let f = a.rem_euclid(TAU) / TAU * SAMPLES as f64;
        let i = f.floor() as usize % SAMPLES;
        let u = f - f.floor();
        self.outline[i] * (1.0 - u) + self.outline[(i + 1) % SAMPLES] * u
    }

    /// Its foliage with `leaf` of the crown in leaf (0 bare, 1 full):
    /// some clumps gone and the rest thinner, the same ones each time.
    pub fn foliage_at(&self, leaf: f64) -> std::borrow::Cow<'_, TriMesh> {
        use std::borrow::Cow;
        let leaf = leaf.clamp(0.0, 1.0);
        if leaf >= 0.999 {
            return Cow::Borrowed(&self.foliage);
        }
        if leaf <= 0.001 {
            return Cow::Owned(TriMesh::default());
        }
        let shrink = (0.55 + 0.45 * leaf) as f32;
        if self.bodies.is_empty() {
            // one fused body: thinner all over
            let mut m = self.foliage.clone();
            let n = m.positions.len().max(1) as f32;
            let mid = m
                .positions
                .iter()
                .fold(glam::Vec3::ZERO, |s, p| s + glam::Vec3::from(*p))
                / n;
            for p in &mut m.positions {
                *p = (mid + (glam::Vec3::from(*p) - mid) * shrink).to_array();
            }
            return Cow::Owned(m);
        }
        // which clumps keep their leaves: in an order of their own
        let mut order: Vec<usize> = (0..self.bodies.len()).collect();
        order.sort_by_key(|i| (*i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40);
        let keep = ((self.bodies.len() as f64 * leaf).ceil() as usize).max(1);
        let mut m = TriMesh::default();
        for &i in order.iter().take(keep) {
            let (p0, pn, i0, ni) = self.bodies[i];
            let c = self.clumps[i].center.as_vec3();
            let base = m.positions.len() as u32;
            for k in p0..p0 + pn {
                let p = glam::Vec3::from(self.foliage.positions[k]);
                m.positions.push((c + (p - c) * shrink).to_array());
                m.normals.push(self.foliage.normals[k]);
            }
            m.indices.extend(
                self.foliage.indices[i0..i0 + ni]
                    .iter()
                    .map(|v| v - p0 as u32 + base),
            );
        }
        Cow::Owned(m)
    }
}

type Cache = Mutex<(HashMap<u64, Arc<Grown>>, Vec<u64>)>;
static CACHE: OnceLock<Cache> = OnceLock::new();
const KEEP: usize = 512;

fn key(shape: &TreeShape, canopy: f64, height: f64) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for v in [
        canopy,
        height,
        shape.depth,
        shape.crown_base,
        shape.trunk,
        shape.clump_size,
        shape.irregularity,
        shape.openness,
        shape.angle,
    ]
    .iter()
    .chain(shape.reach.iter())
    {
        v.to_bits().hash(&mut h);
    }
    (
        shape.crown as u8,
        shape.branching as u8,
        shape.form as u8,
        shape.detail as u8,
        shape.solid,
        shape.clumps,
        shape.seed,
        shape.facets,
    )
        .hash(&mut h);
    h.finish()
}

/// The tree `shape` grows into, `canopy` across and `height` tall: from
/// the cache when it has been grown before.
pub fn grow(shape: &TreeShape, canopy: f64, height: f64) -> Arc<Grown> {
    let k = key(shape, canopy, height);
    let cache = CACHE.get_or_init(Default::default);
    if let Some(g) = cache.lock().unwrap_or_else(|e| e.into_inner()).0.get(&k) {
        return g.clone();
    }
    // grown outside the lock, so two workers never wait on each other
    let g = Arc::new(grow_uncached(shape, canopy, height));
    let mut c = cache.lock().unwrap_or_else(|e| e.into_inner());
    if c.0.len() >= KEEP {
        // the oldest half goes
        let old: Vec<u64> = c.1.drain(..KEEP / 2).collect();
        for o in old {
            c.0.remove(&o);
        }
    }
    if c.0.insert(k, g.clone()).is_none() {
        c.1.push(k);
    }
    g
}

/// How many trees have been grown and kept, for tests and the status bar.
pub fn cached() -> usize {
    CACHE
        .get()
        .map_or(0, |c| c.lock().unwrap_or_else(|e| e.into_inner()).0.len())
}

// ---- the generator ----

/// SplitMix64: small, fast and the same everywhere.
struct Rng(u64);

impl Rng {
    fn new(seed: u32) -> Self {
        Self(seed as u64 ^ 0x5DEE_CE66_D1CE_4E5B)
    }
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }
    /// -0.5 to 0.5.
    fn centred(&mut self) -> f64 {
        self.next() - 0.5
    }
}

fn hash3(x: i64, y: i64, z: i64) -> f64 {
    let mut h = (x as u64).wrapping_mul(0x8DA6_B343)
        ^ (y as u64).wrapping_mul(0xD816_3841)
        ^ (z as u64).wrapping_mul(0xCB1A_B31F);
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 33;
    (h & 0xFF_FFFF) as f64 / 0xFF_FFFF as f64
}

/// Smooth value noise, -1 to 1.
fn noise(p: DVec3) -> f64 {
    let f = p.floor();
    let (xi, yi, zi) = (f.x as i64, f.y as i64, f.z as i64);
    let s = |t: f64| t * t * (3.0 - 2.0 * t);
    let (u, v, w) = (s(p.x - f.x), s(p.y - f.y), s(p.z - f.z));
    let mut r = 0.0;
    for a in 0..2 {
        for b in 0..2 {
            for c in 0..2 {
                let wt = (if a == 1 { u } else { 1.0 - u })
                    * (if b == 1 { v } else { 1.0 - v })
                    * (if c == 1 { w } else { 1.0 - w });
                r += wt * hash3(xi + a, yi + b, zi + c);
            }
        }
    }
    r * 2.0 - 1.0
}

/// A unit sphere: an icosahedron split `level` times (0 to 3), its
/// corners shared.
fn icosphere(level: usize) -> &'static (Vec<DVec3>, Vec<[u32; 3]>) {
    type Sphere = (Vec<DVec3>, Vec<[u32; 3]>);
    static S: OnceLock<[Sphere; 4]> = OnceLock::new();
    &S.get_or_init(|| {
        let t = (1.0 + 5f64.sqrt()) / 2.0;
        let mut v: Vec<DVec3> = [
            (-1.0, t, 0.0),
            (1.0, t, 0.0),
            (-1.0, -t, 0.0),
            (1.0, -t, 0.0),
            (0.0, -1.0, t),
            (0.0, 1.0, t),
            (0.0, -1.0, -t),
            (0.0, 1.0, -t),
            (t, 0.0, -1.0),
            (t, 0.0, 1.0),
            (-t, 0.0, -1.0),
            (-t, 0.0, 1.0),
        ]
        .iter()
        .map(|&(x, y, z)| DVec3::new(x, y, z).normalize())
        .collect();
        let mut f: Vec<[u32; 3]> = vec![
            [0, 11, 5],
            [0, 5, 1],
            [0, 1, 7],
            [0, 7, 10],
            [0, 10, 11],
            [1, 5, 9],
            [5, 11, 4],
            [11, 10, 2],
            [10, 7, 6],
            [7, 1, 8],
            [3, 9, 4],
            [3, 4, 2],
            [3, 2, 6],
            [3, 6, 8],
            [3, 8, 9],
            [4, 9, 5],
            [2, 4, 11],
            [6, 2, 10],
            [8, 6, 7],
            [9, 8, 1],
        ];
        // the icosahedron above is y-up; turned z-up (a proper rotation,
        // so the faces still wind outwards)
        let z_up =
            |v: &[DVec3]| -> Vec<DVec3> { v.iter().map(|p| DVec3::new(p.x, -p.z, p.y)).collect() };
        let mut out: Vec<Sphere> = vec![(z_up(&v), f.clone())];
        for _ in 0..3 {
            let mut mids: HashMap<(u32, u32), u32> = HashMap::new();
            let mut mid = |a: u32, b: u32, v: &mut Vec<DVec3>| {
                *mids.entry((a.min(b), a.max(b))).or_insert_with(|| {
                    v.push(((v[a as usize] + v[b as usize]) / 2.0).normalize());
                    v.len() as u32 - 1
                })
            };
            let mut next = Vec::with_capacity(f.len() * 4);
            for [a, b, c] in f {
                let (ab, bc, ca) = (mid(a, b, &mut v), mid(b, c, &mut v), mid(c, a, &mut v));
                next.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
            }
            f = next;
            out.push((z_up(&v), f.clone()));
        }
        out.try_into().unwrap_or_else(|_| unreachable!())
    })[level.min(3)]
}

fn rotate(p: DVec2, a: f64) -> DVec2 {
    DVec2::from_angle(a).rotate(p)
}

/// A clump in the unit crown: x, y, z and radius.
type Unit = (f64, f64, f64, f64);

fn grow_uncached(shape: &TreeShape, canopy: f64, height: f64) -> Grown {
    let mut rng = Rng::new(shape.seed);
    let h = height.max(0.2);
    let zb = shape.crown_base.clamp(0.0, h * 0.95);
    let ch = (h - zb).max(0.1);
    let irr = shape.irregularity.clamp(0.0, 1.0);
    let open = shape.openness.clamp(0.0, 1.0);
    let n = shape.clumps.clamp(1, 60) as usize;
    let turn = shape.angle.to_radians();

    // 1. the crown's clumps in a unit crown: radius 1 across, -1 to 1 up
    let rc = (0.5 + 0.85 * shape.clump_size.clamp(0.0, 1.0)) / (n as f64).cbrt() * 1.15;
    let golden = PI * (3.0 - 5f64.sqrt());
    let a0 = rng.next() * TAU;
    // (x, y, z, r)
    let mut unit: Vec<Unit> = (0..n)
        .map(|i| {
            let t = ((i as f64 + 0.5) / n as f64 + irr * rng.centred() * 0.22).clamp(0.04, 0.96);
            let ang = a0 + i as f64 * golden + irr * rng.centred() * 0.9;
            let pr = shape.crown.profile(t);
            let r = (rc * (0.75 + 0.5 * rng.next()) * (1.0 - 0.25 * open))
                .min((pr * 0.95).max(rc * 0.45).max(0.12));
            let d = (pr - r * 0.85).max(0.0) * (1.0 - irr * rng.next() * 0.3);
            let z = -1.0 + r * 0.7 + t * (2.0 - r * 1.4).max(0.1);
            (ang.cos() * d, ang.sin() * d, z, r)
        })
        .collect();
    if n == 1 {
        // one clump is the whole crown
        unit[0] = (0.0, 0.0, 0.0, 1.0);
    }
    // an open crown has gaps
    for _ in 0..(n as f64 * open * 0.35) as usize {
        if unit.len() <= 4 {
            break;
        }
        let i = (rng.next() * unit.len() as f64) as usize % unit.len();
        unit.remove(i);
    }
    // fitted so the crown reaches exactly the size typed
    if n > 1 {
        for c in &mut unit {
            c.3 = c.3.min(0.92);
        }
    }
    let fit = |unit: &[Unit], along: &dyn Fn(&Unit) -> f64| {
        let (mut lo, mut hi) = (0.0, 4.0);
        for _ in 0..30 {
            let s = (lo + hi) / 2.0;
            let m = unit.iter().map(|c| along(c) * s + c.3).fold(0.0, f64::max);
            if m > 1.0 {
                hi = s;
            } else {
                lo = s;
            }
        }
        lo
    };
    let s = fit(&unit, &|c| c.0.hypot(c.1));
    let sz = fit(&unit, &|c| c.2.abs());
    for c in &mut unit {
        c.0 *= s;
        c.1 *= s;
        c.2 *= sz;
    }
    let shell = unit.len();
    if n > 1 {
        for _ in 0..(shell / 5).max(2) {
            let c = unit[(rng.next() * shell as f64) as usize % shell];
            unit.push((c.0 * 0.45, c.1 * 0.45, c.2 * 0.5, c.3 * 0.85));
        }
    }

    // into the tree: each clump pulled in or out with its side's reach
    let zc = zb + ch / 2.0;
    let clumps: Vec<Clump> = unit
        .iter()
        .enumerate()
        .map(|(i, &(x, y, z, r))| {
            let a = if x.hypot(y) > 1e-9 { y.atan2(x) } else { 0.0 };
            let big = shape.radius_local(canopy, a);
            Clump {
                center: rotate(DVec2::new(x, y) * big, turn).extend(zc + z * ch / 2.0),
                radius: r * big,
                // never a tall sausage or a flat plate: a mass of leaves
                // is about as deep as it is wide
                half_height: (r * ch / 2.0).clamp(r * big * 0.55, r * big * 1.35),
                core: i >= shell,
            }
        })
        .collect();
    // and held inside the envelope on every side: a side pulled in off a
    // wall is a promise, whatever its neighbours reach
    let mut clumps = clumps;
    for c in clumps.iter_mut() {
        let p = c.center.truncate();
        let f = (0..32)
            .map(|i| {
                let a = TAU * i as f64 / 32.0;
                let d = DVec2::from_angle(a);
                let far = p.dot(d) + c.radius;
                let env = shape.radius_local(canopy, a - turn);
                if far > env {
                    env / far
                } else {
                    1.0
                }
            })
            .fold(1.0, f64::min);
        if f < 1.0 {
            c.center = (p * f).extend(c.center.z);
            c.radius *= f;
        }
    }

    // 2. the bodies' bounds
    let keep_in = |p: DVec3| -> DVec3 {
        // what is fused or smoothed is held inside the sides' envelope,
        // as the clumps are: no further any way than the envelope reaches
        // that way, so a side pulled in off a wall keeps off its face
        let mut xy = p.truncate();
        for i in 0..32 {
            let a = TAU * i as f64 / 32.0;
            let d = DVec2::from_angle(a);
            let over = xy.dot(d) - shape.radius_local(canopy, a - turn);
            if over > 0.0 {
                xy -= d * over;
            }
        }
        xy.extend(p.z)
    };
    let low = clumps
        .iter()
        .map(|c| c.center.z - c.half_height)
        .fold(f64::INFINITY, f64::min);
    let hold = |p: DVec3| {
        let p = keep_in(p);
        DVec3::new(p.x, p.y, p.z.clamp(low, h))
    };

    // 3. the trunk and its branches, each ending in a clump
    let branches = branch(shape, &clumps, zb, ch, h, a0, &mut rng, &hold);
    let limbs = segments(&branches);

    // 4. the bodies
    let mean_r = clumps.iter().map(|c| c.radius).sum::<f64>() / clumps.len().max(1) as f64;
    let mut bodies = vec![];
    let foliage = match shape.form {
        ClumpForm::Metaballs => {
            let level = if shape.detail == TreeDetail::Lite {
                2
            } else {
                3
            };
            let (dirs, faces) = icosphere(level);
            fused(
                &clumps,
                mean_r * 0.5,
                0,
                irr,
                shape.seed,
                dirs,
                faces,
                false,
                &hold,
            )
        }
        ClumpForm::LowPoly => {
            // a few big faces over the whole crown, turned by the seed
            let n = [14, 24, 40, 64, 100][(shape.facets.clamp(1, 5) - 1) as usize];
            let mut local = Rng::new(shape.seed.wrapping_mul(7).wrapping_add(3));
            let golden = PI * (3.0 - 5f64.sqrt());
            let dirs: Vec<DVec3> = (0..n)
                .map(|j| {
                    let z = (1.0 - 2.0 * (j as f64 + 0.5 + 0.35 * local.centred()) / n as f64)
                        .clamp(-1.0, 1.0);
                    let a = j as f64 * golden + 0.35 * local.centred();
                    DVec2::from_angle(a).extend(0.0) * (1.0 - z * z).sqrt() + DVec3::Z * z
                })
                .collect();
            let faces = hull(&dirs);
            fused(
                &clumps,
                mean_r * 0.9,
                1,
                irr * 0.6,
                shape.seed,
                &dirs,
                &faces,
                true,
                &hold,
            )
        }
        _ => {
            let mut m = TriMesh::default();
            for (i, c) in clumps.iter().enumerate() {
                let (p0, i0) = (m.positions.len(), m.indices.len());
                clump_body(&mut m, c, i, shape, irr, &mut rng);
                bodies.push((p0, m.positions.len() - p0, i0, m.indices.len() - i0));
            }
            m
        }
    };
    let mut wood = TriMesh::default();
    let mut twigs = TriMesh::default();
    let low_poly = shape.form == ClumpForm::LowPoly;
    for b in &branches {
        let sides = match (shape.detail, b.rank) {
            // a site of hundreds: the trunk and main limbs only
            (TreeDetail::Lite, 2..) => continue,
            (TreeDetail::Everyday, 3) if low_poly => continue,
            _ if low_poly => [6, 5, 4, 3][b.rank as usize],
            (TreeDetail::Lite, r) => [6, 5, 4, 3][r as usize],
            (TreeDetail::Everyday, r) => [10, 7, 5, 3][r as usize],
            (TreeDetail::Presentation, r) => [18, 12, 8, 5][r as usize],
        };
        // twigs apart: hidden in leaf, they're only built into 3D when
        // the crown thins
        let m = if b.rank == 3 { &mut twigs } else { &mut wood };
        sweep(m, b, sides, shape.solid);
    }

    // 4. the crown seen from above
    let mut outline: Vec<f64> = (0..SAMPLES)
        .map(|i| {
            let d = DVec2::from_angle(TAU * i as f64 / SAMPLES as f64);
            clumps
                .iter()
                .filter(|c| !c.core)
                .filter_map(|c| {
                    let p = c.center.truncate();
                    let b = p.dot(d);
                    let disc = c.radius * c.radius - (p.length_squared() - b * b);
                    (disc >= 0.0).then(|| b + disc.sqrt())
                })
                .fold(f64::NAN, f64::max)
        })
        .collect();
    // a direction no clump reaches takes the nearer of its neighbours
    for i in 0..SAMPLES {
        if outline[i].is_nan() {
            let near = (1..SAMPLES)
                .flat_map(|k| {
                    [
                        outline[(i + k) % SAMPLES],
                        outline[(i + SAMPLES - k) % SAMPLES],
                    ]
                })
                .find(|v| !v.is_nan())
                .unwrap_or(canopy / 2.0);
            outline[i] = near;
        }
    }
    let outline = (0..SAMPLES)
        .map(|i| {
            (0..5)
                .map(|k| outline[(i + SAMPLES + k - 2) % SAMPLES])
                .sum::<f64>()
                / 5.0
        })
        .collect();

    Grown {
        clumps,
        limbs,
        wood,
        twigs,
        foliage,
        bodies,
        branches,
        outline,
        drawing_wood: OnceLock::new(),
        solid: shape.solid,
    }
}

/// A trunk or branch: a smooth path from where it leaves its parent (or
/// the ground) to where it ends, its radius at each point.
#[derive(Clone, Debug, PartialEq)]
pub struct Branch {
    pub points: Vec<DVec3>,
    pub radii: Vec<f64>,
    /// 0 the trunk, 1 a main limb, 2 a branch, 3 a twig.
    pub rank: u8,
}

/// The trunk and its branches. The trunk flares at its foot and runs up
/// to where it divides (or, for a leader, on up through the crown, the
/// branches leaving it along its length). Each main limb leaves the trunk
/// inside it, so the two read as one piece of wood, curves out and up and
/// ends at the centre of a clump of the crown, so the foliage sits on it.
/// Lesser branches leave a main limb where it passes closest to the
/// clumps between, and twigs fan out inside each clump, so a bare tree in
/// winter still has a crown. Every point is held inside the crown's
/// envelope by `hold`, so no wood shows past the foliage.
#[allow(clippy::too_many_arguments)]
fn branch(
    shape: &TreeShape,
    clumps: &[Clump],
    zb: f64,
    ch: f64,
    h: f64,
    a0: f64,
    rng: &mut Rng,
    hold: &dyn Fn(DVec3) -> DVec3,
) -> Vec<Branch> {
    let rt = (shape.trunk / 2.0).max(0.01);
    let leader = shape.crown.leader();
    let irr = shape.irregularity.clamp(0.0, 1.0);
    let shell: Vec<&Clump> = clumps.iter().filter(|c| !c.core).collect();
    let low = clumps
        .iter()
        .map(|c| c.center.z - c.half_height)
        .fold(f64::INFINITY, f64::min);
    // the trunk: to where it divides, a little way into the crown, or for
    // a leader on up to near the top
    let top = if leader {
        (zb + ch * 0.86).min(h - 0.15)
    } else {
        (zb + ch * 0.1).max(zb.min(low + 0.2)).min(h - 0.2)
    }
    .max(0.3);
    let lean = DVec2::from_angle(a0 * 1.7) * irr * 0.04 * top;
    let n = ((top / 0.6).ceil() as usize).clamp(4, 12);
    let mut trunk = Branch {
        points: vec![],
        radii: vec![],
        rank: 0,
    };
    for i in 0..=n {
        let t = i as f64 / n as f64;
        // a gentle lean and sway, never a kink
        let sway =
            lean * (t * t) + DVec2::from_angle(a0 + t * 3.0) * irr * 0.02 * top * (PI * t).sin();
        trunk.points.push(sway.extend(top * t));
        // the root flare, then a slow taper; a leader thins to a point
        let flare = 1.0 + 0.3 * (-t * top / 0.35).exp();
        let taper = if leader {
            1.0 - 0.85 * t
        } else {
            1.0 - 0.18 * t
        };
        trunk.radii.push(rt * flare * taper);
    }
    let along_trunk = |z: f64| -> (DVec3, f64) {
        let f = (z / top).clamp(0.0, 1.0) * n as f64;
        let i = (f.floor() as usize).min(n - 1);
        let u = f - i as f64;
        (
            trunk.points[i].lerp(trunk.points[i + 1], u),
            trunk.radii[i] + (trunk.radii[i + 1] - trunk.radii[i]) * u,
        )
    };
    let mut out = vec![];
    let k = shape.branching.limbs().min(shell.len().max(1));
    let share = match shape.branching {
        creabim_core::tree::Branching::Few => 0.78,
        creabim_core::tree::Branching::Balanced => 0.66,
        creabim_core::tree::Branching::Many => 0.52,
    };
    // the main limbs: each to a clump out its way
    let mut used = vec![false; shell.len()];
    let mut mains: Vec<Branch> = vec![];
    for i in 0..k {
        let ang = a0 + 0.4 + TAU * i as f64 / k as f64 + rng.centred() * 0.45;
        let dir = DVec2::from_angle(ang);
        let zs = if leader {
            // up the trunk, a whorl at a time
            let lo = (zb * 0.85).max(top * 0.12);
            lo + (top * 0.8 - lo) * (i as f64 + 0.3 + 0.4 * rng.next()) / k as f64
        } else {
            top - (top - zb * 0.85).clamp(0.0, top * 0.25) * 0.4 * rng.next()
        };
        let score = |c: &Clump| {
            let p = c.center.truncate();
            let towards = if p.length() > 1e-6 {
                p.normalize().dot(dir)
            } else {
                0.0
            };
            let rise = (c.center.z - zs) / ch;
            let height = if leader {
                -(rise - 0.2).abs() * 2.5
            } else {
                -(rise - 0.45).abs() * 0.6
            };
            towards * 1.5 + height + p.length() / ch * 0.2
        };
        let Some(j) = (0..shell.len())
            .filter(|j| !used[*j] || shell.len() <= k)
            .max_by(|a, b| score(shell[*a]).total_cmp(&score(shell[*b])))
        else {
            continue;
        };
        used[j] = true;
        let target = shell[j].center;
        let (at, rr) = along_trunk(zs);
        let r0 = if leader { rr * share * 0.6 } else { rr * share };
        // leaves the trunk inside it, rises out at an angle and curves up
        // into the clump: a cubic, its first handle out and up
        let to = target;
        let span = (to - at).length().max(0.1);
        let out_dir = (to.truncate() - at.truncate()).normalize_or(dir);
        // leaving a leader nearly level, a fork steeply, so it grows out
        // of the trunk rather than out of its side
        let c1 = if leader {
            at + (out_dir * 0.4 * span).extend(0.18 * span)
        } else {
            at + (out_dir * 0.22 * span).extend(0.45 * span)
        };
        let c2 = to - DVec3::Z * span * 0.22
            + (out_dir * -0.12 * span).extend(0.0)
            + DVec3::new(rng.centred(), rng.centred(), 0.0) * irr * 0.12 * span;
        let start = at - DVec3::Z * rr * 0.6;
        let pts: Vec<DVec3> = cubic(start, c1, c2, to, 9).into_iter().map(hold).collect();
        mains.push(path(pts, r0, (r0 * 0.3).max(0.012), 1));
    }
    // the trunk ends inside where its limbs leave, closing over them
    if !leader {
        let last = *trunk.points.last().unwrap();
        trunk.points.push(last + DVec3::Z * rt * 0.3);
        let r = *trunk.radii.last().unwrap();
        trunk.radii.push(r * 0.8);
    }
    // lesser branches: from the main limb that passes nearest each clump
    // not yet reached, rising out to its centre
    let mut lesser = vec![];
    for (j, c) in shell.iter().enumerate() {
        if used[j] {
            continue;
        }
        let mut best: Option<(f64, usize, usize)> = None;
        for (mi, m) in mains.iter().enumerate() {
            let end = m.points.len().saturating_sub(1);
            for (q, &p) in m.points.iter().enumerate().take(end).skip(2) {
                // below the clump and not past it
                let d = p.distance(c.center) + (p.z - c.center.z).max(0.0) * 1.5;
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, mi, q));
                }
            }
        }
        let Some((_, mi, q)) = best else {
            continue;
        };
        let m = &mains[mi];
        // from inside the limb, a little back along it
        let from = m.points[q].lerp(m.points[q - 1], 0.3);
        let r = m.radii[q];
        let tangent = (m.points[q + 1] - m.points[q - 1]).normalize_or_zero();
        let to = c.center;
        let span = from.distance(to).max(0.05);
        let c1 = from + (tangent * 0.5 + (to - from).normalize_or_zero() * 0.5) * span * 0.4;
        let c2 = to - DVec3::Z * span * 0.15;
        let pts: Vec<DVec3> = cubic(from, c1, c2, to, 6).into_iter().map(hold).collect();
        lesser.push(path(pts, (r * 0.6).max(0.01), (r * 0.22).max(0.007), 2));
    }
    // twigs: a fan from each branch's end inside its clump, out towards
    // its edge, so a bare crown keeps its shape
    let mut twigs = vec![];
    if shape.detail != TreeDetail::Lite {
        let per = if shape.detail == TreeDetail::Presentation {
            5
        } else {
            3
        };
        for b in mains.iter().chain(lesser.iter()) {
            let end = *b.points.last().unwrap();
            let r = *b.radii.last().unwrap();
            let Some(c) = shell.iter().min_by(|a, b| {
                a.center
                    .distance_squared(end)
                    .total_cmp(&b.center.distance_squared(end))
            }) else {
                continue;
            };
            let back = b.points[b.points.len() - 2];
            let base = (end - back).normalize_or_zero();
            for t in 0..per {
                let a = TAU * (t as f64 + 0.5 * rng.next()) / per as f64;
                let side = DVec2::from_angle(a).extend(0.35 + 0.4 * rng.next());
                let d = (base * 0.6 + side.normalize()).normalize_or_zero();
                let len = c.radius.min(c.half_height) * (0.55 + 0.25 * rng.next());
                let mid = end + d * len * 0.5 + DVec3::Z * len * 0.1;
                let to = end + d * len;
                // kept inside the clump, short of its edge
                let q = to - c.center;
                let s = DVec3::new(
                    q.x / c.radius,
                    q.y / c.radius,
                    q.z / c.half_height.max(1e-6),
                );
                let to = if s.length() > 0.85 {
                    c.center + q * (0.85 / s.length())
                } else {
                    to
                };
                let pts: Vec<DVec3> = [end, mid, to].into_iter().map(hold).collect();
                twigs.push(path(pts, (r * 0.75).max(0.006), (r * 0.2).max(0.003), 3));
            }
        }
    }
    out.push(trunk);
    out.extend(mains);
    out.extend(lesser);
    out.extend(twigs);
    out
}

/// A branch along `pts`, tapering from `r0` to `r1`.
fn path(pts: Vec<DVec3>, r0: f64, r1: f64, rank: u8) -> Branch {
    let n = (pts.len() - 1).max(1) as f64;
    let radii = (0..pts.len())
        .map(|i| {
            let t = i as f64 / n;
            // thins faster towards its end, as wood does
            r0 + (r1 - r0) * t.powf(0.8)
        })
        .collect();
    Branch {
        points: pts,
        radii,
        rank,
    }
}

/// `n` points along the cubic from `a` to `d` with handles `b` and `c`.
fn cubic(a: DVec3, b: DVec3, c: DVec3, d: DVec3, n: usize) -> Vec<DVec3> {
    (0..n)
        .map(|i| {
            let t = i as f64 / (n - 1) as f64;
            let u = 1.0 - t;
            a * u * u * u + b * 3.0 * u * u * t + c * 3.0 * u * t * t + d * t * t * t
        })
        .collect()
}

/// The branches as straight lengths, for plans and checks.
fn segments(branches: &[Branch]) -> Vec<Limb> {
    let mut out = vec![];
    for b in branches {
        for i in 0..b.points.len().saturating_sub(1) {
            out.push(Limb {
                a: b.points[i],
                b: b.points[i + 1],
                r0: b.radii[i],
                r1: b.radii[i + 1],
                main: b.rank <= 1,
            });
        }
    }
    out
}

/// One clump's body in its form, added to `m`.
fn clump_body(m: &mut TriMesh, c: &Clump, i: usize, shape: &TreeShape, irr: f64, rng: &mut Rng) {
    let spin = rng.next() * TAU;
    let tilt = rng.centred() * 0.6;
    let turn = |v: DVec3| {
        let v = DVec3::new(
            v.x,
            v.y * tilt.cos() - v.z * tilt.sin(),
            v.y * tilt.sin() + v.z * tilt.cos(),
        );
        rotate(v.truncate(), spin).extend(v.z)
    };
    let off = DVec3::new(i as f64 * 3.7, (shape.seed % 97) as f64, i as f64 * 1.3);
    let place = |v: DVec3, ruffle: f64| {
        // ruffled inwards only, so it never passes the outline
        let s = 1.0 - ruffle * (1.0 - noise(v * 1.7 + off));
        c.center + DVec3::new(v.x * c.radius, v.y * c.radius, v.z * c.half_height) * s
    };
    let mut local = Rng::new(shape.seed.wrapping_mul(31).wrapping_add(i as u32 * 7919));
    let detail = shape.detail;
    let finer = |level: usize| match detail {
        TreeDetail::Lite => level.saturating_sub(2),
        TreeDetail::Everyday => level,
        TreeDetail::Presentation => (level + 1).min(3),
    };
    let form = match (detail, shape.form) {
        // a site of hundreds: twenty faces a clump, whatever the form
        (TreeDetail::Lite, f) if f != ClumpForm::Metaballs => ClumpForm::Polyhedra,
        (_, f) => f,
    };
    let (pts, faces): (Vec<DVec3>, Vec<[u32; 3]>) = match form {
        ClumpForm::Noise | ClumpForm::Metaballs => {
            let (v, f) = icosphere(finer(2));
            let pts: Vec<DVec3> = v.iter().map(|v| place(*v, irr * 0.14)).collect();
            add_smooth(m, &pts, f);
            return;
        }
        ClumpForm::Polyhedra => {
            let (v, f) = icosphere(0);
            (v.iter().map(|v| turn(*v)).collect(), f.clone())
        }
        ClumpForm::Geodesic => {
            let (v, f) = icosphere(finer(2));
            (v.iter().map(|v| turn(*v)).collect(), f.clone())
        }
        form => {
            let k = if detail == TreeDetail::Presentation {
                80
            } else {
                40
            };
            let pts: Vec<DVec3> = match form {
                ClumpForm::Random => (0..k)
                    .map(|_| {
                        let z = local.next() * 2.0 - 1.0;
                        let a = local.next() * TAU;
                        DVec2::from_angle(a).extend(0.0) * (1.0 - z * z).sqrt() + DVec3::Z * z
                    })
                    .collect(),
                _ => {
                    // Fibonacci; pseudo-random nudges each point by up to
                    // half the spacing
                    let nudge = if form == ClumpForm::PseudoRandom {
                        0.5
                    } else {
                        0.0
                    };
                    let golden = PI * (3.0 - 5f64.sqrt());
                    (0..k)
                        .map(|j| {
                            let z = (1.0
                                - 2.0 * (j as f64 + 0.5 + nudge * local.centred()) / k as f64)
                                .clamp(-1.0, 1.0);
                            let a = j as f64 * golden + nudge * local.centred() * golden;
                            let v = DVec2::from_angle(a).extend(0.0) * (1.0 - z * z).sqrt()
                                + DVec3::Z * z;
                            turn(v)
                        })
                        .collect()
                }
            };
            let f = hull(&pts);
            (pts, f)
        }
    };
    let pts: Vec<DVec3> = pts.iter().map(|v| place(*v, irr * 0.1)).collect();
    add_flat(m, &pts, &faces);
}

/// The convex hull of `pts` (points on a sphere, so every one of them is
/// on it), its faces wound outwards.
fn hull(pts: &[DVec3]) -> Vec<[u32; 3]> {
    let n = pts.len();
    if n < 4 {
        return vec![];
    }
    let far = |from: &dyn Fn(DVec3) -> f64, skip: &[usize]| {
        (0..n)
            .filter(|i| !skip.contains(i))
            .max_by(|a, b| from(pts[*a]).total_cmp(&from(pts[*b])))
            .unwrap_or(0)
    };
    let i0 = 0;
    let i1 = far(&|p| p.distance(pts[i0]), &[i0]);
    let i2 = far(
        &|p| (p - pts[i0]).cross(pts[i1] - pts[i0]).length(),
        &[i0, i1],
    );
    let nrm = (pts[i1] - pts[i0]).cross(pts[i2] - pts[i0]);
    let i3 = far(&|p| (p - pts[i0]).dot(nrm).abs(), &[i0, i1, i2]);
    let inner = (pts[i0] + pts[i1] + pts[i2] + pts[i3]) / 4.0;
    let normal = |f: &[u32; 3]| {
        let [a, b, c] = f.map(|i| pts[i as usize]);
        (b - a).cross(c - a)
    };
    let outward = |f: [usize; 3]| {
        let f = f.map(|i| i as u32);
        if normal(&f).dot(pts[f[0] as usize] - inner) < 0.0 {
            [f[0], f[2], f[1]]
        } else {
            f
        }
    };
    let mut faces: Vec<[u32; 3]> = vec![
        outward([i0, i1, i2]),
        outward([i0, i1, i3]),
        outward([i0, i2, i3]),
        outward([i1, i2, i3]),
    ];
    for p in 0..n {
        if [i0, i1, i2, i3].contains(&p) {
            continue;
        }
        let q = pts[p];
        let seen: Vec<bool> = faces
            .iter()
            .map(|f| normal(f).dot(q - pts[f[0] as usize]) > 1e-12)
            .collect();
        if !seen.iter().any(|s| *s) {
            continue;
        }
        let mut edges = std::collections::BTreeSet::new();
        for (f, _) in faces.iter().zip(&seen).filter(|(_, s)| **s) {
            for k in 0..3 {
                edges.insert((f[k], f[(k + 1) % 3]));
            }
        }
        let mut kept: Vec<[u32; 3]> = faces
            .iter()
            .zip(&seen)
            .filter(|(_, s)| !**s)
            .map(|(f, _)| *f)
            .collect();
        for &(a, b) in &edges {
            if !edges.contains(&(b, a)) {
                kept.push([a, b, p as u32]);
            }
        }
        faces = kept;
    }
    faces
}

/// The clumps fused into one smooth body: a sphere about the crown's
/// middle, each of its points pushed out to where the clumps' blended
/// surface is (blended over `blend` metres), ruffled by `irr`, held by
/// `hold`, and evened out `passes` times.
#[allow(clippy::too_many_arguments)]
fn fused(
    clumps: &[Clump],
    blend: f64,
    passes: usize,
    irr: f64,
    seed: u32,
    dirs: &[DVec3],
    faces: &[[u32; 3]],
    flat: bool,
    hold: &dyn Fn(DVec3) -> DVec3,
) -> TriMesh {
    let mut m = TriMesh::default();
    if clumps.is_empty() {
        return m;
    }
    let mid = clumps.iter().map(|c| c.center).sum::<DVec3>() / clumps.len() as f64;
    let k = blend.max(1e-3);
    let field = |p: DVec3| {
        let mut d = f64::INFINITY;
        for c in clumps {
            let q = p - c.center;
            let q = DVec3::new(q.x, q.y, q.z * c.radius / c.half_height.max(1e-6));
            let e = q.length() - c.radius;
            // a smooth minimum: the clumps run into each other
            let hh = (k - (d - e).abs()).max(0.0) / k;
            d = d.min(e) - hh * hh * k * 0.25;
        }
        d
    };
    let reach = clumps
        .iter()
        .map(|c| c.center.distance(mid) + c.radius.max(c.half_height))
        .fold(0.0, f64::max)
        * 1.1
        + k;
    let step = clumps
        .iter()
        .map(|c| c.radius.min(c.half_height))
        .fold(f64::INFINITY, f64::min)
        .max(0.05)
        * 0.3;
    let mut t: Vec<f64> = dirs
        .iter()
        .map(|d| {
            // in from outside to the first point inside, then halved down
            let mut hi = reach;
            while hi > 0.0 {
                let lo = hi - step;
                if field(mid + *d * lo.max(0.0)) < 0.0 {
                    let (mut a, mut b) = (lo.max(0.0), hi);
                    for _ in 0..12 {
                        let m = (a + b) / 2.0;
                        if field(mid + *d * m) < 0.0 {
                            a = m;
                        } else {
                            b = m;
                        }
                    }
                    return a;
                }
                hi = lo;
            }
            f64::NAN
        })
        .collect();
    let mut near: Vec<Vec<usize>> = vec![vec![]; dirs.len()];
    for f in faces {
        for k in 0..3 {
            let (a, b) = (f[k] as usize, f[(k + 1) % 3] as usize);
            near[a].push(b);
        }
    }
    // a gap in the crown takes its neighbours' reach
    for _ in 0..8 {
        if !t.iter().any(|v| v.is_nan()) {
            break;
        }
        let was = t.clone();
        for i in 0..t.len() {
            if was[i].is_nan() {
                let (s, c) = near[i]
                    .iter()
                    .filter(|j| !was[**j].is_nan())
                    .fold((0.0, 0), |(s, c), j| (s + was[*j], c + 1));
                if c > 0 {
                    t[i] = s / c as f64;
                }
            }
        }
    }
    for v in &mut t {
        if v.is_nan() {
            *v = step;
        }
    }
    for _ in 0..passes {
        let was = t.clone();
        for i in 0..t.len() {
            let s: f64 = near[i].iter().map(|j| was[*j]).sum();
            t[i] = was[i] * 0.4 + 0.6 * s / near[i].len().max(1) as f64;
        }
    }
    let off = DVec3::splat((seed % 89) as f64);
    let pts: Vec<DVec3> = dirs
        .iter()
        .zip(&t)
        .map(|(d, r)| {
            let s = 1.0 - irr * 0.08 * (1.0 - noise(*d * 2.3 + off));
            hold(mid + *d * *r * s)
        })
        .collect();
    if flat {
        add_flat(&mut m, &pts, faces);
    } else {
        add_smooth(&mut m, &pts, faces);
    }
    m
}

/// `pts` joined by `faces`, smooth: each corner's normal the mean of its
/// faces'.
fn add_smooth(m: &mut TriMesh, pts: &[DVec3], faces: &[[u32; 3]]) {
    let base = m.positions.len() as u32;
    let mut normals = vec![DVec3::ZERO; pts.len()];
    for f in faces {
        let [a, b, c] = f.map(|i| pts[i as usize]);
        let n = (b - a).cross(c - a);
        for i in f {
            normals[*i as usize] += n;
        }
    }
    for (p, n) in pts.iter().zip(&normals) {
        m.positions.push(p.as_vec3().to_array());
        m.normals.push(n.normalize_or_zero().as_vec3().to_array());
    }
    for f in faces {
        m.indices.extend(f.map(|i| base + i));
    }
}

/// `pts` joined by `faces`, each face flat: faceted.
fn add_flat(m: &mut TriMesh, pts: &[DVec3], faces: &[[u32; 3]]) {
    for f in faces {
        let [a, b, c] = f.map(|i| pts[i as usize]);
        let n = (b - a)
            .cross(c - a)
            .normalize_or_zero()
            .as_vec3()
            .to_array();
        let base = m.positions.len() as u32;
        for p in [a, b, c] {
            m.positions.push(p.as_vec3().to_array());
            m.normals.push(n);
        }
        m.indices.extend([base, base + 1, base + 2]);
    }
}

/// A branch's wood: one smooth tube along its whole path, its rings
/// turned with it so it never twists, closed at both ends when `solid`
/// (its end drawn to a point, so a branch tapers off rather than stopping).
fn sweep(m: &mut TriMesh, b: &Branch, sides: usize, solid: bool) {
    let pts = &b.points;
    let n = pts.len();
    if n < 2 || sides < 3 {
        return;
    }
    let tangent = |i: usize| {
        let (a, c) = (pts[i.saturating_sub(1)], pts[(i + 1).min(n - 1)]);
        (c - a).normalize_or(DVec3::Z)
    };
    // a frame carried along without turning about the path
    let t0 = tangent(0);
    let mut u = if t0.z.abs() < 0.9 {
        DVec3::Z.cross(t0).normalize()
    } else {
        DVec3::X.cross(t0).normalize()
    };
    let base = m.positions.len() as u32;
    let s = sides as u32;
    #[allow(clippy::needless_range_loop)]
    for i in 0..n {
        let t = tangent(i);
        u = (u - t * u.dot(t)).normalize_or(u);
        let v = t.cross(u);
        for k in 0..sides {
            let a = TAU * k as f64 / sides as f64;
            let d = u * a.cos() + v * a.sin();
            m.positions
                .push((pts[i] + d * b.radii[i]).as_vec3().to_array());
            m.normals.push(d.as_vec3().to_array());
        }
    }
    for i in 0..n as u32 - 1 {
        for k in 0..s {
            let j = (k + 1) % s;
            let (a, b2, c, d) = (
                base + i * s + k,
                base + i * s + j,
                base + (i + 1) * s + j,
                base + (i + 1) * s + k,
            );
            m.indices.extend([a, b2, c, a, c, d]);
        }
    }
    if !solid {
        return;
    }
    // the far end drawn to a point a little past it; the trunk's end,
    // under its limbs, a low dome
    let reach = if b.rank == 0 { 0.35 } else { 1.5 };
    let tip = pts[n - 1] + tangent(n - 1) * b.radii[n - 1] * reach;
    let ti = m.positions.len() as u32;
    m.positions.push(tip.as_vec3().to_array());
    m.normals.push(tangent(n - 1).as_vec3().to_array());
    let last = base + (n as u32 - 1) * s;
    for k in 0..s {
        m.indices.extend([last + k, last + (k + 1) % s, ti]);
    }
    // the near end closed flat (inside its parent, or on the ground)
    let ci = m.positions.len() as u32;
    m.positions.push(pts[0].as_vec3().to_array());
    m.normals.push((-tangent(0)).as_vec3().to_array());
    for k in 0..s {
        m.indices.extend([base + (k + 1) % s, base + k, ci]);
    }
}

// ---- the plan symbol ----

/// A grown tree's plan symbol at `c`: its crown's outline from above,
/// drawn as `style` asks, with its trunk and a light tint as the shape
/// asks. Every style is drawn from the same grown crown and seed, so each
/// follows the crown's real outline, clumps and limbs.
pub fn plan_symbol(out: &mut Vec<PlanItem>, id: ElementId, c: DVec2, g: &Grown, shape: &TreeShape) {
    let at = |a: f64| c + DVec2::from_angle(a) * g.reach(a);
    let ring = |n: usize| -> Vec<DVec2> { (0..n).map(|i| at(TAU * i as f64 / n as f64)).collect() };
    if shape.plan_tint {
        // under the linework
        out.push(PlanItem::Shade {
            id,
            points: ring(72),
            rgb: CANOPY_TINT,
        });
    }
    // the outline heavy and the inside light, or both as the plan's
    let (edge, inner) = if shape.plan_bold {
        (Pen::Cut, Pen::Fine)
    } else {
        (Pen::Projection, Pen::Symbol)
    };
    let mut rng = Rng::new(shape.seed ^ 0x5EED_1A7E);
    let per: f64 = (0..SAMPLES)
        .map(|i| {
            at(TAU * i as f64 / SAMPLES as f64).distance(at(TAU * (i + 1) as f64 / SAMPLES as f64))
        })
        .sum();
    // the crown's mean radius, and a length that keeps detail in step
    // with it: a small tree's leaves aren't drawn the size of a big one's
    let rm = per / TAU;
    let unit = (rm / 3.0).clamp(0.35, 1.6);
    let line = |out: &mut Vec<PlanItem>, pen: Pen, a: DVec2, b: DVec2| {
        out.push(PlanItem::Line { id, pen, a, b });
    };
    let closed = |out: &mut Vec<PlanItem>, pen: Pen, pts: &[DVec2]| {
        for i in 0..pts.len() {
            line(out, pen, pts[i], pts[(i + 1) % pts.len()]);
        }
    };
    let dot = |out: &mut Vec<PlanItem>, p: DVec2, r: f64| {
        out.push(PlanItem::Arc {
            id,
            pen: inner,
            center: p,
            radius: r,
            start: 0.0,
            end: TAU,
        });
    };
    // a point `u` of the way out along `a` radians
    let out_at = |a: f64, u: f64| c + DVec2::from_angle(a) * g.reach(a) * u;
    // inside the outline, with a margin of `m` of the reach
    let inside = |p: DVec2, m: f64| {
        let d = p - c;
        d.length() <= g.reach(d.to_angle()) * (1.0 - m)
    };
    // a point at random over the crown, between `u0` and `u1` of the way out
    let scatter = |rng: &mut Rng, u0: f64, u1: f64| {
        let a = rng.next() * TAU;
        let u = (u0 * u0 + (u1 * u1 - u0 * u0) * rng.next()).sqrt();
        out_at(a, u)
    };
    let area = PI * rm * rm;
    let limb_lines = |out: &mut Vec<PlanItem>, pen: Pen, all: bool| {
        for l in g.limbs.iter().skip(1).filter(|l| all || l.main) {
            let (a, b) = (c + l.a.truncate(), c + l.b.truncate());
            if a.distance(b) > 1e-3 {
                line(out, pen, a, b);
            }
        }
    };
    let scallops = |out: &mut Vec<PlanItem>| {
        // each scallop about 0.9 m round the crown, as the simple symbol's
        let n = ((per / 0.9).round() as usize).clamp(7, 40);
        let pts: Vec<DVec2> = (0..n)
            .map(|i| {
                let a = TAU * i as f64 / n as f64;
                let r = g.reach(a);
                // the scallops' bulge reaches the outline
                c + DVec2::from_angle(a) * (r - 0.2 * TAU * r / n as f64).max(r * 0.6)
            })
            .collect();
        for i in 0..n {
            scallop(out, id, edge, pts[i], pts[(i + 1) % n]);
        }
    };
    match shape.plan {
        TreePlan::Outline => closed(out, edge, &ring(72)),
        TreePlan::Clumps => {
            closed(out, edge, &ring(72));
            // each clump of the crown as seen from above, as far as it
            // shows past the clumps above it and inside the outline: soft
            // lobes layered like clouds, never past the edge
            let shell: Vec<&Clump> = g.clumps.iter().filter(|k| !k.core).collect();
            for (j, k) in shell.iter().enumerate() {
                let o = c + k.center.truncate();
                let pts: Vec<DVec2> = (0..=48)
                    .map(|i| o + DVec2::from_angle(TAU * i as f64 / 48.0) * k.radius)
                    .collect();
                for w in pts.windows(2) {
                    let seen = |p: DVec2| {
                        inside(p, 0.015)
                            && !covered(&shell, j, p - c, |a| a.center.z + a.half_height)
                    };
                    if seen(w[0]) && seen(w[1]) {
                        line(out, inner, w[0], w[1]);
                    }
                }
            }
        }
        TreePlan::Scalloped => scallops(out),
        TreePlan::Limbs => {
            scallops(out);
            // the main limbs as they run out from the trunk
            limb_lines(out, inner, false);
        }
        TreePlan::Serrated => {
            // fine teeth round the rim
            let n = ((per / (0.28 * unit)).round() as usize).clamp(24, 200);
            let depth = (0.1 * unit).min(rm * 0.08);
            let pts: Vec<DVec2> = (0..2 * n)
                .map(|i| {
                    let a = TAU * i as f64 / (2 * n) as f64;
                    let r = g.reach(a) - if i % 2 == 1 { depth } else { 0.0 };
                    c + DVec2::from_angle(a) * r
                })
                .collect();
            closed(out, edge, &pts);
            // lines out from the trunk, of uneven length
            let k = ((per / (1.0 * unit)).round() as usize).clamp(8, 28);
            for i in 0..k {
                let a = TAU * (i as f64 + 0.5 * rng.centred()) / k as f64;
                line(
                    out,
                    inner,
                    out_at(a, 0.12),
                    out_at(a, 0.5 + 0.3 * rng.next()),
                );
            }
        }
        TreePlan::Wavy => {
            // a soft wave round the rim
            let n = ((per / (0.75 * unit)).round() as usize).clamp(10, 70);
            let amp = (0.2 * unit).min(rm * 0.09);
            let ph = rng.next() * TAU;
            let m = n * 8;
            let pts: Vec<DVec2> = (0..m)
                .map(|i| {
                    let a = TAU * i as f64 / m as f64;
                    let w = 0.5 - 0.5 * (n as f64 * a + ph).cos();
                    let j = 0.75 + 0.5 * noise(DVec3::new(a.cos() * 2.0, a.sin() * 2.0, ph));
                    c + DVec2::from_angle(a) * (g.reach(a) - amp * w * j)
                })
                .collect();
            closed(out, edge, &pts);
            // small leaf masses inside
            let k = ((area / (2.2 * unit * unit)).round() as usize).clamp(4, 22);
            for _ in 0..k {
                let p = scatter(&mut rng, 0.15, 0.72);
                let r = (0.16 + 0.18 * rng.next()) * unit;
                blob(out, id, inner, p, r, &mut rng);
            }
        }
        TreePlan::Skeleton => {
            // the limbs and branches as they run, each a smooth path,
            // and two twigs on from each branch's end to the rim
            for br in g.branches.iter().filter(|b| (1..=2).contains(&b.rank)) {
                for w in br.points.windows(2) {
                    let (a, b) = (c + w[0].truncate(), c + w[1].truncate());
                    if a.distance(b) > 1e-3 {
                        line(out, inner, a, b);
                    }
                }
            }
            for br in g.branches.iter().filter(|b| b.rank == 2) {
                let Some(end) = br.points.last() else {
                    continue;
                };
                let b = c + end.truncate();
                let d = b - c;
                if d.length() < 1e-3 {
                    continue;
                }
                let base = d.to_angle();
                for s in [-1.0, 1.0] {
                    let a = base + s * (0.18 + 0.2 * rng.next());
                    let to = out_at(a, 0.8 + 0.12 * rng.next());
                    if to.distance(c) > b.distance(c) {
                        line(out, inner, b, to);
                    }
                }
            }
            // and the rim dotted
            let n = ((per / (0.3 * unit)).round() as usize).clamp(20, 220);
            for i in 0..n {
                let a = TAU * i as f64 / n as f64;
                dot(out, out_at(a, 0.97), 0.035 * unit);
            }
        }
        TreePlan::Hatched => {
            closed(out, edge, &ring(72));
            // a band of short strokes round the rim, leaning one way
            let n = ((per / (0.14 * unit)).round() as usize).clamp(30, 300);
            for i in 0..n {
                let a = TAU * i as f64 / n as f64;
                let u0 = 0.8 + 0.06 * rng.next();
                line(out, inner, out_at(a, u0), out_at(a + 0.04, 0.985));
            }
            limb_lines(out, inner, false);
        }
        TreePlan::Cells => {
            closed(out, edge, &ring(72));
            // the crown parted into a cell a clump, as the clumps share it
            // out seen from above (each the ground nearer it than any
            // other, a bigger clump taking more), each cell drawn a little
            // inside its share so the cells read apart
            let shell: Vec<&Clump> = g.clumps.iter().filter(|k| !k.core).collect();
            let rim = ring(72);
            for (i, k) in shell.iter().enumerate() {
                let (pi, ri) = (c + k.center.truncate(), k.radius);
                let mut cell = rim.clone();
                for (j, o) in shell.iter().enumerate() {
                    if j == i || cell.len() < 3 {
                        continue;
                    }
                    let (pj, rj) = (c + o.center.truncate(), o.radius);
                    // |p-pi|^2 - ri^2 <= |p-pj|^2 - rj^2, a half-plane
                    let nrm = pj - pi;
                    let off = (pj.length_squared() - pi.length_squared() - rj * rj + ri * ri) / 2.0;
                    let keep = |p: DVec2| p.dot(nrm) <= off;
                    let mut next = vec![];
                    for t in 0..cell.len() {
                        let (a, b) = (cell[t], cell[(t + 1) % cell.len()]);
                        let (ka, kb) = (keep(a), keep(b));
                        if ka {
                            next.push(a);
                        }
                        if ka != kb {
                            let u = (off - a.dot(nrm)) / (b - a).dot(nrm);
                            next.push(a.lerp(b, u));
                        }
                    }
                    cell = next;
                }
                if cell.len() < 3 {
                    continue;
                }
                let mid = cell.iter().copied().sum::<DVec2>() / cell.len() as f64;
                let gap = 0.12 * unit;
                let inset: Vec<DVec2> = cell
                    .iter()
                    .map(|p| {
                        let d = *p - mid;
                        let l = d.length();
                        mid + d * ((l - gap).max(l * 0.5) / l.max(1e-9))
                    })
                    .collect();
                closed(out, inner, &inset);
            }
        }
        TreePlan::Wedges => {
            closed(out, edge, &ring(72));
            let k = ((per / (1.6 * unit)).round() as usize).clamp(6, 16);
            let a0 = rng.next() * TAU;
            for i in 0..k {
                let a = a0 + TAU * i as f64 / k as f64;
                line(out, inner, out_at(a, 0.08), out_at(a, 1.0));
            }
            // dots, thicker out towards the rim
            let n = ((area / (0.45 * unit * unit)).round() as usize).clamp(12, 160);
            for _ in 0..n {
                let p = scatter(&mut rng, 0.25, 0.94);
                dot(out, p, 0.03 * unit);
            }
        }
        TreePlan::Starburst => {
            // rays from the trunk, the longest reaching the rim
            let n = ((per / (0.3 * unit)).round() as usize).clamp(24, 120);
            for i in 0..n {
                let a = TAU * (i as f64 + 0.4 * rng.centred()) / n as f64;
                let u1 = 0.72 + 0.28 * rng.next();
                line(
                    out,
                    inner,
                    out_at(a, 0.18 + 0.2 * rng.next()),
                    out_at(a, u1),
                );
                if i % 3 == 0 {
                    dot(out, out_at(a, u1), 0.035 * unit);
                }
            }
        }
        TreePlan::Leafy => {
            // a faceted rim: straight runs with a little wander
            let n = ((per / (0.6 * unit)).round() as usize).clamp(9, 80);
            let pts: Vec<DVec2> = (0..n)
                .map(|i| {
                    let a = TAU * (i as f64 + 0.3 * rng.centred()) / n as f64;
                    out_at(a, 0.93 + 0.07 * rng.next())
                })
                .collect();
            closed(out, edge, &pts);
            leaves(out, id, inner, &mut rng, area, unit, &scatter);
        }
        TreePlan::Jagged => {
            // a rim of uneven points
            let n = ((per / (0.3 * unit)).round() as usize).clamp(20, 200);
            let pts: Vec<DVec2> = (0..n)
                .map(|i| {
                    let a = TAU * (i as f64 + 0.25 * rng.centred()) / n as f64;
                    let u = if i % 2 == 0 {
                        0.97 + 0.03 * rng.next()
                    } else {
                        0.86 + 0.06 * rng.next()
                    };
                    out_at(a, u)
                })
                .collect();
            closed(out, edge, &pts);
            // the main limbs, forking as they near the rim
            for l in g.limbs.iter().skip(1).filter(|l| l.main) {
                let (a, b) = (c + l.a.truncate(), c + l.b.truncate());
                if a.distance(b) < 1e-3 {
                    continue;
                }
                line(out, inner, a, b);
            }
            let ends: Vec<DVec2> = g
                .limbs
                .iter()
                .enumerate()
                .skip(1)
                .filter(|(i, l)| l.main && g.limbs.get(i + 1).is_none_or(|n| n.a != l.b))
                .map(|(_, l)| c + l.b.truncate())
                .collect();
            for b in ends {
                let base = (b - c).to_angle();
                for s in [-1.0, 1.0] {
                    let to = out_at(base + s * (0.12 + 0.12 * rng.next()), 0.78);
                    if to.distance(c) > b.distance(c) {
                        line(out, inner, b, to);
                    }
                }
            }
        }
    }
    if !shape.plan_trunk {
        return;
    }
    // the trunk, and a cross on it
    let rt = (shape.trunk / 2.0).max(0.05);
    out.push(PlanItem::Arc {
        id,
        pen: Pen::Symbol,
        center: c,
        radius: rt,
        start: 0.0,
        end: TAU,
    });
    let d = (0..8)
        .map(|i| g.reach(TAU * i as f64 / 8.0))
        .fold(f64::INFINITY, f64::min)
        * 0.25;
    out.push(PlanItem::Line {
        id,
        pen: Pen::Symbol,
        a: c - DVec2::X * d,
        b: c + DVec2::X * d,
    });
    out.push(PlanItem::Line {
        id,
        pen: Pen::Symbol,
        a: c - DVec2::Y * d,
        b: c + DVec2::Y * d,
    });
}

/// Is `p` (about the trunk) under a clump of `shell` other than `own`
/// that stands higher, or nearer, by `rank`?
fn covered(shell: &[&Clump], own: usize, p: DVec2, rank: impl Fn(&Clump) -> f64) -> bool {
    let me = rank(shell[own]);
    shell
        .iter()
        .enumerate()
        .any(|(i, k)| i != own && rank(k) > me && p.distance(k.center.truncate()) < k.radius * 0.98)
}

/// A small soft blob about `r` across at `p`: a mass of leaves, or a gap
/// between them.
fn blob(out: &mut Vec<PlanItem>, id: ElementId, pen: Pen, p: DVec2, r: f64, rng: &mut Rng) {
    let n = 14;
    let ph = rng.next() * 50.0;
    let squash = 0.7 + 0.3 * rng.next();
    let turn = rng.next() * TAU;
    let pts: Vec<DVec2> = (0..n)
        .map(|i| {
            let a = TAU * i as f64 / n as f64;
            let w = 0.75 + 0.5 * noise(DVec3::new(a.cos() * 1.3 + ph, a.sin() * 1.3, ph));
            p + rotate(DVec2::new(a.cos(), a.sin() * squash) * r * w, turn)
        })
        .collect();
    for i in 0..n {
        out.push(PlanItem::Line {
            id,
            pen,
            a: pts[i],
            b: pts[(i + 1) % n],
        });
    }
}

/// Leaves scattered over a crown `area` across: short strokes, as a
/// hand draws foliage.
fn leaves(
    out: &mut Vec<PlanItem>,
    id: ElementId,
    pen: Pen,
    rng: &mut Rng,
    area: f64,
    unit: f64,
    scatter: &dyn Fn(&mut Rng, f64, f64) -> DVec2,
) {
    let n = ((area / (0.35 * unit * unit)).round() as usize).clamp(12, 160);
    for _ in 0..n {
        let p = scatter(rng, 0.0, 0.88);
        let d = DVec2::from_angle(rng.next() * TAU) * (0.06 + 0.05 * rng.next()) * unit;
        out.push(PlanItem::Line {
            id,
            pen,
            a: p - d,
            b: p + d,
        });
    }
}

/// An arc from `p` to `q` bulging outwards (to the right going from `p`
/// to `q` counter-clockwise round the tree).
fn scallop(out: &mut Vec<PlanItem>, id: ElementId, pen: Pen, p: DVec2, q: DVec2) {
    let d = q - p;
    let c = d.length();
    if c < 1e-6 {
        return;
    }
    let outward = DVec2::new(d.y, -d.x) / c;
    let sag = 0.22 * c;
    let rho = (c * c / 4.0 + sag * sag) / (2.0 * sag);
    let center = (p + q) / 2.0 - outward * (rho - sag);
    let start = (p - center).to_angle();
    let mut end = (q - center).to_angle();
    while end <= start {
        end += TAU;
    }
    out.push(PlanItem::Arc {
        id,
        pen,
        center,
        radius: rho,
        start,
        end,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::tree::TreeCut;
    use creabim_core::tree::TreePreset;

    #[test]
    fn the_same_shape_grows_the_same_tree_and_is_kept() {
        let s = TreeShape::default();
        let a = grow(&s, 4.0, 6.0);
        let b = grow(&s, 4.0, 6.0);
        assert!(Arc::ptr_eq(&a, &b), "the second is the cached one");
        let fresh = grow_uncached(&s, 4.0, 6.0);
        assert_eq!(fresh.foliage, a.foliage);
        let other = grow(&TreeShape { seed: 2, ..s }, 4.0, 6.0);
        assert_ne!(other.foliage, a.foliage);
    }

    #[test]
    fn the_crown_reaches_the_size_typed() {
        for p in TreePreset::ALL {
            let (s, canopy, height) = p.shape();
            let g = grow_uncached(&s, canopy, height);
            let (lo, hi) = g.foliage.bounds().unwrap();
            // the noise may carry a clump a little past the envelope
            let across = (hi.x - lo.x) as f64;
            assert!(
                (across - canopy).abs() < canopy * 0.2,
                "{} {across} vs {canopy}",
                p.label()
            );
            assert!(
                (hi.z as f64 - height).abs() < height * 0.12,
                "{} top {}",
                p.label(),
                hi.z
            );
            assert!(
                (lo.z as f64) > s.crown_base * 0.7,
                "{} underside {}",
                p.label(),
                lo.z
            );
            assert!(g.outline.iter().all(|r| *r > 0.0 && *r < canopy));
        }
    }

    #[test]
    fn a_side_pulled_in_keeps_off_a_wall() {
        let mut s = TreeShape {
            depth: 6.0,
            ..TreeShape::default()
        };
        let full = grow_uncached(&s, 6.0, 8.0);
        s.reach[0] = 0.4; // the east side, as against a wall 1.5 m east
        let held = grow_uncached(&s, 6.0, 8.0);
        assert!(held.reach(0.0) < 1.2 + 1e-6, "{}", held.reach(0.0));
        assert!(full.reach(0.0) > 2.4);
        // the west side is as it was
        assert!((held.reach(PI) - full.reach(PI)).abs() < 0.6);
        let (_, hi) = held.foliage.bounds().unwrap();
        assert!((hi.x as f64) < 1.2 + 1e-3, "{}", hi.x);
    }

    /// Every edge of `m` between exactly two of its faces: a closed body.
    fn closed(m: &TriMesh) -> bool {
        let key = |p: [f32; 3]| p.map(|v| (v as f64 * 1e4).round() as i64);
        let mut edges: HashMap<([i64; 3], [i64; 3]), i32> = HashMap::new();
        for t in m.indices.chunks(3) {
            for k in 0..3 {
                let (a, b) = (
                    key(m.positions[t[k] as usize]),
                    key(m.positions[t[(k + 1) % 3] as usize]),
                );
                if a == b {
                    continue;
                }
                *edges.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        edges.values().all(|n| *n == 2)
    }

    #[test]
    fn every_clump_form_grows_to_the_size_typed() {
        let (base, canopy, height) = TreePreset::Shade.shape();
        for form in ClumpForm::ALL {
            for clumps in [1, 12] {
                let s = TreeShape {
                    form,
                    clumps,
                    ..base.clone()
                };
                let g = grow_uncached(&s, canopy, height);
                let (lo, hi) = g.foliage.bounds().unwrap();
                let across = (hi.x - lo.x) as f64;
                let label = format!("{} with {clumps}", form.label());
                // a crown is the typed canopy at its widest, its depth
                // across the other way
                assert!(
                    across > canopy * 0.72 && across < canopy * 1.05,
                    "{label}: {across}"
                );
                assert!((hi.z as f64) <= height + 1e-3, "{label}: top {}", hi.z);
                assert!((hi.z as f64) > height * 0.85, "{label}: top {}", hi.z);
                assert!(g.foliage.triangle_count() < 20_000, "{label}");
                assert!(!g.foliage.is_empty(), "{label}");
            }
        }
    }

    #[test]
    fn a_wrapped_sphere_has_every_point_on_it() {
        let golden = PI * (3.0 - 5f64.sqrt());
        let pts: Vec<DVec3> = (0..40)
            .map(|j| {
                let z = 1.0 - 2.0 * (j as f64 + 0.5) / 40.0;
                DVec2::from_angle(j as f64 * golden).extend(0.0) * (1.0 - z * z).sqrt()
                    + DVec3::Z * z
            })
            .collect();
        let f = hull(&pts);
        assert_eq!(f.len(), 2 * 40 - 4);
        let mut m = TriMesh::default();
        add_flat(&mut m, &pts, &f);
        assert!(closed(&m));
        // wound outwards
        for t in &f {
            let [a, b, c] = t.map(|i| pts[i as usize]);
            assert!((b - a).cross(c - a).dot(a + b + c) > 0.0);
        }
    }

    /// The limbs never show past the crown: every main limb ends inside a
    /// clump, and no wood reaches further out than the foliage does, even
    /// with a side pulled in (the limbs that once stuck out sideways).
    #[test]
    fn limbs_end_in_the_crown_and_never_poke_out() {
        use creabim_core::tree::Branching;
        for p in TreePreset::ALL {
            for branching in Branching::ALL {
                for pulled in [None, Some(0), Some(7)] {
                    let (mut s, canopy, height) = p.shape();
                    s.branching = branching;
                    if let Some(i) = pulled {
                        s.reach[i] = 0.4;
                    }
                    let g = grow_uncached(&s, canopy, height);
                    let label = format!("{} {} {pulled:?}", p.label(), branching.label());
                    let within = |q: DVec3| {
                        g.clumps.iter().any(|c| {
                            let d = q - c.center;
                            (d.truncate() / c.radius).length_squared()
                                + (d.z / c.half_height).powi(2)
                                <= 1.0 + 1e-6
                        })
                    };
                    // each limb's last length ends in the foliage
                    assert!(g.limbs.iter().skip(1).any(|l| l.main), "{label}");
                    for (i, l) in g.limbs.iter().enumerate().skip(1) {
                        let last = g.limbs.get(i + 1).is_none_or(|n| n.a != l.b);
                        if last {
                            assert!(within(l.b), "{label}: a limb ends at {:?}", l.b);
                        }
                    }
                    for p in &g.wood.positions {
                        let q = glam::Vec3::from(*p).as_dvec3();
                        let r = q.truncate().length();
                        if r < s.trunk {
                            continue;
                        }
                        let reach = g.reach(q.y.atan2(q.x));
                        assert!(r <= reach + 0.05, "{label}: wood at {r:.2} past {reach:.2}");
                        assert!(q.z <= height, "{label}: wood above the top");
                    }
                }
            }
        }
    }

    #[test]
    fn solid_wood_is_closed_and_hollow_is_open() {
        let (s, canopy, height) = TreePreset::Shade.shape();
        let solid = grow_uncached(&s, canopy, height);
        assert!(s.solid, "solid by default");
        assert!(closed(&solid.wood));
        let hollow = grow_uncached(&TreeShape { solid: false, ..s }, canopy, height);
        assert!(!closed(&hollow.wood));
    }

    #[test]
    fn a_low_poly_crown_is_one_closed_body_of_few_faces() {
        let (s, canopy, height) = TreePreset::Shade.shape();
        let mut faces = vec![];
        for facets in 1..=5 {
            let mut s = TreeShape {
                form: ClumpForm::LowPoly,
                facets,
                ..s.clone()
            };
            s.reach[0] = 0.4;
            let g = grow_uncached(&s, canopy, height);
            assert!(closed(&g.foliage), "{facets} facets");
            let (lo, hi) = g.foliage.bounds().unwrap();
            // it keeps off the wall the side was pulled in from
            assert!(
                (hi.x as f64) <= s.radius_local(canopy, 0.0) + 1e-3,
                "{}",
                hi.x
            );
            assert!((hi.z as f64) <= height + 1e-3);
            assert!((lo.x as f64) < -canopy * 0.35);
            faces.push(g.foliage.triangle_count());
        }
        // more facets, more faces, never thousands
        assert!(faces.windows(2).all(|w| w[0] < w[1]), "{faces:?}");
        assert!(faces[0] <= 30 && faces[4] <= 200, "{faces:?}");
    }

    /// How many filled cut pieces a section through a tree's trunk gives,
    /// the crown drawn as `elevation`, its wood `solid` or not.
    fn cut_pieces(elevation: TreeElevation, solid: bool, cut: TreeCut) -> Cut {
        use creabim_core::{Element, ElementKind, Level, Model, Site, Transaction};
        let lv = Element::new(
            "Site",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let l = lv.id;
        let mut m = Model::from_elements([lv]).unwrap();
        let (s, c, h) = TreePreset::Shade.shape();
        let shape = TreeShape {
            elevation,
            solid,
            cut,
            ..s
        };
        let tree = Element::new(
            "Tree",
            ElementKind::Site(Site::tree(l, DVec2::ZERO, shape, c, h)),
        );
        let id = tree.id;
        Transaction::new("plant")
            .insert(tree)
            .apply(&mut m)
            .unwrap();
        let d = crate::section::section_view(
            &m,
            DVec2::new(-6.0, 0.0),
            DVec2::new(6.0, 0.0),
            10.0,
            true,
            false,
        );
        let mine = |i: &&PlanItem| match i {
            PlanItem::Fill { id: e, .. }
            | PlanItem::Shade { id: e, .. }
            | PlanItem::Line { id: e, .. } => *e == id,
            _ => false,
        };
        let items: Vec<&PlanItem> = d.items.iter().filter(mine).collect();
        Cut {
            fills: items
                .iter()
                .filter(|i| matches!(i, PlanItem::Fill { .. }))
                .count(),
            washes: items
                .iter()
                .filter_map(|i| match i {
                    PlanItem::Shade { rgb, .. } => Some(*rgb),
                    _ => None,
                })
                .collect(),
            heavy: items
                .iter()
                .filter(|i| matches!(i, PlanItem::Line { pen: Pen::Cut, .. }))
                .count(),
            fine: items
                .iter()
                .filter(|i| matches!(i, PlanItem::Line { pen: Pen::Fine, .. }))
                .count(),
        }
    }

    struct Cut {
        fills: usize,
        washes: Vec<[f32; 3]>,
        heavy: usize,
        fine: usize,
    }

    #[test]
    fn a_section_draws_the_cut_crown_in_its_style_and_fills_solid_wood() {
        // the crown's cut: heavy linework over the wash asked for, never
        // a grey slab
        let c = cut_pieces(TreeElevation::Clumps, false, TreeCut::Tinted);
        assert_eq!(c.fills, 0, "hollow wood and the crown are not filled");
        assert!(c.heavy > 20, "{} heavy lines", c.heavy);
        assert!(c.fine > 0, "the clumps inside the cut");
        assert_eq!(c.washes, vec![CANOPY_TINT]);
        let c = cut_pieces(TreeElevation::Outline, true, TreeCut::Outline);
        assert!(c.washes.is_empty());
        assert!(c.fills >= 1, "the trunk's cut is filled");
        let c = cut_pieces(TreeElevation::Wavy, false, TreeCut::Filled);
        assert_eq!(c.washes.len(), 1);
        assert!(c.washes[0][0] < 0.8 && c.washes[0][0] == c.washes[0][1]);
        assert!(c.heavy > 40 && c.fine > 10, "{} {}", c.heavy, c.fine);
    }

    /// A model of one tree of `shape` at the origin, and its id.
    fn one_tree(shape: TreeShape, canopy: f64, height: f64) -> (creabim_core::Model, ElementId) {
        use creabim_core::{Element, ElementKind, Level, Model, Site, Transaction};
        let lv = Element::new(
            "Site",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let l = lv.id;
        let mut m = Model::from_elements([lv]).unwrap();
        let tree = Element::new(
            "Tree",
            ElementKind::Site(Site::tree(l, DVec2::ZERO, shape, canopy, height)),
        );
        let id = tree.id;
        Transaction::new("plant")
            .insert(tree)
            .apply(&mut m)
            .unwrap();
        (m, id)
    }

    /// The pens of a tree's lines in an elevation from the south.
    fn elevation_pens(shape: TreeShape) -> Vec<Pen> {
        let (_, c, h) = TreePreset::Shade.shape();
        let (m, id) = one_tree(shape, c, h);
        let d = crate::section::section_view(
            &m,
            DVec2::new(-8.0, -10.0),
            DVec2::new(8.0, -10.0),
            30.0,
            false,
            false,
        );
        d.items
            .iter()
            .filter_map(|i| match i {
                PlanItem::Line { id: e, pen, .. } if *e == id => Some(*pen),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn elevations_draw_a_crown_in_a_style_of_its_own() {
        let (s, ..) = TreePreset::Shade.shape();
        let fine = |p: &[Pen]| p.iter().filter(|p| **p == Pen::Fine).count();
        let clumps = elevation_pens(s.clone());
        assert!(clumps.len() > 20);
        assert_eq!(fine(&clumps), 0);
        for elevation in [
            TreeElevation::Wavy,
            TreeElevation::Scalloped,
            TreeElevation::Leafy,
        ] {
            let pens = elevation_pens(TreeShape {
                elevation,
                ..s.clone()
            });
            // its outline, and the leaves or gaps inside it, light
            assert!(fine(&pens) > 10, "{elevation:?}: {}", fine(&pens));
            assert!(pens.contains(&Pen::Projection), "{elevation:?}");
        }
        // the limbs through the crown, in any style
        let limbs = elevation_pens(TreeShape {
            elevation_limbs: true,
            ..s.clone()
        });
        let sym = |p: &[Pen]| p.iter().filter(|p| **p == Pen::Symbol).count();
        assert!(sym(&limbs) > sym(&clumps) + 4, "{} limb lines", sym(&limbs));
    }

    #[test]
    fn elevation_lines_follow_the_crown() {
        let (s, canopy, height) = TreePreset::Shade.shape();
        for elevation in [
            TreeElevation::Wavy,
            TreeElevation::Scalloped,
            TreeElevation::Leafy,
        ] {
            let s = TreeShape {
                elevation,
                ..s.clone()
            };
            let g = grow_uncached(&s, canopy, height);
            let lines = elevation_lines(&g, &s, DVec2::X);
            let (lo, hi) = lines.iter().fold(
                (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
                |(lo, hi), (_, a, b)| (lo.min(a.min(*b)), hi.max(a.max(*b))),
            );
            // as wide as the crown and up to its top, no further
            assert!(
                hi.x - lo.x > canopy * 0.8 && hi.x - lo.x < canopy * 1.1,
                "{lo} {hi}"
            );
            assert!(
                hi.y <= height + 0.2 && lo.y > s.crown_base - 0.5,
                "{lo} {hi}"
            );
        }
    }

    #[test]
    fn every_plan_style_keeps_to_the_crown() {
        let (s, canopy, height) = TreePreset::Shade.shape();
        let g = grow_uncached(&s, canopy, height);
        let c = DVec2::new(10.0, 5.0);
        for plan in TreePlan::ALL {
            for bold in [false, true] {
                let s = TreeShape {
                    plan,
                    plan_bold: bold,
                    plan_trunk: false,
                    ..s.clone()
                };
                let mut items = vec![];
                plan_symbol(&mut items, ElementId::new(), c, &g, &s);
                assert!(items.len() >= 7, "{plan:?}");
                assert!(items.len() <= 700, "{plan:?}: {} items", items.len());
                for i in &items {
                    let pts: Vec<DVec2> = match i {
                        PlanItem::Line { a, b, .. } => vec![*a, *b],
                        PlanItem::Arc { center, radius, .. } => {
                            vec![*center + DVec2::X * *radius]
                        }
                        _ => vec![],
                    };
                    for p in pts {
                        let d = p - c;
                        let r = g.reach(d.to_angle());
                        assert!(
                            d.length() <= r * 1.03 + 0.06,
                            "{plan:?} {} > {r}",
                            d.length()
                        );
                    }
                }
                // bold: the outline heavy, never what is inside it
                let cut = items
                    .iter()
                    .filter(|i| {
                        matches!(
                            i,
                            PlanItem::Line { pen: Pen::Cut, .. }
                                | PlanItem::Arc { pen: Pen::Cut, .. }
                        )
                    })
                    .count();
                let rimless = matches!(plan, TreePlan::Skeleton | TreePlan::Starburst);
                if bold && !rimless {
                    assert!(cut > 0, "{plan:?}");
                } else if !bold {
                    assert_eq!(cut, 0, "{plan:?}");
                }
            }
        }
    }

    #[test]
    fn detail_levels_trade_faces_for_speed() {
        let (s, canopy, height) = TreePreset::Shade.shape();
        let tris = |detail| {
            let g = grow_uncached(
                &TreeShape {
                    detail,
                    ..s.clone()
                },
                canopy,
                height,
            );
            (
                g.foliage.triangle_count() + g.wood.triangle_count(),
                g.limbs.len(),
                g.outline.clone(),
            )
        };
        let (lite, _, o1) = tris(TreeDetail::Lite);
        let (every, l2, o2) = tris(TreeDetail::Everyday);
        let (pres, l3, _) = tris(TreeDetail::Presentation);
        assert!(lite * 3 < every, "{lite} vs {every}");
        assert!(every * 2 < pres, "{every} vs {pres}");
        assert!(l3 > l2, "twigs for close views");
        // the plan never changes with it
        assert_eq!(o1, o2);
    }

    #[test]
    fn a_bare_tree_drops_its_leaves_in_3d_but_not_its_plan() {
        use creabim_core::tree::LeafHabit;
        let (s, canopy, height) = TreePreset::Shade.shape();
        let winter = TreeShape {
            leaves: LeafHabit::Deciduous,
            month: 1,
            own_month: true,
            ..s.clone()
        };
        let (m, id) = one_tree(winter.clone(), canopy, height);
        let e = m.get(id).unwrap();
        let items = crate::parts::scene_items(&m, e);
        assert!(items.iter().all(|i| i.finish != Some(FOLIAGE)), "bare");
        let plan = crate::parts::plan_items(&m, e, 1.0);
        let (m2, id2) = one_tree(s.clone(), canopy, height);
        assert_eq!(
            plan.len(),
            crate::parts::plan_items(&m2, m2.get(id2).unwrap(), 1.0).len()
        );
        // half in leaf: fewer, thinner clumps, the same ones each time
        let g = grow_uncached(&s, canopy, height);
        let half = g.foliage_at(0.5);
        assert!(half.triangle_count() > 0);
        assert!(half.triangle_count() < g.foliage.triangle_count());
        assert_eq!(half.positions, g.foliage_at(0.5).positions);
        // in autumn its crown turns
        let autumn = TreeShape {
            leaves: LeafHabit::Deciduous,
            month: 10,
            own_month: true,
            ..s
        };
        let (m3, id3) = one_tree(autumn.clone(), canopy, height);
        let crown = crate::parts::scene_items(&m3, m3.get(id3).unwrap())
            .into_iter()
            .find(|i| i.finish == Some(FOLIAGE))
            .unwrap();
        assert_eq!(crown.color, autumn.foliage_colour(false));
        assert!(crown.color[0] > autumn.leaf_colour[0] + 0.2);
    }

    /// `cargo test -p creabim-geom --release planted_site -- --ignored --nocapture`
    #[test]
    #[ignore = "a timing, not a check"]
    fn planted_site() {
        use creabim_core::{Element, ElementKind, Level, Model, Site, Transaction};
        let lv = Element::new(
            "Site",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let l = lv.id;
        for n in [1usize, 25, 100] {
            let mut m = Model::from_elements([lv.clone()]).unwrap();
            let side = (n as f64).sqrt().ceil() as usize;
            let mut tx = Transaction::new("plant");
            for i in 0..n {
                let at = DVec2::new((i % side) as f64 * 9.0, (i / side) as f64 * 9.0);
                let (shape, c, h) = TreePreset::Shade.shape();
                let shape = TreeShape {
                    seed: creabim_core::tree::seed_at(at),
                    ..shape
                };
                tx = tx.insert(Element::new(
                    "Tree",
                    ElementKind::Site(Site::tree(l, at, shape, c, h)),
                ));
            }
            tx.apply(&mut m).unwrap();
            let t0 = std::time::Instant::now();
            let cold = crate::scene::build_scene(&m);
            let t1 = t0.elapsed();
            let t0 = std::time::Instant::now();
            let warm = crate::scene::build_scene(&m);
            let t2 = t0.elapsed();
            let tris: usize = warm.iter().map(|i| i.mesh.triangle_count()).sum();
            let t0 = std::time::Instant::now();
            let plan: usize = m
                .iter()
                .map(|e| crate::parts::plan_items(&m, e, 1.2).len())
                .sum();
            let t3 = t0.elapsed();
            println!(
                "{n:>3} trees: first build {:>7.1} ms, rebuilt after an edit {:>6.1} ms, {tris} triangles, plan {plan} items in {:.1} ms",
                t1.as_secs_f64() * 1e3,
                t2.as_secs_f64() * 1e3,
                t3.as_secs_f64() * 1e3
            );
            assert_eq!(cold.len(), warm.len());
        }
        // each form, grown once
        let (base, c, h) = TreePreset::Shade.shape();
        for form in ClumpForm::ALL {
            for elevation in [TreeElevation::Clumps, TreeElevation::Outline] {
                let s = TreeShape {
                    form,
                    elevation,
                    ..base.clone()
                };
                let t0 = std::time::Instant::now();
                let g = grow_uncached(&s, c, h);
                println!(
                    "{:>14} drawn as {:<8} {:>6.2} ms, {} triangles",
                    form.label(),
                    elevation.label(),
                    t0.elapsed().as_secs_f64() * 1e3,
                    g.foliage.triangle_count() + g.wood.triangle_count()
                );
            }
        }
    }

    /// `cargo test -p creabim-geom --release styles_and_detail -- --ignored --nocapture`
    #[test]
    #[ignore = "a timing, not a check"]
    fn styles_and_detail() {
        let (base, c, h) = TreePreset::Shade.shape();
        for detail in TreeDetail::ALL {
            let s = TreeShape {
                detail,
                ..base.clone()
            };
            let t0 = std::time::Instant::now();
            let g = grow_uncached(&s, c, h);
            println!(
                "{:>12}: {:>6.2} ms, {} triangles",
                detail.label(),
                t0.elapsed().as_secs_f64() * 1e3,
                g.foliage.triangle_count() + g.wood.triangle_count()
            );
        }
        let g = grow_uncached(&base, c, h);
        for plan in TreePlan::ALL {
            let s = TreeShape {
                plan,
                ..base.clone()
            };
            let t0 = std::time::Instant::now();
            let mut items = vec![];
            plan_symbol(&mut items, ElementId::new(), DVec2::ZERO, &g, &s);
            println!(
                "{:>12} plan: {:>6.3} ms, {} items",
                plan.label(),
                t0.elapsed().as_secs_f64() * 1e3,
                items.len()
            );
        }
        for elevation in [
            TreeElevation::Wavy,
            TreeElevation::Scalloped,
            TreeElevation::Leafy,
        ] {
            let s = TreeShape {
                elevation,
                ..base.clone()
            };
            let t0 = std::time::Instant::now();
            let lines = elevation_lines(&g, &s, DVec2::X);
            println!(
                "{:>12} elevation: {:>6.3} ms, {} lines",
                elevation.label(),
                t0.elapsed().as_secs_f64() * 1e3,
                lines.len()
            );
        }
    }

    #[test]
    fn a_tree_is_light() {
        let (s, canopy, height) = TreePreset::Shade.shape();
        let g = grow_uncached(&s, canopy, height);
        let tris = g.foliage.triangle_count() + g.wood.triangle_count();
        assert!(tris < 6000, "{tris} triangles");
        let mut items = vec![];
        plan_symbol(&mut items, ElementId::new(), DVec2::ZERO, &g, &s);
        assert!(items.len() <= 45, "{} plan items", items.len());
    }
}
