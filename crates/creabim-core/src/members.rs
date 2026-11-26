//! Building elements with a parametric shape: columns, beams, roofs, stairs
//! and railings. Each one hangs off a level, like walls and slabs, and is
//! described by a few numbers; their meshes are built in `creabim-geom`.
//!
//! This module holds the data and the plain geometry (stair risers and
//! flights, roof planes, railing paths) so it can be tested without meshes.

use crate::{Element, ElementId, ElementKind, Model, Xform};
use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

/// The cross-section of a column.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Profile {
    /// `width` along the column's own x axis, `depth` along its y.
    Rectangle {
        width: f64,
        depth: f64,
    },
    Round {
        diameter: f64,
    },
}

impl Profile {
    /// The outline in plan, counter-clockwise, about `center` and turned
    /// `rotation` degrees. Round profiles come back as 24 sides.
    pub fn outline(&self, center: DVec2, rotation: f64) -> Vec<DVec2> {
        let turn = DVec2::from_angle(rotation.to_radians());
        match *self {
            Profile::Rectangle { width, depth } => {
                let (x, y) = (width / 2.0, depth / 2.0);
                [(-x, -y), (x, -y), (x, y), (-x, y)]
                    .iter()
                    .map(|&(a, b)| center + turn.rotate(DVec2::new(a, b)))
                    .collect()
            }
            Profile::Round { diameter } => (0..ROUND_SIDES)
                .map(|i| {
                    let a = std::f64::consts::TAU * i as f64 / ROUND_SIDES as f64;
                    center + DVec2::from_angle(a) * diameter / 2.0
                })
                .collect(),
        }
    }

    /// Cross-section area, exact for round profiles.
    pub fn area(&self) -> f64 {
        match *self {
            Profile::Rectangle { width, depth } => width * depth,
            Profile::Round { diameter } => std::f64::consts::PI * diameter * diameter / 4.0,
        }
    }

    /// "300 × 300" or "Ø 300", in millimetres.
    pub fn label(&self) -> String {
        self.label_in(&crate::Units::metric())
    }

    /// The label in a project's units: "300 × 300", "1'-0\" × 1'-0\"".
    pub fn label_in(&self, units: &crate::Units) -> String {
        match *self {
            Profile::Rectangle { width, depth } => {
                format!("{} × {}", units.dimension(width), units.dimension(depth))
            }
            Profile::Round { diameter } => format!("Ø {}", units.dimension(diameter)),
        }
    }

    fn check(&self) -> Result<(), String> {
        match *self {
            Profile::Rectangle { width, depth } => {
                positive(width, "column width")?;
                positive(depth, "column depth")
            }
            Profile::Round { diameter } => positive(diameter, "column diameter"),
        }
    }
}

/// Sides of the polygon a round column is drawn and meshed with.
pub const ROUND_SIDES: usize = 24;

/// A vertical column standing on a level.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub level: ElementId,
    /// Centre of the profile in plan.
    pub position: DVec2,
    pub profile: Profile,
    /// Turn of the profile, degrees counter-clockwise.
    #[serde(default)]
    pub rotation: f64,
    /// Height of the base above the level.
    #[serde(default)]
    pub base_offset: f64,
    /// A fixed height; `None` runs up to the next level (or the level's own
    /// height when there is none above).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(default)]
    pub material: Option<ElementId>,
    /// The top constraint: tied to a level (`offset` above it), the top
    /// follows that level, so a column can run from its base level up
    /// through several storeys and stretch when either level moves. A top
    /// level that is gone leaves the column as `height` says.
    #[serde(default, skip_serializing_if = "crate::WallTop::is_unconnected")]
    pub top: crate::WallTop,
    /// A section from the profile library, over `profile` (MU37): its
    /// insertion point on the column's position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<Section>,
}

/// A library profile a member is made with: its name, and its outline as
/// it was when last updated from the library (so editing the profile
/// changes nothing until approved, MU37).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub name: String,
    pub points: Vec<DVec2>,
}

impl Column {
    pub fn new(level: ElementId, position: DVec2, profile: Profile) -> Self {
        Self {
            level,
            position,
            profile,
            rotation: 0.0,
            base_offset: 0.0,
            height: None,
            material: None,
            top: crate::WallTop::Unconnected,
            section: None,
        }
    }

    /// The height as built: up to its top level, the fixed one, or up to
    /// the level above.
    pub fn height_in(&self, model: &Model) -> f64 {
        if let Some(top) = self.constrained_top(model) {
            return (top - elevation(model, self.level) - self.base_offset).max(0.01);
        }
        self.height
            .unwrap_or_else(|| storey_height(model, self.level) - self.base_offset)
            .max(0.01)
    }

    /// The absolute height of the top its top level gives it, when it is
    /// tied to a level that exists.
    pub fn constrained_top(&self, model: &Model) -> Option<f64> {
        match self.top {
            crate::WallTop::UpToLevel { level, offset } => {
                Some(model.level(level)?.elevation + offset)
            }
            crate::WallTop::Unconnected => None,
        }
    }

    /// The level its top is tied to, if any.
    pub fn top_level(&self) -> Option<ElementId> {
        match self.top {
            crate::WallTop::UpToLevel { level, .. } => Some(level),
            crate::WallTop::Unconnected => None,
        }
    }

    /// Absolute heights of the base and the top.
    pub fn span(&self, model: &Model) -> (f64, f64) {
        let z0 = elevation(model, self.level) + self.base_offset;
        (z0, z0 + self.height_in(model))
    }

    /// Stand it on `level`, its top staying where it is: a top that ran up
    /// to the next level is tied to that level, a fixed height grows or
    /// shrinks. So a column drawn on one level can be stood on a lower one
    /// and still reach as high as before.
    pub fn set_base_level(&mut self, model: &Model, level: ElementId) {
        if level == self.level || model.level(level).is_none() {
            return;
        }
        let (_, z1) = self.span(model);
        if self.top.is_unconnected() && self.height.is_none() {
            if let Some(next) = level_above(model, self.level) {
                self.top = crate::WallTop::UpToLevel {
                    level: next,
                    offset: 0.0,
                };
            }
        }
        if self.top.is_unconnected() {
            let z0 = elevation(model, level) + self.base_offset;
            self.height = Some((z1 - z0).max(0.1));
        }
        self.level = level;
    }

    /// Tie its top to `level`, `offset` above it, or (with `None`) let it
    /// go, keeping the height it has.
    pub fn set_top_level(&mut self, model: &Model, level: Option<ElementId>, offset: f64) {
        match level {
            Some(l) => self.top = crate::WallTop::UpToLevel { level: l, offset },
            None => {
                if !self.top.is_unconnected() {
                    self.height = Some(self.height_in(model));
                }
                self.top = crate::WallTop::Unconnected;
            }
        }
    }

    /// Does it pass through the plan of `level` cut `cut` above it? A
    /// column standing on another level shows there when it runs up
    /// through the cut.
    pub fn crosses(&self, model: &Model, level: ElementId, cut: f64) -> bool {
        let Some(l) = model.level(level) else {
            return false;
        };
        let (z0, z1) = self.span(model);
        let z = l.elevation + cut;
        z0 < z && z < z1
    }

    pub fn outline(&self) -> Vec<DVec2> {
        match &self.section {
            Some(s) if s.points.len() >= 3 => {
                let turn = DVec2::from_angle(self.rotation.to_radians());
                let mut pts: Vec<DVec2> = s
                    .points
                    .iter()
                    .map(|p| self.position + turn.rotate(*p))
                    .collect();
                if crate::polygon::signed_area2(&pts) < 0.0 {
                    pts.reverse();
                }
                pts
            }
            _ => self.profile.outline(self.position, self.rotation),
        }
    }
}

/// A horizontal beam between two points, hung from the top of its storey.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Beam {
    pub level: ElementId,
    pub start: DVec2,
    pub end: DVec2,
    pub width: f64,
    /// Height of the section.
    pub depth: f64,
    /// How far the top sits below the top of the storey (the next level,
    /// or the level's height).
    #[serde(default)]
    pub top_offset: f64,
    #[serde(default)]
    pub material: Option<ElementId>,
    /// A section from the profile library (MU37), hung from its insertion
    /// point on the beam's top line; `width` and `depth` follow it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<Section>,
    /// How far each end's top sits above the beam's top (MU40): a sloped
    /// rafter, a purlin up a roof face, a brace in a wall.
    #[serde(default, skip_serializing_if = "is_zero2")]
    pub rise: [f64; 2],
    /// The section's up, square to the face it was drawn on (MU40: a beam
    /// on a roof slope lies in the slope); none, plumb.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub up: Option<DVec3>,
    /// How far each end runs past its point (negative: pulled back, where
    /// a column or wall runs through).
    #[serde(default, skip_serializing_if = "is_zero2")]
    pub extend: [f64; 2],
    /// How each end is cut: square, or along a plane (a mitre with the
    /// next beam of its chain), by that plane's normal.
    #[serde(default, skip_serializing_if = "no_cuts")]
    pub cuts: [Option<DVec3>; 2],
}

fn is_zero2(v: &[f64; 2]) -> bool {
    v[0] == 0.0 && v[1] == 0.0
}

fn no_cuts(v: &[Option<DVec3>; 2]) -> bool {
    v[0].is_none() && v[1].is_none()
}

/// How the beams of a chain meet at its corners (MU40).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BeamCorner {
    /// Each runs to the corner point, as concrete pieces poured apart.
    #[default]
    Separate,
    /// Cut along the line halfway between them, as welded steel.
    Mitred,
    /// The first runs through, the second stops against it.
    Butt,
}

impl BeamCorner {
    pub const ALL: [BeamCorner; 3] = [BeamCorner::Separate, BeamCorner::Mitred, BeamCorner::Butt];

    pub fn label(self) -> &'static str {
        match self {
            BeamCorner::Separate => "Separate",
            BeamCorner::Mitred => "Mitred",
            BeamCorner::Butt => "Butt",
        }
    }

    /// Its icon in the beam tool's bar and menu.
    pub fn icon(self) -> &'static str {
        match self {
            BeamCorner::Separate => "corner-apart",
            BeamCorner::Mitred => "corner-mitre",
            BeamCorner::Butt => "corner-butt",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            BeamCorner::Separate => "Each runs to the corner point, as concrete pieces",
            BeamCorner::Mitred => "Both cut on the line halfway between them, as welded steel",
            BeamCorner::Butt => "The first runs through, the next stops against it",
        }
    }
}

/// Meet beams `a` (ending at the corner) and `b` (starting there) as
/// `how` says: their end cuts and extensions.
pub fn join_beams(a: &mut Beam, b: &mut Beam, how: BeamCorner) {
    let da = (a.end - a.start).normalize_or_zero();
    match how {
        BeamCorner::Separate => {
            a.cuts[1] = None;
            b.cuts[0] = None;
            a.extend[1] = 0.0;
            b.extend[0] = 0.0;
        }
        BeamCorner::Mitred => {
            // the plane halfway between the two runs in 3D, through the
            // corner: on a roof slope it leans with the slope (MU40)
            let run = |x: &Beam| {
                (x.end - x.start)
                    .extend(x.rise[1] - x.rise[0])
                    .normalize_or(DVec3::X)
            };
            let n = (run(a) + run(b)).normalize_or(da.extend(0.0));
            // each end slid onto the cut, inside and outside of the turn
            a.cuts[1] = Some(n);
            b.cuts[0] = Some(n);
            a.extend[1] = 0.0;
            b.extend[0] = 0.0;
        }
        BeamCorner::Butt => {
            // the first runs through to the second's far face, the second
            // stops at the first's near face
            a.cuts[1] = None;
            b.cuts[0] = None;
            a.extend[1] = b.width / 2.0;
            b.extend[0] = -a.width / 2.0;
        }
    }
}

impl Beam {
    pub fn new(level: ElementId, start: DVec2, end: DVec2) -> Self {
        Self {
            level,
            start,
            end,
            width: 0.2,
            depth: 0.4,
            top_offset: 0.0,
            material: None,
            section: None,
            rise: [0.0, 0.0],
            up: None,
            extend: [0.0, 0.0],
            cuts: [None, None],
        }
    }

    /// Its top line's ends in 3D, with its rise and extensions.
    pub fn axis(&self, model: &Model) -> (DVec3, DVec3) {
        let top = self.top(model);
        let a = self.start.extend(top + self.rise[0]);
        let b = self.end.extend(top + self.rise[1]);
        let d = (b - a).normalize_or_zero();
        (a - d * self.extend[0], b + d * self.extend[1])
    }

    pub fn length(&self) -> f64 {
        self.start.distance(self.end)
    }

    /// Absolute height of the top face.
    pub fn top(&self, model: &Model) -> f64 {
        elevation(model, self.level) + storey_height(model, self.level) - self.top_offset
    }

    /// The footprint in plan, counter-clockwise, with its extensions.
    pub fn outline(&self) -> Vec<DVec2> {
        let d = (self.end - self.start).normalize_or_zero();
        let (start, end) = (
            self.start - d * self.extend[0],
            self.end + d * self.extend[1],
        );
        // a library section: as wide as it reaches either side
        let (l, r) = match &self.section {
            Some(s) if !s.points.is_empty() => {
                let lo = s.points.iter().map(|p| p.x).fold(f64::MAX, f64::min);
                let hi = s.points.iter().map(|p| p.x).fold(f64::MIN, f64::max);
                (hi.max(0.0), (-lo).max(0.0))
            }
            _ => (self.width / 2.0, self.width / 2.0),
        };
        let (nl, nr) = (d.perp() * l, d.perp() * r);
        vec![start - nr, end - nr, end + nl, start + nl]
    }
}

/// How a roof slopes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoofShape {
    Flat,
    /// One slope, rising from the eave edge.
    Shed,
    /// Two slopes meeting at a ridge, with upright gable ends.
    Gable,
    /// A slope from every edge.
    #[default]
    Hip,
}

impl RoofShape {
    pub const ALL: [RoofShape; 4] = [
        RoofShape::Flat,
        RoofShape::Shed,
        RoofShape::Gable,
        RoofShape::Hip,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RoofShape::Flat => "Flat",
            RoofShape::Shed => "Mono-pitch",
            RoofShape::Gable => "Gable",
            RoofShape::Hip => "Hip",
        }
    }
}

/// A roof over an outline in plan. Its underside meets the outline at
/// `base_offset` above the level, and runs on past it by the overhang.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Roof {
    pub level: ElementId,
    /// The wall-plate outline (where the slopes start), counter-clockwise.
    pub boundary: Vec<DVec2>,
    pub shape: RoofShape,
    /// Slope in degrees from horizontal.
    pub pitch: f64,
    /// How far the roof reaches past the outline, in plan.
    pub overhang: f64,
    /// Thickness measured square to the slope.
    pub thickness: f64,
    #[serde(default)]
    pub base_offset: f64,
    /// The eave a mono-pitch roof rises from, and the edge a gable roof's
    /// slopes run along (the ridge is parallel to it).
    #[serde(default)]
    pub edge: usize,
    #[serde(default)]
    pub material: Option<ElementId>,
    /// The layered assembly the roof is built of, layers listed from the
    /// top down, each measured square to the slope. With one, the roof's
    /// thickness is the assembly's total.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assembly: Option<ElementId>,
    /// Holes cut upright through it (a roof light, a chimney), each a
    /// polygon in plan.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<Vec<DVec2>>,
    /// Layer edges moved by hand at the eaves and verges (see
    /// [`crate::LayerEdge`]); edge `i` is the eave past boundary edge `i`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layer_edges: Vec<crate::LayerEdge>,
}

/// One flat piece of a roof's underside: its outline in plan and the plane
/// `z = c + g · p` (relative to the roof's base).
#[derive(Clone, Debug, PartialEq)]
pub struct RoofFace {
    pub outline: Vec<DVec2>,
    pub g: DVec2,
    pub c: f64,
}

impl RoofFace {
    pub fn z(&self, p: DVec2) -> f64 {
        self.c + self.g.dot(p)
    }
}

impl Roof {
    /// A roof of `shape` over `boundary`, made counter-clockwise, sloping
    /// along its longest edge.
    pub fn new(level: ElementId, boundary: Vec<DVec2>, shape: RoofShape) -> Self {
        let mut boundary = boundary;
        if crate::polygon::signed_area2(&boundary) < 0.0 {
            boundary.reverse();
        }
        let n = boundary.len();
        let edge = (0..n)
            .max_by(|&a, &b| {
                let len = |i: usize| boundary[i].distance(boundary[(i + 1) % n]);
                len(a).total_cmp(&len(b))
            })
            .unwrap_or(0);
        Self {
            layer_edges: vec![],
            level,
            boundary,
            shape,
            pitch: 30.0,
            overhang: 0.5,
            thickness: 0.25,
            base_offset: 0.0,
            edge,
            material: None,
            assembly: None,
            holes: vec![],
        }
    }

    /// The shape actually built: gables and hips need a convex outline;
    /// others are built flat.
    pub fn effective_shape(&self) -> RoofShape {
        match self.shape {
            RoofShape::Gable | RoofShape::Hip if !is_convex(&self.boundary) => RoofShape::Flat,
            s => s,
        }
    }

    /// Absolute height where the underside meets the outline.
    pub fn base(&self, model: &Model) -> f64 {
        elevation(model, self.level) + self.base_offset
    }

    /// Thickness measured upright.
    pub fn vertical_thickness(&self) -> f64 {
        match self.effective_shape() {
            RoofShape::Flat => self.thickness,
            _ => self.thickness / self.pitch.to_radians().cos().max(0.05),
        }
    }

    /// The outline grown by the overhang: where the roof ends in plan.
    pub fn eave_outline(&self) -> Vec<DVec2> {
        offset_polygon(&self.boundary, self.overhang)
    }

    /// The edges a slope rises from.
    pub fn sloped_edges(&self) -> Vec<usize> {
        let n = self.boundary.len();
        let dir = |i: usize| (self.boundary[(i + 1) % n] - self.boundary[i]).normalize_or_zero();
        match self.effective_shape() {
            RoofShape::Flat => vec![],
            RoofShape::Shed => vec![self.edge % n],
            RoofShape::Hip => (0..n).collect(),
            RoofShape::Gable => {
                let r = dir(self.edge % n);
                let cos = 20f64.to_radians().cos();
                (0..n).filter(|&i| dir(i).dot(r).abs() > cos).collect()
            }
        }
    }

    /// The underside as flat pieces, together covering the eave outline.
    /// Layer `layer`'s eaves: the roof's, with any of its edges moved by
    /// hand pushed out or pulled in.
    pub fn layer_eaves(&self, layer: usize) -> Vec<DVec2> {
        let eaves = self.eave_outline();
        let by = crate::layer_edge_offsets(&self.layer_edges, layer, eaves.len());
        offset_edges(&eaves, &by)
    }

    /// The pieces of layer `layer`'s underside: the roof's planes, over
    /// its own eaves.
    pub fn layer_faces(&self, layer: usize) -> Vec<RoofFace> {
        self.faces_over(self.layer_eaves(layer))
    }

    pub fn faces(&self) -> Vec<RoofFace> {
        self.faces_over(self.eave_outline())
    }

    /// The pieces of the underside over outline `eaves`.
    fn faces_over(&self, eaves: Vec<DVec2>) -> Vec<RoofFace> {
        let n = self.boundary.len();
        if n < 3 {
            return vec![];
        }
        let slope = self.pitch.to_radians().tan();
        // the plane rising from edge i: height = slope × distance inside it
        let plane = |i: usize| {
            let a = self.boundary[i];
            let d = (self.boundary[(i + 1) % n] - a).normalize_or_zero();
            let inward = d.perp();
            (inward * slope, -inward.dot(a) * slope)
        };
        let edges = self.sloped_edges();
        if edges.is_empty() {
            return vec![RoofFace {
                outline: eaves,
                g: DVec2::ZERO,
                c: 0.0,
            }];
        }
        if edges.len() == 1 {
            let (g, c) = plane(edges[0]);
            return vec![RoofFace {
                outline: eaves,
                g,
                c,
            }];
        }
        // the underside is the lowest of the planes, each one where it is lowest
        let mut out = Vec::new();
        for (k, &i) in edges.iter().enumerate() {
            let (gi, ci) = plane(i);
            let mut poly = eaves.clone();
            for (m, &j) in edges.iter().enumerate() {
                if i == j {
                    continue;
                }
                let (gj, cj) = plane(j);
                // gi·p + ci <= gj·p + cj, ties to the earlier edge
                let tie = if m < k { -1e-9 } else { 1e-9 };
                poly = clip_half_plane(&poly, gi - gj, cj - ci + tie);
                if poly.len() < 3 {
                    break;
                }
            }
            if poly.len() >= 3 && crate::polygon::signed_area2(&poly).abs() > 1e-9 {
                out.push(RoofFace {
                    outline: poly,
                    g: gi,
                    c: ci,
                });
            }
        }
        out
    }

    /// Height of the underside above the base at `p` (inside the eaves).
    pub fn underside_at(&self, p: DVec2) -> f64 {
        let faces = self.faces();
        faces
            .iter()
            .map(|f| f.z(p))
            .fold(f64::INFINITY, f64::min)
            .min(if faces.is_empty() { 0.0 } else { f64::INFINITY })
    }

    /// Height of the top of the roof above its base.
    pub fn peak(&self) -> f64 {
        self.faces()
            .iter()
            .flat_map(|f| f.outline.iter().map(|p| f.z(*p)).collect::<Vec<_>>())
            .fold(0.0, f64::max)
            + self.vertical_thickness()
    }

    /// Plan area covered, overhang included.
    pub fn plan_area(&self) -> f64 {
        crate::polygon::signed_area2(&self.eave_outline()).abs() / 2.0
    }
}

/// A turn in an L-shaped stair: a square landing after `after` risers.
/// Kept for stairs made before stairs were drawn by a line; a stair with
/// a `line` turns at the line's corners instead.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Landing {
    pub after: u32,
    /// The second flight turns to the left of the first (else right).
    pub left: bool,
}

/// Which line of a stair its drawn `line` is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StairLine {
    /// The walk line, down the middle.
    #[default]
    Walk,
    /// The stair's left side, looking up it.
    Left,
    /// Its right side.
    Right,
}

impl StairLine {
    fn is_walk(&self) -> bool {
        *self == StairLine::Walk
    }

    /// How far the walk line lies to the left of this line.
    pub fn to_walk(self, width: f64) -> f64 {
        match self {
            StairLine::Walk => 0.0,
            StairLine::Left => -width / 2.0,
            StairLine::Right => width / 2.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            StairLine::Walk => "Walk line",
            StairLine::Left => "Left side",
            StairLine::Right => "Right side",
        }
    }
}

