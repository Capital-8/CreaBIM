//! Annotation data: the kinds of dimension, text notes with leaders, and
//! named text styles that dimensions, tags and notes refer to.
//!
//! Styles are elements like materials, so they are saved, undone and listed
//! in the model. A missing or unset style falls back to [`TextStyle::default`],
//! which is how annotations looked before styles existed.

use crate::{element::HostPoint, ElementId, ElementKind, Model};
use glam::DVec2;
use serde::{Deserialize, Serialize};

/// What a [`crate::Dimension`] measures. Older files have no kind and read
/// as [`DimensionKind::Linear`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DimensionKind {
    /// The distance from `a` to `b`, its line `offset` to the left.
    #[default]
    Linear,
    /// The angle at `center` turning counter-clockwise from `a` to `b`, its
    /// arc `offset` from the vertex.
    Angular,
    /// The radius of the circle about `center` through `a`; its line runs
    /// on `offset` past the circle.
    Radius,
    /// The diameter of the circle about `center` through `a`.
    Diameter,
    /// The length of the arc about `center` from `a` counter-clockwise to
    /// `b`, its line `offset` outside the arc.
    ArcLength,
    /// A spot level: the height of `a` (a slab's top in a plan, or the
    /// plan's own level: a level marker), written at `b` from the
    /// dimension's pattern, such as "+0.45 FFL".
    Spot,
}

impl DimensionKind {
    pub const ALL: [DimensionKind; 6] = [
        DimensionKind::Linear,
        DimensionKind::Angular,
        DimensionKind::Radius,
        DimensionKind::Diameter,
        DimensionKind::ArcLength,
        DimensionKind::Spot,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DimensionKind::Linear => "Linear",
            DimensionKind::Angular => "Angle",
            DimensionKind::Radius => "Radius",
            DimensionKind::Diameter => "Diameter",
            DimensionKind::ArcLength => "Arc length",
            DimensionKind::Spot => "Spot level",
        }
    }

    /// Does this kind measure about a centre point?
    pub fn has_center(self) -> bool {
        !matches!(self, DimensionKind::Linear | DimensionKind::Spot)
    }

    pub fn is_linear(&self) -> bool {
        *self == DimensionKind::Linear
    }
}

/// A text style's typeface: the three every PDF has built in, and the
/// fonts CreaBIM ships (its own and the Classics, 2026-11-16), which go
/// inside every SVG and PDF that uses them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Font {
    /// Helvetica or Arial.
    #[default]
    Sans,
    /// Times.
    Serif,
    /// Courier.
    Mono,
    // CreaBIM's own
    Structa,
    Vectra,
    Spencer,
    Chisel,
    Kinetic,
    TechHand,
    AtelierHand,
    // the Classics
    Inter,
    NotoSans,
    IbmPlexSans,
    IbmPlexSansCondensed,
    IbmPlexMono,
    ArchivoNarrow,
    BarlowCondensed,
    Jost,
    Ubuntu,
    Osifont,
    ArchitectsDaughter,
    Kalam,
    PatrickHand,
}

/// Which list a font sits under in a picker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontGroup {
    /// In every PDF reader: nothing goes inside the file.
    Standard,
    CreaBim,
    Classic,
}

impl Font {
    pub const ALL: [Font; 23] = [
        Font::Sans,
        Font::Serif,
        Font::Mono,
        Font::Structa,
        Font::Vectra,
        Font::Spencer,
        Font::Chisel,
        Font::Kinetic,
        Font::TechHand,
        Font::AtelierHand,
        Font::Inter,
        Font::NotoSans,
        Font::IbmPlexSans,
        Font::IbmPlexSansCondensed,
        Font::IbmPlexMono,
        Font::ArchivoNarrow,
        Font::BarlowCondensed,
        Font::Jost,
        Font::Ubuntu,
        Font::Osifont,
        Font::ArchitectsDaughter,
        Font::Kalam,
        Font::PatrickHand,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Font::Sans => "Sans (Helvetica)",
            Font::Serif => "Serif (Times)",
            Font::Mono => "Mono (Courier)",
            _ => self.family(),
        }
    }

    /// The font's family name, as SVG and PDF name it.
    pub fn family(self) -> &'static str {
        match self {
            Font::Sans => "Helvetica",
            Font::Serif => "Times",
            Font::Mono => "Courier",
            Font::Structa => "CreaBIM Structa",
            Font::Vectra => "CreaBIM Vectra",
            Font::Spencer => "CreaBIM Spencer",
            Font::Chisel => "CreaBIM Chisel",
            Font::Kinetic => "CreaBIM Kinetic",
            Font::TechHand => "CreaBIM TechHand",
            Font::AtelierHand => "CreaBIM AtelierHand",
            Font::Inter => "Inter",
            Font::NotoSans => "Noto Sans",
            Font::IbmPlexSans => "IBM Plex Sans",
            Font::IbmPlexSansCondensed => "IBM Plex Sans Condensed",
            Font::IbmPlexMono => "IBM Plex Mono",
            Font::ArchivoNarrow => "Archivo Narrow",
            Font::BarlowCondensed => "Barlow Condensed",
            Font::Jost => "Jost",
            Font::Ubuntu => "Ubuntu derivative CreaBIM",
            Font::Osifont => "osifont",
            Font::ArchitectsDaughter => "Architects Daughter",
            Font::Kalam => "Kalam",
            Font::PatrickHand => "Patrick Hand",
        }
    }

    pub fn group(self) -> FontGroup {
        match self {
            Font::Sans | Font::Serif | Font::Mono => FontGroup::Standard,
            Font::Structa
            | Font::Vectra
            | Font::Spencer
            | Font::Chisel
            | Font::Kinetic
            | Font::TechHand
            | Font::AtelierHand => FontGroup::CreaBim,
            _ => FontGroup::Classic,
        }
    }

    /// Shipped with CreaBIM, so it goes inside the files that use it.
    pub fn bundled(self) -> bool {
        self.group() != FontGroup::Standard
    }
}

