//! The site around a building: the ground, hard landscape and planting.
//!
//! - **Terrain** is a ground surface over an outline, `thickness` deep:
//!   flat at its `top`, or following spot heights, between which it is
//!   interpolated (inverse distance weighting). Plans can show its
//!   contours. Hard landscape areas lying in a flat terrain are left out
//!   of it, so paving sits flush in the ground instead of on it.
//! - **Areas** are slab-like layers of hard landscape (paving, gravel, a
//!   driveway, decking), each with a material and a surface pattern that
//!   plans draw over it.
//! - **Pads, pits and trenches** are earthworks: each an element of its
//!   own ([`SiteForm::Pad`]) worked into the terrain under it, as a road
//!   is. The terrain keeps the ground as surveyed; the ground as designed
//!   is worked out from it and its works in `creabim_geom::earth`. Works
//!   kept inside a terrain by earlier files become elements on opening
//!   (see [`earlier_pads`]).
//! - **Plants** are trees and shrubs: a symbol the size of their canopy in
//!   plan, and a simple form (a trunk and a crown) in 3D and elevation.
//!
//! Cars, people and other entourage are catalogue objects (see
//! [`crate::catalogue`]). What each pattern or plant stands for is data:
//! the sizes and spacings are the element's own.

use crate::tree::TreeShape;
use crate::{transform::Xform, ElementId, Model};
use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

/// A site element on a level (usually the ground floor's or a site level).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Site {
    pub level: ElementId,
    pub form: SiteForm,
    /// What it is made of: the ground's, the area's surface, a plant's crown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<ElementId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SiteForm {
    /// A ground surface over `boundary` (counter-clockwise), `top` above
    /// the level where it is flat, `thickness` deep below its lowest point.
    Terrain {
        boundary: Vec<DVec2>,
        #[serde(default)]
        top: f64,
        thickness: f64,
        /// Spot heights inside it (`z` above the level); none is flat.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        points: Vec<DVec3>,
        /// Contours every so much height in plan; 0 draws none.
        #[serde(default, skip_serializing_if = "is_zero")]
        contour: f64,
        /// The rest of what shapes and draws it: surveyed lines, how its
        /// edge takes its height, how it is drawn.
        #[serde(default, skip_serializing_if = "GroundData::is_plain")]
        data: GroundData,
    },
    /// Hard landscape: a layer `thickness` deep with its top `top` above
    /// the level, less its holes, its surface patterned.
    Area {
        boundary: Vec<DVec2>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        holes: Vec<Vec<DVec2>>,
        #[serde(default)]
        top: f64,
        thickness: f64,
        #[serde(default)]
        pattern: Pattern,
        /// A car park: its aisles, rows and bays laid inside the outline
        /// (see [`crate::carpark`]).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parking: Option<crate::carpark::CarPark>,
    },
    /// An earthwork, worked into the terrain under it: a pad, a pit or a
    /// trench (see [`Site::work_kind`]).
    ///
    /// A pad levels the ground at `top` above the level over `boundary`
    /// (counter-clockwise), on a base course `base` deep, with banks of
    /// 1 : `cut` where it digs in and 1 : `fill` where it is made up (run
    /// over rise; 0 is an upright, retained face). A pit or a trench has
    /// a `depth`: its floor is that far below the ground all over, and
    /// `top` is not used. A trench also keeps the line it was drawn
    /// along and its width; its `boundary` follows them.
    Pad {
        boundary: Vec<DVec2>,
        top: f64,
        base: f64,
        cut: f64,
        fill: f64,
        /// Above 0: an excavation this deep below the ground.
        #[serde(default, skip_serializing_if = "is_zero")]
        depth: f64,
        /// A trench's line; empty for a pad or a pit.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        path: Vec<DVec2>,
        /// A trench's width.
        #[serde(default, skip_serializing_if = "is_zero")]
        width: f64,
        /// It only digs: where the ground is below it, the ground stays.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        only_cut: bool,
        /// The edges cut upright, whatever the banks: edge `i` runs
        /// from corner `i` to the next. With no wall the ground simply
        /// stands (rock, or a face to be held later).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        retained: Vec<usize>,
        /// The retaining walls standing on retained edges: the edge and
        /// its wall. The wall stands outside the outline, its face on
        /// the edge, and the ground is cut at its back.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        walls: Vec<(usize, ElementId)>,
        /// A trench's bends are rounded to this radius on its line.
        #[serde(default, skip_serializing_if = "is_zero")]
        bend: f64,
    },
    /// A tree or shrub standing at `position`, its base `base` above the
    /// level: `canopy` across its crown, `height` to its top.
    Plant {
        position: DVec2,
        #[serde(default)]
        base: f64,
        form: PlantForm,
        canopy: f64,
        height: f64,
        /// A tree grown from a shape (see [`crate::tree`]); none draws
        /// the simple form. Only a [`PlantForm::Tree`] uses it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tree: Option<Box<TreeShape>>,
    },
    /// A property: the lot's boundary (counter-clockwise), what each edge
    /// is, and the planning rules it is built to (see [`crate::lot`]).
    Property {
        boundary: Vec<DVec2>,
        /// Edge `i` runs from corner `i` to the next; edges past the end
        /// of the list take [`crate::lot::edge_kind`]'s defaults.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        edges: Vec<crate::lot::LotEdge>,
        #[serde(default)]
        rules: crate::lot::PlanningRules,
    },
    /// A road, drive or path along a centreline (see [`crate::road`]).
    Road(crate::road::Road),
    /// A drain run, laid to a fall from chamber to chamber (see
    /// [`crate::services`]).
    Drain(crate::services::Drain),
    /// A tank buried in the ground (see [`crate::services`]).
    Tank(crate::services::Tank),
    /// A path laid on the ground along a line (see [`crate::walk`]).
    Walk(crate::walk::Walk),
    /// A spot level: the ground of the terrain it lies on passes through
    /// `height` above the level at `position`.
    Spot { position: DVec2, height: f64 },
    /// A contour or a break line drawn on a terrain, which the ground of
    /// that terrain passes through (`z` above the level). A contour's
    /// points are all at one height.
    Line(GroundLine),
    /// A borehole, trial pit or probe of a soils report (see
    /// [`crate::investigation`]).
    Investigation(crate::investigation::Probe),
}

/// What a line of ground data is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroundLineKind {
    /// A line of one height. The ground passes through it smoothly.
    #[default]
    Contour,
    /// A surveyed string (a kerb, the top of a bank, a ditch): the ground
    /// folds along it, and runs straight between its points.
    Break,
}

impl GroundLineKind {
    pub fn label(self) -> &'static str {
        match self {
            GroundLineKind::Contour => "Contour",
            GroundLineKind::Break => "Break line",
        }
    }
}

/// A line the ground passes through: its points, `z` above the level.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GroundLine {
    #[serde(default)]
    pub kind: GroundLineKind,
    pub points: Vec<DVec3>,
    /// Joined back to its first point.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub closed: bool,
}

/// What shapes a terrain besides its spot heights, and how it is drawn.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GroundData {
    /// Contours and break lines.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<GroundLine>,
    /// A survey: the outline's corners take their height from the data
    /// nearest them. Otherwise they stay at the terrain's `top`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub survey: bool,
    /// Drawn as the survey's flat triangles, not smoothed through them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub faceted: bool,
    /// Every so many contours one is drawn heavier; 0 is every fifth.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub major: u32,
    /// Heavier contours carry their height.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub labels: bool,
    /// Spot heights carry their height.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub levels: bool,
    /// What was taken off a survey's coordinates when it was brought in:
    /// a point of the survey is its place here plus this (heights too).
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub shift: DVec3,
    /// What the design does to the ground, in order: the existing ground
    /// is the data above, the proposed ground is it with these worked in.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pads: Vec<Pad>,
    /// The ground is dug out under the building's lowest slabs.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub under_buildings: bool,
    /// The soil under the ground as surveyed, layer by layer from the
    /// top; what is below the last is not named.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub strata: Vec<Stratum>,
    /// The ground shaded in plans by its slope, the way it faces or its
    /// height (see [`GroundAnalysis`]).
    #[serde(default, skip_serializing_if = "GroundAnalysis::is_none")]
    pub analysis: GroundAnalysis,
    /// Arrows in plans showing which way water runs off, and its low
    /// points ringed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flow: bool,
}

/// How a terrain is shaded in plans to read its form.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroundAnalysis {
    #[default]
    None,
    /// By how steep it is, in bands of gradient.
    Slope,
    /// By the way it faces: north, north-east...
    Aspect,
    /// By height, in bands.
    Height,
}

impl GroundAnalysis {
    pub const ALL: [GroundAnalysis; 4] = [
        GroundAnalysis::None,
        GroundAnalysis::Slope,
        GroundAnalysis::Aspect,
        GroundAnalysis::Height,
    ];

    pub fn is_none(&self) -> bool {
        *self == GroundAnalysis::None
    }

    pub fn label(self) -> &'static str {
        match self {
            GroundAnalysis::None => "None",
            GroundAnalysis::Slope => "Slope",
            GroundAnalysis::Aspect => "Aspect",
            GroundAnalysis::Height => "Height",
        }
    }
}

/// Which ground a view shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroundState {
    /// The ground with the works in it.
    #[default]
    After,
    /// The ground as surveyed, before any works.
    Before,
    /// The ground with the works, and as it was dashed under it.
    Both,
}

impl GroundState {
    pub const ALL: [GroundState; 3] = [GroundState::After, GroundState::Both, GroundState::Before];

    pub fn label(self) -> &'static str {
        match self {
            GroundState::After => "After the works",
            GroundState::Before => "Before the works",
            GroundState::Both => "After, with the ground as it was",
        }
    }
}

