//! The proposed ground: the existing ground with the design worked in.
//!
//! A terrain's data is the ground as surveyed. Its pads, and the holes
//! dug under the building's slabs, are worked into it one after another:
//! each levels the ground inside its outline and runs a bank out from its
//! edge, up where it digs and down where it makes up, to the line where
//! the bank meets the ground as it was (the daylight line). The result is
//! a ground like any other, built from the survey's data outside the
//! works and from the pad's edge and its daylight line inside them, so
//! plans, sections, 3D and contours draw it as they draw a survey.
//!
//! The volumes dug and made up are measured between the two grounds.

use crate::ground::Ground;
use creabim_core::{ElementKind, GroundLine, GroundLineKind, Model, Pad, Site, SiteForm};
use glam::{DVec2, DVec3};
use std::sync::Arc;

/// An upright face is drawn this far out at its top, so the ground stays
/// one surface with one height at every point.
const UPRIGHT: f64 = 0.02;
/// A sloping pad's sunk floor starts this far inside its edge.
const SUNK_INSET: f64 = 0.03;

/// A closed outline moved `d` inward, its corners mitred.
fn inset(outline: &[DVec2], d: f64) -> Vec<DVec2> {
    let n = outline.len();
    let sign = if creabim_core::polygon::signed_area2(outline) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    (0..n)
        .map(|i| {
            let (p, q, r) = (outline[(i + n - 1) % n], outline[i], outline[(i + 1) % n]);
            let (na, nb) = (
                (q - p).normalize_or_zero().perp(),
                (r - q).normalize_or_zero().perp(),
            );
            let m = (na + nb).normalize_or(na);
            q + m * (sign * d / m.dot(na).max(0.25))
        })
        .collect()
}

/// Banks are followed no further than this from a pad's edge.
const REACH: f64 = 80.0;

/// One bank of a pad: pairs of a point on the pad's edge and the point
/// straight out from it where its bank meets the ground, round the pad.
pub type Bank = Vec<(DVec3, DVec3)>;

/// A terrain's two grounds and what lies between them.
pub struct Shaped {
    /// As surveyed.
    pub existing: Arc<Ground>,
    /// With the pads and the building's holes worked in; the existing
    /// ground itself where nothing changes it.
    pub ground: Arc<Ground>,
    /// The banks, one run for each pad that changed the ground.
    pub banks: Vec<Bank>,
    /// Dug out, in cubic metres.
    pub cut: f64,
    /// Made up, in cubic metres.
    pub fill: f64,
    /// The area in plan the works change, in square metres.
    pub area: f64,
    /// What is dug out, split by the terrain's soil layers from the top
    /// down, with what comes from below the last of them at the end.
    pub cut_layers: Vec<f64>,
}

impl Shaped {
    /// Does the design change the ground at all?
    pub fn changed(&self) -> bool {
        !Arc::ptr_eq(&self.existing, &self.ground)
    }
}

/// The holes a building needs in a terrain: one under each slab that
/// stands on it, down to its underside, with upright sides.
///
/// Footings and the walls on them dig none. Their trenches are filled
/// back once they are cast, so the finished ground lies against them as
/// it lay before: a section then draws earth up to the footing and the
/// wall, not an open trench beside them (owner markup MU122).
pub fn building_pads(model: &Model, site: &Site) -> Vec<Pad> {
    let SiteForm::Terrain { boundary, .. } = &site.form else {
        return vec![];
    };
    let base = creabim_core::members::elevation(model, site.level);
    let mut out: Vec<(f64, Pad)> = vec![];
    for e in model.iter() {
        let ElementKind::Slab(s) = &e.kind else {
            continue;
        };
        let Some(level) = model.level(s.level) else {
            continue;
        };
        let outline = s.outline();
        if outline.len() < 3
            || !outline
                .iter()
                .any(|p| creabim_core::polygon::contains(boundary, *p))
        {
            continue;
        }
        let underside = slab_underside(model, s, level.elevation);
        // a framed floor stands over a crawl space: the ground is dug the
        // project's room below its joists (MU140), unless a pad drawn under
        // it below its underside says where its floor is
        let under = if framed(model, s) {
            let mid = outline.iter().copied().sum::<DVec2>() / outline.len() as f64;
            let drawn = drawn_pads(model).into_iter().any(|(_, at, o)| {
                at < underside - 1e-6 && creabim_core::polygon::contains(&o, mid)
            });
            if drawn {
                continue;
            }
            underside - creabim_core::GroundRules::of(model).crawl - base
        } else {
            underside - base
        };
        out.push((
            under,
            Pad {
                outline,
                level: under,
                cut: 0.0,
                fill: 0.0,
                only_cut: true,
                depth: 0.0,
                heights: vec![],
                sunk: 0.0,
                upright: vec![],
                behind: vec![],
            },
        ));
    }
    // deepest last, so a basement's hole is not filled by the floor above
    out.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.into_iter().map(|p| p.1).collect()
}

/// A slab's underside, with its level at `elevation`.
pub fn slab_underside(model: &Model, s: &creabim_core::Slab, elevation: f64) -> f64 {
    let thickness = match s.assembly.and_then(|a| model.get(a)).map(|a| &a.kind) {
        Some(ElementKind::Assembly(a)) => a.total(),
        _ => s.thickness,
    };
    s.top(elevation) - thickness
}

/// Is material `m` wood: by its hatch, its category, or its name (a stud
/// frame, joists, sheathing, cladding)?
fn wooden(model: &Model, m: Option<creabim_core::ElementId>) -> bool {
    let Some(e) = m.and_then(|m| model.get(m)) else {
        return false;
    };
    let ElementKind::Material(mat) = &e.kind else {
        return false;
    };
    let name = e.name.to_lowercase();
    mat.hatch == creabim_core::Hatch::Timber
        || mat.category.eq_ignore_ascii_case("wood")
        || [
            "timber", "wood", "stud", "joist", "lumber", "plywood", "osb", "cedar", "glulam", "clt",
        ]
        .iter()
        .any(|w| name.contains(w))
}

/// Does what a slab carries its weight on be timber: its assembly's
/// structure layer, or its own material? A floor of joists.
pub fn timber(
    model: &Model,
    assembly: Option<creabim_core::ElementId>,
    material: Option<creabim_core::ElementId>,
) -> bool {
    match assembly.and_then(|a| model.get(a)).map(|a| &a.kind) {
        Some(ElementKind::Assembly(a)) => a
            .layers
            .iter()
            .find(|l| l.function == creabim_core::LayerFunction::Structure)
            .is_some_and(|l| wooden(model, l.material)),
        _ => wooden(model, material),
    }
}

/// Has a wall wood in it anywhere (studs, sheathing, cladding)? The ground
/// keeps off all of it.
pub fn has_wood(
    model: &Model,
    assembly: Option<creabim_core::ElementId>,
    material: Option<creabim_core::ElementId>,
) -> bool {
    match assembly.and_then(|a| model.get(a)).map(|a| &a.kind) {
        Some(ElementKind::Assembly(a)) => a.layers.iter().any(|l| wooden(model, l.material)),
        _ => wooden(model, material),
    }
}

/// Is slab `s` a framed floor (joists) standing on stem walls, so over a
/// crawl space? A timber deck on sleepers, or an upper floor, is not.
pub fn framed(model: &Model, s: &creabim_core::Slab) -> bool {
    if !timber(model, s.assembly, s.material) {
        return false;
    }
    let Some(level) = model.level(s.level) else {
        return false;
    };
    let (top, under) = (
        s.top(level.elevation),
        slab_underside(model, s, level.elevation),
    );
    let outline = s.outline();
    let n = outline.len();
    if n < 3 {
        return false;
    }
    let near_edge = |p: DVec2| {
        (0..n).any(|i| {
            let (a, b) = (outline[i], outline[(i + 1) % n]);
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
            (a + ab * t).distance(p) < 0.6
        })
    };
    // a wall of concrete or masonry under its edge, its top at the floor
    model.iter().any(|e| match &e.kind {
        ElementKind::Wall(w) if w.curtain.is_none() && !has_wood(model, w.assembly, w.material) => {
            let foot = creabim_core::walls::base_z(model, w);
            let wall_top =
                foot + creabim_core::walls::constrained_height(model, w).unwrap_or(w.height);
            wall_top > under - 0.05
                && wall_top < top + 0.05
                && foot < under
                && near_edge((w.start + w.end) / 2.0)
        }
        _ => false,
    })
}

/// The pads drawn in the model: each with its level (absolute) and outline.
fn drawn_pads(model: &Model) -> Vec<(creabim_core::ElementId, f64, Vec<DVec2>)> {
    model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Site(s) if matches!(s.form, SiteForm::Pad { .. }) => {
                let pad = s.as_work()?;
                let at = creabim_core::members::elevation(model, s.level) + pad.level;
                Some((e.id, at, pad.outline))
            }
            _ => None,
        })
        .collect()
}

/// What a pad's Properties say about what is built in it (MU140): a
/// note when all is well, or what is wrong and the pad level that puts it
/// right.
#[derive(Clone, Debug, PartialEq)]
pub enum PadNote {
    /// A slab sinks `by` into the pad; under it the ground is dug to
    /// `dug_to` (absolute).
    Sunk { slab: String, by: f64, dug_to: f64 },
    /// A framed floor stands over a crawl space with room enough: `room`
    /// under its joists, its floor `below` the pad (0 when it is the pad).
    Crawl {
        floor: String,
        room: f64,
        below: f64,
    },
    /// The pad lies `by` above the top of a slab it covers.
    Buried { slab: String, by: f64, fix: f64 },
    /// Under a framed floor the pad leaves only `room`, less than `need`.
    Shallow {
        floor: String,
        room: f64,
        need: f64,
        fix: f64,
    },
    /// The pad lies only `gap` below a timber wall's foot, less than `need`.
    AgainstWood {
        wall: String,
        gap: f64,
        need: f64,
        fix: f64,
    },
}

impl PadNote {
    /// Is it something wrong (an orange note), not an explanation?
    pub fn wrong(&self) -> bool {
        matches!(
            self,
            PadNote::Buried { .. } | PadNote::Shallow { .. } | PadNote::AgainstWood { .. }
        )
    }

    /// The pad's level (absolute) that puts it right, if it is wrong.
    pub fn fix(&self) -> Option<f64> {
        match self {
            PadNote::Buried { fix, .. }
            | PadNote::Shallow { fix, .. }
            | PadNote::AgainstWood { fix, .. } => Some(*fix),
            _ => None,
        }
    }
}