/// The mark at the end of a dimension line or a leader.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Terminator {
    /// A short heavy stroke at 45°: the architectural tick.
    #[default]
    Tick,
    /// A thin 45° slash.
    Slash,
    /// A solid arrowhead.
    Arrow,
    /// A slender solid arrowhead.
    ArrowSlim,
    /// An open arrowhead: two strokes.
    ArrowOpen,
    /// An arrowhead drawn in outline.
    ArrowClosed,
    /// A solid dot.
    Dot,
    /// A small circle.
    DotOpen,
    /// A small solid square.
    Square,
    /// A tick with a dot on the point.
    TickDot,
    /// A short thick stroke with round ends, as a pen leaves it: along a
    /// leader at its end, at 45° on a dimension (the hand line, pens
    /// stage 3).
    Slug,
    /// Nothing: the line just stops.
    None,
}

impl Terminator {
    pub const ALL: [Terminator; 12] = [
        Terminator::Tick,
        Terminator::Slash,
        Terminator::Arrow,
        Terminator::ArrowSlim,
        Terminator::ArrowOpen,
        Terminator::ArrowClosed,
        Terminator::Dot,
        Terminator::DotOpen,
        Terminator::Square,
        Terminator::TickDot,
        Terminator::Slug,
        Terminator::None,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Terminator::Tick => "Architectural tick",
            Terminator::Slash => "Thin slash",
            Terminator::Arrow => "Arrow, filled",
            Terminator::ArrowSlim => "Arrow, slender",
            Terminator::ArrowOpen => "Arrow, open",
            Terminator::ArrowClosed => "Arrow, outline",
            Terminator::Dot => "Dot",
            Terminator::DotOpen => "Open dot",
            Terminator::Square => "Square",
            Terminator::TickDot => "Tick and dot",
            Terminator::Slug => "Slug (pen stroke)",
            Terminator::None => "None",
        }
    }
}

/// The outline of a tag's bubble.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TagShape {
    /// Round for doors and windows, a box for the rest: the look tags
    /// had before shapes could be picked.
    #[default]
    Auto,
    Circle,
    Box,
    RoundedBox,
    /// A box with round ends.
    Pill,
    Hexagon,
    Octagon,
    Diamond,
    /// Just the text, underlined.
    Underline,
    /// Just the text.
    TextOnly,
}

impl TagShape {
    pub const ALL: [TagShape; 10] = [
        TagShape::Auto,
        TagShape::Circle,
        TagShape::Box,
        TagShape::RoundedBox,
        TagShape::Pill,
        TagShape::Hexagon,
        TagShape::Octagon,
        TagShape::Diamond,
        TagShape::Underline,
        TagShape::TextOnly,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TagShape::Auto => "Auto (round for openings)",
            TagShape::Circle => "Circle",
            TagShape::Box => "Box",
            TagShape::RoundedBox => "Rounded box",
            TagShape::Pill => "Pill",
            TagShape::Hexagon => "Hexagon",
            TagShape::Octagon => "Octagon",
            TagShape::Diamond => "Diamond",
            TagShape::Underline => "Underlined text",
            TagShape::TextOnly => "Text only",
        }
    }

    pub fn is_auto(&self) -> bool {
        *self == TagShape::Auto
    }
}

/// What kind of annotation a style is meant for. It sorts the style
/// manager and the pickers; any style can still be used by anything.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StyleUse {
    #[default]
    Any,
    Dimensions,
    Notes,
    Tags,
}

impl StyleUse {
    pub const ALL: [StyleUse; 4] = [
        StyleUse::Any,
        StyleUse::Dimensions,
        StyleUse::Notes,
        StyleUse::Tags,
    ];