/// How a view shows the site: which ground, and each kind of site
/// information on or off. Nothing is baked into the model: the same
/// terrain draws as a survey in one view and an earthworks plan in
/// another.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SiteView {
    pub ground: GroundState,
    /// Contour lines.
    pub contours: bool,
    /// Heights and levels written on the ground and roads.
    pub levels: bool,
    /// Where the ground is dug and made up, coloured by depth.
    pub cut_fill: bool,
    /// The banks of pads and roads, hatched.
    pub banks: bool,
    /// The line round everything the works change.
    pub limit: bool,
    /// Property lines, setbacks and easements.
    pub legal: bool,
    /// Roads, drives and car parks.
    pub roads: bool,
    /// Drains, chambers and tanks under the ground.
    pub services: bool,
    /// Boreholes, trial pits and probes of the soils report.
    pub investigation: bool,
    /// How this view draws the ground: its contours, heights, shading
    /// and water. None is a view that has set nothing of its own: it
    /// draws as the terrain says (see [`GroundLook::of_terrain`]), which
    /// is how every plan of an older file keeps its drawing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub look: Option<GroundLook>,
    /// Setback lines and the buildable area of a property. None is as
    /// the property says, as older files kept it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setbacks: Option<bool>,
    /// Bearings and lengths along a property's edges. None is as the
    /// property says.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bearings: Option<bool>,
    /// A property's labels written along their edge, turned to read from
    /// below or the right; off, they are written level (MU98_03).
    pub along: bool,
    /// The site's tints and fills in colour; off, in greys of the same
    /// lightness, for a plan in black and white (Site polish 5).
    pub tints: bool,
    /// Level tags moved aside in this view, so they read clear of a wall
    /// or each other (owner, cut test).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<TagMove>,
    /// Level tags in a plan (spot levels, a survey's heights, a road's
    /// levels) moved by hand in this view (MU142: they are tags).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub spot_tags: Vec<SpotTagMove>,
}

/// A level tag in a plan moved from where it is drawn: the site element's,
/// the one reading the level at `at`, its words moved by `by`, its leader
/// bent at `elbow` (from `at`), in the drawing's metres.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpotTagMove {
    pub site: ElementId,
    pub at: DVec2,
    pub by: DVec2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elbow: Option<DVec2>,
}

/// How near two points are taken for the same level tag's in a plan.
const SPOT_SAME: f64 = 0.05;

/// A level tag in a section moved from where it is drawn: the terrain's,
/// the one standing at `x` along the section, its words moved by `by`
/// (in the drawing's metres, at its scale).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TagMove {
    pub terrain: ElementId,
    pub x: f64,
    pub by: DVec2,
    /// The bend in its leader, from the point on the ground it measures
    /// (MU142: level tags are tags, with a leader's grips).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elbow: Option<DVec2>,
}

impl SiteView {
    /// The move of the tag of `terrain` standing at `x`, if it has one.
    pub fn tag(&self, terrain: ElementId, x: f64) -> Option<TagMove> {
        self.tags
            .iter()
            .filter(|t| t.terrain == terrain && (t.x - x).abs() < TAG_SAME)
            .min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs()))
            .copied()
    }

    /// How far the tag of `terrain` standing at `x` is moved.
    pub fn tag_move(&self, terrain: ElementId, x: f64) -> Option<DVec2> {
        self.tag(terrain, x).map(|t| t.by)
    }

    /// Moves the tag of `terrain` at `x` by `by`, keeping its leader's
    /// bend; none puts it back where it is drawn, bend and all.
    pub fn set_tag_move(&mut self, terrain: ElementId, x: f64, by: Option<DVec2>) {
        let elbow = self.tag(terrain, x).and_then(|t| t.elbow);
        self.tags
            .retain(|t| !(t.terrain == terrain && (t.x - x).abs() < TAG_SAME));
        if let Some(by) = by {
            self.tags.push(TagMove {
                terrain,
                x,
                by,
                elbow,
            });
        }
    }

    /// Bends the leader of the tag of `terrain` at `x` at `elbow` (from
    /// the point it measures); none straightens it.
    pub fn set_tag_bend(&mut self, terrain: ElementId, x: f64, elbow: Option<DVec2>) {
        let by = self.tag_move(terrain, x).unwrap_or(DVec2::ZERO);
        self.tags
            .retain(|t| !(t.terrain == terrain && (t.x - x).abs() < TAG_SAME));
        if elbow.is_some() || by != DVec2::ZERO {
            self.tags.push(TagMove {
                terrain,
                x,
                by,
                elbow,
            });
        }
    }

    /// The move of the plan level tag of `site` reading the level at `at`.
    pub fn spot_tag(&self, site: ElementId, at: DVec2) -> Option<SpotTagMove> {
        self.spot_tags
            .iter()
            .filter(|t| t.site == site && t.at.distance(at) < SPOT_SAME)
            .min_by(|a, b| a.at.distance(at).total_cmp(&b.at.distance(at)))
            .copied()
    }

    /// Moves the plan level tag of `site` at `at` by `by`, keeping its
    /// leader's bend; none puts it back, bend and all.
    pub fn set_spot_move(&mut self, site: ElementId, at: DVec2, by: Option<DVec2>) {
        let elbow = self.spot_tag(site, at).and_then(|t| t.elbow);
        self.spot_tags
            .retain(|t| !(t.site == site && t.at.distance(at) < SPOT_SAME));
        if let Some(by) = by {
            self.spot_tags.push(SpotTagMove {
                site,
                at,
                by,
                elbow,
            });
        }
    }

    /// Bends the leader of the plan level tag of `site` at `at` at
    /// `elbow` (from `at`); none straightens it.
    pub fn set_spot_bend(&mut self, site: ElementId, at: DVec2, elbow: Option<DVec2>) {
        let by = self.spot_tag(site, at).map_or(DVec2::ZERO, |t| t.by);
        self.spot_tags
            .retain(|t| !(t.site == site && t.at.distance(at) < SPOT_SAME));
        if elbow.is_some() || by != DVec2::ZERO {
            self.spot_tags.push(SpotTagMove {
                site,
                at,
                by,
                elbow,
            });
        }
    }
}

/// How near along a section two tags are taken for the same one, so a
/// moved tag stays moved while the ground under it shifts a little.
const TAG_SAME: f64 = 0.3;

/// How a view draws the ground: a 1:500 site plan and a 1:100 plan want
/// different contour intervals, and a slope study is one view, not all.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GroundLook {
    /// Contours every so much height; 0 draws none.
    pub interval: f64,
    /// Every so many contours one is drawn heavier; 0 is every fifth.
    pub major: u32,
    /// Heavier contours carry their height.
    pub heights: bool,
    /// Spot heights carry their level.
    pub spots: bool,
    /// The ground shaded by its slope, the way it faces or its height.
    pub shade: GroundAnalysis,
    /// Arrows showing which way water runs off, its low points ringed.
    pub flow: bool,
}

impl Default for GroundLook {
    fn default() -> Self {
        Self {
            interval: 0.0,
            major: 0,
            heights: false,
            spots: false,
            shade: GroundAnalysis::None,
            flow: false,
        }
    }
}

impl GroundLook {
    /// What a terrain itself says of how it is drawn: what a view that
    /// has set nothing of its own shows. It is what files from before
    /// views carried this kept, and what a survey sets when brought in.
    pub fn of_terrain(contour: f64, data: &GroundData) -> Self {
        Self {
            interval: contour,
            major: data.major,
            heights: data.labels,
            spots: data.levels,
            shade: data.analysis,
            flow: data.flow,
        }
    }

    /// Every how many contours one is heavier.
    pub fn major_every(&self) -> u32 {
        if self.major == 0 {
            5
        } else {
            self.major
        }
    }
}

impl SiteView {
    /// How this view draws a terrain: its own look, or the terrain's.
    pub fn look_on(&self, contour: f64, data: &GroundData) -> GroundLook {
        self.look
            .clone()
            .unwrap_or_else(|| GroundLook::of_terrain(contour, data))
    }

    /// Whether this view draws a property's setback lines.
    pub fn setbacks_of(&self, rules: &crate::PlanningRules) -> bool {
        rules.has_setbacks && self.setbacks.unwrap_or(rules.setbacks)
    }

    /// Whether this view writes a property's bearings and lengths.
    pub fn bearings_of(&self, rules: &crate::PlanningRules) -> bool {
        self.bearings.unwrap_or(rules.bearings)
    }

    /// The same view with the switches of `preset`, keeping how it draws
    /// the ground and the lot.
    pub fn with_preset(&self, preset: SitePreset) -> SiteView {
        SiteView {
            look: self.look.clone(),
            setbacks: self.setbacks,
            bearings: self.bearings,
            along: self.along,
            tints: self.tints,
            tags: self.tags.clone(),
            ..preset.view()
        }
    }
}

/// How the 3D view draws a terrain's contours: as the first plan that has
/// set its own, or as the terrain says.
pub fn look_in_3d(model: &crate::Model, contour: f64, data: &GroundData) -> GroundLook {
    model
        .iter()
        .find_map(|e| match &e.kind {
            crate::ElementKind::View(v) if matches!(v.kind, crate::ViewKind::Plan { .. }) => {
                v.site.look.clone()
            }
            _ => None,
        })
        .unwrap_or_else(|| GroundLook::of_terrain(contour, data))
}

impl Default for SiteView {
    fn default() -> Self {
        Self {
            ground: GroundState::After,
            contours: true,
            levels: true,
            cut_fill: false,
            banks: true,
            limit: false,
            legal: true,
            roads: true,
            services: true,
            investigation: true,
            look: None,
            setbacks: None,
            bearings: None,
            along: true,
            tints: true,
            tags: vec![],
            spot_tags: vec![],
        }
    }
}

/// A ready-made set of site switches, for the sheet each trade expects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SitePreset {
    Survey,
    Permit,
    Earthworks,
    Drainage,
    /// Where the soils report looked: the ground as surveyed, its
    /// boreholes, trial pits and probes.
    Investigation,
    Everything,
}

impl SitePreset {
    pub const ALL: [SitePreset; 6] = [
        SitePreset::Survey,
        SitePreset::Permit,
        SitePreset::Earthworks,
        SitePreset::Drainage,
        SitePreset::Investigation,
        SitePreset::Everything,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SitePreset::Survey => "Survey",
            SitePreset::Permit => "Permit site plan",
            SitePreset::Earthworks => "Earthworks",
            SitePreset::Drainage => "Drainage",
            SitePreset::Investigation => "Investigation",
            SitePreset::Everything => "Everything",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            SitePreset::Survey => "The ground as surveyed: its contours, levels and the property line, no works",
            SitePreset::Permit => "The works with the ground as it was dashed, levels, roads, drains and property lines; no colours",
            SitePreset::Earthworks => "Cut and fill coloured by depth, banks, the limit of works and both grounds",
            SitePreset::Drainage => "Drains, chambers and tanks with their levels on the finished ground, with roads and property lines; no banks or colours",
            SitePreset::Investigation => "The ground as surveyed with the soils report's boreholes, trial pits and probes, their levels and depths",
            SitePreset::Everything => "Every kind of site information at once",
        }
    }

    pub fn view(self) -> SiteView {
        let all = SiteView {
            ground: GroundState::Both,
            contours: true,
            levels: true,
            cut_fill: true,
            banks: true,
            limit: true,
            legal: true,
            roads: true,
            services: true,
            investigation: true,
            look: None,
            setbacks: None,
            bearings: None,
            along: true,
            tints: true,
            tags: vec![],
            spot_tags: vec![],
        };
        match self {
            SitePreset::Survey => SiteView {
                ground: GroundState::Before,
                cut_fill: false,
                banks: false,
                limit: false,
                roads: false,
                services: false,
                investigation: false,
                ..all
            },
            SitePreset::Permit => SiteView {
                cut_fill: false,
                limit: false,
                investigation: false,
                ..all
            },
            SitePreset::Earthworks => SiteView {
                legal: false,
                investigation: false,
                ..all
            },
            SitePreset::Drainage => SiteView {
                ground: GroundState::After,
                cut_fill: false,
                banks: false,
                limit: false,
                investigation: false,
                ..all
            },
            SitePreset::Investigation => SiteView {
                ground: GroundState::Before,
                cut_fill: false,
                banks: false,
                limit: false,
                roads: false,
                services: false,
                ..all
            },
            SitePreset::Everything => all,
        }
    }

    /// The preset a view's switches are, if any.
    pub fn of(v: &SiteView) -> Option<SitePreset> {
        // a preset is the switches; how the ground is drawn is beside it
        Self::ALL.into_iter().find(|p| v.with_preset(*p) == *v)
    }
}

