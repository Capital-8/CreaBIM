use crate::{triangulate::triangulate, wall::wall_mesh, TriMesh};
use creabim_core::{assembly, csg, BoolOp, Element, ElementId, ElementKind, Model, Opening};
use glam::{DVec2, DVec3};

/// Roughness, metallic and opacity of an element without a material.
pub const PLAIN_SURFACE: [f32; 3] = [creabim_core::element::DEFAULT_ROUGHNESS, 0.0, 1.0];

/// One element's renderable geometry.
#[derive(Clone, Debug)]
pub struct SceneItem {
    pub id: ElementId,
    pub mesh: TriMesh,
    pub color: [f32; 3],
    /// Roughness, metallic and opacity, from the element's material.
    pub surface: [f32; 3],
    /// A cutter: drawn as a see-through outline in 3D, left out of
    /// drawings and exports, but still clickable.
    pub ghost: bool,
    /// For one layer of a layered wall, slab or roof: which layer. A
    /// layered element is one item per layer, all with its id.
    pub layer: Option<LayerPart>,
    /// A look of its own over the element's material, by name: a
    /// window's glass, a door's handle.
    pub finish: Option<&'static str>,
}

/// Which layer of its element's assembly a scene item is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerPart {
    pub index: usize,
    pub material: Option<ElementId>,
    pub function: creabim_core::LayerFunction,
    pub core: bool,
    /// Its join priority: lower runs through higher where elements meet.
    pub priority: u8,
}

impl LayerPart {
    pub fn of(b: &assembly::Band) -> Self {
        Self {
            index: b.index,
            material: b.material,
            function: b.function,
            core: b.core,
            priority: b.priority,
        }
    }
}

/// Default colours for layers with no material, by function.
fn layer_color(f: creabim_core::LayerFunction, plain: [f32; 3]) -> [f32; 3] {
    use creabim_core::LayerFunction as F;
    match f {
        F::Insulation => [0.93, 0.86, 0.45],
        F::Membrane => [0.25, 0.27, 0.30],
        F::Finish => [0.92, 0.91, 0.88],
        _ => plain,
    }
}

/// The solid of each layer of a layered element, in model coordinates,
/// air gaps left out: what an export writes as the element's parts.
/// Empty for an element that isn't layered.
pub fn layer_parts(
    model: &Model,
    e: &Element,
    joins: &std::collections::HashMap<ElementId, creabim_core::WallEnds>,
) -> Vec<(LayerPart, TriMesh)> {
    layer_items(model, e, joins, [0.8; 3])
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_lining())
        .filter_map(|i| Some((i.layer?, i.mesh)))
        .collect()
}

/// The solid of each layer of a layered element, with its look; air gaps
/// are left empty. `None` for an element that isn't layered.
fn layer_items(
    model: &Model,
    e: &Element,
    joins: &std::collections::HashMap<ElementId, creabim_core::WallEnds>,
    plain: [f32; 3],
) -> Option<Vec<SceneItem>> {
    let (_, a) = assembly::assembly_of(model, e)?;
    let mut linings: Vec<SceneItem> = Vec::new();
    let parts: Vec<(assembly::Band, TriMesh)> = match &e.kind {
        ElementKind::Wall(w) => {
            let level = model.level(w.level)?;
            // its doors and windows, and the curtain walls set in it
            let owned = creabim_core::curtain::wall_openings(model, e.id);
            let openings: Vec<&Opening> = owned.iter().collect();
            let ends = joins.get(&e.id).cloned().unwrap_or_default();
            let top = creabim_core::walls::top_profile(model, w);
            linings = reveal_linings(
                model,
                e.id,
                w,
                &openings,
                &assembly::wall_bands(a, w),
                level.elevation,
                plain,
            );
            assembly::wall_bands(a, w)
                .into_iter()
                .map(|b| {
                    let m = crate::wall::wall_layer_mesh_v(
                        w,
                        level.elevation,
                        &openings,
                        &ends,
                        top.as_ref(),
                        (b.s0, b.s1),
                        b.core,
                        b.ext,
                        b.vext,
                    );
                    (b, m)
                })
                .collect()
        }
        ElementKind::Slab(s) => {
            let level = model.level(s.level)?;
            let top = s.top(level.elevation);
            let holes = creabim_core::slab::holes_of(model, e);
            assembly::depth_bands(a, s.thickness)
                .into_iter()
                .map(|b| {
                    let outline = s.layer_outline(b.index);
                    let m = slab_mesh_with_holes(&outline, &holes, top - b.s0, b.s1 - b.s0);
                    (b, m)
                })
                .collect()
        }
        ElementKind::Roof(r) => {
            let base = r.base(model);
            // layers are square to the slope; upright they are thicker
            let k = r.vertical_thickness() / r.thickness.max(1e-9);
            let tv = r.vertical_thickness();
            let holes = creabim_core::slab::holes_of(model, e);
            let windows = crate::members::roof_holes(model, e.id);
            assembly::depth_bands(a, r.thickness)
                .into_iter()
                .map(|b| {
                    let m = crate::members::roof_layer_mesh_with(
                        r,
                        base,
                        tv - b.s1 * k,
                        tv - b.s0 * k,
                        Some(b.index),
                        &windows,
                    );
                    (b, crate::members::cut_holes(m, &holes))
                })
                .collect()
        }
        _ => return None,
    };
    Some(
        parts
            .into_iter()
            .filter(|(b, _)| b.function != creabim_core::LayerFunction::AirGap)
            .map(|(b, mesh)| {
                let look = b.material.and_then(|m| match &model.get(m)?.kind {
                    ElementKind::Material(m) => Some((m.color, m.surface())),
                    _ => None,
                });
                let (color, surface) =
                    look.unwrap_or((layer_color(b.function, plain), PLAIN_SURFACE));
                SceneItem {
                    id: e.id,
                    mesh,
                    color,
                    surface,
                    ghost: false,
                    layer: Some(LayerPart::of(&b)),
                    finish: None,
                }
            })
            .chain(linings)
            .collect(),
    )
}

/// The [`SceneItem::finish`] of a reveal lining.
pub const REVEAL: &str = "Reveal";
/// The finish of a terrain's contour lines lying on it in the 3D view.
pub const GROUND_LINES: &str = "Ground lines";
/// The finish of the thin skin a pad without a base course shows in 3D,
/// so it reads as a made platform: nothing built, so no drawing cuts it
/// (owner, cut test: it stood 12 mm proud of a retaining wall's foot).
/// It is still the pad's shape to pick and to export.
pub const PAD_SKIN: &str = "Pad skin";

/// The finish of a lot's boundary and setback lying on the ground in 3D,
/// which the 3D view can leave out.
pub const LOT_LINES: &str = "Lot lines";

/// The finish of a lot's envelope: the see-through volume its setbacks and
/// height limit leave to build in. It stands round the whole house, so it
/// never takes a pick from anything behind its faces (see [`crate::pick`]).
pub const LOT_ENVELOPE: &str = "Lot envelope";

/// The finish of doors' and windows' linings, architraves and thresholds,
/// which drawings show at 1:20 and larger only.
pub const TRIM: &str = "Trim";

/// The coarsest scale (its denominator) drawings show trim at.
pub const TRIM_SCALE: f64 = 20.0;

impl SceneItem {
    /// A door's or window's trim: left out of drawings coarser than 1:20.
    pub fn is_trim(&self) -> bool {
        self.finish == Some(TRIM)
    }
}

/// How thick a reveal lining is: thin enough to leave the opening its size.
const LINING: f64 = 0.004;

impl SceneItem {
    /// A reveal lining or a ground's contour lines, seen in 3D only:
    /// drawings and exports leave it out.
    pub fn is_lining(&self) -> bool {
        matches!(self.finish, Some(REVEAL) | Some(GROUND_LINES))
    }
}

/// The reveals of a layered wall's doors and windows lined, for the 3D
/// view: each layer's part of the jambs, head and sill in the finish on
/// its side of the wall (render outside, plaster inside), so an opening
/// doesn't show the insulation or the membrane cut open.
fn reveal_linings(
    model: &Model,
    id: ElementId,
    w: &creabim_core::Wall,
    openings: &[&Opening],
    bands: &[assembly::Band],
    level_z: f64,
    plain: [f32; 3],
) -> Vec<SceneItem> {
    use creabim_core::LayerFunction as F;
    let look = |b: &assembly::Band| {
        b.material
            .and_then(|m| match &model.get(m)?.kind {
                ElementKind::Material(m) => Some((m.color, m.surface())),
                _ => None,
            })
            .unwrap_or((layer_color(b.function, plain), PLAIN_SURFACE))
    };
    let finishes: Vec<&assembly::Band> = bands.iter().filter(|b| b.function == F::Finish).collect();
    let d = w.direction();
    let n = DVec2::new(-d.y, d.x);
    let z0 = level_z + w.base_offset;
    let at = |u: f64, v: f64, s: f64| (w.start + d * u + n * s).extend(z0 + v);
    let mut out = Vec::new();
    for o in openings {
        if o.kind == creabim_core::OpeningKind::Hole {
            continue;
        }
        let Some(c) = crate::wall::Cut::of(o, w) else {
            continue;
        };
        for b in bands.iter().filter(|b| b.function != F::AirGap) {
            // a finish shows itself; the rest take the finish nearest them
            let mid = (b.s0 + b.s1) / 2.0;
            let from = if b.function == F::Finish {
                Some(b)
            } else {
                finishes.iter().copied().min_by(|x, y| {
                    ((x.s0 + x.s1) / 2.0 - mid)
                        .abs()
                        .total_cmp(&((y.s0 + y.s1) / 2.0 - mid).abs())
                })
            };
            let (color, surface) = from.map_or((plain, PLAIN_SURFACE), look);
            let mut mesh = TriMesh::default();
            let mut pieces: Vec<[DVec3; 8]> = vec![];
            let mut slab = |u0: f64, u1: f64, v0: f64, v1: f64| {
                let p = |i: usize| {
                    at(
                        if i & 1 == 0 { u0 } else { u1 },
                        if i & 2 == 0 { v0 } else { v1 },
                        if i & 4 == 0 { b.s0 } else { b.s1 },
                    )
                };
                pieces.push([p(0), p(1), p(2), p(3), p(4), p(5), p(6), p(7)]);
            };
            let mut extra: Vec<[DVec3; 8]> = vec![];
            match crate::wall::head_of(o, w) {
                // an arched or raked head lined in straight pieces
                Some(pts) => {
                    slab(c.u0, c.u0 + LINING, c.v0, pts[0].1);
                    slab(c.u1 - LINING, c.u1, c.v0, pts[pts.len() - 1].1);
                    for q in pts.windows(2) {
                        let ((ua, va), (ub, vb)) = (q[0], q[1]);
                        let p = |i: usize| {
                            let (u, v) = if i & 1 == 0 { (ua, va) } else { (ub, vb) };
                            at(
                                u,
                                if i & 2 == 0 { v - LINING } else { v },
                                if i & 4 == 0 { b.s0 } else { b.s1 },
                            )
                        };
                        extra.push([p(0), p(1), p(2), p(3), p(4), p(5), p(6), p(7)]);
                    }
                }
                None => {
                    slab(c.u0, c.u0 + LINING, c.v0, c.v1);
                    slab(c.u1 - LINING, c.u1, c.v0, c.v1);
                    slab(c.u0, c.u1, c.v1 - LINING, c.v1);
                }
            }
            if c.v0 > 1e-6 {
                slab(c.u0, c.u1, c.v0, c.v0 + LINING);
            }
            for q in pieces.into_iter().chain(extra) {
                cuboid(&mut mesh, q);
            }
            // one item per finish, for all the wall's openings
            match out.iter_mut().find(|i: &&mut SceneItem| i.color == color) {
                Some(i) => i.mesh.append(&mesh),
                None => out.push(SceneItem {
                    id,
                    mesh,
                    color,
                    surface,
                    ghost: false,
                    layer: None,
                    finish: Some(REVEAL),
                }),
            }
        }
    }
    out
}

/// A six-sided box from its corners, corner `i` having bit 0 for the far
/// end along the first axis, bit 1 along the second and bit 2 along the
/// third.
fn cuboid(m: &mut TriMesh, c: [DVec3; 8]) {
    let centre = c.iter().copied().sum::<DVec3>() / 8.0;
    for face in [
        [0, 1, 3, 2],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 3, 7, 6],
        [0, 2, 6, 4],
        [1, 3, 7, 5],
    ] {
        let q = face.map(|i| c[i]);
        let mid = q.iter().copied().sum::<DVec3>() / 4.0;
        let normal = (q[1] - q[0]).cross(q[3] - q[0]);
        let normal = if normal.dot(mid - centre) < 0.0 {
            -normal
        } else {
            normal
        };
        m.add_quad(q, normal);
    }
}

/// The lines of every sketch, in model coordinates: (sketch, from, to).
pub fn sketch_lines(model: &Model) -> Vec<(ElementId, DVec3, DVec3)> {
    let mut out = Vec::new();
    for e in model.iter() {
        if let ElementKind::Sketch(s) = &e.kind {
            let z = s
                .level
                .and_then(|l| model.level(l))
                .map_or(0.0, |l| l.elevation);
            for (a, b) in s.segments() {
                out.push((e.id, a + DVec3::Z * z, b + DVec3::Z * z));
            }
        }
    }
    out
}

/// Build viewport geometry for every element with a body.
pub fn build_scene(model: &Model) -> Vec<SceneItem> {
    build_scene_of(model, None)
}

/// [`build_scene`] for `only` these elements (with what cuts them, which
/// the caller includes): a plan slicing a few elements needn't mesh the
/// whole model (30-40 ms for the starter house, every drag frame).
pub fn build_scene_of(
    model: &Model,
    only: Option<&std::collections::HashSet<ElementId>>,
) -> Vec<SceneItem> {
    let mut out = build_scene_unjoined(model, only);
    apply_joins(
        model,
        &mut out,
        CARRY.load(std::sync::atomic::Ordering::Relaxed),
    );
    mend_built(model, &mut out);
    out
}

/// The walls', slabs' and roofs' pieces with no corner of one triangle
/// lying along the side of another (see [`crate::cropbox::mend`]). Their
/// joins and openings split their faces, and a face's pieces that didn't
/// share their corners drew their seams in 3D as short lines across a
/// wall (MU143, owner, the starter house: "more inconsistencies"). Kept by
/// shape, so a build mends only what changed.
fn mend_built(model: &Model, items: &mut [SceneItem]) {
    let now = JOIN_BUILD.load(std::sync::atomic::Ordering::Relaxed);
    for i in items.iter_mut() {
        let built = matches!(
            model.get(i.id).map(|e| &e.kind),
            Some(ElementKind::Wall(_) | ElementKind::Slab(_) | ElementKind::Roof(_))
        );
        if i.ghost || !built {
            continue;
        }
        if let Some(m) = mend_kept(&i.mesh, now) {
            i.mesh = (*m).clone();
        }
    }
}

/// `mesh` mended (`None`: nothing to mend), kept by its shape.
fn mend_kept(mesh: &TriMesh, now: u64) -> Option<std::sync::Arc<TriMesh>> {
    let key = mesh.fingerprint();
    if let Some(m) = mended().get(&key, now) {
        return m;
    }
    let m = crate::cropbox::mend(mesh);
    // only what changed is kept
    let m = (m.indices.len() != mesh.indices.len()).then(|| std::sync::Arc::new(m));
    let bytes = m.as_deref().map_or(16, mesh_bytes);
    mended().insert(key, m.clone(), bytes, now, join_budget() / 2);
    m
}

/// Mended pieces by the shape they mend (`None`: nothing to mend).
type Mended = Memo<u64, Option<std::sync::Arc<TriMesh>>>;

static MENDED: std::sync::LazyLock<std::sync::Mutex<Mended>> =
    std::sync::LazyLock::new(Default::default);

fn mended() -> std::sync::MutexGuard<'static, Mended> {
    MENDED.lock().unwrap_or_else(|e| e.into_inner())
}