/// A stair climbing from its level to the next.
///
/// It is drawn by a line (#65): its walk line or one of its sides, with a
/// landing at each corner. The risers share the rise equally and fill the
/// line from its start; a line longer than the climb leaves some spare at
/// its end, and one too short has its last flight run on past it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stair {
    pub level: ElementId,
    /// Where its line starts, in plan: the middle of the first riser for
    /// a stair drawn by its walk line.
    pub start: DVec2,
    /// Which way the first flight climbs, degrees counter-clockwise from x.
    pub direction: f64,
    pub width: f64,
    /// Tallest riser allowed; the risers share the height equally.
    pub max_riser: f64,
    /// Going: the depth of each tread.
    pub tread: f64,
    /// A fixed rise; `None` climbs to the next level (or the level's height).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(default)]
    pub base_offset: f64,
    /// Thickness of the sloping slab under the steps.
    #[serde(default = "default_waist")]
    pub waist: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landing: Option<Landing>,
    #[serde(default)]
    pub material: Option<ElementId>,
    /// The line it was drawn by, in its own frame: from `start`, x along
    /// `direction`, so moving or turning the stair takes its line along.
    /// Its first point is the origin. Empty for a stair from before lines,
    /// which keeps `landing`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line: Vec<DVec2>,
    /// How far each straight of `line` bends into an arc: the angle in
    /// degrees its direction turns through from one end to the other, to
    /// the left when positive. Missing or 0: straight. A spiral goes round
    /// in arcs of up to 180° each.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bends: Vec<f64>,
    /// Which line of the stair `line` is.
    #[serde(default, skip_serializing_if = "StairLine::is_walk")]
    pub drawn: StairLine,
    /// Changes made to single flights and landings, kept by the straight
    /// or the turn of the line they belong to, so the stair keeps them
    /// when it fills its line again.
    #[serde(default, skip_serializing_if = "StairPieces::is_empty")]
    pub pieces: StairPieces,
    /// What its turns become, unless a turn was given its own.
    #[serde(default, skip_serializing_if = "TurnKind::is_landing")]
    pub at_turns: TurnKind,
    /// How it is built: the shape stays, the structure under the treads
    /// changes.
    #[serde(default, skip_serializing_if = "StairBuild::is_concrete")]
    pub build: StairBuild,
    /// What its treads, risers and landings are finished in, over the
    /// structure.
    #[serde(default, skip_serializing_if = "StairFinish::is_none")]
    pub finish: StairFinish,
    /// The rule set it is checked against.
    #[serde(
        default,
        skip_serializing_if = "crate::stairrules::StairCode::is_default"
    )]
    pub rules: crate::stairrules::StairCode,
    /// A ramp, not steps: the steepest slope it may have, rise over run
    /// (1:12 is 0.0833). A ramp is laid out as a stair of very short
    /// goings ([`RAMP_SLICE`]), so its line, landings, railings and checks
    /// are a stair's; it is drawn and built as one smooth slope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ramp: Option<f64>,
    /// A copy of this stair on a typical floor: the stair at the foot of
    /// the stack, which it is kept the same as (see [`crate::stairstack`]).
    /// Soft: when that stair goes, the lowest copy left takes its place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typical: Option<ElementId>,
    /// A split stair: its line is one arm, and the stair is that arm and
    /// its mirror image side by side, sharing one flight twice as wide up
    /// to the first landing, then parting (see [`Stair::mirror`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub split: bool,
    /// The end that stays put when its treads change: its foot (else its
    /// head, where people arrive, against the floor above: REV00 §3, A).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fixed_foot: bool,
    /// What it had as steps before it was made a ramp, so that making it
    /// steps again gives the same stair back (MU103: Ramp ticked and
    /// unticked lost its treads, winders, construction and finish).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_steps: Option<Box<StepsKept>>,
    /// Where its UP label stands, moved from its place before the first
    /// flight: the offset in plan, metres; none keeps it before the flight
    /// (MU144, owner: "just the stair tag bubble, we don't want to touch
    /// the stair arrow").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub up_offset: Option<DVec2>,
}

/// A stair's steps, kept while it is a ramp.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StepsKept {
    pub tread: f64,
    pub max_riser: f64,
    pub at_turns: TurnKind,
    pub pieces: StairPieces,
    pub build: StairBuild,
    pub finish: StairFinish,
}

/// The line a split stair is mirrored across: along its first flight, on
/// the side away from its first turn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mirror {
    pub origin: DVec2,
    pub dir: DVec2,
    /// Which side of the drawn arm runs along the mirror line.
    pub left: bool,
    /// How far from the first riser, along `dir`, the first landing
    /// starts: the shared flight is the line's length up to here.
    pub shared: f64,
    /// The width of each half of the shared flight (the stair's width).
    pub width: f64,
}

impl Mirror {
    /// Whether `p` is over the shared flight, before the landing.
    pub fn on_shared(&self, p: DVec2) -> bool {
        let q = p - self.origin;
        let along = q.dot(self.dir);
        along > -1e-6
            && along < self.shared + 1e-6
            && q.perp_dot(self.dir).abs() < self.width + 1e-6
    }
}

impl Mirror {
    /// `p` mirrored across the line.
    pub fn pt(&self, p: DVec2) -> DVec2 {
        let q = p - self.origin;
        self.origin + 2.0 * q.dot(self.dir) * self.dir - q
    }

    pub fn pt3(&self, p: DVec3) -> DVec3 {
        self.pt(p.truncate()).extend(p.z)
    }
}

/// The going a ramp is laid out in: short enough that its slope reads as
/// smooth and its flights end within a few centimetres of where a smooth
/// slope would.
pub const RAMP_SLICE: f64 = 0.05;

/// The finishes over a stair's structure. Its risers are measured finish
/// to finish: the treads' finished tops are a riser apart, and the
/// structure sits the tread finish lower.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StairFinish {
    /// The finish on each tread, landing and winder, metres thick (0:
    /// none, the structure is the walking surface).
    #[serde(default)]
    pub tread: f64,
    #[serde(default)]
    pub tread_material: Option<ElementId>,
    /// The finish on each riser's face, metres thick. A stair with open
    /// risers has none.
    #[serde(default)]
    pub riser: f64,
    #[serde(default)]
    pub riser_material: Option<ElementId>,
    /// How far each tread's finish runs out past the riser below it.
    #[serde(default)]
    pub nosing: f64,
}

/// What a stair takes to build, as a builder would measure it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StairQuantities {
    /// The structure's volume, m³: concrete, plate, or boards and
    /// members.
    pub structure: f64,
    /// The plan area its treads, landings and winders cover, m².
    pub treads: f64,
    /// The area of its risers' faces, m² (none where they're open).
    pub risers: f64,
    /// Its run along the walk line, m.
    pub run: f64,
}

impl StairFinish {
    pub fn is_none(&self) -> bool {
        *self == StairFinish::default()
    }

    /// The tread finish's thickness where there is one.
    pub fn tread(&self) -> f64 {
        self.tread.max(0.0)
    }

    /// The riser finish's thickness on a stair built `build`: none where
    /// its risers are open.
    pub fn riser(&self, build: StairBuild) -> f64 {
        if build.open() {
            0.0
        } else {
            self.riser.max(0.0)
        }
    }
}

/// How a stair is built: what carries its treads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StairBuild {
    /// A concrete slab under the steps, its underside smooth: a sloping
    /// plane under a straight flight, a helix under a curved one. Its
    /// thickness is the stair's waist.
    #[default]
    Concrete,
    /// Concrete whose underside steps with the treads, a waist below them.
    Stepped,
    /// A plate of the waist's thickness folded up each riser and along
    /// each tread.
    FoldedPlate,
    /// Treads between a stringer along each side.
    Stringers,
    /// Treads sitting on a stringer cut to the steps along each side.
    CutStringers,
    /// Treads on one beam up the middle, under the walk line.
    Spine,
    /// Treads held out from a wall on one side, nothing under them.
    Cantilever,
    /// Treads round a post at the centre of its curve: a spiral stair.
    Post,
}

impl StairBuild {
    pub const ALL: [StairBuild; 8] = [
        StairBuild::Concrete,
        StairBuild::Stepped,
        StairBuild::FoldedPlate,
        StairBuild::Stringers,
        StairBuild::CutStringers,
        StairBuild::Spine,
        StairBuild::Cantilever,
        StairBuild::Post,
    ];

    pub fn is_concrete(&self) -> bool {
        *self == StairBuild::Concrete
    }

    pub fn label(self) -> &'static str {
        match self {
            StairBuild::Concrete => "Concrete, smooth underside",
            StairBuild::Stepped => "Concrete, stepped underside",
            StairBuild::FoldedPlate => "Folded plate",
            StairBuild::Stringers => "Side stringers",
            StairBuild::CutStringers => "Cut stringers",
            StairBuild::Spine => "Central spine",
            StairBuild::Cantilever => "Cantilevered treads",
            StairBuild::Post => "Spiral post",
        }
    }

    /// Are its treads separate boards or plates, open between them (no
    /// risers), rather than solid steps?
    pub fn open(self) -> bool {
        matches!(
            self,
            StairBuild::Stringers
                | StairBuild::CutStringers
                | StairBuild::Spine
                | StairBuild::Cantilever
                | StairBuild::Post
        )
    }
}

/// How thick a tread board or plate is on an open stair.
pub const TREAD_BOARD: f64 = 0.04;
/// A side stringer's thickness and its depth square to the pitch.
pub const STRINGER: (f64, f64) = (0.05, 0.28);
/// A central spine's width and depth.
pub const SPINE: (f64, f64) = (0.2, 0.25);
/// A spiral stair's post radius.
pub const POST_RADIUS: f64 = 0.1;

/// What a turn of a line stair becomes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnKind {
    /// A level landing.
    #[default]
    Landing,
    /// Winders, this many (0: one for each 30° of the turn, as the
    /// National Building Code of Canada's dwelling winders, 9.8.4.5).
    Winders(u32),
}

impl TurnKind {
    pub fn is_landing(&self) -> bool {
        *self == TurnKind::Landing
    }
}

/// A turn given its own kind.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TurnEdit {
    /// The turn of the line, from 0.
    pub turn: u32,
    pub kind: TurnKind,
}

/// A flight given its own number of treads.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlightEdit {
    /// The straight of the line it runs along, from 0.
    pub straight: u32,
    pub treads: u32,
}

/// A landing made deeper than the line leaves it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LandingEdit {
    /// The turn of the line it sits at, from 0.
    pub turn: u32,
    /// How much sooner the flight before it stops.
    pub deeper: f64,
}

/// What was changed on single pieces of a line stair.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StairPieces {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flights: Vec<FlightEdit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub landings: Vec<LandingEdit>,
    /// Turns given their own kind: winders where the stair has landings,
    /// or a landing where it has winders.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub turns: Vec<TurnEdit>,
}

impl StairPieces {
    pub fn is_empty(&self) -> bool {
        self.flights.is_empty() && self.landings.is_empty() && self.turns.is_empty()
    }

    /// The kind turn `turn` was given, if any.
    pub fn turn_kind(&self, turn: usize) -> Option<TurnKind> {
        self.turns
            .iter()
            .find(|t| t.turn as usize == turn)
            .map(|t| t.kind)
    }

    /// Give turn `turn` its own kind, or (`None`) the stair's again.
    pub fn set_turn_kind(&mut self, turn: usize, kind: Option<TurnKind>) {
        self.turns.retain(|t| t.turn as usize != turn);
        if let Some(kind) = kind {
            self.turns.push(TurnEdit {
                turn: turn as u32,
                kind,
            });
            self.turns.sort_by_key(|t| t.turn);
        }
    }

    /// The treads the flight along `straight` was given, if any.
    pub fn treads(&self, straight: usize) -> Option<u32> {
        self.flights
            .iter()
            .find(|f| f.straight as usize == straight)
            .map(|f| f.treads)
    }

    /// How much deeper the landing at `turn` was made.
    pub fn deeper(&self, turn: usize) -> f64 {
        self.landings
            .iter()
            .find(|l| l.turn as usize == turn)
            .map_or(0.0, |l| l.deeper)
    }

    /// Give the flight along `straight` its own number of treads, or
    /// (`None`) let it take what fits again.
    pub fn set_treads(&mut self, straight: usize, treads: Option<u32>) {
        self.flights.retain(|f| f.straight as usize != straight);
        if let Some(t) = treads {
            self.flights.push(FlightEdit {
                straight: straight as u32,
                treads: t,
            });
            self.flights.sort_by_key(|f| f.straight);
        }
    }

    /// Make the landing at `turn` deeper by `by` (0: as the line leaves it).
    pub fn set_deeper(&mut self, turn: usize, by: f64) {
        self.landings.retain(|l| l.turn as usize != turn);
        if by > 1e-6 {
            self.landings.push(LandingEdit {
                turn: turn as u32,
                deeper: by,
            });
            self.landings.sort_by_key(|l| l.turn);
        }
    }

    /// Keep each change with its piece when point `i` is put into the
    /// line (it splits straight `i - 1`; the new turn is turn `i - 1`).
    pub fn point_added(&mut self, i: usize) {
        for f in &mut self.flights {
            if f.straight as usize >= i {
                f.straight += 1;
            }
        }
        for l in &mut self.landings {
            if l.turn as usize + 1 >= i {
                l.turn += 1;
            }
        }
        for t in &mut self.turns {
            if t.turn as usize + 1 >= i {
                t.turn += 1;
            }
        }
    }

    /// Keep each change with its piece when point `i` of a line of
    /// `points` is taken out: an inner point joins the straights either
    /// side into one (the first's change stays) and takes its turn away;
    /// an end point takes its straight and the turn next to it.
    pub fn point_removed(&mut self, i: usize, points: usize) {
        let last = points.saturating_sub(1);
        let (straight, turn) = if i == 0 {
            (0, 0)
        } else if i >= last {
            (last.saturating_sub(1), last.saturating_sub(2))
        } else {
            (i, i - 1)
        };
        self.flights.retain(|f| f.straight as usize != straight);
        for f in &mut self.flights {
            if f.straight as usize > straight {
                f.straight -= 1;
            }
        }
        self.landings.retain(|l| l.turn as usize != turn);
        for l in &mut self.landings {
            if l.turn as usize > turn {
                l.turn -= 1;
            }
        }
        self.turns.retain(|t| t.turn as usize != turn);
        for t in &mut self.turns {
            if t.turn as usize > turn {
                t.turn -= 1;
            }
        }
    }
}

fn default_waist() -> f64 {
    0.15
}

/// One run of steps: straight, or curved round an arc of its line.
#[derive(Clone, Debug, PartialEq)]
pub struct Flight {
    /// Middle of the first riser, in plan.
    pub start: DVec2,
    /// Unit direction of travel at its first riser.
    pub dir: DVec2,
    pub risers: u32,
    /// Height of the bottom of the flight above the stair's base.
    pub z0: f64,
    /// The straight (or curve) of the walk line it runs along.
    pub straight: usize,
    /// The walk line it climbs, from its first riser: two points for a
    /// straight flight, more round a curve. Empty: straight along `dir`.
    pub path: Vec<DVec2>,
}

impl Flight {
    /// Plan length from the first riser to the last, along the walk line.
    pub fn run(&self, tread: f64) -> f64 {
        self.risers.saturating_sub(1) as f64 * tread
    }

    pub fn end(&self, tread: f64) -> DVec2 {
        self.at(self.run(tread)).0
    }

    /// Which way it climbs at its last riser.
    pub fn end_dir(&self, tread: f64) -> DVec2 {
        self.at(self.run(tread)).1
    }

    /// Does it curve?
    pub fn curved(&self) -> bool {
        self.path.len() > 2
    }

    /// The point `u` along its walk line, and which way it climbs there;
    /// on past either end along the line's end.
    pub fn at(&self, u: f64) -> (DVec2, DVec2) {
        if self.path.len() < 2 {
            return (self.start + self.dir * u, self.dir);
        }
        let mut left = u;
        let n = self.path.len();
        for (k, w) in self.path.windows(2).enumerate() {
            let l = w[0].distance(w[1]);
            let d = (w[1] - w[0]).normalize_or(self.dir);
            if left <= l || k + 2 == n {
                return (w[0] + d * left, d);
            }
            left -= l;
        }
        (self.start + self.dir * u, self.dir)
    }

    /// Riser `i`'s two ends, `half` either side of the walk line (on its
    /// right, then its left); square to the line, radial on a curve.
    pub fn riser(&self, i: u32, tread: f64, half: f64) -> (DVec2, DVec2) {
        let (p, d) = self.at(i as f64 * tread);
        let d = self.bisected(i as f64 * tread).unwrap_or(d);
        (p - d.perp() * half, p + d.perp() * half)
    }

    /// The walk line's direction at a corner of its path at `u`, halfway
    /// between the two sides of the corner.
    fn bisected(&self, u: f64) -> Option<DVec2> {
        let mut run = 0.0;
        for k in 1..self.path.len().saturating_sub(1) {
            run += self.path[k - 1].distance(self.path[k]);
            if (run - u).abs() < 1e-9 {
                let a = (self.path[k] - self.path[k - 1]).normalize_or_zero();
                let b = (self.path[k + 1] - self.path[k]).normalize_or_zero();
                return (a + b).try_normalize();
            }
        }
        None
    }

    /// The corners of its path between `u0` and `u1` along it, each with
    /// the side points `half` either side.
    fn sides_between(&self, u0: f64, u1: f64, half: f64) -> Vec<(DVec2, DVec2)> {
        let mut out = vec![];
        let mut run = 0.0;
        for k in 1..self.path.len().saturating_sub(1) {
            run += self.path[k - 1].distance(self.path[k]);
            if run > u0 + 1e-9 && run < u1 - 1e-9 {
                let a = (self.path[k] - self.path[k - 1]).normalize_or_zero();
                let b = (self.path[k + 1] - self.path[k]).normalize_or_zero();
                let m = (a.perp() + b.perp()).normalize_or(a.perp());
                let w = half / m.dot(a.perp()).max(0.2);
                out.push((self.path[k] - m * w, self.path[k] + m * w));
            }
        }
        out
    }

    /// Tread `i` (between riser `i` and the next) in plan,
    /// counter-clockwise, `half` either side of the walk line.
    pub fn tread_outline(&self, i: u32, tread: f64, half: f64) -> Vec<DVec2> {
        let (r0, l0) = self.riser(i, tread, half);
        let (r1, l1) = self.riser(i + 1, tread, half);
        let mid = self.sides_between(i as f64 * tread, (i + 1) as f64 * tread, half);
        let mut pts = vec![r0];
        pts.extend(mid.iter().map(|m| m.0));
        pts.push(r1);
        pts.push(l1);
        pts.extend(mid.iter().rev().map(|m| m.1));
        pts.push(l0);
        ccw(pts)
    }

    /// The strip of it between `u0` and `u1` along its walk line, `half`
    /// either side, counter-clockwise; it runs on straight past either end.
    pub fn band(&self, u0: f64, u1: f64, half: f64) -> Vec<DVec2> {
        let end = |u: f64| {
            let (p, d) = self.at(u);
            (p - d.perp() * half, p + d.perp() * half)
        };
        let (r0, l0) = end(u0);
        let (r1, l1) = end(u1);
        let mid = self.sides_between(u0, u1, half);
        let mut pts = vec![r0];
        pts.extend(mid.iter().map(|m| m.0));
        pts.push(r1);
        pts.push(l1);
        pts.extend(mid.iter().rev().map(|m| m.1));
        pts.push(l0);
        ccw(pts)
    }

    /// Its footprint in plan, from the first riser to the last,
    /// counter-clockwise.
    pub fn outline(&self, tread: f64, half: f64) -> Vec<DVec2> {
        let n = self.risers.saturating_sub(1);
        let (r0, l0) = self.riser(0, tread, half);
        let (r1, l1) = self.riser(n, tread, half);
        let mid = self.sides_between(0.0, self.run(tread), half);
        let mut pts = vec![r0];
        pts.extend(mid.iter().map(|m| m.0));
        pts.push(r1);
        pts.push(l1);
        pts.extend(mid.iter().rev().map(|m| m.1));
        pts.push(l0);
        ccw(pts)
    }

    /// One side of it, `half` off the walk line (`left`, or its right),
    /// from `u0` to `u1` along it.
    pub fn side(&self, left: bool, u0: f64, u1: f64, half: f64) -> Vec<DVec2> {
        let pick = |p: (DVec2, DVec2)| if left { p.1 } else { p.0 };
        let end = |u: f64| {
            let (p, d) = self.at(u);
            let d = self.bisected(u).unwrap_or(d);
            pick((p - d.perp() * half, p + d.perp() * half))
        };
        let mut pts = vec![end(u0)];
        pts.extend(self.sides_between(u0, u1, half).into_iter().map(pick));
        pts.push(end(u1));
        pts
    }
}

/// One winder: a tapered tread at a turn.
#[derive(Clone, Debug, PartialEq)]
pub struct Winder {
    /// Its tread in plan, counter-clockwise.
    pub outline: Vec<DVec2>,
    /// Height of its tread above the stair's base.
    pub z: f64,
    /// The turn of the walk line it is at.
    pub turn: usize,
}

/// The parts of a stair, laid out in plan (model coordinates).
#[derive(Clone, Debug, PartialEq)]
pub struct StairLayout {
    pub risers: u32,
    pub riser: f64,
    pub flights: Vec<Flight>,
    /// Each landing's outline in plan and its height above the base.
    pub landings: Vec<(Vec<DVec2>, f64)>,
    /// The turn of the walk line each landing sits at (the first, for a
    /// landing across two turns).
    pub turns: Vec<usize>,
    /// The winders, in climbing order.
    pub winders: Vec<Winder>,
    /// The walk line from the first riser to the last, each point with the
    /// height of the pitch line over it (above the base): it rises with
    /// the nosings, runs level across a landing and climbs to the next
    /// flight's first nosing over the landing's second half.
    pub walk: Vec<DVec3>,
    /// Each winder riser inside a turn: where it crosses the walk line, and
    /// its outer end. A handrail meets that riser along this line, not
    /// square to the walk line.
    pub winder_risers: Vec<(DVec2, DVec2)>,
    /// Treads the line is too short for: the last flight runs on past it.
    pub short: u32,
    /// Line left over past the last riser.
    pub spare: f64,
}

impl StairLayout {
    /// Its treads: one fewer than the risers on each flight, so a landing
    /// takes the place of one.
    pub fn treads(&self) -> u32 {
        self.flights
            .iter()
            .map(|f| f.risers.saturating_sub(1))
            .sum::<u32>()
            + self.winders.len() as u32
    }
}

/// Turns sharper than this (degrees) are drawn as two.
const MAX_TURN: f64 = 135.0;

impl Stair {
    pub fn new(level: ElementId, start: DVec2, direction: f64) -> Self {
        Self {
            level,
            start,
            direction,
            width: 1.0,
            max_riser: 0.18,
            tread: 0.28,
            height: None,
            base_offset: 0.0,
            waist: default_waist(),
            landing: None,
            material: None,
            line: vec![],
            bends: vec![],
            build: StairBuild::Concrete,
            finish: StairFinish::default(),
            rules: Default::default(),
            drawn: StairLine::Walk,
            pieces: StairPieces::default(),
            at_turns: TurnKind::Landing,
            ramp: None,
            typical: None,
            split: false,
            fixed_foot: false,
            as_steps: None,
            up_offset: None,
        }
    }

    /// Where its UP label stands in plan, laid out as `lay`: before its
    /// first flight, moved by `up_offset`. Its arrow stays where it is.
    pub fn up_label_at(&self, lay: &StairLayout) -> Option<DVec2> {
        let f = lay.flights.first()?;
        Some(f.start - f.dir * 0.35 + self.up_offset.unwrap_or(DVec2::ZERO))
    }

    pub fn is_ramp(&self) -> bool {
        self.ramp.is_some()
    }

    /// Make it a ramp no steeper than `slope` (rise over run): its goings
    /// become [`RAMP_SLICE`] and its turns landings.
    pub fn make_ramp(&mut self, slope: f64) {
        let slope = slope.clamp(0.01, 0.5);
        if self.ramp.is_none() {
            self.as_steps = Some(Box::new(StepsKept {
                tread: self.tread,
                max_riser: self.max_riser,
                at_turns: self.at_turns,
                pieces: self.pieces.clone(),
                build: self.build,
                finish: self.finish,
            }));
        }
        self.ramp = Some(slope);
        self.tread = RAMP_SLICE;
        self.max_riser = RAMP_SLICE * slope;
        self.at_turns = TurnKind::Landing;
        self.pieces.turns.clear();
        self.pieces.flights.clear();
        self.finish = StairFinish::default();
        self.build = StairBuild::Concrete;
    }

    /// A ramp's slope changed, keeping its flights' lengths and its build.
    pub fn set_slope(&mut self, slope: f64) {
        let slope = slope.clamp(0.01, 0.5);
        self.ramp = Some(slope);
        self.tread = RAMP_SLICE;
        self.max_riser = RAMP_SLICE * slope;
        self.at_turns = TurnKind::Landing;
    }

    /// Make it steps again: the steps it had before it was a ramp, or
    /// the usual riser and going when it was drawn as a ramp.
    pub fn make_steps(&mut self) {
        if self.ramp.take().is_some() {
            match self.as_steps.take() {
                Some(k) => {
                    self.tread = k.tread;
                    self.max_riser = k.max_riser;
                    self.at_turns = k.at_turns;
                    self.pieces = k.pieces;
                    self.build = k.build;
                    self.finish = k.finish;
                }
                None => {
                    self.tread = 0.28;
                    self.max_riser = 0.18;
                    self.pieces.flights.clear();
                }
            }
        }
    }

    /// A ramp's slope as built, rise over run (its risers share the rise
    /// equally, so it is at most the slope asked for); a stair's pitch as
    /// a ratio.
    pub fn slope(&self, model: &Model) -> f64 {
        let lay = self.layout(self.rise(model));
        lay.riser / self.tread.max(1e-9)
    }

    /// A stair drawn by `points` (model coordinates), the `drawn` line of
    /// it. Repeated points and points on a straight run are dropped.
    pub fn from_line(level: ElementId, points: &[DVec2], drawn: StairLine) -> Option<Self> {
        let pts = clean_line(points);
        if pts.len() < 2 {
            return None;
        }
        let dir = (pts[1] - pts[0]).to_angle().to_degrees();
        let mut s = Stair::new(level, pts[0], dir);
        s.drawn = drawn;
        s.set_line(&pts);
        Some(s)
    }