impl SiteView {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// A levelled platform worked into the ground: dug where the ground is
/// above it, made up where it is below, with a bank each way running out
/// to meet the ground as it was.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pad {
    /// Its outline in plan.
    pub outline: Vec<DVec2>,
    /// Its height above the terrain's level.
    pub level: f64,
    /// The bank where it is dug in: how far it runs out for each metre
    /// it climbs. 0 is a retained, upright face.
    #[serde(default = "one")]
    pub cut: f64,
    /// The bank where it is made up, the same way.
    #[serde(default = "two")]
    pub fill: f64,
    /// It only digs: where the ground is below it, the ground stays (the
    /// hole dug for a building's slab).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub only_cut: bool,
    /// An excavation: above 0, its floor is this far below the ground
    /// all over (a trench, a pit) and `level` is not used.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub depth: f64,
    /// A sloping pad (a road): each corner's own height, `level` not
    /// used; between corners its edge runs straight. None is level.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heights: Vec<f64>,
    /// A sloping pad's floor, just inside its edge, this far below it:
    /// room for what is built on it (a road's make-up and kerbs).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sunk: f64,
    /// The edges of its outline with an upright face whatever its banks
    /// (a retaining wall): edge `i` runs from corner `i` to the next.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upright: Vec<usize>,
    /// How far behind each upright edge the ground is cut: the thickness
    /// of the wall standing there, beside `upright`. Empty is none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub behind: Vec<f64>,
}

/// Which earthwork a [`SiteForm::Pad`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkKind {
    /// The ground levelled at a height.
    Pad,
    /// Dug to a depth below the ground inside an outline.
    Pit,
    /// Dug to a depth along a line.
    Trench,
}

impl WorkKind {
    pub fn label(self) -> &'static str {
        match self {
            WorkKind::Pad => "Pad",
            WorkKind::Pit => "Pit",
            WorkKind::Trench => "Trench",
        }
    }
}

/// A layer of soil under a terrain, from the ground as surveyed down.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stratum {
    pub name: String,
    /// How thick it is.
    pub thickness: f64,
}

fn one() -> f64 {
    1.0
}

fn two() -> f64 {
    2.0
}

impl Pad {
    /// A pad with the usual banks: 1 in 1 dug, 1 in 2 made up.
    pub fn new(outline: Vec<DVec2>, level: f64) -> Self {
        Self {
            outline,
            level,
            cut: 1.0,
            fill: 2.0,
            only_cut: false,
            depth: 0.0,
            heights: vec![],
            sunk: 0.0,
            upright: vec![],
            behind: vec![],
        }
    }

    /// An excavation `depth` below the ground inside `outline`, with
    /// sides running out `sides` for each metre of depth.
    pub fn dig(outline: Vec<DVec2>, depth: f64, sides: f64) -> Self {
        Self {
            outline,
            level: 0.0,
            cut: sides,
            fill: 0.0,
            only_cut: false,
            depth,
            heights: vec![],
            sunk: 0.0,
            upright: vec![],
            behind: vec![],
        }
    }

    /// Its height at `q` on (or near) its edge: its level, or a sloping
    /// pad's height along its nearest edge.
    pub fn height_at(&self, q: DVec2) -> f64 {
        let n = self.outline.len();
        if self.heights.len() != n || n < 2 {
            return self.level;
        }
        let mut best = (f64::INFINITY, self.level);
        for i in 0..n {
            let (a, b) = (self.outline[i], self.outline[(i + 1) % n]);
            let d = b - a;
            let t = ((q - a).dot(d) / d.length_squared().max(1e-12)).clamp(0.0, 1.0);
            let dist = (a + d * t).distance(q);
            if dist < best.0 {
                let z = self.heights[i] + (self.heights[(i + 1) % n] - self.heights[i]) * t;
                best = (dist, z);
            }
        }
        best.1
    }

    pub fn sloping(&self) -> bool {
        self.heights.len() == self.outline.len() && !self.heights.is_empty()
    }
}

/// The outline of a trench `width` wide along `path`: the path offset
/// half the width each side, mitred at its corners.
pub fn trench_outline(path: &[DVec2], width: f64) -> Vec<DVec2> {
    trench_round(path, 0.0, width)
}

/// A trench's outline along `path` with its bends rounded to `radius`:
/// round a bend its inner side a smaller arc, or a sharp corner where the
/// trench is wider than the bend (see [`crate::road::offset_round`]).
pub fn trench_round(path: &[DVec2], radius: f64, width: f64) -> Vec<DVec2> {
    if path.len() < 2 {
        return vec![];
    }
    let h = width / 2.0;
    let mut out = crate::road::offset_round(path, radius, h);
    out.extend(
        crate::road::offset_round(path, radius, -h)
            .into_iter()
            .rev(),
    );
    ccw(&mut out);
    out
}

fn is_zero3(v: &DVec3) -> bool {
    *v == DVec3::ZERO
}

impl GroundData {
    pub fn is_plain(&self) -> bool {
        *self == Self::default()
    }

    /// Every how many contours one is heavier.
    pub fn major_every(&self) -> u32 {
        if self.major == 0 {
            5
        } else {
            self.major
        }
    }
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// The pattern drawn over an area's surface in plan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Pattern {
    /// Its outline only.
    #[default]
    None,
    /// Rectangular units `size` (along `angle`, across) with joints
    /// between: flags, blocks, setts. `stagger` offsets every other course
    /// by half a unit, as stretcher bond.
    Units {
        size: DVec2,
        #[serde(default)]
        stagger: bool,
        /// Degrees, counter-clockwise from the x axis.
        #[serde(default)]
        angle: f64,
    },
    /// Parallel lines `spacing` apart: boards, decking, ribbed concrete.
    Lines {
        spacing: f64,
        #[serde(default)]
        angle: f64,
    },
    /// Scattered dots about `spacing` apart: gravel, chippings, sand.
    Stipple { spacing: f64 },
}

impl Pattern {
    pub fn label(&self) -> &'static str {
        match self {
            Pattern::None => "None",
            Pattern::Units { .. } => "Units",
            Pattern::Lines { .. } => "Lines",
            Pattern::Stipple { .. } => "Stipple",
        }
    }

    fn validate(&self) -> Result<(), String> {
        let ok = match *self {
            Pattern::None => true,
            Pattern::Units { size, angle, .. } => {
                size.is_finite() && size.min_element() > 1e-3 && angle.is_finite()
            }
            Pattern::Lines { spacing, angle } => {
                spacing.is_finite() && spacing > 1e-3 && angle.is_finite()
            }
            Pattern::Stipple { spacing } => spacing.is_finite() && spacing > 1e-3,
        };
        if ok {
            Ok(())
        } else {
            Err("a surface pattern's sizes must be positive".into())
        }
    }
}

/// The shape a plant is drawn as.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlantForm {
    /// A broadleaf tree: a clear trunk under a round crown.
    #[default]
    Tree,
    /// A conifer: a cone from near the ground.
    Conifer,
    /// A palm: a tall bare trunk and a head of fronds.
    Palm,
    /// A shrub or bush: a low mound from the ground.
    Shrub,
}

impl PlantForm {
    pub const ALL: [PlantForm; 4] = [
        PlantForm::Tree,
        PlantForm::Conifer,
        PlantForm::Palm,
        PlantForm::Shrub,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PlantForm::Tree => "Tree",
            PlantForm::Conifer => "Conifer",
            PlantForm::Palm => "Palm",
            PlantForm::Shrub => "Shrub",
        }
    }
}

impl Site {
    /// A flat terrain over `boundary`.
    pub fn terrain(level: ElementId, mut boundary: Vec<DVec2>, top: f64, thickness: f64) -> Self {
        ccw(&mut boundary);
        Self {
            level,
            form: SiteForm::Terrain {
                boundary,
                top,
                thickness,
                points: vec![],
                contour: 0.0,
                data: GroundData::default(),
            },
            material: None,
        }
    }

    /// A hard landscape area over `boundary`.
    pub fn area(
        level: ElementId,
        mut boundary: Vec<DVec2>,
        thickness: f64,
        pattern: Pattern,
    ) -> Self {
        ccw(&mut boundary);
        Self {
            level,
            form: SiteForm::Area {
                boundary,
                holes: vec![],
                top: 0.0,
                thickness,
                pattern,
                parking: None,
            },
            material: None,
        }
    }

    /// The car park a hard landscape area is, if it is one.
    pub fn as_car_park(&self) -> Option<&crate::carpark::CarPark> {
        match &self.form {
            SiteForm::Area { parking, .. } => parking.as_ref(),
            _ => None,
        }
    }

