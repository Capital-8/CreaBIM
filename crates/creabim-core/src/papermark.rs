//! Paper marks (MU22): what belongs to a sheet rather than to a view. A
//! north arrow, turned to the project's north, and a scale bar for a
//! drawing scale, placed in sheet millimetres. Text on a sheet is a
//! [`crate::Note`] written on it.

use crate::ElementId;
use glam::DVec2;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PaperMarkKind {
    /// Points to north: the project's true north when `rotation` is none,
    /// else turned this many degrees anticlockwise from the sheet's up.
    NorthArrow {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rotation: Option<f64>,
        /// Its look from the symbol bank (a style id); none follows the
        /// project's pick, "creabim" is CreaBIM's own drawing
        /// (Mockup_Sheet_Symbols_REV00).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style: Option<String>,
    },
    /// A bar `length` metres long at 1:`scale`, in `parts` alternating
    /// black and white parts, numbered in metres.
    ScaleBar {
        scale: f64,
        length: f64,
        parts: u8,
        /// Counted in feet: its length a round number of feet, its
        /// numbers in feet (an imperial project's bars; owner, after MU92).
        /// `length` stays in metres.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        feet: bool,
        /// Its look from the symbol bank (a style id); none follows the
        /// project's pick, "creabim" is CreaBIM's own drawing
        /// (Mockup_Sheet_Symbols_REV00).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style: Option<String>,
    },
    /// A revision cloud round a change, `w` by `h` mm about `at`, with its
    /// revision's letter, date and what changed (Annotate board step 4).
    Cloud {
        w: f64,
        h: f64,
        rev: String,
        #[serde(default)]
        date: String,
        #[serde(default)]
        what: String,
        /// A freeform outline, offsets from `at` in mm (MU25); empty, the
        /// `w` by `h` box.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        points: Vec<DVec2>,
        /// The scallops' length along the outline, mm.
        #[serde(default = "arc_default")]
        arc: f64,
    },
    /// The sheet's revisions: one row per revision cloud on it, from `at`
    /// (its lower left), `size` mm text.
    RevisionTable,
    /// A picture on the sheet (MU40: collages, site context, mood
    /// boards): `w` by `h` mm about `at`, its pixels kept in the project
    /// as a master's are (RGBA rows from the top, deflated, base64).
    Picture {
        w: f64,
        h: f64,
        width: u32,
        height: u32,
        rgba: String,
        #[serde(default = "one")]
        opacity: f64,
    },
    /// A vector logo or drawing from an SVG, crisp at any size: its shapes
    /// in a unit box (as a master's logo), `w` by `h` mm about `at`.
    Vector {
        w: f64,
        h: f64,
        shapes: Vec<crate::titleblock::TitleItem>,
    },
}

fn one() -> f64 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaperMark {
    /// The sheet it is on.
    pub sheet: ElementId,
    /// Where it is, sheet millimetres from the lower left: a north arrow's
    /// middle, a scale bar's left end.
    pub at: DVec2,
    /// A north arrow's height, a scale bar's thickness, millimetres.
    pub size: f64,
    pub kind: PaperMarkKind,
    /// Its own colour (sRGB, 0..1); black without one (MU25).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 3]>,
    /// The mark it was copied from with Placed on; none on the first. The
    /// copies are ordinary marks: moved and styled one by one, listed
    /// together (Mockup_Copy_To_Sheets_REV00).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy_of: Option<ElementId>,
    /// The drawing (a viewport on its sheet) it belongs to: a scale bar
    /// reads that drawing's scale, so a 1:5 detail beside a 1:50 section
    /// gets its own bar (owner, after MU91).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measures: Option<ElementId>,
}

fn arc_default() -> f64 {
    6.0
}

impl PaperMark {
    pub fn north_arrow(sheet: ElementId, at: DVec2) -> Self {
        Self {
            sheet,
            at,
            size: 18.0,
            kind: PaperMarkKind::NorthArrow {
                rotation: None,
                style: None,
            },
            color: None,
            copy_of: None,
            measures: None,
        }
    }

