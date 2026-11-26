//! Graphic styles as data: how each category of element is drawn (object
//! styles), the hatch patterns cut faces and surfaces are filled with, the
//! pen widths each scale draws with, view templates and per-view overrides.
//!
//! All of it travels as one [`StylePack`], a JSON file an office keeps its
//! drawing standard in. CreaBIM names no standard of its own: the look it
//! draws with when a project has no pack is the *Standard* pack shipped in
//! `packs/standard.json`, read like any other.

use crate::annotation::TextStyle;
use crate::graphics::LineWeights;
use crate::marks::MarkRules;
use crate::titleblock::SheetTemplate;
use crate::visibility::Category;
use crate::{Element, ElementId, ElementKind, Hatch, Model, Transaction, View, ViewStyle};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;

/// An sRGB colour, 0..1. Packs write it as `"#rrggbb"`; a list of three
/// numbers reads too.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb(pub [f32; 3]);

impl Rgb {
    pub fn hex(self) -> String {
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!(
            "#{:02x}{:02x}{:02x}",
            c(self.0[0]),
            c(self.0[1]),
            c(self.0[2])
        )
    }

    pub fn parse(s: &str) -> Option<Self> {
        let h = s.strip_prefix('#')?;
        if h.len() != 6 {
            return None;
        }
        let v = |i: usize| {
            u8::from_str_radix(&h[i..i + 2], 16)
                .ok()
                .map(|b| b as f32 / 255.0)
        };
        Some(Rgb([v(0)?, v(2)?, v(4)?]))
    }
}

impl Serialize for Rgb {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.hex())
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Hex(String),
            List([f32; 3]),
        }
        match Raw::deserialize(d)? {
            Raw::Hex(s) => {
                Rgb::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("bad colour {s}")))
            }
            Raw::List(v) => Ok(Rgb(v)),
        }
    }
}

/// The pens of a view's [`LineWeights`], by name: the six built-in pens.
/// They stay, so old projects open unchanged; a pack can rename them and
/// add pens of its own ([`OwnPen`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PenName {
    Cut,
    Projection,
    Symbol,
    Fine,
    Hatch,
    Hidden,
}

impl PenName {
    pub const ALL: [PenName; 6] = [
        PenName::Cut,
        PenName::Projection,
        PenName::Symbol,
        PenName::Fine,
        PenName::Hatch,
        PenName::Hidden,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PenName::Cut => "Cut pen",
            PenName::Projection => "Projection pen",
            PenName::Symbol => "Symbol pen",
            PenName::Fine => "Fine pen",
            PenName::Hatch => "Hatch pen",
            PenName::Hidden => "Hidden pen",
        }
    }

    pub fn width(self, w: &LineWeights) -> f64 {
        match self {
            PenName::Cut => w.cut,
            PenName::Projection => w.projection,
            PenName::Symbol => w.symbol,
            PenName::Fine => w.fine,
            PenName::Hatch => w.hatch,
            PenName::Hidden => w.hidden,
        }
    }
}

/// A line weight: one of the view's pens (so it follows the scale), one
/// of the pack's own pens, or a fixed width on paper in millimetres.
/// Written `"Fine"`, `{"pen": 3}` or `0.25`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Weight {
    Pen(PenName),
    Mm(f64),
    /// One of the pack's [`OwnPen`]s, by its id.
    Own {
        pen: u32,
    },
}

impl Weight {
    /// Its width with the built-in pens `w`; a pen of the user's own reads
    /// as the projection pen here ([`PenWidths::mm`] knows its width).
    pub fn mm(self, w: &LineWeights) -> f64 {
        match self {
            Weight::Pen(p) => p.width(w),
            Weight::Mm(v) => v,
            Weight::Own { .. } => w.projection,
        }
    }

    /// Its name without a pack: [`StylePack::weight_label`] names the
    /// pack's pens.
    pub fn label(self) -> String {
        match self {
            Weight::Pen(p) => p.label().to_string(),
            Weight::Mm(v) => format!("{v:.2} mm"),
            Weight::Own { pen } => format!("Pen {pen}"),
        }
    }
}

/// A pen of the user's own (GitHub Discussion #8), kept in the style pack
/// beside the six built-in ones. It has a width in every pen set and every
/// scale table ([`PenTable::own`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OwnPen {
    pub id: u32,
    pub name: String,
}

/// The widths a drawing draws with: the built-in pens and the pack's own,
/// for one pen set at one scale.
#[derive(Clone, Debug, PartialEq)]
pub struct PenWidths {
    pub weights: LineWeights,
    pub own: BTreeMap<u32, f64>,
}

impl PenWidths {
    pub fn mm(&self, w: Weight) -> f64 {
        match w {
            Weight::Own { pen } => self
                .own
                .get(&pen)
                .copied()
                .unwrap_or(self.weights.projection),
            w => w.mm(&self.weights),
        }
    }
}

/// A named version of the whole pen table (AMD-016, pen sets): Technical,
/// Sketch, Presentation and Fine line ship with CreaBIM; a pack keeps the
/// ones it changed and its own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PenSet {
    pub name: String,
    /// Pens by scale, smallest `up_to` first.
    pub tables: Vec<PenTable>,
}

/// The pen sets that ship with CreaBIM, in their order.
pub const SHIPPED_SETS: [&str; 4] = ["Technical", "Sketch", "Presentation", "Fine line"];

/// What each shipped set is for, as the set row says it.
pub fn shipped_set_about(name: &str) -> &'static str {
    match name {
        "Technical" => "Permit and construction drawings: a clear hierarchy, everything readable.",
        "Sketch" => "Ideation: bold and loose, reads from across the room.",
        "Presentation" => "Client boards: a strong cut over hairlines.",
        "Fine line" => "Details and small prints: light and precise.",
        _ => "",
    }
}

/// A shipped set as it comes from the factory. *Technical* is the pens
/// CreaBIM has always drawn with ([`LineWeights::for_scale`]), so a
/// project that never picked a set draws as before.
pub fn shipped_set(name: &str) -> Option<PenSet> {
    let lw = |v: [f64; 6]| LineWeights {
        cut: v[0],
        projection: v[1],
        symbol: v[2],
        fine: v[3],
        hatch: v[4],
        hidden: v[5],
    };
    let table = |up_to, weights| PenTable {
        up_to,
        weights,
        own: BTreeMap::new(),
    };
    if name == "Technical" {
        return Some(PenSet {
            name: name.into(),
            tables: crate::graphics::TECHNICAL_ROWS
                .iter()
                .map(|(up_to, _)| table(*up_to, LineWeights::for_scale(*up_to)))
                .collect(),
        });
    }
    let (large, small) = match name {
        "Sketch" => (
            lw([1.0, 0.50, 0.35, 0.25, 0.18, 0.35]),
            lw([0.70, 0.35, 0.25, 0.18, 0.13, 0.25]),
        ),
        "Presentation" => (
            lw([0.70, 0.18, 0.13, 0.09, 0.09, 0.13]),
            lw([0.60, 0.13, 0.09, 0.05, 0.05, 0.09]),
        ),
        "Fine line" => (
            lw([0.50, 0.18, 0.13, 0.10, 0.08, 0.13]),
            lw([0.35, 0.13, 0.10, 0.08, 0.06, 0.10]),
        ),
        _ => return None,
    };
    Some(PenSet {
        name: name.into(),
        tables: vec![table(75.0, large), table(100000.0, small)],
    })
}

impl PenSet {
    /// The widths for a drawing at 1:`scale`: the first table whose scale
    /// it is at or larger than, else the last.
    pub fn widths_at(&self, scale: f64) -> PenWidths {
        let mut tables: Vec<&PenTable> = self.tables.iter().collect();
        tables.sort_by(|a, b| a.up_to.total_cmp(&b.up_to));
        match tables.iter().find(|t| scale <= t.up_to).or(tables.last()) {
            Some(t) => PenWidths {
                weights: t.weights,
                own: t.own.clone(),
            },
            None => PenWidths {
                weights: LineWeights::for_scale(scale),
                own: BTreeMap::new(),
            },
        }
    }

    /// The built-in widths and scales only, as the factory compares them
    /// (a user's own pens don't make a shipped set "changed").
    pub fn built_in_part(&self) -> Vec<(f64, LineWeights)> {
        self.tables.iter().map(|t| (t.up_to, t.weights)).collect()
    }
}

/// How a line is broken up on paper.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LinePattern {
    Solid,
    Dashed,
    Dotted,
    /// Long and short dashes, for centre lines.
    Centre,
    DashDot,
    /// Dash, gap, dash, gap… in paper millimetres; a dash of 0 is a dot.
    Custom(Vec<f64>),
}