    pub fn label(self) -> &'static str {
        match self {
            StyleUse::Any => "Any annotation",
            StyleUse::Dimensions => "Dimensions",
            StyleUse::Notes => "Notes and leaders",
            StyleUse::Tags => "Tags",
        }
    }

    pub fn is_any(&self) -> bool {
        *self == StyleUse::Any
    }
}

/// Which way a linear dimension measures: along the line between its
/// points, or only across the drawing's X or Y (the projected distance).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DimAxis {
    #[default]
    Aligned,
    /// The distance along X, with a horizontal dimension line.
    Horizontal,
    /// The distance along Y, with a vertical dimension line.
    Vertical,
}

impl DimAxis {
    pub const ALL: [DimAxis; 3] = [DimAxis::Aligned, DimAxis::Horizontal, DimAxis::Vertical];

    pub fn label(self) -> &'static str {
        match self {
            DimAxis::Aligned => "Aligned",
            DimAxis::Horizontal => "Horizontal",
            DimAxis::Vertical => "Vertical",
        }
    }

    pub fn is_aligned(&self) -> bool {
        *self == DimAxis::Aligned
    }

    /// The direction its line runs for points `a` to `b`, pointing from
    /// `a` towards `b`; none when they coincide along it.
    pub fn direction(self, a: DVec2, b: DVec2) -> Option<DVec2> {
        let d = b - a;
        let u = match self {
            DimAxis::Aligned => d,
            DimAxis::Horizontal => DVec2::new(d.x, 0.0),
            DimAxis::Vertical => DVec2::new(0.0, d.y),
        };
        (u.length() > 1e-9).then(|| u.normalize())
    }

    /// The axis a dimension line placed at `at` reads as, for points `a`
    /// and `b`, the way most CAD programs pick it: past both points to the
    /// left or right it measures up the drawing, above or below both it
    /// measures across, and between them it is aligned. Two points already
    /// level or plumb measure that way.
    pub fn pick(a: DVec2, b: DVec2, at: DVec2) -> DimAxis {
        let d = b - a;
        if d.y.abs() < 1e-9 {
            return DimAxis::Horizontal;
        }
        if d.x.abs() < 1e-9 {
            return DimAxis::Vertical;
        }
        let (lo, hi) = (a.min(b), a.max(b));
        let beside = at.x < lo.x || at.x > hi.x;
        let over = at.y < lo.y || at.y > hi.y;
        match (beside, over) {
            (true, false) => DimAxis::Vertical,
            (false, true) => DimAxis::Horizontal,
            _ => DimAxis::Aligned,
        }
    }
}

/// How annotation text (and its linework) looks on paper. The element's
/// name is the style's name. Beyond the text it sets the ends of
/// dimension lines and leaders and the shape of tags, so one style serves
/// as a dimension style, a note style or a tag style.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextStyle {
    #[serde(default)]
    pub font: Font,
    /// Letter height on paper, mm.
    pub size: f64,
    /// sRGB, 0..1. `None` uses the view's ink: black on paper, the usual
    /// annotation blue on screen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 3]>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    /// The ends of dimension lines; none is the tick of linear dimensions
    /// and the arrow of angles, radii and arcs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminator: Option<Terminator>,
    /// The end of a note's leader; none is a filled arrow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leader_end: Option<Terminator>,
    /// Size of those ends on paper, mm; none is the usual 2.5 mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_size: Option<f64>,
    #[serde(default, skip_serializing_if = "TagShape::is_auto")]
    pub tag_shape: TagShape,
    #[serde(default, skip_serializing_if = "StyleUse::is_any")]
    pub purpose: StyleUse,
    /// Dimension lines in this style leave a gap where a tag or a note
    /// sits across them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub break_at_tags: bool,
    /// How a note's or tag's lines line up (MU31).
    #[serde(default, skip_serializing_if = "TextAlign::is_usual")]
    pub align: TextAlign,
    /// A note's leader: one elbow, or a smooth curve to what it points at.
    #[serde(default, skip_serializing_if = "LeaderShape::is_straight")]
    pub leader_shape: LeaderShape,
}

/// What a linear dimension's text reads and where it sits (MU80, A on
/// all three): its own text instead of the measure (`<>` brings the measure
/// back), words before and after it, a short note below the line, and its
/// place along the line or moved off it with a leader.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DimText {
    /// Read `text` instead of the measure. Off keeps `text` for next time.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub instead: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub before: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub after: String,
    /// A line of smaller text under the dimension line.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub below: String,
    #[serde(default, skip_serializing_if = "DimPlace::is_centre")]
    pub place: DimPlace,
    /// Where moved text sits from its centred place, in the view's plane
    /// (metres), for [`DimPlace::Moved`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "DimLeader::is_when_outside")]
    pub leader: DimLeader,
    /// The text's grip shows, so this dimension's text can be dragged; off
    /// on the rest, so none is moved by mistake (MU81).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub movable: bool,
}

impl DimText {
    pub fn is_plain(&self) -> bool {
        *self == Self::default()
    }