/// The scene for exchange (IFC, IFCX): each element's own body, its
/// explicit cuts applied but not the wall and slab joins, so a slab
/// stays the plain extrusion other tools read and re-import as a slab.
pub fn build_scene_for_exchange(model: &Model) -> Vec<SceneItem> {
    build_scene_unjoined(model, None)
}

/// [`build_scene_for_exchange`] for `only` these elements (timing probe).
#[doc(hidden)]
pub fn build_scene_for_exchange_of(
    model: &Model,
    only: &std::collections::HashSet<ElementId>,
) -> Vec<SceneItem> {
    build_scene_unjoined(model, Some(only))
}

static UNJOINED: std::sync::Mutex<Option<(u64, std::sync::Arc<Vec<SceneItem>>)>> =
    std::sync::Mutex::new(None);

/// The scene of `model`'s revision before walls and slabs are cut into
/// each other: what a project just opened shows at once while the worker
/// makes those cuts (Highland Haven: 88 ms, against 18 s with the cuts).
/// The last one is kept, by revision.
pub fn shared_unjoined_scene(model: &Model) -> std::sync::Arc<Vec<SceneItem>> {
    let rev = model.scene_revision();
    if let Some((r, s)) = UNJOINED.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if *r == rev && rev != 0 {
            return s.clone();
        }
    }
    let scene = std::sync::Arc::new(build_scene_unjoined(model, None));
    *UNJOINED.lock().unwrap_or_else(|e| e.into_inner()) = Some((rev, scene.clone()));
    scene
}

/// The box `model`'s scene fills, from the joined scene if it is built
/// already, else from the unjoined one: a join only cuts away, so the box
/// is the same, and framing a view never waits for the cuts.
pub fn scene_extents(model: &Model) -> Option<(glam::Vec3, glam::Vec3)> {
    let scene = shared_scene_ready(model).unwrap_or_else(|| shared_unjoined_scene(model));
    scene
        .iter()
        .filter_map(|s| s.mesh.bounds())
        .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))
}

fn build_scene_unjoined(
    model: &Model,
    only: Option<&std::collections::HashSet<ElementId>>,
) -> Vec<SceneItem> {
    let mut out = Vec::new();
    // how walls meet: of the whole building, so not worked out when only
    // shapes and drawings are asked for (it was most of the 25 ms a pulled
    // shape took to be built again alone, MU46)
    let joined = only.is_none_or(|o| {
        o.iter().any(|id| {
            model.get(*id).is_some_and(|e| {
                !matches!(e.kind, ElementKind::FreeMesh(_) | ElementKind::Sketch(_))
            })
        })
    });
    let joins = if joined {
        creabim_core::wall_joins(model)
    } else {
        Default::default()
    };
    for e in model.iter() {
        if only.is_some_and(|o| !o.contains(&e.id)) {
            continue;
        }
        let plain = match &e.kind {
            ElementKind::Wall(_) => [0.86, 0.84, 0.80],
            ElementKind::Slab(_) => [0.70, 0.70, 0.72],
            _ => [0.66, 0.34, 0.26],
        };
        if let ElementKind::Wall(
            w @ creabim_core::Wall {
                curtain: Some(_), ..
            },
        ) = &e.kind
        {
            out.extend(curtain_items(model, e, w));
            continue;
        }
        if let Some(layers) = layer_items(model, e, &joins, plain) {
            out.extend(layers);
            continue;
        }
        let (mesh, color) = match &e.kind {
            ElementKind::Wall(w) => {
                let Some(level) = model.level(w.level) else {
                    continue;
                };
                // its doors and windows, and the curtain walls set in it
                let owned = creabim_core::curtain::wall_openings(model, e.id);
                let openings: Vec<&Opening> = owned.iter().collect();
                let ends = joins.get(&e.id).cloned().unwrap_or_default();
                let top = creabim_core::walls::top_profile(model, w);
                (
                    wall_mesh(w, level.elevation, &openings, &ends, top.as_ref()),
                    [0.86, 0.84, 0.80],
                )
            }
            ElementKind::Slab(s) => {
                let Some(level) = model.level(s.level) else {
                    continue;
                };
                let holes = creabim_core::slab::holes_of(model, e);
                (
                    slab_mesh_with_holes(&s.outline(), &holes, s.top(level.elevation), s.thickness),
                    [0.70, 0.70, 0.72],
                )
            }
            ElementKind::Shaft(s) => {
                // a see-through stand-in, to see and pick the shaft by
                let (z0, z1) = s.span(model);
                out.push(SceneItem {
                    id: e.id,
                    mesh: slab_mesh(&s.boundary, z1, z1 - z0),
                    color: [0.85, 0.45, 0.25],
                    surface: PLAIN_SURFACE,
                    ghost: true,
                    layer: None,
                    finish: None,
                });
                continue;
            }
            ElementKind::Ceiling(c) => {
                let Some(outline) = c.outline(model) else {
                    continue;
                };
                let (_, top) = c.span(model);
                (slab_mesh(&outline, top, c.thickness), [0.95, 0.95, 0.93])
            }
            ElementKind::Opening(o) => {
                // a roof window, set in its roof's slope
                if o.roof_at.is_some() {
                    out.extend(roof_window_items(model, e, o));
                    continue;
                }
                let Some(ElementKind::Wall(w)) = model.get(o.host).map(|h| &h.kind) else {
                    continue;
                };
                let Some(level) = model.level(w.level) else {
                    continue;
                };
                out.extend(opening_items(model, e, o, w, level.elevation));
                // a bare hole has nothing in it: a see-through box shows
                // it in 3D and lets it be clicked (MU18)
                if o.kind == creabim_core::OpeningKind::Hole {
                    if let Some(mesh) = crate::opening::cut_box_mesh(o, w, level.elevation) {
                        out.push(SceneItem {
                            id: e.id,
                            mesh,
                            color: [0.85, 0.45, 0.25],
                            surface: PLAIN_SURFACE,
                            ghost: true,
                            layer: None,
                            finish: None,
                        });
                    }
                }
                continue;
            }
            ElementKind::Part(_) | ElementKind::Site(_) => {
                out.extend(crate::parts::scene_items(model, e));
                continue;
            }
            ElementKind::Object(_) => {
                // one item per part, each in its own material
                for (mesh, material) in creabim_core::objects::model_parts(model, e) {
                    let look = material.and_then(|m| match &model.get(m)?.kind {
                        ElementKind::Material(m) => Some((m.color, m.surface())),
                        _ => None,
                    });
                    let (color, surface) = look.unwrap_or(([0.8, 0.78, 0.74], PLAIN_SURFACE));
                    out.push(SceneItem {
                        id: e.id,
                        mesh: edit_mesh_to_tri(&mesh, DVec3::ZERO),
                        color,
                        surface,
                        ghost: false,
                        layer: None,
                        finish: None,
                    });
                }
                continue;
            }
            ElementKind::FreeMesh(f) => {
                let z = f
                    .level
                    .and_then(|l| model.level(l))
                    .map_or(0.0, |l| l.elevation);
                (
                    edit_mesh_to_tri(&f.evaluated().mesh, f.origin + DVec3::Z * z),
                    [0.62, 0.75, 0.90],
                )
            }
            ElementKind::Level(_)
            | ElementKind::Grid(_)
            | ElementKind::View(_)
            | ElementKind::Sheet(_)
            | ElementKind::Viewport(_)
            | ElementKind::Material(_)
            | ElementKind::Assembly(_)
            | ElementKind::Dimension(_)
            | ElementKind::Tag(_)
            | ElementKind::Note(_)
            | ElementKind::TextStyle(_)
            | ElementKind::SheetTemplate(_)
            | ElementKind::Units(_)
            | ElementKind::Project(_)
            | ElementKind::Reference(_)
            | ElementKind::PaperMark(_)
            | ElementKind::MarkRules(_)
            | ElementKind::StylePack(_)
            | ElementKind::ElementType(_)
            | ElementKind::Sketch(_)
            | ElementKind::Cut(_)
            | ElementKind::Guide(_)
            | ElementKind::CropBox(_)
            | ElementKind::GridGroup(_)
            | ElementKind::Room(_)
            | ElementKind::RoomSeparator(_)
            | ElementKind::Join(_)
            | ElementKind::Detail(_) => continue,
            ElementKind::Stair(st) => {
                // its structure in its own material, each finish over it
                // a layer in the finish's
                let (structure, finishes) =
                    crate::members::stair_parts(st, st.base(model), st.rise(model));
                let (c, surface) = creabim_core::material_of(model, e)
                    .map_or(([0.82, 0.80, 0.76], PLAIN_SURFACE), |(_, m)| {
                        (m.color, m.surface())
                    });
                out.push(SceneItem {
                    id: e.id,
                    mesh: structure,
                    color: c,
                    surface,
                    ghost: false,
                    layer: None,
                    finish: None,
                });
                for (i, (mesh, material)) in finishes.into_iter().enumerate() {
                    let look = material.and_then(|m| match &model.get(m)?.kind {
                        ElementKind::Material(m) => Some((m.color, m.surface())),
                        _ => None,
                    });
                    let (color, surface) = look.unwrap_or(([0.92, 0.91, 0.88], PLAIN_SURFACE));
                    out.push(SceneItem {
                        id: e.id,
                        mesh,
                        color,
                        surface,
                        ghost: false,
                        layer: Some(LayerPart {
                            index: i + 1,
                            material,
                            function: creabim_core::LayerFunction::Finish,
                            core: false,
                            priority: creabim_core::joins::layer_priority(
                                creabim_core::LayerFunction::Finish,
                                None,
                            ),
                        }),
                        finish: None,
                    });
                }
                // where the head comes too close to what is above: a red,
                // see-through stand-in over that stretch
                let need = creabim_core::stairwell::need(model, st);
                let head = creabim_core::stairwell::headroom(model, st, need);
                if !head.low.is_empty() {
                    out.push(SceneItem {
                        id: e.id,
                        mesh: crate::members::headroom_envelope(&head.low, st.width / 2.0, need),
                        color: [0.86, 0.15, 0.18],
                        surface: PLAIN_SURFACE,
                        ghost: true,
                        layer: None,
                        finish: Some("Headroom"),
                    });
                }
                continue;
            }
            ElementKind::Railing(r) => {
                // its metal in its material, its glass as glass
                let (metal, glass) = crate::members::railing_parts(model, r);
                let (color, surface) = creabim_core::material_of(model, e)
                    .map_or(([0.32, 0.34, 0.38], PLAIN_SURFACE), |(_, m)| {
                        (m.color, m.surface())
                    });
                out.push(SceneItem {
                    id: e.id,
                    mesh: metal,
                    color,
                    surface,
                    ghost: false,
                    layer: None,
                    finish: None,
                });
                if !glass.positions.is_empty() {
                    out.push(SceneItem {
                        id: e.id,
                        mesh: glass,
                        color: GLASS_COLOR,
                        surface: GLASS_SURFACE,
                        ghost: false,
                        layer: None,
                        finish: Some("Glass"),
                    });
                }
                continue;
            }
            ElementKind::Column(_)
            | ElementKind::Beam(_)
            | ElementKind::Roof(_)
            | ElementKind::Footing(_) => match crate::members::member_mesh(model, e) {
                Some(m) => m,
                None => continue,
            },
        };
        // a material's look wins over the element type's default
        let (color, surface) = creabim_core::material_of(model, e)
            .map_or((color, PLAIN_SURFACE), |(_, m)| (m.color, m.surface()));
        out.push(SceneItem {
            id: e.id,
            mesh,
            color,
            surface,
            // a shape that is a void or a guide: seen and picked in 3D,
            // left out of drawings and exports, as a cutter is (MU43)
            ghost: matches!(&e.kind, ElementKind::FreeMesh(f) if !f.role.is_solid()),
            layer: None,
            finish: None,
        });
    }
    apply_cuts(model, &mut out);
    out
}

/// The scene as drawings show it: cutters left out, and a pad's skin.
pub fn drawn_scene(model: &Model) -> Vec<SceneItem> {
    shared_scene(model)
        .iter()
        .filter(|i| !i.ghost && !i.is_lining() && i.finish != Some(PAD_SKIN))
        .cloned()
        .collect()
}

/// How many scenes [`shared_scene`] keeps: the model as it is, and a few
/// others (a view's filtered copy, the state before an edit).
const SHARED_SCENES: usize = 4;

static SHARED: std::sync::Mutex<Vec<(u64, std::sync::Arc<Vec<SceneItem>>)>> =
    std::sync::Mutex::new(Vec::new());

/// The shared scene of `model`'s revision if it is built already, without
/// waiting: `None` while it isn't, or while another thread holds the
/// cache (building one).
pub fn shared_scene_ready(model: &Model) -> Option<std::sync::Arc<Vec<SceneItem>>> {
    let rev = model.scene_revision();
    let cache = SHARED.try_lock().ok()?;
    cache
        .iter()
        .find(|(r, _)| *r == rev && rev != 0)
        .map(|(_, s)| s.clone())
}

/// [`build_scene`] built once per model revision and shared: the 3D view,
/// every section, elevation and 3D drawing of the same revision use the
/// one scene instead of each building its own. Revisions are unique
/// across models (see [`Model::revision`]); a model at revision 0 is
/// built afresh every time.
pub fn shared_scene(model: &Model) -> std::sync::Arc<Vec<SceneItem>> {
    // (by the revision of what drawings show: what is only on a sheet's
    // paper, or a view's own look, changes nothing in the scene)
    let rev = model.scene_revision();
    if rev == 0 {
        return std::sync::Arc::new(build_scene(model));
    }
    // held while building, so two threads after the same revision build it once
    let mut cache = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(i) = cache.iter().position(|(r, _)| *r == rev) {
        let hit = cache.remove(i);
        let scene = hit.1.clone();
        cache.push(hit);
        return scene;
    }
    let scene = std::sync::Arc::new(build_scene(model));
    if cache.len() >= SHARED_SCENES {
        cache.remove(0);
    }
    cache.push((rev, scene.clone()));
    scene
}

/// The shared scene of `model`'s revision made from `base`, the scene of
/// the model as it was before the edit under way, with only `changed`
/// built again: for an edit that follows the pointer and changes nothing
/// but those elements' own bodies (a shape being pulled or moved). The
/// whole scene took a worker about 50 ms a step on the starter house, so
/// the 3D view trailed the pointer; one shape is built in well under a
/// millisecond. It is kept as the revision's scene for every other user
/// (sections, elevations, the tools).
///
/// The caller answers for `changed` being all that differs from `base`,
/// and for none of them being joined to or cutting anything (see
/// [`stands_alone`]).
pub fn shared_scene_patched(
    model: &Model,
    base: &std::sync::Arc<Vec<SceneItem>>,
    changed: &std::collections::HashSet<ElementId>,
) -> std::sync::Arc<Vec<SceneItem>> {
    let rev = model.scene_revision();
    let patched = || {
        // their own bodies only: what stands alone joins nothing
        let mut fresh = build_scene_unjoined(model, Some(changed));
        fresh.retain(|i| changed.contains(&i.id));
        let mut out: Vec<SceneItem> = Vec::with_capacity(base.len() + fresh.len());
        let mut placed = false;
        for item in base.iter() {
            if changed.contains(&item.id) {
                // where the old ones were, so what is drawn keeps its order
                if !placed {
                    out.append(&mut fresh);
                    placed = true;
                }
            } else {
                out.push(item.clone());
            }
        }
        out.append(&mut fresh);
        std::sync::Arc::new(out)
    };
    if rev == 0 {
        return patched();
    }
    let mut cache = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, s)) = cache.iter().find(|(r, _)| *r == rev) {
        return s.clone();
    }
    let scene = patched();
    if cache.len() >= SHARED_SCENES {
        cache.remove(0);
    }
    cache.push((rev, scene.clone()));
    scene
}