/// What pad `id` says about what is built in it: only what is wrong when
/// anything is (the orange notes), else what is built in it (the blue
/// ones). The two never show together.
pub fn pad_notes(model: &Model, id: creabim_core::ElementId) -> Vec<PadNote> {
    let Some((_, at, outline)) = drawn_pads(model).into_iter().find(|p| p.0 == id) else {
        return vec![];
    };
    if outline.len() < 3 {
        return vec![];
    }
    let rules = creabim_core::GroundRules::of(model);
    let (mut fine, mut wrong) = (vec![], vec![]);
    let floors: Vec<Vec<DVec2>> = model
        .iter()
        .filter_map(|f| match &f.kind {
            ElementKind::Slab(s) => Some(s.outline()),
            _ => None,
        })
        .collect();
    for e in model.iter() {
        match &e.kind {
            ElementKind::Slab(s) => {
                let Some(level) = model.level(s.level) else {
                    continue;
                };
                let slab = s.outline();
                if slab.len() < 3 || !creabim_core::polygon::overlap(&slab, &outline) {
                    continue;
                }
                let (top, under) = (
                    s.top(level.elevation),
                    slab_underside(model, s, level.elevation),
                );
                // a floor with another between it and the pad stands on that
                // one, not on the ground
                let between = model.iter().any(|o| match &o.kind {
                    ElementKind::Slab(t) if o.id != e.id => model.level(t.level).is_some_and(|l| {
                        let t_top = t.top(l.elevation);
                        let ol = t.outline();
                        t_top > at + 1e-6
                            && t_top <= under + 1e-6
                            && ol.len() >= 3
                            && creabim_core::polygon::overlap(&ol, &slab)
                    }),
                    _ => false,
                });
                if between {
                    continue;
                }
                if at >= top - 1e-6 {
                    let fix = top - rules.wood;
                    wrong.push(PadNote::Buried {
                        slab: e.name.clone(),
                        by: at - top,
                        fix,
                    });
                } else if framed(model, s) {
                    // its crawl floor: this pad where it lies under it (the
                    // floor then digs nothing, see building_pads), else the
                    // dig the floor makes for itself in the pad
                    let room = under - at;
                    if room <= 1e-6 {
                        let floor = under - rules.crawl;
                        fine.push(PadNote::Crawl {
                            floor: e.name.clone(),
                            room: rules.crawl,
                            below: at - floor,
                        });
                    } else if room + 1e-6 < rules.crawl {
                        let fix = under - rules.crawl;
                        wrong.push(PadNote::Shallow {
                            floor: e.name.clone(),
                            room,
                            need: rules.crawl,
                            fix,
                        });
                    } else {
                        fine.push(PadNote::Crawl {
                            floor: e.name.clone(),
                            room,
                            below: 0.0,
                        });
                    }
                } else if at > under + 1e-6 {
                    fine.push(PadNote::Sunk {
                        slab: e.name.clone(),
                        by: at - under,
                        dug_to: under,
                    });
                }
            }
            ElementKind::Wall(w)
                if w.curtain.is_none() && has_wood(model, w.assembly, w.material) =>
            {
                let foot = creabim_core::walls::base_z(model, w);
                let (a, b) = (w.start, w.end);
                // only where the ground lies against it: beside it, on one
                // side or the other, is pad and no floor (a partition
                // standing on the slab has the slab both sides)
                let mid = (a + b) / 2.0;
                let side = (b - a).normalize_or_zero().perp() * (w.thickness / 2.0 + 0.05);
                let on = [mid + side, mid - side].iter().any(|p| {
                    creabim_core::polygon::contains(&outline, *p)
                        && !floors
                            .iter()
                            .any(|f| f.len() >= 3 && creabim_core::polygon::contains(f, *p))
                });
                let gap = foot - at;
                if on && gap > -1e-6 && gap + 1e-6 < rules.wood {
                    let fix = foot - rules.wood;
                    if !wrong
                        .iter()
                        .any(|n| matches!(n, PadNote::AgainstWood { .. }))
                    {
                        wrong.push(PadNote::AgainstWood {
                            wall: e.name.clone(),
                            gap,
                            need: rules.wood,
                            fix,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    if wrong.is_empty() {
        fine
    } else {
        wrong
    }
}

/// How far above the ground under it a plant's level may be and the
/// plant still stand on that ground. Further up it keeps its level, but
/// only where a slab or roof at about that level is under it (a roof
/// garden, a terrace): on a lot falling away from the house a tree on the
/// ground floor's level stands on the ground, however far down (owner
/// film review 2026-11-22, Ridgeline's floating maples).
const ON_GROUND: f64 = 1.0;

/// Whether a slab or a roof at about height `z` is under `p`: what a plant
/// on a roof or terrace stands on.
fn built_under(model: &Model, p: DVec2, z: f64) -> bool {
    model.iter().any(|e| match &e.kind {
        ElementKind::Slab(s) => {
            let top = s.top(creabim_core::members::elevation(model, s.level));
            (top - z).abs() <= ON_GROUND && creabim_core::polygon::contains(&s.outline(), p)
        }
        ElementKind::Roof(r) => {
            let at = creabim_core::members::elevation(model, r.level) + r.base_offset;
            (at - z).abs() <= ON_GROUND && creabim_core::polygon::contains(&r.boundary, p)
        }
        _ => false,
    })
}

/// Where a plant stands: the worked ground under it, at the lowest point
/// round its trunk so it never floats on a slope, plus its `base`. None
/// when no terrain lies under it near its level; it then stands on its
/// level. Trees placed on a hill stood at the level and sank into it
/// (owner, cut test).
pub fn plant_foot(model: &Model, plant: &Site) -> Option<f64> {
    let SiteForm::Plant { position, base, .. } = &plant.form else {
        return None;
    };
    let r = match plant.tree_shape() {
        Some(t) => (t.trunk / 2.0).clamp(0.1, 1.0),
        None => 0.15,
    };
    let own = creabim_core::members::elevation(model, plant.level);
    let up = built_under(model, *position, own);
    let mut best: Option<f64> = None;
    for e in model.iter() {
        let ElementKind::Site(t) = &e.kind else {
            continue;
        };
        let SiteForm::Terrain { boundary, .. } = &t.form else {
            continue;
        };
        if !creabim_core::polygon::contains(boundary, *position) {
            continue;
        }
        let Some(g) = shaped(model, t) else {
            continue;
        };
        let tb = creabim_core::members::elevation(model, t.level);
        let Some(mid) = g.ground.height(*position) else {
            continue;
        };
        let low = (0..8)
            .filter_map(|i| {
                let a = i as f64 * std::f64::consts::FRAC_PI_4;
                g.ground.height(*position + DVec2::from_angle(a) * r)
            })
            .fold(mid, f64::min);
        let z = tb + low;
        if up && own > z + ON_GROUND {
            continue;
        }
        if best.is_none_or(|b| z > b) {
            best = Some(z);
        }
    }
    best.map(|z| z + base)
}

/// The worked ground's height above the project's zero at `p`: the
/// highest terrain's under it; none off them all.
pub fn ground_at(model: &Model, p: DVec2) -> Option<f64> {
    ground_sampler(model)(p)
}

/// [`ground_at`] for many points: the terrains and their worked grounds
/// found once. A lot's lines asked [`ground_at`] for every point, and each
/// call walked the model for the works again (MU142 lag: 63 ms a scene).
pub fn ground_sampler(model: &Model) -> impl Fn(DVec2) -> Option<f64> {
    let grounds: Vec<(Vec<DVec2>, Arc<Shaped>, f64)> = model
        .iter()
        .filter_map(|e| {
            let ElementKind::Site(t) = &e.kind else {
                return None;
            };
            let SiteForm::Terrain { boundary, .. } = &t.form else {
                return None;
            };
            let g = shaped(model, t)?;
            Some((
                boundary.clone(),
                g,
                creabim_core::members::elevation(model, t.level),
            ))
        })
        .collect();
    move |p| {
        let mut best: Option<f64> = None;
        for (boundary, g, base) in &grounds {
            if !creabim_core::polygon::contains(boundary, p) {
                continue;
            }
            let Some(z) = g.ground.height(p) else {
                continue;
            };
            let z = z + base;
            if best.is_none_or(|b| z > b) {
                best = Some(z);
            }
        }
        best
    }
}

/// The height a plant's foot is at: on the ground under it, or its level.
pub fn plant_z(model: &Model, plant: &Site) -> f64 {
    plant_foot(model, plant).unwrap_or_else(|| {
        let base = match &plant.form {
            SiteForm::Plant { base, .. } => *base,
            _ => 0.0,
        };
        creabim_core::members::elevation(model, plant.level) + base
    })
}

/// The ground as it was at `p`, above `zero` (the road's level), from
/// whichever terrain lies under it; none off them all.
fn ground_was(
    model: &Model,
    terrains: &[(Arc<Ground>, f64, Vec<DVec2>)],
    p: DVec2,
    zero: f64,
) -> Option<f64> {
    let _ = model;
    terrains
        .iter()
        .filter(|(_, _, b)| creabim_core::polygon::contains(b, p))
        .filter_map(|(g, tb, _)| g.height(p).map(|z| z + tb - zero))
        .reduce(f64::max)
}

/// The terrains of a model, each as its ground as surveyed, its level's
/// height and its boundary.
fn surveyed(model: &Model) -> Vec<(Arc<Ground>, f64, Vec<DVec2>, u64)> {
    let mut out = vec![];
    for e in model.iter() {
        let ElementKind::Site(t) = &e.kind else {
            continue;
        };
        let SiteForm::Terrain { boundary, .. } = &t.form else {
            continue;
        };
        let whole = creabim_core::site::whole_ground(model, t);
        let site = whole.as_ref().unwrap_or(t);
        let (Some(g), Some(k)) = (crate::ground::of(site), crate::ground::data_key(site)) else {
            continue;
        };
        out.push((
            g,
            creabim_core::members::elevation(model, t.level),
            boundary.clone(),
            k,
        ));
    }
    out
}

/// What a road's levels following the ground are worked out from.
fn fit_key(model: &Model, s: &Site, r: &creabim_core::road::Road, grounds: &[u64]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for p in &r.path {
        (p.x.to_bits(), p.y.to_bits()).hash(&mut h);
    }
    (r.centre_radius().to_bits(), r.width.to_bits()).hash(&mut h);
    if let creabim_core::road::Levels::Ground { steepest, ease } = r.levels {
        (steepest.to_bits(), ease.to_bits()).hash(&mut h);
    }
    creabim_core::members::elevation(model, s.level)
        .to_bits()
        .hash(&mut h);
    (r.start_at.map(f64::to_bits), r.end_at.map(f64::to_bits)).hash(&mut h);
    grounds.hash(&mut h);
    h.finish().max(1)
}

/// A road's levels following the ground: the ground as it was every
/// [`creabim_core::road::PROFILE_STEP`] along its middle, smoothed over its
/// easing length and held to its steepest grade; where no ground lies
/// under it, its points' own levels. Also each of its points' level on
/// that profile. None when it doesn't follow the ground.
pub fn fit_road(model: &Model, s: &Site) -> Option<(Vec<f64>, Vec<f64>)> {
    let SiteForm::Road(r) = &s.form else {
        return None;
    };
    let creabim_core::road::Levels::Ground { steepest, ease } = r.levels else {
        return None;
    };
    let terrains: Vec<(Arc<Ground>, f64, Vec<DVec2>)> = surveyed(model)
        .into_iter()
        .map(|(g, tb, b, _)| (g, tb, b))
        .collect();
    fit_profile(model, s, r, &terrains, steepest, ease)
}

fn fit_profile(
    model: &Model,
    s: &Site,
    r: &creabim_core::road::Road,
    terrains: &[(Arc<Ground>, f64, Vec<DVec2>)],
    steepest: f64,
    ease: f64,
) -> Option<(Vec<f64>, Vec<f64>)> {
    use creabim_core::road::PROFILE_STEP;
    let zero = creabim_core::members::elevation(model, s.level);
    // its middle as by its points: where it runs, and its levels there
    let mut plain = r.clone();
    plain.profile.clear();
    let line = plain.line();
    let mut gone = vec![0.0];
    for w in line.windows(2) {
        gone.push(gone.last().unwrap() + w[0].truncate().distance(w[1].truncate()));
    }
    let total = *gone.last()?;
    if total < 1e-6 {
        return None;
    }
    let at = |t: f64| -> DVec3 {
        let i = gone.partition_point(|g| *g <= t).clamp(1, line.len() - 1);
        let (a, b) = (line[i - 1], line[i]);
        let len = (gone[i] - gone[i - 1]).max(1e-12);
        a.lerp(b, ((t - gone[i - 1]) / len).clamp(0.0, 1.0))
    };
    let n = (total / PROFILE_STEP).ceil() as usize + 1;
    let t_of = |k: usize| (k as f64 * PROFILE_STEP).min(total);
    let mut z: Vec<f64> = (0..n)
        .map(|k| {
            let p = at(t_of(k));
            ground_was(model, terrains, p.truncate(), zero).unwrap_or(p.z)
        })
        .collect();
    // eased: averaged over the easing length, twice (a gentle curve)
    let smooth = |z: &mut Vec<f64>, over: f64| {
        let half = ((over / PROFILE_STEP) / 2.0).round() as usize;
        if half == 0 {
            return;
        }
        let was = z.clone();
        for (k, v) in z.iter_mut().enumerate() {
            let (lo, hi) = (k.saturating_sub(half), (k + half).min(was.len() - 1));
            *v = was[lo..=hi].iter().sum::<f64>() / (hi - lo + 1) as f64;
        }
    };
    smooth(&mut z, ease);
    smooth(&mut z, ease);
    // an end held at a level: the road eases from it to the ground over
    // its easing length (or more, as its steepest grade needs)
    let pins = [(0usize, r.start_at), (n - 1, r.end_at)];
    for (k, at) in pins {
        let Some(at) = at else {
            continue;
        };
        let reach = (ease.max(PROFILE_STEP) / PROFILE_STEP).ceil() as usize;
        for (j, v) in z.iter_mut().enumerate() {
            let d = k.abs_diff(j);
            if d <= reach {
                let w = 1.0 - d as f64 / reach as f64;
                // a gentle start: smoothstep
                let w = w * w * (3.0 - 2.0 * w);
                *v = *v * (1.0 - w) + at * w;
            }
        }
    }
    let pin = |z: &mut Vec<f64>| {
        for (k, at) in pins {
            if let Some(at) = at {
                z[k] = at;
            }
        }
    };
    pin(&mut z);
    // held to its steepest grade, both ways until it settles
    if steepest > 0.0 {
        for _ in 0..40 {
            let mut moved = false;
            for k in 1..n {
                let most = steepest * (t_of(k) - t_of(k - 1));
                let v = z[k].clamp(z[k - 1] - most, z[k - 1] + most);
                moved |= (v - z[k]).abs() > 1e-9;
                z[k] = v;
            }
            for k in (0..n - 1).rev() {
                let most = steepest * (t_of(k + 1) - t_of(k));
                let v = z[k].clamp(z[k + 1] - most, z[k + 1] + most);
                moved |= (v - z[k]).abs() > 1e-9;
                z[k] = v;
            }
            if !moved {
                break;
            }
        }
        // held ends stay held; between two held ends too steep for the
        // grade, the grade gives (the panel says how steep it is)
        let held = z.clone();
        pin(&mut z);
        let _ = held;
        // the kinks the holding leaves, eased a little (an average of a
        // line no steeper than this is no steeper either)
        smooth(&mut z, ease / 3.0);
        pin(&mut z);
    }
    // each point's level: where it is nearest the middle
    let points = r
        .path
        .iter()
        .map(|c| {
            let k = (0..n)
                .min_by(|a, b| {
                    at(t_of(*a))
                        .truncate()
                        .distance(c.truncate())
                        .total_cmp(&at(t_of(*b)).truncate().distance(c.truncate()))
                })
                .unwrap_or(0);
            z[k]
        })
        .collect();
    Some((z, points))
}

/// After a change: each road following the ground whose road or ground
/// changed takes its levels again (see [`creabim_core::document::SITE_SETTLE`]).
pub fn settle_roads(model: &Model, _tx: &creabim_core::Transaction) -> creabim_core::Transaction {
    let mut out = creabim_core::Transaction::new("Road levels");
    let following: Vec<&creabim_core::Element> = model
        .iter()
        .filter(|e| {
            matches!(&e.kind, ElementKind::Site(s)
                if matches!(&s.form, SiteForm::Road(r) if !r.levels.is_points() || !r.profile.is_empty()))
        })
        .collect();
    if following.is_empty() {
        return out;
    }
    let all = surveyed(model);
    let keys: Vec<u64> = all.iter().map(|t| t.3).collect();
    let terrains: Vec<(Arc<Ground>, f64, Vec<DVec2>)> =
        all.into_iter().map(|(g, tb, b, _)| (g, tb, b)).collect();
    for e in following {
        let ElementKind::Site(s) = &e.kind else {
            continue;
        };
        let SiteForm::Road(r) = &s.form else {
            continue;
        };
        let mut after = e.clone();
        let ElementKind::Site(sa) = &mut after.kind else {
            continue;
        };
        let SiteForm::Road(ra) = &mut sa.form else {
            continue;
        };
        match r.levels {
            // back to its points: its profile goes
            creabim_core::road::Levels::Points => {
                ra.profile.clear();
                ra.profile_key = 0;
            }
            creabim_core::road::Levels::Ground { steepest, ease } => {
                let key = fit_key(model, s, r, &keys);
                if key == r.profile_key {
                    continue;
                }
                ra.profile_key = key;
                match fit_profile(model, s, r, &terrains, steepest, ease) {
                    Some((z, points)) => {
                        ra.profile = z;
                        for (c, z) in ra.path.iter_mut().zip(points) {
                            c.z = z;
                        }
                    }
                    None => ra.profile.clear(),
                }
            }
        }
        if after != *e {
            out = out.replace(e.clone(), after);
        }
    }
    out
}

/// The roads that lie on a terrain, each as a sloping pad at its
/// surface along its outer edges, its floor sunk under its make-up.
pub fn road_pads(model: &Model, site: &Site) -> Vec<Pad> {
    road_works(model, site).into_iter().map(|w| w.1).collect()
}

/// [`road_pads`], each with its road.
fn road_works(model: &Model, site: &Site) -> Vec<(creabim_core::ElementId, Pad)> {
    let SiteForm::Terrain { boundary, .. } = &site.form else {
        return vec![];
    };
    let base = creabim_core::members::elevation(model, site.level);
    let mut out = vec![];
    for e in model.iter() {
        let ElementKind::Site(s) = &e.kind else {
            continue;
        };
        // a car park: the ground worked to its underside, so it lies in
        // its own bed and the ground never shows through it (owner)
        if let SiteForm::Area {
            boundary: outline,
            top,
            thickness,
            parking: Some(cp),
            ..
        } = &s.form
        {
            if cp.ground
                && outline
                    .iter()
                    .any(|p| creabim_core::polygon::contains(boundary, *p))
            {
                let lift = creabim_core::members::elevation(model, s.level) - base;
                let mut pad = Pad::new(outline.clone(), top + lift);
                pad.cut = 1.0;
                pad.fill = 2.0;
                // level all round, its floor sunk under its make-up as a
                // road is
                pad.heights = vec![top + lift; outline.len()];
                pad.sunk = *thickness;
                out.push((e.id, pad));
            }
            continue;
        }
        let Some(r) = s.as_road() else {
            continue;
        };
        if !r.works || r.path.len() < 2 {
            continue;
        }
        let (outline, heights) = r.outline();
        if !outline
            .iter()
            .any(|p| creabim_core::polygon::contains(boundary, *p))
        {
            continue;
        }
        let lift = creabim_core::members::elevation(model, s.level) - base;
        let mut pad = Pad::new(outline, 0.0);
        pad.heights = heights.iter().map(|z| z + lift).collect();
        pad.level = pad.heights.iter().sum::<f64>() / pad.heights.len() as f64;
        pad.cut = r.cut;
        pad.fill = r.fill;
        pad.sunk = r.kerb + r.thickness + r.width / 2.0 * r.fall / 100.0;
        // its held sides: the outline's edges along those sides
        if !r.held.is_none() {
            let (l, rt) = r.outer();
            let on = |line: &[DVec3], p: DVec2| {
                line.windows(2).any(|w| {
                    let (a, b) = (w[0].truncate(), w[1].truncate());
                    let d = b - a;
                    let t = ((p - a).dot(d) / d.length_squared().max(1e-12)).clamp(0.0, 1.0);
                    (a + d * t).distance(p) < 1e-6
                })
            };
            let n = pad.outline.len();
            pad.upright = (0..n)
                .filter(|i| {
                    let (a, b) = (pad.outline[*i], pad.outline[(i + 1) % n]);
                    let m = (a + b) / 2.0;
                    (r.held.left() && on(&l, a) && on(&l, b) && on(&l, m))
                        || (r.held.right() && on(&rt, a) && on(&rt, b) && on(&rt, m))
                })
                .collect();
        }
        out.push((e.id, pad));
    }
    out
}

/// The works whose platforms `id`'s banks run onto, on the ground as it
/// was: where they do, the bank stops at that work's edge and the ground
/// steps there, so the panel says so (owner, cut test: spill-overs).
pub fn runs_onto(model: &Model, id: creabim_core::ElementId) -> Vec<creabim_core::ElementId> {
    let mut found: Vec<creabim_core::ElementId> = vec![];
    for e in model.iter() {
        let ElementKind::Site(t) = &e.kind else {
            continue;
        };
        if !matches!(t.form, SiteForm::Terrain { .. }) {
            continue;
        }
        let mut all = work_pads(model, t);
        all.extend(road_works(model, t));
        let Some((_, pad)) = all.iter().find(|w| w.0 == id) else {
            continue;
        };
        let whole = creabim_core::site::whole_ground(model, t);
        let Some(g) = crate::ground::of(whole.as_ref().unwrap_or(t)) else {
            continue;
        };
        let perimeter: f64 = (0..pad.outline.len())
            .map(|i| pad.outline[i].distance(pad.outline[(i + 1) % pad.outline.len()]))
            .sum();
        let step = (perimeter / 240.0).max(0.5);
        let mut bank: Bank = vec![];
        for (q, out, held, stretch) in corners_mitred(pad, around_edges(&pad.outline, step)) {
            bank_sample_free(&g, pad, q, out, held, stretch, &mut bank);
        }
        for (other, o) in all.iter().filter(|w| w.0 != id) {
            if found.contains(other) {
                continue;
            }
            let outline = [o.outline.as_slice()];
            if bank
                .iter()
                .any(|(a, d)| first_entry(a.truncate(), d.truncate(), &outline).is_some())
            {
                found.push(*other);
            }
        }
    }
    found
}

/// The earthworks (pads, pits and trenches) that lie on a terrain, with
/// the element each is: its level made the terrain's, so one drawn on
/// another level is worked in where it is.
pub fn work_pads(model: &Model, site: &Site) -> Vec<(creabim_core::ElementId, Pad)> {
    let SiteForm::Terrain { boundary, .. } = &site.form else {
        return vec![];
    };
    let base = creabim_core::members::elevation(model, site.level);
    let mut out = vec![];
    for e in model.iter() {
        let ElementKind::Site(s) = &e.kind else {
            continue;
        };
        let Some(mut pad) = s.as_work() else {
            continue;
        };
        if !pad
            .outline
            .iter()
            .any(|p| creabim_core::polygon::contains(boundary, *p))
        {
            continue;
        }
        pad.level += creabim_core::members::elevation(model, s.level) - base;
        // behind a retaining wall the ground is cut at the wall's back:
        // the outline is pushed out there by the wall's thickness, as it
        // is now
        if let SiteForm::Pad { walls, .. } = &s.form {
            let behind: Vec<f64> = pad
                .upright
                .iter()
                .map(|edge| {
                    walls
                        .iter()
                        .find(|w| w.0 == *edge)
                        .and_then(|w| match model.get(w.1).map(|e| &e.kind) {
                            Some(ElementKind::Wall(wall)) => {
                                Some((wall.thickness - HELD_INSIDE).max(0.0))
                            }
                            _ => None,
                        })
                        .unwrap_or(0.0)
                })
                .collect();
            if behind.iter().any(|b| *b > 1e-6) {
                pad.outline = pushed_out(&pad.outline, &pad.upright, &behind);
                pad.behind = behind;
            }
        }
        out.push((e.id, pad));
    }
    out
}

/// `outline` with each edge in `edges` moved out by its distance in
/// `by` (beside it), its neighbours running on to meet it.
pub fn pushed_out(outline: &[DVec2], edges: &[usize], by: &[f64]) -> Vec<DVec2> {
    let n = outline.len();
    if n < 3 {
        return outline.to_vec();
    }
    let ccw = creabim_core::polygon::signed_area2(outline) >= 0.0;
    // each edge as a point on its (moved) line and its direction
    let lines: Vec<(DVec2, DVec2)> = (0..n)
        .map(|i| {
            let (a, b) = (outline[i], outline[(i + 1) % n]);
            let d = (b - a).normalize_or_zero();
            let out = if ccw {
                DVec2::new(d.y, -d.x)
            } else {
                DVec2::new(-d.y, d.x)
            };
            let off = edges
                .iter()
                .position(|e| *e == i)
                .and_then(|k| by.get(k))
                .copied()
                .unwrap_or(0.0);
            (a + out * off, d)
        })
        .collect();
    (0..n)
        .map(|i| {
            let (p, d) = lines[(i + n - 1) % n];
            let (q, e) = lines[i];
            let cross = d.perp_dot(e);
            if cross.abs() < 1e-9 {
                q
            } else {
                p + d * ((q - p).perp_dot(e) / cross)
            }
        })
        .collect()
}

/// Everything worked into a terrain, in order: the pads earlier files
/// kept in it, the earthworks lying on it, its roads, then the
/// building's holes if it is set to dig them.
pub fn works(model: &Model, site: &Site) -> Vec<Pad> {
    let SiteForm::Terrain { data, .. } = &site.form else {
        return vec![];
    };
    let mut pads = data.pads.clone();
    pads.extend(work_pads(model, site).into_iter().map(|w| w.1));
    pads.extend(road_pads(model, site));
    if data.under_buildings {
        pads.extend(building_pads(model, site));
    }
    pads
}

/// Points along a closed outline no further apart than `step`, each with
/// the way out from the outline there. A corner is given once for each
/// edge it ends, so a bank turns a corner as a fan.
fn around(outline: &[DVec2], step: f64) -> Vec<(DVec2, DVec2)> {
    around_edges(outline, step)
        .into_iter()
        .map(|(q, out, _)| (q, out))
        .collect()
}

/// As [`around`], each point with the edge of the outline it is on.
fn around_edges(outline: &[DVec2], step: f64) -> Vec<(DVec2, DVec2, usize)> {
    let ccw = creabim_core::polygon::signed_area2(outline) >= 0.0;
    let n = outline.len();
    let mut out = vec![];
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let d = b - a;
        let len = d.length();
        if len < 1e-6 {
            continue;
        }
        let along = d / len;
        let normal = if ccw {
            DVec2::new(along.y, -along.x)
        } else {
            DVec2::new(-along.y, along.x)
        };
        let k = (len / step).ceil().max(1.0) as usize;
        for j in 0..=k {
            out.push((a + d * (j as f64 / k as f64), normal, i));
        }
    }
    out
}

/// Where the bank from `q` (on a pad at `level`) going `out` meets the
/// ground `g`: how far out, and the ground's height there. `run` is the
/// bank's run for each metre of height.
fn daylight(g: &Ground, q: DVec2, out: DVec2, level: f64, run: f64, up: bool) -> (f64, f64) {
    daylight_from(g, q, out, level, run, up, UPRIGHT)
}

/// How far short of another work's edge a bank stops: clear of the
/// 5 cm round a bank where the ground's data is taken out, or that
/// work's edge would go with it.
const STEP_SHORT: f64 = 0.1;

/// The least height a bank is made for, in metres.
const BANK_LEAST: f64 = 0.03;

/// A face held by a retaining wall is this far out at its top for each
/// metre of height: near enough upright to stay behind the wall.
const HELD: f64 = 0.004;
/// The ground is cut this far inside a retaining wall's back, so the
/// face's lean stays within the wall and no gap shows behind it.
const HELD_INSIDE: f64 = 0.012;

/// [`daylight`], its bank never steeper than a run of `least`.
fn daylight_from(
    g: &Ground,
    q: DVec2,
    out: DVec2,
    level: f64,
    run: f64,
    up: bool,
    least: f64,
) -> (f64, f64) {
    let run = run.max(least);
    let bank = |t: f64| {
        if up {
            level + t / run
        } else {
            level - t / run
        }
    };
    // how far the ground is above the bank (dug) or below it (made up)
    let gap = |t: f64| {
        g.height(q + out * t)
            .map(|e| if up { e - bank(t) } else { bank(t) - e })
    };
    let step = (run * 0.1).clamp(if least < UPRIGHT { 0.001 } else { 0.005 }, 0.25);
    let (mut t0, mut t1) = (0.0, step);
    loop {
        match gap(t1) {
            // the bank ran off the terrain: it stops at the edge
            None => return (t0, bank(t0)),
            Some(d) if d <= 0.0 => break,
            Some(_) if t1 > REACH => return (t1, bank(t1)),
            Some(_) => {
                t0 = t1;
                t1 += step;
            }
        }
    }
    for _ in 0..24 {
        let m = (t0 + t1) / 2.0;
        match gap(m) {
            Some(d) if d > 0.0 => t0 = m,
            _ => t1 = m,
        }
    }
    (t1, bank(t1))
}

/// Where segments `a0`-`a1` and `b0`-`b1` cross, as how far along the
/// first; none if they don't, or only touch at their ends.
fn crossing(a0: DVec2, a1: DVec2, b0: DVec2, b1: DVec2) -> Option<f64> {
    let (r, q) = (a1 - a0, b1 - b0);
    let den = r.perp_dot(q);
    if den.abs() < 1e-14 {
        return None;
    }
    let t = (b0 - a0).perp_dot(q) / den;
    let u = (b0 - a0).perp_dot(r) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(t)
}

/// In an inside corner the banks of the two edges run into each other:
/// their daylight lines cross and loop. Each loop is closed up into the
/// point where the lines cross, the valley the two banks meet in.
fn untangle(bank: &mut Bank) {
    let n = bank.len();
    if n < 4 {
        return;
    }
    let reach = (n / 2).min(80);
    let mut i = 0;
    while i < n {
        let (a0, a1) = (bank[i].1, bank[(i + 1) % n].1);
        // the furthest line ahead that this one crosses
        let hit = (2..reach).rev().find_map(|k| {
            let j = (i + k) % n;
            let (b0, b1) = (bank[j].1, bank[(j + 1) % n].1);
            if a0.truncate().distance(a1.truncate()) < 1e-9
                || b0.truncate().distance(b1.truncate()) < 1e-9
            {
                return None;
            }
            crossing(a0.truncate(), a1.truncate(), b0.truncate(), b1.truncate()).map(|t| (k, t))
        });
        match hit {
            Some((k, t)) => {
                let x = a0.lerp(a1, t);
                for m in 1..=k {
                    bank[(i + m) % n].1 = x;
                }
                i += k;
            }
            None => i += 1,
        }
    }
}

/// A pad's edge samples with each convex corner mitred, as banks meet in
/// grading: two sloping edges in one hip along the corner's bisector; a
/// sloping edge against an upright one, its bank carried on along the
/// upright face's line, so the face goes on past the corner as one plane.
/// Each sample: where, which way out, held upright, and how much longer
/// the bank's run is that way (Site polish 6: at a corner the two edges'
/// own samples fanned out and the daylight line zigzagged between them).
fn corners_mitred(
    pad: &Pad,
    samples: Vec<(DVec2, DVec2, usize)>,
) -> Vec<(DVec2, DVec2, bool, f64)> {
    let n = samples.len();
    let held = |e: usize| pad.upright.contains(&e);
    let mut out = Vec::with_capacity(n);
    let mut k = 0;
    while k < n {
        let (q, na, ea) = samples[k];
        let next = samples[(k + 1) % n];
        let corner = next.2 != ea && next.0.distance(q) < 1e-9;
        if !corner {
            out.push((q, na, held(ea), 1.0));
            k += 1;
            continue;
        }
        let (_, nb, eb) = next;
        // turning out (convex): the two outward normals part
        // (anticlockwise, the outward normal turns anticlockwise there)
        let turn = na.perp_dot(nb)
            * if creabim_core::polygon::signed_area2(&pad.outline) >= 0.0 {
                1.0
            } else {
                -1.0
            };
        let convex = turn > 1e-9;
        let (ha, hb) = (held(ea), held(eb));
        if ha && hb {
            // two upright faces meet in one upright corner: one sample
            // along the bisector, not one for each face a hair apart,
            // which left a stepped corner and slivers of ground fanning
            // out from it (owner, cut test E and the trench's bends)
            out.push((q, (na + nb).normalize_or(na), true, 1.0));
        } else if !convex {
            out.push((q, na, ha, 1.0));
            out.push((q, nb, hb, 1.0));
        } else if !ha && !hb {
            // a hip: one sample along the bisector, its run longer by
            // as much as the bisector leans off each edge's normal
            let m = (na + nb).normalize_or(na);
            let c = m.dot(na).max(0.2);
            out.push((q, m, false, 1.0 / c));
        } else {
            // the upright edge's corner, then the sloping bank carried on
            // along the upright edge's line
            let (n_up, n_slope, up_first) = if ha { (na, nb, true) } else { (nb, na, false) };
            // the upright edge's line, pointing away from the pad past the corner
            let along = if up_first {
                DVec2::new(n_up.y, -n_up.x)
            } else {
                DVec2::new(-n_up.y, n_up.x)
            };
            let along = if along.dot(n_slope) < 0.0 {
                -along
            } else {
                along
            };
            let c = along.dot(n_slope).max(0.2);
            // the upright edge's corner, then the bank carried on along
            // its line (marked by a stretch of at least a hair over one,
            // so it folds as a hip does)
            let carried = (q, along, false, (1.0 / c).max(1.0 + 1e-6));
            if up_first {
                out.push((q, n_up, true, 1.0));
                out.push(carried);
            } else {
                out.push(carried);
                out.push((q, n_up, true, 1.0));
            }
        }
        k += 2;
    }
    out
}

/// One sample of a pad's bank: at `q`, going `out`; `held` upright, or a
/// bank whose run is `stretch` times the pad's.
#[allow(clippy::too_many_arguments)]
fn bank_sample(
    g: &Ground,
    pad: &Pad,
    q: DVec2,
    out: DVec2,
    held: bool,
    stretch: f64,
    bank: &mut Bank,
    earlier: &[&[DVec2]],
) {
    let before = bank.len();
    bank_sample_free(g, pad, q, out, held, stretch, bank);
    // a bank stops where it would run onto a work laid before it: that
    // work's platform stays as it is, and the ground steps at its edge
    // (owner, cut test: a road's bank buried part of pad C)
    if bank.len() > before {
        let (a, d) = bank[bank.len() - 1];
        if let Some(f) = first_entry(a.truncate(), d.truncate(), earlier) {
            // a little short of that edge, so the step is the ground's
            // own and not two heights on one spot
            let len = a.truncate().distance(d.truncate()).max(1e-9);
            let f = (f - STEP_SHORT / len).max(0.0);
            bank[before] = (a, a.lerp(d, f));
        }
    }
}

/// Where the line from `a` to `b` first goes into one of `outlines`
/// that `a` is not inside, as a fraction of the way; none if it doesn't.
fn first_entry(a: DVec2, b: DVec2, outlines: &[&[DVec2]]) -> Option<f64> {
    let d = b - a;
    if d.length_squared() < 1e-12 {
        return None;
    }
    let mut best: Option<f64> = None;
    for o in outlines {
        if o.len() < 3 || creabim_core::polygon::contains(o, a) {
            continue;
        }
        for i in 0..o.len() {
            let (p, q) = (o[i], o[(i + 1) % o.len()]);
            let e = q - p;
            let den = d.perp_dot(e);
            if den.abs() < 1e-12 {
                continue;
            }
            let t = (p - a).perp_dot(e) / den;
            let u = (p - a).perp_dot(d) / den;
            if t > 1e-6
                && t < 1.0
                && (-1e-9..=1.0 + 1e-9).contains(&u)
                && best.is_none_or(|b| t < b)
            {
                best = Some(t);
            }
        }
    }
    best
}

/// One bank sample on its own, as if nothing else were worked in.
fn bank_sample_free(
    g: &Ground,
    pad: &Pad,
    q: DVec2,
    out: DVec2,
    held: bool,
    stretch: f64,
    bank: &mut Bank,
) {
    // the part of a pad that hangs over the terrain's edge is left off
    let Some(e) = g.height(q) else {
        return;
    };
    if pad.depth > 0.0 {
        // an excavation: its floor follows the ground, so far below it
        let floor = e - pad.depth;
        let (t, z) = if held {
            daylight_from(g, q, out, floor, 0.0, true, HELD)
        } else {
            daylight(g, q, out, floor, pad.cut * stretch, true)
        };
        bank.push((q.extend(floor), (q + out * t).extend(z)));
        return;
    }
    let level = pad.height_at(q);
    if pad.only_cut && e <= level + 1e-6 {
        bank.push((q.extend(e), q.extend(e)));
        return;
    }
    // a bank only a few centimetres high is a sliver turned any which
    // way; the pad's edge meets the ground there (owner, cut test: they
    // fanned out where a pad passes from dug to made up)
    if (e - level).abs() < BANK_LEAST {
        bank.push((q.extend(level), q.extend(level)));
        return;
    }
    let up = e > level;
    let (t, z) = if held {
        daylight_from(g, q, out, level, 0.0, up, HELD)
    } else {
        daylight(
            g,
            q,
            out,
            level,
            if up { pad.cut } else { pad.fill } * stretch,
            up,
        )
    };
    bank.push((q.extend(level), (q + out * t).extend(z)));
}

/// One pad worked into `site`, whose ground is `g`: the site with the
/// pad's edge and daylight line as break lines and the survey's data
/// inside them taken out, and the pad's bank.
fn work_in(site: &Site, g: &Ground, pad: &Pad, earlier: &[&[DVec2]]) -> Option<(Site, Bank)> {
    let SiteForm::Terrain { boundary, .. } = &site.form else {
        return None;
    };
    let perimeter: f64 = (0..pad.outline.len())
        .map(|i| pad.outline[i].distance(pad.outline[(i + 1) % pad.outline.len()]))
        .sum();
    let step = (perimeter / 240.0).max(0.5);
    let mut bank: Bank = vec![];
    // the hips, where two sloping banks meet along a convex corner's bisector
    let mut hips: Vec<(DVec2, DVec2)> = vec![];
    // and the line each hip folds along, from the floor to the ground: a
    // break line, so the ground is not smoothed round it into steps
    let mut hip_lines: Vec<(DVec3, DVec3)> = vec![];
    for (q, out, held, stretch) in corners_mitred(pad, around_edges(&pad.outline, step)) {
        let before = bank.len();
        bank_sample(g, pad, q, out, held, stretch, &mut bank, earlier);
        if bank.len() > before && stretch > 1.0 + 1e-9 && !held {
            hips.push((q, out));
            hip_lines.push(bank[bank.len() - 1]);
        }
    }
    // a sample whose bank runs on past a hip lies under the other bank
    // there: it goes, and the two banks meet in one clean crease along
    // the hip (Site polish 6: kept, the daylight line zigzagged)
    if !hips.is_empty() {
        bank.retain(|(a, d)| {
            !hips.iter().any(|(q, m)| {
                let (a2, d2) = (a.truncate(), d.truncate());
                if a2.distance(*q) < 1e-9 {
                    return false;
                }
                let side = |p: DVec2| m.perp_dot(p - *q);
                let (sa, sd) = (side(a2), side(d2));
                // on opposite sides of the hip's line, and the crossing
                // ahead of the corner, not behind it
                sa * sd < 0.0 && {
                    let t = sa / (sa - sd);
                    (a2 + (d2 - a2) * t - *q).dot(*m) > 0.0
                }
            })
        });
    }
    if bank.len() < 3 {
        return None;
    }
    untangle(&mut bank);
    // nothing to do where the ground is already as the pad wants it
    let moved = bank.iter().any(|(a, b)| a.distance(*b) > 1e-4);
    let inside_pad = |p: DVec2| creabim_core::polygon::contains(&pad.outline, p);
    let level_inside = pad.depth > 0.0 || !pad.only_cut || {
        let c = pad.outline.iter().sum::<DVec2>() / pad.outline.len() as f64;
        g.height(c).is_some_and(|e| e > pad.level + 1e-6)
    };
    if !moved && !level_inside {
        return None;
    }
    let ring: Vec<DVec2> = bank.iter().map(|(_, d)| d.truncate()).collect();
    // what of the survey the works replace: everything on the pad and on
    // its banks (a hole only dug keeps the ground already below it)
    let gone = |p: DVec3| {
        let q = p.truncate();
        if inside_pad(q) {
            return !(pad.only_cut && p.z <= pad.level);
        }
        creabim_core::polygon::contains(&ring, q)
            || bank.iter().any(|(a, d)| {
                let (a, d) = (a.truncate(), d.truncate());
                let ad = d - a;
                let l2 = ad.length_squared();
                l2 > 1e-8 && {
                    let t = ((q - a).dot(ad) / l2).clamp(0.0, 1.0);
                    (a + ad * t).distance(q) < 0.05
                }
            })
    };
    let mut next = site.clone();
    let SiteForm::Terrain { points, data, .. } = &mut next.form else {
        return None;
    };
    points.retain(|p| !gone(*p));
    let mut lines = vec![
        GroundLine {
            kind: GroundLineKind::Break,
            points: bank.iter().map(|(a, _)| *a).collect(),
            closed: true,
        },
        GroundLine {
            kind: GroundLineKind::Break,
            points: bank.iter().map(|(_, d)| *d).collect(),
            closed: true,
        },
    ];
    for (a, d) in &hip_lines {
        if a.distance(*d) > 1e-4 {
            lines.push(GroundLine {
                kind: GroundLineKind::Break,
                points: vec![*a, *d],
                closed: false,
            });
        }
    }
    // the survey's lines stop at the works and go on beyond them
    for l in std::mem::take(&mut data.lines) {
        if !l.points.iter().any(|p| gone(*p)) {
            lines.push(l);
            continue;
        }
        let mut run = vec![];
        for p in l.points.iter().chain(l.closed.then_some(&l.points[0])) {
            if gone(*p) {
                if run.len() > 1 {
                    lines.push(GroundLine {
                        kind: l.kind,
                        points: std::mem::take(&mut run),
                        closed: false,
                    });
                }
                run.clear();
            } else {
                run.push(*p);
            }
        }
        if run.len() > 1 {
            lines.push(GroundLine {
                kind: l.kind,
                points: run,
                closed: false,
            });
        }
    }
    // a sloping pad's floor, sunk just inside its edge
    if pad.sloping() && pad.sunk > 0.0 {
        let floor: Vec<DVec3> = inset(&pad.outline, SUNK_INSET)
            .into_iter()
            .map(|q| q.extend(pad.height_at(q) - pad.sunk))
            .collect();
        if floor.len() >= 3 {
            lines.push(GroundLine {
                kind: GroundLineKind::Break,
                points: floor,
                closed: true,
            });
        }
    }
    data.lines = lines;
    // a pad's own floor, where the survey gave it no point to stand on
    if level_inside && !pad.only_cut && pad.depth <= 0.0 && !pad.sloping() {
        let c = pad.outline.iter().sum::<DVec2>() / pad.outline.len() as f64;
        if inside_pad(c) && creabim_core::polygon::contains(boundary, c) {
            points.push(c.extend(pad.level));
        }
    }
    // a hole only dug: its floor where the ground was above it, the
    // ground where it was below, sampled so the line between them holds
    if pad.only_cut || pad.depth > 0.0 {
        let (lo, hi) = pad.outline.iter().fold(
            (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
            |(lo, hi), p| (lo.min(*p), hi.max(*p)),
        );
        let size = hi - lo;
        let cell = (size.x.max(size.y) / 24.0).max(0.25);
        let (nx, ny) = (
            (size.x / cell).ceil() as usize,
            (size.y / cell).ceil() as usize,
        );
        for j in 1..ny {
            for i in 1..nx {
                let q =
                    lo + DVec2::new(size.x * i as f64 / nx as f64, size.y * j as f64 / ny as f64);
                if !inside_pad(q) {
                    continue;
                }
                if let Some(e) = g.height(q) {
                    points.push(q.extend(if pad.depth > 0.0 {
                        e - pad.depth
                    } else {
                        e.min(pad.level)
                    }));
                }
            }
        }
    }
    // beyond its banks the ground is to stay as it was. Where the survey
    // is sparse (a few spot levels), the works' new edges would pull long
    // flat facets out to its far points, and the ground round a pad
    // looked bent. A ring of the ground's own heights a little way out
    // from the daylight line holds it there
    if moved && points.len() < GUARD_ONLY_UNDER {
        let n = ring.len();
        let ccw = creabim_core::polygon::signed_area2(&ring) >= 0.0;
        let mut last: Option<DVec2> = None;
        for i in 0..n {
            let along = ring[(i + 1) % n] - ring[(i + n - 1) % n];
            let mut out = DVec2::new(along.y, -along.x).normalize_or_zero();
            if out == DVec2::ZERO {
                continue;
            }
            if !ccw {
                out = -out;
            }
            let q = ring[i] + out * GUARD_OUT;
            if last.is_some_and(|l| l.distance(q) < GUARD_STEP)
                || !creabim_core::polygon::contains(boundary, q)
                || creabim_core::polygon::contains(&ring, q)
                || inside_pad(q)
                || points
                    .iter()
                    .any(|s| s.truncate().distance(q) < GUARD_STEP / 2.0)
            {
                continue;
            }
            if let Some(z) = g.height(q) {
                points.push(q.extend(z));
                last = Some(q);
            }
        }
    }
    Some((next, bank))
}

/// How far out from a work's daylight line the ground is held as it was,
/// how far apart the points that hold it, and the size of survey above
/// which it needs no holding: its own points are near enough.
const GUARD_OUT: f64 = 2.0;
const GUARD_STEP: f64 = 2.0;
const GUARD_ONLY_UNDER: usize = 2000;

/// The volumes between two grounds over the box `lo`..`hi`: dug, made
/// up, and the area changed.
fn volumes(
    existing: &Ground,
    proposed: &Ground,
    banks: &[Bank],
    strata: &[f64],
    lo: DVec2,
    hi: DVec2,
) -> (f64, f64, f64, Vec<f64>) {
    // only what the works cover is counted: beyond them the two grounds
    // are the same ground
    let rings: Vec<(Vec<DVec2>, Vec<DVec2>)> = banks
        .iter()
        .map(|b| {
            (
                b.iter().map(|(a, _)| a.truncate()).collect(),
                b.iter().map(|(_, d)| d.truncate()).collect(),
            )
        })
        .collect();
    let worked = |p: DVec2| {
        rings.iter().any(|(pad, day)| {
            creabim_core::polygon::contains(pad, p) || creabim_core::polygon::contains(day, p)
        })
    };
    let size = (hi - lo).max(DVec2::splat(1e-6));
    let cell = (size.x * size.y / 60_000.0).sqrt().clamp(0.02, 0.5);
    let (nx, ny) = (
        (size.x / cell).ceil() as usize,
        (size.y / cell).ceil() as usize,
    );
    let (dx, dy) = (size.x / nx as f64, size.y / ny as f64);
    let (mut cut, mut fill, mut area) = (0.0, 0.0, 0.0);
    let mut layers = vec![0.0; strata.len() + 1];
    for j in 0..ny {
        for i in 0..nx {
            let p = lo + DVec2::new((i as f64 + 0.5) * dx, (j as f64 + 0.5) * dy);
            if !worked(p) {
                continue;
            }
            let (Some(e), Some(n)) = (existing.height(p), proposed.height(p)) else {
                continue;
            };
            let d = n - e;
            if d.abs() < 1e-3 {
                continue;
            }
            area += 1.0;
            if d < 0.0 {
                cut -= d;
                // the column dug, through the layers it passes
                let mut top = 0.0;
                for (i, th) in strata.iter().enumerate() {
                    layers[i] += ((-d).min(top + th) - top).max(0.0);
                    top += th;
                }
                layers[strata.len()] += (-d - top).max(0.0);
            } else {
                fill += d;
            }
        }
    }
    let a = dx * dy;
    layers.iter_mut().for_each(|v| *v *= a);
    (cut * a, fill * a, area * a, layers)
}

/// How far round the pads the ground is rebuilt between one pad and the
/// next, and how near its edge a bank may come.
const WINDOW: f64 = 12.0;
const WINDOW_EDGE: f64 = 3.0;

/// A rectangle round the pads that lies wholly inside a large terrain:
/// between pads only the ground in it is rebuilt, since nothing a pad
/// asks of the ground lies far from it. None for a small terrain, or
/// one whose outline comes near the pads.
fn window(site: &Site, pads: &[Pad]) -> Option<(DVec2, DVec2)> {
    let SiteForm::Terrain {
        boundary,
        points,
        data,
        ..
    } = &site.form
    else {
        return None;
    };
    let count = points.len() + data.lines.iter().map(|l| l.points.len()).sum::<usize>();
    if count < 600 || pads.len() < 2 {
        return None;
    }
    let (lo, hi) = pads.iter().flat_map(|p| p.outline.iter()).fold(
        (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
        |(lo, hi), p| (lo.min(*p), hi.max(*p)),
    );
    let (lo, hi) = (lo - DVec2::splat(WINDOW), hi + DVec2::splat(WINDOW));
    let rect = [lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)];
    let inside = rect
        .iter()
        .all(|c| creabim_core::polygon::contains(boundary, *c));
    let n = boundary.len();
    let crossed = (0..n).any(|i| {
        let (a, b) = (boundary[i], boundary[(i + 1) % n]);
        let within = |p: DVec2| p.cmpge(lo).all() && p.cmple(hi).all();
        within(a) || (0..4).any(|k| crossing(a, b, rect[k], rect[(k + 1) % 4]).is_some())
    });
    let held = points
        .iter()
        .filter(|p| p.truncate().cmpge(lo).all() && p.truncate().cmple(hi).all())
        .count();
    (inside && !crossed && held * 2 < count).then_some((lo, hi))
}

/// The terrain cut down to a window: its outline the window's, its data
/// what lies in it, and the heights of the ground as it was along the
/// window's edge, so that the ground inside is the same as the whole
/// terrain's.
fn cropped(site: &Site, existing: &Ground, (lo, hi): (DVec2, DVec2)) -> Site {
    let mut s = site.clone();
    let within = |p: DVec2| p.cmpge(lo).all() && p.cmple(hi).all();
    if let SiteForm::Terrain {
        boundary,
        points,
        data,
        ..
    } = &mut s.form
    {
        *boundary = vec![lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)];
        points.retain(|p| within(p.truncate()));
        data.lines
            .retain(|l| l.points.iter().all(|p| within(p.truncate())));
        for (q, _) in around(boundary, 1.5) {
            if let Some(z) = existing.height(q) {
                points.push(q.extend(z));
            }
        }
        data.survey = true;
    }
    s
}

/// Every pad worked into the terrain in turn: the ground the last pad
/// was worked into, the terrain's data with them all in, and their
/// banks. With a window the ground between pads is rebuilt in it alone;
/// none when a bank then comes too near the window's edge.
#[allow(clippy::type_complexity)]
fn work_all(
    existing: &Arc<Ground>,
    start: &Site,
    pads: &[Pad],
    window: Option<(DVec2, DVec2)>,
) -> Option<(Arc<Ground>, Site, Vec<Bank>)> {
    use std::hash::{Hash, Hasher};
    // the ground after each work is kept by what led to it (the terrain,
    // the window, and every work up to that one), so a change to one
    // work starts from the ground before it: the works before it are
    // not worked in again
    let mut keys = Vec::with_capacity(pads.len());
    let mut h = std::collections::hash_map::DefaultHasher::new();
    crate::ground::data_key(start)?.hash(&mut h);
    if let Some((lo, hi)) = window {
        [lo.x, lo.y, hi.x, hi.y].map(f64::to_bits).hash(&mut h);
    }
    for p in pads {
        for q in &p.outline {
            (q.x.to_bits(), q.y.to_bits()).hash(&mut h);
        }
        (
            p.outline.len(),
            p.level.to_bits(),
            p.cut.to_bits(),
            p.fill.to_bits(),
            p.depth.to_bits(),
            p.only_cut,
            p.sunk.to_bits(),
            &p.upright,
        )
            .hash(&mut h);
        for z in &p.heights {
            z.to_bits().hash(&mut h);
        }
        keys.push(h.clone().finish());
    }
    let kept = {
        let s = lock(&STEPS);
        (0..pads.len())
            .rev()
            .find_map(|i| s.iter().find(|e| e.0 == keys[i]).map(|e| (i, e.1.clone())))
    };
    let (from, mut ground, mut now, mut banks) = match kept {
        Some((i, step)) => (i + 1, step.0.clone(), (*step.1).clone(), step.2.clone()),
        None => (0, existing.clone(), start.clone(), vec![]),
    };
    let keep = |key: u64, ground: &Arc<Ground>, now: &Site, banks: &[Bank]| {
        let step = Arc::new((ground.clone(), Arc::new(now.clone()), banks.to_vec()));
        let mut s = lock(&STEPS);
        s.retain(|e| e.0 != key);
        if s.len() >= STEPS_KEPT {
            s.remove(0);
        }
        s.push((key, step));
    };
    for (i, pad) in pads.iter().enumerate().skip(from) {
        let earlier: Vec<&[DVec2]> = pads[..i].iter().map(|p| p.outline.as_slice()).collect();
        let Some((next, bank)) = work_in(&now, &ground, pad, &earlier) else {
            continue;
        };
        let built = match window {
            Some(w) => {
                let (lo, hi) = (
                    w.0 + DVec2::splat(WINDOW_EDGE),
                    w.1 - DVec2::splat(WINDOW_EDGE),
                );
                let fits = bank.iter().all(|(a, d)| {
                    [a.truncate(), d.truncate()]
                        .iter()
                        .all(|p| p.cmpge(lo).all() && p.cmple(hi).all())
                });
                if !fits {
                    return None;
                }
                Ground::build(&cropped(&next, existing, w))
            }
            None => Ground::build(&next),
        };
        let Some(g) = built else {
            continue;
        };
        now = next;
        ground = Arc::new(g);
        banks.push(bank);
        // the last is kept by `shaped` itself
        if i + 1 < pads.len() {
            keep(keys[i], &ground, &now, &banks);
        }
    }
    Some((ground, now, banks))
}

/// How many grounds part way through a terrain's works are kept.
const STEPS_KEPT: usize = 24;

type Step = Arc<(Arc<Ground>, Arc<Site>, Vec<Bank>)>;

/// The ground after each work, by what led to it (see `work_all`). These
/// caches are shared by every thread, as [`crate::ground::of`]'s is.
static STEPS: Mutex<Vec<(u64, Step)>> = Mutex::new(Vec::new());

fn build(model: &Model, site: &Site, pads: &[Pad]) -> Option<Shaped> {
    let existing = crate::ground::of(site)?;
    let mut now = site.clone();
    // the terrain's corners keep the heights they have, whatever is
    // worked in near them
    if let SiteForm::Terrain {
        boundary, points, ..
    } = &mut now.form
    {
        for c in boundary.iter() {
            if let Some(z) = existing.height(*c) {
                points.push(c.extend(z));
            }
        }
    }
    let within = window(site, pads);
    let (last, now, banks, whole) = match work_all(&existing, &now, pads, within) {
        Some((g, now, banks)) => (g, now, banks, within.is_none()),
        // a bank ran out of the window: every step on the whole ground
        None => {
            let (g, now, banks) = work_all(&existing, &now, pads, None)?;
            (g, now, banks, true)
        }
    };
    let ground = if banks.is_empty() {
        existing.clone()
    } else if whole {
        // worked in on the whole ground, the last step's ground is it:
        // it is not built a second time
        last
    } else {
        Arc::new(Ground::build(&now)?)
    };
    let _ = model;
    let strata: Vec<f64> = match &site.form {
        SiteForm::Terrain { data, .. } => data.strata.iter().map(|l| l.thickness).collect(),
        _ => vec![],
    };
    let (mut cut, mut fill, mut area) = (0.0, 0.0, 0.0);
    let mut cut_layers = vec![0.0; strata.len() + 1];
    if !banks.is_empty() {
        let pts = banks
            .iter()
            .flatten()
            .flat_map(|(a, d)| [a.truncate(), d.truncate()]);
        let (lo, hi) = pts.fold(
            (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
            |(lo, hi), p| (lo.min(p), hi.max(p)),
        );
        (cut, fill, area, cut_layers) = volumes(
            &existing,
            &ground,
            &banks,
            &strata,
            lo - DVec2::splat(0.1),
            hi + DVec2::splat(0.1),
        );
    }
    Some(Shaped {
        existing,
        ground,
        banks,
        cut,
        fill,
        area,
        cut_layers,
    })
}

/// What one earthwork does to the ground on its own.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorkVolumes {
    /// Dug out, in cubic metres.
    pub cut: f64,
    /// Made up, in cubic metres.
    pub fill: f64,
    /// The area in plan it changes, banks included, in square metres.
    pub area: f64,
}

static ALONE: Mutex<Vec<(u64, WorkVolumes)>> = Mutex::new(Vec::new());

use crate::ground::kept as lock;
use std::sync::Mutex;

/// What `work` (an earthwork element lying on `site`, a terrain) digs
/// out and makes up, measured as if it were the only work on the ground
/// as surveyed: where two works overlap, each counts the overlap, so
/// these do not add up to the terrain's totals. Kept from one drawing to
/// the next. None when it is not an earthwork on this terrain.
pub fn work_alone(
    model: &Model,
    site: &Site,
    work: creabim_core::ElementId,
) -> Option<WorkVolumes> {
    use std::hash::{Hash, Hasher};
    let pad = work_pads(model, site).into_iter().find(|w| w.0 == work)?.1;
    let whole = whole(model, site, crate::ground::data_key(site)?);
    let site = whole.as_deref().unwrap_or(site);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    crate::ground::data_key(site)?.hash(&mut h);
    for q in &pad.outline {
        (q.x.to_bits(), q.y.to_bits()).hash(&mut h);
    }
    (
        pad.level.to_bits(),
        pad.cut.to_bits(),
        pad.fill.to_bits(),
        pad.depth.to_bits(),
        pad.only_cut,
        &pad.upright,
    )
        .hash(&mut h);
    let k = h.finish();
    let hit = lock(&ALONE).iter().find(|e| e.0 == k).map(|e| e.1);
    if let Some(v) = hit {
        return Some(v);
    }
    if held() {
        let last = lock(&LAST_ALONE).iter().find(|e| e.0 == work).map(|e| e.1);
        if let Some(v) = last {
            HELD_USED.store(true, std::sync::atomic::Ordering::Relaxed);
            return Some(v);
        }
    }
    let s = build(model, site, std::slice::from_ref(&pad))?;
    let v = WorkVolumes {
        cut: s.cut,
        fill: s.fill,
        area: s.area,
    };
    {
        let mut c = lock(&ALONE);
        c.retain(|e| e.0 != k);
        if c.len() >= 64 {
            c.remove(0);
        }
        c.push((k, v));
    }
    {
        let mut l = lock(&LAST_ALONE);
        l.retain(|e| e.0 != work);
        if l.len() >= 64 {
            l.remove(0);
        }
        l.push((work, v));
    }
    Some(v)
}

static CACHE: Mutex<Vec<(u64, Arc<Shaped>)>> = Mutex::new(Vec::new());

/// A terrain with the spot levels and lines lying on it taken in, by
/// the model's revision and the terrain's own data.
#[allow(clippy::type_complexity)]
static WHOLE: Mutex<Vec<((u64, u64), Option<Arc<Site>>)>> = Mutex::new(Vec::new());

/// [`creabim_core::site::whole_ground`], kept while the model stays as
/// it is: it is asked for many times a frame, and walks every element.
fn whole(model: &Model, site: &Site, kept: u64) -> Option<Arc<Site>> {
    let key = (model.revision(), kept);
    // a model never changed has no revision of its own to tell it by
    if key.0 == 0 {
        return creabim_core::site::whole_ground(model, site).map(Arc::new);
    }
    let hit = lock(&WHOLE)
        .iter()
        .find(|e| e.0 == key)
        .map(|e| e.1.clone());
    if let Some(hit) = hit {
        return hit;
    }
    let made = creabim_core::site::whole_ground(model, site).map(Arc::new);
    let mut c = lock(&WHOLE);
    c.retain(|e| e.0 != key);
    if c.len() >= 8 {
        c.remove(0);
    }
    c.push((key, made.clone()));
    made
}

/// A terrain's grounds, existing and proposed, kept from one drawing to
/// the next while the terrain and what is worked into it stay the same.
pub fn shaped(model: &Model, site: &Site) -> Option<Arc<Shaped>> {
    use std::hash::{Hash, Hasher};
    // the terrain as it is kept names the ground held through a drag;
    // its ground is worked out with the spot levels and lines lying on it
    let kept = crate::ground::data_key(site)?;
    let whole = whole(model, site, kept);
    let site = whole.as_deref().unwrap_or(site);
    let pads = works(model, site);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    crate::ground::data_key(site)?.hash(&mut h);
    if let SiteForm::Terrain { data, .. } = &site.form {
        for l in &data.strata {
            l.thickness.to_bits().hash(&mut h);
        }
    }
    for p in &pads {
        for q in &p.outline {
            (q.x.to_bits(), q.y.to_bits()).hash(&mut h);
        }
        (
            p.outline.len(),
            p.level.to_bits(),
            p.cut.to_bits(),
            p.fill.to_bits(),
            p.depth.to_bits(),
            p.only_cut,
            p.sunk.to_bits(),
            &p.upright,
        )
            .hash(&mut h);
        for z in &p.heights {
            z.to_bits().hash(&mut h);
        }
    }
    let k = h.finish();
    let ground = kept;
    let hit = {
        let mut c = lock(&CACHE);
        c.iter().position(|e| e.0 == k).map(|i| {
            let hit = c.remove(i);
            c.push(hit.clone());
            hit.1
        })
    };
    if let Some(hit) = hit {
        return Some(hit);
    }
    // while a drag is live, the ground stays as it last was: only the
    // work's own outline moves, and the ground is worked out on the drop
    if held() {
        let last = lock(&LAST)
            .iter()
            .find(|e| e.0 == ground)
            .map(|e| e.1.clone());
        if let Some(last) = last {
            HELD_USED.store(true, std::sync::atomic::Ordering::Relaxed);
            return Some(last);
        }
    }
    let s = Arc::new(build(model, site, &pads)?);
    {
        let mut c = lock(&CACHE);
        c.retain(|e| e.0 != k);
        if c.len() >= 12 {
            c.remove(0);
        }
        c.push((k, s.clone()));
    }
    {
        let mut l = lock(&LAST);
        l.retain(|e| e.0 != ground);
        if l.len() >= 8 {
            l.remove(0);
        }
        l.push((ground, s.clone()));
    }
    Some(s)
}

/// The ground last worked out for each terrain, by its data.
static LAST: Mutex<Vec<(u64, Arc<Shaped>)>> = Mutex::new(Vec::new());
/// What each earthwork last did on its own, by its element.
static LAST_ALONE: Mutex<Vec<(creabim_core::ElementId, WorkVolumes)>> = Mutex::new(Vec::new());

static HOLD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static HELD_USED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn held() -> bool {
    HOLD.load(std::sync::atomic::Ordering::Relaxed)
}

/// While something is being dragged, the ground is not worked out again
/// at each step: [`shaped`] gives the ground as it last was, so only the
/// outline of the work being dragged moves (a pad step costs 30 ms on a
/// small survey and over half a second on a large one). The app turns
/// this on when a drag starts and off when it ends. Returns whether, on
/// turning it off, a held ground was served since it was turned on: what
/// was drawn meanwhile is then to be drawn again.
pub fn hold(on: bool) -> bool {
    use std::sync::atomic::Ordering::Relaxed;
    let was = HOLD.swap(on, Relaxed);
    if was && !on {
        return HELD_USED.swap(false, Relaxed);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::{Element, Level};

    #[test]
    fn a_ground_worked_out_on_one_thread_serves_the_next() {
        // MU142 lag: each scene and drawing is built on a thread of its
        // own, and a cache kept per thread was empty every time
        let m = creabim_core::ridgeline::ridgeline_house();
        let terrain = m
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Site(t) if matches!(t.form, SiteForm::Terrain { .. }) => {
                    Some(t.clone())
                }
                _ => None,
            })
            .expect("Ridgeline has a terrain");
        let m = Arc::new(m);
        let on_a_thread = || {
            let (m, t) = (m.clone(), terrain.clone());
            std::thread::spawn(move || shaped(&m, &t).expect("its ground"))
                .join()
                .unwrap()
        };
        let first = on_a_thread();
        let next = on_a_thread();
        assert!(
            Arc::ptr_eq(&first, &next),
            "the second thread worked the ground out again"
        );
    }

    // ---- MU140: pads and what is built in them ----

    fn named(m: &Model, name: &str) -> creabim_core::ElementId {
        m.iter()
            .find(|e| e.name == name)
            .unwrap_or_else(|| panic!("{name}"))
            .id
    }

    /// Ridgeline with its ground floor made a framed floor (the I-joist
    /// floor of its first floor): a house over a crawl space.
    fn ridgeline_framed() -> Model {
        let mut m = creabim_core::ridgeline::ridgeline_house();
        let joists = named(&m, "Upper floor, I-joist");
        let gf = named(&m, "Ground floor slab");
        let mut e = m.get(gf).unwrap().clone();
        if let ElementKind::Slab(s) = &mut e.kind {
            s.assembly = Some(joists);
        }
        creabim_core::Transaction::new("framed")
            .replace(m.get(gf).unwrap().clone(), e)
            .apply(&mut m)
            .unwrap();
        m
    }

    fn ground(m: &Model) -> Site {
        match &m.get(named(m, "Ground")).unwrap().kind {
            ElementKind::Site(s) => s.clone(),
            _ => panic!(),
        }
    }

    fn gf(m: &Model) -> (creabim_core::Slab, f64) {
        match &m.get(named(m, "Ground floor slab")).unwrap().kind {
            ElementKind::Slab(s) => {
                let z = m.level(s.level).unwrap().elevation;
                (s.clone(), slab_underside(m, s, z))
            }
            _ => panic!(),
        }
    }

    /// `m` with a pad named `name` over the ground floor at `at` (absolute).
    fn with_pad(m: &Model, name: &str, at: f64) -> (Model, creabim_core::ElementId) {
        let mut m = m.clone();
        let (s, _) = gf(&m);
        let o = s.outline();
        let (lo, hi) = o
            .iter()
            .fold((o[0], o[0]), |(a, b), p| (a.min(*p), b.max(*p)));
        let pad = Site::pad(
            s.level,
            creabim_core::polygon::rectangle(lo + 0.3, hi - 0.3),
            at - m.level(s.level).unwrap().elevation,
        );
        let e = Element::new(name, ElementKind::Site(pad));
        let id = e.id;
        creabim_core::Transaction::new("pad")
            .insert(e)
            .apply(&mut m)
            .unwrap();
        (m, id)
    }

    /// A framed floor stands over a crawl space: the ground under it is
    /// dug the project's 18 in below its joists, not to its underside
    /// (MU140: the earth lay against the joists).
    #[test]
    fn a_framed_floor_digs_a_crawl_space_under_it() {
        let m = ridgeline_framed();
        let (s, under) = gf(&m);
        assert!(framed(&m, &s));
        let site = ground(&m);
        let base = creabim_core::members::elevation(&m, site.level);
        let deepest = building_pads(&m, &site)
            .iter()
            .map(|p| p.level + base)
            .fold(f64::MAX, f64::min);
        assert!(
            (deepest - (under - 0.4572)).abs() < 1e-6,
            "dug to {deepest}, its underside is {under}"
        );
        // a slab on grade still sits in a bed dug to its underside
        let plain = creabim_core::ridgeline::ridgeline_house();
        let (s, under) = gf(&plain);
        assert!(!framed(&plain, &s));
        let deepest = building_pads(&plain, &ground(&plain))
            .iter()
            .map(|p| p.level + base)
            .fold(f64::MAX, f64::min);
        assert!((deepest - under).abs() < 1e-6);
    }

    /// A crawl pad drawn under a framed floor is its crawl floor: the
    /// floor digs nothing of its own, and the pad says when it leaves too
    /// little room, with the level that puts it right.
    #[test]
    fn a_crawl_pad_too_shallow_says_so_and_how_to_fix_it() {
        let m = ridgeline_framed();
        let (_, under) = gf(&m);
        let (m, pad) = with_pad(&m, "Crawl pad", under - 0.23);
        assert!(
            building_pads(&m, &ground(&m))
                .iter()
                .all(|p| (p.level - (under - 0.4572)).abs() > 1e-3),
            "the floor digs nothing of its own over a drawn crawl floor"
        );
        let notes = pad_notes(&m, pad);
        let fix = match notes.as_slice() {
            [PadNote::Shallow {
                room, need, fix, ..
            }] => {
                assert!((room - 0.23).abs() < 1e-6 && (need - 0.4572).abs() < 1e-9);
                *fix
            }
            other => panic!("{other:?}"),
        };
        assert!((fix - (under - 0.4572)).abs() < 1e-6);
        let (m2, pad2) = with_pad(&ridgeline_framed(), "Crawl pad", fix);
        assert!(
            matches!(pad_notes(&m2, pad2).as_slice(), [PadNote::Crawl { .. }]),
            "{:?}",
            pad_notes(&m2, pad2)
        );
    }

    /// A section through a crawl space draws its vapour retarder: a thin
    /// line on its floor, 18 in under the joists.
    #[test]
    fn a_section_draws_the_crawl_spaces_retarder() {
        use crate::plan::{Pen, PlanItem};
        let m = ridgeline_framed();
        let (s, under) = gf(&m);
        let gf_id = named(&m, "Ground floor slab");
        let o = s.outline();
        let (lo, hi) = o
            .iter()
            .fold((o[0], o[0]), |(a, b), p| (a.min(*p), b.max(*p)));
        let y = (lo.y + hi.y) / 2.0;
        let cut = crate::section::section_view(
            &m,
            DVec2::new(lo.x - 3.0, y),
            DVec2::new(hi.x + 3.0, y),
            4.0,
            true,
            false,
        );
        let lines: Vec<(DVec2, DVec2)> = cut
            .items
            .iter()
            .filter_map(|i| match i {
                PlanItem::Line {
                    id,
                    pen: Pen::Fine,
                    a,
                    b,
                } if *id == gf_id => Some((*a, *b)),
                _ => None,
            })
            .collect();
        assert!(!lines.is_empty(), "no retarder line");
        let want = under - 0.4572 + 0.015;
        assert!(
            lines
                .iter()
                .all(|(a, b)| (a.y - want).abs() < 0.02 && (b.y - want).abs() < 0.02),
            "{:?}",
            &lines[..3.min(lines.len())]
        );
        // a slab on grade has none
        let plain = creabim_core::ridgeline::ridgeline_house();
        let cut = crate::section::section_view(
            &plain,
            DVec2::new(lo.x - 3.0, y),
            DVec2::new(hi.x + 3.0, y),
            4.0,
            true,
            false,
        );
        assert!(!cut
            .items
            .iter()
            .any(|i| matches!(i, PlanItem::Line { id, pen: Pen::Fine, .. } if *id == gf_id)));
    }

    /// A pad above the top of a slab it covers buries the floor.
    #[test]
    fn a_pad_over_the_floor_buries_it() {
        let m = creabim_core::ridgeline::ridgeline_house();
        let (s, _) = gf(&m);
        let top = s.top(m.level(s.level).unwrap().elevation);
        let (m, pad) = with_pad(&m, "Buried", top + 0.07);
        match pad_notes(&m, pad).as_slice() {
            [PadNote::Buried { by, fix, .. }, ..] => {
                assert!((by - 0.07).abs() < 1e-6);
                assert!((fix - (top - 0.2032)).abs() < 1e-6);
            }
            other => panic!("{other:?}"),
        }
    }

    /// `m` with its house pad's top set to `top`.
    fn house_pad_at(m: &Model, top_at: f64) -> (Model, creabim_core::ElementId) {
        let mut m = m.clone();
        let id = named(&m, "House pad");
        let mut e = m.get(id).unwrap().clone();
        if let ElementKind::Site(s) = &mut e.kind {
            if let SiteForm::Pad { top, .. } = &mut s.form {
                *top = top_at;
            }
        }
        creabim_core::Transaction::new("pad")
            .replace(m.get(id).unwrap().clone(), e)
            .apply(&mut m)
            .unwrap();
        (m, id)
    }

    /// Ridgeline's house pad: its ground lies 7 7/8 in under the cedar
    /// walls, an eighth short of the 8 in the wood keeps off it, and it
    /// says so with the level that puts it right. Set there, it explains
    /// the slab sunk in it, as REV00 drew it.
    #[test]
    fn ridgelines_house_pad_explains_the_slab_in_it() {
        let m = creabim_core::ridgeline::ridgeline_house();
        let notes = pad_notes(&m, named(&m, "House pad"));
        let fix = match notes.as_slice() {
            [PadNote::AgainstWood { gap, need, fix, .. }] => {
                assert!((gap - 0.2).abs() < 1e-6 && (need - 0.2032).abs() < 1e-9);
                *fix
            }
            other => panic!("{other:?}"),
        };
        let (m, id) = house_pad_at(&m, fix);
        match pad_notes(&m, id).as_slice() {
            [PadNote::Sunk { slab, by, dug_to }] => {
                assert_eq!(slab, "Ground floor slab");
                assert!((by - (0.376 - 0.2032)).abs() < 1e-6 && (dug_to + 0.376).abs() < 1e-6);
            }
            other => panic!("{other:?}"),
        }
    }

    /// The ground round the house kept off the wood: a pad 4 in under the
    /// stud walls' foot says so.
    #[test]
    fn a_pad_against_the_wood_says_so() {
        let (m, id) = house_pad_at(&creabim_core::ridgeline::ridgeline_house(), -0.1);
        let notes = pad_notes(&m, id);
        assert!(
            notes.iter().any(|n| matches!(n, PadNote::AgainstWood { gap, fix, .. } if (gap - 0.1).abs() < 1e-6 && (fix + 0.2032).abs() < 1e-6)),
            "{notes:?}"
        );
        assert!(
            notes.iter().all(PadNote::wrong),
            "blue and orange never together: {notes:?}"
        );
    }

    /// A 40 m square of ground falling 4 m from north to south (1 in 10).
    fn slope() -> (Model, Site) {
        let lv = Element::new(
            "Ground",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let mut t = Site::terrain(
            lv.id,
            creabim_core::polygon::rectangle(DVec2::ZERO, DVec2::new(40.0, 40.0)),
            0.0,
            1.0,
        );
        if let SiteForm::Terrain { points, data, .. } = &mut t.form {
            for i in 0..=8 {
                for j in 0..=8 {
                    let (x, y) = (i as f64 * 5.0, j as f64 * 5.0);
                    points.push(DVec3::new(x, y, y * 0.1));
                }
            }
            data.survey = true;
            data.faceted = true;
        }
        (Model::from_elements([lv]).unwrap(), t)
    }

    fn pads(t: &mut Site) -> &mut creabim_core::GroundData {
        match &mut t.form {
            SiteForm::Terrain { data, .. } => data,
            _ => unreachable!(),
        }
    }

    #[test]
    fn a_road_is_worked_in_along_its_fall_with_its_make_up_under_it() {
        let (m, t) = slope();
        // up the slope, flatter than the ground: made up at its foot,
        // dug in at its head
        let mut road = Site::road(
            t.level,
            vec![DVec2::new(20.0, 2.0), DVec2::new(20.0, 38.0)],
            6.0,
        );
        if let SiteForm::Road(r) = &mut road.form {
            r.path[0].z = 1.0;
            r.path[1].z = 3.0;
        }
        let mut els: Vec<Element> = m.iter().cloned().collect();
        els.push(Element::new("Road", ElementKind::Site(road)));
        let m = Model::from_elements(els).unwrap();
        let s = shaped(&m, &t).unwrap();
        assert!(s.changed());
        assert!(s.cut > 1.0 && s.fill > 1.0, "{} {}", s.cut, s.fill);
        // under its middle the ground is its formation, below its make-up
        let z = s.ground.height(DVec2::new(20.0, 20.0)).unwrap();
        assert!(z < 2.0 - 0.3 && z > 2.0 - 0.8, "{z}");
        // its edges are at its kerbs' tops: 3.1 m out, 2.0 - 0.075 + 0.1
        let e = s.ground.height(DVec2::new(23.0 + 0.1, 20.0)).unwrap();
        assert!((e - 2.025).abs() < 0.1, "{e}");
    }

    #[test]
    fn a_road_following_the_ground_sits_on_it_no_steeper_than_its_limit() {
        let (m, t) = slope();
        let mut els: Vec<Element> = m.iter().cloned().collect();
        els.push(Element::new("Ground", ElementKind::Site(t.clone())));
        // across the slope, then up it: the ground climbs 10%
        let mut road = Site::road(
            t.level,
            vec![
                DVec2::new(5.0, 5.0),
                DVec2::new(20.0, 5.0),
                DVec2::new(20.0, 35.0),
            ],
            5.0,
        );
        if let SiteForm::Road(r) = &mut road.form {
            r.levels = creabim_core::road::Levels::Ground {
                steepest: 0.08,
                ease: 10.0,
            };
        }
        let e = Element::new("Road", ElementKind::Site(road));
        let id = e.id;
        els.push(e);
        let m = Model::from_elements(els).unwrap();
        let tx = settle_roads(&m, &creabim_core::Transaction::new("x"));
        assert_eq!(tx.ops.len(), 1, "it takes its levels");
        let mut m2 = m.clone();
        tx.apply(&mut m2).unwrap();
        let ElementKind::Site(s) = &m2.get(id).unwrap().kind else {
            panic!()
        };
        let SiteForm::Road(r) = &s.form else { panic!() };
        assert!(!r.profile.is_empty() && r.profile_key != 0);
        // never steeper than 8%, though the ground climbs at 10%
        for w in r.profile.windows(2) {
            assert!(
                (w[1] - w[0]).abs() <= 0.08 * creabim_core::road::PROFILE_STEP + 1e-6,
                "{w:?}"
            );
        }
        assert!(r.steepest() <= 0.08 + 1e-6, "{}", r.steepest());
        // across the slope it sits at the ground: 0.5 at y = 5
        assert!((r.profile[5] - 0.5).abs() < 0.15, "{}", r.profile[5]);
        // settled, nothing more to do until the road or the ground changes
        assert!(settle_roads(&m2, &creabim_core::Transaction::new("x"))
            .ops
            .is_empty());
    }

    #[test]
    fn a_bank_stops_at_a_platform_laid_before_it() {
        let (m, t) = slope();
        let mut els: Vec<Element> = m.iter().cloned().collect();
        els.push(Element::new("Ground", ElementKind::Site(t.clone())));
        // a pad east of a raised road: the road's fill bank would run onto it
        let pad = Site::pad(
            t.level,
            creabim_core::polygon::rectangle(DVec2::new(20.0, 10.0), DVec2::new(30.0, 25.0)),
            1.5,
        );
        let pad = Element::new("Pad", ElementKind::Site(pad));
        let pad_id = pad.id;
        els.push(pad);
        let mut road = Site::road(
            t.level,
            vec![DVec2::new(15.0, 5.0), DVec2::new(15.0, 30.0)],
            5.0,
        );
        if let SiteForm::Road(r) = &mut road.form {
            r.path[0].z = 4.0;
            r.path[1].z = 4.0;
        }
        let road = Element::new("Road", ElementKind::Site(road));
        let road_id = road.id;
        els.push(road);
        let m = Model::from_elements(els).unwrap();
        let s = shaped(&m, &t).unwrap();
        // just inside the pad, at its level, not under the road's bank
        let z = s.ground.height(DVec2::new(20.5, 17.0)).unwrap();
        assert!((z - 1.5).abs() < 0.05, "{z}");
        // and the panel can say so
        assert_eq!(runs_onto(&m, road_id), vec![pad_id]);
        assert!(runs_onto(&m, pad_id).is_empty() || runs_onto(&m, pad_id) == vec![road_id]);
    }

    #[test]
    fn a_tree_stands_on_the_ground_under_it_not_on_its_level() {
        let (m, t) = slope();
        let tree = Site::plant(
            t.level,
            DVec2::new(20.0, 30.0),
            creabim_core::PlantForm::Tree,
            4.0,
            6.0,
        );
        let mut els: Vec<Element> = m.iter().cloned().collect();
        els.push(Element::new("Ground", ElementKind::Site(t.clone())));
        els.push(Element::new("Tree", ElementKind::Site(tree.clone())));
        let m = Model::from_elements(els).unwrap();
        let s = shaped(&m, &t).unwrap();
        let ground = s.ground.height(DVec2::new(20.0, 30.0)).unwrap();
        assert!(ground > 1.0, "the slope rises here: {ground}");
        let foot = plant_foot(&m, &tree).unwrap();
        // at the ground, a touch lower on its downhill side, never above
        assert!(
            foot <= ground + 1e-9 && foot > ground - 0.1,
            "{foot} {ground}"
        );
        // off the terrain it keeps its level
        let away = Site::plant(
            t.level,
            DVec2::new(500.0, 500.0),
            creabim_core::PlantForm::Tree,
            4.0,
            6.0,
        );
        assert!(plant_foot(&m, &away).is_none());
    }

    #[test]
    fn a_plant_high_over_the_ground_keeps_its_level_only_over_a_slab() {
        let (m, t) = slope();
        let at = DVec2::new(20.0, 30.0);
        let mut els: Vec<Element> = m.iter().cloned().collect();
        els.push(Element::new("Ground", ElementKind::Site(t.clone())));
        // a level well above the ground there
        let ground = shaped(&Model::from_elements(els.clone()).unwrap(), &t)
            .unwrap()
            .ground
            .height(at)
            .unwrap();
        let lv = creabim_core::Level {
            elevation: ground + 4.0,
            height: 3.0,
        };
        let up = Element::new("Terrace", ElementKind::Level(lv));
        let up_id = up.id;
        els.push(up);
        let tree = Site::plant(up_id, at, creabim_core::PlantForm::Tree, 4.0, 6.0);
        els.push(Element::new("Tree", ElementKind::Site(tree.clone())));
        // no slab under it: it stands on the ground, not in the air
        let open = Model::from_elements(els.clone()).unwrap();
        let foot = plant_foot(&open, &tree).unwrap();
        assert!(
            foot <= ground + 1e-9 && foot > ground - 0.1,
            "{foot} {ground}"
        );
        // over a terrace slab at its level it keeps its level
        let deck = creabim_core::Slab::new(
            up_id,
            creabim_core::polygon::rectangle(at - DVec2::splat(3.0), at + DVec2::splat(3.0)),
            0.2,
        );
        els.push(Element::new("Deck", ElementKind::Slab(deck)));
        let built = Model::from_elements(els).unwrap();
        assert!(plant_foot(&built, &tree).is_none());
        assert!((plant_z(&built, &tree) - (ground + 4.0)).abs() < 1e-9);
    }

    #[test]
    fn ridgeline_plants_stand_on_the_ground_drawn_under_them() {
        // owner film review 2026-11-22: the two sugar maples floated over
        // the lot's ground where it falls to the south
        let m = creabim_core::ridgeline::ridgeline_house();
        let terrain = m
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Site(t) if matches!(t.form, SiteForm::Terrain { .. }) => {
                    Some(t.clone())
                }
                _ => None,
            })
            .unwrap();
        let s = shaped(&m, &terrain).unwrap();
        let tb = creabim_core::members::elevation(&m, terrain.level);
        let mut out = vec![];
        for e in m.iter() {
            let ElementKind::Site(t) = &e.kind else {
                continue;
            };
            let SiteForm::Plant { position, base, .. } = &t.form else {
                continue;
            };
            let Some(g) = s.ground.height(*position) else {
                continue;
            };
            let ground = tb + g + base;
            let z = plant_z(&m, t);
            let own = creabim_core::members::elevation(&m, t.level);
            out.push(format!(
                "{}: plant {z:.3} ground {ground:.3} level {own:.3}",
                e.name
            ));
            // at the ground, a touch lower on a slope, never above it
            assert!(
                z <= ground + 1e-6 && z > ground - 0.15,
                "{}",
                out.join(
                    "
"
                )
            );
        }
        assert!(out.len() >= 5, "{out:?}");
    }

    #[test]
    fn a_terrain_with_nothing_worked_in_is_its_survey() {
        let (m, t) = slope();
        let s = shaped(&m, &t).unwrap();
        assert!(!s.changed());
        assert_eq!((s.cut, s.fill), (0.0, 0.0));
    }

    #[test]
    fn a_pad_is_level_and_its_banks_meet_the_ground() {
        let (m, mut t) = slope();
        // a 10 m square pad at the height of the ground through its middle
        let pad = Pad::new(
            creabim_core::polygon::rectangle(DVec2::new(15.0, 15.0), DVec2::new(25.0, 25.0)),
            2.0,
        );
        pads(&mut t).pads.push(pad);
        let s = shaped(&m, &t).unwrap();
        assert!(s.changed());
        let h = |x: f64, y: f64| s.ground.height(DVec2::new(x, y)).unwrap();
        // level all over
        for (x, y) in [(16.0, 16.0), (20.0, 20.0), (24.0, 24.0), (20.0, 15.5)] {
            assert!((h(x, y) - 2.0).abs() < 1e-6, "{x} {y}: {}", h(x, y));
        }
        // far from it the ground is as it was
        assert!((h(5.0, 5.0) - 0.5).abs() < 1e-6);
        assert!((h(20.0, 38.0) - 3.8).abs() < 1e-6);
        // dug 1 in 1 on the uphill side: half a metre out it has climbed
        // half a metre, and it meets the 1 in 10 ground 0.5 / 0.9 m out
        assert!((h(20.0, 25.25) - 2.25).abs() < 0.02, "{}", h(20.0, 25.25));
        assert!((h(20.0, 26.0) - 2.6).abs() < 0.02, "{}", h(20.0, 26.0));
        // made up 1 in 2 on the downhill side: it meets the ground
        // 0.5 / 0.4 = 1.25 m out
        assert!((h(20.0, 14.5) - 1.75).abs() < 0.02, "{}", h(20.0, 14.5));
        assert!((h(20.0, 13.0) - 1.3).abs() < 0.02, "{}", h(20.0, 13.0));
        // by the prism it is: a wedge 10 m wide, 5 m long and half a metre
        // high at its end each way is 12.5 m3, and the banks add a little
        assert!(s.cut > 12.5 && s.cut < 16.0, "{}", s.cut);
        assert!(s.fill > 12.5 && s.fill < 18.0, "{}", s.fill);
        assert_eq!(s.banks.len(), 1);
    }

    #[test]
    fn a_trench_is_so_deep_all_along_and_its_spoil_is_split_by_layer() {
        let (m, mut t) = slope();
        // 0.6 m wide, 1.2 m deep, straight up the slope, upright sides
        let outline = creabim_core::site::trench_outline(
            &[DVec2::new(20.0, 8.0), DVec2::new(20.0, 32.0)],
            0.6,
        );
        assert_eq!(outline.len(), 4);
        let d = pads(&mut t);
        d.pads.push(Pad::dig(outline, 1.2, 0.0));
        d.strata = vec![
            creabim_core::Stratum {
                name: "Topsoil".into(),
                thickness: 0.3,
            },
            creabim_core::Stratum {
                name: "Clay".into(),
                thickness: 0.5,
            },
        ];
        let s = shaped(&m, &t).unwrap();
        let h = |x: f64, y: f64| s.ground.height(DVec2::new(x, y)).unwrap();
        // its floor follows the ground, 1.2 m under it
        for y in [10.0, 20.0, 30.0] {
            assert!(
                (h(20.0, y) - (y * 0.1 - 1.2)).abs() < 0.03,
                "{y}: {}",
                h(20.0, y)
            );
        }
        // beside it the ground is as it was
        assert!((h(21.0, 20.0) - 2.0).abs() < 1e-6);
        // 24 m by 0.6 m by 1.2 m is 17.28 m3: 4.32 of topsoil, 7.2 of
        // clay and 5.76 from under them
        assert!((s.cut - 17.28).abs() < 1.2, "{}", s.cut);
        assert!(s.fill < 0.05);
        let l = &s.cut_layers;
        assert_eq!(l.len(), 3);
        assert!((l[0] - 4.32).abs() < 0.4, "{l:?}");
        assert!((l[1] - 7.2).abs() < 0.6, "{l:?}");
        assert!((l[2] - 5.76).abs() < 0.6, "{l:?}");
        assert!((l.iter().sum::<f64>() - s.cut).abs() < 1e-6);
    }

    #[test]
    fn banks_meet_in_an_inside_corner_without_throwing_the_ground_up() {
        let (m, mut t) = slope();
        if let SiteForm::Terrain { data, .. } = &mut t.form {
            // read smooth, as a survey is: the reading that overshoots
            data.faceted = false;
        }
        // a trench with a bend, upright, and an L of a pad dug 1 in 1
        let d = pads(&mut t);
        d.pads.push(Pad::dig(
            creabim_core::site::trench_outline(
                &[
                    DVec2::new(5.0, 8.0),
                    DVec2::new(17.0, 7.0),
                    DVec2::new(24.5, 7.5),
                ],
                0.6,
            ),
            1.0,
            0.0,
        ));
        d.pads.push(Pad::new(
            vec![
                DVec2::new(10.0, 20.0),
                DVec2::new(30.0, 20.0),
                DVec2::new(30.0, 26.0),
                DVec2::new(18.0, 26.0),
                DVec2::new(18.0, 34.0),
                DVec2::new(10.0, 34.0),
            ],
            1.5,
        ));
        let s = shaped(&m, &t).unwrap();
        assert_eq!(s.banks.len(), 2);
        // nowhere does the designed ground leave the range the survey and
        // the works span: from the trench's floor to the top of the slope
        let (lo, hi) = s.ground.range();
        assert!(lo > -0.45 && hi < 4.05, "{lo} {hi}");
        // and in the L's inside corner the banks meet in a valley
        let h = s.ground.height(DVec2::new(18.7, 26.7)).unwrap();
        assert!(h > 1.5 && h < 3.0, "{h}");
    }

    #[test]
    fn a_footing_is_buried_and_leaves_the_ground_as_it_was() {
        let (m, mut t) = slope();
        let lv = m.iter().next().unwrap().clone();
        // a strip footing at the level (0), where the ground is well
        // above it: it is under the ground from end to end
        let f = creabim_core::Footing::strip(lv.id, DVec2::new(15.0, 20.0), DVec2::new(25.0, 20.0));
        let m =
            Model::from_elements([lv, Element::new("Footing", ElementKind::Footing(f))]).unwrap();
        pads(&mut t).under_buildings = true;
        // the building digs nothing for it, and the ground stays
        assert!(building_pads(&m, &t).is_empty());
        let s = shaped(&m, &t).unwrap();
        assert!(!s.changed(), "a buried footing must not open the ground");
    }

    #[test]
    fn the_ground_is_dug_under_a_slab_and_left_where_it_is_lower() {
        let (m, mut t) = slope();
        let lv = m.iter().next().unwrap().clone();
        let level = lv.id;
        // a slab 300 thick with its top at 2.0: its underside, 1.7, is in
        // the ground north of y = 17 and above it to the south
        let mut slab = creabim_core::Slab::new(
            level,
            creabim_core::polygon::rectangle(DVec2::new(15.0, 12.0), DVec2::new(25.0, 22.0)),
            0.3,
        );
        slab.top_offset = 2.0;
        let m = Model::from_elements([lv, Element::new("Slab", ElementKind::Slab(slab))]).unwrap();
        // not asked for, the ground stays
        assert!(!shaped(&m, &t).unwrap().changed());
        pads(&mut t).under_buildings = true;
        let s = shaped(&m, &t).unwrap();
        assert!(s.changed());
        let h = |x: f64, y: f64| s.ground.height(DVec2::new(x, y)).unwrap();
        // dug to the underside where the ground was above it
        assert!((h(20.0, 21.0) - 1.7).abs() < 1e-6, "{}", h(20.0, 21.0));
        assert!((h(20.0, 18.0) - 1.7).abs() < 1e-6, "{}", h(20.0, 18.0));
        assert!((h(20.0, 17.2) - 1.7).abs() < 0.02, "{}", h(20.0, 17.2));
        // left alone where it was below
        assert!((h(20.0, 13.0) - 1.3).abs() < 0.02, "{}", h(20.0, 13.0));
        // an upright face behind it: the ground is back within a hand
        assert!((h(20.0, 22.1) - 2.21).abs() < 0.02, "{}", h(20.0, 22.1));
        // nothing is made up, and what is dug is the wedge: 10 m wide,
        // 5 m long, 0.5 m deep at its end
        assert!(s.fill < 0.05, "{}", s.fill);
        assert!((s.cut - 12.5).abs() < 0.8, "{}", s.cut);
    }
}

#[cfg(test)]
mod window_tests {
    use super::*;

    #[test]
    fn pads_in_a_large_survey_are_worked_in_a_window_to_the_same_ground() {
        let level = creabim_core::ElementId::new();
        let mut site = Site::terrain(level, vec![], 0.0, 0.5);
        let SiteForm::Terrain {
            boundary,
            points,
            data,
            ..
        } = &mut site.form
        else {
            panic!()
        };
        for j in 0..31 {
            for i in 0..41 {
                let (x, y) = (i as f64 * 3.0 - 60.0, j as f64 * 3.0 - 45.0);
                points.push(DVec3::new(
                    x,
                    y,
                    0.06 * y + 0.02 * x + (x / 11.0).sin() * 0.3,
                ));
            }
        }
        *boundary =
            creabim_core::polygon::rectangle(DVec2::new(-60.0, -45.0), DVec2::new(60.0, 45.0));
        data.survey = true;
        let rect = |x: f64, y: f64, w: f64, h: f64| {
            creabim_core::polygon::rectangle(DVec2::new(x, y), DVec2::new(x + w, y + h))
        };
        // three pads, two of them side by side so that one's bank meets the other
        let pads = vec![
            Pad::new(rect(-6.0, -4.0, 8.0, 7.0), 0.2),
            Pad::new(rect(2.0, -4.0, 6.0, 7.0), -0.3),
            Pad::dig(rect(-3.0, 6.0, 3.0, 2.0), 0.8, 0.0),
        ];
        let w = window(&site, &pads).expect("a window round the pads");
        assert!(w.0.x > -60.0 && w.1.y < 45.0);
        let existing = crate::ground::of(&site).unwrap();
        let (_, a, banks) = work_all(&existing, &site, &pads, Some(w)).unwrap();
        let (_, b, _) = work_all(&existing, &site, &pads, None).unwrap();
        assert_eq!(banks.len(), 3);
        let (a, b) = (Ground::build(&a).unwrap(), Ground::build(&b).unwrap());
        let mut worst: f64 = 0.0;
        for j in 0..40 {
            for i in 0..50 {
                let p = DVec2::new(i as f64 * 0.7 - 14.0, j as f64 * 0.7 - 12.0);
                worst = worst.max((a.height(p).unwrap() - b.height(p).unwrap()).abs());
            }
        }
        assert!(worst < 0.01, "{worst}");
        // a small terrain, or pads by its edge, are worked in whole
        assert!(window(&site, &pads[..1]).is_none());
        let by_the_edge = vec![pads[0].clone(), Pad::new(rect(50.0, 30.0, 5.0, 5.0), 0.0)];
        assert!(window(&site, &by_the_edge).is_none());
    }
}