impl LinePattern {
    pub const NAMED: [LinePattern; 5] = [
        LinePattern::Solid,
        LinePattern::Dashed,
        LinePattern::Dotted,
        LinePattern::Centre,
        LinePattern::DashDot,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            LinePattern::Solid => "Solid",
            LinePattern::Dashed => "Dashed",
            LinePattern::Dotted => "Dotted",
            LinePattern::Centre => "Centre",
            LinePattern::DashDot => "Dash dot",
            LinePattern::Custom(_) => "Custom",
        }
    }

    /// Its dashes and gaps in paper mm; none when it is solid.
    pub fn dashes(&self) -> Option<Vec<f64>> {
        match self {
            LinePattern::Solid => None,
            LinePattern::Dashed => Some(vec![2.0, 1.0]),
            LinePattern::Dotted => Some(vec![0.0, 0.8]),
            LinePattern::Centre => Some(vec![6.0, 1.0, 1.0, 1.0]),
            LinePattern::DashDot => Some(vec![3.0, 1.0, 0.0, 1.0]),
            LinePattern::Custom(v) => {
                let ok = v.len() >= 2 && v.iter().all(|x| *x >= 0.0) && v.iter().sum::<f64>() > 0.0;
                ok.then(|| v.clone())
            }
        }
    }
}

/// How a category draws, model-wide: an object style. With `material`,
/// only where its elements are of that material (by name). Every field is
/// optional; what is left out draws as CreaBIM does without a style.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObjectStyle {
    pub category: Category,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// Lines seen in projection (below the cut, in elevation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection_weight: Option<Weight>,
    /// Outlines of what the view cuts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut_weight: Option<Weight>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<Rgb>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<LinePattern>,
    /// Behind the cut hatch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut_fill: Option<Rgb>,
    /// A [`HatchPattern`] by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut_pattern: Option<String>,
    /// The fill of its surfaces seen in projection (a roof from above).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_fill: Option<Rgb>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_pattern: Option<String>,
    /// Lower draws first, under higher; 0 when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draw_order: Option<i32>,
    /// 0 opaque to 1 invisible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparency: Option<f32>,
}

impl ObjectStyle {
    pub fn new(category: Category) -> Self {
        Self {
            category,
            material: None,
            projection_weight: None,
            cut_weight: None,
            colour: None,
            pattern: None,
            cut_fill: None,
            cut_pattern: None,
            surface_fill: None,
            surface_pattern: None,
            draw_order: None,
            transparency: None,
        }
    }
}

/// Whether a pattern's sizes are on paper (fixed-size dots and lines) or
/// in the model (true-size block coursing and tiles).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternUnits {
    /// Millimetres on paper.
    #[default]
    Paper,
    /// Metres in the model.
    Model,
}

/// One family of parallel lines in a hatch, as in a CAD `.pat` file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HatchLine {
    /// Degrees counter-clockwise from the drawing's x axis.
    #[serde(default)]
    pub angle: f64,
    /// Distance between the lines.
    pub spacing: f64,
    /// The family moved this far across its lines.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub offset: f64,
    /// Each line's dashes slid this far along it from the one before,
    /// for staggered coursing.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub shift: f64,
    /// Where the first line's dashes start along it.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub phase: f64,
    /// Dash, gap, dash, gap…; a dash of 0 is a dot. None is solid.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dashes: Vec<f64>,
    /// The angle is from the outline's longest side, not the x axis: grain
    /// running along a timber.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub along_longest: bool,
}

/// A small drawing repeated across a fill, for patterns lines can't make.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    /// Width and height of one tile.
    pub size: [f64; 2],
    /// Polylines inside the tile, from its lower left corner.
    pub strokes: Vec<Vec<[f64; 2]>>,
}

/// A named fill pattern: lines, tiles or a recipe (bricks, boards, a tile
/// you drew) over a background.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HatchPattern {
    pub name: String,
    #[serde(default)]
    pub units: PatternUnits,
    /// Behind the lines; none leaves it white.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Rgb>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<HatchLine>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile: Option<Tile>,
    /// Units laid out by a generator or a drawn tile, in the pattern's
    /// units (see [`crate::pattern`]). Drawn as their outlines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe: Option<crate::pattern::Recipe>,
    /// The recipe turned this many degrees counter-clockwise.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub angle: f64,
    /// The recipe runs along the outline's longest side, from its start,
    /// instead of the drawing's x axis: courses along a wall in plan.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub along_longest: bool,
    /// Drawn smaller than a scale, another (symbolic) pattern stands in:
    /// brick coursing at 1:20, the diagonal brick hatch at 1:100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_scale: Option<SmallScale>,
}

/// The pattern a true-size pattern gives way to in small-scale drawings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SmallScale {
    /// Drawings at 1:`below` and smaller (1:100 when 50) take `pattern`.
    pub below: f64,
    pub pattern: String,
}

impl HatchPattern {
    /// A true-size pattern made by `recipe`.
    pub fn from_recipe(name: &str, recipe: crate::pattern::Recipe) -> Self {
        Self {
            name: name.to_string(),
            units: PatternUnits::Model,
            recipe: Some(recipe),
            ..Default::default()
        }
    }

    /// The pattern to draw at 1:`scale`: this one, or the one it names
    /// for small scales.
    pub fn small_scale_name(&self, scale: f64) -> Option<&str> {
        self.small_scale
            .as_ref()
            .filter(|s| {
                s.below > 0.0 && scale >= s.below && !s.pattern.eq_ignore_ascii_case(&self.name)
            })
            .map(|s| s.pattern.as_str())
    }
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// The paper spacing the built-in patterns are sized from, mm.
pub const BASE_SPACING: f64 = 1.2;

/// The built-in pattern a material's [`Hatch`] names: the same lines
/// CreaBIM drew before patterns were data, so every material still reads.
pub fn builtin_pattern(h: Hatch) -> HatchPattern {
    let s = BASE_SPACING;
    let grey = |g: f32| Some(Rgb([g; 3]));
    let line = |angle: f64, spacing: f64, offset: f64| HatchLine {
        angle,
        spacing,
        offset,
        phase: 0.0,
        shift: 0.0,
        dashes: vec![],
        along_longest: false,
    };
    let (fill, lines) = match h {
        Hatch::Empty => (None, vec![]),
        Hatch::Solid => (grey(0.6), vec![]),
        Hatch::Diagonal => (grey(0.88), vec![line(45.0, s, 0.0)]),
        Hatch::Cross => (
            grey(0.88),
            vec![line(45.0, s * 1.4, 0.0), line(-45.0, s * 1.4, 0.0)],
        ),
        Hatch::Brick => (
            grey(0.88),
            vec![line(45.0, s * 2.0, 0.0), line(45.0, s * 2.0, s * 0.5)],
        ),
        Hatch::Concrete => (
            grey(0.88),
            vec![
                line(45.0, s * 4.0, 0.0),
                HatchLine {
                    dashes: vec![0.0, s * 1.3],
                    shift: s * 0.65,
                    ..line(0.0, s * 1.3, 0.0)
                },
            ],
        ),
        Hatch::Timber => (
            grey(0.88),
            vec![HatchLine {
                along_longest: true,
                ..line(0.0, s * 1.2, s * 0.3)
            }],
        ),
    };
    HatchPattern {
        name: h.label().to_string(),
        units: PatternUnits::Paper,
        fill,
        lines,
        ..Default::default()
    }
}

/// The pens for drawings up to a scale: `up_to` 75 covers 1:75 and larger.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PenTable {
    pub up_to: f64,
    pub weights: LineWeights,
    /// The widths of the pack's own pens, by id; one missing draws as the
    /// projection pen.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub own: BTreeMap<u32, f64>,
}

/// How a cut material is patterned at one scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScalePattern {
    /// Timber grain and end grain, batt insulation, at their real size.
    TrueSize,
    /// The material's hatch, its lines the row's spacing apart.
    Plain,
    /// The material's fill, no lines.
    Fill,
}

impl ScalePattern {
    pub const ALL: [ScalePattern; 3] = [
        ScalePattern::TrueSize,
        ScalePattern::Plain,
        ScalePattern::Fill,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ScalePattern::TrueSize => "True size",
            ScalePattern::Plain => "Plain",
            ScalePattern::Fill => "Fill",
        }
    }
}

/// What a room tag says in a row of the scale table (owner, MU131).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoomTagSays {
    /// Its name, area and number (or the fields the tag asks for).
    #[default]
    All,
    /// Its name and number.
    NameNumber,
    /// Its number.
    Number,
}

impl RoomTagSays {
    pub fn label(self) -> &'static str {
        match self {
            RoomTagSays::All => "Name, area, number",
            RoomTagSays::NameNumber => "Name, number",
            RoomTagSays::Number => "Number",
        }
    }
}

/// One row of the scale table: how drawings up to 1:`up_to` are hatched,
/// lettered and dimensioned, beside their pens (owner,
/// Mockup_Scale_Aware_Drawings_REV00). Sizes are on paper, in millimetres.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScaleRule {
    pub up_to: f64,
    /// Plain hatch lines this far apart.
    pub hatch_spacing: f64,
    pub pattern: ScalePattern,
    /// A layer thinner than this is filled, not hatched.
    pub thin_layer: f64,
    /// Notes and tags without a text style of their own.
    pub note_text: f64,
    /// View titles.
    pub title_text: f64,
    /// Dimension text.
    pub dim_text: f64,
    /// A dimension's tick, end to end.
    pub dim_tick: f64,
    /// Dimensions in metric units round to this many millimetres.
    pub round_mm: f64,
    /// Dimensions in feet and inches round to 1/`round_in` of an inch.
    pub round_in: u32,
    /// What a room tag says.
    #[serde(default)]
    pub room_tag: RoomTagSays,
}