    /// A scale bar for 1:`scale`, its length a round number of metres
    /// about 60 mm long on paper.
    pub fn scale_bar(sheet: ElementId, at: DVec2, scale: f64) -> Self {
        let scale = if scale > 0.0 { scale } else { 100.0 };
        Self {
            sheet,
            at,
            size: 2.0,
            kind: PaperMarkKind::ScaleBar {
                scale,
                length: round_length(0.06 * scale),
                parts: 5,
                feet: false,
                style: None,
            },
            color: None,
            copy_of: None,
            measures: None,
        }
    }

    /// A revision cloud `w` by `h` about `at`.
    pub fn cloud(sheet: ElementId, at: DVec2, rev: &str) -> Self {
        Self {
            sheet,
            at,
            size: 4.0,
            kind: PaperMarkKind::Cloud {
                w: 60.0,
                h: 40.0,
                rev: rev.into(),
                date: String::new(),
                what: String::new(),
                points: vec![],
                arc: 6.0,
            },
            color: None,
            copy_of: None,
            measures: None,
        }
    }

    /// The revisions table, from `at`.
    pub fn revision_table(sheet: ElementId, at: DVec2) -> Self {
        Self {
            sheet,
            at,
            size: 2.5,
            kind: PaperMarkKind::RevisionTable,
            color: None,
            copy_of: None,
            measures: None,
        }
    }

    /// The first of its copies (itself when it is the first).
    pub fn first(&self, me: ElementId) -> ElementId {
        self.copy_of.unwrap_or(me)
    }

    /// A scale bar at 1:`scale`: its paper length kept, its length in
    /// metres a round number again. Other marks unchanged.
    pub fn at_scale(&self, scale: f64) -> Self {
        let mut m = self.clone();
        if let PaperMarkKind::ScaleBar {
            scale: s,
            length,
            feet,
            ..
        } = &mut m.kind
        {
            if scale > 0.0 && (scale - *s).abs() > 1e-9 {
                let paper = *length / s.max(1.0);
                *length = round_length_in(paper * scale * 1.0001, *feet);
                *s = scale;
            }
        }
        m
    }

    /// A scale bar counted in feet (or metres again), its length rounded
    /// to whole feet (or metres) about the same paper length.
    pub fn in_feet(&self, feet: bool) -> Self {
        let mut m = self.clone();
        if let PaperMarkKind::ScaleBar {
            length, feet: f, ..
        } = &mut m.kind
        {
            if *f != feet {
                *f = feet;
                *length = round_length_in(*length * 1.0001, feet);
            }
        }
        m
    }

    /// Its extent on the sheet, millimetres (lower left, upper right).
    pub fn bounds(&self) -> (DVec2, DVec2) {
        match &self.kind {
            PaperMarkKind::Cloud { arc, .. } => {
                let (lo, hi) = self.cloud_outline().iter().fold(
                    (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)),
                    |(a, b), p| (a.min(*p), b.max(*p)),
                );
                (
                    lo - DVec2::splat(arc / 2.0),
                    hi + DVec2::splat(arc / 2.0) + DVec2::new(8.0, 8.0),
                )
            }
            PaperMarkKind::RevisionTable => {
                (self.at, self.at + DVec2::new(110.0, self.size * 2.4 * 5.0))
            }
            PaperMarkKind::Picture { w, h, .. } | PaperMarkKind::Vector { w, h, .. } => {
                let half = DVec2::new(*w, *h) / 2.0;
                (self.at - half, self.at + half)
            }
            PaperMarkKind::NorthArrow { .. } => {
                let r = self.size * 0.6;
                (self.at - DVec2::splat(r), self.at + DVec2::new(r, r + 4.0))
            }
            PaperMarkKind::ScaleBar { scale, length, .. } => {
                let w = length * 1000.0 / scale.max(1.0);
                (
                    self.at - DVec2::new(1.0, 5.0),
                    self.at + DVec2::new(w + 8.0, self.size + 5.0),
                )
            }
        }
    }

    /// A revision cloud's outline on the sheet, anticlockwise: its
    /// freeform points, else the corners of its box.
    pub fn cloud_outline(&self) -> Vec<DVec2> {
        let PaperMarkKind::Cloud { w, h, points, .. } = &self.kind else {
            return vec![];
        };
        let mut out: Vec<DVec2> = if points.len() >= 3 {
            points.iter().map(|p| self.at + *p).collect()
        } else {
            let (x, y) = (w / 2.0, h / 2.0);
            [(-x, -y), (x, -y), (x, y), (-x, y)]
                .iter()
                .map(|(a, b)| self.at + DVec2::new(*a, *b))
                .collect()
        };
        let area: f64 = (0..out.len())
            .map(|i| out[i].perp_dot(out[(i + 1) % out.len()]))
            .sum();
        if area < 0.0 {
            out.reverse();
        }
        out
    }

    /// The angle a north arrow points, degrees anticlockwise from up.
    pub fn north_angle(&self, true_north: f64) -> f64 {
        match self.kind {
            PaperMarkKind::NorthArrow { rotation, .. } => rotation.unwrap_or(true_north),
            _ => 0.0,
        }
    }
}