    pub fn dir(&self) -> DVec2 {
        DVec2::from_angle(self.direction.to_radians())
    }

    /// A point of the stair's own frame in plan.
    pub fn to_plan(&self, p: DVec2) -> DVec2 {
        self.start + self.dir().rotate(p)
    }

    /// A plan point in the stair's own frame.
    pub fn to_local(&self, p: DVec2) -> DVec2 {
        let d = self.dir();
        DVec2::new(d.x, -d.y).rotate(p - self.start)
    }

    /// Set the line from points in plan; `start` and `direction` stay,
    /// and so do the bends of the straights still there.
    pub fn set_line(&mut self, points: &[DVec2]) {
        self.set_line_gap(points, 0.0);
    }

    /// `set_line`, a turn sharper than [`MAX_TURN`] becoming a half-turn
    /// whose next run lies `gap` beside the last (Stair Guides REV02 §6).
    pub fn set_line_gap(&mut self, points: &[DVec2], gap: f64) {
        let points = if self.bends.iter().any(|b| b.abs() > 1e-9) {
            points.to_vec()
        } else {
            split_sharp_turns(points, self.drawn.to_walk(self.width), self.width, gap)
        };
        let local: Vec<DVec2> = points.iter().map(|p| self.to_local(*p)).collect();
        self.line = local;
        self.landing = None;
        self.bends.truncate(self.line.len().saturating_sub(1));
    }

    /// Draw it by another of its lines, the stair staying where it is:
    /// its line moves across to the new side (MU101 #2). A curved stair's
    /// arcs keep their ends, so it moves half a width, as before.
    pub fn set_drawn_in_place(&mut self, drawn: StairLine, rise: f64) {
        if drawn == self.drawn {
            return;
        }
        if self.line.len() < 2 || self.curved() {
            self.drawn = drawn;
            return;
        }
        let walk = self.walk_in_plan(rise);
        let back = -drawn.to_walk(self.width);
        let line = if back == 0.0 {
            walk
        } else {
            crate::offset::offset_path(&walk, back)
        };
        self.drawn = drawn;
        self.line = line.iter().map(|p| self.to_local(*p)).collect();
    }

    /// Whether any two of its flights, landings and winders overlap: a line
    /// folded back so its flights cross (MU101 #5). Pieces that only meet
    /// along an edge or at a corner don't count.
    pub fn pieces_overlap(&self, rise: f64) -> bool {
        let shrunk: Vec<Vec<DVec2>> = self
            .outlines(rise)
            .iter()
            .filter(|o| o.len() >= 3 && crate::polygon::signed_area2(o).abs() > 1e-6)
            .map(|o| {
                let ccw = if crate::polygon::signed_area2(o) < 0.0 {
                    o.iter().rev().copied().collect()
                } else {
                    o.clone()
                };
                offset_polygon(&ccw, -0.01)
            })
            .collect();
        (0..shrunk.len())
            .any(|i| (i + 1..shrunk.len()).any(|j| crate::polygon::overlap(&shrunk[i], &shrunk[j])))
    }

    /// Where its line's last point goes when dragged to `p` (plan) so the
    /// last flight takes a whole number of treads: moved along the last
    /// straight by under half a tread (Stair Guides REV00 §2, A).
    pub fn whole_treads_end(&self, rise: f64, p: DVec2) -> DVec2 {
        let n = self.line.len();
        if n < 2 || self.bend(n - 2).abs() > 1e-9 {
            return p;
        }
        let mut s = self.clone();
        s.line[n - 1] = s.to_local(p);
        let prev = s.to_plan(s.line[n - 2]);
        let Some(dir) = (p - prev).try_normalize() else {
            return p;
        };
        let lay = s.layout(rise);
        let (Some(f), Some(end)) = (lay.flights.last(), s.walk_in_plan(rise).last().copied())
        else {
            return p;
        };
        if f.curved() {
            return p;
        }
        let avail = (end - f.start).dot(f.dir);
        let whole = (avail / s.tread).round().max(1.0) * s.tread;
        p + dir * ((whole - avail) / f.dir.dot(dir).max(0.2))
    }

    /// Where its head is in plan: the middle of its last riser.
    pub fn head(&self, rise: f64) -> Option<(DVec2, DVec2)> {
        let lay = self.layout(rise);
        let f = lay.flights.last()?;
        Some((f.end(self.tread), f.end_dir(self.tread)))
    }

    /// After its treads changed from `before`'s (a tread, a riser, its
    /// rise), its head kept where it was unless its foot is the end fixed:
    /// the first straight of its line takes up the difference, so the
    /// first flight grows or shrinks and its turns stay (REV01 decision 6,
    /// A). A curved first straight keeps the foot.
    pub fn keep_fixed_end(&mut self, before: &Stair, model: &Model) {
        if self.fixed_foot || self.line.len() < 2 || self.bend(0).abs() > 1e-9 {
            return;
        }
        let (Some((h0, dir)), Some((h1, _))) =
            (before.head(before.rise(model)), self.head(self.rise(model)))
        else {
            return;
        };
        let by = (h1 - h0).dot(dir);
        if by.abs() < 1e-9 {
            return;
        }
        let mut pts: Vec<DVec2> = self.line.iter().map(|p| self.to_plan(*p)).collect();
        let Some(u) = (pts[1] - pts[0]).try_normalize() else {
            return;
        };
        pts[0] -= u * by;
        if (pts[1] - pts[0]).dot(u) < 0.05 {
            return;
        }
        self.start = pts[0];
        self.line = pts.iter().map(|p| self.to_local(*p)).collect();
    }

    /// Each landing's centre, its depth (across, along the flight that
    /// arrives at it) and, at a half-turn, the gap between the runs either
    /// side of it: the temporary dimensions shown while drawing and on a
    /// selected stair (Stair Guides REV02 §5).
    pub fn landing_sizes(&self, rise: f64) -> Vec<(DVec2, f64, Option<f64>)> {
        let lay = self.layout(rise);
        let t = self.tread;
        lay.landings
            .iter()
            .enumerate()
            .filter_map(|(i, (l, _))| {
                let (f1, f2) = (lay.flights.get(i)?, lay.flights.get(i + 1)?);
                let d = f1.end_dir(t);
                let along: Vec<f64> = l.iter().map(|p| p.dot(d)).collect();
                let lo = along.iter().copied().fold(f64::INFINITY, f64::min);
                let hi = along.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let c = l.iter().copied().sum::<DVec2>() / l.len().max(1) as f64;
                // back the way it came: the runs side by side
                let gap = (f2.dir.dot(d) < -0.9)
                    .then(|| ((f2.start - f1.end(t)).perp_dot(d).abs() - self.width).max(0.0));
                Some((c, hi - lo, gap))
            })
            .collect()
    }

    /// How many treads it lacks: those of its last flight past the end of
    /// its line, with that part's outline in plan (to show as a ghost).
    pub fn short_treads(&self, rise: f64) -> (u32, Vec<Vec<DVec2>>) {
        let lay = self.layout(rise);
        if lay.short == 0 {
            return (0, vec![]);
        }
        let Some(f) = lay.flights.last() else {
            return (lay.short, vec![]);
        };
        let top = f.risers.saturating_sub(1);
        let from = top.saturating_sub(lay.short);
        let ghosts = (from..top)
            .map(|i| f.tread_outline(i, self.tread, self.width / 2.0))
            .collect();
        (lay.short, ghosts)
    }

    /// Make each turn of its line sharper than [`MAX_TURN`] a half-turn
    /// (after a corner was added or moved), the changes kept on its
    /// flights and turns following their straights. Whether it changed.
    pub fn split_sharp(&mut self) -> bool {
        if self.line.len() < 3 || self.curved() {
            return false;
        }
        let plan: Vec<DVec2> = self.line.iter().map(|p| self.to_plan(*p)).collect();
        let (pts, added) =
            split_sharp_turns_at(&plan, self.drawn.to_walk(self.width), self.width, 0.0);
        // ...or a run back made parallel, no point put in
        let squared = added.is_empty()
            && pts.len() == plan.len()
            && pts.iter().zip(&plan).any(|(a, b)| a.distance(*b) > 1e-9);
        if added.is_empty() && !squared {
            return false;
        }
        for &i in &added {
            self.pieces.point_added(i);
        }
        self.line = pts.iter().map(|p| self.to_local(*p)).collect();
        self.bends.clear();
        true
    }

    /// How far straight `i` of the line bends (degrees, left positive).
    pub fn bend(&self, i: usize) -> f64 {
        self.bends.get(i).copied().unwrap_or(0.0)
    }

    /// Bend straight `i` of the line by `deg` (see `bends`).
    pub fn set_bend(&mut self, i: usize, deg: f64) {
        let n = self.line.len().saturating_sub(1);
        if i >= n {
            return;
        }
        if self.bends.len() < n {
            self.bends.resize(n, 0.0);
        }
        self.bends[i] = deg.clamp(-MAX_BEND, MAX_BEND);
        if self.bends.iter().all(|b| b.abs() < 1e-9) {
            self.bends.clear();
        }
    }

    /// The centre its line curves round: the first arc's, in plan.
    pub fn centre(&self) -> Option<DVec2> {
        let i = (0..self.line.len().saturating_sub(1)).find(|i| self.bend(*i).abs() > 1e-9)?;
        Some(arc_centre(
            self.to_plan(self.line[i]),
            self.to_plan(self.line[i + 1]),
            self.bend(i).to_radians(),
        ))
    }

    /// How much it takes to build, climbing `rise`.
    pub fn quantities(&self, rise: f64) -> StairQuantities {
        let q = self.half_quantities(rise);
        // a split stair: both arms
        if self.mirror(rise).is_some() {
            return StairQuantities {
                structure: q.structure * 2.0,
                treads: q.treads * 2.0,
                risers: q.risers * 2.0,
                run: q.run,
            };
        }
        q
    }

    fn half_quantities(&self, rise: f64) -> StairQuantities {
        let lay = self.layout(rise);
        let (r, g, half) = (lay.riser, self.tread, self.width / 2.0);
        let area = |p: &[DVec2]| crate::lot::area(p);
        let over = self.finish.riser(self.build) + self.finish.nosing.max(0.0);
        let mut q = StairQuantities::default();
        let mut slope_len = 0.0;
        let mut steps = 0.0;
        let mut tread_area = 0.0;
        for f in &lay.flights {
            let run = f.run(g);
            q.run += run;
            slope_len += (run * run + (f.risers.saturating_sub(1) as f64 * r).powi(2)).sqrt();
            for t in 0..f.risers.saturating_sub(1) {
                let a = area(&f.band(t as f64 * g, (t + 1) as f64 * g, half));
                tread_area += a;
                steps += a * r / 2.0;
                q.treads += area(&f.band(t as f64 * g - over, (t + 1) as f64 * g, half));
            }
            if !self.build.open() {
                q.risers += f.risers as f64 * self.width * (r - self.finish.tread()).max(0.0);
            }
        }
        let landings: f64 = lay.landings.iter().map(|(o, _)| area(o)).sum::<f64>()
            + lay.winders.iter().map(|w| area(&w.outline)).sum::<f64>();
        q.treads += landings;
        let sec = (1.0 + (r / g.max(1e-6)).powi(2)).sqrt();
        let wv = self.waist * sec;
        q.structure = match self.build {
            StairBuild::Concrete => self.width * self.waist * slope_len + steps + landings * wv,
            StairBuild::Stepped => tread_area * (r + wv) + landings * wv,
            StairBuild::FoldedPlate => {
                let risers = self.width * r * lay.risers as f64;
                self.waist * (tread_area + landings + risers)
            }
            b => {
                let board = if b == StairBuild::Cantilever {
                    2.0
                } else {
                    1.0
                } * TREAD_BOARD;
                let members = match b {
                    StairBuild::Stringers | StairBuild::CutStringers => {
                        2.0 * STRINGER.0 * STRINGER.1 * slope_len
                    }
                    StairBuild::Spine => SPINE.0 * SPINE.1 * slope_len,
                    StairBuild::Post if self.centre().is_some() => {
                        std::f64::consts::PI * POST_RADIUS * POST_RADIUS * (rise + 0.9)
                    }
                    StairBuild::Post => SPINE.0 * SPINE.1 * slope_len,
                    _ => 0.0,
                };
                (tread_area + landings) * board + members
            }
        };
        q
    }

    /// Does any straight of its line bend?
    pub fn curved(&self) -> bool {
        self.bends.iter().any(|b| b.abs() > 1e-9)
    }

    /// A point was put into the line after point `i`, splitting straight
    /// `i`: each half keeps half its bend.
    pub fn bend_split(&mut self, i: usize) {
        if i < self.bends.len() {
            let b = self.bends[i] / 2.0;
            self.bends[i] = b;
            self.bends.insert(i + 1, b);
        }
    }

    /// Point `i` of the line is taken out: the straights either side of it
    /// become one, bent by both.
    pub fn bend_joined(&mut self, i: usize) {
        if self.bends.is_empty() {
            return;
        }
        if i == 0 {
            self.bends.remove(0);
        } else if i < self.bends.len() {
            let b = self.bends.remove(i);
            self.bends[i - 1] = (self.bends[i - 1] + b).clamp(-MAX_BEND, MAX_BEND);
        } else {
            self.bends.truncate(i - 1);
        }
    }

    /// The line it was drawn by, in plan. A stair from before lines gets
    /// the walk line its flights and landing make.
    pub fn line_in_plan(&self, rise: f64) -> Vec<DVec2> {
        if self.line.len() >= 2 && self.curved() {
            let mut out = vec![self.to_plan(self.line[0])];
            for (i, w) in self.line.windows(2).enumerate() {
                let (a, b) = (self.to_plan(w[0]), self.to_plan(w[1]));
                out.extend(arc_between(a, b, self.bend(i)));
                if out.last().is_none_or(|q| q.distance(b) > 1e-6) {
                    out.push(b);
                }
            }
            return out;
        }
        if self.line.len() >= 2 {
            return clean_line(
                &self
                    .line
                    .iter()
                    .map(|p| self.to_plan(*p))
                    .collect::<Vec<_>>(),
            );
        }
        // straight, or a square landing after `after` risers
        let n = riser_count(rise, self.max_riser);
        let (h, t) = (self.width / 2.0, self.tread);
        let local = match self.landing.filter(|l| l.after >= 1 && l.after < n) {
            None => vec![
                DVec2::ZERO,
                DVec2::X * (n.saturating_sub(1) as f64 * t).max(t),
            ],
            Some(l) => {
                let x = l.after.saturating_sub(1) as f64 * t + h;
                let y = (n - l.after).saturating_sub(1) as f64 * t + h;
                let side = if l.left { 1.0 } else { -1.0 };
                vec![DVec2::ZERO, DVec2::new(x, 0.0), DVec2::new(x, side * y)]
            }
        };
        local.into_iter().map(|p| self.to_plan(p)).collect()
    }

    /// The walk line as drawn (before it is filled), in plan.
    pub fn walk_in_plan(&self, rise: f64) -> Vec<DVec2> {
        let line = self.line_in_plan(rise);
        let by = if self.line.len() >= 2 {
            self.drawn.to_walk(self.width)
        } else {
            0.0
        };
        if by == 0.0 {
            line
        } else {
            crate::offset::offset_path(&line, by)
        }
    }

    /// The total rise as built.
    pub fn rise(&self, model: &Model) -> f64 {
        self.height
            .unwrap_or_else(|| storey_height(model, self.level) - self.base_offset)
            .max(0.05)
    }

    /// The corners of the box round its treads, landings and winders in
    /// plan; none for a stair with nothing laid out.
    pub fn footprint(&self, model: &Model) -> Option<(DVec2, DVec2)> {
        let lay = self.layout(self.rise(model));
        let mut pts: Vec<DVec2> = vec![];
        for f in &lay.flights {
            for i in 0..f.risers {
                let (a, c) = f.riser(i, self.tread, self.width / 2.0);
                pts.push(a);
                pts.push(c);
            }
        }
        for (outline, _) in &lay.landings {
            pts.extend(outline.iter().copied());
        }
        for w in &lay.winders {
            pts.extend(w.outline.iter().copied());
        }
        // and its other half
        if let Some(m) = self.mirror_of(&lay) {
            let other: Vec<DVec2> = pts.iter().map(|p| m.pt(*p)).collect();
            pts.extend(other);
        }
        let first = *pts.first()?;
        Some(
            pts.iter()
                .fold((first, first), |(l, h), p| (l.min(*p), h.max(*p))),
        )
    }

    /// A split stair's mirror line, for a rise of `rise`; None when it
    /// isn't split, or its line doesn't turn (there is nothing to part).
    pub fn mirror(&self, rise: f64) -> Option<Mirror> {
        if !self.split {
            return None;
        }
        self.mirror_of(&self.layout(rise))
    }

    /// As [`Stair::mirror`], from its layout.
    pub fn mirror_of(&self, lay: &StairLayout) -> Option<Mirror> {
        if !self.split || self.curved() {
            return None;
        }
        let f = lay.flights.first()?;
        let d = f.dir.normalize_or_zero();
        // the walk line's first change of direction
        let next = lay.walk.windows(2).find_map(|w| {
            let v = (w[1] - w[0]).truncate();
            (v.length() > 1e-6 && v.normalize().dot(d) < 0.98).then(|| v.normalize())
        })?;
        let turns_left = d.perp_dot(next) > 0.0;
        // away from the turn
        let n = if turns_left { -d.perp() } else { d.perp() };
        Some(Mirror {
            origin: f.start + n * (self.width / 2.0),
            dir: d,
            left: !turns_left,
            shared: f.run(self.tread),
            width: self.width,
        })
    }

    /// The box a 3D view cut out round it keeps: its footprint a little
    /// wider, from under its foot to above its head.
    pub fn cut_out(&self, model: &Model) -> Option<(DVec3, DVec3)> {
        let (lo, hi) = self.footprint(model)?;
        let base = self.base(model);
        Some((
            (lo - DVec2::splat(0.15)).extend(base - 0.3),
            (hi + DVec2::splat(0.15)).extend(base + self.rise(model) + 1.2),
        ))
    }

    /// Absolute height of the bottom.
    pub fn base(&self, model: &Model) -> f64 {
        elevation(model, self.level) + self.base_offset
    }

    /// The flights and landings for a rise of `rise`, filling the walk
    /// line from its start: a flight along each straight length, a landing
    /// at each corner (one landing across two corners close together, as
    /// in a half-turn stair), the line's spare length kept as a deeper
    /// landing before the next turn.
    pub fn layout(&self, rise: f64) -> StairLayout {
        let n = riser_count(rise, self.max_riser);
        let r = rise / n as f64;
        let walk = self.walk_in_plan(rise);
        // a ramp turns on landings only
        let ramp_pieces;
        let (pieces, at_turns) = if self.is_ramp() {
            ramp_pieces = StairPieces {
                turns: vec![],
                ..self.pieces.clone()
            };
            (&ramp_pieces, TurnKind::Landing)
        } else {
            (&self.pieces, self.at_turns)
        };
        layout_walk(
            &walk,
            n,
            r,
            self.width,
            self.tread.max(0.01),
            pieces,
            at_turns,
        )
    }

    /// The path a handrail follows along one side, over the nosings
    /// (heights above the stair's base), set `inset` in from the edge: the
    /// walk line moved to that side, so it stays continuous round every
    /// turn, tight on the inside, round the landing on the outside.
    pub fn side_path(&self, rise: f64, left: bool, inset: f64) -> Vec<DVec3> {
        let lay = self.layout(rise);
        side_of(&lay, left, self.width / 2.0 - inset)
    }

    /// Plan footprint of each flight, counter-clockwise.
    pub fn flight_outlines(&self, rise: f64) -> Vec<Vec<DVec2>> {
        self.layout(rise)
            .flights
            .iter()
            .map(|f| f.outline(self.tread, self.width / 2.0))
            .collect()
    }

    /// Every flight's and landing's outline in plan.
    pub fn outlines(&self, rise: f64) -> Vec<Vec<DVec2>> {
        let mut out = self.flight_outlines(rise);
        let lay = self.layout(rise);
        out.extend(lay.landings.into_iter().map(|(l, _)| l));
        out.extend(lay.winders.into_iter().map(|w| w.outline));
        out
    }

    /// Cut the line off at the last riser, so nothing is left over past
    /// the top of the stair.
    pub fn trim(&mut self, rise: f64) {
        let lay = self.layout(rise);
        if lay.spare < 1e-6 || self.line.len() < 2 {
            return;
        }
        if self.curved() {
            return self.trim_curved(lay.spare);
        }
        let mut walk = self.walk_in_plan(rise);
        let mut cut = lay.spare;
        while walk.len() >= 2 && cut > 1e-9 {
            let n = walk.len();
            let (a, b) = (walk[n - 2], walk[n - 1]);
            let l = a.distance(b);
            if l <= cut + 1e-9 && n > 2 {
                walk.pop();
                cut -= l;
            } else {
                walk[n - 1] = b + (a - b).normalize_or_zero() * cut.min(l - 0.01);
                cut = 0.0;
            }
        }
        let back = -self.drawn.to_walk(self.width);
        let line = if back == 0.0 {
            walk
        } else {
            crate::offset::offset_path(&walk, back)
        };
        self.set_line(&line);
    }

    /// `trim` for a line with arcs: its last straight or arc shortened
    /// (round the same centre), `cut` measured along the walk line.
    fn trim_curved(&mut self, mut cut: f64) {
        let by = self.drawn.to_walk(self.width);
        while self.line.len() >= 2 && cut > 1e-9 {
            let n = self.line.len();
            let (a, b) = (self.line[n - 2], self.line[n - 1]);
            let sweep = self.bend(n - 2).to_radians();
            let chord = a.distance(b);
            // its length along the walk line
            let walk_len = if sweep.abs() < 1e-9 {
                chord
            } else {
                let r = chord / (2.0 * (sweep / 2.0).sin().abs());
                (r - by * sweep.signum()).max(0.0) * sweep.abs()
            };
            if walk_len <= cut + 1e-9 && n > 2 {
                self.line.pop();
                self.bends.truncate(n - 2);
                cut -= walk_len;
                continue;
            }
            let f = (walk_len - cut.min(walk_len - 0.01)) / walk_len.max(1e-9);
            if sweep.abs() < 1e-9 {
                self.line[n - 1] = a + (b - a) * f;
            } else {
                let c = arc_centre(a, b, sweep);
                self.line[n - 1] = c + DVec2::from_angle(sweep * f).rotate(a - c);
                self.bends[n - 2] = sweep.to_degrees() * f;
            }
            cut = 0.0;
        }
    }

    /// Its IFC predefined type, from its shape.
    pub fn ifc_type(&self, rise: f64) -> &'static str {
        let lay = self.layout(rise);
        if self.mirror_of(&lay).is_some() {
            return "DOUBLE_RETURN_STAIR";
        }
        if self.is_ramp() {
            // IFC's ramp types: turns count by quarters
            let turns: f64 = lay
                .walk
                .windows(3)
                .map(|w| {
                    let (a, b) = ((w[1] - w[0]).truncate(), (w[2] - w[1]).truncate());
                    if a.length() < 1e-9 || b.length() < 1e-9 {
                        0.0
                    } else {
                        a.angle_to(b).to_degrees()
                    }
                })
                .sum();
            if self.curved() {
                return "SPIRAL_RAMP";
            }
            return match ((turns.abs() / 90.0).round() as i32, lay.landings.len()) {
                (0, 0) => "STRAIGHT_RUN_RAMP",
                (0, _) => "TWO_STRAIGHT_RUN_RAMP",
                (1, _) => "QUARTER_TURN_RAMP",
                (2, n) if n >= 2 => "TWO_QUARTER_TURN_RAMP",
                (2, _) => "HALF_TURN_RAMP",
                _ => "USERDEFINED",
            };
        }
        if self.curved() {
            // round a centre, its inside edge closer to it than the stair
            // is wide: a spiral; else a curved flight
            let by = self.drawn.to_walk(self.width);
            let n = self.line.len() - 1;
            let sign = self.bend(0).signum();
            let tight = (0..n).all(|i| {
                let sweep = self.bend(i).to_radians();
                if sweep.abs() < 1e-9 || sweep.signum() != sign {
                    return false;
                }
                let chord = self.line[i].distance(self.line[i + 1]);
                let r = chord / (2.0 * (sweep / 2.0).sin().abs());
                r - by * sign - self.width / 2.0 < self.width
            });
            return if tight && lay.landings.is_empty() && lay.winders.is_empty() {
                "SPIRAL_STAIR"
            } else {
                "CURVED_RUN_STAIR"
            };
        }
        let turns: f64 = lay
            .walk
            .windows(3)
            .map(|w| {
                let (a, b) = ((w[1] - w[0]).truncate(), (w[2] - w[1]).truncate());
                if a.length() < 1e-9 || b.length() < 1e-9 {
                    0.0
                } else {
                    a.angle_to(b).to_degrees()
                }
            })
            .sum();
        let quarters = (turns.abs() / 90.0).round() as i32;
        if !lay.winders.is_empty() {
            return match quarters {
                1 => "QUARTER_WINDING_STAIR",
                2 if lay.landings.is_empty() => "HALF_WINDING_STAIR",
                2 => "TWO_QUARTER_WINDING_STAIR",
                3 => "THREE_QUARTER_WINDING_STAIR",
                _ => "USERDEFINED",
            };
        }
        match (lay.flights.len(), quarters) {
            (_, 0) if lay.landings.is_empty() => "STRAIGHT_RUN_STAIR",
            (_, 0) => "TWO_STRAIGHT_RUN_STAIR",
            (_, 1) => "QUARTER_TURN_STAIR",
            (_, 2) if lay.landings.len() >= 2 => "TWO_QUARTER_TURN_STAIR",
            (_, 2) => "HALF_TURN_STAIR",
            (_, 3) => "THREE_QUARTER_TURN_STAIR",
            _ => "USERDEFINED",
        }
    }
}

