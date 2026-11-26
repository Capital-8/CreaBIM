//! A generated tree: the shape a broadleaf tree's crown, limbs and plan
//! symbol are grown from (see `creabim_geom::tree`).
//!
//! The shape is data, kept on the tree itself: its crown's depth (its
//! width is the plant's canopy), clear trunk, trunk, crown outline,
//! clumps, branching, how far the crown reaches on each side, and a seed.
//! The same shape and seed always grow the same tree, so the plan, the
//! elevation and the 3D view agree, and a file opens as it was saved.
//!
//! The crown is drawn inside an envelope: an ellipse `canopy` by `depth`
//! in plan, pulled in or pushed out on each of eight sides by `reach`
//! (a tree beside a wall can be kept off it while its trunk stays put),
//! and shaped up its height by the crown outline.

use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

/// How a broadleaf tree's crown is grown.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TreeShape {
    /// The crown across along the tree's own y; its width (along x) is
    /// the plant's canopy.
    pub depth: f64,
    /// The clear trunk: the crown's underside above the tree's base.
    pub crown_base: f64,
    /// The trunk's diameter at the ground.
    pub trunk: f64,
    /// Its outline up its height.
    pub crown: CrownShape,
    /// How many masses of foliage the crown is made of.
    pub clumps: u32,
    /// The masses' size, 0 small to 1 large.
    pub clump_size: f64,
    /// 0 a tidy crown to 1 a ragged one.
    pub irregularity: f64,
    /// 0 a dense crown to 1 an open one, with gaps.
    pub openness: f64,
    pub branching: Branching,
    /// How far the crown reaches on each side, a fraction of its radius
    /// that way: east, north-east, north... counter-clockwise in the
    /// tree's own frame. Between two sides it eases from one to the next.
    pub reach: [f64; 8],
    /// Degrees counter-clockwise: the tree's own frame in the model's.
    pub angle: f64,
    /// How each clump of foliage is built in 3D.
    pub form: ClumpForm,
    /// Solid wood: the trunk and limbs closed, so a section fills their
    /// cut. Off, they are open tubes, as the first trees were.
    pub solid: bool,
    /// How plans draw it.
    pub plan: TreePlan,
    /// Plans mark the trunk and the crown's centre.
    pub plan_trunk: bool,
    /// Plans give the crown a light tint.
    pub plan_tint: bool,
    /// Plans draw the crown's outline heavy and what is inside it light.
    pub plan_bold: bool,
    /// How sections and elevations draw the crown.
    pub elevation: TreeElevation,
    /// Sections and elevations draw the main limbs through the crown.
    pub elevation_limbs: bool,
    /// When it is in leaf.
    pub leaves: LeafHabit,
    /// The month 3D shows it in, 1 (January) to 12. South of the equator
    /// its year runs six months on.
    pub month: u8,
    /// Its leaves in full leaf (sRGB, 0 to 1).
    pub leaf_colour: [f32; 3],
    /// Its leaves as they turn, before they fall (sRGB, 0 to 1).
    pub autumn_colour: [f32; 3],
    /// Its own month, kept when the project's month changes; off, it
    /// follows the project's (see [`crate::ProjectInfo::month`]).
    pub own_month: bool,
    /// How finely 3D builds it.
    pub detail: TreeDetail,
    /// A low-poly crown's facets, 1 (a few big faces) to 5.
    pub facets: u8,
    /// How a section draws the crown where it cuts it.
    pub cut: TreeCut,
    /// Which tree of all those with this shape: another seed grows
    /// another, as alike as two trees of a kind.
    pub seed: u32,
}

impl Default for TreeShape {
    fn default() -> Self {
        Self {
            depth: 3.7,
            crown_base: 2.2,
            trunk: 0.3,
            crown: CrownShape::Rounded,
            clumps: 12,
            clump_size: 0.5,
            irregularity: 0.4,
            openness: 0.2,
            branching: Branching::Balanced,
            reach: [1.0; 8],
            angle: 0.0,
            form: ClumpForm::Noise,
            solid: true,
            plan: TreePlan::Scalloped,
            plan_trunk: true,
            plan_tint: false,
            plan_bold: false,
            elevation: TreeElevation::Clumps,
            elevation_limbs: false,
            leaves: LeafHabit::Evergreen,
            month: 7,
            leaf_colour: LEAF_GREEN,
            autumn_colour: AUTUMN_ORANGE,
            own_month: false,
            detail: TreeDetail::Everyday,
            facets: 2,
            cut: TreeCut::Tinted,
            seed: 1,
        }
    }
}