/// The scale table CreaBIM ships with: six rows, 1:5 to 1:200 and smaller.
pub fn shipped_scale_rules() -> Vec<ScaleRule> {
    let row = |up_to,
               hatch_spacing,
               pattern,
               thin_layer,
               note_text,
               title_text,
               dim_text,
               dim_tick,
               round_mm,
               round_in,
               room_tag| ScaleRule {
        up_to,
        hatch_spacing,
        pattern,
        thin_layer,
        note_text,
        title_text,
        dim_text,
        dim_tick,
        round_mm,
        round_in,
        room_tag,
    };
    use RoomTagSays::*;
    use ScalePattern::*;
    vec![
        row(5.0, 3.0, TrueSize, 1.0, 2.5, 5.0, 2.5, 2.0, 1.0, 16, All),
        row(10.0, 2.5, TrueSize, 1.0, 2.5, 5.0, 2.5, 2.0, 1.0, 8, All),
        row(20.0, 2.0, TrueSize, 1.5, 2.5, 5.0, 2.5, 1.8, 5.0, 4, All),
        row(50.0, 1.5, Plain, 1.5, 2.5, 5.0, 2.5, 1.5, 10.0, 2, All),
        row(
            100.0, 1.2, Plain, 1.5, 2.5, 5.0, 2.5, 1.5, 10.0, 1, NameNumber,
        ),
        row(
            100000.0, 1.0, Fill, 2.0, 2.0, 4.0, 2.0, 1.2, 50.0, 1, Number,
        ),
    ]
}

/// The row of `rules` for a drawing at 1:`scale`: the first whose scale it
/// is at or larger than, else the last.
pub fn scale_rule_in(rules: &[ScaleRule], scale: f64) -> ScaleRule {
    let mut rows: Vec<&ScaleRule> = rules.iter().collect();
    rows.sort_by(|a, b| a.up_to.total_cmp(&b.up_to));
    match rows.iter().find(|r| scale <= r.up_to).or(rows.last()) {
        Some(r) => **r,
        None => scale_rule_in(&shipped_scale_rules(), scale),
    }
}

/// What a per-view override applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Target {
    Category(Category),
    Element(ElementId),
}

/// A view's change to how a category or one element draws in it, on top
/// of the object styles: "site elements halftone", "boundary wall dashed".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Override {
    pub target: Target,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<Weight>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut_weight: Option<Weight>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<Rgb>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<LinePattern>,
    /// Drawn half-strength, mixed toward white.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub halftone: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparency: Option<f32>,
    /// Cut hatching on or off; none keeps the style's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hatch: Option<bool>,
}

impl Override {
    pub fn new(target: Target) -> Self {
        Self {
            target,
            weight: None,
            cut_weight: None,
            colour: None,
            pattern: None,
            halftone: false,
            transparency: None,
            hatch: None,
        }
    }
}

/// Sun shadows: cast from each element onto the ground below it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shadows {
    /// The compass bearing shadows fall toward, degrees counter-clockwise
    /// from the drawing's x axis.
    pub direction: f64,
    /// How long a shadow is per metre of height.
    pub length: f64,
    /// 0 none to 1 black.
    pub opacity: f32,
}

/// Depth and shadow cues of a view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DepthCues {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadows: Option<Shadows>,
    /// Lines and fills further from the cut or the viewer get lighter,
    /// up to this much at the far end of the view (0..1).
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub fade: f32,
}

fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

impl DepthCues {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// A named bundle of how views draw, applied to many views: change it and
/// every view that uses it changes. Views name their template, so loading
/// another pack with templates of the same names restyles them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewTemplate {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<ViewStyle>,
    /// Categories its views leave out, besides their own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<Category>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<Override>,
    /// Pens of its own, whatever the scale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights: Option<LineWeights>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub below_fill: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cues: Option<DepthCues>,
    /// A pen set of the pack, by name, instead of the project's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pen_set: Option<String>,
    /// A line style by name, instead of the project's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_style: Option<String>,
    /// A palette by name, instead of the project's (pens stage 7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
    /// Symbol picks of its own, kind by kind, over the pack's.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub symbols: crate::symbols::SymbolPicks,
}

impl ViewTemplate {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            scale: None,
            style: None,
            hidden: vec![],
            overrides: vec![],
            weights: None,
            below_fill: None,
            cues: None,
            pen_set: None,
            line_style: None,
            palette: None,
            symbols: Default::default(),
        }
    }
}

/// An annotation style carried by a pack, by name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedTextStyle {
    pub name: String,
    pub style: TextStyle,
}

/// A title block carried by a pack, by name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedTitleBlock {
    pub name: String,
    pub template: SheetTemplate,
}

/// An office's drawing standard as one file: pens, object styles, hatch
/// patterns, view templates, text styles, title blocks and mark rules.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StylePack {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The project draws with it. One pack in a project is.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub default: bool,
    /// Pens by scale, smallest `up_to` first: a pack's pens from before pen
    /// sets. Read as the project's set while no set is picked
    /// ([`StylePack::normalised`] turns them into one).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pens: Vec<PenTable>,
    /// The pen sets it keeps: shipped ones it changed, and its own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pen_sets: Vec<PenSet>,
    /// The project's pen set, by name; none is *Technical*.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pen_set: Option<String>,
    /// Line styles it keeps: shipped ones it adjusted (same name), and its
    /// own (pens stage 4, owner 2026-11-16).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line_styles: Vec<crate::linestyle::LineStyle>,
    /// The project's line style, by name; none is *Ruled*.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_style: Option<String>,
    /// Palettes it keeps: shipped ones it adjusted (same name), and its
    /// own (pens stage 7, Mockup_Pens_REV05).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub palettes: Vec<crate::palette::Palette>,
    /// The project's palette, by name; none is *Ink*.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
    /// The symbol each kind of mark draws with (kind id to style id); a
    /// kind without one draws CreaBIM's own mark.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub symbols: crate::symbols::SymbolPicks,
    /// Kinds whose symbol follows the line style: Hand and Loose take the
    /// kind's Hand-family style.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbols_follow: Vec<String>,
    /// The font of dimensions, notes and tags that have no style of their
    /// own (MU85: a font changed on the styles left unstyled tags as they
    /// were). None is Helvetica.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_font: Option<crate::Font>,
    /// The user's own pens, beside the six built-in ones.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub own_pens: Vec<OwnPen>,
    /// Built-in pens given another name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pen_names: BTreeMap<PenName, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub objects: Vec<ObjectStyle>,
    /// Patterns by name. One named like a built-in (*Brick*, *Concrete*…)
    /// replaces it for every material using that hatch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patterns: Vec<HatchPattern>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub templates: Vec<ViewTemplate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text_styles: Vec<NamedTextStyle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub title_blocks: Vec<NamedTitleBlock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark_rules: Option<MarkRules>,
    /// The template a view without one of its own takes, by kind.
    #[serde(default, skip_serializing_if = "KindTemplates::is_empty")]
    pub default_templates: KindTemplates,
    /// How every room tag in the project is laid out.
    #[serde(default, skip_serializing_if = "RoomTagStyle::is_default")]
    pub room_tags: RoomTagStyle,
    /// The family every section, elevation and callout mark is drawn in,
    /// so a project never mixes them.
    #[serde(default, skip_serializing_if = "MarkerFamily::is_default")]
    pub markers: MarkerFamily,
    /// One of `marker_styles` by name, drawn instead of the family.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker_style: Option<String>,
    /// The pack's own marker styles, mixed in the marker designer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marker_styles: Vec<NamedMarkerStyle>,
    /// How membranes are drawn in details.
    #[serde(default, skip_serializing_if = "crate::MembraneLook::is_default")]
    pub membranes: crate::MembraneLook,
    /// The drawing standard the project draws to: its sheet sizes and
    /// scales; none is ISO.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standard: Option<crate::standards::DrawingStandard>,
    /// The scale table's rows beyond pens; none is CreaBIM's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scale_rules: Vec<ScaleRule>,
    /// How numbers read, per kind of annotation (A5-2).
    #[serde(
        default,
        skip_serializing_if = "crate::units::NumberFormats::is_default"
    )]
    pub number_formats: crate::units::NumberFormats,
}

/// How a room tag lays out its fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoomTagStyle {
    /// The name in capitals and semibold, the area below it, the number
    /// small and last.
    #[default]
    Stacked,
    /// The number in a small rounded outline on top, the name, then the
    /// area.
    NumberBadge,
    /// A box with the name, and a split footer: the number and the area.
    Boxed,
    /// The tag as it was first drawn: the number and the area on one line.
    Inline,
}

impl RoomTagStyle {
    pub const ALL: [RoomTagStyle; 4] = [
        RoomTagStyle::Stacked,
        RoomTagStyle::NumberBadge,
        RoomTagStyle::Boxed,
        RoomTagStyle::Inline,
    ];

    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub fn label(self) -> &'static str {
        match self {
            RoomTagStyle::Stacked => "Stacked",
            RoomTagStyle::NumberBadge => "Number badge",
            RoomTagStyle::Boxed => "Boxed",
            RoomTagStyle::Inline => "Inline",
        }
    }
}