/// A stair's line with each turn sharper than [`MAX_TURN`] made a
/// half-turn: a short leg square to the run before, so the next run lies
/// beside it with `gap` between the two, and the line after it moved
/// across with it. A U drawn with one click at its turn (or a corner
/// dragged back past 135°) is laid out as the U it means, its two corners
/// sharing one landing, instead of a corner the layout cannot turn.
/// `to_walk` is how far the walk line lies left of the line (its side).
pub fn split_sharp_turns(points: &[DVec2], to_walk: f64, width: f64, gap: f64) -> Vec<DVec2> {
    split_sharp_turns_at(points, to_walk, width, gap).0
}

/// [`split_sharp_turns`], with the index of each point it put in.
fn split_sharp_turns_at(
    points: &[DVec2],
    to_walk: f64,
    width: f64,
    gap: f64,
) -> (Vec<DVec2>, Vec<usize>) {
    let mut added = vec![];
    let mut pts = clean_line(points);
    let mut i = 1;
    while i + 1 < pts.len() {
        let d1 = (pts[i] - pts[i - 1]).normalize_or_zero();
        let d2 = (pts[i + 1] - pts[i]).normalize_or_zero();
        let turn = d1.angle_to(d2);
        if d1 != DVec2::ZERO && d2 != DVec2::ZERO && turn.abs() > MAX_TURN.to_radians() {
            let s = turn.signum();
            // a half-turn's runs are parallel: the line after the turn is
            // swung square to the run before (MU103: a return run a few
            // degrees off left a wedge at the landing and skewed treads)
            let swing = DVec2::from_angle(s * std::f64::consts::PI - turn);
            let pivot = pts[i];
            for p in pts.iter_mut().skip(i + 1) {
                *p = pivot + swing.rotate(*p - pivot);
            }
            // the walk lines a width and the gap apart; the drawn sides
            // that far plus each side's offset from its walk line
            let k = (width + gap.max(0.0) + 2.0 * s * to_walk).max(0.002);
            let shift = d1.perp() * s * k;
            for p in pts.iter_mut().skip(i + 1) {
                *p += shift;
            }
            pts.insert(i + 1, pts[i] + shift);
            added.push(i + 1);
            i += 1;
        }
        i += 1;
    }
    // a U drawn with two clicks: where the leg between two turns is too
    // short for a flight (so they share one landing) and the run back is
    // within NEAR_PARALLEL of parallel, it is made parallel. Drawn by hand
    // a degree or two off, the runs closed on each other and their
    // flights overlapped (MU103).
    let mut i = 1;
    while i + 2 < pts.len() {
        let d0 = (pts[i] - pts[i - 1]).normalize_or_zero();
        let mid = pts[i + 1] - pts[i];
        let d2 = (pts[i + 2] - pts[i + 1]).normalize_or_zero();
        let off = (-d0).angle_to(d2);
        if d0 != DVec2::ZERO
            && d2 != DVec2::ZERO
            && mid.length() < 2.0 * width + 1.0
            && off.abs() > 1e-12
            && off.abs() < NEAR_PARALLEL.to_radians()
        {
            let swing = DVec2::from_angle(-off);
            let pivot = pts[i + 1];
            for p in pts.iter_mut().skip(i + 2) {
                *p = pivot + swing.rotate(*p - pivot);
            }
        }
        i += 1;
    }
    (pts, added)
}

/// How far off parallel (degrees) a half-turn's run back may be drawn and
/// still be read as parallel to the run before.
pub const NEAR_PARALLEL: f64 = 12.0;

/// What line of a stair a guide runs along (Stair Guides REV02 §1, §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StairMark {
    /// Across its first riser.
    Foot,
    /// Across its last riser: where it arrives.
    Head,
    /// Across the first or last riser of a flight part way up.
    FlightFoot,
    FlightHead,
    /// Along a straight flight's side.
    Side,
    /// Along an edge of a landing.
    Landing,
    /// Through the centre a curved stair or spiral turns round.
    Centre,
}

impl StairMark {
    pub fn label(self) -> &'static str {
        match self {
            StairMark::Foot => "foot",
            StairMark::Head => "head",
            StairMark::FlightFoot => "flight's foot",
            StairMark::FlightHead => "flight's head",
            StairMark::Side => "side",
            StairMark::Landing => "landing edge",
            StairMark::Centre => "centre",
        }
    }
}

/// A line a stair offers to line others up with: through `a` and `b`
/// (endless), what it is, in plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StairGuide {
    pub mark: StairMark,
    pub a: DVec2,
    pub b: DVec2,
}

impl Stair {
    /// The lines it offers to line other stairs up with: the foot and
    /// head of the stair and of each flight, the sides of each straight
    /// flight, each landing's edges, and level and plumb lines through the
    /// centre a curved stair or spiral turns round.
    pub fn guide_lines(&self, rise: f64) -> Vec<StairGuide> {
        let lay = self.layout(rise);
        let (t, h) = (self.tread, self.width / 2.0);
        let mut out: Vec<StairGuide> = vec![];
        let mut add = |mark: StairMark, a: DVec2, b: DVec2| {
            if a.distance(b) < 1e-6 {
                return;
            }
            let d = (b - a).normalize();
            // one guide per line: the first, most telling, name wins
            let dup = out.iter().any(|g| {
                let e = (g.b - g.a).normalize_or_zero();
                d.perp_dot(e).abs() < 1e-6 && (a - g.a).perp_dot(e).abs() < 1e-6
            });
            if !dup {
                out.push(StairGuide { mark, a, b });
            }
        };
        let n = lay.flights.len();
        for (k, f) in lay.flights.iter().enumerate() {
            let top = f.risers.saturating_sub(1);
            let (r0, l0) = f.riser(0, t, h);
            let (r1, l1) = f.riser(top, t, h);
            add(
                if k == 0 {
                    StairMark::Foot
                } else {
                    StairMark::FlightFoot
                },
                r0,
                l0,
            );
            add(
                if k + 1 == n {
                    StairMark::Head
                } else {
                    StairMark::FlightHead
                },
                r1,
                l1,
            );
            if !f.curved() {
                add(StairMark::Side, r0, r1);
                add(StairMark::Side, l0, l1);
            }
        }
        for (l, _) in &lay.landings {
            for i in 0..l.len() {
                add(StairMark::Landing, l[i], l[(i + 1) % l.len()]);
            }
        }
        if let Some(c) = self.centre() {
            add(StairMark::Centre, c, c + DVec2::X);
            add(StairMark::Centre, c, c + DVec2::Y);
        }
        out
    }
}

/// `points` without repeats and without corners on a straight run.
fn clean_line(points: &[DVec2]) -> Vec<DVec2> {
    let mut out: Vec<DVec2> = vec![];
    for &p in points {
        if !p.is_finite() || out.last().is_some_and(|q| q.distance(p) < 1e-6) {
            continue;
        }
        if out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let (u, v) = ((b - a).normalize(), (p - b).normalize());
            if u.perp_dot(v).abs() < 1e-6 && u.dot(v) > 0.0 {
                *out.last_mut().unwrap() = p;
                continue;
            }
        }
        out.push(p);
    }
    out
}

/// A stair's walk line moved `by` to its left (right when `left` is
/// false), heights kept: the path a handrail follows. Only the line's
/// corners are mitred; each nosing on a straight is met square to it, and
/// each winder's riser along its own line, so on the inside of a turn the
/// points keep their order instead of folding back past the corner.
pub fn side_of(lay: &StairLayout, left: bool, by: f64) -> Vec<DVec3> {
    let walk = &lay.walk;
    let by = if left { by } else { -by };
    let plan: Vec<DVec2> = walk.iter().map(|p| p.truncate()).collect();
    if plan.len() < 2 {
        return walk.clone();
    }
    // the corners of the line: where it turns
    let mut key = vec![0usize];
    for i in 1..plan.len() - 1 {
        let a = (plan[i] - plan[*key.last().unwrap()]).normalize_or_zero();
        let b = (plan[i + 1] - plan[i]).normalize_or_zero();
        if a.perp_dot(b).abs() > 1e-6 || a.dot(b) < 0.0 {
            key.push(i);
        }
    }
    key.push(plan.len() - 1);
    let corners: Vec<DVec2> = key.iter().map(|&i| plan[i]).collect();
    let side = crate::offset::offset_path(&corners, by);
    let mut out: Vec<DVec3> = vec![];
    let mut last_at = f64::MIN;
    for (i, w) in walk.iter().enumerate() {
        let p = plan[i];
        let leg = key.iter().rposition(|&k| k <= i).unwrap_or(0);
        let at = if key[leg] == i {
            side[leg]
        } else if let Some((_, o)) = lay.winder_risers.iter().find(|(c, _)| c.distance(p) < 1e-9) {
            // along the winder's riser to this side
            let d = (*o - p).normalize_or_zero();
            let hits = [ray_hit(p, d, &side), ray_hit(p, -d, &side)];
            hits.into_iter()
                .flatten()
                .min_by(|a, b| a.distance(p).total_cmp(&b.distance(p)))
                .unwrap_or(p)
        } else {
            // square to its straight, kept within that straight's side
            let (a, b) = (side[leg], side[(leg + 1).min(side.len() - 1)]);
            let e = b - a;
            let t = if e.length_squared() > 0.0 {
                ((p - a).dot(e) / e.length_squared()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            a + e * t
        };
        // never back past the point before
        let s = param_on(&side, at);
        let at = match out.last() {
            Some(q) if s < last_at => q.truncate(),
            _ => at,
        };
        last_at = last_at.max(s);
        let pt = at.extend(w.z);
        if out.last().is_none_or(|q| q.distance(pt) > 1e-9) {
            out.push(pt);
        }
    }
    out
}

/// One corner of the walk line, where a landing goes.
struct Corner {
    v: DVec2,
    d1: DVec2,
    d2: DVec2,
    /// How far each flight stops short of the corner along the line.
    back: f64,
    inner: DVec2,
    outer_in: DVec2,
    outer: DVec2,
    outer_out: DVec2,
}

impl Corner {
    fn new(v: DVec2, d1: DVec2, d2: DVec2, h: f64) -> Self {
        let left = d1.perp_dot(d2) > 0.0;
        let turn = d1.angle_to(d2).abs().min(MAX_TURN.to_radians());
        let back = h * (turn / 2.0).tan();
        let side = |d: DVec2| if left { d.perp() } else { -d.perp() };
        let (n1, n2) = (side(d1), side(d2));
        let inner = v - d1 * back + n1 * h;
        let outer_in = v - d1 * back - n1 * h;
        let outer_out = v + d2 * back - n2 * h;
        // where the two outer sides meet
        let den = d1.perp_dot(d2);
        let outer = if den.abs() < 1e-9 {
            (outer_in + outer_out) / 2.0
        } else {
            outer_in + d1 * ((outer_out - outer_in).perp_dot(d2) / den)
        };
        Self {
            v,
            d1,
            d2,
            back,
            inner,
            outer_in,
            outer,
            outer_out,
        }
    }

    /// The landing's outline, its incoming side moved `deeper` back.
    fn outline(&self, deeper: f64) -> Vec<DVec2> {
        let b = self.d1 * deeper;
        let mut pts = vec![
            self.inner - b,
            self.outer_in - b,
            self.outer,
            self.outer_out,
        ];
        // made deeper at a turn that isn't square: its inside corner too,
        // or the edge the next flight leaves from runs a hair askew of it
        // (MU103's cross-check)
        if (self.inner - self.outer_out).perp_dot(b).abs() > 1e-9 {
            pts.push(self.inner);
        }
        ccw(pts)
    }
}

impl Corner {
    /// How many winders it takes by itself: one for each 30° it turns,
    /// one more for each going it was made deeper.
    fn winders(&self, deeper: f64, g: f64) -> u32 {
        let turn = self.d1.angle_to(self.d2).abs().min(MAX_TURN.to_radians());
        // whole goings only: half a going rounded up made a tread 60 %
        // deep before the turn (MU103)
        (turn.to_degrees() / 30.0).round().max(2.0) as u32 + (deeper / g + 1e-6).floor() as u32
    }

    /// `k` winders round it, the first riser `deeper` back from where a
    /// landing would start: each tread's outline, and each riser after the
    /// first where it crosses the walk line, with its outer end. One winder
    /// for each 30° fans out from the inside corner at equal angles; any
    /// more are square treads before the turn, each a going `g` deep where
    /// `deeper` has room for them (sharing it when asked for more than
    /// fit). What is left of `deeper` goes to the first winder, so no
    /// tread is shallower than a going and no riser meets the inside edge
    /// a sliver short of the corner.
    fn winders_round(
        &self,
        deeper: f64,
        k: u32,
        h: f64,
        g: f64,
    ) -> (Vec<Vec<DVec2>>, Vec<(DVec2, DVec2)>) {
        let left = self.d1.perp_dot(self.d2) > 0.0;
        let side = |d: DVec2| if left { d.perp() } else { -d.perp() };
        let b = self.d1 * deeper;
        let inner_out = self.v + self.d2 * self.back + side(self.d2) * h;
        let inner = clean_line(&[self.inner - b, self.inner, inner_out]);
        let outer = clean_line(&[self.outer_in - b, self.outer_in, self.outer, self.outer_out]);
        let walk = clean_line(&[
            self.v - self.d1 * (self.back + deeper),
            self.v,
            self.v + self.d2 * self.back,
        ]);
        let k = k.max(1);
        let turn = self.d1.angle_to(self.d2).abs().min(MAX_TURN.to_radians());
        let fanned = ((turn.to_degrees() / 30.0).round().max(2.0) as u32).min(k);
        // square treads before the turn: a going each, or a fair share of
        // `deeper` when more were asked for than fit at a going
        let square = if deeper > 0.05 * (k - fanned) as f64 {
            k - fanned
        } else {
            0
        };
        let fanned = k - square;
        let step = if square > 0 {
            g.min(deeper / square as f64)
        } else {
            0.0
        };
        let corner = *inner.last().unwrap();
        let (a0, a1) = (
            *outer.first().unwrap() - *inner.first().unwrap(),
            *outer.last().unwrap() - corner,
        );
        let fan = a0.angle_to(a1);
        // the risers: on the inside and the outside
        let mut risers: Vec<(DVec2, DVec2)> = vec![(inner[0], outer[0])];
        for q in 1..=square {
            let by = self.d1 * (step * q as f64);
            risers.push((inner[0] + by, outer[0] + by));
        }
        for q in 1..fanned {
            let dir = DVec2::from_angle(fan * q as f64 / fanned as f64).rotate(a0);
            let out = ray_hit(corner, dir, &outer).unwrap_or(corner + dir);
            risers.push((corner, out));
        }
        risers.push((*inner.last().unwrap(), *outer.last().unwrap()));
        let at = |path: &[DVec2], p: DVec2| param_on(path, p);
        let mut treads = vec![];
        for q in 0..k as usize {
            let (i0, o0) = risers[q];
            let (i1, o1) = risers[q + 1];
            let (s0, s1) = (at(&inner, i0), at(&inner, i1));
            let (t0, t1) = (at(&outer, o0), at(&outer, o1));
            let mut pts = vec![i0];
            pts.extend(between(&inner, s0, s1));
            pts.push(i1);
            pts.push(o1);
            let mut back = between(&outer, t0, t1);
            back.reverse();
            pts.extend(back);
            pts.push(o0);
            let mut clean: Vec<DVec2> = vec![];
            for p in pts {
                if clean.last().is_none_or(|q: &DVec2| q.distance(p) > 1e-9) {
                    clean.push(p);
                }
            }
            if clean.len() > 2 && clean[0].distance(*clean.last().unwrap()) < 1e-9 {
                clean.pop();
            }
            treads.push(ccw(clean));
        }
        // where each inner riser crosses the walk line, with its outer end
        let crossings = risers[1..k as usize]
            .iter()
            .map(|(i, o)| {
                let p = walk
                    .windows(2)
                    .find_map(|w| segment_cross(*i, *o, w[0], w[1]))
                    .unwrap_or((*i + *o) / 2.0);
                (p, *o)
            })
            .collect();
        (treads, crossings)
    }
}

fn path_length(pts: &[DVec2]) -> f64 {
    pts.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// The point `at` along path `pts`.
fn point_along(pts: &[DVec2], at: f64) -> DVec2 {
    let mut left = at;
    for w in pts.windows(2) {
        let l = w[0].distance(w[1]);
        if left <= l {
            return w[0].lerp(w[1], if l > 0.0 { left / l } else { 0.0 });
        }
        left -= l;
    }
    *pts.last().unwrap_or(&DVec2::ZERO)
}

/// How far along path `pts` its nearest point to `p` is.
fn param_on(pts: &[DVec2], p: DVec2) -> f64 {
    let mut best = (f64::MAX, 0.0);
    let mut run = 0.0;
    for w in pts.windows(2) {
        let d = w[1] - w[0];
        let l2 = d.length_squared();
        let t = if l2 > 0.0 {
            ((p - w[0]).dot(d) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let dist = (w[0] + d * t).distance(p);
        if dist < best.0 - 1e-12 {
            best = (dist, run + t * l2.sqrt());
        }
        run += l2.sqrt();
    }
    best.1
}

/// The corners of path `pts` strictly between lengths `a` and `b`.
fn between(pts: &[DVec2], a: f64, b: f64) -> Vec<DVec2> {
    let mut out = vec![];
    let mut run = 0.0;
    for (i, p) in pts.iter().enumerate() {
        if i > 0 {
            run += pts[i - 1].distance(*p);
        }
        if run > a + 1e-9 && run < b - 1e-9 {
            out.push(*p);
        }
    }
    out
}

/// Where the ray from `from` along `dir` first meets path `pts`.
fn ray_hit(from: DVec2, dir: DVec2, pts: &[DVec2]) -> Option<DVec2> {
    let mut best: Option<(f64, DVec2)> = None;
    for w in pts.windows(2) {
        let e = w[1] - w[0];
        let den = dir.perp_dot(e);
        if den.abs() < 1e-12 {
            continue;
        }
        let t = (w[0] - from).perp_dot(e) / den;
        let u = (w[0] - from).perp_dot(dir) / den;
        if t > 1e-9 && (-1e-9..=1.0 + 1e-9).contains(&u) && best.is_none_or(|(bt, _)| t < bt) {
            best = Some((t, from + dir * t));
        }
    }
    best.map(|(_, p)| p)
}

/// Where segments `a`–`b` and `c`–`d` cross.
fn segment_cross(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> Option<DVec2> {
    let (r, s) = (b - a, d - c);
    let den = r.perp_dot(s);
    if den.abs() < 1e-12 {
        return None;
    }
    let t = (c - a).perp_dot(s) / den;
    let u = (c - a).perp_dot(r) / den;
    ((-1e-9..=1.0 + 1e-9).contains(&t) && (-1e-9..=1.0 + 1e-9).contains(&u)).then(|| a + r * t)
}

fn ccw(mut pts: Vec<DVec2>) -> Vec<DVec2> {
    if crate::polygon::signed_area2(&pts) < 0.0 {
        pts.reverse();
    }
    pts
}

/// The most one straight of a stair's line bends, in degrees: more
/// goes round in several arcs.
pub const MAX_BEND: f64 = 180.0;

/// The centre of the arc from `a` to `b` turning through `sweep`
/// (radians, left positive).
pub fn arc_centre(a: DVec2, b: DVec2, sweep: f64) -> DVec2 {
    let c = b - a;
    a + c / 2.0 + c.perp().normalize_or_zero() * (c.length() / 2.0) / (sweep / 2.0).tan()
}

/// The points strictly inside the arc from `a` to `b` turning through
/// `deg` degrees (left positive), at most 5° apart; none when straight.
pub fn arc_between(a: DVec2, b: DVec2, deg: f64) -> Vec<DVec2> {
    if deg.abs() < 1e-9 || a.distance(b) < 1e-9 {
        return vec![];
    }
    let sweep = deg.to_radians();
    let c = arc_centre(a, b, sweep);
    let k = (deg.abs() / 5.0).ceil().max(1.0) as usize;
    (1..k)
        .map(|j| c + DVec2::from_angle(sweep * j as f64 / k as f64).rotate(a - c))
        .collect()
}

/// Turns gentler than this (degrees) keep a flight going round them: a
/// curve, drawn as a finely bent line.
pub const SMOOTH_TURN: f64 = 15.0;

/// A walk line cut at its turns into straights and curves.
fn split_runs(walk: &[DVec2]) -> Vec<Vec<DVec2>> {
    let mut runs = vec![vec![walk[0]]];
    for k in 1..walk.len() {
        runs.last_mut().unwrap().push(walk[k]);
        if k + 1 < walk.len() {
            let a = (walk[k] - walk[k - 1]).normalize();
            let b = (walk[k + 1] - walk[k]).normalize();
            if a.angle_to(b).abs().to_degrees() >= SMOOTH_TURN {
                runs.push(vec![walk[k]]);
            }
        }
    }
    runs
}

/// A flight's path along `run` from `from` (at `start`), and its first
/// direction.
fn flight_path(run: &[DVec2], from: f64, start: &DVec2, d0: DVec2) -> (Vec<DVec2>, DVec2) {
    let mut path = vec![*start];
    let mut acc = 0.0;
    for w in run.windows(2) {
        acc += w[0].distance(w[1]);
        if acc > from + 1e-9 {
            path.push(w[1]);
        }
    }
    if path.len() < 2 {
        path.push(*start + d0);
    }
    let d = (path[1] - path[0]).normalize_or(d0);
    (path, d)
}

/// The fewest treads a flight between two landings has; a shorter
/// straight between two turns becomes part of one landing.
pub const MIN_FLIGHT_TREADS: u32 = 2;

/// Lay `n` risers of `r` along the walk line `walk` (plan), `width` wide
/// with goings of `g`, keeping the changes `pieces` made to single
/// flights and landings.
pub fn layout_walk(
    walk: &[DVec2],
    n: u32,
    r: f64,
    width: f64,
    g: f64,
    pieces: &StairPieces,
    at_turns: TurnKind,
) -> StairLayout {
    let h = width / 2.0;
    let walk = clean_line(walk);
    let walk = if walk.len() >= 2 {
        walk
    } else {
        let a = walk.first().copied().unwrap_or(DVec2::ZERO);
        vec![a, a + DVec2::X * g]
    };
    // its straights and curves (a finely bent line), between its turns
    let runs = split_runs(&walk);
    let m = runs.len();
    let seg: Vec<(DVec2, DVec2, f64)> = runs
        .iter()
        .map(|r| (r[0], (r[1] - r[0]).normalize(), path_length(r)))
        .collect();
    let end_dir = |r: &Vec<DVec2>| (r[r.len() - 1] - r[r.len() - 2]).normalize();
    let corners: Vec<Corner> = (1..m)
        .map(|k| Corner::new(runs[k][0], end_dir(&runs[k - 1]), seg[k].1, h))
        .collect();
    // a point along run `i`, on past its ends
    let along = |i: usize, u: f64| {
        let r = &runs[i];
        if u <= 0.0 {
            return r[0] + seg[i].1 * u;
        }
        let l = seg[i].2;
        if u >= l {
            return r[r.len() - 1] + end_dir(r) * (u - l);
        }
        point_along(r, u)
    };
    let back = |k: usize| corners.get(k).map_or(0.0, |c| c.back);
    // usable length of each straight, between the landings
    let usable: Vec<f64> = (0..m)
        .map(|i| {
            seg[i].2 - if i > 0 { back(i - 1) } else { 0.0 } - if i + 1 < m { back(i) } else { 0.0 }
        })
        .collect();
    // a straight between two corners too short for a flight of three
    // risers joins their landings into one (a half-turn stair)
    // what each turn becomes, and how many winders
    let kind = |k: usize| pieces.turn_kind(k).unwrap_or(at_turns);
    let landing_at = |k: usize| kind(k) == TurnKind::Landing;
    // ...joined only between two landings: winders turn one after the other
    let joined: Vec<bool> = (0..m)
        .map(|i| {
            i > 0
                && i + 1 < m
                && landing_at(i - 1)
                && landing_at(i)
                && usable[i] < MIN_FLIGHT_TREADS as f64 * g - 1e-6
        })
        .collect();
    let need = n.saturating_sub(1);
    let mut flights: Vec<Flight> = vec![];
    let mut landings: Vec<(Vec<DVec2>, f64)> = vec![];
    let mut turns: Vec<usize> = vec![];
    let mut winders: Vec<Winder> = vec![];
    let mut walk3: Vec<DVec3> = vec![];
    let mut winder_risers: Vec<(DVec2, DVec2)> = vec![];
    let push = |walk3: &mut Vec<DVec3>, p: DVec2, z: f64| {
        if walk3.last().is_none_or(|q| q.truncate().distance(p) > 1e-9) {
            walk3.push(p.extend(z));
        }
    };
    // levels climbed so far: treads and landings
    let mut done: u32 = 0;
    let mut i = 0;
    let mut spare = 0.0;
    let mut short = 0;
    let mut last_was_landing = false;
    // how far past its corner the next flight starts: after a half-turn
    // landing made deeper, level with the flight that came up to it
    let mut lead = 0.0;
    while i < m {
        let from = if i > 0 { back(i - 1) } else { 0.0 } + lead;
        let start = along(i, from);
        let d = flight_path(&runs[i], from, &start, seg[i].1);
        if !joined[i] {
            let last_line = i + 1 == m;
            // a deeper landing next: this flight stops that much sooner
            let room = usable[i] - lead - if last_line { 0.0 } else { pieces.deeper(i) };
            let fits = (room / g + 1e-6).floor().max(0.0) as u32;
            let left = need - done;
            let mut t = fits.min(left);
            if last_line && fits < left {
                short = left - fits;
                t = left;
            }
            // a flight given its own treads, where they fit (the last
            // flight always takes the rest)
            if let Some(own) = pieces.treads(i).filter(|_| !last_line) {
                t = own.min(fits).min(left);
            }
            let z0 = done as f64 * r;
            let flight = Flight {
                start,
                dir: d.1,
                risers: t + 1,
                z0,
                straight: i,
                path: d.0,
            };
            push(&mut walk3, start, z0 + r);
            // round a curve, over each nosing and each bend of the line
            if flight.curved() {
                let mut marks: Vec<(f64, f64)> = (1..t)
                    .map(|j| (j as f64 * g, z0 + (j + 1) as f64 * r))
                    .collect();
                let mut run = 0.0;
                for w in flight.path.windows(2) {
                    run += w[0].distance(w[1]);
                    if run < t as f64 * g - 1e-9 {
                        marks.push((run, z0 + r + run / g * r));
                    }
                }
                marks.sort_by(|a, b| a.0.total_cmp(&b.0));
                for (u, z) in marks {
                    push(&mut walk3, flight.at(u).0, z);
                }
            }
            let end = flight.end(g);
            flights.push(flight);
            done += t;
            push(&mut walk3, end, done as f64 * r + r);
            if done >= need {
                spare = (seg[i].2 - from - t as f64 * g).max(0.0)
                    + seg[i + 1..].iter().map(|s| s.2).sum::<f64>();
                last_was_landing = false;
                break;
            }
            // the rest of this straight makes the landing deeper
            let deeper = (usable[i] - lead - t as f64 * g).max(0.0);
            lead = 0.0;
            let c = &corners[i];
            let z = done as f64 * r + r;
            let entry = end;
            if let TurnKind::Winders(asked) = kind(i) {
                let k = if asked == 0 {
                    c.winders(deeper, g)
                } else {
                    asked.max(2)
                };
                let k = k.min(need - done).max(1);
                let (treads, cross) = c.winders_round(deeper, k, h, g);
                winder_risers.extend(cross.iter().copied());
                let cross: Vec<DVec2> = cross.into_iter().map(|(p, _)| p).collect();
                for (q, outline) in treads.into_iter().enumerate() {
                    winders.push(Winder {
                        outline,
                        z: z + q as f64 * r,
                        turn: i,
                    });
                }
                // the walk line over each winder's nosing, round the corner
                push(&mut walk3, entry, z);
                let mut corner_done = false;
                for (q, p) in cross.iter().enumerate() {
                    let zq = z + (q + 1) as f64 * r;
                    // past the corner (on the way out): the corner first
                    if !corner_done && p.distance(c.v) < 1e-9 {
                        corner_done = true;
                    } else if !corner_done && (*p - c.v).dot(c.d2) > 1e-9 {
                        push(&mut walk3, c.v, zq - r / 2.0);
                        corner_done = true;
                    }
                    push(&mut walk3, *p, zq);
                }
                if !corner_done {
                    push(&mut walk3, c.v, z + (k as f64 - 0.5) * r);
                }
                done += k;
                i += 1;
                last_was_landing = true;
                if done >= need {
                    break;
                }
                continue;
            }
            if i + 1 < m && joined[i + 1] && i + 2 < m {
                let c2 = &corners[i + 1];
                let mut pts = c.outline(deeper);
                pts.extend(c2.outline(0.0));
                // a half-turn (the run back parallel): the landing is
                // square, its edge straight across both runs, so the next
                // flight starts level with this one's end (MU103: the edge
                // ran askew across the gap)
                if c.d1.dot(c2.d2) < -0.999 && deeper > 1e-9 {
                    let far = c2.d2 * deeper;
                    let left = c2.d1.perp_dot(c2.d2) > 0.0;
                    let n2 = if left { c2.d2.perp() } else { -c2.d2.perp() };
                    pts.push(c2.outer_out + far);
                    pts.push(c2.v + c2.d2 * c2.back + n2 * h + far);
                    lead = deeper;
                }
                landings.push((crate::objects::hull(&pts), z));
                turns.push(i);
                push(&mut walk3, entry, z);
                push(&mut walk3, c.v, z);
                push(&mut walk3, c2.v, z);
                i += 2;
            } else {
                landings.push((c.outline(deeper), z));
                turns.push(i);
                push(&mut walk3, entry, z);
                push(&mut walk3, c.v, z);
                i += 1;
            }
            done += 1;
            last_was_landing = true;
            if done >= need {
                break;
            }
        } else {
            i += 1;
        }
    }
    // a landing reached the top level: one riser more, up from its far side
    if last_was_landing && done >= need {
        let k = i.min(m - 1);
        let from = if k > 0 { back(k - 1) } else { 0.0 } + lead;
        let start = along(k, from);
        let d = flight_path(&runs[k], from, &start, seg[k].1);
        flights.push(Flight {
            start,
            dir: d.1,
            path: d.0,
            risers: 1,
            z0: done as f64 * r,
            straight: k,
        });
        push(&mut walk3, start, done as f64 * r + r);
    }
    let walk = walk3;
    StairLayout {
        risers: n,
        riser: r,
        flights,
        landings,
        turns,
        winders,
        walk,
        winder_risers,
        short,
        spare,
    }
}

/// How many equal risers climb `rise` with none taller than `max_riser`.
pub fn riser_count(rise: f64, max_riser: f64) -> u32 {
    if !(rise > 0.0 && max_riser > 0.0) {
        return 1;
    }
    ((rise / max_riser) - 1e-9).ceil().clamp(1.0, 500.0) as u32
}

/// Which side of a stair a railing runs along.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StairSide {
    pub stair: ElementId,
    pub left: bool,
}

/// A railing: posts and a top rail along a path, or along a stair's side.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Railing {
    pub level: ElementId,
    /// Points of the path, heights above the level. Ignored while the
    /// railing follows a stair.
    pub path: Vec<DVec3>,
    pub height: f64,
    pub post_spacing: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stair: Option<StairSide>,
    #[serde(default)]
    pub material: Option<ElementId>,
    /// The top rail's profile from the library (MU36); none: a square bar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rail_profile: Option<String>,
    /// On a stair: how far the handrail runs on past the bottom and top
    /// nosings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extend: Option<RailExtend>,
    /// Along a wall: a handrail on brackets, clear of the wall, its ends
    /// turned back to the wall; no posts and no bottom rail.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub on_wall: bool,
    /// What fills a guard between its top rail and the floor.
    #[serde(default, skip_serializing_if = "RailInfill::is_rail")]
    pub infill: RailInfill,
    /// A guard taller than its handrail: its top rail at this height, the
    /// handrail on its posts at `height`. None: one rail, at `height`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guard_height: Option<f64>,
    /// Heavier posts at its ends and corners; where it climbs steeply (round
    /// the inside of winders) its rail stops at one and starts again higher.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub newels: bool,
}

/// What fills a guard below its top rail.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RailInfill {
    /// A bottom rail only.
    #[default]
    Rail,
    /// Upright balusters, no gap a 100 mm sphere passes.
    Balusters,
    /// A glass panel between each pair of posts.
    Glass,
}

impl RailInfill {
    pub const ALL: [RailInfill; 3] = [RailInfill::Rail, RailInfill::Balusters, RailInfill::Glass];

    pub fn is_rail(&self) -> bool {
        *self == RailInfill::Rail
    }

    pub fn label(self) -> &'static str {
        match self {
            RailInfill::Rail => "Bottom rail",
            RailInfill::Balusters => "Balusters",
            RailInfill::Glass => "Glass",
        }
    }
}

/// A baluster's side, and the clear gap between two: under the 100 mm a
/// guard's openings must stop.
pub const BALUSTER: f64 = 0.02;
pub const BALUSTER_GAP: f64 = 0.095;
/// A newel post's side.
pub const NEWEL: f64 = 0.09;
/// Glass in a guard: its thickness.
pub const GLASS: f64 = 0.012;
/// A rail steeper than this (rise over plan run) stops at a newel and
/// starts again higher.
pub const STEEP_RAIL: f64 = 1.0;

/// Can a hand grip a rail of this section? Circular, 30 to 43 mm across;
/// otherwise 100 to 125 mm round and no more than 45 mm across (NBC
/// 9.8.7.5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandGrip {
    /// Its largest dimension across, m.
    pub across: f64,
    /// Its perimeter, m.
    pub perimeter: f64,
    pub round: bool,
    pub ok: bool,
}

/// The grip of a rail section `outline` (None: the square bar).
pub fn hand_grip(outline: Option<&[DVec2]>) -> HandGrip {
    let square = [
        DVec2::new(-RAIL_SIZE / 2.0, -RAIL_SIZE / 2.0),
        DVec2::new(RAIL_SIZE / 2.0, -RAIL_SIZE / 2.0),
        DVec2::new(RAIL_SIZE / 2.0, RAIL_SIZE / 2.0),
        DVec2::new(-RAIL_SIZE / 2.0, RAIL_SIZE / 2.0),
    ];
    // what the hand closes round: its outside (a tube's outline runs
    // round its bore too, which read a round tube as a shaped rail: MU103)
    let outside = crate::objects::hull(outline.filter(|o| o.len() >= 3).unwrap_or(&square));
    let pts = &outside;
    let n = pts.len();
    let perimeter: f64 = (0..n).map(|i| pts[i].distance(pts[(i + 1) % n])).sum();
    let mut across: f64 = 0.0;
    for a in pts {
        for b in pts {
            across = across.max(a.distance(*b));
        }
    }
    // round: every corner about as far from the middle
    // (its middle by its extents: a point doubled on the outline, as
    // where a tube's wall closes, pulls the points' mean off centre)
    let (min, max) = pts
        .iter()
        .fold((pts[0], pts[0]), |(a, b), p| (a.min(*p), b.max(*p)));
    let c = (min + max) / 2.0;
    let (lo, hi) = pts.iter().fold((f64::MAX, 0.0f64), |(lo, hi), p| {
        let d = p.distance(c);
        (lo.min(d), hi.max(d))
    });
    let round = n >= 8 && hi - lo < 0.05 * hi;
    let ok = if round {
        (0.030..=0.043).contains(&across)
    } else {
        (0.100..=0.125).contains(&perimeter) && across <= 0.045
    };
    HandGrip {
        across,
        perimeter,
        round,
        ok,
    }
}

/// How a stair's handrail runs on past its ends.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RailExtend {
    /// Level run past the bottom, after the slope (if any).
    pub bottom: f64,
    /// Level run past the top nosing.
    pub top: f64,
    /// The rail keeps its slope for one going past the bottom nosing
    /// before it runs level.
    #[serde(default)]
    pub slope_bottom: bool,
}