/// Does this element's body touch no other element's: a shape or a
/// drawing with nothing cut by it or cutting it? Then it can be built
/// again alone (see [`shared_scene_patched`]).
pub fn stands_alone(model: &Model, id: ElementId) -> bool {
    model.get(id).is_some_and(|e| {
        // a piece of furniture too: nothing joins it, and a cut reaches it
        // only through a Cut, which is a dependent (MU142: a chair turned
        // in 3D rebuilt every snap of the house on letting go, 564 ms)
        matches!(
            e.kind,
            ElementKind::FreeMesh(_) | ElementKind::Sketch(_) | ElementKind::Object(_)
        ) && model.dependents(id).is_empty()
    })
}

/// One item per element, a layered element's layers in one mesh with its
/// core's look: for exports that take one shape per element.
pub fn merged(items: Vec<SceneItem>) -> Vec<SceneItem> {
    let mut out: Vec<SceneItem> = Vec::with_capacity(items.len());
    let mut at: std::collections::HashMap<ElementId, usize> = std::collections::HashMap::new();
    for item in items.into_iter().filter(|i| !i.is_lining()) {
        match at.get(&item.id) {
            Some(&i) => {
                let o = &mut out[i];
                o.mesh.append(&item.mesh);
                // the first core layer gives the look
                if item.layer.is_some_and(|l| l.core) && !o.layer.is_some_and(|l| l.core) {
                    (o.color, o.surface, o.layer) = (item.color, item.surface, item.layer);
                }
            }
            None => {
                at.insert(item.id, out.len());
                out.push(item);
            }
        }
    }
    for o in &mut out {
        o.layer = None;
    }
    out
}

/// [`build_scene`] with one item per element.
pub fn element_scene(model: &Model) -> Vec<SceneItem> {
    merged(build_scene(model))
}

/// [`drawn_scene`] with one item per element.
pub fn drawn_elements(model: &Model) -> Vec<SceneItem> {
    merged(drawn_scene(model))
}

/// Every cut in the model, in order: (target, cutter, operation).
pub fn cuts(model: &Model) -> Vec<(ElementId, ElementId, BoolOp)> {
    model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Cut(c) => Some((c.target, c.cutter, c.op)),
            _ => None,
        })
        .collect()
}

/// [`cuts`], and what each void cuts: everything that is cut in the model.
pub fn all_cuts(model: &Model) -> Vec<(ElementId, ElementId, BoolOp)> {
    let mut out = cuts(model);
    out.extend(
        creabim_core::parts::void_cuts(model)
            .into_iter()
            .map(|(t, v)| (t, v, BoolOp::Difference)),
    );
    out
}

/// Is `id` used to cut something? Cutters are hidden from drawings.
pub fn is_cutter(model: &Model, id: ElementId) -> bool {
    model
        .iter()
        .any(|e| matches!(&e.kind, ElementKind::Cut(c) if c.cutter == id))
}

/// Apply every cut to the meshes of its target, using the cutters' own
/// shapes (a cutter that is itself cut cuts with its uncut shape, so cuts
/// can never loop), and turn the cutters into ghosts.
fn apply_cuts(model: &Model, items: &mut [SceneItem]) {
    let cuts = all_cuts(model);
    if cuts.is_empty() {
        return;
    }
    // a layered cutter cuts with all its layers
    let mut original: std::collections::HashMap<ElementId, TriMesh> =
        std::collections::HashMap::new();
    for (_, c, _) in &cuts {
        if original.contains_key(c) {
            continue;
        }
        let mut tool = TriMesh::default();
        for i in items.iter().filter(|i| i.id == *c) {
            tool.append(&i.mesh);
        }
        if !tool.is_empty() {
            original.insert(*c, tool);
        }
    }
    // how far each cut reaches, up and down (wave 3)
    let reach: std::collections::HashMap<(ElementId, ElementId), creabim_core::Reach> = model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Cut(c) if !c.reach.is_as_drawn() => Some(((c.target, c.cutter), c.reach)),
            _ => None,
        })
        .collect();
    for (target, cutter, op) in &cuts {
        let Some(tool) = original.get(cutter) else {
            continue;
        };
        let reached;
        let tool = match reach.get(&(*target, *cutter)) {
            Some(r) => {
                reached = reached_tool(tool, *r);
                &reached
            }
            None => tool,
        };
        // a layered element is a mesh per layer: a cut or an intersection
        // works on each, but an addition is joined once, onto its largest
        // layer (MU71: joined onto every layer, the copies overlapped and
        // drew lines down the middle of the wall)
        if *op == BoolOp::Union {
            let biggest = items
                .iter()
                .enumerate()
                .filter(|(_, i)| i.id == *target && !i.mesh.positions.is_empty())
                .max_by(|a, b| {
                    let v = |m: &TriMesh| {
                        m.bounds().map_or(0.0, |(lo, hi)| {
                            let d = hi - lo;
                            d.x * d.y * d.z
                        })
                    };
                    v(&a.1.mesh).total_cmp(&v(&b.1.mesh))
                })
                .map(|(k, _)| k);
            if let Some(k) = biggest {
                for (j, item) in items.iter_mut().enumerate() {
                    if item.id != *target {
                        continue;
                    }
                    // the other layers make way for it, so none overlap
                    let op = if j == k {
                        BoolOp::Union
                    } else {
                        BoolOp::Difference
                    };
                    item.mesh = boolean_mesh(&item.mesh, tool, op);
                }
            }
            continue;
        }
        for item in items.iter_mut().filter(|i| i.id == *target) {
            item.mesh = boolean_mesh(&item.mesh, tool, *op);
        }
    }
    for item in items.iter_mut() {
        if original.contains_key(&item.id) {
            item.ghost = true;
        }
    }
}

/// How far "without end" runs a cutter: far past any building.
const REACH_RUN: f32 = 1000.0;

/// A cutter run on as `reach` asks: the corners at its top taken far up,
/// those at its bottom far down. A box or an upright prism stays one, as
/// tall as any building.
pub fn reached_tool(tool: &TriMesh, reach: creabim_core::Reach) -> TriMesh {
    let (up, down) = reach.runs();
    let Some((lo, hi)) = tool.bounds() else {
        return tool.clone();
    };
    let eps = 1e-4_f32.max((hi.z - lo.z) * 1e-4);
    let mut out = tool.clone();
    for v in out.positions.iter_mut() {
        if up && v[2] >= hi.z - eps {
            v[2] = hi.z + REACH_RUN;
        } else if down && v[2] <= lo.z + eps {
            v[2] = lo.z - REACH_RUN;
        }
    }
    out
}

/// Where a wall and a slab overlap in 3D, the one the drawings let run
/// through cuts the other (A4-6), so 3D, sections and plans agree: by the
/// junction's own rule if it has one, else layer by layer (the lower join
/// priority runs through: cores through finishes), then walls through
/// slabs. As strong, they are left as they are. Each result is kept, keyed
/// by the two shapes, so editing one wall redoes only its own joins.
/// The most triangles a join cut may leave one piece with (see
/// [`apply_joins`]): far past any real wall or slab.
const MAX_JOIN_TRIS: usize = 60_000;

#[cfg(test)]
thread_local! {
    /// A lower limit than [`MAX_JOIN_TRIS`], for tests to have a cut refused.
    static TEST_CUT_LIMIT: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    /// The join cuts made on this thread, for tests to count.
    static CUTS_MADE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// A smaller budget than [`JOIN_BUDGET`], for tests to see it kept.
    static TEST_JOIN_BUDGET: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// The most triangles a join cut may leave its piece with.
fn join_tris_limit() -> usize {
    #[cfg(test)]
    if let Some(l) = TEST_CUT_LIMIT.with(|l| l.get()) {
        return l;
    }
    MAX_JOIN_TRIS
}

/// Returns how many pieces took their cuts along (see `carry`).
fn apply_joins(model: &Model, items: &mut [SceneItem], carry: bool) -> usize {
    let rules = creabim_core::joins::rules(model);
    let is = |id: ElementId, wall: bool| match model.get(id).map(|e| &e.kind) {
        Some(ElementKind::Wall(w)) => wall && w.curtain.is_none(),
        Some(ElementKind::Slab(_)) => !wall,
        _ => false,
    };
    // a slab's earth fill (gravel, soil: a material of category "earth")
    // gives way to whatever a wall brings into it: a foundation's inside
    // insulation runs up through the gravel under a slab on grade and meets
    // the slab's insulation, where it stopped under the gravel and left a
    // gap the cold went through (MU144, owner: "shouldn't insulation meet
    // insulation?"). Weaker than any layer, so no other join changes
    const EARTH_FILL: u8 = 5;
    let earth = |i: &SceneItem| {
        is(i.id, false)
            && i.layer.and_then(|l| l.material).is_some_and(|m| {
                matches!(model.get(m).map(|e| &e.kind),
                    Some(ElementKind::Material(mat)) if mat.category == "earth")
            })
    };
    let strength = |i: &SceneItem| {
        let p = i.layer.map_or(1, |l| l.priority);
        (
            std::cmp::Reverse(if earth(i) { p.max(EARTH_FILL) } else { p }),
            model
                .get(i.id)
                .map_or(0, |e| creabim_core::joins::category_rank(&e.kind)),
        )
    };
    let boxes: Vec<Option<(glam::Vec3, glam::Vec3)>> = items
        .iter()
        .map(|i| if i.ghost { None } else { i.mesh.bounds() })
        .collect();
    let overlap = |a: (glam::Vec3, glam::Vec3), b: (glam::Vec3, glam::Vec3)| {
        let lo = a.0.max(b.0);
        let hi = a.1.min(b.1);
        (hi - lo).min_element() > 1e-4
    };
    let walls: Vec<usize> = (0..items.len())
        .filter(|k| boxes[*k].is_some() && is(items[*k].id, true))
        .collect();
    let slabs: Vec<usize> = (0..items.len())
        .filter(|k| boxes[*k].is_some() && is(items[*k].id, false))
        .collect();
    // (target, cutter) pairs, in item indices
    let mut pairs: Vec<(usize, usize)> = vec![];
    // a wall's layer reaches a slab's layer only through the slab's layers
    // on its way there: one that comes up into the slab from below and is
    // stopped by its base or its core does not break through its finish
    // on top, which runs over it instead (MU141, owner: "the insulation is
    // pushing away the floor finish?": a foundation wall's inside
    // insulation, cut away by the ground floor's base and core, still cut
    // a sliver out of its finish at the wall)
    let reaches = |w: usize, s: usize| -> bool {
        let (Some(bw), Some(bs)) = (boxes[w], boxes[s]) else {
            return true;
        };
        const EPS: f32 = 1e-4;
        let (below, above) = (bw.0.z < bs.0.z - EPS, bw.1.z > bs.1.z + EPS);
        if !below && !above {
            return true;
        }
        // every layer of the slab lying between `lo` and `hi` that the
        // wall's layer meets, it beats
        let clear = |lo: f32, hi: f32| {
            slabs.iter().all(|&k| {
                if k == s || items[k].id != items[s].id {
                    return true;
                }
                let Some(bk) = boxes[k] else { return true };
                let between = bk.0.z >= lo - EPS && bk.1.z <= hi + EPS;
                !(between && overlap(bw, bk)) || strength(&items[w]) > strength(&items[k])
            })
        };
        (below && clear(bw.0.z, bs.0.z)) || (above && clear(bs.1.z, bw.1.z))
    };
    for &w in &walls {
        for &sl in &slabs {
            let (Some(bw), Some(bs)) = (boxes[w], boxes[sl]) else {
                continue;
            };
            if !overlap(bw, bs) {
                continue;
            }
            let (a, b) = (&items[w], &items[sl]);
            let rule = rules.iter().find(|r| r.joins(a.id, b.id));
            if rule.is_some_and(|r| !r.cleanup) {
                continue;
            }
            let wall_wins = match rule.and_then(|r| r.wins(a.id, b.id)) {
                Some(w) => w,
                None => {
                    let (sa, sb) = (strength(a), strength(b));
                    if sa == sb {
                        continue;
                    }
                    // by the layers' priorities: only where it can get to
                    sa > sb && reaches(w, sl)
                }
            };
            pairs.push(if wall_wins { (sl, w) } else { (w, sl) });
        }
    }
    // a column runs through the slabs it passes: each stops at its faces
    // (MU143, owner, the starter house's balcony: "find out why there is
    // an issue here": the loggia's column rose through the balcony, whose
    // edge, flush with the column's faces, showed its layers on the
    // column). A rule can still unjoin the two, or let the slab through
    let columns: Vec<usize> = (0..items.len())
        .filter(|k| {
            boxes[*k].is_some()
                && matches!(
                    model.get(items[*k].id).map(|e| &e.kind),
                    Some(ElementKind::Column(_))
                )
        })
        .collect();
    for &c in &columns {
        for &sl in &slabs {
            let (Some(bc), Some(bs)) = (boxes[c], boxes[sl]) else {
                continue;
            };
            if !overlap(bc, bs) {
                continue;
            }
            let (a, b) = (&items[c], &items[sl]);
            let rule = rules.iter().find(|r| r.joins(a.id, b.id));
            if rule.is_some_and(|r| !r.cleanup) {
                continue;
            }
            let column_wins = rule.and_then(|r| r.wins(a.id, b.id)).unwrap_or(true);
            pairs.push(if column_wins { (sl, c) } else { (c, sl) });
        }
    }
    // a ceiling is a finish: every wall it reaches cuts it, so it stops at
    // the wall's face (MU139: a ceiling's board and a porch soffit ran on
    // through the timber wall). A rule can still unjoin the two, or let
    // the ceiling run through
    let ceilings: Vec<usize> = (0..items.len())
        .filter(|k| {
            boxes[*k].is_some()
                && matches!(
                    model.get(items[*k].id).map(|e| &e.kind),
                    Some(ElementKind::Ceiling(_))
                )
        })
        .collect();
    for &c in &ceilings {
        for &w in &walls {
            let (Some(bc), Some(bw)) = (boxes[c], boxes[w]) else {
                continue;
            };
            if !overlap(bc, bw) {
                continue;
            }
            let (a, b) = (&items[w], &items[c]);
            let rule = rules.iter().find(|r| r.joins(a.id, b.id));
            if rule.is_some_and(|r| !r.cleanup) {
                continue;
            }
            let wall_wins = rule.and_then(|r| r.wins(a.id, b.id)).unwrap_or(true);
            pairs.push(if wall_wins { (c, w) } else { (w, c) });
        }
    }
    // a pad is the ground made level for the building: what is built on
    // it and dug down from it (footings, stem walls, columns, a slab sunk
    // into it) passes through it, so every one it reaches cuts it, as the
    // ground gives way to them (MU140: the house pad ran through the slab
    // and the stem walls)
    let pads: Vec<usize> = (0..items.len())
        .filter(|k| {
            boxes[*k].is_some()
                && matches!(
                    model.get(items[*k].id).map(|e| &e.kind),
                    Some(ElementKind::Site(s)) if matches!(s.form, creabim_core::SiteForm::Pad { .. })
                )
        })
        .collect();
    if !pads.is_empty() {
        let built: Vec<usize> = (0..items.len())
            .filter(|k| {
                boxes[*k].is_some()
                    && match model.get(items[*k].id).map(|e| &e.kind) {
                        Some(ElementKind::Wall(w)) => w.curtain.is_none(),
                        Some(
                            ElementKind::Slab(_) | ElementKind::Footing(_) | ElementKind::Column(_),
                        ) => true,
                        _ => false,
                    }
            })
            .collect();
        for &p in &pads {
            for &b in &built {
                let (Some(bp), Some(bb)) = (boxes[p], boxes[b]) else {
                    continue;
                };
                if overlap(bp, bb) {
                    pairs.push((p, b));
                }
            }
        }
    }
    // curved walls crossing a wall along its length (not meeting it at an
    // end, which the end joins see to): the stronger runs through, the
    // other is cut by it, as plans draw them (MU42: crossing circles)
    let wall_of = |id: ElementId| match model.get(id).map(|e| &e.kind) {
        Some(ElementKind::Wall(w)) if w.curtain.is_none() => Some(w),
        _ => None,
    };
    for (k, &a) in walls.iter().enumerate() {
        for &b in &walls[k + 1..] {
            let (ia, ib) = (items[a].id, items[b].id);
            if ia == ib {
                continue;
            }
            let (Some(wa), Some(wb)) = (wall_of(ia), wall_of(ib)) else {
                continue;
            };
            if !(wa.is_arc() || wb.is_arc()) || wa.level != wb.level {
                continue;
            }
            let (Some(ba), Some(bb)) = (boxes[a], boxes[b]) else {
                continue;
            };
            if !overlap(ba, bb) {
                continue;
            }
            // met at an end: the end joins have done it
            let reach = (wa.thickness + wb.thickness) / 2.0 + 0.01;
            let ends_meet = [wa.start, wa.end]
                .iter()
                .any(|p| [wb.start, wb.end].iter().any(|q| p.distance(*q) < reach));
            if ends_meet {
                continue;
            }
            let rule = rules.iter().find(|r| r.joins(ia, ib));
            if rule.is_some_and(|r| !r.cleanup) {
                continue;
            }
            let a_wins = match rule.and_then(|r| r.wins(ia, ib)) {
                Some(w) => w,
                None => {
                    let (sa, sb) = (strength(&items[a]), strength(&items[b]));
                    if sa != sb {
                        sa > sb
                    } else if (wa.thickness - wb.thickness).abs() > 1e-6 {
                        wa.thickness > wb.thickness
                    } else {
                        // as strong: the one drawn first runs through
                        true
                    }
                }
            };
            pairs.push(if a_wins { (b, a) } else { (a, b) });
        }
    }
    if pairs.is_empty() {
        return 0;
    }
    // this build, for the caches to tell what is still in use
    let now = JOIN_BUILD.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    // the cutters' shapes before any join cuts them (only theirs: a copy
    // of every mesh was megabytes with heavy masses in the model)
    let originals: std::collections::HashMap<usize, TriMesh> = pairs
        .iter()
        .map(|(_, c)| *c)
        .collect::<std::collections::HashSet<usize>>()
        .into_iter()
        .map(|c| (c, items[c].mesh.clone()))
        .collect();
    // each target with what cuts it, in order (a target's cuts depend on
    // no other target's: cutters cut with the shape they had before)
    let mut chains: Vec<(usize, Vec<usize>)> = vec![];
    let mut chain_of: std::collections::HashMap<usize, usize> = Default::default();
    for (t, c) in pairs {
        let k = *chain_of.entry(t).or_insert_with(|| {
            chains.push((t, vec![]));
            chains.len() - 1
        });
        chains[k].1.push(c);
    }
    let mut shapes: std::collections::HashMap<usize, (u64, DVec3)> = Default::default();
    let mut taken = 0;
    for (t, cutters) in chains {
        // the target with all its cuts, kept by how the shapes stand to
        // each other and not by where: while a move is live ([`carry_cuts`])
        // a building that moved as one, walls and slabs together, takes
        // its finished cuts along instead of making each again (150 ms to
        // over a second for a house, on every step of the move). Only a
        // finished target is carried, never a cut made on a carried
        // shape: a shape set down elsewhere differs in the last bits of
        // its single-precision corners, and a cut between one carried and
        // one made afresh met faces a hair apart and split without end
        let shape = shapes
            .entry(t)
            .or_insert_with(|| shape_key(&items[t].mesh))
            .to_owned();
        let (target_shape, at) = shape;
        let key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            target_shape.hash(&mut h);
            for c in &cutters {
                let (cs, c_at) = shapes
                    .entry(*c)
                    .or_insert_with(|| shape_key(&originals[c]))
                    .to_owned();
                cs.hash(&mut h);
                let d = (c_at - at) * 1.0e4;
                (d.x.round() as i64, d.y.round() as i64, d.z.round() as i64).hash(&mut h);
            }
            h.finish()
        };
        if carry {
            let kept = carried().get(&key, now);
            if let Some((was, mesh)) = kept {
                if was != at {
                    let by = at - was;
                    let mut mesh = (*mesh).clone();
                    for q in &mut mesh.positions {
                        *q = (DVec3::from(glam::Vec3::from(*q)) + by)
                            .as_vec3()
                            .to_array();
                    }
                    items[t].mesh = mesh;
                    CARRIED_USED.store(true, std::sync::atomic::Ordering::Relaxed);
                    taken += 1;
                    continue;
                }
            }
        }
        // the target's polygons are kept from cut to cut, planes and all,
        // and made triangles once, after the last (bruwy, #85): made
        // triangles and rounded to single precision between cuts, each cut
        // fed the next one more split planes, and a slab crossing twelve
        // walls came out at 3 800 triangles instead of 850
        let mut kept: Option<JoinedPolygons> = None;
        let mut target_key = mesh_key(&items[t].mesh);
        for c in cutters {
            // nothing left to cut: a later cut leaves it empty
            if kept.as_ref().is_some_and(|p| p.is_empty()) {
                break;
            }
            let key = (target_key, mesh_key(&originals[&c]));
            let done = join_cache().get(&key, now);
            let (cut, refused) = match done {
                Some(done) => done,
                None => {
                    #[cfg(test)]
                    CUTS_MADE.with(|n| n.set(n.get() + 1));
                    let target = kept
                        .as_ref()
                        .map_or_else(|| mesh_polygons(&items[t].mesh), |p| p.as_ref().clone());
                    let cutter = mesh_polygons(&originals[&c]);
                    let (a, b) = (polygon_tris(&target), polygon_tris(&cutter));
                    let result = csg::boolean(target, cutter, BoolOp::Difference);
                    // the cut splits faces along whole planes, and cut after cut
                    // those pieces can multiply: a slab that took a wall's
                    // corner went from 5 000 triangles to three quarters of a
                    // million, and the app froze. A cut that multiplies far
                    // past its inputs is left out: the wall's piece stays in
                    // the slab, hidden in it, rather than the app running out
                    let tris = polygon_tris(&result);
                    let refused = tris > 3 * (a + b) + 600 || tris > join_tris_limit();
                    let result = std::sync::Arc::new(if refused { Vec::new() } else { result });
                    let bytes = polygons_bytes(&result);
                    join_cache().insert(key, (result.clone(), refused), bytes, now, join_budget());
                    (result, refused)
                }
            };
            // a refused cut leaves the target as it was, and one cut that
            // multiplies it says the others will too: they are not made
            // (Highland Haven: 48 refused cuts, each made in full and
            // thrown away, were 13 s of the 18 s its 3D scene took)
            if refused {
                break;
            }
            target_key = {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                key.hash(&mut h);
                h.finish()
            };
            kept = Some(cut);
        }
        if let Some(p) = kept {
            items[t].mesh = polygon_mesh(&p);
        }
        // mended before it is carried, so a piece carried along is the
        // piece at rest corner for corner (see [`mend_built`])
        if let Some(m) = mend_kept(&items[t].mesh, now) {
            items[t].mesh = (*m).clone();
        }
        let mesh = std::sync::Arc::new(items[t].mesh.clone());
        let bytes = mesh_bytes(&mesh);
        carried().insert(key, (at, mesh), bytes, now, join_budget() / 2);
    }
    taken
}