/// A family of view marks: sections, elevations, callouts and detail
/// callouts drawn as one set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkerFamily {
    /// A split bubble (the view's number over its sheet), a solid wedge
    /// the way the view looks, heavy line ends and a dash-dot line.
    #[default]
    Classic,
    /// No bubbles: a small arrow, the number in semibold and the sheet
    /// after it; callouts a thin solid box with a dark label tab.
    Minimal,
    /// Filled dark heads with white text and an accent wedge; callouts a
    /// heavy box with the bubble on its edge.
    Bold,
    /// The marks as they were first drawn.
    Plain,
}

impl MarkerFamily {
    pub const ALL: [MarkerFamily; 4] = [
        MarkerFamily::Classic,
        MarkerFamily::Minimal,
        MarkerFamily::Bold,
        MarkerFamily::Plain,
    ];

    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub fn label(self) -> &'static str {
        match self {
            MarkerFamily::Classic => "Classic",
            MarkerFamily::Minimal => "Minimal",
            MarkerFamily::Bold => "Bold",
            MarkerFamily::Plain => "Plain (first markers)",
        }
    }

    /// The parts it is drawn with; none for the first markers, drawn as
    /// they always were.
    pub fn parts(self) -> Option<MarkerParts> {
        use MarkerColour as C;
        Some(match self {
            MarkerFamily::Classic => MarkerParts::default(),
            MarkerFamily::Minimal => MarkerParts {
                head: MarkerHead::TextOnly,
                fill: MarkerFill::Outline,
                text: MarkerText::Inline,
                pointer: MarkerPointer::SmallArrow,
                line: MarkerLine::Thin,
                edge: CalloutEdge::SolidThin,
                colour: C::Ink,
            },
            MarkerFamily::Bold => MarkerParts {
                head: MarkerHead::Circle,
                fill: MarkerFill::Filled,
                text: MarkerText::Split,
                pointer: MarkerPointer::AccentWedge,
                line: MarkerLine::EndsOnly,
                edge: CalloutEdge::Solid2,
                colour: C::Ink,
            },
            MarkerFamily::Plain => return None,
        })
    }
}

/// A marker's head: what holds the view's number and sheet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkerHead {
    #[default]
    Circle,
    RoundedBox,
    TextOnly,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkerFill {
    #[default]
    Outline,
    /// Filled in the colour, with white text.
    Filled,
}

/// How the number and the sheet are written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkerText {
    /// The number over the sheet, split by a line.
    #[default]
    Split,
    /// "A · A301" on one line.
    Inline,
}

/// What points the way the view looks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkerPointer {
    #[default]
    Wedge,
    SmallArrow,
    /// A wedge in the accent colour.
    AccentWedge,
}

/// A section's line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkerLine {
    /// Heavy ends and a dash-dot line between them.
    #[default]
    FullHeavyEnds,
    /// A thin dash-dot line end to end.
    Thin,
    /// Short heavy ends and nothing between.
    EndsOnly,
}

/// A callout's box or circle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalloutEdge {
    #[default]
    Dashed,
    SolidThin,
    /// A heavy solid edge, about 2 px on screen.
    Solid2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkerColour {
    #[default]
    Ink,
    Accent,
    /// Annotation blue.
    Blue,
}

impl MarkerColour {
    pub const ALL: [MarkerColour; 3] =
        [MarkerColour::Ink, MarkerColour::Accent, MarkerColour::Blue];

    /// sRGB, 0..1.
    pub fn rgb(self) -> [f32; 3] {
        match self {
            MarkerColour::Ink => [0.17, 0.19, 0.16],
            MarkerColour::Accent => ACCENT_RGB,
            MarkerColour::Blue => [0.13, 0.27, 0.58],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MarkerColour::Ink => "Ink",
            MarkerColour::Accent => "Accent",
            MarkerColour::Blue => "Annotation blue",
        }
    }
}

/// The sage accent (#4F6B4C), sRGB 0..1.
pub const ACCENT_RGB: [f32; 3] = [0.31, 0.42, 0.30];

/// The parts every view mark is drawn with: sections, elevations,
/// callouts and detail callouts. The default is the Classic family.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkerParts {
    #[serde(default)]
    pub head: MarkerHead,
    #[serde(default)]
    pub fill: MarkerFill,
    #[serde(default)]
    pub text: MarkerText,
    #[serde(default)]
    pub pointer: MarkerPointer,
    #[serde(default)]
    pub line: MarkerLine,
    #[serde(default)]
    pub edge: CalloutEdge,
    #[serde(default)]
    pub colour: MarkerColour,
}

/// A marker style of the pack's own, by name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedMarkerStyle {
    pub name: String,
    pub parts: MarkerParts,
}

/// A template name for each kind of view.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KindTemplates {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    /// Plans that look down from above: roof and site plans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_from_above: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elevation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub axonometric: Option<String>,
    /// Detail views called out from a plan or section; without one they
    /// take their parent kind's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl KindTemplates {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// The name for a view of `v`'s kind.
    pub fn of(&self, v: &View) -> Option<&str> {
        use crate::ViewKind as K;
        if let (Some(_), Some(d)) = (v.callout, self.detail.as_deref()) {
            return Some(d);
        }
        match &v.kind {
            K::Plan { .. } if v.graphics.range.from_above => self.plan_from_above.as_deref(),
            K::Plan { .. } => self.plan.as_deref(),
            K::Section { cut: true, .. } => self.section.as_deref(),
            K::Section { cut: false, .. } => self.elevation.as_deref(),
            K::Axonometric { .. } => self.axonometric.as_deref(),
            K::Schedule { .. }
            | K::LongSection { .. }
            | K::BoreholeLog { .. }
            | K::WalkSection { .. } => None,
        }
    }
}

/// The two packs that ship with CreaBIM, as data.
pub const STANDARD_PACK: &str = include_str!("../packs/standard.json");
pub const LIGHT_PACK: &str = include_str!("../packs/light-linework.json");

impl StylePack {
    /// The look of a project without a pack: the *Standard* pack.
    pub fn standard() -> Self {
        Self::from_json(STANDARD_PACK).expect("the standard pack reads")
    }