impl Default for RailExtend {
    /// One going sloped and 300 level at the bottom, 300 at the top: the
    /// accessible-stair ends most rules ask for.
    fn default() -> Self {
        Self {
            bottom: 0.3,
            top: 0.3,
            slope_bottom: true,
        }
    }
}

/// Rail and post section sizes.
pub const RAIL_SIZE: f64 = 0.05;
pub const POST_SIZE: f64 = 0.04;
/// The clear gap between a wall handrail and its wall.
pub const WALL_GAP: f64 = 0.05;

impl Railing {
    pub fn new(level: ElementId, path: Vec<DVec3>) -> Self {
        Self {
            level,
            path,
            height: 1.0,
            post_spacing: 1.2,
            stair: None,
            material: None,
            rail_profile: None,
            extend: None,
            on_wall: false,
            infill: RailInfill::Rail,
            guard_height: None,
            newels: false,
        }
    }

    /// The height of its top rail: the guard's, else the handrail's.
    pub fn top_height(&self) -> f64 {
        self.guard_height
            .map_or(self.height, |g| g.max(self.height))
    }

    /// A railing along one side of `stair`.
    pub fn along_stair(model: &Model, stair: ElementId, left: bool) -> Option<Self> {
        let Some(ElementKind::Stair(s)) = model.get(stair).map(|e| &e.kind) else {
            return None;
        };
        let mut r = Self::new(s.level, vec![]);
        r.stair = Some(StairSide { stair, left });
        Some(r)
    }

    /// The path as built, in model coordinates.
    pub fn path_in(&self, model: &Model) -> Vec<DVec3> {
        if let Some(side) = self.stair {
            if let Some(ElementKind::Stair(s)) = model.get(side.stair).map(|e| &e.kind) {
                let z = s.base(model);
                return stair_rail(s, s.rise(model), side.left, self.on_wall, self.extend)
                    .into_iter()
                    .map(|p| p + DVec3::Z * z)
                    .collect();
            }
        }
        let z = elevation(model, self.level);
        self.path.iter().map(|p| *p + DVec3::Z * z).collect()
    }

    pub fn length(&self, model: &Model) -> f64 {
        self.path_in(model)
            .windows(2)
            .map(|w| w[0].truncate().distance(w[1].truncate()))
            .sum()
    }

    /// Where posts stand: every corner, and in between no further apart
    /// than the spacing.
    pub fn posts(&self, model: &Model) -> Vec<DVec3> {
        posts_along(&self.path_in(model), self.post_spacing)
    }
}

/// The path of a handrail along one side of a stair, heights above its
/// base: over the nosings, continuous round its turns, run on past its
/// ends by `extend`, and on a wall turned back to the wall at both ends.
pub fn stair_rail(
    s: &Stair,
    rise: f64,
    left: bool,
    on_wall: bool,
    extend: Option<RailExtend>,
) -> Vec<DVec3> {
    let lay = s.layout(rise);
    let inset = if on_wall {
        WALL_GAP + RAIL_SIZE / 2.0
    } else {
        RAIL_SIZE
    };
    let mut p = side_of(&lay, left, s.width / 2.0 - inset);
    // a split stair's shared flight has no rail down its middle: the side
    // along the mirror line starts at the landing
    let mut extend = extend;
    if let Some(m) = s.mirror_of(&lay).filter(|m| m.left == left) {
        // it runs along the mirror line (the rail's inset off it) up the
        // flight and across the landing; it starts where it leaves it
        let off = |v: &DVec3| ((v.truncate() - m.origin).perp_dot(m.dir).abs() - inset).abs();
        if let Some(k) = p.iter().position(|v| off(v) > 1e-6) {
            if k > 0 {
                p.drain(..k - 1);
                // from the mirror line itself, so it meets its mirror
                // image there at one newel
                let n = m.dir.perp();
                let off = (p[0].truncate() - m.origin).dot(n);
                p[0] -= (n * off).extend(0.0);
            }
        }
        extend = extend.map(|e| RailExtend {
            bottom: 0.0,
            slope_bottom: false,
            ..e
        });
    }
    if p.len() < 2 {
        return p;
    }
    let flat = |v: DVec3| v.truncate().normalize_or_zero().extend(0.0);
    let d0 = flat(p[1] - p[0]);
    let d1 = flat(p[p.len() - 1] - p[p.len() - 2]);
    if let Some(e) = extend {
        let mut head = vec![];
        let mut first = p[0];
        if e.slope_bottom {
            first = first - d0 * s.tread - DVec3::Z * lay.riser;
            head.push(first);
        }
        if e.bottom > 1e-9 {
            head.push(first - d0 * e.bottom);
        }
        head.reverse();
        p.splice(0..0, head);
        if e.top > 1e-9 {
            let last = p[p.len() - 1];
            p.push(last + d1 * e.top);
        }
    }
    if on_wall {
        // back to the wall, square to the rail's ends
        let towards = |d: DVec3| {
            let n = d.truncate().perp();
            (if left { n } else { -n }).extend(0.0) * inset
        };
        let (a, b) = (p[0], p[p.len() - 1]);
        let (da, db) = (flat(p[1] - p[0]), flat(b - p[p.len() - 2]));
        p.insert(0, a + towards(da));
        p.push(b + towards(db));
    }
    p
}

/// Does a wall stand along the `left` (else right) side of `s`, within
/// 15 cm of its edge over most of the stair's length, and up beside it?
/// Its railing there is then a wall handrail.
pub fn stair_side_on_wall(model: &Model, s: &Stair, left: bool) -> bool {
    let rise = s.rise(model);
    let base = s.base(model);
    let lay = s.layout(rise);
    let probe = side_of(&lay, left, s.width / 2.0 + 0.1);
    let walls: Vec<Vec<DVec2>> = model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Wall(w) => {
                let z0 = elevation(model, w.level) + w.base_offset;
                let up = z0 < base + 0.5 && z0 + w.height > base + 1.0;
                let d = (w.end - w.start).normalize_or_zero().perp() * (w.thickness / 2.0 + 0.06);
                up.then(|| vec![w.start - d, w.end - d, w.end + d, w.start + d])
            }
            _ => None,
        })
        .collect();
    let mut hits = 0;
    let mut tries = 0;
    for w in probe.windows(2) {
        let (a, b) = (w[0].truncate(), w[1].truncate());
        if a.distance(b) < 0.2 {
            continue;
        }
        for t in [0.2, 0.5, 0.8] {
            tries += 1;
            let p = a.lerp(b, t);
            if walls.iter().any(|o| crate::polygon::contains(o, p)) {
                hits += 1;
            }
        }
    }
    tries > 0 && hits * 3 >= tries * 2
}

/// Post positions along `path`: each corner in plan, and evenly between.
/// Where the path only climbs (over winders, say), or bends a little at a
/// time round a curve, it gets no post of its own, so posts don't bunch
/// up round the inside of a turn or along an arc.
pub fn posts_along(path: &[DVec3], spacing: f64) -> Vec<DVec3> {
    let plan: Vec<DVec2> = path.iter().map(|p| p.truncate()).collect();
    // how far along the path in plan each point is
    let mut run = vec![0.0];
    for w in plan.windows(2) {
        run.push(run.last().unwrap() + w[0].distance(w[1]));
    }
    let at = |d: f64| -> DVec3 {
        for i in 1..path.len() {
            if run[i] >= d - 1e-9 && run[i] > run[i - 1] {
                let t = ((d - run[i - 1]) / (run[i] - run[i - 1])).clamp(0.0, 1.0);
                return path[i - 1].lerp(path[i], t);
            }
        }
        *path.last().unwrap()
    };
    // the corners in plan
    let mut key = vec![0usize];
    for i in 1..path.len().saturating_sub(1) {
        // the way in: from the last point before it in plan
        let before = (0..i).rev().find(|&j| plan[j].distance(plan[i]) > 1e-9);
        let a = before.map_or(DVec2::ZERO, |j| (plan[i] - plan[j]).normalize_or_zero());
        let b = (plan[i + 1] - plan[i]).normalize_or_zero();
        // a real corner, not one bend of a curve drawn as a fine polyline
        let sharp = a.angle_to(b).abs() > (SMOOTH_TURN / 2.0).to_radians();
        if a != DVec2::ZERO && b != DVec2::ZERO && sharp {
            key.push(i);
        }
    }
    let mut out = Vec::new();
    if path.is_empty() {
        return out;
    }
    key.push(path.len() - 1);
    for w in key.windows(2) {
        let (d0, d1) = (run[w[0]], run[w[1]]);
        let len = d1 - d0;
        let n = if spacing > 0.0 {
            (len / spacing - 1e-9).ceil().max(1.0) as usize
        } else {
            1
        };
        for i in 0..n {
            out.push(if i == 0 {
                path[w[0]]
            } else {
                at(d0 + len * i as f64 / n as f64)
            });
        }
    }
    out.push(*path.last().unwrap());
    out
}

/// The elevation of a level, 0 when it is missing.
/// How far below height `z` the underside of a slab at plan point `at`
/// lies, for a slab that `z` is inside of (its top at or above `z`, its
/// underside below). None when no slab there holds `z`.
pub fn slab_depth_at(model: &Model, at: DVec2, z: f64) -> Option<f64> {
    model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Slab(s) => Some(s),
            _ => None,
        })
        .filter_map(|s| {
            let top = s.top(elevation(model, s.level));
            let bottom = top - s.thickness;
            (top >= z - 1e-3 && bottom < z - 1e-3 && crate::polygon::contains(&s.outline(), at))
                .then_some(z - bottom)
        })
        .fold(None, |m: Option<f64>, d| Some(m.map_or(d, |m| m.max(d))))
}

/// Where a new beam on `level` from `a` to `b` hangs: its top at the
/// underside of the slab above, so it never sits inside it.
pub fn beam_top_offset(model: &Model, level: ElementId, a: DVec2, b: DVec2) -> f64 {
    let top = elevation(model, level) + storey_height(model, level);
    slab_depth_at(model, (a + b) / 2.0, top).unwrap_or(0.0)
}

pub fn elevation(model: &Model, level: ElementId) -> f64 {
    model.level(level).map_or(0.0, |l| l.elevation)
}

/// The nearest level above `level`.
pub fn level_above(model: &Model, level: ElementId) -> Option<ElementId> {
    let z = model.level(level)?.elevation;
    model
        .levels()
        .filter(|(_, l)| l.elevation > z + 1e-6)
        .min_by(|a, b| a.1.elevation.total_cmp(&b.1.elevation))
        .map(|(e, _)| e.id)
}

/// Floor-to-floor height of a level: up to the next level above, else
/// the level's own height.
pub fn storey_height(model: &Model, level: ElementId) -> f64 {
    let Some(l) = model.level(level) else {
        return 3.0;
    };
    model
        .levels()
        .map(|(_, o)| o.elevation - l.elevation)
        .filter(|d| *d > 1e-6)
        .fold(None, |m: Option<f64>, d| Some(m.map_or(d, |m| m.min(d))))
        .unwrap_or(l.height)
}

fn positive(v: f64, what: &str) -> Result<(), String> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(format!("{what} must be positive, got {v}"))
    }
}

fn finite(v: f64, what: &str) -> Result<(), String> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(format!("{what} must be a number"))
    }
}