/// A crown's outline up its height.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrownShape {
    /// Widest at the middle: a shade tree, an oak, a lime.
    #[default]
    Rounded,
    /// Wide and flat-topped: an umbrella, an acacia, a flame tree.
    Spreading,
    /// Narrow and tall: a columnar hornbeam, a cypress-like form.
    Upright,
    /// Widest low, to a point: a young conifer-like broadleaf.
    Conical,
    /// Narrow low, widest high: an elm, a zelkova.
    Vase,
}

impl CrownShape {
    pub const ALL: [CrownShape; 5] = [
        CrownShape::Rounded,
        CrownShape::Spreading,
        CrownShape::Upright,
        CrownShape::Conical,
        CrownShape::Vase,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CrownShape::Rounded => "Rounded",
            CrownShape::Spreading => "Spreading",
            CrownShape::Upright => "Upright",
            CrownShape::Conical => "Conical",
            CrownShape::Vase => "Vase",
        }
    }

    /// The crown's radius at `t` up it (0 its underside, 1 its top), a
    /// fraction of its widest.
    pub fn profile(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        let s = |x: f64| (std::f64::consts::PI * x).sin().max(0.0);
        match self {
            CrownShape::Rounded => s(t).powf(0.55),
            CrownShape::Spreading => s(t.powf(0.55)).powf(0.4) * (0.55 + 0.45 * t),
            CrownShape::Upright => s(t).powf(0.85),
            CrownShape::Conical => ((1.0 - t) * 1.1).min(1.0) * (t * 4.0 + 0.3).min(1.0),
            CrownShape::Vase => (0.3 + 0.7 * t) * s(0.08 + 0.84 * t).powf(0.35),
        }
    }

    /// A trunk that runs up through the crown (a leader) rather than
    /// splitting into limbs low in it.
    pub fn leader(self) -> bool {
        matches!(self, CrownShape::Upright | CrownShape::Conical)
    }
}

/// How a tree's limbs divide.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Branching {
    /// Three heavy limbs.
    Few,
    #[default]
    Balanced,
    /// Eight finer limbs.
    Many,
}

impl Branching {
    pub const ALL: [Branching; 3] = [Branching::Few, Branching::Balanced, Branching::Many];

    pub fn label(self) -> &'static str {
        match self {
            Branching::Few => "Few heavy limbs",
            Branching::Balanced => "Balanced",
            Branching::Many => "Many fine limbs",
        }
    }

    pub fn limbs(self) -> usize {
        match self {
            Branching::Few => 3,
            Branching::Balanced => 5,
            Branching::Many => 8,
        }
    }
}

/// How plans draw a generated tree: always its crown's real outline seen
/// from above, simplified so it reads at drawing scales.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreePlan {
    /// The outline as a row of scallops, with the trunk.
    #[default]
    Scalloped,
    /// The outline as one smooth line, with the trunk.
    Outline,
    /// Scallops, with the main limbs drawn from the trunk.
    Limbs,
    /// The outline, with each clump of the crown drawn inside it.
    Clumps,
    /// A finely toothed rim, with lines running out from the trunk.
    Serrated,
    /// A wavy rim, with small leaf masses inside.
    Wavy,
    /// Every limb and twig, the rim dotted.
    Skeleton,
    /// The rim hatched, with the main limbs.
    Hatched,
    /// The crown as polygonal cells, one a clump.
    Cells,
    /// Radial wedges, dotted.
    Wedges,
    /// Rays out from the trunk, ending in dots.
    Starburst,
    /// A faceted rim, with leaves scattered inside.
    Leafy,
    /// A jagged rim, with limbs running out.
    Jagged,
}