/// [`round_length`] in feet when `feet` (metres in and out).
pub fn round_length_in(m: f64, feet: bool) -> f64 {
    if feet {
        round_length(m / 0.3048) * 0.3048
    } else {
        round_length(m)
    }
}

/// A length as 1, 2 or 5 times a power of ten, the nearest at or under it.
pub fn round_length(m: f64) -> f64 {
    if m <= 0.0 || !m.is_finite() {
        return 1.0;
    }
    let p = 10f64.powf(m.log10().floor());
    let f = m / p;
    let step = if f >= 5.0 {
        5.0
    } else if f >= 2.0 {
        2.0
    } else {
        1.0
    };
    step * p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cloud_follows_its_own_points_either_way_round() {
        let mut m = PaperMark::cloud(ElementId::new(), DVec2::new(100.0, 100.0), "A");
        assert_eq!(m.cloud_outline().len(), 4);
        if let PaperMarkKind::Cloud { points, .. } = &mut m.kind {
            // clockwise, an L of five points
            *points = vec![
                DVec2::new(0.0, 0.0),
                DVec2::new(0.0, 40.0),
                DVec2::new(20.0, 40.0),
                DVec2::new(20.0, 20.0),
                DVec2::new(50.0, 0.0),
            ];
        }
        let o = m.cloud_outline();
        assert_eq!(o.len(), 5);
        let area: f64 = (0..5).map(|i| o[i].perp_dot(o[(i + 1) % 5])).sum();
        assert!(area > 0.0, "turned anticlockwise, so scallops bulge out");
        let (lo, hi) = m.bounds();
        assert!(lo.x < 100.0 && hi.x > 150.0 && hi.y > 140.0);
    }

    #[test]
    fn a_scale_bar_is_a_round_length_about_six_centimetres_long() {
        let m = PaperMark::scale_bar(ElementId::new(), DVec2::ZERO, 100.0);
        let PaperMarkKind::ScaleBar { length, .. } = m.kind else {
            panic!()
        };
        assert_eq!(length, 5.0);
        // read at 1:5 it keeps about its paper length, in round metres
        let PaperMarkKind::ScaleBar { scale, length, .. } = m.at_scale(5.0).kind else {
            panic!()
        };
        assert_eq!((scale, length), (5.0, 0.2));
        // in feet: 5 m (16.4 ft) becomes 10 ft
        let PaperMarkKind::ScaleBar { length, feet, .. } = m.in_feet(true).kind else {
            panic!()
        };
        assert!(feet && (length - 3.048).abs() < 1e-9);
        assert_eq!(round_length(12.0), 10.0);
        assert_eq!(round_length(3.0), 2.0);
        let (lo, hi) = m.bounds();
        assert!(hi.x - lo.x > 50.0 && hi.x - lo.x < 70.0);
    }
}