    /// A car park's bays as laid out in `model`: its entrances are where
    /// roads end at its outline, its doors where paths do.
    pub fn car_park_layout(&self, model: &crate::Model) -> Option<crate::carpark::Layout> {
        let SiteForm::Area {
            boundary,
            holes,
            parking: Some(cp),
            ..
        } = &self.form
        else {
            return None;
        };
        // an end within a stride of the outline, or inside it
        let meets = |p: DVec2| {
            let n = boundary.len();
            (0..n).any(|i| {
                let (a, b) = (boundary[i], boundary[(i + 1) % n]);
                let ab = b - a;
                let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
                p.distance(a + ab * t) < 1.0
            })
        };
        let onto = |p: DVec2| {
            // the nearest point of the outline
            let n = boundary.len();
            (0..n)
                .map(|i| {
                    let (a, b) = (boundary[i], boundary[(i + 1) % n]);
                    let ab = b - a;
                    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
                    a + ab * t
                })
                .min_by(|x, y| x.distance(p).total_cmp(&y.distance(p)))
                .unwrap_or(p)
        };
        let (mut entrances, mut doors) = (vec![], vec![]);
        let mut clear: Vec<Vec<DVec2>> = vec![];
        let mut all: Vec<&crate::Element> = model.iter().collect();
        all.sort_by_key(|e| e.id);
        for e in all {
            let crate::ElementKind::Site(s) = &e.kind else {
                continue;
            };
            match &s.form {
                SiteForm::Road(r) => {
                    for p in [r.path.first(), r.path.last()].into_iter().flatten() {
                        if meets(p.truncate()) {
                            entrances.push(onto(p.truncate()));
                        }
                    }
                }
                SiteForm::Walk(w) => {
                    for p in [w.path.first(), w.path.last()].into_iter().flatten() {
                        if meets(*p) {
                            doors.push(onto(*p));
                        }
                    }
                }
                // a tree standing in it: the bays keep off its trunk
                SiteForm::Plant { position, .. }
                    if cp.tree_clear > 0.0 && crate::polygon::contains(boundary, *position) =>
                {
                    clear.push(
                        (0..8)
                            .map(|k| {
                                *position
                                    + DVec2::from_angle(std::f64::consts::TAU * k as f64 / 8.0)
                                        * cp.tree_clear
                            })
                            .collect(),
                    );
                }
                _ => {}
            }
        }
        Some(cp.layout_clear(boundary, holes, &entrances, &doors, &clear))
    }

    /// A pad over `boundary` at `top` above the level, with the usual
    /// banks: 1 in 1 dug, 1 in 2 made up.
    pub fn pad(level: ElementId, mut boundary: Vec<DVec2>, top: f64) -> Self {
        ccw(&mut boundary);
        Self {
            level,
            form: SiteForm::Pad {
                boundary,
                top,
                base: 0.0,
                cut: 1.0,
                fill: 2.0,
                depth: 0.0,
                path: vec![],
                width: 0.0,
                only_cut: false,
                retained: vec![],
                walls: vec![],
                bend: 0.0,
            },
            material: None,
        }
    }

    /// A pit `depth` below the ground inside `boundary`, its sides lying
    /// back `sides` for each metre of depth.
    pub fn pit(level: ElementId, boundary: Vec<DVec2>, depth: f64, sides: f64) -> Self {
        let mut s = Self::pad(level, boundary, 0.0);
        if let SiteForm::Pad {
            cut,
            fill,
            depth: d,
            ..
        } = &mut s.form
        {
            (*cut, *fill, *d) = (sides, 0.0, depth);
        }
        s
    }

    /// A trench `width` wide and `depth` deep along `path`, its sides
    /// upright.
    pub fn trench(level: ElementId, path: Vec<DVec2>, width: f64, depth: f64) -> Self {
        let mut s = Self::pit(level, trench_outline(&path, width), depth, 0.0);
        if let SiteForm::Pad {
            path: line,
            width: w,
            ..
        } = &mut s.form
        {
            (*line, *w) = (path, width);
        }
        s
    }

    /// A spot level at `position`, `height` above the level.
    pub fn spot(level: ElementId, position: DVec2, height: f64) -> Self {
        Self {
            level,
            form: SiteForm::Spot { position, height },
            material: None,
        }
    }

    /// A contour along `path`, all of it `height` above the level.
    pub fn contour(level: ElementId, path: Vec<DVec2>, height: f64) -> Self {
        Self {
            level,
            form: SiteForm::Line(GroundLine {
                kind: GroundLineKind::Contour,
                points: path.into_iter().map(|p| p.extend(height)).collect(),
                closed: false,
            }),
            material: None,
        }
    }

    /// A spot level or a ground line: an element that shapes the terrain
    /// it lies on and is nothing built.
    pub fn is_ground_data(&self) -> bool {
        matches!(self.form, SiteForm::Spot { .. } | SiteForm::Line(_))
    }

    /// The one height of a spot level or of a contour; none for a break
    /// line, whose points each have their own, or anything else.
    pub fn ground_height(&self) -> Option<f64> {
        match &self.form {
            SiteForm::Spot { height, .. } => Some(*height),
            SiteForm::Line(l) if l.kind == GroundLineKind::Contour => l.points.first().map(|p| p.z),
            _ => None,
        }
    }

    /// Give a spot level or a contour its height. False for anything
    /// else.
    pub fn set_ground_height(&mut self, h: f64) -> bool {
        match &mut self.form {
            SiteForm::Spot { height, .. } => *height = h,
            SiteForm::Line(l) if l.kind == GroundLineKind::Contour => {
                l.points.iter_mut().for_each(|p| p.z = h)
            }
            _ => return false,
        }
        true
    }

    /// Which earthwork it is; none for anything else.
    pub fn work_kind(&self) -> Option<WorkKind> {
        match &self.form {
            SiteForm::Pad { path, .. } if !path.is_empty() => Some(WorkKind::Trench),
            SiteForm::Pad { depth, .. } if *depth > 0.0 => Some(WorkKind::Pit),
            SiteForm::Pad { .. } => Some(WorkKind::Pad),
            _ => None,
        }
    }

    /// An earthwork as the ground engine works it in, its level above
    /// its own element's level: a pad at the underside of its base
    /// course. None for anything else.
    pub fn as_work(&self) -> Option<Pad> {
        let SiteForm::Pad {
            boundary,
            top,
            base,
            cut,
            fill,
            depth,
            only_cut,
            retained,
            ..
        } = &self.form
        else {
            return None;
        };
        Some(Pad {
            outline: boundary.clone(),
            level: top - base,
            cut: *cut,
            fill: *fill,
            only_cut: *only_cut,
            depth: *depth,
            heights: vec![],
            sunk: 0.0,
            upright: retained.clone(),
            behind: vec![],
        })
    }

    /// A corner was put in after corner `i` of an earthwork's outline:
    /// its retained edges keep their places, and a retained edge that
    /// was split is retained both sides of the new corner.
    pub fn corner_added(&mut self, i: usize) {
        if let SiteForm::Area {
            parking: Some(cp), ..
        } = &mut self.form
        {
            cp.corner_added(i);
        }
        if let SiteForm::Pad {
            retained, walls, ..
        } = &mut self.form
        {
            for w in walls.iter_mut().filter(|w| w.0 > i) {
                w.0 += 1;
            }
            let split = retained.contains(&i);
            for e in retained.iter_mut().filter(|e| **e > i) {
                *e += 1;
            }
            if split {
                retained.push(i + 1);
            }
            retained.sort_unstable();
        }
    }

    /// Corner `i` of an earthwork's outline was taken away: the edge
    /// that ended there and the one that began there are one edge now,
    /// retained if the one that began there was.
    pub fn corner_removed(&mut self, i: usize) {
        if let SiteForm::Area {
            parking: Some(cp), ..
        } = &mut self.form
        {
            cp.corner_removed(i);
        }
        if let SiteForm::Pad {
            retained,
            boundary,
            walls,
            ..
        } = &mut self.form
        {
            // `boundary` has lost the corner already: it had one more
            let n = boundary.len() + 1;
            let before = (i + n - 1) % n;
            // where an edge of the old outline is in the new one; the two
            // that met at the corner are one now, where `before` was
            let now = |e: usize| -> usize {
                let e = if e == i { before } else { e };
                if e > i {
                    e - 1
                } else {
                    e
                }
            };
            // a wall on either edge that met there stands where it was,
            // but is no longer this work's
            walls.retain(|w| w.0 != before && w.0 != i);
            for w in walls.iter_mut() {
                w.0 = now(w.0);
            }
            // the merged edge is retained if the one that began there was
            let merged = retained.contains(&i);
            retained.retain(|e| *e != before && *e != i);
            for e in retained.iter_mut() {
                *e = now(*e);
            }
            if merged {
                retained.push(now(before));
            }
            retained.retain(|e| *e < boundary.len());
            retained.sort_unstable();
            retained.dedup();
        }
    }

    /// Lay a trench's outline along its line again, after either or its
    /// width changed.
    pub fn retrench(&mut self) {
        if let SiteForm::Pad {
            boundary,
            path,
            width,
            bend,
            ..
        } = &mut self.form
        {
            if path.len() >= 2 && *width > 0.0 {
                *boundary = trench_round(path, *bend, *width);
                ccw(boundary);
            }
        }
    }

    /// A property over `boundary`, with the default rules.
    pub fn property(level: ElementId, mut boundary: Vec<DVec2>) -> Self {
        let mut edges = vec![];
        crate::lot::make_ccw(&mut boundary, &mut edges, &mut [], &mut []);
        Self {
            level,
            form: SiteForm::Property {
                boundary,
                edges,
                rules: crate::lot::PlanningRules::default(),
            },
            material: None,
        }
    }

    pub fn is_property(&self) -> bool {
        matches!(self.form, SiteForm::Property { .. })
    }

    /// A road along `path`, `width` wide.
    pub fn road(level: ElementId, path: Vec<DVec2>, width: f64) -> Self {
        Self {
            level,
            form: SiteForm::Road(crate::road::Road::new(path, width)),
            material: None,
        }
    }

    /// A drain along `path`, carrying `service`.
    pub fn drain(level: ElementId, path: Vec<DVec2>, service: crate::services::DrainUse) -> Self {
        Self {
            level,
            form: SiteForm::Drain(crate::services::Drain::new(path, service)),
            material: None,
        }
    }

    /// A tank of `service` at `centre`.
    pub fn tank(level: ElementId, centre: DVec2, service: crate::services::TankUse) -> Self {
        Self {
            level,
            form: SiteForm::Tank(crate::services::Tank::new(centre, service)),
            material: None,
        }
    }

    /// A path of `surface` along `path`.
    pub fn walk(level: ElementId, path: Vec<DVec2>, surface: crate::walk::Surface) -> Self {
        Self {
            level,
            form: SiteForm::Walk(crate::walk::Walk::new(path, surface)),
            material: None,
        }
    }

    /// A drain or a tank: a service under the ground.
    pub fn is_service(&self) -> bool {
        matches!(self.form, SiteForm::Drain(_) | SiteForm::Tank(_))
    }

    pub fn as_road(&self) -> Option<&crate::road::Road> {
        match &self.form {
            SiteForm::Road(r) => Some(r),
            _ => None,
        }
    }