impl TreePlan {
    pub const ALL: [TreePlan; 13] = [
        TreePlan::Scalloped,
        TreePlan::Outline,
        TreePlan::Limbs,
        TreePlan::Clumps,
        TreePlan::Serrated,
        TreePlan::Wavy,
        TreePlan::Skeleton,
        TreePlan::Hatched,
        TreePlan::Cells,
        TreePlan::Wedges,
        TreePlan::Starburst,
        TreePlan::Leafy,
        TreePlan::Jagged,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TreePlan::Scalloped => "Scalloped",
            TreePlan::Outline => "Outline",
            TreePlan::Limbs => "Limbs",
            TreePlan::Clumps => "Clumps",
            TreePlan::Serrated => "Serrated",
            TreePlan::Wavy => "Wavy",
            TreePlan::Skeleton => "Skeleton",
            TreePlan::Hatched => "Hatched rim",
            TreePlan::Cells => "Cells",
            TreePlan::Wedges => "Wedges",
            TreePlan::Starburst => "Starburst",
            TreePlan::Leafy => "Leafy",
            TreePlan::Jagged => "Jagged",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            TreePlan::Scalloped => "Its crown's outline as scallops",
            TreePlan::Outline => "Its crown's outline as one smooth line",
            TreePlan::Limbs => "Scallops, with its main limbs from the trunk",
            TreePlan::Clumps => "Its outline, with each clump of the crown inside it",
            TreePlan::Serrated => "A finely toothed rim, with lines out from the trunk",
            TreePlan::Wavy => "A soft wavy rim, with small leaf masses inside",
            TreePlan::Skeleton => "Its limbs and twigs, inside a dotted rim: a tree in winter",
            TreePlan::Hatched => "A hatched band round the rim, with its main limbs",
            TreePlan::Cells => "Each clump a polygonal cell inside the outline",
            TreePlan::Wedges => "The crown split into wedges from the trunk, dotted",
            TreePlan::Starburst => "Rays out from the trunk, ending in dots",
            TreePlan::Leafy => "A faceted rim, with leaves scattered inside",
            TreePlan::Jagged => "A jagged rim, with limbs running out to it",
        }
    }
}

/// How sections and elevations draw a generated tree's crown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreeElevation {
    /// Each clump's outline, and where the section cuts them.
    #[default]
    Clumps,
    /// One smooth outline round the whole crown, cut as one: it reads as
    /// the plan does.
    Outline,
    /// A soft wavy outline, with gaps between the leaves inside.
    Wavy,
    /// A scalloped outline, with the clumps inside it drawn light.
    Scalloped,
    /// A faceted outline, with leaves scattered inside.
    Leafy,
}

impl TreeElevation {
    pub const ALL: [TreeElevation; 5] = [
        TreeElevation::Clumps,
        TreeElevation::Outline,
        TreeElevation::Wavy,
        TreeElevation::Scalloped,
        TreeElevation::Leafy,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TreeElevation::Clumps => "Clumps",
            TreeElevation::Outline => "Outline",
            TreeElevation::Wavy => "Wavy",
            TreeElevation::Scalloped => "Scalloped",
            TreeElevation::Leafy => "Leafy",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            TreeElevation::Clumps => "Each clump's outline, and where the section cuts them",
            TreeElevation::Outline => "One smooth outline round the whole crown",
            TreeElevation::Wavy => "A soft wavy outline, with gaps between the leaves inside",
            TreeElevation::Scalloped => "A scalloped outline, with its clumps drawn light inside",
            TreeElevation::Leafy => "A faceted outline, with leaves scattered inside",
        }
    }

    /// Drawn as linework of its own, as plans are, rather than from the
    /// crown's 3D outline.
    pub fn drawn(self) -> bool {
        !matches!(self, TreeElevation::Clumps | TreeElevation::Outline)
    }
}

/// A tree's leaves in full leaf, by default (sRGB).
pub const LEAF_GREEN: [f32; 3] = [0.36, 0.53, 0.29];
/// A tree's leaves as they turn, by default (sRGB).
pub const AUTUMN_ORANGE: [f32; 3] = [0.80, 0.46, 0.16];
/// New leaves, lighter and yellower than full leaf.
const FRESH: [f32; 3] = [0.62, 0.78, 0.33];

/// When a tree is in leaf.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeafHabit {
    /// In leaf all year.
    #[default]
    Evergreen,
    /// Bare in winter, new leaves in spring, turning in autumn: a
    /// temperate oak, a maple, a lime.
    Deciduous,
    /// Losing its leaves in the dry season: a flame tree, a teak, a
    /// frangipani.
    DrySeason,
}