/// The memory a mesh's corners and triangles take.
fn mesh_bytes(m: &TriMesh) -> usize {
    std::mem::size_of_val(m.positions.as_slice())
        + std::mem::size_of_val(m.normals.as_slice())
        + std::mem::size_of_val(m.indices.as_slice())
}

/// One build of a scene's joins after another, across every thread: what
/// a cache last served is told by it.
static JOIN_BUILD: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How much the cuts kept may take: past it, those not used by the last
/// few builds go. A drag made some 30 cuts a step, each kept, and the
/// cache let go of them only at 4 000 (65 MB on the Ridgeline; the app's
/// memory grew all session, MU142).
const JOIN_BUDGET: usize = 16 << 20;

fn join_budget() -> usize {
    #[cfg(test)]
    if let Some(b) = TEST_JOIN_BUDGET.with(|b| b.get()) {
        return b;
    }
    JOIN_BUDGET
}

/// Values kept by key, each with the build that last used it and its
/// size: over its budget, what the last few builds didn't use goes, and
/// what they use stays however large.
pub(crate) struct Memo<K, V> {
    map: std::collections::HashMap<K, (V, u64, usize)>,
    bytes: usize,
}

impl<K, V> Default for Memo<K, V> {
    fn default() -> Self {
        Self {
            map: Default::default(),
            bytes: 0,
        }
    }
}

impl<K: std::hash::Hash + Eq, V: Clone> Memo<K, V> {
    /// How many builds back a value counts as in use: scenes and drawings
    /// are built side by side on threads of their own.
    const RECENT: u64 = 4;

    pub(crate) fn get(&mut self, k: &K, now: u64) -> Option<V> {
        let e = self.map.get_mut(k)?;
        e.1 = e.1.max(now);
        Some(e.0.clone())
    }

    pub(crate) fn insert(&mut self, k: K, v: V, bytes: usize, now: u64, budget: usize) {
        if let Some((_, _, was)) = self.map.insert(k, (v, now, bytes)) {
            self.bytes -= was;
        }
        self.bytes += bytes;
        if self.bytes > budget {
            self.map.retain(|_, e| e.1 + Self::RECENT >= now);
            self.bytes = self.map.values().map(|e| e.2).sum();
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }

    #[cfg(test)]
    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }

    #[cfg(test)]
    pub(crate) fn clear(&mut self) {
        self.map.clear();
        self.bytes = 0;
    }
}

/// While a move is live, a building that moves as one takes its finished
/// wall and slab cuts along ([`apply_joins`]). The app turns this on when
/// a move starts and off when it ends. Returns whether, on turning it
/// off, any carried cut was served since it was turned on: the scene and
/// the drawings made meanwhile are then to be made again, so that what
/// stays after the drop is cut where it stands, exactly as without this.
pub fn carry_cuts(on: bool) -> bool {
    use std::sync::atomic::Ordering::Relaxed;
    let was = CARRY.swap(on, Relaxed);
    if was && !on {
        return CARRIED_USED.swap(false, Relaxed);
    }
    false
}

static CARRY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static CARRIED_USED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A target with all its cuts, and where its first corner stood.
type Carried = Memo<u64, (DVec3, std::sync::Arc<TriMesh>)>;

static CARRIED: std::sync::LazyLock<std::sync::Mutex<Carried>> =
    std::sync::LazyLock::new(Default::default);

fn carried() -> std::sync::MutexGuard<'static, Carried> {
    CARRIED.lock().unwrap_or_else(|e| e.into_inner())
}

/// A fingerprint of a mesh's shape as it stands from its first corner,
/// to a tenth of a millimetre, with that corner: the same for the mesh
/// set down anywhere. (A mesh's corners are single precision, so the same
/// shape elsewhere differs in its last bits, microns at 50 m; a tenth of
/// a millimetre is far above that and far below anything built.)
fn shape_key(m: &TriMesh) -> (u64, DVec3) {
    use std::hash::{Hash, Hasher};
    let at = m
        .positions
        .first()
        .map_or(DVec3::ZERO, |p| DVec3::from(glam::Vec3::from(*p)));
    let mut h = std::collections::hash_map::DefaultHasher::new();
    m.indices.hash(&mut h);
    for p in &m.positions {
        let d = (DVec3::from(glam::Vec3::from(*p)) - at) * 1.0e4;
        (d.x.round() as i64, d.y.round() as i64, d.z.round() as i64).hash(&mut h);
    }
    (h.finish(), at)
}

/// A target's polygons after its cuts so far, before they are made triangles.
type JoinedPolygons = std::sync::Arc<Vec<csg::Polygon>>;

/// A join's cut, kept by the target so far (its mesh, or the cuts it has
/// taken) and the cutter's mesh, as polygons. A refused cut is kept too,
/// as refused, so it is not made again.
type JoinCuts = Memo<(u64, u64), (JoinedPolygons, bool)>;

/// The triangles a set of polygons makes, each a fan.
fn polygon_tris(p: &[csg::Polygon]) -> usize {
    p.iter().map(|q| q.points.len().saturating_sub(2)).sum()
}

/// The memory a set of polygons takes, as the join cache counts it.
fn polygons_bytes(p: &[csg::Polygon]) -> usize {
    p.iter()
        .map(|q| std::mem::size_of::<csg::Polygon>() + std::mem::size_of_val(q.points.as_slice()))
        .sum()
}

/// The cuts made. One cache for every thread: each scene is built on a
/// thread of its own, and a cache kept by the thread was empty for every
/// one of them, so each scene made every cut of the building again
/// (100 ms and more) whatever had changed.
static JOIN_CACHE: std::sync::LazyLock<std::sync::Mutex<JoinCuts>> =
    std::sync::LazyLock::new(Default::default);

fn join_cache() -> std::sync::MutexGuard<'static, JoinCuts> {
    JOIN_CACHE.lock().unwrap_or_else(|e| e.into_inner())
}

/// A fingerprint of a mesh's shape.
fn mesh_key(m: &TriMesh) -> u64 {
    m.fingerprint()
}

/// `a op b` for two closed triangle meshes.
pub fn boolean_mesh(a: &TriMesh, b: &TriMesh, op: BoolOp) -> TriMesh {
    polygon_mesh(&csg::boolean(mesh_polygons(a), mesh_polygons(b), op))
}

/// A closed triangle mesh as the BSP's polygons, one a triangle.
fn mesh_polygons(m: &TriMesh) -> Vec<csg::Polygon> {
    m.indices
        .chunks_exact(3)
        .filter_map(|t| {
            let p = |i: u32| {
                let v = m.positions[i as usize];
                DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64)
            };
            csg::Polygon::new(vec![p(t[0]), p(t[1]), p(t[2])])
        })
        .collect()
}

/// The BSP's polygons made a triangle mesh, each a fan.
fn polygon_mesh(result: &[csg::Polygon]) -> TriMesh {
    let mut out = TriMesh::default();
    for p in result {
        let n = (p.points[1] - p.points[0])
            .cross(p.points[2] - p.points[0])
            .normalize_or_zero();
        let fan: Vec<[usize; 3]> = (1..p.points.len() - 1).map(|i| [0, i, i + 1]).collect();
        // the fan's winding already follows the polygon's
        out.add_polygon(&p.points, &fan, newell(&p.points).normalize_or(n));
    }
    out
}

fn newell(p: &[DVec3]) -> DVec3 {
    let mut n = DVec3::ZERO;
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        n += DVec3::new(
            (a.y - b.y) * (a.z + b.z),
            (a.z - b.z) * (a.x + b.x),
            (a.x - b.x) * (a.y + b.y),
        );
    }
    n
}

/// A door's or window's parts, each in its own colour: the frame and leaf
/// in the opening's material (or a painted default), glass see-through.
/// An element can so have several items, all with its id.
fn opening_items(
    model: &Model,
    e: &creabim_core::Element,
    o: &Opening,
    w: &creabim_core::Wall,
    level_z: f64,
) -> Vec<SceneItem> {
    part_items(model, e, o, crate::opening::opening_meshes(o, w, level_z))
}

/// A roof window's parts, each in its own colour, as a window's in a wall.
fn roof_window_items(model: &Model, e: &creabim_core::Element, o: &Opening) -> Vec<SceneItem> {
    part_items(model, e, o, crate::opening::roof_window_meshes(model, o))
}