    /// A plant at `position`.
    pub fn plant(
        level: ElementId,
        position: DVec2,
        form: PlantForm,
        canopy: f64,
        height: f64,
    ) -> Self {
        Self {
            level,
            form: SiteForm::Plant {
                position,
                base: 0.0,
                form,
                canopy,
                height,
                tree: None,
            },
            material: None,
        }
    }

    /// A tree grown from `shape`, `canopy` across and `height` tall.
    pub fn tree(
        level: ElementId,
        position: DVec2,
        shape: TreeShape,
        canopy: f64,
        height: f64,
    ) -> Self {
        let mut s = Self::plant(level, position, PlantForm::Tree, canopy, height);
        if let SiteForm::Plant { tree, .. } = &mut s.form {
            *tree = Some(Box::new(shape));
        }
        s
    }

    /// The shape it is grown from: a tree's that has one.
    pub fn tree_shape(&self) -> Option<&TreeShape> {
        match &self.form {
            SiteForm::Plant {
                form: PlantForm::Tree,
                tree: Some(t),
                ..
            } => Some(t),
            _ => None,
        }
    }

    /// "Terrain", "Hard landscape", "Tree"...: what it is called by default.
    pub fn label(&self) -> &'static str {
        match &self.form {
            SiteForm::Terrain { .. } => "Terrain",
            SiteForm::Area {
                parking: Some(_), ..
            } => "Car park",
            SiteForm::Area { .. } => "Hard landscape",
            SiteForm::Pad { .. } => match self.work_kind() {
                Some(WorkKind::Pit) => "Pit",
                Some(WorkKind::Trench) => "Trench",
                _ => "Pad",
            },
            SiteForm::Plant { form, .. } => form.label(),
            SiteForm::Property { .. } => "Property",
            SiteForm::Road(_) => "Road",
            SiteForm::Drain(d) => match d.service {
                crate::services::DrainUse::Foul => "Foul drain",
                crate::services::DrainUse::Surface => "Surface water drain",
                crate::services::DrainUse::Combined => "Combined drain",
                crate::services::DrainUse::Field => "Drainage field",
            },
            SiteForm::Tank(t) => t.service.label(),
            SiteForm::Walk(_) => "Path",
            SiteForm::Investigation(p) => p.kind.label(),
            SiteForm::Spot { .. } => "Spot level",
            SiteForm::Line(l) => l.kind.label(),
        }
    }

    pub fn is_plant(&self) -> bool {
        matches!(self.form, SiteForm::Plant { .. })
    }

    /// Ground and areas can be cut by voids (a pool, a light well);
    /// plants can't.
    pub fn cuttable(&self) -> bool {
        !self.is_plant()
            && !self.is_property()
            && self.as_road().is_none()
            && !self.is_service()
            && !matches!(self.form, SiteForm::Walk(_) | SiteForm::Investigation(_))
            && !self.is_ground_data()
    }

    /// A borehole, trial pit or probe of the soils report.
    pub fn as_probe(&self) -> Option<&crate::investigation::Probe> {
        match &self.form {
            SiteForm::Investigation(p) => Some(p),
            _ => None,
        }
    }

    /// A borehole, trial pit or probe of `kind` at `position`.
    pub fn probe(level: ElementId, position: DVec2, kind: crate::investigation::ProbeKind) -> Self {
        Self {
            level,
            form: SiteForm::Investigation(crate::investigation::Probe::new(position, kind)),
            material: None,
        }
    }

    /// The outline of a terrain or an area.
    /// Every surveyed height of a terrain: its spot heights and the
    /// points of its lines.
    pub fn heights(&self) -> impl Iterator<Item = DVec3> + '_ {
        let (points, lines): (&[DVec3], &[GroundLine]) = match &self.form {
            SiteForm::Terrain { points, data, .. } => (points, &data.lines),
            _ => (&[], &[]),
        };
        points
            .iter()
            .copied()
            .chain(lines.iter().flat_map(|l| l.points.iter().copied()))
    }

    pub fn boundary(&self) -> Option<&[DVec2]> {
        match &self.form {
            SiteForm::Terrain { boundary, .. }
            | SiteForm::Area { boundary, .. }
            | SiteForm::Pad { boundary, .. }
            | SiteForm::Property { boundary, .. } => Some(boundary),
            SiteForm::Plant { .. }
            | SiteForm::Road(_)
            | SiteForm::Drain(_)
            | SiteForm::Tank(_)
            | SiteForm::Walk(_)
            | SiteForm::Spot { .. }
            | SiteForm::Line(_)
            | SiteForm::Investigation(_) => None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let positive = |v: f64, what: &str| {
            if v.is_finite() && v > 0.0 {
                Ok(())
            } else {
                Err(format!("{what} must be positive, got {v}"))
            }
        };
        let outline = |b: &[DVec2]| {
            if b.len() >= 3 && b.iter().all(|p| p.is_finite()) {
                Ok(())
            } else {
                Err("a site outline needs three finite corners".to_string())
            }
        };
        match &self.form {
            SiteForm::Terrain {
                boundary,
                top,
                thickness,
                points,
                contour,
                data,
            } => {
                outline(boundary)?;
                positive(*thickness, "terrain thickness")?;
                if !top.is_finite() || points.iter().any(|p| !p.is_finite()) {
                    return Err("terrain heights must be finite".into());
                }
                for l in &data.lines {
                    if l.points.len() < 2 || l.points.iter().any(|p| !p.is_finite()) {
                        return Err("a ground line needs two finite points".into());
                    }
                }
                for pad in &data.pads {
                    if pad.outline.len() < 3
                        || pad.outline.iter().any(|p| !p.is_finite())
                        || !pad.level.is_finite()
                        || !(pad.cut.is_finite() && pad.cut >= 0.0)
                        || !(pad.fill.is_finite() && pad.fill >= 0.0)
                        || !(pad.depth.is_finite() && pad.depth >= 0.0)
                    {
                        return Err(
                            "a pad needs an outline of three points, a level and banks that are not negative"
                                .into(),
                        );
                    }
                }
                if data
                    .strata
                    .iter()
                    .any(|l| !(l.thickness.is_finite() && l.thickness > 0.0))
                {
                    return Err("a soil layer needs a thickness above zero".into());
                }
                if !(contour.is_finite() && *contour >= 0.0) {
                    return Err("the contour interval can't be negative".into());
                }
                Ok(())
            }
            SiteForm::Area {
                boundary,
                holes,
                top,
                thickness,
                pattern,
                parking,
            } => {
                outline(boundary)?;
                holes.iter().try_for_each(|h| outline(h))?;
                positive(*thickness, "area thickness")?;
                if !top.is_finite() {
                    return Err("an area's top must be finite".into());
                }
                if let Some(cp) = parking {
                    cp.validate()?;
                }
                pattern.validate()
            }
            SiteForm::Pad {
                boundary,
                top,
                base,
                cut,
                fill,
                depth,
                path,
                width,
                ..
            } => {
                outline(boundary)?;
                if !top.is_finite() {
                    return Err("a pad's level must be finite".into());
                }
                if !(base.is_finite() && *base >= 0.0) {
                    return Err("a pad's base course can't be negative".into());
                }
                if !(depth.is_finite() && *depth >= 0.0) {
                    return Err("a pit's depth can't be negative".into());
                }
                for (v, what) in [
                    (cut, "a bank where it is dug"),
                    (fill, "a bank where it is made up"),
                ] {
                    if !(v.is_finite() && *v >= 0.0) {
                        return Err(format!("{what} can't be negative"));
                    }
                }
                if !path.is_empty() {
                    if path.len() < 2 || path.iter().any(|p| !p.is_finite()) {
                        return Err("a trench needs a line of two points or more".into());
                    }
                    positive(*width, "trench width")?;
                }
                Ok(())
            }
            SiteForm::Plant {
                position,
                base,
                canopy,
                height,
                tree,
                ..
            } => {
                if !(position.is_finite() && base.is_finite()) {
                    return Err("a plant's position must be finite".into());
                }
                positive(*canopy, "canopy size")?;
                positive(*height, "plant height")?;
                tree.as_ref().map_or(Ok(()), |t| t.validate(*height))
            }
            SiteForm::Property {
                boundary, rules, ..
            } => {
                outline(boundary)?;
                rules.validate()
            }
            SiteForm::Road(r) => r.validate(),
            SiteForm::Drain(d) => d.validate(),
            SiteForm::Tank(t) => t.validate(),
            SiteForm::Walk(w) => w.validate(),
            SiteForm::Investigation(p) => p.validate(),
            SiteForm::Spot { position, height } => {
                if position.is_finite() && height.is_finite() {
                    Ok(())
                } else {
                    Err("a spot level's place and height must be finite".into())
                }
            }
            SiteForm::Line(l) => {
                if l.points.len() < 2 || l.points.iter().any(|p| !p.is_finite()) {
                    return Err("a ground line needs two finite points or more".into());
                }
                Ok(())
            }
        }
    }

    /// The corners of its bounding box, in model coordinates.
    pub fn bounds(&self, model: &Model) -> Option<(DVec3, DVec3)> {
        let z = crate::members::elevation(model, self.level);
        match &self.form {
            SiteForm::Terrain {
                boundary,
                top,
                thickness,
                points,
                data,
                ..
            } => {
                let (lo, hi) = flat_bounds(boundary)?;
                let lines = data.lines.iter().flat_map(|l| l.points.iter());
                let zs = points.iter().chain(lines).map(|p| p.z);
                // a survey's ground needn't pass through its nominal top
                let zs =
                    zs.chain((!data.survey || self.heights().next().is_none()).then_some(*top));
                let (zlo, zhi) = zs.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                    (a.min(v), b.max(v))
                });
                Some((lo.extend(z + zlo - thickness), hi.extend(z + zhi)))
            }
            SiteForm::Area {
                boundary,
                top,
                thickness,
                ..
            } => {
                let (lo, hi) = flat_bounds(boundary)?;
                Some((lo.extend(z + top - thickness), hi.extend(z + top)))
            }
            SiteForm::Pad {
                boundary,
                top,
                base,
                ..
            } => {
                let (lo, hi) = flat_bounds(boundary)?;
                Some((lo.extend(z + top - base), hi.extend(z + top)))
            }
            SiteForm::Plant {
                position,
                base,
                canopy,
                height,
                tree,
                ..
            } => {
                let r = DVec2::splat(match tree {
                    Some(t) => {
                        let reach = t.reach.iter().copied().fold(1.0, f64::max);
                        canopy.max(t.depth) / 2.0 * reach
                    }
                    None => canopy / 2.0,
                });
                Some((
                    (*position - r).extend(z + base),
                    (*position + r).extend(z + base + height),
                ))
            }
            SiteForm::Property { boundary, .. } => {
                let (lo, hi) = flat_bounds(boundary)?;
                Some((lo.extend(z), hi.extend(z)))
            }
            SiteForm::Road(r) => {
                let (ring, zs) = r.outline();
                let (lo, hi) = flat_bounds(&ring)?;
                let (zlo, zhi) = r
                    .path
                    .iter()
                    .map(|p| p.z)
                    .chain(zs)
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                        (a.min(v), b.max(v))
                    });
                Some((lo.extend(z + zlo - r.thickness), hi.extend(z + zhi)))
            }
            // in plan, down to a rough depth: its levels come from the
            // ground it is laid in
            SiteForm::Drain(d) => {
                let (lo, hi) = flat_bounds(&d.path)?;
                let r = DVec2::splat(d.chamber.max(d.trench) / 2.0 + 0.1);
                let deep = d.depth + d.fall / 100.0 * d.length() + d.diameter + d.bed;
                Some(((lo - r).extend(z - deep), (hi + r).extend(z)))
            }
            // on the ground: its height is the ground's, so roughly
            SiteForm::Walk(w) => {
                let (lo, hi) = flat_bounds(&w.outline())?;
                Some((lo.extend(z - w.thickness - 5.0), hi.extend(z + 5.0)))
            }
            SiteForm::Tank(t) => {
                let (lo, hi) = flat_bounds(&t.outline())?;
                Some((
                    lo.extend(z - t.cover - t.depth - 2.0 * t.wall),
                    hi.extend(z - t.cover),
                ))
            }
            SiteForm::Spot { position, height } => {
                let r = DVec2::splat(0.25);
                Some((
                    (*position - r).extend(z + height),
                    (*position + r).extend(z + height),
                ))
            }
            SiteForm::Line(l) => {
                let flat: Vec<DVec2> = l.points.iter().map(|p| p.truncate()).collect();
                let (lo, hi) = flat_bounds(&flat)?;
                let (zlo, zhi) = l
                    .points
                    .iter()
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| {
                        (a.min(p.z), b.max(p.z))
                    });
                Some((lo.extend(z + zlo), hi.extend(z + zhi)))
            }
            // its ground is the report's or the terrain's, so a margin
            SiteForm::Investigation(p) => {
                let top = p.ground.unwrap_or(z);
                let r = DVec2::splat(0.5);
                Some((
                    (p.position - r).extend(top - p.depth - 5.0),
                    (p.position + r).extend(top + 5.0),
                ))
            }
        }
    }

    /// Where it is, for turning and scaling it about itself.
    pub fn pivot(&self, model: &Model) -> Option<DVec3> {
        let (lo, hi) = self.bounds(model)?;
        Some(((lo + hi) / 2.0).truncate().extend(lo.z))
    }
}