impl LeafHabit {
    pub const ALL: [LeafHabit; 3] = [
        LeafHabit::Evergreen,
        LeafHabit::Deciduous,
        LeafHabit::DrySeason,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LeafHabit::Evergreen => "Evergreen",
            LeafHabit::Deciduous => "Deciduous, temperate",
            LeafHabit::DrySeason => "Dry-season, tropical",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            LeafHabit::Evergreen => "In leaf all year",
            LeafHabit::Deciduous => "Bare in winter, fresh in spring, turning in autumn",
            LeafHabit::DrySeason => "Loses its leaves in the dry season, flushes with the rains",
        }
    }

    /// Its leaves in `month` (1 to 12, the northern year).
    pub fn state(self, month: u8) -> LeafState {
        // (in leaf, turned, fresh) a month
        const DECIDUOUS: [(f64, f64, f64); 12] = [
            (0.0, 1.0, 0.0),
            (0.0, 1.0, 0.0),
            (0.1, 0.0, 1.0),
            (0.5, 0.0, 1.0),
            (0.9, 0.0, 0.5),
            (1.0, 0.0, 0.0),
            (1.0, 0.0, 0.0),
            (1.0, 0.1, 0.0),
            (1.0, 0.4, 0.0),
            (0.85, 1.0, 0.0),
            (0.35, 1.0, 0.0),
            (0.0, 1.0, 0.0),
        ];
        const DRY: [(f64, f64, f64); 12] = [
            (0.55, 0.7, 0.0),
            (0.2, 1.0, 0.0),
            (0.1, 1.0, 0.0),
            (0.45, 0.0, 1.0),
            (0.9, 0.0, 0.6),
            (1.0, 0.0, 0.0),
            (1.0, 0.0, 0.0),
            (1.0, 0.0, 0.0),
            (1.0, 0.0, 0.0),
            (1.0, 0.0, 0.0),
            (1.0, 0.15, 0.0),
            (0.85, 0.4, 0.0),
        ];
        let i = (month.clamp(1, 12) - 1) as usize;
        let (leaf, turned, fresh) = match self {
            LeafHabit::Evergreen => (1.0, 0.0, if i == 4 { 0.25 } else { 0.0 }),
            LeafHabit::Deciduous => DECIDUOUS[i],
            LeafHabit::DrySeason => DRY[i],
        };
        LeafState {
            leaf,
            turned,
            fresh,
        }
    }
}

/// A tree's leaves in a month.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeafState {
    /// How much of the crown is in leaf: 0 bare to 1 full.
    pub leaf: f64,
    /// 0 its full-leaf colour to 1 its autumn colour.
    pub turned: f64,
    /// 0 its full-leaf colour to 1 new leaves' lighter green.
    pub fresh: f64,
}

/// How a section draws a crown where it cuts through it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreeCut {
    /// The cut's outline, heavy, over a light green wash.
    #[default]
    Tinted,
    /// The cut's outline only.
    Outline,
    /// The cut's outline over a grey fill, as solid material is.
    Filled,
}

impl TreeCut {
    pub const ALL: [TreeCut; 3] = [TreeCut::Tinted, TreeCut::Outline, TreeCut::Filled];

    pub fn label(self) -> &'static str {
        match self {
            TreeCut::Tinted => "Tinted",
            TreeCut::Outline => "Outline",
            TreeCut::Filled => "Filled",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            TreeCut::Tinted => "The cut's outline, heavy, over a light green wash",
            TreeCut::Outline => "The cut's outline only",
            TreeCut::Filled => "The cut's outline over a grey fill, as solid material is",
        }
    }
}

/// How finely 3D builds a generated tree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreeDetail {
    /// Few faces and only the main limbs: for a site of hundreds.
    Lite,
    #[default]
    Everyday,
    /// Finer clumps and limbs, with twigs: for close views and renders.
    Presentation,
}

impl TreeDetail {
    pub const ALL: [TreeDetail; 3] = [
        TreeDetail::Lite,
        TreeDetail::Everyday,
        TreeDetail::Presentation,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TreeDetail::Lite => "Lite",
            TreeDetail::Everyday => "Everyday",
            TreeDetail::Presentation => "Presentation",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            TreeDetail::Lite => "Few faces and only the main limbs: for a site of hundreds",
            TreeDetail::Everyday => "Balanced: what most views need",
            TreeDetail::Presentation => {
                "Finer clumps and limbs, with twigs: for close views and renders"
            }
        }
    }
}

