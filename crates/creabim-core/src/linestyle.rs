//! Line styles: how a drawing's lines are drawn, Ruled to hand-made (pens
//! plan stage 4, the reconciled Hand Line Studio styles, owner 2026-11-16).
//!
//! A line style never changes a line's weight (that is the pen set's) nor
//! its colour: ink stays black in every style (owner, 2026-11-14 and MU82).
//! It changes how strokes start, end and meet, how hatches are laid, and
//! later the lettering, leaders and ticks. The numbers are the Gemini
//! studios' own, kept as they were so "Reset to its factory values" brings
//! them back; `creabim_geom::handline` turns them into paper millimetres.

use serde::{Deserialize, Serialize};

use crate::annotation::Terminator;

/// How a stroke ends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineEnd {
    #[default]
    None,
    /// A pen blob where the nib rests.
    InkBlob,
    /// Thinning in and out.
    Taper,
    /// A small lift-off hook.
    Flick,
}

impl LineEnd {
    pub const ALL: [LineEnd; 4] = [
        LineEnd::None,
        LineEnd::InkBlob,
        LineEnd::Taper,
        LineEnd::Flick,
    ];
    pub fn label(self) -> &'static str {
        match self {
            LineEnd::None => "None",
            LineEnd::InkBlob => "Ink blob",
            LineEnd::Taper => "Taper",
            LineEnd::Flick => "Flick",
        }
    }
}

/// Which lettering a style writes in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lettering {
    /// CreaBIM Vectra.
    #[default]
    Cad,
    /// CreaBIM Tech Hand.
    ArchitectHand,
    /// CreaBIM Atelier Hand.
    DraftingSlant,
}

impl Lettering {
    pub const ALL: [Lettering; 3] = [
        Lettering::Cad,
        Lettering::ArchitectHand,
        Lettering::DraftingSlant,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Lettering::Cad => "CAD (Vectra)",
            Lettering::ArchitectHand => "Architect's hand (Tech Hand)",
            Lettering::DraftingSlant => "Drafting slant (Atelier Hand)",
        }
    }
}

/// How a note's leader runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeaderRun {
    #[default]
    Straight,
    /// Level under the text, then on to the point.
    Stepped,
    Curved,
}

impl LeaderRun {
    pub const ALL: [LeaderRun; 3] = [LeaderRun::Straight, LeaderRun::Stepped, LeaderRun::Curved];
    pub fn label(self) -> &'static str {
        match self {
            LeaderRun::Straight => "Straight",
            LeaderRun::Stepped => "Stepped",
            LeaderRun::Curved => "Curved",
        }
    }
}

fn d_wave() -> f64 {
    80.0
}
fn d_density() -> f64 {
    1.0
}
fn d_slash() -> Terminator {
    Terminator::Slash
}
fn d_arrow() -> Terminator {
    Terminator::Arrow
}
fn d_seed() -> u32 {
    1042
}

/// A line style: the 22 settings of the reconciled studios, by group.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineStyle {
    pub name: String,
    // Line
    #[serde(default)]
    pub overshoot: f64,
    #[serde(default)]
    pub wobble: f64,
    #[serde(default = "d_wave")]
    pub wobble_length: f64,
    #[serde(default)]
    pub end: LineEnd,
    #[serde(default)]
    pub end_amount: f64,
    #[serde(default)]
    pub pressure: f64,
    #[serde(default)]
    pub double_pass: f64,
    #[serde(default)]
    pub double_strength: f64,
    // Hatches
    #[serde(default)]
    pub hatch_spacing_jitter: f64,
    #[serde(default)]
    pub hatch_angle_jitter: f64,
    #[serde(default)]
    pub hatch_broken: f64,
    #[serde(default = "d_density")]
    pub hatch_density: f64,
    #[serde(default)]
    pub timber_x: bool,
    #[serde(default)]
    pub earth_weave: bool,
    // Colour: flat wash under the ink; the ink itself stays black
    #[serde(default)]
    pub wash: f64,
    // Lettering
    #[serde(default)]
    pub lettering: Lettering,
    #[serde(default)]
    pub letter_jitter: f64,
    // Leaders and ticks
    #[serde(default)]
    pub leader: LeaderRun,
    #[serde(default = "d_arrow")]
    pub leader_end: Terminator,
    #[serde(default = "d_slash")]
    pub tick: Terminator,
    /// Keeps a drawing's hand the same at every redraw and export.
    #[serde(default = "d_seed")]
    pub seed: u32,
}

impl LineStyle {
    /// The clean, plotted line: what drawings have always been.
    pub fn ruled() -> Self {
        LineStyle {
            name: RULED.into(),
            overshoot: 0.0,
            wobble: 0.0,
            wobble_length: 80.0,
            end: LineEnd::None,
            end_amount: 0.0,
            pressure: 0.0,
            double_pass: 0.0,
            double_strength: 0.0,
            hatch_spacing_jitter: 0.0,
            hatch_angle_jitter: 0.0,
            hatch_broken: 0.0,
            hatch_density: 1.0,
            timber_x: false,
            earth_weave: false,
            wash: 0.0,
            lettering: Lettering::Cad,
            letter_jitter: 0.0,
            leader: LeaderRun::Straight,
            leader_end: Terminator::Arrow,
            tick: Terminator::Slash,
            seed: 1042,
        }
    }