fn ccw(pts: &mut [DVec2]) {
    if crate::polygon::signed_area2(pts) < 0.0 {
        pts.reverse();
    }
}

fn flat_bounds(pts: &[DVec2]) -> Option<(DVec2, DVec2)> {
    let first = *pts.first()?;
    Some(
        pts.iter()
            .fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p))),
    )
}

/// A surface through spot heights, inverse distance weighted (power 2);
/// `flat` where there are none.
pub fn surface_height(flat: f64, points: &[DVec3], p: DVec2) -> f64 {
    if points.is_empty() {
        return flat;
    }
    let (mut sum, mut weight) = (0.0, 0.0);
    for q in points {
        let d2 = q.truncate().distance_squared(p);
        if d2 < 1e-12 {
            return q.z;
        }
        let w = 1.0 / d2;
        sum += w * q.z;
        weight += w;
    }
    sum / weight
}

/// Is `inner` inside `outer` and clear of its edges (a hole touching
/// the outline would split the ground)?
fn clear_inside(inner: &[DVec2], outer: &[DVec2]) -> bool {
    let near = |p: DVec2, poly: &[DVec2]| {
        (0..poly.len()).any(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let d = b - a;
            let t = ((p - a).dot(d) / d.length_squared().max(1e-12)).clamp(0.0, 1.0);
            p.distance(a + d * t) < 1e-3
        })
    };
    inner
        .iter()
        .all(|p| crate::polygon::contains(outer, *p) && !near(*p, outer))
        && outer.iter().all(|p| !near(*p, inner))
}

/// The hard landscape areas a flat terrain leaves room for: those on its
/// level lying inside it, whose top is in its depth. Their outlines, as
/// holes in the ground.
pub fn ground_holes(model: &Model, terrain: &Site) -> Vec<Vec<DVec2>> {
    let SiteForm::Terrain {
        boundary,
        top,
        thickness,
        points,
        ..
    } = &terrain.form
    else {
        return vec![];
    };
    if !points.is_empty() || surveyed(model, terrain) {
        return vec![];
    }
    model
        .iter()
        .filter_map(|e| match &e.kind {
            crate::ElementKind::Site(Site {
                level,
                form:
                    SiteForm::Area {
                        boundary: b,
                        top: t,
                        ..
                    },
                ..
            }) if *level == terrain.level
                && *t <= top + 0.1
                && *t > top - thickness
                && clear_inside(b, boundary) =>
            {
                Some(b.clone())
            }
            _ => None,
        })
        .collect()
}

// ------------------------------------------------------- the ground's objects

/// The spot levels and ground lines lying on a terrain, as elements of
/// their own: each with its heights above the terrain's level. A spot
/// level lies on the terrain it is inside; a line on the terrain any of
/// its points is inside.
#[allow(clippy::type_complexity)]
pub fn ground_objects(
    model: &Model,
    terrain: &Site,
) -> (Vec<(ElementId, DVec3)>, Vec<(ElementId, GroundLine)>) {
    let (mut spots, mut lines) = (vec![], vec![]);
    let SiteForm::Terrain { boundary, .. } = &terrain.form else {
        return (spots, lines);
    };
    let mut base = None;
    let mut rise = |level: ElementId| {
        let base = *base.get_or_insert_with(|| crate::members::elevation(model, terrain.level));
        if level == terrain.level {
            0.0
        } else {
            crate::members::elevation(model, level) - base
        }
    };
    for e in model.iter() {
        let crate::ElementKind::Site(s) = &e.kind else {
            continue;
        };
        match &s.form {
            SiteForm::Spot { position, height }
                if crate::polygon::contains(boundary, *position) =>
            {
                spots.push((e.id, position.extend(height + rise(s.level))));
            }
            SiteForm::Line(l)
                if l.points
                    .iter()
                    .any(|p| crate::polygon::contains(boundary, p.truncate())) =>
            {
                let dz = rise(s.level);
                let mut l = l.clone();
                l.points.iter_mut().for_each(|p| p.z += dz);
                lines.push((e.id, l));
            }
            _ => {}
        }
    }
    (spots, lines)
}

/// A terrain with the spot levels and ground lines lying on it taken
/// into its own data: what its ground is worked out from. None when
/// nothing lies on it, and the terrain is whole as it is.
pub fn whole_ground(model: &Model, terrain: &Site) -> Option<Site> {
    let (spots, lines) = ground_objects(model, terrain);
    if spots.is_empty() && lines.is_empty() {
        return None;
    }
    let mut whole = terrain.clone();
    if let SiteForm::Terrain { points, data, .. } = &mut whole.form {
        points.extend(spots.into_iter().map(|s| s.1));
        data.lines.extend(lines.into_iter().map(|l| l.1));
    }
    Some(whole)
}

/// A terrain has heights to shape it: its own, or a spot level or a
/// ground line lying on it. Without any it is a flat slab.
pub fn surveyed(model: &Model, terrain: &Site) -> bool {
    if terrain.heights().next().is_some() {
        return true;
    }
    let (spots, lines) = ground_objects(model, terrain);
    !spots.is_empty() || !lines.is_empty()
}

/// More heights than this in a terrain are a survey's, and stay its data.
pub const GROUND_OBJECTS_MOST: usize = 40;

// ------------------------------------------------------------ earlier pads

/// What turns the works earlier files kept inside a terrain into
/// elements: each pad and pit in a terrain's own list becomes an
/// earthwork element on the terrain's level (a trench kept that way has
/// lost its line, so it comes back as a pit of its outline), and the
/// list is emptied. The ground is the same after.
///
/// A terrain's own spot heights and lines become elements the same way
/// (a spot level each, a contour or a break line each), unless they are
/// a survey's: brought in from a file, or more than
/// [`GROUND_OBJECTS_MOST`] of them. Those stay the terrain's data.
///
/// None when no terrain holds any. Committed on opening, it is one step.
pub fn earlier_pads(model: &Model) -> Option<crate::Transaction> {
    let mut tx = crate::Transaction::new("Convert earlier site data");
    for e in model.iter() {
        let crate::ElementKind::Site(site) = &e.kind else {
            continue;
        };
        let SiteForm::Terrain { data, .. } = &site.form else {
            continue;
        };
        let SiteForm::Terrain { points, .. } = &site.form else {
            continue;
        };
        // a sloping pad has no element to become: a list with one stays
        let pads = !data.pads.is_empty() && !data.pads.iter().any(|p| p.sloping());
        let heights = points.len() + data.lines.len();
        let ground = heights > 0
            && heights <= GROUND_OBJECTS_MOST
            && data.shift == DVec3::ZERO
            && data.lines.iter().all(|l| l.points.len() >= 2);
        if !pads && !ground {
            continue;
        }
        let mut after = e.clone();
        if let crate::ElementKind::Site(Site {
            form: SiteForm::Terrain { points, data, .. },
            ..
        }) = &mut after.kind
        {
            if pads {
                data.pads.clear();
            }
            if ground {
                points.clear();
                data.lines.clear();
            }
        }
        tx = tx.replace(e.clone(), after);
        if ground {
            for p in points {
                let spot = Site::spot(site.level, p.truncate(), p.z);
                tx = tx.insert(crate::Element::new(
                    spot.label(),
                    crate::ElementKind::Site(spot),
                ));
            }
            for l in &data.lines {
                let line = Site {
                    level: site.level,
                    form: SiteForm::Line(l.clone()),
                    material: None,
                };
                tx = tx.insert(crate::Element::new(
                    line.label(),
                    crate::ElementKind::Site(line),
                ));
            }
        }
        for w in data.pads.iter().filter(|_| pads) {
            let mut boundary = w.outline.clone();
            ccw(&mut boundary);
            let work = Site {
                level: site.level,
                form: SiteForm::Pad {
                    boundary,
                    top: w.level,
                    base: 0.0,
                    cut: w.cut,
                    fill: w.fill,
                    depth: w.depth,
                    path: vec![],
                    width: 0.0,
                    only_cut: w.only_cut,
                    retained: vec![],
                    walls: vec![],
                    bend: 0.0,
                },
                material: None,
            };
            let name = work.label();
            tx = tx.insert(crate::Element::new(name, crate::ElementKind::Site(work)));
        }
    }
    (!tx.ops.is_empty()).then_some(tx)
}