    /// The packs that come with CreaBIM.
    pub fn shipped() -> Vec<Self> {
        [STANDARD_PACK, LIGHT_PACK]
            .iter()
            .map(|s| Self::from_json(s).expect("shipped packs read"))
            .collect()
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let p: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        p.validate()?;
        Ok(p)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a pack always serializes")
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("a style pack needs a name".into());
        }
        for p in &self.patterns {
            for l in &p.lines {
                if l.spacing.is_nan() || l.spacing <= 0.0 {
                    return Err(format!("pattern {}: spacing must be above 0", p.name));
                }
            }
            if let Some(t) = &p.tile {
                if t.size.iter().any(|v| v.is_nan() || *v <= 0.0) {
                    return Err(format!("pattern {}: tile size must be above 0", p.name));
                }
            }
        }
        for t in self
            .pens
            .iter()
            .chain(self.pen_sets.iter().flat_map(|s| &s.tables))
        {
            if t.up_to.is_nan() || t.up_to <= 0.0 {
                return Err("pen tables need a scale above 0".into());
            }
        }
        for s in &self.pen_sets {
            if s.name.trim().is_empty() {
                return Err("a pen set needs a name".into());
            }
            if s.tables.is_empty() {
                return Err(format!("pen set {}: it needs a scale table", s.name));
            }
        }
        if let Some(r) = &self.mark_rules {
            r.validate()?;
        }
        Ok(())
    }

    /// The parts the pack's view marks are drawn with: its own style when
    /// it names one, else its family's; none for the first markers.
    pub fn marker_parts(&self) -> Option<MarkerParts> {
        if let Some(own) = self
            .marker_style
            .as_ref()
            .and_then(|n| self.marker_styles.iter().find(|s| &s.name == n))
        {
            return Some(own.parts);
        }
        self.markers.parts()
    }

    /// What the pack's markers are called: its own style's name, or the
    /// family's.
    pub fn marker_label(&self) -> String {
        match self
            .marker_style
            .as_ref()
            .filter(|n| self.marker_styles.iter().any(|s| &s.name == *n))
        {
            Some(n) => n.clone(),
            None => self.markers.label().to_string(),
        }
    }

    /// The scale table's row for a drawing at 1:`scale`.
    pub fn rule_at(&self, scale: f64) -> ScaleRule {
        if self.scale_rules.is_empty() {
            scale_rule_in(&shipped_scale_rules(), scale)
        } else {
            scale_rule_in(&self.scale_rules, scale)
        }
    }

    /// The pens a drawing at 1:`scale` takes, with the project's set.
    pub fn weights_at(&self, scale: f64) -> LineWeights {
        self.pens_at(None, scale).weights
    }

    /// The project's pen set by name: its own pick, else *Technical*.
    pub fn default_set_name(&self) -> String {
        self.pen_set
            .clone()
            .filter(|n| self.set(n).is_some())
            .unwrap_or_else(|| SHIPPED_SETS[0].to_string())
    }

    /// A pen set by name: the pack's own copy, else a shipped one as it
    /// comes from the factory.
    pub fn set(&self, name: &str) -> Option<PenSet> {
        self.pen_sets
            .iter()
            .find(|s| s.name == name)
            .cloned()
            .or_else(|| shipped_set(name))
    }

    /// Every set it offers: the shipped four, then its own.
    pub fn set_names(&self) -> Vec<String> {
        let mut out: Vec<String> = SHIPPED_SETS.iter().map(|s| s.to_string()).collect();
        for s in &self.pen_sets {
            if !out.contains(&s.name) {
                out.push(s.name.clone());
            }
        }
        out
    }

    /// The tables of pen set `set`, or of the project's set: a pack from
    /// before pen sets draws with its own tables until a set is picked.
    pub fn tables(&self, set: Option<&str>) -> Vec<PenTable> {
        if let Some(s) = set.and_then(|n| self.set(n)) {
            return s.tables;
        }
        if let Some(s) = self.pen_set.as_deref().and_then(|n| self.set(n)) {
            return s.tables;
        }
        if !self.pens.is_empty() {
            return self.pens.clone();
        }
        shipped_set(SHIPPED_SETS[0]).expect("shipped").tables
    }

    /// The widths of every pen, built-in and own, at 1:`scale` with set
    /// `set` (none: the project's).
    pub fn pens_at(&self, set: Option<&str>, scale: f64) -> PenWidths {
        PenSet {
            name: String::new(),
            tables: self.tables(set),
        }
        .widths_at(scale)
    }

    /// A built-in pen's name in this pack.
    pub fn pen_label(&self, p: PenName) -> String {
        self.pen_names
            .get(&p)
            .cloned()
            .unwrap_or_else(|| p.label().to_string())
    }

    /// A weight's name in this pack: its renamed or own pens by name.
    pub fn weight_label(&self, w: Weight) -> String {
        match w {
            Weight::Pen(p) => self.pen_label(p),
            Weight::Own { pen } => self
                .own_pens
                .iter()
                .find(|o| o.id == pen)
                .map_or_else(|| "A removed pen".into(), |o| o.name.clone()),
            w => w.label(),
        }
    }

    /// The pack with its old pen tables made a pen set: none when they are
    /// *Technical*'s, else a set of their own, picked for the project. It
    /// draws the same.
    pub fn normalised(&self) -> StylePack {
        let mut p = self.clone();
        if p.pens.is_empty() {
            return p;
        }
        let tables = std::mem::take(&mut p.pens);
        if p.pen_set.is_some() {
            return p;
        }
        let technical = shipped_set(SHIPPED_SETS[0]).expect("shipped");
        let as_set = PenSet {
            name: String::new(),
            tables: tables.clone(),
        };
        if as_set.built_in_part() == technical.built_in_part()
            && tables.iter().all(|t| t.own.is_empty())
        {
            return p;
        }
        let mut name = format!("{} pens", p.name);
        let mut n = 2;
        while p.set(&name).is_some() {
            name = format!("{} pens {n}", p.name);
            n += 1;
        }
        p.pen_sets.push(PenSet {
            name: name.clone(),
            tables,
        });
        p.pen_set = Some(name);
        p
    }

    /// How many object styles of the pack name weight `w` for their cut or
    /// projection lines; categories left *As drawn* count for the cut and
    /// projection pens they fall back to.
    pub fn pen_users(&self, w: Weight, categories: &[Category]) -> usize {
        categories
            .iter()
            .filter(|c| {
                let s = self.object_style(**c, None);
                let cut = s
                    .and_then(|s| s.cut_weight)
                    .unwrap_or(Weight::Pen(PenName::Cut));
                let proj = s
                    .and_then(|s| s.projection_weight)
                    .unwrap_or(Weight::Pen(PenName::Projection));
                cut == w || proj == w
            })
            .count()
    }

    /// A new pen of the user's own, with a width in every table of every
    /// set it keeps (the shipped ones it hasn't changed take the projection
    /// width until they are). Returns its id.
    pub fn add_own_pen(&mut self, name: &str) -> u32 {
        let id = self.own_pens.iter().map(|o| o.id).max().unwrap_or(0) + 1;
        self.own_pens.push(OwnPen {
            id,
            name: name.into(),
        });
        for t in self
            .pens
            .iter_mut()
            .chain(self.pen_sets.iter_mut().flat_map(|s| s.tables.iter_mut()))
        {
            t.own.insert(id, t.weights.projection);
        }
        id
    }

    /// Delete an own pen from every set; what named it draws *As drawn*.
    pub fn remove_own_pen(&mut self, id: u32) {
        self.own_pens.retain(|o| o.id != id);
        for t in self
            .pens
            .iter_mut()
            .chain(self.pen_sets.iter_mut().flat_map(|s| s.tables.iter_mut()))
        {
            t.own.remove(&id);
        }
        let gone = Some(Weight::Own { pen: id });
        let clear = |w: &mut Option<Weight>| {
            if *w == gone {
                *w = None;
            }
        };
        for o in &mut self.objects {
            clear(&mut o.cut_weight);
            clear(&mut o.projection_weight);
        }
        for t in &mut self.templates {
            for o in &mut t.overrides {
                clear(&mut o.weight);
                clear(&mut o.cut_weight);
            }
        }
    }

    /// A pattern by name: the pack's own, else a built-in one.
    pub fn pattern(&self, name: &str) -> Option<Cow<'_, HatchPattern>> {
        if let Some(p) = self
            .patterns
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
        {
            return Some(Cow::Borrowed(p));
        }
        Hatch::ALL
            .iter()
            .find(|h| h.label().eq_ignore_ascii_case(name))
            .map(|h| Cow::Owned(builtin_pattern(*h)))
    }

    /// Every pattern name a material or style can pick: built-ins, then
    /// the pack's own.
    pub fn pattern_names(&self) -> Vec<String> {
        let mut v: Vec<String> = Hatch::ALL.iter().map(|h| h.label().to_string()).collect();
        for p in &self.patterns {
            if !v.iter().any(|n| n.eq_ignore_ascii_case(&p.name)) {
                v.push(p.name.clone());
            }
        }
        v
    }

    pub fn template(&self, name: &str) -> Option<&ViewTemplate> {
        self.templates.iter().find(|t| t.name == name)
    }

    /// The object style of a category, the one for its material first.
    pub fn object_style(&self, c: Category, material: Option<&str>) -> Option<&ObjectStyle> {
        let of = |m: Option<&str>| {
            self.objects.iter().find(|s| {
                s.category == c
                    && match (&s.material, m) {
                        (None, None) => true,
                        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                        _ => false,
                    }
            })
        };
        material.and_then(|m| of(Some(m))).or_else(|| of(None))
    }
}

/// The style packs a project holds.
/// The materials that name pattern `name`: for their cuts (by name, or
/// by a built-in hatch of that name) or their surfaces.
pub fn pattern_users(model: &Model, name: &str) -> Vec<(ElementId, String)> {
    model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Material(m) => {
                let named =
                    |n: &Option<String>| n.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(name));
                let by_hatch = m.pattern.is_none() && m.hatch.label().eq_ignore_ascii_case(name);
                (named(&m.pattern) || named(&m.surface_pattern) || by_hatch)
                    .then(|| (e.id, e.name.clone()))
            }
            _ => None,
        })
        .collect()
}

/// Is pack `new` pack `old` with only patterns changed that nothing draws
/// with: no material, no other pattern and no style names them (a tile
/// being drawn before a material takes it)? Such an edit changes no
/// drawing and nothing in 3D, so nothing is built again (owner
/// 2026-11-18: drawing a tile lagged, 700 ms a stroke rebuilding 3D).
pub fn only_unused_patterns_changed(model: &Model, old: &StylePack, new: &StylePack) -> bool {
    if old.patterns == new.patterns {
        return false;
    }
    let (mut a, mut b) = (old.clone(), new.clone());
    a.patterns.clear();
    b.patterns.clear();
    if a != b {
        return false;
    }
    let find =
        |ps: &[HatchPattern], n: &str| ps.iter().find(|p| p.name.eq_ignore_ascii_case(n)).cloned();
    let mut changed: Vec<String> = vec![];
    for p in old.patterns.iter().chain(new.patterns.iter()) {
        if find(&old.patterns, &p.name) != find(&new.patterns, &p.name)
            && !changed.iter().any(|c| c.eq_ignore_ascii_case(&p.name))
        {
            changed.push(p.name.clone());
        }
    }
    let rest = serde_json::to_string(&b).unwrap_or_default().to_lowercase();
    changed.iter().all(|n| {
        let named_by_pattern = old.patterns.iter().chain(new.patterns.iter()).any(|p| {
            p.small_scale
                .as_ref()
                .is_some_and(|s| s.pattern.eq_ignore_ascii_case(n))
        });
        pattern_users(model, n).is_empty()
            && !named_by_pattern
            && !rest.contains(&format!("\"{}\"", n.to_lowercase()))
    })
}