    /// Its own text is shown instead of the measure.
    pub fn overridden(&self) -> bool {
        self.instead && !self.text.trim().is_empty()
    }

    /// The line the dimension reads, given the `measured` value.
    pub fn compose(&self, measured: &str) -> String {
        let main = if self.overridden() {
            self.text.replace("<>", measured)
        } else {
            measured.to_string()
        };
        [self.before.trim(), main.trim(), self.after.trim()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Where a dimension's text sits along its line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimPlace {
    #[default]
    Centre,
    /// Past the line's start, as read.
    Left,
    /// Past the line's end, as read.
    Right,
    /// Dragged off by its grip.
    Moved,
}

impl DimPlace {
    pub fn is_centre(&self) -> bool {
        *self == DimPlace::Centre
    }
}

/// When moved dimension text draws a leader back to its line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimLeader {
    /// Only when the text sits outside the witness lines.
    #[default]
    WhenOutside,
    Always,
    Never,
}

impl DimLeader {
    pub fn is_when_outside(&self) -> bool {
        *self == DimLeader::WhenOutside
    }
}

/// The shape of a note's leader (pens stage 3, the owner's markup).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeaderShape {
    /// Straight runs, bent at the shoulder and the elbow.
    #[default]
    Straight,
    /// A smooth curve from the shoulder to the end.
    Curved,
}

impl LeaderShape {
    pub fn is_straight(&self) -> bool {
        *self == LeaderShape::Straight
    }
}

/// How the lines of a note or tag line up: Usual is each kind's own
/// (notes from the left, room tags centred).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    #[default]
    Usual,
    Left,
    Centre,
    Right,
}

impl TextAlign {
    pub fn is_usual(&self) -> bool {
        *self == TextAlign::Usual
    }
}

/// Letter height of dimensions, tags and notes without a style, paper mm.
pub const DEFAULT_TEXT_SIZE: f64 = 2.5;

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font: Font::Sans,
            size: DEFAULT_TEXT_SIZE,
            color: None,
            bold: false,
            italic: false,
            terminator: None,
            leader_end: None,
            end_size: None,
            tag_shape: TagShape::Auto,
            purpose: StyleUse::Any,
            break_at_tags: false,
            align: TextAlign::Usual,
            leader_shape: LeaderShape::Straight,
        }
    }
}

impl TextStyle {
    pub fn validate(&self) -> Result<(), String> {
        if !(self.size.is_finite() && self.size > 0.1 && self.size < 100.0) {
            return Err(format!(
                "text size must be between 0.1 and 100 mm, got {}",
                self.size
            ));
        }
        if self
            .color
            .is_some_and(|c| c.iter().any(|v| !(0.0..=1.0).contains(v)))
        {
            return Err("text colour components must be within 0..1".into());
        }
        if self
            .end_size
            .is_some_and(|v| !(v.is_finite() && v > 0.1 && v < 50.0))
        {
            return Err("terminator size must be between 0.1 and 50 mm".into());
        }
        Ok(())
    }
}

/// The pattern a spot level is written with when it has none of its own.
pub const SPOT_PATTERN: &str = "{elevation}";

/// A spot level's text: `pattern` with `{elevation}` (signed, in the
/// project's units), `{m}` (signed metres to two places, "+0.45"), `{m3}`
/// (to three), `{mm}` (signed millimetres) and `{level}` (the name of the
/// level at or below it) filled in for the height `z`.
pub fn spot_text(model: &Model, pattern: &str, z: f64) -> String {
    let signed = |v: f64, places: usize| {
        let t = format!("{:.*}", places, v.abs());
        if t.trim_start_matches(['0', '.']).is_empty() {
            format!("±{t}")
        } else if v < 0.0 {
            format!("-{t}")
        } else {
            format!("+{t}")
        }
    };
    let level = model
        .levels()
        .filter(|(_, l)| l.elevation <= z + 1e-6)
        .max_by(|a, b| a.1.elevation.total_cmp(&b.1.elevation))
        .map(|(e, _)| e.name.clone())
        .unwrap_or_default();
    pattern
        .replace(
            "{elevation}",
            &crate::write_number(model, crate::NumberKind::SpotLevels, None, z),
        )
        .replace("{m3}", &signed(z, 3))
        .replace("{m}", &signed(z, 2))
        .replace("{mm}", &signed(z * 1000.0, 0))
        .replace("{level}", &level)
}

/// The style an annotation refers to, or the default when it refers to
/// none (or to one since deleted), with the annotation's own colour on top.
pub fn text_style(model: &Model, style: Option<ElementId>, color: Option<[f32; 3]>) -> TextStyle {
    let mut s = style
        .and_then(|id| match &model.get(id)?.kind {
            ElementKind::TextStyle(s) => Some(*s),
            _ => None,
        })
        .unwrap_or_else(|| TextStyle {
            font: crate::styles::active_pack(model)
                .text_font
                .unwrap_or_default(),
            ..TextStyle::default()
        });
    if color.is_some() {
        s.color = color;
    }
    s
}