    /// Draws as Ruled: nothing for the hand engine to do.
    pub fn is_ruled(&self) -> bool {
        self.overshoot == 0.0
            && self.wobble == 0.0
            && self.pressure == 0.0
            && self.end == LineEnd::None
            && (self.double_pass == 0.0 || self.double_strength == 0.0)
            && self.hatch_spacing_jitter == 0.0
            && self.hatch_angle_jitter == 0.0
            && self.hatch_broken == 0.0
    }
}

/// The name of the plain style, and the default.
pub const RULED: &str = "Ruled";

/// The eight shipped styles (Mockup_Drawing_Styles_REV00): Ruled, Hand and
/// Loose (ours), then the five the Gemini studios added, all in black ink.
pub fn shipped() -> Vec<LineStyle> {
    let r = LineStyle::ruled();
    #[allow(clippy::too_many_arguments)]
    let s = |name: &str,
             line: (f64, f64, f64, LineEnd, f64, f64, f64, f64),
             hatch: (f64, f64, f64, f64, bool, bool),
             wash: f64,
             letters: (Lettering, f64),
             leaders: (LeaderRun, Terminator, Terminator),
             seed: u32| LineStyle {
        name: name.into(),
        overshoot: line.0,
        wobble: line.1,
        wobble_length: line.2,
        end: line.3,
        end_amount: line.4,
        pressure: line.5,
        double_pass: line.6,
        double_strength: line.7,
        hatch_spacing_jitter: hatch.0,
        hatch_angle_jitter: hatch.1,
        hatch_broken: hatch.2,
        hatch_density: hatch.3,
        timber_x: hatch.4,
        earth_weave: hatch.5,
        wash,
        lettering: letters.0,
        letter_jitter: letters.1,
        leader: leaders.0,
        leader_end: leaders.1,
        tick: leaders.2,
        seed,
    };
    use LeaderRun::*;
    use LineEnd::*;
    use Terminator::{Arrow, Dot, Slash, Slug};
    vec![
        r,
        s(
            "Hand",
            (3.1, 0.24, 68.0, InkBlob, 0.75, 0.24, 0.0, 0.0),
            (0.28, 3.5, 0.18, 1.05, false, false),
            0.0,
            (Lettering::ArchitectHand, 0.45),
            (Straight, Slug, Slug),
            2084,
        ),
        s(
            "Loose",
            (5.2, 0.42, 50.0, InkBlob, 0.85, 0.36, 0.45, 0.3),
            (0.42, 6.0, 0.4, 0.95, false, false),
            0.0,
            (Lettering::ArchitectHand, 0.6),
            (Curved, Slug, Slug),
            5150,
        ),
        s(
            "Vellum Graphite",
            (4.4, 0.14, 95.0, Taper, 0.85, 0.32, 0.65, 0.34),
            (0.22, 2.0, 0.26, 1.25, false, false),
            0.0,
            (Lettering::ArchitectHand, 0.3),
            (Stepped, Dot, Slash),
            3190,
        ),
        s(
            "Linen Ink",
            (1.8, 0.34, 46.0, Flick, 0.8, 0.48, 0.3, 0.2),
            (0.38, 5.5, 0.36, 1.15, false, false),
            0.0,
            (Lettering::DraftingSlant, 0.52),
            (Curved, Arrow, Dot),
            4721,
        ),
        s(
            "Marker & Wash",
            (2.3, 0.2, 72.0, InkBlob, 0.55, 0.26, 0.24, 0.22),
            (0.24, 2.5, 0.16, 1.12, true, true),
            0.88,
            (Lettering::ArchitectHand, 0.42),
            (Curved, Arrow, Slash),
            6233,
        ),
        s(
            "Field Bold",
            (1.9, 0.16, 84.0, InkBlob, 0.65, 0.22, 0.44, 0.52),
            (0.18, 1.8, 0.12, 1.32, true, true),
            0.52,
            (Lettering::ArchitectHand, 0.34),
            (Stepped, Slug, Slug),
            7381,
        ),
        s(
            "Millwork Study",
            (3.8, 0.3, 54.0, Flick, 0.72, 0.36, 0.52, 0.38),
            (0.34, 4.5, 0.28, 1.0, true, false),
            0.68,
            (Lettering::DraftingSlant, 0.55),
            (Curved, Arrow, Slash),
            8812,
        ),
    ]
}

/// A shipped style by name.
pub fn shipped_style(name: &str) -> Option<LineStyle> {
    shipped().into_iter().find(|s| s.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_styles_ruled_first_and_only_ruled_is_plain() {
        let all = shipped();
        assert_eq!(all.len(), 8);
        assert_eq!(all[0].name, RULED);
        assert!(all[0].is_ruled());
        assert!(all[1..].iter().all(|s| !s.is_ruled()));
        let mut names: Vec<_> = all.iter().map(|s| s.name.clone()).collect();
        names.dedup();
        assert_eq!(names.len(), 8);
        // a style written to a file reads back the same
        let hand = shipped_style("Hand").unwrap();
        let back: LineStyle = serde_json::from_str(&serde_json::to_string(&hand).unwrap()).unwrap();
        assert_eq!(back, hand);
    }
}