/// Rename pattern `old` to `new` in pack `pack` (its own patterns and
/// every style and pattern naming it) and in every material naming it,
/// as one transaction with the pack as it is to be saved.
pub fn rename_pattern(
    model: &Model,
    pack_id: Option<ElementId>,
    mut pack: StylePack,
    old: &str,
    new: &str,
) -> Transaction {
    let same = |n: &str| n.eq_ignore_ascii_case(old);
    let fix = |n: &mut Option<String>| {
        if n.as_deref().is_some_and(same) {
            *n = Some(new.to_string());
        }
    };
    for p in &mut pack.patterns {
        if same(&p.name) {
            p.name = new.to_string();
        }
        if let Some(s) = &mut p.small_scale {
            if same(&s.pattern) {
                s.pattern = new.to_string();
            }
        }
    }
    for s in &mut pack.objects {
        fix(&mut s.cut_pattern);
        fix(&mut s.surface_pattern);
    }
    let mut tx = Transaction::new("Rename pattern");
    match pack_id.and_then(|id| model.get(id)) {
        Some(e) => {
            let mut after = e.clone();
            after.kind = ElementKind::StylePack(Box::new(pack));
            tx.ops.push(crate::Op::Replace {
                before: e.clone(),
                after,
            });
        }
        None => tx.ops.extend(apply_pack(model, &pack).ops),
    }
    for e in model.iter() {
        if let ElementKind::Material(m) = &e.kind {
            let mut m2 = m.clone();
            fix(&mut m2.pattern);
            fix(&mut m2.surface_pattern);
            if &m2 != m {
                let mut after = e.clone();
                after.kind = ElementKind::Material(m2);
                tx.ops.push(crate::Op::Replace {
                    before: e.clone(),
                    after,
                });
            }
        }
    }
    tx
}

pub fn packs(model: &Model) -> Vec<(ElementId, &StylePack)> {
    model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::StylePack(p) => Some((e.id, p.as_ref())),
            _ => None,
        })
        .collect()
}

/// The pack a project draws with: the one marked default, else its first,
/// else the *Standard* pack.
pub fn active_pack(model: &Model) -> Cow<'_, StylePack> {
    let all = packs(model);
    all.iter()
        .find(|(_, p)| p.default)
        .or(all.first())
        .map_or_else(
            || Cow::Owned(StylePack::standard()),
            |(_, p)| Cow::Borrowed(*p),
        )
}

/// Load `pack` into the project and make it the default: a pack of the
/// same name is replaced, and the pack's text styles, title blocks and
/// mark rules replace the project's of the same names. One undo step.
pub fn apply_pack(model: &Model, pack: &StylePack) -> Transaction {
    let mut t = Transaction::new(format!("Apply style pack {}", pack.name));
    let mut pack = pack.clone();
    pack.default = true;
    let mut placed = false;
    for (id, p) in packs(model) {
        let before = model.get(id).expect("listed");
        let mut e = before.clone();
        if p.name == pack.name && !placed {
            e.kind = ElementKind::StylePack(Box::new(pack.clone()));
            e.name = pack.name.clone();
            placed = true;
        } else if p.default {
            let mut q = p.clone();
            q.default = false;
            e.kind = ElementKind::StylePack(Box::new(q));
        } else {
            continue;
        }
        t = t.replace(before.clone(), e);
    }
    if !placed {
        t = t.insert(Element::new(
            &pack.name,
            ElementKind::StylePack(Box::new(pack.clone())),
        ));
    }
    for s in &pack.text_styles {
        let found = model
            .iter()
            .find(|e| e.name == s.name && matches!(e.kind, ElementKind::TextStyle(_)));
        match found {
            Some(before) => {
                let mut e = before.clone();
                e.kind = ElementKind::TextStyle(s.style);
                t = t.replace(before.clone(), e);
            }
            None => t = t.insert(Element::new(&s.name, ElementKind::TextStyle(s.style))),
        }
    }
    for b in &pack.title_blocks {
        let found = model
            .iter()
            .find(|e| e.name == b.name && matches!(e.kind, ElementKind::SheetTemplate(_)));
        match found {
            Some(before) => {
                let mut e = before.clone();
                e.kind = ElementKind::SheetTemplate(b.template.clone());
                t = t.replace(before.clone(), e);
            }
            None => {
                t = t.insert(Element::new(
                    &b.name,
                    ElementKind::SheetTemplate(b.template.clone()),
                ))
            }
        }
    }
    if let Some(r) = &pack.mark_rules {
        for op in crate::marks::set_rules(model, r.clone()).ops {
            t.ops.push(op);
        }
    }
    t
}

/// Make the pack `id` the project's default.
pub fn make_default(model: &Model, id: ElementId) -> Transaction {
    let mut t = Transaction::new("Default style pack");
    for (pid, p) in packs(model) {
        if (pid == id) != p.default {
            let before = model.get(pid).expect("listed");
            let mut e = before.clone();
            let mut q = p.clone();
            q.default = pid == id;
            e.kind = ElementKind::StylePack(Box::new(q));
            t = t.replace(before.clone(), e);
        }
    }
    t
}

/// For serde: a view's look is left out of its file while it is the default.
#[allow(clippy::borrowed_box)]
pub fn boxed_look_is_default(l: &Box<ViewLook>) -> bool {
    l.is_default()
}

/// What a view adds to the pack's styles: its template, its own overrides
/// and its depth cues.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ViewLook {
    /// A [`ViewTemplate`] of the active pack, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// Applied after the template's, so the view's own win.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<Override>,
    #[serde(default, skip_serializing_if = "DepthCues::is_default")]
    pub cues: DepthCues,
    /// A pen set of the pack, by name, instead of its template's or the
    /// project's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pen_set: Option<String>,
    /// A line style by name, instead of its template's or the project's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_style: Option<String>,
    /// A palette by name, instead of its template's or the project's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
    /// Symbol picks of its own (its template's are laid in by
    /// [`resolved`]).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub symbols: crate::symbols::SymbolPicks,
    /// Set on the copy [`resolved`] makes, which has its template laid in
    /// already.
    #[serde(skip)]
    pub resolved: bool,
}

impl ViewLook {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The view's override of `target`, made if it has none.
    pub fn override_mut(&mut self, target: Target) -> &mut Override {
        if let Some(i) = self.overrides.iter().position(|o| o.target == target) {
            return &mut self.overrides[i];
        }
        self.overrides.push(Override::new(target));
        self.overrides.last_mut().expect("just pushed")
    }

    /// Drop overrides that change nothing.
    pub fn tidy(&mut self) {
        self.overrides.retain(|o| *o != Override::new(o.target));
    }
}

/// The view as it draws: its template's scale, style, hidden categories,
/// pens and cues laid under its own settings. A view without a template
/// takes its pack's default for its kind; a template the pack lacks is
/// ignored.
pub fn resolved<'a>(model: &Model, v: &'a View) -> Cow<'a, View> {
    if v.look.resolved {
        return Cow::Borrowed(v);
    }
    let pack = active_pack(model);
    let name = v
        .look
        .template
        .as_deref()
        .or_else(|| pack.default_templates.of(v));
    let Some(t) = name.and_then(|n| pack.template(n)) else {
        return Cow::Borrowed(v);
    };
    let mut out = v.clone();
    if let Some(s) = t.scale {
        if !matches!(v.kind, crate::ViewKind::Schedule { .. }) {
            out.scale = s;
        }
    }
    if let Some(s) = t.style {
        out.style = s;
    }
    for c in &t.hidden {
        if !out.hidden.hides_category(*c) {
            out.hidden.set_category(*c, true);
        }
    }
    let mut overrides = t.overrides.clone();
    overrides.extend(v.look.overrides.iter().cloned());
    out.look.overrides = overrides;
    if out.graphics.weights.is_none() {
        out.graphics.weights = t.weights;
    }
    if let Some(b) = t.below_fill {
        out.graphics.below_fill = b;
    }
    if let (Some(c), true) = (t.cues, v.look.cues.is_default()) {
        out.look.cues = c;
    }
    if out.look.pen_set.is_none() {
        out.look.pen_set = t.pen_set.clone();
    }
    if out.look.line_style.is_none() {
        out.look.line_style = t.line_style.clone();
    }
    if out.look.palette.is_none() {
        out.look.palette = t.palette.clone();
    }
    for (k, s) in &t.symbols {
        out.look
            .symbols
            .entry(k.clone())
            .or_insert_with(|| s.clone());
    }
    out.look.resolved = true;
    Cow::Owned(out)
}

/// The line style a view draws with: its own, its template's, else the
/// project's; `None` when that is Ruled, so plain drawings take no detour.
pub fn view_line_style(model: &Model, v: &View) -> Option<crate::linestyle::LineStyle> {
    let pack = active_pack(model);
    let v = resolved(model, v);
    let name = v
        .look
        .line_style
        .clone()
        .or_else(|| pack.line_style.clone())?;
    pack.line_style_named(&name).filter(|s| !s.is_ruled())
}

/// The palette a view draws with: its own, its template's, else the
/// project's; Ink when none is picked (decision 1).
pub fn view_palette(model: &Model, v: &View) -> crate::palette::Palette {
    let pack = active_pack(model);
    let v = resolved(model, v);
    v.look
        .palette
        .clone()
        .or_else(|| pack.palette.clone())
        .and_then(|n| pack.palette_named(&n))
        .unwrap_or_default()
}

/// The project's palette (the style pack's pick); Ink when none.
pub fn project_palette(model: &Model) -> crate::palette::Palette {
    let pack = active_pack(model);
    pack.palette
        .clone()
        .and_then(|n| pack.palette_named(&n))
        .unwrap_or_default()
}