/// The styles a project starts with, one for each kind of annotation, to
/// show how styles work and to duplicate into one's own (MU85).
pub fn starter_text_styles() -> Vec<(&'static str, TextStyle)> {
    vec![
        (
            "Dimensions · Technical",
            TextStyle {
                size: 2.5,
                terminator: Some(Terminator::Tick),
                purpose: StyleUse::Dimensions,
                ..TextStyle::default()
            },
        ),
        // the sections of the project's notes placed on sheets (MU88)
        (
            "General notes",
            TextStyle {
                size: 2.5,
                purpose: StyleUse::Notes,
                ..TextStyle::default()
            },
        ),
        (
            "Notes · Hand",
            TextStyle {
                font: Font::TechHand,
                size: 2.5,
                leader_end: Some(Terminator::Dot),
                leader_shape: LeaderShape::Curved,
                purpose: StyleUse::Notes,
                ..TextStyle::default()
            },
        ),
        (
            "Tags · Structa",
            TextStyle {
                font: Font::Structa,
                size: 2.5,
                tag_shape: TagShape::Hexagon,
                purpose: StyleUse::Tags,
                ..TextStyle::default()
            },
        ),
    ]
}

/// A text note in a view (drawing units) or on a sheet (paper mm), with an
/// optional leader ending in an arrow at `leader`. A leader point on an
/// element follows it, like a dimension point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    /// The view, or the sheet, it is written in.
    pub view: ElementId,
    /// Where the first line of text starts (its left end, mid-height).
    pub at: DVec2,
    /// The text; line breaks start new lines.
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leader: Option<HostPoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<ElementId>,
    /// Overrides the style's colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 3]>,
    /// A bend in its leader, in the same units as `at`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elbow: Option<DVec2>,
    /// The mark at the leader's end; none is its style's (an arrow unless
    /// the style says otherwise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leader_end: Option<Terminator>,
    /// A text box (MU26): lines wrap to this width, paper mm; none, each
    /// line runs as long as it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// Read the text as markdown: headings, bullets, numbers, checkboxes.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub markdown: bool,
    /// Drawn as a CreaBIM symbol placed with the Symbol tool, its text in
    /// the symbol's slot (Mockup_Symbols_REV00).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<PlacedSymbol>,
    /// Shows a section of the project's notes (its key, "general",
    /// "todos"…) instead of its own text, read as Markdown: one text on
    /// every sheet that shows it (Properties REV04, decision 9). Its own
    /// text stays, shown again when the link goes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    /// The block's height on paper, mm: where it stops when its text goes
    /// on in another block (long notes, Mockup_Long_Notes_REV00). None,
    /// it runs as long as its text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// A first block whose text goes on in the blocks that follow it
    /// (Continued); off, it shows all of it and they show nothing (Whole).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub continued: bool,
    /// A block that carries on another block's text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follows: Option<NoteFollow>,
    /// A first block's next parts go to its left; off, to its right (MU91:
    /// some read right to left, most left to right).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub continue_left: bool,
    /// The leader's point while the leader is turned off, so turning it
    /// back on points where it pointed before (MU135: it came back at a
    /// default spot, its direction lost).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leader_kept: Option<HostPoint>,
    /// Reads its words from an object: `{Field}` in its text is filled
    /// from what it is linked to (MU135, Mockup_Linked_Leaders_REV00).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reads: Option<NoteReads>,
}

fn yes() -> bool {
    true
}
fn is_true(b: &bool) -> bool {
    *b
}

/// What a note reads from an object: by default whatever its leader's
/// arrow is attached to, so moving the arrow onto another object reads
/// that one; with `follow` off, the object it was reading when it was
/// turned off.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoteReads {
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub follow: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed: Option<ElementId>,
    /// The read words in capitals, as notes on drawings often are.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub caps: bool,
}

impl Default for NoteReads {
    fn default() -> Self {
        Self {
            follow: true,
            fixed: None,
            caps: false,
        }
    }
}

/// The fields a note can read, by key and the word written in braces in
/// its text (`{Mark}`); either spelling, in any case, is read.
pub const NOTE_FIELDS: [(&str, &str); 8] = [
    ("name", "Name"),
    ("mark", "Mark"),
    ("ifc", "IFC class"),
    ("type", "Type"),
    ("material", "Material"),
    ("level", "Level"),
    ("size", "Size"),
    ("layers", "Layers"),
];

/// The field key a `{word}` names, if it names one.
pub fn note_field_key(word: &str) -> Option<&'static str> {
    let w = word.trim();
    NOTE_FIELDS
        .iter()
        .find(|(k, l)| k.eq_ignore_ascii_case(w) || l.eq_ignore_ascii_case(w))
        .map(|(k, _)| *k)
}