/// The months' initials, January first.
pub const MONTHS: [&str; 12] = ["J", "F", "M", "A", "M", "J", "J", "A", "S", "O", "N", "D"];
/// The months' names, January first.
pub const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// How a clump of foliage is built: from a few flat faces to one smooth
/// body for the whole crown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClumpForm {
    /// An icosahedron: twenty flat faces.
    Polyhedra,
    /// Points scattered at random over the clump, wrapped.
    Random,
    /// Points spread evenly, then nudged: random but never bunched.
    PseudoRandom,
    /// Points on a Fibonacci spiral, wrapped: even, without rows.
    Fibonacci,
    /// A geodesic sphere, its faces flat.
    Geodesic,
    /// A smooth sphere ruffled by noise.
    #[default]
    Noise,
    /// The clumps fused into one smooth body.
    Metaballs,
    /// The whole crown as one faceted body: a low-poly tree, as few
    /// faces as its facets setting asks.
    LowPoly,
}

impl ClumpForm {
    pub const ALL: [ClumpForm; 8] = [
        ClumpForm::Polyhedra,
        ClumpForm::Random,
        ClumpForm::PseudoRandom,
        ClumpForm::Fibonacci,
        ClumpForm::Geodesic,
        ClumpForm::Noise,
        ClumpForm::Metaballs,
        ClumpForm::LowPoly,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ClumpForm::Polyhedra => "Polyhedra",
            ClumpForm::Random => "Fully random",
            ClumpForm::PseudoRandom => "Pseudo-random",
            ClumpForm::Fibonacci => "Fibonacci",
            ClumpForm::Geodesic => "Geodesic",
            ClumpForm::Noise => "Noise",
            ClumpForm::Metaballs => "Metaballs",
            ClumpForm::LowPoly => "Low poly",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            ClumpForm::Polyhedra => "Twenty flat faces a clump: the lightest",
            ClumpForm::Random => "Facets of every size, at random",
            ClumpForm::PseudoRandom => "Irregular facets of about one size",
            ClumpForm::Fibonacci => "Even facets without rows",
            ClumpForm::Geodesic => "A faceted geodesic sphere",
            ClumpForm::Noise => "Smooth and ruffled",
            ClumpForm::Metaballs => "The clumps fused into one smooth crown",
            ClumpForm::LowPoly => "One faceted crown on simple limbs: a low-poly tree",
        }
    }
}

/// Starting points for a generated tree: its shape, canopy and height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreePreset {
    Shade,
    Umbrella,
    Columnar,
    Conical,
    Vase,
}

impl TreePreset {
    pub const ALL: [TreePreset; 5] = [
        TreePreset::Shade,
        TreePreset::Umbrella,
        TreePreset::Columnar,
        TreePreset::Conical,
        TreePreset::Vase,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TreePreset::Shade => "Shade tree",
            TreePreset::Umbrella => "Umbrella",
            TreePreset::Columnar => "Columnar",
            TreePreset::Conical => "Conical",
            TreePreset::Vase => "Vase",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            TreePreset::Shade => "A round crown on a clear trunk: an oak, a lime, a mango",
            TreePreset::Umbrella => "Wide and flat-topped: an acacia, a flame tree, a rain tree",
            TreePreset::Columnar => "Narrow and tall, for tight spaces and screens",
            TreePreset::Conical => "Widest low, to a point",
            TreePreset::Vase => "Narrow at the base, widest high: an elm, a zelkova",
        }
    }

    /// (shape, canopy, height); the seed and the sides' reach are the
    /// caller's to keep.
    pub fn shape(self) -> (TreeShape, f64, f64) {
        let base = TreeShape::default();
        let s = |depth,
                 crown_base,
                 trunk,
                 crown,
                 clumps,
                 clump_size,
                 irregularity,
                 openness,
                 branching| TreeShape {
            depth,
            crown_base,
            trunk,
            crown,
            clumps,
            clump_size,
            irregularity,
            openness,
            branching,
            ..base.clone()
        };
        use Branching as B;
        use CrownShape as C;
        match self {
            TreePreset::Shade => (
                s(6.6, 2.6, 0.38, C::Rounded, 12, 0.5, 0.4, 0.2, B::Balanced),
                7.2,
                9.0,
            ),
            TreePreset::Umbrella => (
                s(10.0, 3.6, 0.45, C::Spreading, 16, 0.45, 0.5, 0.3, B::Few),
                11.0,
                8.0,
            ),
            TreePreset::Columnar => (
                s(3.4, 1.4, 0.3, C::Upright, 14, 0.55, 0.25, 0.1, B::Many),
                3.6,
                12.0,
            ),
            TreePreset::Conical => (
                s(5.0, 1.0, 0.32, C::Conical, 16, 0.4, 0.2, 0.05, B::Many),
                5.0,
                11.0,
            ),
            TreePreset::Vase => (
                s(8.0, 3.2, 0.4, C::Vase, 14, 0.45, 0.45, 0.25, B::Few),
                8.5,
                10.0,
            ),
        }
    }
}