/// Checks for the elements in this module.
pub fn validate(kind: &ElementKind) -> Result<(), String> {
    match kind {
        ElementKind::Column(c) => {
            c.profile.check()?;
            finite(c.base_offset, "column base offset")?;
            finite(c.rotation, "column rotation")?;
            if !c.position.is_finite() {
                return Err("column position must be finite".into());
            }
            if let crate::WallTop::UpToLevel { offset, .. } = c.top {
                finite(offset, "column top offset")?;
            }
            match c.height {
                Some(h) => positive(h, "column height"),
                None => Ok(()),
            }
        }
        ElementKind::Beam(b) => {
            positive(b.width, "beam width")?;
            positive(b.depth, "beam depth")?;
            finite(b.top_offset, "beam top offset")?;
            positive(b.length(), "beam length")
        }
        ElementKind::Roof(r) => {
            positive(r.thickness, "roof thickness")?;
            finite(r.base_offset, "roof base offset")?;
            if !(r.overhang.is_finite() && r.overhang >= 0.0) {
                return Err("roof overhang must not be negative".into());
            }
            if r.boundary.len() < 3 || r.boundary.iter().any(|p| !p.is_finite()) {
                return Err("a roof outline needs at least 3 corners".into());
            }
            if r.shape != RoofShape::Flat && !(r.pitch > 0.0 && r.pitch < 80.0) {
                return Err(format!(
                    "roof pitch must be between 0° and 80°, got {}",
                    r.pitch
                ));
            }
            crate::slab::check_holes(&r.holes)
        }
        ElementKind::Stair(s) => {
            positive(s.width, "stair width")?;
            positive(s.tread, "tread depth")?;
            positive(s.max_riser, "riser height")?;
            positive(s.waist, "stair waist")?;
            finite(s.base_offset, "stair base offset")?;
            finite(s.direction, "stair direction")?;
            if let Some(h) = s.height {
                positive(h, "stair rise")?;
            }
            if !s.start.is_finite() {
                return Err("stair start must be finite".into());
            }
            Ok(())
        }
        ElementKind::Footing(f) => f.validate(),
        ElementKind::Railing(r) => {
            positive(r.height, "railing height")?;
            positive(r.post_spacing, "post spacing")?;
            if r.stair.is_none() && r.path.len() < 2 {
                return Err("a railing path needs two points".into());
            }
            if r.path.iter().any(|p| !p.is_finite()) {
                return Err("railing points must be finite".into());
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The level of a member, if `kind` is one.
pub fn level_of(kind: &ElementKind) -> Option<ElementId> {
    Some(match kind {
        ElementKind::Column(c) => c.level,
        ElementKind::Beam(b) => b.level,
        ElementKind::Roof(r) => r.level,
        ElementKind::Stair(s) => s.level,
        ElementKind::Railing(r) => r.level,
        ElementKind::Footing(f) => f.level,
        ElementKind::Part(p) => p.level,
        ElementKind::Site(s) => s.level,
        _ => return None,
    })
}

/// The level reference of a member, to repoint when pasting.
pub fn level_mut(kind: &mut ElementKind) -> Option<&mut ElementId> {
    Some(match kind {
        ElementKind::Column(c) => &mut c.level,
        ElementKind::Beam(b) => &mut b.level,
        ElementKind::Roof(r) => &mut r.level,
        ElementKind::Stair(s) => &mut s.level,
        ElementKind::Railing(r) => &mut r.level,
        ElementKind::Footing(f) => &mut f.level,
        ElementKind::Part(p) => &mut p.level,
        ElementKind::Site(s) => &mut s.level,
        _ => return None,
    })
}

/// What a member is made of.
pub fn material_mut(kind: &mut ElementKind) -> Option<&mut Option<ElementId>> {
    Some(match kind {
        ElementKind::Column(c) => &mut c.material,
        ElementKind::Beam(b) => &mut b.material,
        ElementKind::Roof(r) => &mut r.material,
        ElementKind::Stair(s) => &mut s.material,
        ElementKind::Railing(r) => &mut r.material,
        ElementKind::Footing(f) => &mut f.material,
        ElementKind::Part(p) => &mut p.material,
        ElementKind::Site(s) => &mut s.material,
        _ => return None,
    })
}

pub fn material(kind: &ElementKind) -> Option<ElementId> {
    match kind {
        ElementKind::Column(c) => c.material,
        ElementKind::Beam(b) => b.material,
        ElementKind::Roof(r) => r.material,
        ElementKind::Stair(s) => s.material,
        ElementKind::Railing(r) => r.material,
        ElementKind::Footing(f) => f.material,
        ElementKind::Part(p) => p.material,
        ElementKind::Site(s) => s.material,
        _ => None,
    }
}

/// Is this one of the elements in this module?
pub fn is_member(kind: &ElementKind) -> bool {
    level_of(kind).is_some()
}

/// Move, turn or scale a member. `None` when it can't take `t`.
pub fn transform(model: &Model, kind: &mut ElementKind, t: &Xform) -> Option<()> {
    // a part can tilt: it then stands on a plane of its own
    if !t.keeps_upright() && !matches!(kind, ElementKind::Part(_)) {
        return None;
    }
    let dz = match t {
        Xform::Move(d) => d.z,
        _ => 0.0,
    };
    let turn = match t {
        Xform::Rotate { degrees, .. } => *degrees,
        _ => 0.0,
    };
    match kind {
        ElementKind::Column(c) => {
            c.position = t.point2(c.position);
            c.rotation = (c.rotation + turn).rem_euclid(360.0);
            c.base_offset += dz;
            match t {
                Xform::Scale { factor, .. } => match &mut c.profile {
                    Profile::Rectangle { width, depth } => {
                        *width *= factor;
                        *depth *= factor;
                    }
                    Profile::Round { diameter } => *diameter *= factor,
                },
                // a mirror in plan: square and round columns look the same
                // turned over, so it only goes to the mirrored place
                Xform::Stretch { factor, .. } => {
                    c.rotation = crate::transform::mirrored_angle(c.rotation, *factor)?;
                }
                _ => {}
            }
        }
        ElementKind::Beam(b) => {
            b.start = t.point2(b.start);
            b.end = t.point2(b.end);
            b.top_offset -= dz;
            if matches!(t, Xform::Stretch { .. }) && b.length() < 1e-6 {
                return None;
            }
        }
        ElementKind::Roof(r) => {
            for p in r.boundary.iter_mut().chain(r.holes.iter_mut().flatten()) {
                *p = t.point2(*p);
            }
            r.base_offset += dz;
            if crate::polygon::signed_area2(&r.boundary) < 0.0 {
                // a mirror: run the outline the other way, keeping its eave
                let n = r.boundary.len();
                r.boundary.reverse();
                r.edge = (2 * n - 2 - r.edge % n) % n;
            }
        }
        ElementKind::Stair(s) => {
            let d = s.dir();
            s.start = t.point2(s.start);
            let nd = t.point2(d) - t.point2(DVec2::ZERO);
            s.direction = nd.to_angle().to_degrees().rem_euclid(360.0);
            s.base_offset += dz;
            if let (Xform::Stretch { factor, .. }, Some(l)) = (t, s.landing.as_mut()) {
                if factor.x * factor.y < 0.0 {
                    l.left = !l.left;
                }
            }
        }
        ElementKind::Footing(f) => {
            f.below -= dz;
            // one under a wall or a column goes where its host goes
            if f.host.is_some_and(|h| model.get(h).is_some()) {
                return Some(());
            }
            f.start = t.point2(f.start);
            f.end = t.point2(f.end);
            f.rotation = (f.rotation + turn).rem_euclid(360.0);
            if f.kind == crate::FootingKind::Strip && f.start.distance(f.end) < 1e-6 {
                return None;
            }
        }
        ElementKind::Railing(r) => {
            if r.stair.is_some() {
                // a railing on a stair goes where the stair goes
                return if matches!(t, Xform::Move(_)) {
                    Some(())
                } else {
                    None
                };
            }
            let z = elevation(model, r.level);
            for p in &mut r.path {
                *p = t.point3(*p + DVec3::Z * z) - DVec3::Z * z;
            }
        }
        ElementKind::Part(p) => {
            let lift = elevation(model, p.level);
            crate::parts::transform(p, t, lift)?
        }
        ElementKind::Site(s) => crate::site::transform(s, t)?,
        _ => return None,
    }
    Some(())
}

/// Where a member is, for turning and scaling it about itself.
pub fn pivot(model: &Model, e: &Element) -> Option<DVec3> {
    Some(match &e.kind {
        ElementKind::Column(c) => c.position.extend(c.span(model).0),
        ElementKind::Beam(b) => ((b.start + b.end) / 2.0).extend(b.top(model)),
        ElementKind::Roof(r) => {
            let n = r.boundary.len().max(1) as f64;
            (r.boundary.iter().copied().sum::<DVec2>() / n).extend(r.base(model))
        }
        ElementKind::Stair(s) => s.start.extend(s.base(model)),
        ElementKind::Railing(r) => {
            let p = r.path_in(model);
            let n = p.len().max(1) as f64;
            p.iter().copied().sum::<DVec3>() / n
        }
        ElementKind::Footing(f) => f.anchor(model),
        ElementKind::Part(p) => p.pivot(model)?,
        ElementKind::Site(s) => s.pivot(model)?,
        _ => return None,
    })
}

/// How a grip on a member moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GripMove {
    /// Anywhere on the horizontal plane through it.
    Flat,
    /// Up and down.
    Vertical,
    /// Along a direction in plan.
    Along(DVec2),
}

/// A drag handle on a member.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grip {
    pub at: DVec3,
    pub how: GripMove,
}

/// The grips of a member: ends, corners, heights.
pub fn grips(model: &Model, e: &Element) -> Vec<Grip> {
    let flat = |at| Grip {
        at,
        how: GripMove::Flat,
    };
    let up = |at| Grip {
        at,
        how: GripMove::Vertical,
    };
    match &e.kind {
        ElementKind::Column(c) => {
            let (z0, z1) = c.span(model);
            // the base grip moves it across the plan; the two upright
            // ones, best dragged in a section, set its top and its base
            vec![
                flat(c.position.extend(z0)),
                up(c.position.extend(z1)),
                up(c.position.extend(z0)),
            ]
        }
        ElementKind::Beam(b) => {
            let z = b.top(model);
            vec![
                flat(b.start.extend(z)),
                flat(b.end.extend(z)),
                up(((b.start + b.end) / 2.0).extend(z - b.depth)),
            ]
        }
        ElementKind::Roof(r) => {
            let z = r.base(model);
            let mut out: Vec<Grip> = r.boundary.iter().map(|p| flat(p.extend(z))).collect();
            let c = r.boundary.iter().copied().sum::<DVec2>() / r.boundary.len().max(1) as f64;
            out.push(up(c.extend(z + r.peak())));
            out
        }
        ElementKind::Stair(s) => {
            let rise = s.rise(model);
            let lay = s.layout(rise);
            let z = s.base(model);
            let f = lay.flights[0].clone();
            let mut out = vec![flat(s.start.extend(z))];
            // the far side of the first flight sets its width
            out.push(Grip {
                at: (f.start + f.dir.perp() * s.width / 2.0).extend(z),
                how: GripMove::Along(f.dir.perp()),
            });
            // the end of the last flight sets the tread depth
            let last = lay.flights.last().cloned().unwrap_or_else(|| f.clone());
            out.push(Grip {
                at: last
                    .end(s.tread)
                    .extend(z + last.z0 + last.risers as f64 * lay.riser),
                how: GripMove::Along(last.end_dir(s.tread)),
            });
            out
        }
        ElementKind::Railing(r) if r.stair.is_none() => {
            let z = elevation(model, r.level);
            let mut out: Vec<Grip> = r.path.iter().map(|p| flat(*p + DVec3::Z * z)).collect();
            if let Some(p) = r.path.first() {
                out.push(up(*p + DVec3::Z * (z + r.height)));
            }
            out
        }
        ElementKind::Railing(r) => {
            let p = r.path_in(model);
            p.first()
                .map(|a| vec![up(*a + DVec3::Z * r.height)])
                .unwrap_or_default()
        }
        ElementKind::Footing(f) => footing_grips(model, f)
            .into_iter()
            .map(|(g, _)| g)
            .collect(),
        _ => vec![],
    }
}

/// What a footing's grip changes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum FootingGrip {
    Start,
    End,
    Center,
    /// The side square to a strip, or a pad's side along its own x axis.
    Width,
    /// A pad's side along its own y axis.
    Length,
    /// The top: how far below its level it sits.
    Top,
    /// The underside: its thickness.
    Underside,
}

/// A footing's grips: a free one's ends or centre, its sides, its top and
/// its underside. One under a host has no grips to move it: it goes where
/// the host goes.
fn footing_grips(model: &Model, f: &crate::Footing) -> Vec<(Grip, FootingGrip)> {
    let hosted = f.host.is_some_and(|h| model.get(h).is_some());
    let top = f.top(model);
    let flat = |at: DVec3| Grip {
        at,
        how: GripMove::Flat,
    };
    let along = |at: DVec3, d: DVec2| Grip {
        at,
        how: GripMove::Along(d),
    };
    let up = |at: DVec3| Grip {
        at,
        how: GripMove::Vertical,
    };
    let mut out = vec![];
    let c = f.anchor(model).truncate();
    let mid = top - f.thickness / 2.0;
    match f.kind {
        crate::FootingKind::Strip => {
            let (a, b) = f.line(model);
            if !hosted {
                out.push((flat(a.extend(top)), FootingGrip::Start));
                out.push((flat(b.extend(top)), FootingGrip::End));
            }
            let n = (b - a).normalize_or_zero().perp();
            // a side's bar at the middle of the side, in sections too (A4-8)
            out.push((
                along((c + n * f.width / 2.0).extend(mid), n),
                FootingGrip::Width,
            ));
        }
        crate::FootingKind::Pad => {
            let (_, turn) = f.place(model);
            let x = DVec2::from_angle(turn.to_radians());
            if !hosted {
                out.push((flat(c.extend(top)), FootingGrip::Center));
            }
            out.push((
                along((c + x * f.width / 2.0).extend(mid), x),
                FootingGrip::Width,
            ));
            out.push((
                along((c + x.perp() * f.length / 2.0).extend(mid), x.perp()),
                FootingGrip::Length,
            ));
        }
    }
    // the top and underside in the middle, best dragged in a section
    out.push((up(c.extend(top)), FootingGrip::Top));
    out.push((up(c.extend(top - f.thickness)), FootingGrip::Underside));
    out
}

fn drag_footing(
    model: &Model,
    f: &mut crate::Footing,
    i: usize,
    d: DVec3,
    now: DVec3,
    step: f64,
) -> Result<(), String> {
    let round = |v: f64, s: f64| if s > 0.0 { (v / s).round() * s } else { v };
    let grips = footing_grips(model, f);
    let (g, what) = *grips.get(i).ok_or("no such handle")?;
    let fine = step / 5.0;
    match what {
        FootingGrip::Start => f.start = now.truncate(),
        FootingGrip::End => f.end = now.truncate(),
        FootingGrip::Center => {
            f.start = now.truncate();
            f.end = f.start;
        }
        FootingGrip::Width | FootingGrip::Length => {
            let GripMove::Along(dir) = g.how else {
                return Ok(());
            };
            // both sides move: the footing stays centred on its line
            let by = 2.0 * d.truncate().dot(dir);
            if what == FootingGrip::Width {
                f.width = round(f.width + by, fine).max(0.1);
            } else {
                f.length = round(f.length + by, fine).max(0.1);
            }
        }
        FootingGrip::Top => {
            // the underside stays where it is
            let dz = round(d.z, fine).min(f.thickness - 0.05);
            f.below -= dz;
            f.thickness -= dz;
        }
        FootingGrip::Underside => {
            f.thickness = round(f.thickness - d.z, fine).max(0.05);
        }
    }
    if f.kind == crate::FootingKind::Strip && f.host.is_none() && f.start.distance(f.end) < 0.05 {
        return Err("a footing needs some length".into());
    }
    Ok(())
}

/// Grip `i` of `e` dragged from `grab` to `now`; lengths round to `step`.
pub fn drag_grip(
    model: &Model,
    e: &mut Element,
    i: usize,
    grab: DVec3,
    now: DVec3,
    step: f64,
) -> Result<(), String> {
    let round = |v: f64, s: f64| if s > 0.0 { (v / s).round() * s } else { v };
    let d = now - grab;
    let grips = grips(model, e);
    let g = *grips.get(i).ok_or("no such handle")?;
    match &mut e.kind {
        ElementKind::Column(c) => match i {
            0 => c.position = now.truncate(),
            1 => {
                let h = c.height_in(model);
                let new = round(h + d.z, step).max(0.1);
                match &mut c.top {
                    // tied to a level: the drag sets how far above it the top is
                    crate::WallTop::UpToLevel { offset, .. } => *offset += new - h,
                    crate::WallTop::Unconnected => c.height = Some(new),
                }
            }
            _ => {
                // the base moves, the top stays where it is
                let (z0, z1) = c.span(model);
                let dz = round(d.z, step).min(z1 - z0 - 0.1);
                c.base_offset += dz;
                if c.top.is_unconnected() && c.height.is_some() {
                    c.height = Some((z1 - z0 - dz).max(0.1));
                }
            }
        },
        ElementKind::Beam(b) => match i {
            0 => b.start = now.truncate(),
            1 => b.end = now.truncate(),
            _ => b.depth = round(b.depth - d.z, step / 5.0).max(0.05),
        },
        ElementKind::Roof(r) => {
            let n = r.boundary.len();
            if i < n {
                r.boundary[i] = now.truncate();
            } else {
                // the peak sets the pitch
                let peak = g.at.z - r.base(model) - r.vertical_thickness();
                let want = (peak + d.z).max(0.05);
                if peak > 1e-6 && r.effective_shape() != RoofShape::Flat {
                    let slope = r.pitch.to_radians().tan() * want / peak;
                    r.pitch = round(slope.atan().to_degrees(), 0.5).clamp(1.0, 75.0);
                } else {
                    r.thickness = round(r.thickness + d.z, step / 5.0).max(0.02);
                }
            }
        }
        ElementKind::Stair(s) => match i {
            0 => s.start = now.truncate(),
            1 => {
                let GripMove::Along(dir) = g.how else {
                    return Ok(());
                };
                let w = s.width + d.truncate().dot(dir);
                s.width = round(w, step / 5.0).max(0.5);
            }
            _ => {
                let rise = s.rise(model);
                let lay = s.layout(rise);
                let last = lay.flights.last().cloned().ok_or("no flight")?;
                let treads = last.risers.saturating_sub(1).max(1) as f64;
                let GripMove::Along(dir) = g.how else {
                    return Ok(());
                };
                let t = s.tread + d.truncate().dot(dir) / treads;
                s.tread = round(t, 0.005).clamp(0.15, 0.6);
            }
        },
        ElementKind::Footing(f) => drag_footing(model, f, i, d, now, step)?,
        ElementKind::Railing(r) => {
            if r.stair.is_none() && i < r.path.len() {
                let z = r.path[i].z;
                r.path[i] = now.truncate().extend(z);
            } else {
                r.height = round(r.height + d.z, step / 5.0).max(0.2);
            }
        }
        _ => return Err("this handle doesn't fit this element".into()),
    }
    Ok(())
}

/// A short description of a member's size, for the status bar.
pub fn readout(model: &Model, e: &Element) -> Option<String> {
    Some(match &e.kind {
        ElementKind::Column(c) => format!(
            "{} mm, height {:.2} m",
            c.profile.label(),
            c.height_in(model)
        ),
        ElementKind::Beam(b) => format!(
            "length {:.2} m, {:.0} × {:.0} mm",
            b.length(),
            b.width * 1000.0,
            b.depth * 1000.0
        ),
        ElementKind::Roof(r) => format!(
            "{} roof, pitch {:.1}°, {:.2} m²",
            r.effective_shape().label(),
            r.pitch,
            r.plan_area()
        ),
        ElementKind::Stair(s) => {
            let rise = s.rise(model);
            let lay = s.layout(rise);
            format!(
                "{} risers of {:.0} mm, treads {:.0} mm, width {:.2} m",
                lay.risers,
                lay.riser * 1000.0,
                s.tread * 1000.0,
                s.width
            )
        }
        ElementKind::Railing(r) => {
            format!("length {:.2} m, height {:.2} m", r.length(model), r.height)
        }
        ElementKind::Footing(f) => match f.kind {
            crate::FootingKind::Strip => format!(
                "strip {:.2} m long, {:.0} wide, {:.0} thick",
                f.strip_length(model),
                f.width * 1000.0,
                f.thickness * 1000.0
            ),
            crate::FootingKind::Pad => format!(
                "pad {:.0} × {:.0}, {:.0} thick",
                f.width * 1000.0,
                f.length * 1000.0,
                f.thickness * 1000.0
            ),
        },
        _ => return None,
    })
}

/// Is the polygon convex (either winding)?
pub fn is_convex(poly: &[DVec2]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut sign = 0.0;
    for i in 0..n {
        let (a, b, c) = (poly[i], poly[(i + 1) % n], poly[(i + 2) % n]);
        let z = (b - a).perp_dot(c - b);
        if z.abs() < 1e-12 {
            continue;
        }
        if sign == 0.0 {
            sign = z.signum();
        } else if z.signum() != sign {
            return false;
        }
    }
    sign != 0.0
}

/// A counter-clockwise polygon grown outwards by `by` (mitred corners).
/// `poly` with edge `i` (corner `i` to `i + 1`) moved out square to
/// itself by `by[i]` (negative: in); each corner where its two edges'
/// new lines cross, or slid along when they are parallel.
pub fn offset_edges(poly: &[DVec2], by: &[f64]) -> Vec<DVec2> {
    let n = poly.len();
    if n < 3 || by.iter().all(|b| b.abs() < 1e-12) {
        return poly.to_vec();
    }
    let ccw = crate::polygon::signed_area2(poly) > 0.0;
    let dir = |i: usize| (poly[(i + 1) % n] - poly[i]).normalize_or_zero();
    let out_n = |i: usize| if ccw { -dir(i).perp() } else { dir(i).perp() };
    let off = |i: usize| by.get(i).copied().unwrap_or(0.0);
    (0..n)
        .map(|i| {
            let prev = (i + n - 1) % n;
            // the two moved lines: p0 + t d0 and p1 + s d1
            let (d0, d1) = (dir(prev), dir(i));
            let p0 = poly[i] + out_n(prev) * off(prev);
            let p1 = poly[i] + out_n(i) * off(i);
            let den = d0.perp_dot(d1);
            if den.abs() < 1e-9 {
                p1
            } else {
                p0 + d0 * ((p1 - p0).perp_dot(d1) / den)
            }
        })
        .collect()
}

pub fn offset_polygon(poly: &[DVec2], by: f64) -> Vec<DVec2> {
    let n = poly.len();
    if n < 3 || by.abs() < 1e-12 {
        return poly.to_vec();
    }
    let ccw = crate::polygon::signed_area2(poly) > 0.0;
    let out_n = |i: usize| {
        let d = (poly[(i + 1) % n] - poly[i]).normalize_or_zero();
        if ccw {
            -d.perp()
        } else {
            d.perp()
        }
    };
    (0..n)
        .map(|i| {
            let prev = (i + n - 1) % n;
            let (n0, n1) = (out_n(prev), out_n(i));
            let bis = n0 + n1;
            let cos = n0.dot(bis.normalize_or_zero());
            if bis.length() < 1e-9 || cos.abs() < 0.2 {
                poly[i] + n1 * by
            } else {
                poly[i] + bis.normalize() * (by / cos)
            }
        })
        .collect()
}

/// The part of convex polygon `poly` where `n · p <= c`.
pub fn clip_half_plane(poly: &[DVec2], n: DVec2, c: f64) -> Vec<DVec2> {
    let mut out = Vec::new();
    let len = poly.len();
    let side = |p: DVec2| n.dot(p) - c;
    for i in 0..len {
        let (a, b) = (poly[i], poly[(i + 1) % len]);
        let (sa, sb) = (side(a), side(b));
        if sa <= 0.0 {
            out.push(a);
        }
        if (sa < 0.0 && sb > 0.0) || (sa > 0.0 && sb < 0.0) {
            out.push(a + (b - a) * (sa / (sa - sb)));
        }
    }
    out.dedup_by(|a, b| a.distance(*b) < 1e-9);
    if out.len() > 1 && out[0].distance(out[out.len() - 1]) < 1e-9 {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Level, Transaction};

    #[test]
    fn a_tube_is_a_round_rail() {
        let tube = crate::profiles::preset("Tube").unwrap();
        let g = hand_grip(Some(&tube));
        assert!(g.round, "{g:?}");
        assert!((g.across - 0.048).abs() < 0.001, "{g:?}");
    }

    #[test]
    fn a_hand_holds_a_slim_round_rail_not_a_square_bar() {
        let circle = |d: f64| -> Vec<DVec2> {
            (0..24)
                .map(|k| DVec2::from_angle(k as f64 * std::f64::consts::TAU / 24.0) * (d / 2.0))
                .collect()
        };
        let g = hand_grip(Some(&circle(0.038)));
        assert!(g.round && g.ok, "{g:?}");
        assert!(!hand_grip(Some(&circle(0.05))).ok);
        // the 50 mm square bar: 200 mm round
        let g = hand_grip(None);
        assert!(!g.round && !g.ok && (g.perimeter - 0.2).abs() < 1e-9);
        // a 30 by 25 rail: 110 mm round, 39 mm across
        let rect = [
            DVec2::ZERO,
            DVec2::new(0.03, 0.0),
            DVec2::new(0.03, 0.025),
            DVec2::new(0.0, 0.025),
        ];
        assert!(hand_grip(Some(&rect)).ok);
        // the library's Handrail preset can be held
        let h = crate::profiles::preset("Handrail").unwrap();
        assert!(hand_grip(Some(&h)).ok, "{:?}", hand_grip(Some(&h)));
    }

    fn two_levels() -> (Model, ElementId, ElementId) {
        let mut m = Model::new();
        let a = Element::new(
            "Ground",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let b = Element::new(
            "First",
            ElementKind::Level(Level {
                elevation: 2.88,
                height: 3.0,
            }),
        );
        let (ia, ib) = (a.id, b.id);
        Transaction::new("levels")
            .insert(a)
            .insert(b)
            .apply(&mut m)
            .unwrap();
        (m, ia, ib)
    }

    fn rect(w: f64, d: f64) -> Vec<DVec2> {
        vec![
            DVec2::ZERO,
            DVec2::new(w, 0.0),
            DVec2::new(w, d),
            DVec2::new(0.0, d),
        ]
    }

    #[test]
    fn offset_edges_moves_only_the_edges_asked() {
        let sq = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(4.0, 0.0),
            DVec2::new(4.0, 3.0),
            DVec2::new(0.0, 3.0),
        ];
        // the bottom edge out 0.1, the right one in 0.2
        let p = offset_edges(&sq, &[0.1, -0.2, 0.0, 0.0]);
        assert!(p[0].distance(DVec2::new(0.0, -0.1)) < 1e-9, "{p:?}");
        assert!(p[1].distance(DVec2::new(3.8, -0.1)) < 1e-9, "{p:?}");
        assert!(p[2].distance(DVec2::new(3.8, 3.0)) < 1e-9, "{p:?}");
        assert_eq!(p[3], sq[3]);
        // clockwise, out is still out
        let cw: Vec<DVec2> = sq.iter().rev().copied().collect();
        let q = offset_edges(&cw, &[0.0, 0.0, 0.0, 0.1]);
        assert!(crate::polygon::signed_area2(&q).abs() > crate::polygon::signed_area2(&cw).abs());
    }

    #[test]
    fn a_roof_layer_pushed_out_at_one_eave_reaches_further() {
        let mut r = Roof {
            layer_edges: vec![],
            level: ElementId::new(),
            boundary: vec![
                DVec2::new(0.0, 0.0),
                DVec2::new(6.0, 0.0),
                DVec2::new(6.0, 4.0),
                DVec2::new(0.0, 4.0),
            ],
            shape: RoofShape::Hip,
            pitch: 30.0,
            overhang: 0.4,
            thickness: 0.3,
            base_offset: 0.0,
            edge: 0,
            material: None,
            assembly: None,
            holes: vec![],
        };
        crate::set_layer_edge(&mut r.layer_edges, 0, 0, 0.1);
        let ys = |faces: &[RoofFace]| {
            faces
                .iter()
                .flat_map(|f| f.outline.iter().map(|p| p.y))
                .fold(f64::INFINITY, f64::min)
        };
        assert!((ys(&r.faces()) + 0.4).abs() < 1e-9);
        assert!((ys(&r.layer_faces(0)) + 0.5).abs() < 1e-9);
        // other layers keep the roof's eaves
        assert!((ys(&r.layer_faces(1)) + 0.4).abs() < 1e-9);
    }

    #[test]
    fn risers_never_exceed_the_maximum() {
        assert_eq!(riser_count(2.88, 0.18), 16);
        assert_eq!(riser_count(3.0, 0.18), 17);
        assert_eq!(riser_count(0.18, 0.18), 1);
        let (m, ground, _) = two_levels();
        let s = Stair::new(ground, DVec2::ZERO, 0.0);
        let rise = s.rise(&m);
        assert!((rise - 2.88).abs() < 1e-9, "climbs to the next level");
        let lay = s.layout(rise);
        assert_eq!(lay.risers, 16);
        assert!((lay.riser - 0.18).abs() < 1e-9);
        // 16 risers, 15 treads
        assert!((lay.flights[0].run(s.tread) - 15.0 * 0.28).abs() < 1e-9);
    }

    /// The stairs of the proposal: 2.9 between floors, 17 risers of
    /// 170.6, goings of 280, a metre wide.
    fn drawn(pts: &[(f64, f64)], drawn: StairLine) -> (Stair, StairLayout) {
        let (_, ground, _) = two_levels();
        let pts: Vec<DVec2> = pts.iter().map(|&(x, y)| DVec2::new(x, y)).collect();
        let s = Stair::from_line(ground, &pts, drawn).unwrap();
        let lay = s.layout(2.9);
        (s, lay)
    }

    fn treads(lay: &StairLayout) -> u32 {
        lay.flights.iter().map(|f| f.risers - 1).sum::<u32>() + lay.landings.len() as u32
    }

    #[test]
    fn a_line_with_a_corner_gets_a_landing_there() {
        let (_, lay) = drawn(&[(1.2, 4.6), (4.51, 4.6), (4.51, 2.69)], StairLine::Walk);
        assert_eq!(lay.risers, 17);
        assert_eq!(lay.short, 0);
        assert_eq!(lay.flights.len(), 2);
        assert_eq!(lay.landings.len(), 1);
        assert_eq!(treads(&lay), 16, "every level but the top one");
        assert_eq!(lay.flights[0].risers, 11);
        assert_eq!(lay.flights[1].risers, 6);
        // the landing is a metre square, at the 11th riser
        let (l, z) = &lay.landings[0];
        assert!((crate::polygon::signed_area2(l) / 2.0 - 1.0).abs() < 0.05);
        assert!((z - 11.0 * 2.9 / 17.0).abs() < 1e-9);
        // the second flight leaves the landing's far side, going down y
        let f2 = lay.flights[1].clone();
        assert!((f2.dir + DVec2::Y).length() < 1e-9);
        assert!((f2.start - DVec2::new(4.51, 4.1)).length() < 1e-9);
        // the walk line climbs from the first nosing to the floor above
        assert!((lay.walk[0].z - 2.9 / 17.0).abs() < 1e-9);
        assert!((lay.walk.last().unwrap().z - 2.9).abs() < 1e-9);
    }

    #[test]
    fn a_flight_keeps_its_own_treads_and_a_landing_its_depth() {
        let pts = [(1.2, 4.6), (4.51, 4.6), (4.51, 1.0)];
        let (mut s, lay) = drawn(&pts, StairLine::Walk);
        assert_eq!(
            lay.flights.iter().map(|f| f.risers).collect::<Vec<_>>(),
            [11, 6]
        );
        assert_eq!(lay.turns, [0]);
        assert_eq!(lay.flights[1].straight, 1);
        // seven treads on the first flight: the rest go up the second,
        // the landing takes what the first flight leaves
        s.pieces.set_treads(0, Some(7));
        let lay = s.layout(2.9);
        assert_eq!(
            lay.flights.iter().map(|f| f.risers).collect::<Vec<_>>(),
            [8, 9]
        );
        assert_eq!(treads(&lay), 16);
        let area = crate::polygon::signed_area2(&lay.landings[0].0) / 2.0;
        assert!((area - (1.0 + 2.81 - 7.0 * 0.28)).abs() < 0.01, "{area}");
        // more treads than fit: as many as fit
        s.pieces.set_treads(0, Some(40));
        assert_eq!(s.layout(2.9).flights[0].risers, 11);
        s.pieces.set_treads(0, None);
        assert!(s.pieces.is_empty());
        // a landing 0.6 deeper: the first flight stops sooner
        s.pieces.set_deeper(0, 0.6);
        let lay = s.layout(2.9);
        assert_eq!(lay.flights[0].risers, 8);
        assert_eq!(lay.flights[1].risers, 9);
        // the stair re-flows when its line changes, the changes stay
        s.pieces.set_treads(0, Some(5));
        // a turn put into the first straight: the flight's change stays
        // with its first part, the landing's with its turn
        s.pieces.point_added(1);
        assert_eq!(s.pieces.flights[0].straight, 0);
        assert_eq!(s.pieces.landings[0].turn, 1);
        s.pieces.point_removed(1, 4);
        assert_eq!(s.pieces.flights[0].straight, 0);
        assert_eq!(s.pieces.landings[0].turn, 0);
        // taking the turn away takes its landing's change with it
        s.pieces.point_removed(1, 3);
        assert!(s.pieces.landings.is_empty());
        assert_eq!(s.pieces.flights.len(), 1);
    }

    #[test]
    fn a_turn_on_winders_climbs_round_the_corner() {
        let pts = [(1.2, 4.6), (4.51, 4.6), (4.51, 1.0)];
        let (mut s, landing) = drawn(&pts, StairLine::Walk);
        s.at_turns = TurnKind::Winders(0);
        let lay = s.layout(2.9);
        // three winders of 30° each instead of the landing: three more
        // treads than the landing's one, so two fewer in the flights
        assert_eq!(lay.winders.len(), 3);
        assert!(lay.landings.is_empty());
        assert_eq!(lay.treads(), 16);
        assert_eq!(lay.short, 0);
        assert_eq!(
            lay.flights.iter().map(|f| f.risers).collect::<Vec<_>>(),
            [11, 4]
        );
        // together they cover the landing, fanning from its inner corner
        let area: f64 = lay
            .winders
            .iter()
            .map(|w| crate::polygon::signed_area2(&w.outline) / 2.0)
            .sum();
        let was = crate::polygon::signed_area2(&landing.landings[0].0) / 2.0;
        assert!((area - was).abs() < 1e-6, "{area} {was}");
        assert!(lay.winders.iter().all(|w| w.outline.len() >= 3));
        // each a riser higher, on from the first flight's top
        let r = lay.riser;
        for (q, w) in lay.winders.iter().enumerate() {
            assert!((w.z - (11.0 + q as f64) * r).abs() < 1e-9);
        }
        // the walk line climbs all the way round, never doubling back
        assert!(lay.walk.windows(2).all(|w| w[1].z >= w[0].z - 1e-9));
        assert!(lay
            .walk
            .windows(2)
            .all(|w| w[1].x >= w[0].x - 1e-9 && w[1].y <= w[0].y + 1e-9));
        assert!((lay.walk.last().unwrap().z - 2.9).abs() < 1e-9);
        // both handrails climb round the turn the same way, the inner one
        // too, and their posts stay apart round its narrow end
        for left in [false, true] {
            let rail = s.side_path(2.9, left, RAIL_SIZE);
            assert!(
                rail.windows(2).all(|w| w[1].z >= w[0].z - 1e-9
                    && w[1].x >= w[0].x - 1e-9
                    && w[1].y <= w[0].y + 1e-9),
                "{left} {rail:?}"
            );
            let posts = posts_along(&rail, 1.2);
            assert!(
                posts
                    .windows(2)
                    .all(|w| w[0].truncate().distance(w[1].truncate()) > 0.3),
                "{left} {posts:?}"
            );
        }
        assert_eq!(s.ifc_type(2.9), "QUARTER_WINDING_STAIR");
        // one turn back to a landing, on its own
        s.pieces.set_turn_kind(0, Some(TurnKind::Landing));
        assert_eq!(s.layout(2.9).landings.len(), 1);
        // four winders asked for
        s.pieces.set_turn_kind(0, Some(TurnKind::Winders(4)));
        let lay = s.layout(2.9);
        assert_eq!(lay.winders.len(), 4);
        assert_eq!(lay.treads(), 16);
    }

    #[test]
    fn a_bent_line_makes_one_curved_flight() {
        // a quarter circle of radius 3 on the walk line, then straight on
        let r = 3.0;
        let pts = [(0.0, 0.0), (r, r), (r, 6.0)];
        let (mut s, _) = drawn(&pts, StairLine::Walk);
        s.set_bend(0, 90.0);
        let lay = s.layout(2.9);
        // no turn: one flight round the curve and on up the straight
        assert_eq!(lay.flights.len(), 1);
        assert!(lay.landings.is_empty());
        let f = &lay.flights[0];
        assert!(f.curved());
        assert_eq!(f.risers, 17);
        // every riser square to the walk line: through the centre on the arc
        let c = DVec2::new(0.0, r);
        for i in 0..6 {
            let (a, b) = f.riser(i, s.tread, s.width / 2.0);
            let mid = (a + b) / 2.0;
            assert!((mid.distance(c) - r).abs() < 0.01, "{i}");
            assert!(
                (b - a).normalize().perp_dot((mid - c).normalize()).abs() < 0.05,
                "{i}"
            );
        }
        // treads keep the going on the walk line
        assert!((f.end(s.tread).distance(f.at(16.0 * s.tread).0)).abs() < 1e-9);
        assert_eq!(s.ifc_type(2.9), "CURVED_RUN_STAIR");
        // the walk line climbs all the way
        assert!(lay.walk.windows(2).all(|w| w[1].z >= w[0].z - 1e-9));
        // both railings follow it without doubling back
        for left in [false, true] {
            let rail = s.side_path(2.9, left, RAIL_SIZE);
            assert!(rail.windows(2).all(|w| w[1].z >= w[0].z - 1e-9));
            // posts evenly round the curve, not at each bend of it
            let posts = posts_along(&rail, 1.2);
            assert!(posts
                .windows(2)
                .all(|w| w[0].truncate().distance(w[1].truncate()) > 0.6));
        }
    }

    #[test]
    fn a_spiral_goes_round_in_arcs_and_trims_on_its_arc() {
        // two half circles of radius 1.2 on the walk line: a full turn
        let pts = [(0.0, 0.0), (0.0, 2.4), (0.0001, 0.0)];
        let (mut s, _) = drawn(&pts, StairLine::Walk);
        s.line = vec![DVec2::ZERO, DVec2::new(0.0, 2.4), DVec2::new(0.0, 0.0)];
        s.line[2].x = 1e-4;
        s.set_bend(0, -180.0);
        s.set_bend(1, -180.0);
        s.width = 1.4;
        s.tread = 0.24;
        let lay = s.layout(2.9);
        assert_eq!(lay.flights.len(), 1);
        assert_eq!(s.ifc_type(2.9), "SPIRAL_STAIR");
        // round a 1.2 m walk radius: 7.54 m of line, 16 goings use 3.84 m
        assert!(lay.spare > 3.0, "{}", lay.spare);
        s.trim(2.9);
        let lay = s.layout(2.9);
        assert!(lay.spare < 0.02, "{}", lay.spare);
        assert_eq!(s.line.len(), 3);
        // the second arc was cut short round the same centre
        let c = DVec2::new(0.0, 1.2);
        assert!((s.line[2].distance(c) - 1.2).abs() < 1e-3);
        assert!(s.bend(1) > -180.0 && s.bend(1) < 0.0);
    }

    #[test]
    fn bends_follow_points_put_in_and_taken_out() {
        let (mut s, _) = drawn(&[(0.0, 0.0), (3.0, 0.0), (3.0, 3.0)], StairLine::Walk);
        s.set_bend(1, 40.0);
        s.bend_split(1);
        assert_eq!(s.bends, [0.0, 20.0, 20.0]);
        s.bend_joined(2);
        assert_eq!(s.bends, [0.0, 40.0]);
        s.set_bend(1, 0.0);
        assert!(s.bends.is_empty() && !s.curved());
    }

    #[test]
    fn a_half_turn_on_winders_turns_twice() {
        // a dog-leg drawn as a U: the flights side by side
        let (mut s, _) = drawn(
            &[(0.0, 0.0), (3.0, 0.0), (3.0, 1.0), (0.0, 1.0)],
            StairLine::Walk,
        );
        s.at_turns = TurnKind::Winders(0);
        let lay = s.layout(2.9);
        // three at each corner: the 260 mm the line leaves over before
        // the first is less than a going, so it deepens the first winder
        // instead of making a shallow tread (MU103)
        let at = |t: usize| lay.winders.iter().filter(|w| w.turn == t).count();
        assert_eq!((at(0), at(1)), (3, 3));
        assert!(lay.landings.is_empty());
        assert_eq!(lay.treads() + lay.short, 16);
        assert_eq!(s.ifc_type(2.9), "HALF_WINDING_STAIR");
    }

    #[test]
    fn two_corners_close_together_share_one_landing() {
        // a dog-leg: the flights side by side, a metre apart
        let (_, lay) = drawn(
            &[(1.2, 2.0), (3.95, 2.0), (3.95, 3.0), (1.48, 3.0)],
            StairLine::Walk,
        );
        assert_eq!(lay.short, 0);
        assert_eq!(lay.landings.len(), 1, "one half landing");
        assert_eq!(lay.flights.len(), 2);
        assert_eq!(treads(&lay), 16);
        let (l, _) = &lay.landings[0];
        // two flights wide, one deep
        let area = crate::polygon::signed_area2(l) / 2.0;
        assert!((area - 2.0).abs() < 0.05, "{area}");
        // a half metre well: too little between the turns for a flight
        let (_, lay) = drawn(
            &[(0.0, 0.0), (0.0, 2.75), (1.5, 2.75), (1.5, 0.0)],
            StairLine::Walk,
        );
        assert_eq!((lay.flights.len(), lay.landings.len()), (2, 1));
        // three flights round a well keep two landings
        let (_, lay) = drawn(
            &[(1.6, 1.2), (3.51, 1.2), (3.51, 3.34), (1.6, 3.34)],
            StairLine::Walk,
        );
        assert_eq!((lay.flights.len(), lay.landings.len()), (3, 2));
        assert_eq!(treads(&lay), 16);
    }

    #[test]
    fn a_u_drawn_with_one_click_at_its_turn_is_a_half_turn() {
        // MU101: one click at the turn, the line coming back at ~172°
        for side in [StairLine::Walk, StairLine::Left, StairLine::Right] {
            let (s, lay) = drawn(&[(0.0, 0.0), (3.0, 0.0), (0.3, 0.4)], side);
            assert_eq!(s.line.len(), 4, "{side:?}: the turn became two corners");
            assert_eq!((lay.flights.len(), lay.landings.len()), (2, 1), "{side:?}");
            assert!(lay.winders.is_empty(), "{side:?}: no wedge");
            let (f1, f2) = (&lay.flights[0], &lay.flights[1]);
            // exactly back the way it came: the runs parallel (MU103)
            assert!(
                f2.dir.dot(-DVec2::X) > 1.0 - 1e-9,
                "{side:?}: back the way it came"
            );
            // the second run beside the first, a width apart: no overlap
            let apart = (f2.start - f1.start).perp_dot(f1.dir).abs();
            // (a side line folded back on itself is kept 2 mm apart)
            assert!((apart - 1.0).abs() < 0.003, "{side:?}: {apart}");
            // its first riser level with the first run's last, and the
            // landing a rectangle across both (MU103: its edge ran askew)
            let f1_top = f1.end(s.tread).x;
            assert!((f2.start.x - f1_top).abs() < 1e-6, "{side:?}: risers level");
            // one landing two flights wide, one deep, past the first run
            let (l, _) = &lay.landings[0];
            let area = crate::polygon::signed_area2(l).abs() / 2.0;
            let depth = l.iter().map(|p| p.x).fold(f64::MIN, f64::max) - f1_top;
            // a rectangle: two flights wide, as deep on both sides
            assert!(
                (area - (1.0 + apart) * depth).abs() < 1e-6,
                "{side:?}: {area}"
            );
            let f1_end = f1.start.x + f1.run(s.tread);
            assert!(
                l.iter().all(|p| p.x > f1_end - 1e-6),
                "{side:?}: landing past flight 1"
            );
            // every tread laid out (a short line runs its last flight on)
            assert_eq!(treads(&lay), 16, "{side:?}");
        }
        // a gap between the runs moves the second one further across
        let (_, ground, _) = two_levels();
        let mut s = Stair::new(ground, DVec2::ZERO, 0.0);
        s.set_line_gap(
            &[DVec2::ZERO, DVec2::new(3.0, 0.0), DVec2::new(0.3, 0.4)],
            0.2,
        );
        let lay = s.layout(2.9);
        let apart = (lay.flights[1].start - lay.flights[0].start).y.abs();
        assert!((apart - 1.2).abs() < 0.08, "{apart}");
        // a turn under 135° is left as drawn
        let (s, _) = drawn(&[(0.0, 0.0), (3.0, 0.0), (3.0, 3.0)], StairLine::Right);
        assert_eq!(s.line.len(), 3);
    }

    /// MU103's cross-check: every kind of straight-run stair, at many
    /// odd sizes, as drawn by each of its lines. No two of its pieces
    /// overlap, each flight starts and ends on the edge of the landing it
    /// meets, no winder is a sliver, and every tread is laid out.
    #[test]
    fn every_kind_of_stair_at_odd_sizes_has_no_overlap_gap_or_sliver() {
        let on_edge = |l: &[DVec2], p: DVec2| {
            (0..l.len()).any(|i| {
                let (a, b) = (l[i], l[(i + 1) % l.len()]);
                let d = b - a;
                let t = ((p - a).dot(d) / d.length_squared().max(1e-12)).clamp(0.0, 1.0);
                (a + d * t).distance(p) < 1e-6
            })
        };
        let mut seen = 0;
        for over in [0.0, 0.05, 0.13, 0.2, 0.27] {
            for gap in [0.0, 0.2, 0.475] {
                for skew in [0.0, 0.04, -0.06] {
                    let a = 0.5 + 6.0 * 0.28 + over;
                    let w = 1.0 + gap;
                    let shapes: Vec<(&str, Vec<(f64, f64)>)> = vec![
                        ("straight", vec![(0.0, 0.0), (5.0 + over, skew)]),
                        ("L", vec![(0.0, 0.0), (a, 0.0), (a + skew, 3.5)]),
                        (
                            "U, two clicks",
                            vec![(0.0, 0.0), (a, 0.0), (a, w), (0.0, w + skew)],
                        ),
                        (
                            "U, one click",
                            vec![(0.0, 0.0), (a, 0.0), (0.2, 0.3 + skew)],
                        ),
                        (
                            "round a well",
                            vec![
                                (0.0, 0.0),
                                (a, 0.0),
                                (a, 2.6 + over),
                                (0.3, 2.6 + over + skew),
                            ],
                        ),
                        (
                            "Z",
                            vec![
                                (0.0, 0.0),
                                (a, 0.0),
                                (a, 2.0 + over),
                                (a + 3.0, 2.0 + over + skew),
                            ],
                        ),
                    ];
                    for (name, pts) in &shapes {
                        for side in [StairLine::Walk, StairLine::Left, StairLine::Right] {
                            for turns in [TurnKind::Landing, TurnKind::Winders(0)] {
                                for split in [false, true] {
                                    let (_, ground, _) = two_levels();
                                    let mut pts: Vec<DVec2> =
                                        pts.iter().map(|&(x, y)| DVec2::new(x, y)).collect();
                                    // the U's legs set apart on the line drawn so
                                    // that its runs are the gap apart
                                    if *name == "U, two clicks" {
                                        let apart = w + 2.0 * side.to_walk(1.0);
                                        if apart < 0.01 {
                                            continue;
                                        }
                                        pts[2].y = apart;
                                        pts[3].y = apart + skew;
                                    }
                                    let mut s = Stair::from_line(ground, &pts, side).unwrap();
                                    s.set_line_gap(&pts, gap);
                                    s.at_turns = turns;
                                    s.split = split && s.line.len() >= 3;
                                    let what = format!("{name}, by {side:?}, {turns:?}, split {split}, over {over}, gap {gap}, skew {skew}");
                                    let lay = s.layout(2.9);
                                    seen += 1;
                                    assert_eq!(
                                        lay.treads() as usize + lay.landings.len(),
                                        16,
                                        "{what}: treads"
                                    );
                                    assert!(!s.pieces_overlap(2.9), "{what}: pieces overlap");
                                    // each landing meets the flight before and after it
                                    for (i, (l, _)) in lay.landings.iter().enumerate() {
                                        let t = lay.turns[i];
                                        let before = lay.flights.iter().find(|f| f.straight == t);
                                        if let Some(f) = before {
                                            assert!(
                                                on_edge(l, f.end(s.tread)),
                                                "{what}: flight {t} stops off its landing"
                                            );
                                        }
                                        if let Some(f) = lay.flights.iter().find(|f| f.straight > t)
                                        {
                                            assert!(on_edge(l, f.start), "{what}: the flight after landing {i} starts off it: {:?} vs {l:?}", f.start);
                                        }
                                    }
                                    // no sliver of a winder
                                    for wd in &lay.winders {
                                        let area =
                                            crate::polygon::signed_area2(&wd.outline).abs() / 2.0;
                                        assert!(area > 0.1, "{what}: a winder of {area:.3} m²");
                                    }
                                    // a ramp and back, split and back: the same stair
                                    let was = s.clone();
                                    s.make_ramp(1.0 / 12.0);
                                    s.make_steps();
                                    assert_eq!(s, was, "{what}: ramp and back");
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(seen > 1000);
    }

    #[test]
    fn a_ramp_made_steps_again_is_the_stair_it_was() {
        // MU103: Ramp ticked and unticked
        let (mut s, _) = drawn(&[(0.0, 0.0), (2.0, 0.0), (2.0, 3.0)], StairLine::Walk);
        s.at_turns = TurnKind::Winders(0);
        s.tread = 0.25;
        s.max_riser = 0.17;
        s.build = StairBuild::Stringers;
        let was = s.clone();
        s.make_ramp(1.0 / 12.0);
        s.set_slope(1.0 / 10.0);
        s.make_steps();
        assert_eq!(s, was);
    }

    #[test]
    fn no_tread_before_a_turn_on_winders_is_shallower_than_a_going() {
        // MU103: 7 treads and 170 mm over made an eighth tread 170 mm deep
        for over in [0.0, 0.1, 0.17, 0.27, 0.3, 0.5] {
            let leg = 0.5 + 7.0 * 0.28 + over;
            let (mut s, _) = drawn(&[(0.0, 0.0), (leg, 0.0), (leg, 3.0)], StairLine::Walk);
            s.at_turns = TurnKind::Winders(0);
            let lay = s.layout(2.9);
            for w in &lay.winders {
                // a square tread's depth along the first run, where it is
                // one (all its points within the first run's width)
                let xs: Vec<f64> = w.outline.iter().map(|p| p.x).collect();
                let (lo, hi) = (
                    xs.iter().copied().fold(f64::MAX, f64::min),
                    xs.iter().copied().fold(f64::MIN, f64::max),
                );
                let square = w.outline.len() == 4 && hi < leg - 0.5 + 1e-6;
                assert!(
                    !square || hi - lo > 0.28 - 1e-6,
                    "over {over}: a tread {} deep",
                    hi - lo
                );
            }
            assert!(!s.pieces_overlap(2.9), "over {over}");
        }
    }

    #[test]
    fn changing_the_line_it_is_drawn_by_keeps_the_stair_in_place() {
        let (mut s, before) = drawn(&[(0.0, 0.0), (3.0, 0.0), (3.0, 2.5)], StairLine::Right);
        for d in [StairLine::Walk, StairLine::Left, StairLine::Right] {
            s.set_drawn_in_place(d, 2.9);
            assert_eq!(s.drawn, d);
            let after = s.layout(2.9);
            for (a, b) in before.walk.iter().zip(&after.walk) {
                assert!(a.distance(*b) < 1e-6, "{d:?}: {a} {b}");
            }
        }
    }

    #[test]
    fn a_line_folded_back_on_itself_has_flights_that_overlap() {
        let (s, _) = drawn(&[(0.0, 0.0), (3.0, 0.0), (3.0, 2.5)], StairLine::Walk);
        assert!(!s.pieces_overlap(2.9), "an L is sound");
        let (s, _) = drawn(&[(0.0, 0.0), (3.0, 0.0), (0.3, 0.4)], StairLine::Right);
        assert!(!s.pieces_overlap(2.9), "a U from one click is sound");
        // MU101 #5: turns under 135° each, its last flight back across
        // the first
        // the first (round a triangle, up a taller storey)
        let (s, _) = drawn(
            &[
                (0.0, 0.0),
                (2.25, 0.0),
                (3.375, 1.95),
                (1.125, 1.95),
                (0.2, 0.35),
            ],
            StairLine::Walk,
        );
        assert!(s.pieces_overlap(4.5));
    }

    #[test]
    fn a_stair_offers_its_head_foot_sides_and_landing_edges_as_guides() {
        let (s, lay) = drawn(&[(0.0, 0.0), (3.0, 0.0), (3.0, 2.5)], StairLine::Walk);
        let g = s.guide_lines(2.9);
        let has = |m: StairMark| g.iter().filter(|x| x.mark == m).count();
        assert_eq!((has(StairMark::Foot), has(StairMark::Head)), (1, 1));
        // an L's landing edges lie on its flights' sides: one guide each
        assert_eq!(has(StairMark::Side), 4);
        // a U's landing has a back edge of its own
        let (u, _) = drawn(&[(0.0, 0.0), (3.0, 0.0), (0.3, 0.4)], StairLine::Walk);
        assert!(u
            .guide_lines(2.9)
            .iter()
            .any(|x| x.mark == StairMark::Landing));
        // the foot runs across the first riser, at the line's start
        let foot = g.iter().find(|x| x.mark == StairMark::Foot).unwrap();
        assert!(foot.a.x.abs() < 1e-9 && foot.b.x.abs() < 1e-9);
        // the head across the last riser of the last flight
        let last = lay.flights.last().unwrap();
        let top = last.end(s.tread);
        let head = g.iter().find(|x| x.mark == StairMark::Head).unwrap();
        assert!((head.a.y - top.y).abs() < 1e-9 && (head.b.y - top.y).abs() < 1e-9);
        // no line twice
        for (i, x) in g.iter().enumerate() {
            for y in &g[i + 1..] {
                let (d, e) = ((x.b - x.a).normalize(), (y.b - y.a).normalize());
                assert!(!(d.perp_dot(e).abs() < 1e-6 && (x.a - y.a).perp_dot(e).abs() < 1e-6));
            }
        }
    }

    #[test]
    fn its_line_end_steps_by_whole_treads_and_short_treads_show() {
        let (s, _) = drawn(&[(0.0, 0.0), (2.0, 0.0)], StairLine::Walk);
        let rise = 2.9;
        // 2.1 m: 7.5 treads of 280; it goes to 7 or 8 treads' length
        let p = s.whole_treads_end(rise, DVec2::new(2.1, 0.0));
        let k = p.x / s.tread;
        assert!((k - k.round()).abs() < 1e-6, "{p}");
        assert!(p.y.abs() < 1e-9);
        // a 2 m line: 16 - 7 treads short, each one a ghost
        let (short, ghosts) = s.short_treads(rise);
        assert_eq!(short, 9);
        assert_eq!(ghosts.len(), 9);
        // the ghosts lie past the end of the line
        assert!(ghosts.iter().all(|g| g.iter().all(|q| q.x > 2.0 - 0.3)));
        // drawn by its side, the same
        let (s, _) = drawn(&[(0.0, 0.0), (3.0, 0.0), (3.0, 2.0)], StairLine::Right);
        let p = s.whole_treads_end(rise, DVec2::new(3.0, 2.13));
        let mut t = s.clone();
        let n = t.line.len();
        t.line[n - 1] = t.to_local(p);
        let lay = t.layout(rise);
        let f = lay.flights.last().unwrap();
        let end = *t.walk_in_plan(rise).last().unwrap();
        let avail = (end - f.start).dot(f.dir) / t.tread;
        assert!((avail - avail.round()).abs() < 1e-6, "{avail}");
    }

    #[test]
    fn a_landing_reports_its_depth_and_a_half_turn_its_gap() {
        let (_, ground, _) = two_levels();
        let mut s = Stair::new(ground, DVec2::ZERO, 0.0);
        let pts = [DVec2::ZERO, DVec2::new(3.0, 0.0), DVec2::new(0.3, 0.0001)];
        s.set_line_gap(&pts, 0.2);
        let sizes = s.landing_sizes(2.9);
        assert_eq!(sizes.len(), 1);
        let (_, depth, gap) = sizes[0];
        // a width deep, and what is left of a going where the flight stops
        assert!(depth > 1.0 - 1e-6 && depth < 1.0 + s.tread, "{depth}");
        assert!((gap.unwrap() - 0.2).abs() < 0.02, "{gap:?}");
        // an L's landing has no gap
        let (s, _) = drawn(&[(0.0, 0.0), (3.0, 0.0), (3.0, 2.5)], StairLine::Walk);
        assert!(s.landing_sizes(2.9).iter().all(|x| x.2.is_none()));
    }

    #[test]
    fn its_head_stays_put_when_its_treads_change_unless_its_foot_is_fixed() {
        let (m, ground, _) = two_levels();
        // a straight stair: its head exactly where it was
        let line = [DVec2::ZERO, DVec2::new(6.0, 0.0)];
        let mut st = Stair::from_line(ground, &line, StairLine::Walk).unwrap();
        let rise = st.rise(&m);
        let b0 = st.clone();
        let (h0, _) = st.head(rise).unwrap();
        st.tread = 0.25;
        st.keep_fixed_end(&b0, &m);
        assert!(h0.distance(st.head(rise).unwrap().0) < 1e-6);
        // an L: its turn stays, so its last flight keeps whole treads from
        // the landing; the head stays within a tread of where it was
        let pts = [DVec2::ZERO, DVec2::new(3.0, 0.0), DVec2::new(3.0, 3.0)];
        let mut s = Stair::from_line(ground, &pts, StairLine::Walk).unwrap();
        let before = s.clone();
        let (h0, _) = s.head(rise).unwrap();
        s.tread = 0.25;
        s.keep_fixed_end(&before, &m);
        let (h1, _) = s.head(rise).unwrap();
        assert!(h0.distance(h1) < s.tread, "{h0} {h1}");
        assert!((s.to_plan(s.line[1]) - DVec2::new(3.0, 0.0)).length() < 1e-9);
        // its foot fixed: the head moves, the start stays
        let mut f = before.clone();
        f.fixed_foot = true;
        let b2 = f.clone();
        f.tread = 0.25;
        f.keep_fixed_end(&b2, &m);
        assert!((f.start - DVec2::ZERO).length() < 1e-9);
    }

    #[test]
    fn a_side_drawn_line_puts_the_stair_beside_it() {
        let (s, lay) = drawn(&[(0.0, 0.0), (6.0, 0.0)], StairLine::Left);
        // drawn by its left side: the walk line is half a width to the right
        assert!((lay.flights[0].start - DVec2::new(0.0, -0.5)).length() < 1e-9);
        let (_, lay2) = drawn(&[(0.0, 0.0), (6.0, 0.0)], StairLine::Right);
        assert!((lay2.flights[0].start - DVec2::new(0.0, 0.5)).length() < 1e-9);
        // the line it was drawn by is kept, in the stair's frame
        assert_eq!(s.line, vec![DVec2::ZERO, DVec2::new(6.0, 0.0)]);
        assert!((s.line_in_plan(2.9)[1] - DVec2::new(6.0, 0.0)).length() < 1e-9);
    }

    #[test]
    fn a_short_line_runs_on_and_a_long_one_leaves_spare() {
        let (_, lay) = drawn(&[(0.0, 0.0), (2.0, 0.0)], StairLine::Walk);
        assert_eq!(lay.short, 16 - 7, "7 treads fit in 2 m");
        assert_eq!(
            lay.flights[0].risers, 17,
            "the flight still reaches the floor"
        );
        let (_, lay) = drawn(&[(0.0, 0.0), (6.0, 0.0), (6.0, 3.0)], StairLine::Walk);
        assert_eq!(lay.short, 0);
        assert_eq!(lay.landings.len(), 0, "the climb ends before the corner");
        assert!((lay.spare - (6.0 - 16.0 * 0.28 + 3.0)).abs() < 1e-9);
    }

    #[test]
    fn moving_and_turning_a_stair_takes_its_line_along() {
        let (mut s, _) = drawn(&[(1.0, 1.0), (4.0, 1.0), (4.0, 4.0)], StairLine::Walk);
        s.start += DVec2::new(10.0, 0.0);
        s.direction += 90.0;
        let line = s.line_in_plan(2.9);
        assert!((line[0] - DVec2::new(11.0, 1.0)).length() < 1e-9);
        assert!((line[1] - DVec2::new(11.0, 4.0)).length() < 1e-9);
        assert!((line[2] - DVec2::new(8.0, 4.0)).length() < 1e-9);
    }

    #[test]
    fn handrails_stay_continuous_and_run_on_past_the_ends() {
        let (s, _) = drawn(
            &[(1.2, 2.0), (3.95, 2.0), (3.95, 3.0), (1.48, 3.0)],
            StairLine::Walk,
        );
        let inner = stair_rail(&s, 2.9, true, false, None);
        let outer = stair_rail(&s, 2.9, false, true, Some(RailExtend::default()));
        // no gap anywhere along either rail
        for p in [&inner, &outer] {
            assert!(p
                .windows(2)
                .all(|w| w[0].truncate().distance(w[1].truncate()) < 3.0));
        }
        // the inner rail turns tight round the turn, inside the stair
        let ys: Vec<f64> = inner.iter().map(|p| p.y).collect();
        assert!(ys.iter().all(|y| (2.4..=2.6).contains(y)), "{ys:?}");
        // the wall rail: back to the wall, one going sloped and 300 level
        // at the bottom, 300 level at the top
        let first = outer[0];
        let level = outer[1];
        let sloped = outer[2];
        assert!((first.y - 1.5).abs() < 1e-6, "returns to the wall");
        assert!((level.z - sloped.z).abs() < 1e-9);
        assert!((sloped.x - level.x - 0.3).abs() < 1e-9);
        assert!((outer[3].z - sloped.z - 2.9 / 17.0).abs() < 1e-9);
        let n = outer.len();
        assert!((outer[n - 2].z - 2.9).abs() < 1e-9, "level at the top");
        assert!((outer[n - 1].y - 3.5).abs() < 1e-6);
    }

    #[test]
    fn an_l_stair_turns_after_its_landing() {
        let (m, ground, _) = two_levels();
        let mut s = Stair::new(ground, DVec2::ZERO, 0.0);
        s.landing = Some(Landing {
            after: 8,
            left: true,
        });
        let lay = s.layout(s.rise(&m));
        assert_eq!(lay.flights.len(), 2);
        assert_eq!(lay.flights[0].risers + lay.flights[1].risers, 16);
        let f2 = lay.flights[1].clone();
        assert!((f2.dir - DVec2::Y).length() < 1e-9);
        assert!((f2.z0 - 8.0 * 0.18).abs() < 1e-9);
        // the second flight starts at the landing's left side
        let edge = 7.0 * 0.28;
        assert!((f2.start - DVec2::new(edge + 0.5, 0.5)).length() < 1e-9);
        let path = s.side_path(2.88, false, 0.0);
        assert!(path.len() > 4, "the outer rail rounds the landing");
        assert!((path.last().unwrap().z - 2.88).abs() < 1e-9);
    }

    #[test]
    fn a_hip_roof_on_a_rectangle_has_four_faces_and_a_ridge() {
        let (_, ground, _) = two_levels();
        let mut r = Roof::new(ground, rect(10.0, 6.0), RoofShape::Hip);
        r.overhang = 0.0;
        r.pitch = 45.0;
        let faces = r.faces();
        assert_eq!(faces.len(), 4);
        // the ridge sits half the short side up at 45°
        let ridge = r.peak() - r.vertical_thickness();
        assert!((ridge - 3.0).abs() < 1e-9);
        let area: f64 = faces
            .iter()
            .map(|f| crate::polygon::signed_area2(&f.outline) / 2.0)
            .sum();
        assert!((area - 60.0).abs() < 1e-6);
        // the ridge line runs from (3,3) to (7,3)
        assert!((r.underside_at(DVec2::new(5.0, 3.0)) - 3.0).abs() < 1e-9);
        assert!((r.underside_at(DVec2::new(1.0, 3.0)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_gable_roof_slopes_from_the_long_sides_only() {
        let (_, ground, _) = two_levels();
        let mut r = Roof::new(ground, rect(10.0, 6.0), RoofShape::Gable);
        r.overhang = 0.5;
        r.pitch = 30.0;
        assert_eq!(r.sloped_edges(), vec![0, 2]);
        assert_eq!(r.faces().len(), 2);
        let ridge = r.underside_at(DVec2::new(5.0, 3.0));
        assert!((ridge - 3.0 * 30f64.to_radians().tan()).abs() < 1e-9);
        // the overhang dips below the wall plate
        assert!(r.underside_at(DVec2::new(5.0, -0.5)) < 0.0);
        assert!((r.plan_area() - 11.0 * 7.0).abs() < 1e-9);
    }

    #[test]
    fn a_shed_roof_rises_from_its_eave() {
        let (_, ground, _) = two_levels();
        let mut r = Roof::new(ground, rect(4.0, 3.0), RoofShape::Shed);
        r.overhang = 0.0;
        r.edge = 0;
        r.pitch = 10.0;
        assert_eq!(r.faces().len(), 1);
        let top = r.underside_at(DVec2::new(2.0, 3.0));
        assert!((top - 3.0 * 10f64.to_radians().tan()).abs() < 1e-9);
    }

    #[test]
    fn hips_on_an_l_shape_fall_back_to_flat() {
        let (_, ground, _) = two_levels();
        let l = vec![
            DVec2::ZERO,
            DVec2::new(4.0, 0.0),
            DVec2::new(4.0, 2.0),
            DVec2::new(2.0, 2.0),
            DVec2::new(2.0, 4.0),
            DVec2::new(0.0, 4.0),
        ];
        let r = Roof::new(ground, l, RoofShape::Hip);
        assert_eq!(r.effective_shape(), RoofShape::Flat);
        assert_eq!(r.faces().len(), 1);
    }

    #[test]
    fn columns_run_to_the_next_level_unless_given_a_height() {
        let (m, ground, first) = two_levels();
        let mut c = Column::new(
            ground,
            DVec2::new(1.0, 1.0),
            Profile::Rectangle {
                width: 0.3,
                depth: 0.3,
            },
        );
        assert!((c.height_in(&m) - 2.88).abs() < 1e-9);
        c.height = Some(2.0);
        assert_eq!(c.span(&m), (0.0, 2.0));
        // the top level has none above: its own height
        let top = Column::new(first, DVec2::ZERO, Profile::Round { diameter: 0.3 });
        assert!((top.height_in(&m) - 3.0).abs() < 1e-9);
        let b = Beam::new(ground, DVec2::ZERO, DVec2::new(5.0, 0.0));
        assert!((b.top(&m) - 2.88).abs() < 1e-9);
    }

    #[test]
    fn new_elements_round_trip_through_json() {
        let (mut m, ground, _) = two_levels();
        let s = Element::new(
            "Stair",
            ElementKind::Stair(Stair::new(ground, DVec2::ZERO, 90.0)),
        );
        let sid = s.id;
        let els = vec![
            Element::new(
                "Column",
                ElementKind::Column(Column::new(
                    ground,
                    DVec2::ZERO,
                    Profile::Round { diameter: 0.4 },
                )),
            ),
            Element::new(
                "Beam",
                ElementKind::Beam(Beam::new(ground, DVec2::ZERO, DVec2::X)),
            ),
            Element::new(
                "Roof",
                ElementKind::Roof(Roof::new(ground, rect(4.0, 4.0), RoofShape::Gable)),
            ),
            s,
        ];
        let mut tx = Transaction::new("add");
        for e in els {
            tx = tx.insert(e);
        }
        tx.apply(&mut m).unwrap();
        let rail = Railing::along_stair(&m, sid, true).unwrap();
        Transaction::new("rail")
            .insert(Element::new("Railing", ElementKind::Railing(rail)))
            .apply(&mut m)
            .unwrap();
        let back = Model::from_json(&m.to_json()).unwrap();
        assert_eq!(back.len(), m.len());
        for e in m.iter() {
            assert_eq!(back.get(e.id), Some(e));
        }
        // a stair's railing goes with it
        let tx = Transaction::new("delete").remove_cascade(&m, sid);
        assert_eq!(tx.ops.len(), 2);
    }

    #[test]
    fn older_files_without_optional_fields_still_load() {
        let json = r#"{"level":"00000000-0000-0000-0000-000000000001","start":[0.0,0.0],"direction":0.0,"width":1.0,"max_riser":0.18,"tread":0.28}"#;
        let s: Stair = serde_json::from_str(json).unwrap();
        assert_eq!(s.waist, 0.15);
        assert!(s.landing.is_none() && s.height.is_none());
    }

    #[test]
    fn columns_from_older_files_run_up_their_storey_as_before() {
        let (mut m, ground, first) = two_levels();
        let json = format!(
            r#"{{"level":"{}","position":[1.0,2.0],"profile":{{"Rectangle":{{"width":0.3,"depth":0.3}}}},"material":null}}"#,
            ground.0
        );
        let c: Column = serde_json::from_str(&json).unwrap();
        assert!(c.top.is_unconnected());
        assert_eq!(c.span(&m), (0.0, 2.88));
        // a constraint saves, and a column without one saves as before
        assert!(!serde_json::to_string(&c).unwrap().contains("top"));
        let mut tied = c.clone();
        tied.set_top_level(&m, Some(first), 0.5);
        assert_eq!(tied.span(&m), (0.0, 3.38));
        let back: Column = serde_json::from_str(&serde_json::to_string(&tied).unwrap()).unwrap();
        assert_eq!(back, tied);
        // its top level gone, it keeps the height it had when let go
        tied.set_top_level(&m, None, 0.0);
        assert_eq!(tied.height, Some(3.38));
        // the base grip lowers the base and leaves the top
        let mut e = Element::new("C", ElementKind::Column(c));
        let g = grips(&m, &e);
        assert_eq!(g.len(), 3);
        drag_grip(&m, &mut e, 2, g[2].at, g[2].at + DVec3::Z * 0.5, 0.0).unwrap();
        let ElementKind::Column(c) = &e.kind else {
            panic!()
        };
        assert_eq!(c.span(&m), (0.5, 2.88));
        Transaction::new("c").insert(e).apply(&mut m).unwrap();
    }

    #[test]
    fn posts_fall_at_corners_and_within_the_spacing() {
        let path = [
            DVec3::ZERO,
            DVec3::new(3.0, 0.0, 0.0),
            DVec3::new(3.0, 1.0, 0.0),
        ];
        let posts = posts_along(&path, 1.2);
        // 3 m in three bays, 1 m in one, plus the end
        assert_eq!(posts.len(), 5);
        assert!(posts.contains(&DVec3::new(3.0, 0.0, 0.0)));
    }
}

#[cfg(test)]
mod default_height_tests {
    use super::*;

    #[test]
    fn a_new_beam_hangs_under_the_slab_above_and_a_footing_under_the_ground_slab() {
        let m = crate::starter::starter_house();
        let level = |n: &str| {
            m.iter()
                .find(|e| e.name == n && matches!(e.kind, ElementKind::Level(_)))
                .unwrap()
                .id
        };
        let (gf, ff) = (level("Ground Floor"), level("First Floor"));
        // under the first floor slab, the depth of that slab
        let slab = m
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Slab(s) if s.level == ff => Some(s.clone()),
                _ => None,
            })
            .unwrap();
        let c = slab.outline().iter().copied().sum::<DVec2>() / slab.outline().len() as f64;
        let off = beam_top_offset(&m, gf, c - DVec2::X, c + DVec2::X);
        assert!(
            (off - (slab.thickness - slab.top_offset)).abs() < 1e-6,
            "{off}"
        );
        // far outside the house, nothing to hang under
        let far = DVec2::splat(500.0);
        assert_eq!(beam_top_offset(&m, gf, far, far + DVec2::X), 0.0);
    }
}