/// `text` with each `{Field}` filled by `value`; a field with no value
/// reads "—", and braces around anything else are left as typed.
pub fn fill_note_fields(text: &str, value: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after.find('}') {
            Some(j) => {
                let word = &after[..j];
                match note_field_key(word) {
                    Some(k) => out.push_str(&value(k).unwrap_or_else(|| "—".into())),
                    None => {
                        out.push('{');
                        out.push_str(word);
                        out.push('}');
                    }
                }
                rest = &after[j + 1..];
            }
            None => {
                out.push_str(&rest[i..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Where a block that carries on a note's text sits in its chain.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoteFollow {
    /// The first block of the chain.
    pub first: ElementId,
    /// Its place after the first: 1, 2…
    pub part: u32,
    #[serde(default)]
    pub title: PartTitle,
}

/// What a carried-on block says above its text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartTitle {
    None,
    #[default]
    Continued,
    Name,
    NameContinued,
}

impl PartTitle {
    pub const ALL: [PartTitle; 4] = [
        PartTitle::None,
        PartTitle::Continued,
        PartTitle::Name,
        PartTitle::NameContinued,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PartTitle::None => "None",
            PartTitle::Continued => "(continued)",
            PartTitle::Name => "The section's name",
            PartTitle::NameContinued => "Name (continued)",
        }
    }

    /// The title line for a section named `name`; None for no title.
    pub fn line(self, name: &str) -> Option<String> {
        let up = name.trim().to_uppercase();
        match self {
            PartTitle::None => None,
            PartTitle::Continued => Some("(continued)".into()),
            PartTitle::Name => Some(up),
            PartTitle::NameContinued => Some(format!("{up} (continued)")),
        }
    }
}

/// A symbol placed with the Symbol tool: its kind, its style (none is the
/// kind's pick in the view, else the kind's first) and its turn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlacedSymbol {
    /// `points`, `slopes`, `centerlines`, `materials` or `mep`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// Degrees, anticlockwise.
    #[serde(default)]
    pub angle: f64,
}

impl Note {
    /// A symbol of `kind` placed at `at`.
    pub fn symbol(view: ElementId, at: DVec2, kind: &str) -> Self {
        let mut n = Self::new(view, at, "");
        n.symbol = Some(PlacedSymbol {
            kind: kind.into(),
            style: None,
            angle: 0.0,
        });
        n
    }

    pub fn new(view: ElementId, at: DVec2, text: impl Into<String>) -> Self {
        Self {
            view,
            at,
            text: text.into(),
            leader: None,
            style: None,
            color: None,
            elbow: None,
            leader_end: None,
            width: None,
            markdown: false,
            symbol: None,
            section: None,
            height: None,
            continued: false,
            follows: None,
            continue_left: false,
            leader_kept: None,
            reads: None,
        }
    }

    /// The object a reading note reads: its arrow's host, or the one it
    /// was kept on when Follow the arrow was turned off.
    pub fn reads_from(&self) -> Option<ElementId> {
        let r = self.reads.as_ref()?;
        if r.follow {
            self.leader.and_then(|l| l.host)
        } else {
            r.fixed
        }
    }

    /// Turn the leader on or off. Off keeps its point; on brings that
    /// point back, or a new one at `fresh` when it never had one.
    pub fn set_leader(&mut self, on: bool, fresh: DVec2) {
        if on {
            if self.leader.is_none() {
                self.leader = Some(
                    self.leader_kept
                        .take()
                        .unwrap_or_else(|| HostPoint::free(fresh)),
                );
            }
        } else if let Some(l) = self.leader.take() {
            self.leader_kept = Some(l);
        }
    }

    /// A note showing the notes section `key`, wrapping at `width` mm.
    pub fn linked(view: ElementId, at: DVec2, key: &str, width: f64) -> Self {
        let mut n = Self::new(view, at, "");
        n.section = Some(key.into());
        n.width = Some(width);
        n.markdown = true;
        n
    }

    pub fn validate(&self) -> Result<(), String> {
        let leader_ok = self
            .leader
            .is_none_or(|p| p.local.is_finite() && p.body.is_none_or(|b| b.is_finite()));
        let symbol_ok = self.symbol.as_ref().is_none_or(|s| s.angle.is_finite())
            && self.height.is_none_or(|h| h.is_finite() && h > 0.0);
        if self.at.is_finite() && leader_ok && symbol_ok && self.elbow.is_none_or(|p| p.is_finite())
        {
            Ok(())
        } else {
            Err("note position must be finite".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Dimension, Element, Transaction};

    /// `{Field}` words are filled by name or key in any case; a missing
    /// value reads "—"; other braces stay as typed (MU135).
    #[test]
    fn a_note_fills_its_fields_and_leaves_other_braces() {
        let v = |k: &str| match k {
            "mark" => Some("P-03".to_string()),
            "size" => Some("Ø 14\"".to_string()),
            _ => None,
        };
        assert_eq!(
            fill_note_fields("{Size} piers · {mark} · {IFC class} · {x} · {Mark", v),
            "Ø 14\" piers · P-03 · — · {x} · {Mark"
        );
        assert_eq!(note_field_key(" ifc class "), Some("ifc"));
        assert_eq!(note_field_key("Colour"), None);
    }

    /// A leader turned off and on again points where it pointed before
    /// (MU135: it came back at a default spot); a note that never had one
    /// gets the fresh point.
    #[test]
    fn a_leader_turned_off_and_on_keeps_its_point() {
        let mut n = Note::new(ElementId::new(), DVec2::ZERO, "Piers");
        let aim = DVec2::new(2.0, 3.5);
        n.leader = Some(HostPoint::free(aim));
        n.set_leader(false, DVec2::new(-1.5, -1.0));
        assert!(n.leader.is_none());
        n.set_leader(true, DVec2::new(-1.5, -1.0));
        assert_eq!(n.leader, Some(HostPoint::free(aim)));
        assert!(n.leader_kept.is_none());
        // a kept point survives saving
        n.set_leader(false, DVec2::ZERO);
        let back: Note = serde_json::from_str(&serde_json::to_string(&n).unwrap()).unwrap();
        assert_eq!(back.leader_kept, Some(HostPoint::free(aim)));

        let mut fresh = Note::new(ElementId::new(), DVec2::ZERO, "New");
        fresh.set_leader(true, DVec2::new(-1.5, -1.0));
        assert_eq!(fresh.leader, Some(HostPoint::free(DVec2::new(-1.5, -1.0))));
    }

    #[test]
    fn a_dimension_reads_its_words_and_override() {
        let mut t = DimText::default();
        assert_eq!(t.compose("3810"), "3810");
        t.before = "±".into();
        t.after = "CLR".into();
        assert_eq!(t.compose("3810"), "± 3810 CLR");
        // typed text waits until Text instead is on, and <> keeps the measure
        t.text = "<> MIN".into();
        assert_eq!(t.compose("3810"), "± 3810 CLR");
        t.instead = true;
        assert_eq!(t.compose("3810"), "± 3810 MIN CLR");
        assert!(t.overridden());
        t = DimText {
            instead: true,
            text: "EQ".into(),
            ..Default::default()
        };
        assert_eq!(t.compose("1200"), "EQ");
        // a plain one writes nothing to the file
        let d = Dimension::linear(
            crate::ElementId::new(),
            crate::element::HostPoint::free(DVec2::ZERO),
            crate::element::HostPoint::free(DVec2::X),
            0.5,
        );
        assert!(!serde_json::to_string(&d).unwrap().contains("label"));
    }

    #[test]
    fn linear_dimensions_from_older_files_still_read() {
        let view = ElementId::new();
        let old = format!(
            r#"{{"view":"{}","a":{{"host":null,"local":[0.0,0.0],"from_end":false}},"b":{{"host":null,"local":[3.0,0.0],"from_end":false}},"offset":-1.5}}"#,
            view.0
        );
        let d: Dimension = serde_json::from_str(&old).unwrap();
        assert_eq!(d.kind, DimensionKind::Linear);
        assert_eq!(d.center, None);
        assert_eq!(d.style, None);
        assert_eq!(d.color, None);
        assert_eq!(d.offset, -1.5);
        assert_eq!(d.axis, DimAxis::Aligned);
        // and a linear dimension still writes nothing new
        let text = serde_json::to_string(&d).unwrap();
        for field in ["kind", "center", "style", "color", "axis"] {
            assert!(!text.contains(field), "{field} in {text}");
        }
        // a newer kind round-trips
        let mut r = d.clone();
        r.kind = DimensionKind::Radius;
        r.center = Some(HostPoint::free(DVec2::ONE));
        r.color = Some([1.0, 0.0, 0.0]);
        let back: Dimension = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(back, r);
        // so does a horizontal one
        let mut h = d.clone();
        h.axis = DimAxis::Horizontal;
        let back: Dimension = serde_json::from_str(&serde_json::to_string(&h).unwrap()).unwrap();
        assert_eq!(back.axis, DimAxis::Horizontal);
    }

    #[test]
    fn styles_from_older_files_read_with_the_old_ends_and_shapes() {
        let old = r#"{"font":"Serif","size":3.5,"bold":true}"#;
        let s: TextStyle = serde_json::from_str(old).unwrap();
        assert_eq!((s.terminator, s.leader_end, s.end_size), (None, None, None));
        assert_eq!((s.tag_shape, s.purpose), (TagShape::Auto, StyleUse::Any));
        // a default style writes none of the new fields
        let text = serde_json::to_string(&TextStyle::default()).unwrap();
        for field in [
            "terminator",
            "leader_end",
            "end_size",
            "tag_shape",
            "purpose",
        ] {
            assert!(!text.contains(field), "{field} in {text}");
        }
        let dots = TextStyle {
            terminator: Some(Terminator::Dot),
            leader_end: Some(Terminator::ArrowOpen),
            end_size: Some(1.5),
            tag_shape: TagShape::Hexagon,
            purpose: StyleUse::Tags,
            ..TextStyle::default()
        };
        let back: TextStyle = serde_json::from_str(&serde_json::to_string(&dots).unwrap()).unwrap();
        assert_eq!(back, dots);
        assert!(TextStyle {
            end_size: Some(0.0),
            ..dots
        }
        .validate()
        .is_err());
    }

    #[test]
    fn dimension_axes_are_picked_from_where_the_line_goes() {
        let (a, b) = (DVec2::ZERO, DVec2::new(4.0, 1.0));
        // above both points: across; beside them: up; between: aligned
        assert_eq!(
            DimAxis::pick(a, b, DVec2::new(2.0, 3.0)),
            DimAxis::Horizontal
        );
        assert_eq!(DimAxis::pick(a, b, DVec2::new(6.0, 0.5)), DimAxis::Vertical);
        assert_eq!(DimAxis::pick(a, b, DVec2::new(2.0, 0.6)), DimAxis::Aligned);
        assert_eq!(
            DimAxis::Horizontal.direction(a, b),
            Some(DVec2::X),
            "points the way a goes to b"
        );
        assert_eq!(DimAxis::Vertical.direction(b, a), Some(-DVec2::Y));
        assert_eq!(DimAxis::Vertical.direction(a, DVec2::X), None);
    }

    #[test]
    fn tags_from_older_files_still_read() {
        let (view, host) = (ElementId::new(), ElementId::new());
        let old = format!(
            r#"{{"view":"{}","host":"{}","offset":[1.0,2.0]}}"#,
            view.0, host.0
        );
        let t: crate::Tag = serde_json::from_str(&old).unwrap();
        assert_eq!(t, crate::Tag::new(view, host, DVec2::new(1.0, 2.0)));
    }

    #[test]
    fn styles_resolve_with_fallbacks_and_undo() {
        let mut doc = crate::Document::new();
        let style = Element::new(
            "Titles",
            ElementKind::TextStyle(TextStyle {
                font: Font::Serif,
                size: 5.0,
                color: Some([0.8, 0.1, 0.1]),
                bold: true,
                italic: false,
                ..TextStyle::default()
            }),
        );
        let id = style.id;
        doc.commit(Transaction::new("New text style").insert(style))
            .unwrap();
        let s = text_style(doc.model(), Some(id), None);
        assert_eq!((s.font, s.size, s.bold), (Font::Serif, 5.0, true));
        // an element's colour wins over its style's
        let blue = [0.0, 0.0, 1.0];
        assert_eq!(
            text_style(doc.model(), Some(id), Some(blue)).color,
            Some(blue)
        );
        // no style, or a deleted one, is the old look
        assert_eq!(text_style(doc.model(), None, None), TextStyle::default());
        doc.undo();
        assert_eq!(
            text_style(doc.model(), Some(id), None),
            TextStyle::default()
        );
        doc.redo();
        assert_eq!(text_style(doc.model(), Some(id), None).size, 5.0);
        // sizes are checked
        let bad = Element::new(
            "Bad",
            ElementKind::TextStyle(TextStyle {
                size: 0.0,
                ..TextStyle::default()
            }),
        );
        assert!(doc.commit(Transaction::new("Bad").insert(bad)).is_err());
    }

    #[test]
    fn notes_follow_their_view_and_round_trip() {
        let mut doc = crate::Document::from_model(crate::sample::sample_house());
        let plan = doc
            .model()
            .iter()
            .find(|e| matches!(e.kind, ElementKind::View(_)))
            .unwrap()
            .id;
        let mut note = Note::new(plan, DVec2::new(1.0, 2.0), "Line one\nLine two");
        note.leader = Some(HostPoint::free(DVec2::ZERO));
        let e = Element::new("Note", ElementKind::Note(note.clone()));
        let id = e.id;
        doc.commit(Transaction::new("Add note").insert(e)).unwrap();
        let text = serde_json::to_string(doc.model().get(id).unwrap()).unwrap();
        let back: Element = serde_json::from_str(&text).unwrap();
        assert_eq!(back.kind, ElementKind::Note(note));
        // it depends on its view: deleting the view takes the note with it
        let tx = Transaction::new("Delete view").remove_cascade(doc.model(), plan);
        doc.commit(tx).unwrap();
        assert!(doc.model().get(id).is_none());
        doc.undo();
        assert!(doc.model().get(id).is_some());
        // a view can leave notes out, like dimensions and tags
        let mut hidden = crate::visibility::ViewHidden::default();
        assert!(!hidden.hidden_set(doc.model()).contains(&id));
        hidden.set_category(crate::visibility::Category::Notes, true);
        assert!(hidden.hidden_set(doc.model()).contains(&id));
    }
}