impl StylePack {
    /// A palette by name: the pack's adjusted or own one first, else the
    /// shipped one.
    pub fn palette_named(&self, name: &str) -> Option<crate::palette::Palette> {
        self.palettes
            .iter()
            .find(|p| p.name == name)
            .cloned()
            .or_else(|| crate::palette::shipped(name))
    }

    /// Every palette it offers: the shipped ones in their order, then its
    /// own.
    pub fn palette_names(&self) -> Vec<String> {
        let mut out: Vec<String> = crate::palette::SHIPPED
            .iter()
            .map(|s| s.to_string())
            .collect();
        for p in &self.palettes {
            if !out.contains(&p.name) {
                out.push(p.name.clone());
            }
        }
        out
    }

    /// Put an adjusted palette in the pack; one equal to its shipped self
    /// is dropped, so "factory values" leaves nothing behind.
    pub fn put_palette(&mut self, p: crate::palette::Palette) {
        self.palettes.retain(|x| x.name != p.name);
        if crate::palette::shipped(&p.name).as_ref() != Some(&p) {
            self.palettes.push(p);
        }
    }

    /// A line style by name: the pack's adjusted or own one first, else the
    /// shipped one.
    pub fn line_style_named(&self, name: &str) -> Option<crate::linestyle::LineStyle> {
        self.line_styles
            .iter()
            .find(|s| s.name == name)
            .cloned()
            .or_else(|| crate::linestyle::shipped_style(name))
    }

    /// Every line style it offers: the shipped ones in their order, then its
    /// own.
    pub fn line_style_names(&self) -> Vec<String> {
        let mut out: Vec<String> = crate::linestyle::shipped()
            .into_iter()
            .map(|s| s.name)
            .collect();
        for s in &self.line_styles {
            if !out.contains(&s.name) {
                out.push(s.name.clone());
            }
        }
        out
    }

    /// Put an adjusted line style in the pack; one equal to its shipped
    /// self is dropped, so "factory values" leaves nothing behind.
    pub fn put_line_style(&mut self, s: crate::linestyle::LineStyle) {
        self.line_styles.retain(|x| x.name != s.name);
        if crate::linestyle::shipped_style(&s.name).as_ref() != Some(&s) {
            self.line_styles.push(s);
        }
    }
}

/// The pens a view draws with: its own, its template's, else its pack's
/// for its scale.
pub fn view_weights(model: &Model, v: &View) -> LineWeights {
    view_pens(model, v).weights
}

/// The scale table's row for a view: at the scale it is drawn and printed
/// at (a view's viewports on sheets take its scale).
pub fn view_rule(model: &Model, v: &View) -> ScaleRule {
    active_pack(model).rule_at(table_scale(v))
}

/// The scale a view reads its row of the scale table at: its own, but a
/// schedule (printed at no scale) takes the 1:100 row, not the lightest.
pub fn table_scale(v: &View) -> f64 {
    match v.kind {
        crate::ViewKind::Schedule { .. } => 100.0,
        _ => v.paper_scale(),
    }
}

/// Every pen's width in a view, the pack's own pens too: its pen set (its
/// own, its template's, else the project's) at its scale; pens of the
/// view's own replace the built-in ones.
pub fn view_pens(model: &Model, v: &View) -> PenWidths {
    let v = resolved(model, v);
    let scale = table_scale(&v);
    let mut p = active_pack(model).pens_at(v.look.pen_set.as_deref(), scale);
    if let Some(w) = v.graphics.weights {
        p.weights = w;
    }
    p
}