/// A door's or window's part meshes as scene items, each in its colour.
fn part_items(
    model: &Model,
    e: &creabim_core::Element,
    o: &Opening,
    meshes: Vec<(crate::opening::Part, TriMesh)>,
) -> Vec<SceneItem> {
    use crate::opening::Part;
    use creabim_core::OpeningKind;
    let material = creabim_core::material_of(model, e).map(|(_, m)| (m.color, m.surface()));
    meshes
        .into_iter()
        .map(|(part, mesh)| {
            let steel =
                o.frame.construction.leaf == creabim_core::construction::LeafStyle::SlimSteel;
            let (color, surface) = match (part, o.kind, material) {
                (Part::Glass, ..) => (GLASS_COLOR, GLASS_SURFACE),
                (Part::Handle, ..) => ([0.72, 0.73, 0.75], [0.3, 1.0, 1.0]),
                (_, _, Some(m)) => m,
                // slim steel: its frame and leaves black
                (Part::Frame | Part::Leaf, _, None) if steel => STEEL_BLACK,
                (Part::Frame, OpeningKind::Window | OpeningKind::Hole, None) => {
                    ([0.93, 0.93, 0.91], PLAIN_SURFACE)
                }
                (Part::Frame, OpeningKind::Door, None) => ([0.50, 0.35, 0.22], PLAIN_SURFACE),
                (Part::Leaf, _, None) => ([0.62, 0.45, 0.30], PLAIN_SURFACE),
                // painted trim
                (Part::Trim, ..) => ([0.93, 0.92, 0.89], PLAIN_SURFACE),
                // a roof window's flashing: dark grey metal; its insulation
                // collar: pale, as insulation is drawn
                (Part::Flashing, ..) => ([0.30, 0.31, 0.33], [0.4, 0.7, 1.0]),
                (Part::Insulation, ..) => ([0.94, 0.90, 0.70], PLAIN_SURFACE),
            };
            SceneItem {
                id: e.id,
                mesh,
                color,
                surface,
                ghost: false,
                layer: None,
                finish: match part {
                    Part::Glass => Some("Glass"),
                    Part::Handle => Some("Metal"),
                    Part::Trim => Some(TRIM),
                    Part::Flashing => Some("Metal"),
                    Part::Frame | Part::Leaf | Part::Insulation => None,
                },
            }
        })
        .collect()
}

/// Slim steel doors' frames and leaves: black, a little sheen.
pub const STEEL_BLACK: ([f32; 3], [f32; 3]) = ([0.11, 0.11, 0.12], [0.45, 0.6, 1.0]);

/// Glass in doors and windows: pale blue, smooth and mostly see-through.
pub const GLASS_COLOR: [f32; 3] = [0.62, 0.78, 0.88];
pub const GLASS_SURFACE: [f32; 3] = [0.05, 0.0, 0.3];

/// Each element's whole mesh: the items of one element (a window's frame
/// and glass) merged.
pub fn element_meshes(scene: &[SceneItem]) -> std::collections::HashMap<ElementId, TriMesh> {
    let mut out: std::collections::HashMap<ElementId, TriMesh> = Default::default();
    for i in scene.iter().filter(|i| !i.is_lining()) {
        out.entry(i.id).or_default().append(&i.mesh);
    }
    out
}

/// A curtain wall's mullions and panels, each kind in its own look: the
/// mullions and solid panels in the wall's material (or anodised
/// aluminium and a pale grey), glass see-through.
fn curtain_items(model: &Model, e: &Element, w: &creabim_core::Wall) -> Vec<SceneItem> {
    use crate::curtain::CurtainPart;
    let (Some(level), Some(grid)) = (model.level(w.level), &w.curtain) else {
        return vec![];
    };
    let openings: Vec<&Opening> = model
        .dependents(e.id)
        .into_iter()
        .filter_map(|d| match &model.get(d)?.kind {
            ElementKind::Opening(o) => Some(o),
            _ => None,
        })
        .collect();
    let material = creabim_core::material_of(model, e).map(|(_, m)| (m.color, m.surface()));
    let at = crate::curtain::setting(model, w);
    crate::curtain::curtain_meshes_under(w, grid, &openings, level.elevation, &at)
        .into_iter()
        .map(|(part, mesh)| {
            let (color, surface) = match (part, material) {
                (CurtainPart::Glass, _) => (GLASS_COLOR, GLASS_SURFACE),
                // the sealed joint between flush panes reads dark
                (CurtainPart::Seal, _) => ([0.13, 0.14, 0.15], PLAIN_SURFACE),
                (_, Some(m)) => m,
                (CurtainPart::Mullion, None) => ([0.62, 0.64, 0.67], [0.35, 0.9, 1.0]),
                (CurtainPart::Panel, None) => ([0.80, 0.80, 0.78], PLAIN_SURFACE),
            };
            SceneItem {
                id: e.id,
                mesh,
                color,
                surface,
                ghost: false,
                layer: None,
                finish: match part {
                    CurtainPart::Glass => Some("Glass"),
                    CurtainPart::Mullion => Some("Metal"),
                    CurtainPart::Panel | CurtainPart::Seal => None,
                },
            }
        })
        .collect()
}

/// An extruded slab with holes through it: top face at `top_z`,
/// `thickness` downwards, each hole (a polygon inside the outline) cut
/// straight through.
pub fn slab_mesh_with_holes(
    boundary: &[DVec2],
    holes: &[Vec<DVec2>],
    top_z: f64,
    thickness: f64,
) -> TriMesh {
    if holes.is_empty() {
        return slab_mesh(boundary, top_z, thickness);
    }
    let face = creabim_core::polygon::bridged(boundary, holes);
    let mut m = TriMesh::default();
    let tris = triangulate(&face);
    let top: Vec<DVec3> = face.iter().map(|p| p.extend(top_z)).collect();
    let bottom: Vec<DVec3> = face.iter().map(|p| p.extend(top_z - thickness)).collect();
    m.add_polygon(&top, &tris, DVec3::Z);
    m.add_polygon(&bottom, &tris, -DVec3::Z);
    // the outline counter-clockwise and the holes clockwise: material is
    // on the left of every edge, so the sides face right
    let mut loops = vec![boundary.to_vec()];
    loops.extend(holes.iter().cloned());
    for (k, lp) in loops.iter_mut().enumerate() {
        let ccw = crate::triangulate::signed_area2(lp) > 0.0;
        if ccw != (k == 0) {
            lp.reverse();
        }
        let n = lp.len();
        for i in 0..n {
            let (a, b) = (lp[i], lp[(i + 1) % n]);
            let e = (b - a).normalize_or_zero();
            m.add_quad(
                [
                    a.extend(top_z - thickness),
                    b.extend(top_z - thickness),
                    b.extend(top_z),
                    a.extend(top_z),
                ],
                DVec3::new(e.y, -e.x, 0.0),
            );
        }
    }
    m
}

/// Extruded slab: top face at `top_z`, `thickness` downwards.
pub fn slab_mesh(boundary: &[DVec2], top_z: f64, thickness: f64) -> TriMesh {
    let mut m = TriMesh::default();
    let tris = triangulate(boundary);
    let top: Vec<DVec3> = boundary.iter().map(|p| p.extend(top_z)).collect();
    let bottom: Vec<DVec3> = boundary
        .iter()
        .map(|p| p.extend(top_z - thickness))
        .collect();
    m.add_polygon(&top, &tris, DVec3::Z);
    m.add_polygon(&bottom, &tris, -DVec3::Z);
    let ccw = crate::triangulate::signed_area2(boundary) > 0.0;
    for i in 0..boundary.len() {
        let (a, b) = (boundary[i], boundary[(i + 1) % boundary.len()]);
        let e = (b - a).normalize_or_zero();
        let out = if ccw {
            DVec2::new(e.y, -e.x)
        } else {
            DVec2::new(-e.y, e.x)
        };
        m.add_quad(
            [
                bottom[i],
                bottom[(i + 1) % boundary.len()],
                top[(i + 1) % boundary.len()],
                top[i],
            ],
            out.extend(0.0),
        );
    }
    m
}

/// Flat-shaded triangles from a polygon mesh, offset to `origin`, face by
/// face (so triangle k still maps back to its face).
///
/// Coincident corners are welded: corners closer than a micron become one
/// position, and faces lying in one plane share the vertex itself. A
/// corner on a crease is still one vertex per face direction, so every
/// face keeps its own flat normal, but they all sit at exactly the same
/// point, so the mesh is closed wherever the edit mesh is (see
/// [`TriMesh::is_closed`]).
///
/// The welding of a large mesh (a mass with panels or louvres: thousands
/// of faces) is kept by the mesh's content, and only its corners are
/// placed again: a tower that moved, or stood still while something else
/// changed, cost 5 ms in every scene, and fifty of them a quarter second.
pub fn edit_mesh_to_tri(mesh: &creabim_core::EditMesh, origin: DVec3) -> TriMesh {
    use std::sync::{Arc, Mutex};
    /// Smaller meshes weld faster than their content hashes and copies.
    const KEEP_FROM_FACES: usize = 200;
    const KEPT: usize = 128;
    if mesh.faces.len() < KEEP_FROM_FACES {
        return weld(mesh, origin).0;
    }
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for v in &mesh.vertices {
        v.to_array().map(f64::to_bits).hash(&mut h);
    }
    mesh.faces.hash(&mut h);
    let key = h.finish();
    type Kept = Vec<(u64, Arc<(TriMesh, Vec<u32>)>)>;
    static WELDED: Mutex<Kept> = Mutex::new(Vec::new());
    let hit = WELDED.lock().ok().and_then(|mut c| {
        let i = c.iter().position(|(k, _)| *k == key)?;
        // the last used last, so the oldest goes first
        let e = c.remove(i);
        let w = e.1.clone();
        c.push(e);
        Some(w)
    });
    if let Some(w) = hit {
        let (m, from) = &*w;
        let placed = TriMesh {
            positions: from
                .iter()
                .map(|c| (mesh.vertices[*c as usize] + origin).as_vec3().to_array())
                .collect(),
            normals: m.normals.clone(),
            indices: m.indices.clone(),
        };
        note_placed(&placed, key, origin);
        return placed;
    }
    let (m, from) = weld(mesh, origin);
    if let Ok(mut c) = WELDED.lock() {
        if c.len() >= KEPT {
            c.remove(0);
        }
        c.push((key, Arc::new((m.clone(), from))));
    }
    note_placed(&m, key, origin);
    m
}

/// Large meshes made by [`edit_mesh_to_tri`], by fingerprint: the shape
/// each is (a number for its content before it was placed) and where it
/// was placed.
static PLACED: std::sync::Mutex<Vec<(u64, u64, DVec3)>> = std::sync::Mutex::new(Vec::new());

fn note_placed(mesh: &TriMesh, shape: u64, origin: DVec3) {
    const KEPT: usize = 2048;
    let print = mesh.fingerprint();
    if let Ok(mut p) = PLACED.lock() {
        if p.iter().any(|e| e.0 == print) {
            return;
        }
        if p.len() >= KEPT {
            p.drain(..KEPT / 2);
        }
        p.push((print, shape, origin));
    }
}

/// For a large mass's mesh as the scene holds it, by its
/// [`TriMesh::fingerprint`]: which shape it is and where it stands. Two
/// meshes of one shape differ only by where they stand, so what was
/// worked out for one (its feature edges) serves the other, moved. None
/// for other meshes, and for one cut or joined after it was made.
pub fn placed_shape(fingerprint: u64) -> Option<(u64, DVec3)> {
    let p = PLACED.lock().ok()?;
    p.iter()
        .rev()
        .find(|e| e.0 == fingerprint)
        .map(|e| (e.1, e.2))
}