/// The eight sides' directions, in the tree's own frame.
pub const SIDES: [&str; 8] = [
    "east",
    "north-east",
    "north",
    "north-west",
    "west",
    "south-west",
    "south",
    "south-east",
];

impl TreeShape {
    /// A shape `canopy` across with a seed of its own, from where it
    /// stands, so a row of new trees are not clones.
    pub fn new_at(canopy: f64, at: DVec2) -> Self {
        Self {
            depth: canopy * 0.92,
            seed: seed_at(at),
            ..Self::default()
        }
    }

    /// The direction of side `i` in the model, a unit vector.
    pub fn side(&self, i: usize) -> DVec2 {
        DVec2::from_angle(self.angle.to_radians() + TAU * i as f64 / 8.0)
    }

    /// The reach at `a` radians in the tree's own frame, eased between
    /// the sides either side of it.
    pub fn reach_local(&self, a: f64) -> f64 {
        let f = a.rem_euclid(TAU) / (TAU / 8.0);
        let i = f.floor() as usize % 8;
        let u = f - f.floor();
        let u = u * u * (3.0 - 2.0 * u);
        self.reach[i] * (1.0 - u) + self.reach[(i + 1) % 8] * u
    }

    /// The crown's radius at `a` radians in the tree's own frame: its
    /// `canopy` by `depth` ellipse there, times the reach.
    pub fn radius_local(&self, canopy: f64, a: f64) -> f64 {
        let (rx, ry) = (canopy / 2.0, self.depth / 2.0);
        let (c, s) = (a.cos(), a.sin());
        let e = rx * ry / ((ry * c).powi(2) + (rx * s).powi(2)).sqrt().max(1e-9);
        e * self.reach_local(a)
    }

    /// The crown's radius along `dir` in the model.
    pub fn radius_towards(&self, canopy: f64, dir: DVec2) -> f64 {
        self.radius_local(canopy, dir.to_angle() - self.angle.to_radians())
    }

    /// Where the crown is widest, a fraction of the way up it.
    pub fn widest(&self) -> f64 {
        (0..=40)
            .map(|i| i as f64 / 40.0)
            .max_by(|a, b| self.crown.profile(*a).total_cmp(&self.crown.profile(*b)))
            .unwrap_or(0.5)
    }

    /// Checks against the tree's `height`.
    pub fn validate(&self, height: f64) -> Result<(), String> {
        let finite = [
            self.depth,
            self.crown_base,
            self.trunk,
            self.clump_size,
            self.irregularity,
            self.openness,
            self.angle,
        ]
        .iter()
        .chain(self.reach.iter())
        .all(|v| v.is_finite());
        if !finite {
            return Err("a tree's shape must be finite".into());
        }
        if self.depth <= 0.0 || self.trunk <= 0.0 {
            return Err("a tree's crown depth and trunk must be positive".into());
        }
        if self.crown_base < 0.0 || self.crown_base >= height {
            return Err("a tree's crown must start between its base and its top".into());
        }
        if !(1..=60).contains(&self.clumps) {
            return Err("a crown is 1 to 60 clumps".into());
        }
        let unit = |v: f64| (0.0..=1.0).contains(&v);
        if !(unit(self.clump_size) && unit(self.irregularity) && unit(self.openness)) {
            return Err("clump size, irregularity and openness run from 0 to 1".into());
        }
        if self.reach.iter().any(|r| !(0.1..=2.5).contains(r)) {
            return Err("a crown's reach on a side runs from 0.1 to 2.5".into());
        }
        if !(1..=5).contains(&self.facets) {
            return Err("a low-poly crown has 1 to 5 facets".into());
        }
        if !(1..=12).contains(&self.month) {
            return Err("a tree's month runs from 1 to 12".into());
        }
        let colour = |c: &[f32; 3]| c.iter().all(|v| (0.0..=1.0).contains(v));
        if !(colour(&self.leaf_colour) && colour(&self.autumn_colour)) {
            return Err("a tree's leaf colours run from 0 to 1".into());
        }
        Ok(())
    }