/// Give `views` the template `name` (none takes it off). One undo step.
pub fn set_template(model: &Model, views: &[ElementId], name: Option<&str>) -> Transaction {
    let mut t = Transaction::new(match name {
        Some(n) => format!("View template {n}"),
        None => "No view template".into(),
    });
    for id in views {
        let Some(e) = model.get(*id) else { continue };
        let ElementKind::View(v) = &e.kind else {
            continue;
        };
        if v.look.template.as_deref() == name {
            continue;
        }
        let mut after = e.clone();
        if let ElementKind::View(v) = &mut after.kind {
            v.look.template = name.map(str::to_string);
        }
        t = t.replace(e.clone(), after);
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;

    #[test]
    fn own_marker_styles_travel_with_the_pack() {
        let mut pack = StylePack::shipped().remove(0);
        let parts = MarkerParts {
            head: MarkerHead::RoundedBox,
            fill: MarkerFill::Filled,
            colour: MarkerColour::Accent,
            ..MarkerParts::default()
        };
        pack.marker_styles.push(NamedMarkerStyle {
            name: "Rounded green".into(),
            parts,
        });
        pack.marker_style = Some("Rounded green".into());
        let text = serde_json::to_string(&pack).unwrap();
        let back: StylePack = serde_json::from_str(&text).unwrap();
        assert_eq!(back.marker_parts(), Some(parts));
        assert_eq!(back.marker_label(), "Rounded green");
        // a pack saved before the designer still reads
        let mut old: serde_json::Value =
            serde_json::to_value(StylePack::shipped().remove(0)).unwrap();
        old.as_object_mut().unwrap().remove("marker_styles");
        assert!(serde_json::from_value::<StylePack>(old).is_ok());
    }

    #[test]
    fn shipped_packs_read_and_differ() {
        let packs = StylePack::shipped();
        assert_eq!(packs.len(), 2);
        let (a, b) = (&packs[0], &packs[1]);
        assert_eq!(a.name, "Standard");
        assert_eq!(b.name, "Light linework");
        assert_ne!(a.weights_at(100.0), b.weights_at(100.0));
        // the standard pack draws as CreaBIM did before packs
        assert_eq!(a.weights_at(50.0), LineWeights::for_scale(50.0));
        assert_eq!(a.weights_at(100.0), LineWeights::for_scale(100.0));
        assert_eq!(a.weights_at(500.0), LineWeights::for_scale(500.0));
        // a pack writes and reads back the same
        for p in &packs {
            assert_eq!(&StylePack::from_json(&p.to_json()).unwrap(), p);
        }
        // the light pack's templates are named like the standard's
        for t in &a.templates {
            assert!(b.template(&t.name).is_some(), "{}", t.name);
        }
    }

    #[test]
    fn every_old_hatch_names_a_pattern() {
        let p = StylePack::standard();
        for h in Hatch::ALL {
            let pat = p.pattern(h.label()).unwrap();
            assert_eq!(pat.name, h.label());
        }
        assert_eq!(builtin_pattern(Hatch::Empty).fill, None);
        assert!(builtin_pattern(Hatch::Solid).lines.is_empty());
        assert_eq!(builtin_pattern(Hatch::Cross).lines.len(), 2);
        assert!(p.pattern("No such pattern").is_none());
    }

    #[test]
    fn values_read_in_the_short_forms_packs_use() {
        let w: Weight = serde_json::from_str("\"Fine\"").unwrap();
        assert_eq!(w, Weight::Pen(PenName::Fine));
        let w: Weight = serde_json::from_str("0.35").unwrap();
        assert_eq!(w, Weight::Mm(0.35));
        let c: Rgb = serde_json::from_str("\"#ff8000\"").unwrap();
        assert_eq!(c.hex(), "#ff8000");
        let c: Rgb = serde_json::from_str("[0.0, 1.0, 0.0]").unwrap();
        assert_eq!(c.hex(), "#00ff00");
        assert!(serde_json::from_str::<Rgb>("\"red\"").is_err());
        let p: LinePattern = serde_json::from_str(r#"{"Custom":[3.0,1.0]}"#).unwrap();
        assert_eq!(p.dashes(), Some(vec![3.0, 1.0]));
        assert_eq!(LinePattern::Custom(vec![-1.0, 2.0]).dashes(), None);
        assert!(StylePack::from_json(r#"{"name":""}"#).is_err());
        let bad = r#"{"name":"x","patterns":[{"name":"p","lines":[{"spacing":0}]}]}"#;
        assert!(StylePack::from_json(bad).is_err());
    }

    #[test]
    fn applying_a_pack_makes_it_the_default_and_undoes() {
        let mut doc = Document::from_model(crate::starter::starter_house());
        assert_eq!(active_pack(doc.model()).name, "Standard");
        let light = StylePack::shipped().remove(1);
        doc.commit(apply_pack(doc.model(), &light)).unwrap();
        assert_eq!(active_pack(doc.model()).name, "Light linework");
        // back to the standard one: both are kept, one is the default
        let standard = StylePack::standard();
        doc.commit(apply_pack(doc.model(), &standard)).unwrap();
        assert_eq!(packs(doc.model()).len(), 2);
        assert_eq!(active_pack(doc.model()).name, "Standard");
        let light_id = packs(doc.model())
            .iter()
            .find(|p| p.1.name == "Light linework")
            .unwrap()
            .0;
        doc.commit(make_default(doc.model(), light_id)).unwrap();
        assert_eq!(active_pack(doc.model()).name, "Light linework");
        doc.undo();
        doc.undo();
        doc.undo();
        assert!(packs(doc.model()).is_empty());
        // a pack survives saving
        doc.commit(apply_pack(doc.model(), &light)).unwrap();
        let back = Model::from_json(&doc.model().to_json()).unwrap();
        assert_eq!(active_pack(&back).as_ref(), &{
            let mut l = light.clone();
            l.default = true;
            l
        });
    }

    #[test]
    fn templates_restyle_every_view_that_names_them() {
        let mut doc = Document::from_model(crate::starter::starter_house());
        let plans: Vec<ElementId> = doc
            .model()
            .iter()
            .filter(|e| {
                matches!(&e.kind, ElementKind::View(v) if matches!(v.kind, crate::ViewKind::Plan { .. }))
            })
            .map(|e| e.id)
            .collect();
        assert!(plans.len() >= 2);
        let mut pack = StylePack::standard();
        let mut t = ViewTemplate::new("Plan test");
        t.hidden = vec![Category::Objects];
        t.scale = Some(200.0);
        let mut o = Override::new(Target::Category(Category::Walls));
        o.halftone = true;
        t.overrides.push(o);
        pack.templates.push(t);
        doc.commit(apply_pack(doc.model(), &pack)).unwrap();
        doc.commit(set_template(doc.model(), &plans, Some("Plan test")))
            .unwrap();
        let view = |doc: &Document, id| match &doc.model().get(id).unwrap().kind {
            ElementKind::View(v) => v.clone(),
            _ => unreachable!(),
        };
        for id in &plans {
            let v = view(&doc, *id);
            let r = resolved(doc.model(), &v);
            assert_eq!(r.scale, 200.0);
            assert!(r.hidden.hides_category(Category::Objects));
            assert!(r.look.overrides[0].halftone);
        }
        // changing the template changes them all
        pack.templates.last_mut().unwrap().scale = Some(50.0);
        doc.commit(apply_pack(doc.model(), &pack)).unwrap();
        for id in &plans {
            let v = view(&doc, *id);
            assert_eq!(resolved(doc.model(), &v).scale, 50.0);
            assert_eq!(view_weights(doc.model(), &v), LineWeights::for_scale(50.0));
        }
    }

    #[test]
    fn detail_views_take_the_detail_template_else_their_parents_kind() {
        let m = crate::starter::starter_house();
        let (pid, parent) = m
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::View(v) if matches!(v.kind, crate::ViewKind::Plan { .. }) => {
                    Some((e.id, v.clone()))
                }
                _ => None,
            })
            .unwrap();
        let detail = View::callout(pid, &parent, glam::DVec2::ZERO, glam::DVec2::ONE, 20.0);
        let mut d = KindTemplates {
            plan: Some("Floor plan".into()),
            ..Default::default()
        };
        assert_eq!(d.of(&detail), Some("Floor plan"));
        d.detail = Some("Detail".into());
        assert_eq!(d.of(&detail), Some("Detail"));
        assert_eq!(d.of(&parent), Some("Floor plan"));
    }

    #[test]
    fn old_views_and_materials_read_with_no_look() {
        let old = r#"{"kind":{"Plan":{"level":"00000000-0000-0000-0000-000000000001","cut_height":1.2}},"scale":50.0}"#;
        let v: View = serde_json::from_str(old).unwrap();
        assert!(v.look.is_default());
        assert!(!serde_json::to_string(&v).unwrap().contains("look"));
        let m: crate::Material =
            serde_json::from_str(r#"{"hatch":"Brick","color":[1,0,0]}"#).unwrap();
        assert_eq!(m.pattern, None);
        assert!(!serde_json::to_string(&m).unwrap().contains("pattern"));
    }

    #[test]
    fn technical_is_the_pens_projects_always_drew_with() {
        let p = StylePack::standard();
        for scale in [20.0, 50.0, 75.0, 100.0, 200.0, 500.0] {
            assert_eq!(p.weights_at(scale), LineWeights::for_scale(scale));
        }
        // an old pack with Technical's tables loses them and draws the same
        let mut old = p.clone();
        old.pen_set = None;
        old.pens = shipped_set("Technical").unwrap().tables;
        let n = old.normalised();
        assert!(n.pens.is_empty() && n.pen_sets.is_empty());
        assert_eq!(n.weights_at(50.0), old.weights_at(50.0));
    }

    #[test]
    fn an_old_pack_of_its_own_pens_becomes_its_own_set_and_draws_the_same() {
        let mut old = StylePack::standard();
        old.pen_set = None;
        let mut t = shipped_set("Technical").unwrap().tables;
        t[0].weights.cut = 0.9;
        old.pens = t;
        let n = old.normalised();
        assert!(n.pens.is_empty());
        assert_eq!(n.pen_set.as_deref(), Some("Standard pens"));
        for scale in [50.0, 100.0] {
            assert_eq!(n.weights_at(scale), old.weights_at(scale));
        }
        assert!(StylePack::from_json(&n.to_json()).is_ok());
    }

    #[test]
    fn own_pens_draw_at_their_width_in_each_set_and_go_cleanly() {
        let mut p = StylePack::standard();
        p.pen_sets.push(shipped_set("Sketch").unwrap());
        let id = p.add_own_pen("Site boundary");
        p.pen_sets[0].tables[0].own.insert(id, 1.2);
        let mut walls = ObjectStyle::new(Category::Walls);
        walls.cut_weight = Some(Weight::Own { pen: id });
        p.objects.push(walls);
        assert_eq!(
            p.pens_at(Some("Sketch"), 50.0).mm(Weight::Own { pen: id }),
            1.2
        );
        // a set it has no width in yet draws it as the projection pen
        let tech = p.pens_at(Some("Technical"), 50.0);
        assert_eq!(tech.mm(Weight::Own { pen: id }), tech.weights.projection);
        assert_eq!(p.weight_label(Weight::Own { pen: id }), "Site boundary");
        assert_eq!(p.pen_users(Weight::Own { pen: id }, &[Category::Walls]), 1);
        // written and read back
        let back = StylePack::from_json(&p.to_json()).unwrap();
        assert_eq!(back, p);
        p.remove_own_pen(id);
        assert!(p.own_pens.is_empty());
        assert_eq!(
            p.object_style(Category::Walls, None).unwrap().cut_weight,
            None
        );
        assert!(p.pen_sets[0].tables.iter().all(|t| t.own.is_empty()));
    }

    #[test]
    fn a_view_or_its_template_picks_another_set() {
        let mut doc = Document::from_model(crate::starter::starter_house());
        let model = doc.model();
        let (vid, v) = model
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::View(v) if matches!(v.kind, crate::ViewKind::Plan { .. }) => {
                    Some((e.id, v.clone()))
                }
                _ => None,
            })
            .unwrap();
        let before = view_pens(model, &v);
        let mut v2 = v.clone();
        v2.look.pen_set = Some("Sketch".into());
        let sketch = view_pens(model, &v2);
        assert_ne!(before.weights, sketch.weights);
        let scale = resolved(model, &v2).paper_scale();
        assert_eq!(
            sketch.weights,
            shipped_set("Sketch").unwrap().widths_at(scale).weights
        );
        let _ = (vid, &mut doc);
    }

    #[test]
    fn renamed_built_in_pens_keep_their_place() {
        let mut p = StylePack::standard();
        p.pen_names
            .insert(PenName::Symbol, "Doors and windows".into());
        assert_eq!(
            p.weight_label(Weight::Pen(PenName::Symbol)),
            "Doors and windows"
        );
        let back = StylePack::from_json(&p.to_json()).unwrap();
        assert_eq!(back.pen_label(PenName::Symbol), "Doors and windows");
        assert_eq!(back.pen_label(PenName::Cut), "Cut pen");
    }
}

#[cfg(test)]
mod scale_table_tests {
    use super::*;

    #[test]
    fn each_scale_takes_its_own_row_of_pens_and_rules() {
        let pack = StylePack::shipped()[0].clone();
        // six rows of pens, thinning from a detail to a site plan
        let cuts: Vec<f64> = [5.0, 10.0, 20.0, 50.0, 100.0, 500.0]
            .iter()
            .map(|s| pack.weights_at(*s).cut)
            .collect();
        assert_eq!(cuts, vec![0.70, 0.60, 0.50, 0.50, 0.35, 0.25]);
        // a 1:8 detail takes the 1:10 row; 1:24 takes 1:50
        assert_eq!(pack.weights_at(8.0).cut, 0.60);
        assert_eq!(pack.rule_at(24.0).up_to, 50.0);
        // true size in details, plain hatches opening up, fill at 1:200
        assert_eq!(pack.rule_at(5.0).pattern, ScalePattern::TrueSize);
        assert_eq!(pack.rule_at(50.0).pattern, ScalePattern::Plain);
        assert_eq!(pack.rule_at(200.0).pattern, ScalePattern::Fill);
        assert!(pack.rule_at(5.0).hatch_spacing > pack.rule_at(100.0).hatch_spacing);
        // text keeps its size on paper from a detail to a plan
        assert_eq!(pack.rule_at(5.0).note_text, pack.rule_at(100.0).note_text);
        // a pack's own rows replace the shipped ones
        let mut own = pack.clone();
        own.scale_rules = vec![ScaleRule {
            up_to: 1e9,
            ..pack.rule_at(50.0)
        }];
        assert_eq!(own.rule_at(5.0).pattern, ScalePattern::Plain);
    }
}