/// Move, turn or scale a site element; it stays upright.
pub fn transform(site: &mut Site, t: &Xform) -> Option<()> {
    let dz = match t {
        Xform::Move(d) => d.z,
        _ => 0.0,
    };
    let factor = match t {
        Xform::Scale { factor, .. } => Some(*factor),
        Xform::Stretch { .. } if site.is_plant() => return None,
        _ => None,
    };
    match &mut site.form {
        SiteForm::Pad {
            boundary,
            top,
            path,
            retained,
            walls,
            ..
        } => {
            for p in boundary.iter_mut().chain(path.iter_mut()) {
                *p = t.point2(*p);
            }
            // turned round by a mirror, its edges are counted the other way
            if crate::polygon::signed_area2(boundary) < 0.0 {
                let n = boundary.len();
                for e in retained.iter_mut().filter(|e| **e + 1 < n) {
                    *e = n - 2 - *e;
                }
                retained.sort_unstable();
                for w in walls.iter_mut().filter(|w| w.0 + 1 < n) {
                    w.0 = n - 2 - w.0;
                }
            }
            ccw(boundary);
            *top += dz;
        }
        SiteForm::Terrain {
            boundary,
            top,
            points,
            data,
            ..
        } => {
            for p in boundary.iter_mut() {
                *p = t.point2(*p);
            }
            ccw(boundary);
            let lines = data.lines.iter_mut().flat_map(|l| l.points.iter_mut());
            for p in points.iter_mut().chain(lines) {
                *p = t.point2(p.truncate()).extend(p.z + dz);
            }
            for pad in &mut data.pads {
                for p in &mut pad.outline {
                    *p = t.point2(*p);
                }
                pad.level += dz;
                pad.heights.iter_mut().for_each(|z| *z += dz);
            }
            *top += dz;
        }
        SiteForm::Area {
            boundary,
            holes,
            top,
            ..
        } => {
            for p in boundary.iter_mut().chain(holes.iter_mut().flatten()) {
                *p = t.point2(*p);
            }
            ccw(boundary);
            *top += dz;
        }
        SiteForm::Property {
            boundary,
            edges,
            rules,
        } => {
            for p in boundary.iter_mut() {
                *p = t.point2(*p);
            }
            crate::lot::make_ccw(boundary, edges, &mut rules.easements, &mut rules.own);
            // a lot's setbacks are rules, not sizes: a scale leaves them
        }
        SiteForm::Drain(d) => {
            for p in d.path.iter_mut() {
                *p = t.point2(*p);
            }
        }
        SiteForm::Spot { position, height } => {
            *position = t.point2(*position);
            *height += dz;
        }
        SiteForm::Line(l) => {
            for p in l.points.iter_mut() {
                *p = t.point2(p.truncate()).extend(p.z + dz);
            }
        }
        // an investigation point is where it was dug: only moved, and its
        // depths stay as logged
        SiteForm::Investigation(p) => {
            p.position = t.point2(p.position);
            if let Some(g) = p.ground.as_mut() {
                *g += dz;
            }
        }
        SiteForm::Walk(w) => {
            for p in w.path.iter_mut() {
                *p = t.point2(*p);
            }
            if let Some(f) = factor {
                w.width *= f;
            }
        }
        SiteForm::Tank(k) => {
            let ahead = t.point2(k.centre + DVec2::from_angle(k.angle.to_radians()));
            k.centre = t.point2(k.centre);
            k.angle = (ahead - k.centre).to_angle().to_degrees();
            if let Some(f) = factor {
                k.length *= f;
                k.width *= f;
            }
        }
        SiteForm::Road(r) => {
            for p in r.path.iter_mut() {
                *p = t.point2(p.truncate()).extend(p.z + dz);
            }
            if let Some(f) = factor {
                r.width *= f;
                r.footpath *= f;
            }
        }
        SiteForm::Plant {
            position,
            base,
            canopy,
            height,
            tree,
            ..
        } => {
            *position = t.point2(*position);
            *base += dz;
            if let Some(f) = factor {
                *canopy *= f;
                *height *= f;
            }
            if let Some(tr) = tree {
                if let Some(f) = factor {
                    tr.scale(f);
                }
                // its reach turns with it: the side kept off a wall stays so
                if let Xform::Rotate { degrees, .. } = t {
                    tr.angle = (tr.angle + degrees).rem_euclid(360.0);
                }
            }
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_moved_level_tag_is_kept_by_where_it_stands() {
        let t = ElementId::new();
        let mut sv = SiteView::default();
        sv.set_tag_move(t, 7.5, Some(DVec2::new(-0.6, 0.4)));
        // the ground under it shifting a little keeps it moved
        assert_eq!(sv.tag_move(t, 7.62), Some(DVec2::new(-0.6, 0.4)));
        assert_eq!(sv.tag_move(t, 9.0), None);
        // moved again replaces it; none puts it back
        sv.set_tag_move(t, 7.55, Some(DVec2::new(1.0, 0.0)));
        assert_eq!(sv.tags.len(), 1);
        sv.set_tag_move(t, 7.5, None);
        assert!(sv.tags.is_empty());
    }

    /// MU142: a level tag's leader bends as any tag's. Its bend and its
    /// move keep each other; putting it back takes both, straightening
    /// takes the bend only.
    #[test]
    fn a_level_tags_bend_and_move_keep_each_other() {
        let t = ElementId::new();
        let mut sv = SiteView::default();
        sv.set_tag_bend(t, 7.5, Some(DVec2::new(0.3, 0.2)));
        assert_eq!(
            sv.tag(t, 7.5).and_then(|m| m.elbow),
            Some(DVec2::new(0.3, 0.2))
        );
        assert_eq!(sv.tag_move(t, 7.5), Some(DVec2::ZERO));
        sv.set_tag_move(t, 7.5, Some(DVec2::new(-1.0, 0.5)));
        assert_eq!(
            sv.tag(t, 7.5).and_then(|m| m.elbow),
            Some(DVec2::new(0.3, 0.2)),
            "a move keeps the bend"
        );
        sv.set_tag_bend(t, 7.5, None);
        assert_eq!(
            sv.tag_move(t, 7.5),
            Some(DVec2::new(-1.0, 0.5)),
            "straightening keeps the move"
        );
        sv.set_tag_bend(t, 7.5, Some(DVec2::new(0.1, 0.1)));
        sv.set_tag_move(t, 7.5, None);
        assert!(sv.tags.is_empty(), "put back, bend and all");
        // a bend alone, straightened, leaves nothing
        sv.set_tag_bend(t, 7.5, Some(DVec2::new(0.1, 0.1)));
        sv.set_tag_bend(t, 7.5, None);
        assert!(sv.tags.is_empty());
    }

    use super::*;
    use crate::{Element, ElementKind, Level};

    /// A level and a flat 40 m terrain that keeps a pad and a pit in its
    /// own list, as files did before earthworks were elements.
    fn earlier() -> (Model, ElementId) {
        let lv = Element::new(
            "Ground",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let sq =
            |c: f64, h: f64| crate::polygon::rectangle(DVec2::new(c - h, -h), DVec2::new(c + h, h));
        let mut t = Site::terrain(lv.id, sq(0.0, 20.0), 0.0, 1.0);
        if let SiteForm::Terrain { data, .. } = &mut t.form {
            data.pads.push(Pad::new(sq(-8.0, 4.0), 0.85));
            data.pads.push(Pad::dig(sq(8.0, 2.0), 1.2, 0.5));
        }
        let ground = Element::new("Terrain", ElementKind::Site(t));
        let g = ground.id;
        (Model::from_elements([lv, ground]).unwrap(), g)
    }

    fn works(m: &Model) -> Vec<&Site> {
        m.iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Site(s) if s.work_kind().is_some() => Some(s),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn works_kept_in_a_terrain_become_elements() {
        let (m, g) = earlier();
        let mut doc = crate::Document::from_model(m);
        doc.commit(earlier_pads(doc.model()).unwrap()).unwrap();
        let ElementKind::Site(t) = &doc.model().get(g).unwrap().kind else {
            unreachable!()
        };
        let SiteForm::Terrain { data, .. } = &t.form else {
            unreachable!()
        };
        assert!(data.pads.is_empty());
        let w = works(doc.model());
        assert_eq!(w.len(), 2);
        let pad = w
            .iter()
            .find(|s| s.work_kind() == Some(WorkKind::Pad))
            .unwrap();
        let pit = w
            .iter()
            .find(|s| s.work_kind() == Some(WorkKind::Pit))
            .unwrap();
        // each is worked in as it was
        let (a, b) = (pad.as_work().unwrap(), pit.as_work().unwrap());
        assert_eq!((a.level, a.cut, a.fill, a.depth), (0.85, 1.0, 2.0, 0.0));
        assert_eq!((b.cut, b.fill, b.depth), (0.5, 0.0, 1.2));
        assert_eq!(pad.level, t.level);
        // nothing is left to convert, and one undo puts the file back
        assert!(earlier_pads(doc.model()).is_none());
        assert!(doc.undo());
        assert!(works(doc.model()).is_empty());
    }

    #[test]
    fn a_pad_element_from_an_earlier_file_is_an_earthwork_as_it_is() {
        // as saved before pits and trenches: no depth, line or width
        let old = r#"{"level":"00000000-0000-0000-0000-000000000001","form":{"Pad":{"boundary":[[0.0,0.0],[4.0,0.0],[4.0,4.0],[0.0,4.0]],"top":1.0,"base":0.15,"cut":1.5,"fill":2.0}}}"#;
        let s: Site = serde_json::from_str(old).unwrap();
        s.validate().unwrap();
        assert_eq!(s.work_kind(), Some(WorkKind::Pad));
        assert_eq!(s.label(), "Pad");
        // worked in at the underside of its base course
        let w = s.as_work().unwrap();
        assert!((w.level - 0.85).abs() < 1e-12);
        assert_eq!((w.cut, w.fill), (1.5, 2.0));
    }

    #[test]
    fn a_trench_keeps_its_line_and_follows_it() {
        let l = ElementId::new();
        let mut t = Site::trench(l, vec![DVec2::ZERO, DVec2::new(10.0, 0.0)], 0.6, 1.0);
        t.validate().unwrap();
        assert_eq!(t.work_kind(), Some(WorkKind::Trench));
        assert_eq!(t.label(), "Trench");
        let wide = |s: &Site| {
            let b = s.boundary().unwrap();
            b.iter().map(|p| p.y).fold(f64::MIN, f64::max)
                - b.iter().map(|p| p.y).fold(f64::MAX, f64::min)
        };
        assert!((wide(&t) - 0.6).abs() < 1e-9);
        if let SiteForm::Pad { width, .. } = &mut t.form {
            *width = 1.0;
        }
        t.retrench();
        assert!((wide(&t) - 1.0).abs() < 1e-9);
        // moved, its line goes with it
        transform(&mut t, &Xform::Move(DVec3::new(0.0, 5.0, 0.0))).unwrap();
        let SiteForm::Pad { path, .. } = &t.form else {
            unreachable!()
        };
        assert!((path[0].y - 5.0).abs() < 1e-12);
    }

    #[test]
    fn a_terrains_own_heights_become_spot_levels_and_lines() {
        let lv = Element::new(
            "Ground Floor",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let sq = crate::polygon::rectangle(DVec2::splat(-20.0), DVec2::splat(20.0));
        let mut t = Site::terrain(lv.id, sq, 0.0, 1.0);
        if let SiteForm::Terrain { points, data, .. } = &mut t.form {
            points.push(DVec3::new(-5.0, 0.0, 1.0));
            points.push(DVec3::new(5.0, 0.0, 2.0));
            data.lines.push(GroundLine {
                kind: GroundLineKind::Contour,
                points: vec![DVec3::new(-10.0, 10.0, 3.0), DVec3::new(10.0, 10.0, 3.0)],
                closed: false,
            });
        }
        let before = t.clone();
        let ground = Element::new("Terrain", ElementKind::Site(t));
        let gid = ground.id;
        let mut doc = crate::Document::from_model(Model::from_elements([lv, ground]).unwrap());
        doc.commit(earlier_pads(doc.model()).unwrap()).unwrap();
        let m = doc.model();
        let ElementKind::Site(now) = &m.get(gid).unwrap().kind else {
            panic!()
        };
        assert_eq!(
            now.heights().count(),
            0,
            "the terrain keeps none of its own"
        );
        let (spots, lines) = ground_objects(m, now);
        assert_eq!((spots.len(), lines.len()), (2, 1));
        assert!(surveyed(m, now));
        // taken back into its data, it is the terrain it was
        assert_eq!(whole_ground(m, now).unwrap(), before);
        assert!(
            earlier_pads(m).is_none(),
            "and there is nothing left to convert"
        );
        // a spot level and a contour take one height; a move carries them
        let mut spot = Site::spot(now.level, DVec2::new(1.0, 2.0), 0.5);
        assert!(spot.set_ground_height(0.75));
        transform(&mut spot, &Xform::Move(DVec3::new(1.0, 0.0, 0.25))).unwrap();
        assert_eq!(
            spot.form,
            SiteForm::Spot {
                position: DVec2::new(2.0, 2.0),
                height: 1.0
            }
        );
        let mut c = Site::contour(now.level, vec![DVec2::ZERO, DVec2::X], 2.0);
        assert_eq!(c.ground_height(), Some(2.0));
        assert!(c.set_ground_height(2.5));
        assert_eq!(c.ground_height(), Some(2.5));
        assert!(c.validate().is_ok() && !c.cuttable());
    }

    #[test]
    fn retained_edges_follow_corners_added_and_removed() {
        let lv = ElementId::new();
        let pentagon = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 0.0),
            DVec2::new(12.0, 6.0),
            DVec2::new(5.0, 10.0),
            DVec2::new(-2.0, 6.0),
        ];
        let retained_of = |s: &Site| match &s.form {
            SiteForm::Pad { retained, .. } => retained.clone(),
            _ => vec![],
        };
        let with = |edges: &[usize]| {
            let mut s = Site::pad(lv, pentagon.clone(), 0.0);
            if let SiteForm::Pad { retained, .. } = &mut s.form {
                *retained = edges.to_vec();
            }
            s
        };
        let drop = |s: &mut Site, i: usize| {
            if let SiteForm::Pad { boundary, .. } = &mut s.form {
                boundary.remove(i);
            }
            s.corner_removed(i);
        };
        // corner 2 gone: edges 1 and 2 are one, where 1 was, retained as 2 was
        let mut s = with(&[2, 4]);
        drop(&mut s, 2);
        assert_eq!(retained_of(&s), vec![1, 3]);
        let mut s = with(&[1, 4]);
        drop(&mut s, 2);
        assert_eq!(
            retained_of(&s),
            vec![3],
            "the edge that ended there is not kept"
        );
        // corner 0 gone: the closing edge and edge 0 are one, the last now
        let mut s = with(&[0, 2]);
        drop(&mut s, 0);
        assert_eq!(retained_of(&s), vec![1, 3]);
        // a corner put into a retained edge: both halves are retained
        let mut s = with(&[1, 3]);
        if let SiteForm::Pad { boundary, .. } = &mut s.form {
            boundary.insert(2, DVec2::new(11.0, 3.0));
        }
        s.corner_added(1);
        assert_eq!(retained_of(&s), vec![1, 2, 4]);
    }

    #[test]
    fn a_surveys_heights_stay_the_terrains_data() {
        let lv = Element::new(
            "Ground Floor",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let sq = crate::polygon::rectangle(DVec2::splat(-20.0), DVec2::splat(20.0));
        let mut many = Site::terrain(lv.id, sq.clone(), 0.0, 1.0);
        if let SiteForm::Terrain { points, .. } = &mut many.form {
            for i in 0..=GROUND_OBJECTS_MOST {
                points.push(DVec3::new(i as f64 * 0.1 - 10.0, 0.0, 1.0));
            }
        }
        let mut brought = Site::terrain(lv.id, sq, 0.0, 1.0);
        if let SiteForm::Terrain { points, data, .. } = &mut brought.form {
            points.push(DVec3::new(0.0, 5.0, 1.0));
            data.shift = DVec3::new(500000.0, 100000.0, 0.0);
        }
        let m = Model::from_elements([
            lv,
            Element::new("Terrain", ElementKind::Site(many)),
            Element::new("Terrain", ElementKind::Site(brought)),
        ])
        .unwrap();
        assert!(earlier_pads(&m).is_none());
    }

    #[test]
    fn spot_heights_shape_the_ground() {
        let pts = [DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 0.0, 2.0)];
        assert_eq!(surface_height(0.5, &[], DVec2::ZERO), 0.5);
        assert_eq!(surface_height(0.5, &pts, DVec2::new(10.0, 0.0)), 2.0);
        let mid = surface_height(0.5, &pts, DVec2::new(5.0, 0.0));
        assert!((mid - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_terrain_from_before_ground_data_opens_as_it_was() {
        // as saved before lines and surveys: no `data` at all
        let old = r#"{"level":"00000000-0000-0000-0000-000000000001","form":{"Terrain":{
            "boundary":[[0.0,0.0],[10.0,0.0],[10.0,10.0],[0.0,10.0]],"top":-0.15,"thickness":0.5,
            "points":[[5.0,5.0,1.2]],"contour":0.25}}}"#;
        let s: Site = serde_json::from_str(old).unwrap();
        let SiteForm::Terrain {
            points,
            contour,
            data,
            ..
        } = &s.form
        else {
            panic!()
        };
        assert_eq!((points.len(), *contour), (1, 0.25));
        // a drawn terrain: its corners keep its top, it is read smoothly
        assert!(data.is_plain() && !data.survey && !data.faceted);
        assert_eq!(data.major_every(), 5);
        s.validate().unwrap();
        // and saved again it gains nothing
        assert!(!serde_json::to_string(&s).unwrap().contains("data"));
        // lines move and are bounded with the rest
        let mut s = s;
        if let SiteForm::Terrain { data, .. } = &mut s.form {
            data.lines.push(GroundLine {
                kind: GroundLineKind::Break,
                points: vec![DVec3::new(1.0, 1.0, 3.0), DVec3::new(9.0, 1.0, 3.5)],
                closed: false,
            });
        }
        transform(&mut s, &Xform::Move(DVec3::new(100.0, 0.0, 1.0))).unwrap();
        let SiteForm::Terrain { data, .. } = &s.form else {
            panic!()
        };
        assert_eq!(data.lines[0].points[1], DVec3::new(109.0, 1.0, 4.5));
        assert_eq!(s.heights().count(), 3);
        let back: Site = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn paving_sits_in_the_ground() {
        let lv = Element::new(
            "Site",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let l = lv.id;
        let ground = Site::terrain(
            l,
            crate::polygon::rectangle(DVec2::splat(-20.0), DVec2::splat(20.0)),
            0.0,
            0.5,
        );
        let drive = Site::area(
            l,
            crate::polygon::rectangle(DVec2::ZERO, DVec2::new(3.0, 8.0)),
            0.15,
            Pattern::Units {
                size: DVec2::new(0.2, 0.1),
                stagger: true,
                angle: 0.0,
            },
        );
        let outside = Site::area(
            l,
            crate::polygon::rectangle(DVec2::splat(30.0), DVec2::splat(32.0)),
            0.15,
            Pattern::None,
        );
        let m = Model::from_elements([
            lv,
            Element::new("Ground", ElementKind::Site(ground.clone())),
            Element::new("Drive", ElementKind::Site(drive)),
            Element::new("Far", ElementKind::Site(outside)),
        ])
        .unwrap();
        assert_eq!(ground_holes(&m, &ground).len(), 1);
    }

    #[test]
    fn plants_scale_but_terrain_stretches() {
        let mut p = Site::plant(ElementId::new(), DVec2::ZERO, PlantForm::Tree, 4.0, 6.0);
        let stretch = Xform::Stretch {
            center: DVec3::ZERO,
            factor: DVec3::new(2.0, 1.0, 1.0),
        };
        assert!(transform(&mut p, &stretch).is_none());
        transform(
            &mut p,
            &Xform::Scale {
                center: DVec3::ZERO,
                factor: 0.5,
            },
        )
        .unwrap();
        assert!(matches!(p.form, SiteForm::Plant { canopy, .. } if canopy == 2.0));
        let mut t = Site::terrain(
            ElementId::new(),
            crate::polygon::rectangle(DVec2::ZERO, DVec2::ONE),
            0.0,
            0.3,
        );
        assert!(transform(&mut t, &stretch).is_some());
        assert!(t.validate().is_ok());
    }
}