/// [`edit_mesh_to_tri`] worked out: the mesh, and for each of its
/// positions the vertex of `mesh` it sits at.
fn weld(mesh: &creabim_core::EditMesh, origin: DVec3) -> (TriMesh, Vec<u32>) {
    use std::collections::HashMap;
    const TOL: f64 = 1e-6;
    let key = |p: DVec3| {
        (
            (p.x / TOL).round() as i64,
            (p.y / TOL).round() as i64,
            (p.z / TOL).round() as i64,
        )
    };
    let mut first: HashMap<(i64, i64, i64), usize> = HashMap::new();
    let corner: Vec<usize> = (0..mesh.vertices.len())
        .map(|v| *first.entry(key(mesh.vertices[v])).or_insert(v))
        .collect();
    let mut m = TriMesh::default();
    let mut from: Vec<u32> = Vec::new();
    let mut shared: HashMap<(usize, [i64; 3]), u32> = HashMap::new();
    for f in 0..mesh.faces.len() {
        let face = &mesh.faces[f];
        let n = mesh.face_normal(f);
        let nk = (n * 1e5).round().as_i64vec3().to_array();
        let mut index = |k: usize| -> (u32, DVec3) {
            let c = corner[face[k] as usize];
            let p = mesh.vertices[c] + origin;
            let i = *shared.entry((c, nk)).or_insert_with(|| {
                m.positions.push(p.as_vec3().to_array());
                m.normals.push(n.as_vec3().to_array());
                from.push(c as u32);
                (m.positions.len() - 1) as u32
            });
            (i, p)
        };
        let tris: Vec<[(u32, DVec3); 3]> = mesh
            .face_corners(f)
            .into_iter()
            .map(|t| t.map(&mut index))
            .collect();
        for [(a, pa), (b, pb), (c, pc)] in tris {
            // keep the winding consistent with the normal
            if (pb - pa).cross(pc - pa).dot(n) >= 0.0 {
                m.indices.extend([a, b, c]);
            } else {
                m.indices.extend([a, c, b]);
            }
        }
    }
    (m, from)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that count what the shared cut cache holds take turns.
    static CUTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// `model` with everything that stands by itself moved by `by`.
    fn moved(model: &Model, by: DVec3) -> Model {
        use creabim_core::transform::{transformed, Xform};
        let mut doc = creabim_core::Document::from_model(model.clone());
        let mut tx = creabim_core::Transaction::new("Move");
        for e in model.iter() {
            // what stands by itself (windows and doors go with their walls)
            let moves = matches!(
                e.kind,
                ElementKind::Wall(_)
                    | ElementKind::Slab(_)
                    | ElementKind::Roof(_)
                    | ElementKind::Column(_)
                    | ElementKind::Beam(_)
                    | ElementKind::Footing(_)
                    | ElementKind::Stair(_)
                    | ElementKind::Object(_)
                    | ElementKind::Shaft(_)
                    | ElementKind::Cut(_)
                    | ElementKind::Railing(_)
                    | ElementKind::FreeMesh(_)
                    | ElementKind::Part(_)
            );
            if let Some(n) = transformed(model, e, &Xform::Move(by)).filter(|_| moves) {
                tx = tx.replace(e.clone(), n);
            }
        }
        doc.commit(tx).unwrap();
        doc.model().clone()
    }

    fn joined(model: &Model, carry: bool) -> (Vec<SceneItem>, usize) {
        let mut items = build_scene_unjoined(model, None);
        let taken = apply_joins(model, &mut items, carry);
        mend_built(model, &mut items);
        (items, taken)
    }

    /// Is `p` inside the closed `mesh`: how many of its triangles a ray from
    /// `p` crosses, odd or even (the ray slanted, so it meets no edge).
    fn inside_mesh(mesh: &TriMesh, p: DVec3) -> bool {
        let d = DVec3::new(1.0, 0.371, 0.213).normalize();
        let at = |i: u32| glam::Vec3::from(mesh.positions[i as usize]).as_dvec3();
        let mut n = 0;
        for t in mesh.indices.chunks(3) {
            let (a, b, c) = (at(t[0]), at(t[1]), at(t[2]));
            let (e1, e2) = (b - a, c - a);
            let h = d.cross(e2);
            let det = e1.dot(h);
            if det.abs() < 1e-12 {
                continue;
            }
            let s = p - a;
            let u = s.dot(h) / det;
            let q = s.cross(e1);
            let v = d.dot(q) / det;
            if u < 0.0 || v < 0.0 || u + v > 1.0 {
                continue;
            }
            if e2.dot(q) / det > 1e-9 {
                n += 1;
            }
        }
        n % 2 == 1
    }

    /// How many of the ceilings' triangles lie inside a wall, in `items`.
    fn ceiling_in_walls(model: &Model, items: &[SceneItem]) -> usize {
        let kind = |i: &SceneItem| model.get(i.id).map(|e| &e.kind);
        let walls: Vec<&SceneItem> = items
            .iter()
            .filter(|i| {
                !i.ghost && matches!(kind(i), Some(ElementKind::Wall(w)) if w.curtain.is_none())
            })
            .collect();
        let mut n = 0;
        for c in items
            .iter()
            .filter(|i| matches!(kind(i), Some(ElementKind::Ceiling(_))))
        {
            let at = |i: u32| glam::Vec3::from(c.mesh.positions[i as usize]).as_dvec3();
            for t in c.mesh.indices.chunks(3) {
                let m = (at(t[0]) + at(t[1]) + at(t[2])) / 3.0;
                let hit = walls.iter().any(|w| {
                    w.mesh.bounds().is_some_and(|(lo, hi)| {
                        let (lo, hi) = (lo.as_dvec3() + 2e-3, hi.as_dvec3() - 2e-3);
                        m.cmpgt(lo).all() && m.cmplt(hi).all()
                    }) && inside_mesh(&w.mesh, m)
                });
                n += usize::from(hit);
            }
        }
        n
    }

    /// A ceiling is a finish: the walls it reaches cut it, so its board
    /// stops at their faces (MU139: the gypsum board and the porch's cedar
    /// soffit ran on through the timber walls).
    #[test]
    fn walls_cut_the_ceilings_they_reach() {
        let model = creabim_core::ridgeline::ridgeline_house();
        let plain = build_scene_unjoined(&model, None);
        let before = ceiling_in_walls(&model, &plain);
        assert!(
            before > 0,
            "Ridgeline's ceilings reach into its walls before the joins"
        );
        let (items, _) = joined(&model, false);
        assert_eq!(ceiling_in_walls(&model, &items), 0, "of {before}");
    }

    /// How many of the pads' triangles lie inside a wall, slab, footing or
    /// column, in `items`.
    fn pad_in_built(model: &Model, items: &[SceneItem]) -> usize {
        let kind = |i: &SceneItem| model.get(i.id).map(|e| &e.kind);
        let built: Vec<&SceneItem> = items
            .iter()
            .filter(|i| {
                !i.ghost
                    && match kind(i) {
                        Some(ElementKind::Wall(w)) => w.curtain.is_none(),
                        Some(
                            ElementKind::Slab(_) | ElementKind::Footing(_) | ElementKind::Column(_),
                        ) => true,
                        _ => false,
                    }
            })
            .collect();
        let pad = |i: &SceneItem| matches!(kind(i), Some(ElementKind::Site(s)) if matches!(s.form, creabim_core::SiteForm::Pad { .. }));
        // points through the middle of each pad's skin, every 0.2 m: a
        // triangle's centre misses a stem wall under a skin of a dozen
        // large triangles
        let mut n = 0;
        for c in items.iter().filter(|i| pad(i)) {
            let Some((lo, hi)) = c.mesh.bounds() else {
                continue;
            };
            let (lo, hi) = (lo.as_dvec3(), hi.as_dvec3());
            let z = (lo.z + hi.z) / 2.0;
            let (nx, ny) = (
                ((hi.x - lo.x) / 0.2) as usize,
                ((hi.y - lo.y) / 0.2) as usize,
            );
            for i in 0..nx {
                for j in 0..ny {
                    let m = DVec3::new(lo.x + 0.1 + 0.2 * i as f64, lo.y + 0.1 + 0.2 * j as f64, z);
                    let in_built = built.iter().any(|w| {
                        w.mesh.bounds().is_some_and(|(lo, hi)| {
                            let (lo, hi) = (lo.as_dvec3() + 2e-3, hi.as_dvec3() - 2e-3);
                            m.cmpgt(lo).all() && m.cmplt(hi).all()
                        }) && inside_mesh(&w.mesh, m)
                    });
                    n += usize::from(in_built && inside_mesh(&c.mesh, m));
                }
            }
        }
        n
    }

    /// A pad is the ground made level for the building: the slab sunk
    /// into it, the stem walls and footings dug down from it pass through
    /// it (MU140: Ridgeline's house pad ran through the slab and the stem
    /// walls).
    #[test]
    fn what_is_built_on_a_pad_passes_through_it() {
        let model = creabim_core::ridgeline::ridgeline_house();
        let plain = build_scene_unjoined(&model, None);
        let before = pad_in_built(&model, &plain);
        assert!(
            before > 0,
            "Ridgeline's house pad runs through its foundations before the joins"
        );
        let t = std::time::Instant::now();
        let (items, _) = joined(&model, false);
        eprintln!("joined in {:?}", t.elapsed());
        assert_eq!(pad_in_built(&model, &items), 0, "of {before}");
        // and the pad is still there round the house
        assert!(items.iter().any(|i| matches!(
            model.get(i.id).map(|e| &e.kind),
            Some(ElementKind::Site(s)) if matches!(s.form, creabim_core::SiteForm::Pad { .. })
        ) && !i.mesh.indices.is_empty()));
    }

    /// MU141 (owner, at a wall's foot in a cropped 3D view: "the insulation
    /// is pushing away the floor finish?"): a foundation wall's inside
    /// insulation, which the ground floor's base and core cut away, cut a
    /// sliver out of the floor finish on top. It can't get there through
    /// them: the finish runs over it to the stem wall.
    #[test]
    fn a_foundations_insulation_does_not_break_through_the_floor_finish() {
        let model = creabim_core::ridgeline::ridgeline_house();
        let (items, _) = joined(&model, false);
        let plain = build_scene_unjoined(&model, None);
        let name = |id: ElementId| model.get(id).map_or(String::new(), |e| e.name.clone());
        let layer = |i: &SceneItem, f: creabim_core::LayerFunction| {
            i.layer.is_some_and(|l| l.function == f)
        };
        use creabim_core::LayerFunction as F;
        let finish: Vec<&SceneItem> = items
            .iter()
            .filter(|i| name(i.id) == "Ground floor slab" && layer(i, F::Finish))
            .collect();
        assert!(!finish.is_empty());
        let (_, top) = finish[0].mesh.bounds().unwrap();
        let mut seen = 0;
        for strip in plain
            .iter()
            .filter(|i| name(i.id).starts_with("FDN") && layer(i, F::Insulation))
        {
            let (lo, hi) = strip.mesh.bounds().unwrap();
            // the strip's middle, in the finish's band
            let p = DVec3::new(
                ((lo.x + hi.x) / 2.0) as f64,
                ((lo.y + hi.y) / 2.0) as f64,
                top.z as f64 - 0.009,
            );
            let in_finish = finish.iter().any(|f| inside_mesh(&f.mesh, p));
            let in_strip = items
                .iter()
                .filter(|i| i.id == strip.id && layer(i, F::Insulation))
                .any(|i| inside_mesh(&i.mesh, p));
            // only where the slab runs to it
            if !in_finish && !in_strip {
                continue;
            }
            seen += 1;
            assert!(
                in_finish && !in_strip,
                "{}: at {p} the finish {in_finish}, the insulation {in_strip}",
                name(strip.id)
            );
        }
        assert!(seen >= 4, "{seen} foundation walls along the slab");
    }

    /// MU144 (owner: "shouldn't insulation meet insulation? … is the
    /// gravel poured above the insulation?"): a foundation wall's inside
    /// insulation stopped under the slab's gravel, which ran over it to the
    /// concrete and left a gap between it and the slab's insulation. Earth
    /// fill gives way to whatever a wall brings into it, so the wall's
    /// insulation runs up through the gravel to meet the slab's.
    #[test]
    fn a_foundations_insulation_meets_the_slabs_through_its_gravel() {
        let model = creabim_core::ridgeline::ridgeline_house();
        let (items, _) = joined(&model, false);
        let plain = build_scene_unjoined(&model, None);
        let name = |id: ElementId| model.get(id).map_or(String::new(), |e| e.name.clone());
        let layer = |i: &SceneItem, f: creabim_core::LayerFunction| {
            i.layer.is_some_and(|l| l.function == f)
        };
        use creabim_core::LayerFunction as F;
        // the slab's gravel, as drawn before any join
        let gravel: Vec<&SceneItem> = plain
            .iter()
            .filter(|i| name(i.id) == "Ground floor slab" && layer(i, F::Substrate))
            .collect();
        assert!(!gravel.is_empty());
        let (glo, ghi) = gravel[0].mesh.bounds().unwrap();
        let mut seen = 0;
        for strip in plain
            .iter()
            .filter(|i| name(i.id).starts_with("FDN") && layer(i, F::Insulation))
        {
            let (lo, hi) = strip.mesh.bounds().unwrap();
            // the strip's middle, halfway up the gravel
            let p = DVec3::new(
                ((lo.x + hi.x) / 2.0) as f64,
                ((lo.y + hi.y) / 2.0) as f64,
                ((glo.z + ghi.z) / 2.0) as f64,
            );
            if !gravel.iter().any(|g| inside_mesh(&g.mesh, p)) {
                continue; // the slab does not run to this wall
            }
            seen += 1;
            let in_gravel = items
                .iter()
                .filter(|i| name(i.id) == "Ground floor slab" && layer(i, F::Substrate))
                .any(|i| inside_mesh(&i.mesh, p));
            let in_strip = items
                .iter()
                .filter(|i| i.id == strip.id && layer(i, F::Insulation))
                .any(|i| inside_mesh(&i.mesh, p));
            assert!(
                in_strip && !in_gravel,
                "{}: at {p} the insulation {in_strip}, the gravel {in_gravel}",
                name(strip.id)
            );
        }
        assert!(seen >= 4, "{seen} foundation walls along the slab's gravel");
    }

    /// MU142's memory (Windows flagged the test builds' growing memory):
    /// a wall dragged step by step made some 30 join cuts a step, every one
    /// kept until the cache held 4 000 (65 MB on the Ridgeline). Past their
    /// budget the caches keep only what the last builds use: a drag holds
    /// them near it, and the scene as it now stands still makes no cut again.
    #[test]
    fn a_drag_keeps_the_join_caches_to_what_is_in_use() {
        use creabim_core::{Document, Transaction, Xform};
        let _turn = CUTS.lock().unwrap_or_else(|e| e.into_inner());
        const BUDGET: usize = 6 << 20;
        TEST_JOIN_BUDGET.with(|b| b.set(Some(BUDGET)));
        join_cache().clear();
        carried().clear();
        let mut doc = Document::from_model(creabim_core::ridgeline::ridgeline_house());
        let wall = doc
            .model()
            .iter()
            .find(|e| e.name == "GF West")
            .map(|e| e.id)
            .unwrap();
        let mut most = 0;
        for _ in 0..30 {
            let _ = build_scene(doc.model());
            most = most.max(join_cache().bytes());
            // a drag's step: the wall 2 cm further along
            let before = doc.model().get(wall).unwrap().clone();
            let after = creabim_core::transform::transformed(
                doc.model(),
                &before,
                &Xform::Move(DVec3::new(0.02, 0.0, 0.0)),
            )
            .unwrap();
            doc.commit(Transaction::new("Move").replace(before, after))
                .unwrap();
        }
        let _ = build_scene(doc.model());
        // other tests' scenes, built at the same time, may add theirs
        assert!(
            most < 3 * BUDGET,
            "the cuts kept grew to {:.1} MB",
            most as f64 / 1e6
        );
        assert!(
            carried().bytes() < 3 * BUDGET / 2,
            "{:.1} MB carried",
            carried().bytes() as f64 / 1e6
        );
        // and the scene as it stands is served from what is kept
        let made = CUTS_MADE.with(|n| n.get());
        let _ = build_scene(doc.model());
        assert_eq!(
            CUTS_MADE.with(|n| n.get()),
            made,
            "the scene as it stands made its cuts again"
        );
        TEST_JOIN_BUDGET.with(|b| b.set(None));
    }

    /// The same model gives the same meshes, build after build: a mesh
    /// that came out with its corners in another order was a new shape to
    /// the join cache every time, cut and kept again (MU143: the Ridgeline's
    /// ceilings over its rooms, traced from whichever wall came first).
    #[test]
    fn the_same_model_builds_the_same_meshes() {
        let model = creabim_core::ridgeline::ridgeline_house();
        let (a, b) = (
            build_scene_unjoined(&model, None),
            build_scene_unjoined(&model, None),
        );
        assert_eq!(a.len(), b.len());
        let name = |id: ElementId| model.get(id).map_or(String::new(), |e| e.name.clone());
        let differ: Vec<String> = a
            .iter()
            .zip(&b)
            .filter(|(x, y)| x.mesh.fingerprint() != y.mesh.fingerprint())
            .map(|(x, _)| name(x.id))
            .collect();
        assert!(differ.is_empty(), "built differently: {differ:?}");
    }

    /// MU143 (owner, the starter house in 3D: "more inconsistencies", "this
    /// kind of inconsistencies"): short lines across a wall's face where a
    /// join cut split it. The cut's pieces didn't share their corners, and
    /// each side left unmatched drew. After the joins no piece of a wall or
    /// a slab has a corner lying along another triangle's side.
    #[test]
    fn join_cuts_leave_no_corner_on_a_side() {
        let model = creabim_core::starter::starter_house();
        let (items, _) = joined(&model, false);
        let name = |id: ElementId| model.get(id).map_or(String::new(), |e| e.name.clone());
        let built = |id: ElementId| {
            matches!(
                model.get(id).map(|e| &e.kind),
                Some(ElementKind::Wall(_) | ElementKind::Slab(_))
            )
        };
        let loose: Vec<String> = items
            .iter()
            .filter(|i| !i.ghost && built(i.id))
            .filter(|i| crate::cropbox::mend(&i.mesh).indices.len() != i.mesh.indices.len())
            .map(|i| name(i.id))
            .collect();
        assert!(
            loose.is_empty(),
            "{} pieces with corners on others' sides: {loose:?}",
            loose.len()
        );
    }

    /// MU143 (owner, the starter house's balcony: "find out why there is
    /// an issue here"): the loggia's column rose through the balcony, which
    /// ran on through it, its edge flush with the column's faces showing
    /// its layers on the column. A column runs through the slabs it passes.
    #[test]
    fn a_column_runs_through_the_slabs_it_passes() {
        let model = creabim_core::starter::starter_house();
        let (items, _) = joined(&model, false);
        let name = |id: ElementId| model.get(id).map_or(String::new(), |e| e.name.clone());
        let column = items
            .iter()
            .find(|i| name(i.id) == "Loggia column")
            .unwrap();
        let (lo, hi) = column.mesh.bounds().unwrap();
        let axis = ((lo + hi) / 2.0).as_dvec3();
        let mut met = 0;
        for slab in items.iter().filter(|i| {
            !i.ghost && matches!(model.get(i.id).map(|e| &e.kind), Some(ElementKind::Slab(_)))
        }) {
            let (sl, sh) = slab.mesh.bounds().unwrap();
            // only those it passes, not the one it stands on
            if sl.x > hi.x
                || sh.x < lo.x
                || sl.y > hi.y
                || sh.y < lo.y
                || sl.z < lo.z + 1e-3
                || sh.z > hi.z - 1e-3
            {
                continue;
            }
            let p = DVec3::new(axis.x, axis.y, ((sl.z + sh.z) / 2.0) as f64);
            met += 1;
            assert!(
                !inside_mesh(&slab.mesh, p),
                "{} runs through the column at {p}",
                name(slab.id)
            );
        }
        assert!(met >= 1, "the column passes no slab");
    }

    /// The walls' and slabs' pieces of a scene, by element.
    fn pieces<'a>(
        model: &Model,
        items: &'a [SceneItem],
    ) -> std::collections::HashMap<ElementId, Vec<&'a TriMesh>> {
        let mut out: std::collections::HashMap<ElementId, Vec<&TriMesh>> = Default::default();
        for i in items.iter().filter(|i| !i.ghost) {
            let joined = model
                .get(i.id)
                .is_some_and(|e| matches!(e.kind, ElementKind::Wall(_) | ElementKind::Slab(_)));
            if joined {
                out.entry(i.id).or_default().push(&i.mesh);
            }
        }
        out
    }

    #[test]
    fn a_shape_added_to_a_layered_wall_is_joined_once_and_the_layers_make_way() {
        let _turn = CUTS.lock().unwrap_or_else(|e| e.into_inner());
        use creabim_core::{Cut, Element, ElementKind, FreeMesh, Transaction};
        let mut model = creabim_core::starter::starter_house();
        let scene = build_scene(&model);
        // a wall drawn as several layers
        let (wall, w) = model
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Wall(w)
                    if scene.iter().filter(|i| i.id == e.id).count() > 1 && w.length() > 2.0 =>
                {
                    Some((e.id, w.clone()))
                }
                _ => None,
            })
            .expect("a layered wall");
        let z = model.level(w.level).map_or(0.0, |l| l.elevation);
        let d = w.direction();
        let n = DVec2::new(-d.y, d.x);
        let mid = (w.start + w.end) / 2.0;
        // a box from inside the wall to 0.6 m out of one face, 1 m tall
        let size = DVec3::new(0.5, 0.6 + w.thickness, 1.0);
        let mut shape = FreeMesh::new(
            Some(w.level),
            DVec3::ZERO,
            creabim_core::EditMesh::cuboid(size),
        );
        // turned with the wall, its near side in the wall's far face
        shape.rotation = d.y.atan2(d.x).to_degrees();
        let corner = mid - d * 0.25 - n * (w.thickness / 2.0);
        shape.origin = corner.extend(z);
        let s = Element::new("Shape", ElementKind::FreeMesh(shape));
        let sid = s.id;
        let cut = Element::new(
            "Cut",
            ElementKind::Cut(Cut {
                target: wall,
                cutter: sid,
                op: BoolOp::Union,
                reach: Default::default(),
            }),
        );
        Transaction::new("add")
            .insert(s)
            .insert(cut)
            .apply(&mut model)
            .unwrap();
        let scene = build_scene(&model);
        // how far each of the wall's layers reaches from its centre line
        let reach = |m: &TriMesh| {
            m.positions
                .iter()
                .map(|p| {
                    (glam::Vec3::from(*p).as_dvec3().truncate() - mid)
                        .dot(n)
                        .abs()
                })
                .fold(0.0, f64::max)
        };
        let out: Vec<f64> = scene
            .iter()
            .filter(|i| i.id == wall)
            .map(|i| reach(&i.mesh))
            .collect();
        let past = out.iter().filter(|r| **r > w.thickness / 2.0 + 0.3).count();
        assert_eq!(past, 1, "layers reaching out with the shape: {out:?}");
    }

    #[test]
    fn a_moving_building_takes_its_cuts_along_and_is_cut_afresh_when_dropped() {
        let _turn = CUTS.lock().unwrap_or_else(|e| e.into_inner());
        let here = creabim_core::starter::starter_house();
        let by = glam::Vec3::new(7.3, -4.1, 0.0);
        let there = moved(&here, by.as_dvec3());
        // at rest: the cuts as made
        let (rest, none) = joined(&here, false);
        assert_eq!(none, 0);
        // moving: the cut pieces are the ones at rest, set down where the
        // building now stands, corner for corner
        let (moving, taken) = joined(&there, true);
        assert!(taken > 20, "{taken} pieces carried");
        let (a, b) = (pieces(&here, &rest), pieces(&there, &moving));
        let mut same = 0;
        for (id, was) in &a {
            let Some(now) = b.get(id) else {
                continue;
            };
            if was.len() != now.len() {
                continue;
            }
            for (x, y) in was.iter().zip(now) {
                if x.positions.len() != y.positions.len() || x.indices != y.indices {
                    continue;
                }
                let off = x
                    .positions
                    .iter()
                    .zip(&y.positions)
                    .map(|(p, q)| (glam::Vec3::from(*p) + by).distance(glam::Vec3::from(*q)))
                    .fold(0.0f32, f32::max);
                // a mended piece lists its corners triangle by triangle, so a
                // piece cut afresh may have the same count and another order:
                // only the carried ones are the same corner for corner, and
                // all of them must be (`same >= taken` below)
                if off < 1e-4 {
                    same += 1;
                }
            }
        }
        assert!(same >= taken, "{same} pieces the same, {taken} carried");
        // dropped: nothing is carried, and it is cut where it stands,
        // exactly as a scene made with nothing kept
        let (dropped, none) = joined(&there, false);
        assert_eq!(none, 0, "the drop carries nothing");
        join_cache().clear();
        carried().clear();
        let (plain, _) = joined(&there, false);
        let (a, b) = (pieces(&there, &dropped), pieces(&there, &plain));
        assert_eq!(a.len(), b.len());
        for (id, x) in &a {
            let y = &b[id];
            assert_eq!(x.len(), y.len());
            for (m, n) in x.iter().zip(y) {
                assert_eq!(m.indices.len(), n.indices.len(), "{id:?}");
                assert_eq!(m.bounds(), n.bounds(), "{id:?}");
            }
        }
    }

    #[test]
    fn carrying_changes_nothing_for_what_stands_still() {
        let _turn = CUTS.lock().unwrap_or_else(|e| e.into_inner());
        let model = creabim_core::garden::garden_house();
        let (plain, _) = joined(&model, false);
        let (carrying, taken) = joined(&model, true);
        assert_eq!(taken, 0);
        for (a, b) in plain.iter().zip(&carrying) {
            let joined = model
                .get(a.id)
                .is_some_and(|e| matches!(e.kind, ElementKind::Wall(_) | ElementKind::Slab(_)));
            if joined {
                assert_eq!(a.mesh.positions, b.mesh.positions);
                assert_eq!(a.mesh.indices, b.mesh.indices);
            }
        }
    }

    #[test]
    fn a_pulled_shape_patched_into_the_scene_is_the_scene_built_whole() {
        use creabim_core::{EditMesh, FreeMesh, Transaction};
        let mut model = creabim_core::starter::starter_house();
        let shape = Element::new(
            "Shape",
            ElementKind::FreeMesh(FreeMesh::new(
                None,
                glam::DVec3::new(30.0, 30.0, 0.0),
                EditMesh::cuboid(glam::DVec3::new(4.0, 2.0, 1.0)),
            )),
        );
        let id = shape.id;
        Transaction::new("add")
            .insert(shape.clone())
            .apply(&mut model)
            .unwrap();
        assert!(stands_alone(&model, id));
        let wall = model
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Wall(_)))
            .unwrap()
            .id;
        assert!(!stands_alone(&model, wall), "a wall is joined to others");
        let base = std::sync::Arc::new(build_scene(&model));
        // pulled: its top goes up
        let mut pulled = shape.clone();
        if let ElementKind::FreeMesh(f) = &mut pulled.kind {
            let top = (0..6).find(|k| f.mesh.face_normal(*k).z > 0.9).unwrap();
            f.mesh.pull_face(top, 1.5);
        }
        Transaction::new("pull")
            .replace(shape, pulled)
            .apply(&mut model)
            .unwrap();
        let changed: std::collections::HashSet<ElementId> = [id].into_iter().collect();
        let patched = shared_scene_patched(&model, &base, &changed);
        let whole = build_scene(&model);
        assert_eq!(patched.len(), whole.len());
        // the same shapes (an outline found again may start on another of
        // its corners: the same points, in another order)
        let points = |i: &SceneItem| {
            let mut p: Vec<[i64; 3]> = i
                .mesh
                .positions
                .iter()
                .map(|q| q.map(|c| (c as f64 * 1e4).round() as i64))
                .collect();
            p.sort_unstable();
            p
        };
        for (a, b) in patched.iter().zip(&whole) {
            assert_eq!(a.id, b.id, "in the same order");
            assert_eq!(points(a), points(b));
            assert_eq!(a.mesh.indices.len(), b.mesh.indices.len());
        }
        let top = patched
            .iter()
            .find(|i| i.id == id)
            .and_then(|i| i.mesh.bounds())
            .unwrap()
            .1
            .z;
        assert!((top - 2.5).abs() < 1e-5, "{top}");
    }

    /// A chair turned is built again alone, into the scene as it was, and
    /// that is the scene built whole (MU142: turning one rebuilt every
    /// snap of the house on letting go).
    #[test]
    fn a_turned_chair_patched_into_the_scene_is_the_scene_built_whole() {
        let mut model = creabim_core::ridgeline::ridgeline_house();
        let chair = model
            .iter()
            .find(|e| e.name == "Dining chair" && matches!(e.kind, ElementKind::Object(_)))
            .unwrap()
            .clone();
        assert!(stands_alone(&model, chair.id), "furniture stands alone");
        let base = std::sync::Arc::new(build_scene(&model));
        let mut turned = chair.clone();
        if let ElementKind::Object(o) = &mut turned.kind {
            o.rotation += 37.0;
        }
        creabim_core::Transaction::new("turn")
            .replace(chair.clone(), turned)
            .apply(&mut model)
            .unwrap();
        let changed: std::collections::HashSet<ElementId> = [chair.id].into_iter().collect();
        let patched = shared_scene_patched(&model, &base, &changed);
        let whole = build_scene(&model);
        assert_eq!(patched.len(), whole.len());
        let key = |i: &SceneItem| {
            let mut p: Vec<[i64; 3]> = i
                .mesh
                .positions
                .iter()
                .map(|q| q.map(|c| (c as f64 * 1e4).round() as i64))
                .collect();
            p.sort_unstable();
            (i.id, p, i.mesh.indices.len())
        };
        // the chair as the whole scene builds it, turned
        let of = |s: &[SceneItem], mine: bool| {
            let mut v: Vec<_> = s
                .iter()
                .filter(|i| (i.id == chair.id) == mine)
                .map(key)
                .collect();
            v.sort();
            v
        };
        assert!(!of(&patched, true).is_empty());
        assert_eq!(of(&patched, true), of(&whole, true), "the chair, turned");
        assert_ne!(of(&patched, true), of(&base, true), "the chair did turn");
        // and all else as it was (a ceiling may come out of a whole build
        // with its triangles in another order, see the MU142 notes)
        assert!(
            of(&patched, false) == of(&base, false),
            "something else changed"
        );
    }

    #[test]
    fn a_void_or_guide_shape_is_seen_in_3d_and_left_out_of_drawings() {
        use creabim_core::shape::Role;
        let house = creabim_core::sample::sample_house();
        let level = house
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Level(_)))
            .unwrap()
            .id;
        let plan = house
            .iter()
            .find(|e| {
                matches!(&e.kind, ElementKind::View(v)
                    if matches!(v.kind, creabim_core::ViewKind::Plan { level: l, .. } if l == level))
            })
            .unwrap()
            .id;
        for role in Role::ALL {
            let mut f = creabim_core::FreeMesh::new(
                Some(level),
                DVec3::new(20.0, 20.0, 0.0),
                creabim_core::EditMesh::cuboid(DVec3::new(2.0, 2.0, 2.0)),
            );
            f.role = role;
            let shape = creabim_core::Element::new("Shape", ElementKind::FreeMesh(f));
            let id = shape.id;
            let model = Model::from_elements(house.iter().cloned().chain([shape])).unwrap();
            let in_3d = build_scene(&model).iter().any(|i| i.id == id);
            let drawn = drawn_scene(&model).iter().any(|i| i.id == id);
            let in_plan = crate::section::view_drawing(&model, plan)
                .unwrap()
                .items
                .iter()
                .any(|i| i.id() == id);
            assert!(in_3d, "{role:?} is in the 3D scene");
            assert_eq!(
                drawn,
                role.is_solid(),
                "{role:?} in sections and elevations"
            );
            assert_eq!(in_plan, role.is_solid(), "{role:?} in its plan");
        }
    }

    #[test]
    fn a_join_cut_kept_on_one_thread_serves_the_next() {
        let _turn = CUTS.lock().unwrap_or_else(|e| e.into_inner());
        let model = creabim_core::starter::starter_house();
        join_cache().clear();
        let first = build_scene(&model);
        let kept = join_cache().len();
        assert!(kept > 10, "the starter house has joins to cut");
        // a scene built on another thread makes no cut again, and is the
        // same. Counted by the cuts that thread makes, not by the cache's
        // size: other tests' scenes, built at the same time, add theirs to
        // the shared cache (it failed in full runs only)
        let m = model.clone();
        let (again, made) = std::thread::spawn(move || {
            CUTS_MADE.with(|n| n.set(0));
            let s = build_scene(&m);
            (s, CUTS_MADE.with(|n| n.get()))
        })
        .join()
        .unwrap();
        assert_eq!(made, 0, "cuts made again on the second thread");
        // (walls and slabs: what joins cut; a ceiling's outline starts at
        // any corner from one build to the next)
        for (a, b) in first.iter().zip(&again) {
            let joined = model
                .get(a.id)
                .is_some_and(|e| matches!(e.kind, ElementKind::Wall(_) | ElementKind::Slab(_)));
            if joined {
                assert_eq!(a.mesh.positions, b.mesh.positions);
                assert_eq!(a.mesh.indices, b.mesh.indices);
            }
        }
    }

    #[test]
    fn a_large_mesh_welded_once_is_the_same_mesh_wherever_it_stands() {
        use creabim_core::modifiers::{Modifier, ModifierKind};
        let mut tower = creabim_core::FreeMesh::new(
            None,
            DVec3::ZERO,
            creabim_core::EditMesh::cuboid(DVec3::new(10.0, 8.0, 24.0)),
        );
        tower.modifiers = vec![Modifier::new(ModifierKind::Panelize {
            width: 1.2,
            height: 3.0,
            joint: 0.02,
            depth: 0.05,
        })];
        let shape = tower.evaluated().mesh;
        assert!(shape.faces.len() > 1000);
        for at in [
            DVec3::ZERO,
            DVec3::new(31.7, -12.25, 3.0),
            DVec3::new(-1e3, 2e3, 0.1),
        ] {
            let (plain, _) = weld(&shape, at);
            // the first asks it, the second takes what was kept
            for _ in 0..2 {
                let kept = edit_mesh_to_tri(&shape, at);
                assert_eq!(kept.positions, plain.positions);
                assert_eq!(kept.normals, plain.normals);
                assert_eq!(kept.indices, plain.indices);
            }
        }
    }

    #[test]
    fn a_partition_held_to_the_ceilings_has_no_sliver_at_its_ends() {
        // MU20: FF Spine runs a few centimetres past the ceiling's edge into
        // the wall it joins; its top used to rise there to its own height
        let m = creabim_core::starter::starter_house();
        let id = m.iter().find(|e| e.name == "FF Spine").unwrap().id;
        let top = build_scene_of(&m, None)
            .iter()
            .filter(|i| i.id == id)
            .filter_map(|i| i.mesh.bounds())
            .map(|b| b.1.z)
            .fold(f32::MIN, f32::max);
        assert!((top - 5.5).abs() < 1e-3, "its top reaches {top}");
    }

    #[test]
    fn a_slab_whose_cut_is_refused_is_not_cut_again() {
        // Highland Haven: imported landscape slabs swelled past the limit
        // at every wall's cut, and each refused cut was made in full and
        // thrown away, one wall layer after another: 13 of the 18 s the
        // window froze on open. Once a cut is refused the slab stays as it
        // is, so the walls after it are not cut into it either, and the
        // refusal is remembered for the next build
        use creabim_core::{Element, Level, Slab, Transaction, Wall};
        let level = |name: &str, z: f64| {
            Element::new(
                name,
                ElementKind::Level(Level {
                    elevation: z,
                    height: 3.0,
                }),
            )
        };
        let (l0, l1) = (level("L0", 0.0), level("L1", 3.0));
        // somewhere no other test builds, so no cut is cached yet
        let o = DVec2::new(731.0, -417.0);
        let wall = |a: DVec2, b: DVec2| {
            Element::new(
                "Wall",
                ElementKind::Wall(Wall::new(l0.id, o + a, o + b, 0.3, 3.0)),
            )
        };
        let slab = Element::new(
            "Slab",
            ElementKind::Slab(Slab::new(
                l1.id,
                [(0.0, 0.0), (6.0, 0.0), (6.0, 4.0), (0.0, 4.0)]
                    .map(|(x, y)| o + DVec2::new(x, y))
                    .to_vec(),
                0.25,
            )),
        );
        let sid = slab.id;
        let (w0, w1) = (
            wall(DVec2::ZERO, DVec2::new(6.0, 0.0)),
            wall(DVec2::new(0.0, 4.0), DVec2::new(6.0, 4.0)),
        );
        let mut doc = creabim_core::document::Document::new();
        doc.commit(
            Transaction::new("setup")
                .insert(l0)
                .insert(l1)
                .insert(w0)
                .insert(w1)
                .insert(slab),
        )
        .unwrap();
        let _turn = CUTS.lock().unwrap_or_else(|e| e.into_inner());
        let plain = build_scene_unjoined(doc.model(), None)
            .into_iter()
            .find(|i| i.id == sid)
            .unwrap()
            .mesh;
        // every cut refused
        TEST_CUT_LIMIT.with(|l| l.set(Some(0)));
        CUTS_MADE.with(|c| c.set(0));
        let (items, _) = joined(doc.model(), false);
        let first = CUTS_MADE.with(|c| c.get());
        CUTS_MADE.with(|c| c.set(0));
        let _ = joined(doc.model(), false);
        let again = CUTS_MADE.with(|c| c.get());
        TEST_CUT_LIMIT.with(|l| l.set(None));
        assert_eq!(
            first, 1,
            "cuts made into the slab after the first was refused"
        );
        assert_eq!(again, 0, "cuts made again on the next build");
        let slab_mesh = &items.iter().find(|i| i.id == sid).unwrap().mesh;
        assert_eq!(
            slab_mesh.fingerprint(),
            plain.fingerprint(),
            "the slab stays as it was"
        );
    }

    #[test]
    fn a_slab_edge_in_a_wall_is_cut_by_it_in_3d_as_in_the_drawings() {
        // A4-6: a floor slab whose edge runs into the wall below it, with
        // no junction rule: the wall runs through, so nothing of the slab
        // is left inside the wall in 3D, as in plans and sections
        use creabim_core::{Element, Level, Slab, Transaction, Wall};
        let l0 = Element::new(
            "L0",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let l1 = Element::new(
            "L1",
            ElementKind::Level(Level {
                elevation: 3.0,
                height: 3.0,
            }),
        );
        let wall = Element::new(
            "Wall",
            ElementKind::Wall(Wall::new(
                l0.id,
                DVec2::ZERO,
                DVec2::new(6.0, 0.0),
                0.3,
                3.0,
            )),
        );
        let slab = Element::new(
            "Slab",
            ElementKind::Slab(Slab::new(
                l1.id,
                vec![
                    DVec2::new(0.0, 0.0),
                    DVec2::new(6.0, 0.0),
                    DVec2::new(6.0, 4.0),
                    DVec2::new(0.0, 4.0),
                ],
                0.25,
            )),
        );
        let (wid, sid) = (wall.id, slab.id);
        let mut doc = creabim_core::document::Document::new();
        doc.commit(
            Transaction::new("setup")
                .insert(l0)
                .insert(l1)
                .insert(wall)
                .insert(slab),
        )
        .unwrap();
        let items = build_scene_of(doc.model(), None);
        let (lo, hi) = items
            .iter()
            .filter(|i| i.id == wid)
            .filter_map(|i| i.mesh.bounds())
            .next()
            .unwrap();
        let slab_mesh = &items.iter().find(|i| i.id == sid).unwrap().mesh;
        let (slo, _) = slab_mesh.bounds().unwrap();
        // its edge now stops at the wall's face, not its middle
        assert!(
            (slo.y - hi.y).abs() < 1e-3,
            "slab starts at y {} (wall face {})",
            slo.y,
            hi.y
        );
        for t in slab_mesh.indices.chunks_exact(3) {
            let c = t
                .iter()
                .map(|k| glam::Vec3::from(slab_mesh.positions[*k as usize]))
                .sum::<glam::Vec3>()
                / 3.0;
            let inside = (c - lo).min_element() > 0.002 && (hi - c).min_element() > 0.002;
            assert!(
                !inside,
                "a slab face at {c} is inside the wall ({lo}..{hi})"
            );
        }
    }

    #[test]
    fn slab_volume_matches_area_times_thickness() {
        let l = [
            DVec2::new(0.0, 0.0),
            DVec2::new(4.0, 0.0),
            DVec2::new(4.0, 2.0),
            DVec2::new(2.0, 2.0),
            DVec2::new(2.0, 4.0),
            DVec2::new(0.0, 4.0),
        ];
        let m = slab_mesh(&l, 0.0, 0.25);
        assert!((m.volume() - 12.0 * 0.25).abs() < 1e-5);
        let mut cw = l.to_vec();
        cw.reverse();
        assert!((slab_mesh(&cw, 0.0, 0.25).volume() - 3.0).abs() < 1e-5);
    }

    #[test]
    fn a_slab_top_offset_lifts_its_mesh_and_arcs_round_it() {
        let mut model = creabim_core::sample::sample_house();
        let (id, elevation, t) = model
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Slab(s) => Some((e.id, model.level(s.level)?.elevation, s.thickness)),
                _ => None,
            })
            .unwrap();
        let mut e = model.get(id).unwrap().clone();
        let ElementKind::Slab(s) = &mut e.kind else {
            panic!()
        };
        // a 4 x 2 m slab whose far side is a half circle, its top 0.6 m up,
        // far from any wall (walls cut what of a slab runs into them, A4-6)
        let o = DVec2::splat(100.0);
        s.boundary = vec![
            o + DVec2::new(0.0, 0.0),
            o + DVec2::new(4.0, 0.0),
            o + DVec2::new(4.0, 2.0),
            o + DVec2::new(0.0, 2.0),
        ];
        s.bulges = vec![0.0, 1.0, 0.0, 0.0];
        s.top_offset = 0.6;
        creabim_core::Transaction::new("edit")
            .replace(model.get(id).unwrap().clone(), e)
            .apply(&mut model)
            .unwrap();
        let scene = build_scene(&model);
        let item = scene.iter().find(|i| i.id == id).unwrap();
        let (lo, hi) = item.mesh.bounds().unwrap();
        assert!((hi.z as f64 - (elevation + 0.6)).abs() < 1e-5);
        assert!((lo.z as f64 - (elevation + 0.6 - t)).abs() < 1e-5);
        // the arc bulges a metre past the straight side
        assert!((hi.x - 105.0).abs() < 1e-4);
        let area = 8.0 + std::f64::consts::PI / 2.0;
        assert!((item.mesh.volume() - area * t).abs() < 0.01);
    }

    #[test]
    fn a_slab_with_a_hole_loses_its_volume() {
        let outer = creabim_core::polygon::rectangle(DVec2::ZERO, DVec2::new(4.0, 4.0));
        let hole = creabim_core::polygon::rectangle(DVec2::new(1.0, 1.0), DVec2::new(2.0, 3.0));
        let m = slab_mesh_with_holes(&outer, std::slice::from_ref(&hole), 0.0, 0.25);
        assert!((m.volume() - 14.0 * 0.25).abs() < 1e-5, "{}", m.volume());
        // the plan cut at mid-depth has the hole in it
        let cut = crate::slice::slice(&m, -0.1);
        assert_eq!(cut.len(), 1);
        let area = crate::triangulate::signed_area2(&cut[0]) / 2.0;
        assert!((area - 14.0).abs() < 1e-5, "{area}");
    }

    #[test]
    fn edit_mesh_corners_are_welded_and_faces_stay_flat() {
        // a box whose faces each carry their own copies of the corners, as
        // an imported mesh does
        let cube = creabim_core::EditMesh::cuboid(DVec3::new(2.0, 2.0, 2.0));
        let mut split = creabim_core::EditMesh::default();
        for f in &cube.faces {
            let base = split.vertices.len() as u32;
            split
                .vertices
                .extend(f.iter().map(|&v| cube.vertices[v as usize]));
            split
                .faces
                .push((0..f.len() as u32).map(|i| base + i).collect());
        }
        let m = edit_mesh_to_tri(&split, DVec3::ZERO);
        assert!(m.is_closed());
        // eight corners, three face directions each
        assert_eq!(m.positions.len(), 24);
        let mut corners: Vec<[u32; 3]> = m.positions.iter().map(|p| p.map(f32::to_bits)).collect();
        corners.sort_unstable();
        corners.dedup();
        assert_eq!(corners.len(), 8);
        for t in m.indices.chunks(3) {
            let n = |i: u32| m.normals[i as usize];
            assert!(n(t[0]) == n(t[1]) && n(t[1]) == n(t[2]), "flat faces");
        }
        // a face split in two shares the corners along the split
        let mut halves = cube.clone();
        let top = halves
            .faces
            .iter()
            .position(|f| f.iter().all(|&v| cube.vertices[v as usize].z > 1.0))
            .unwrap();
        let q = halves.faces.remove(top);
        halves.faces.push(vec![q[0], q[1], q[2]]);
        halves.faces.push(vec![q[0], q[2], q[3]]);
        let m = edit_mesh_to_tri(&halves, DVec3::ZERO);
        assert_eq!(m.positions.len(), 24);
        assert!(m.is_closed());
        // and the plan cut of the closed box is one loop
        let cut = crate::slice::slice(&m, 1.0);
        assert_eq!(cut.len(), 1);
    }

    #[test]
    fn edit_mesh_volume_is_preserved() {
        let mut e = creabim_core::EditMesh::cuboid(DVec3::new(2.0, 2.0, 2.0));
        e.extrude_face(1, 1.0);
        let m = edit_mesh_to_tri(&e, DVec3::new(10.0, 0.0, 0.0));
        assert!((m.volume() - 12.0).abs() < 1e-4);
    }

    #[test]
    fn window_reveals_show_the_finish_not_the_insulation() {
        let m = creabim_core::starter::starter_house();
        let scene = build_scene(&m);
        let linings: Vec<&SceneItem> = scene.iter().filter(|i| i.is_lining()).collect();
        assert!(!linings.is_empty());
        let insulation = layer_color(creabim_core::LayerFunction::Insulation, [0.0; 3]);
        let insulation_materials: Vec<[f32; 3]> = scene
            .iter()
            .filter(|i| {
                i.layer
                    .is_some_and(|l| l.function == creabim_core::LayerFunction::Insulation)
            })
            .map(|i| i.color)
            .collect();
        for l in &linings {
            assert_ne!(l.color, insulation);
            assert!(!insulation_materials.contains(&l.color), "{:?}", l.color);
            // thin, closed boxes
            assert!(l.mesh.volume() > 0.0);
        }
        // drawings and exports leave them out
        assert!(!drawn_scene(&m).iter().any(|i| i.is_lining()));
        let walls: f64 = element_meshes(&scene).values().map(|m| m.volume()).sum();
        let without: Vec<SceneItem> = scene.iter().filter(|i| !i.is_lining()).cloned().collect();
        let plain: f64 = element_meshes(&without).values().map(|m| m.volume()).sum();
        assert!((walls - plain).abs() < 1e-9);
    }

    #[test]
    fn a_stair_short_of_headroom_shows_its_shortfall_in_red() {
        use creabim_core::members::StairLine;
        use creabim_core::{Document, Element, Level, Slab, Stair, Transaction};
        let mut doc = Document::new();
        let lv = |n: &str, z: f64| {
            Element::new(
                n,
                ElementKind::Level(Level {
                    elevation: z,
                    height: 3.0,
                }),
            )
        };
        let (g, f) = (lv("Ground", 0.0), lv("First", 3.0));
        let floor = Slab::new(
            f.id,
            vec![
                DVec2::new(-2.0, -3.0),
                DVec2::new(10.0, -3.0),
                DVec2::new(10.0, 3.0),
                DVec2::new(-2.0, 3.0),
            ],
            0.25,
        );
        let s =
            Stair::from_line(g.id, &[DVec2::ZERO, DVec2::new(6.0, 0.0)], StairLine::Walk).unwrap();
        let st = Element::new("Stair", ElementKind::Stair(s));
        let id = st.id;
        let tx = Transaction::new("t")
            .insert(g)
            .insert(f)
            .insert(Element::new("Floor", ElementKind::Slab(floor)))
            .insert(st);
        doc.commit(tx).unwrap();
        let red = |doc: &Document| {
            build_scene(doc.model())
                .into_iter()
                .filter(|i| i.id == id && i.finish == Some("Headroom"))
                .count()
        };
        assert_eq!(red(&doc), 1);
        doc.commit(creabim_core::stairwell::cut_well(doc.model(), id, 0.0).unwrap())
            .unwrap();
        assert_eq!(red(&doc), 0);
    }

    // bruwy, #85: a slab cut by wall after wall stays compact and whole
    #[test]
    fn successive_wall_joins_keep_the_slab_mesh_compact() {
        use creabim_core::{Element, Level, Slab, Transaction, Wall};
        let level = Element::new(
            "Level",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let mut slab = Slab::new(
            level.id,
            vec![
                DVec2::ZERO,
                DVec2::new(11.5, 0.0),
                DVec2::new(11.5, 11.0),
                DVec2::new(0.0, 11.0),
            ],
            0.14,
        );
        slab.top_offset = 1.30;
        let slab = Element::new("Slab", ElementKind::Slab(slab));
        let slab_id = slab.id;
        let level_id = level.id;
        let mut transaction = Transaction::new("setup").insert(level).insert(slab);
        for i in 0..12 {
            let p = 0.3 + 10.0 * f64::from(i) / 12.0;
            let (start, end) = if i % 2 == 0 {
                (DVec2::new(-0.2, p), DVec2::new(11.7, p))
            } else {
                (DVec2::new(p, -0.2), DVec2::new(p, 11.7))
            };
            let mut wall = Wall::new(level_id, start, end, 0.16, 3.1);
            wall.disallow_join = [true; 2];
            transaction = transaction.insert(Element::new("Wall", ElementKind::Wall(wall)));
        }
        let mut doc = creabim_core::document::Document::new();
        doc.commit(transaction).unwrap();
        let items = build_scene(doc.model());
        let mesh = &items.iter().find(|item| item.id == slab_id).unwrap().mesh;
        let triangles = mesh.indices.len() / 3;
        // 852 as the cuts leave it (bruwy's bound was 1 000); 1 116 once
        // its seams are mended (MU143), which adds triangles; 3 814 when
        // each cut was made triangles before the next
        assert!(
            triangles < 1_500,
            "the joined slab has {triangles} triangles"
        );
        let expected = (11.5 - 6.0 * 0.16) * (11.0 - 6.0 * 0.16) * 0.14;
        assert!((mesh.volume() - expected).abs() < 1e-4);
        let rebuilt = build_scene(doc.model());
        let cached = &rebuilt.iter().find(|item| item.id == slab_id).unwrap().mesh;
        assert_eq!(cached.positions, mesh.positions);
        assert_eq!(cached.indices, mesh.indices);
    }

    #[test]
    fn a_later_wall_join_keeps_a_fully_cut_slab_empty() {
        use creabim_core::{Element, Level, Slab, Transaction, Wall};
        let level = Element::new(
            "Level",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let mut slab = Slab::new(
            level.id,
            vec![DVec2::ZERO, DVec2::X, DVec2::ONE, DVec2::Y],
            0.14,
        );
        slab.top_offset = 1.30;
        let slab = Element::new("Slab", ElementKind::Slab(slab));
        let slab_id = slab.id;
        let mut covering = Wall::new(
            level.id,
            DVec2::new(-1.0, 0.5),
            DVec2::new(2.0, 0.5),
            2.0,
            3.1,
        );
        let mut crossing = Wall::new(
            level.id,
            DVec2::new(0.6, -1.0),
            DVec2::new(0.6, 2.0),
            0.16,
            3.1,
        );
        covering.disallow_join = [true; 2];
        crossing.disallow_join = [true; 2];
        let mut doc = creabim_core::document::Document::new();
        doc.commit(
            Transaction::new("setup")
                .insert(level)
                .insert(slab)
                .insert(Element::new("Covering", ElementKind::Wall(covering)))
                .insert(Element::new("Crossing", ElementKind::Wall(crossing))),
        )
        .unwrap();
        let items = build_scene(doc.model());
        let mesh = &items.iter().find(|item| item.id == slab_id).unwrap().mesh;
        assert!(mesh.is_empty());
    }
}