    /// Its month when the project shows `project` (1 to 12): its own, or
    /// the project's.
    pub fn month_in(&self, project: u8) -> u8 {
        if self.own_month {
            self.month
        } else {
            project.clamp(1, 12)
        }
    }

    /// Its leaves in its month, `southern` of the equator or not.
    pub fn leaf_state(&self, southern: bool) -> LeafState {
        let m = if southern {
            (self.month.clamp(1, 12) + 5) % 12 + 1
        } else {
            self.month
        };
        self.leaves.state(m)
    }

    /// Its leaves' colour in its month (sRGB).
    pub fn foliage_colour(&self, southern: bool) -> [f32; 3] {
        let s = self.leaf_state(southern);
        let mix = |a: [f32; 3], b: [f32; 3], t: f64| {
            let t = t.clamp(0.0, 1.0) as f32;
            [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
        };
        let c = mix(self.leaf_colour, self.autumn_colour, s.turned);
        mix(c, FRESH, s.fresh * 0.6)
    }

    /// Scaled about its tree by `f`.
    pub fn scale(&mut self, f: f64) {
        self.depth *= f;
        self.crown_base *= f;
        self.trunk *= f;
    }
}

/// A seed from a point: the same point, the same seed.
pub fn seed_at(p: DVec2) -> u32 {
    let q = |v: f64| (v * 1000.0).round() as i64 as u64;
    let mut h =
        q(p.x).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ q(p.y).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 29;
    (h as u32 % 99_999) + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reach_eases_between_sides() {
        let mut t = TreeShape::default();
        t.reach[0] = 0.5;
        assert!((t.reach_local(0.0) - 0.5).abs() < 1e-12);
        assert!((t.reach_local(TAU / 8.0) - 1.0).abs() < 1e-12);
        let mid = t.reach_local(TAU / 16.0);
        assert!(mid > 0.5 && mid < 1.0);
        // turned a quarter, side 0 faces north
        t.angle = 90.0;
        let r = t.radius_towards(4.0, DVec2::Y);
        assert!((r - 1.0).abs() < 1e-9, "{r}");
    }

    #[test]
    fn presets_are_valid_and_seeds_differ_by_place() {
        for p in TreePreset::ALL {
            let (s, _, h) = p.shape();
            s.validate(h).unwrap();
        }
        assert_ne!(seed_at(DVec2::ZERO), seed_at(DVec2::new(5.0, 0.0)));
        assert_eq!(seed_at(DVec2::new(1.0, 2.0)), seed_at(DVec2::new(1.0, 2.0)));
    }

    #[test]
    fn deciduous_trees_are_bare_in_winter_and_turn_in_autumn() {
        let mut t = TreeShape {
            leaves: LeafHabit::Deciduous,
            month: 1,
            ..TreeShape::default()
        };
        assert_eq!(t.leaf_state(false).leaf, 0.0);
        // south of the equator, January is high summer
        assert_eq!(t.leaf_state(true).leaf, 1.0);
        t.month = 10;
        let c = t.foliage_colour(false);
        assert!(c[0] > t.leaf_colour[0] + 0.2, "{c:?}");
        t.month = 7;
        assert_eq!(t.foliage_colour(false), t.leaf_colour);
        // evergreens never lose theirs
        t.leaves = LeafHabit::Evergreen;
        assert!((1..=12).all(|m| LeafHabit::Evergreen.state(m).leaf == 1.0));
        t.month = 13;
        assert!(t.validate(9.0).is_err());
    }

    #[test]
    fn a_crown_cant_start_above_its_top() {
        let t = TreeShape::default();
        assert!(t.validate(2.0).is_err());
        assert!(t.validate(9.0).is_ok());
    }
}
