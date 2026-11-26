//! Drawing standards (standards plan, owner's decisions A on all four):
//! a named bundle a project draws to, its sheet sizes and the scales it
//! offers and how they are written. Standards are data, kept in the style
//! pack, so an office can make its own; ISO and US ship. Pickers show the
//! project's standard's sizes and scales, with the others under *Show all*.

use crate::Model;
use serde::{Deserialize, Serialize};

/// A paper size, mm, landscape (long side first).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaperSize {
    pub name: String,
    pub w: f64,
    pub h: f64,
}

/// A scale a standard offers: its ratio (48 is 1:48) and how drawings
/// write it (`1/4" = 1'-0"`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScaleStep {
    pub ratio: f64,
    pub label: String,
}

/// A drawing standard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrawingStandard {
    pub name: String,
    /// The sheet sizes its pickers offer first.
    pub sheets: Vec<PaperSize>,
    /// The size a new sheet takes.
    pub default_sheet: String,
    /// The scales its pickers offer, largest first.
    pub scales: Vec<ScaleStep>,
}

const INCH: f64 = 25.4;

fn paper(name: &str, w: f64, h: f64) -> PaperSize {
    PaperSize {
        name: name.into(),
        w,
        h,
    }
}

fn inches(name: &str, w: f64, h: f64) -> PaperSize {
    paper(name, w * INCH, h * INCH)
}

/// ISO: A sheets, scales as ratios. CreaBIM's behaviour before standards.
pub fn iso() -> DrawingStandard {
    DrawingStandard {
        name: "ISO".into(),
        sheets: vec![
            paper("A0", 1189.0, 841.0),
            paper("A1", 841.0, 594.0),
            paper("A2", 594.0, 420.0),
            paper("A3", 420.0, 297.0),
            paper("A4", 297.0, 210.0),
        ],
        default_sheet: "A3".into(),
        scales: [1.0, 2.0, 5.0, 10.0, 20.0, 25.0, 50.0, 100.0, 200.0, 500.0]
            .iter()
            .map(|r| ScaleStep {
                ratio: *r,
                label: format!("1:{r:.0}"),
            })
            .collect(),
    }
}

/// US (ANSI and ARCH sheets, inch-foot scales): for US offices.
pub fn us() -> DrawingStandard {
    let arch = |label: &str, ratio: f64| ScaleStep {
        ratio,
        label: label.into(),
    };
    DrawingStandard {
        name: "US".into(),
        sheets: vec![
            inches("ARCH E", 48.0, 36.0),
            inches("ARCH E1", 42.0, 30.0),
            inches("ARCH D", 36.0, 24.0),
            inches("ARCH C", 24.0, 18.0),
            inches("ARCH B", 18.0, 12.0),
            inches("ARCH A", 12.0, 9.0),
            inches("ANSI E", 44.0, 34.0),
            inches("ANSI D", 34.0, 22.0),
            inches("ANSI C", 22.0, 17.0),
            inches("ANSI B (tabloid)", 17.0, 11.0),
            inches("ANSI A (letter)", 11.0, 8.5),
        ],
        default_sheet: "ARCH D".into(),
        scales: vec![
            arch("12\" = 1'-0\"", 1.0),
            arch("6\" = 1'-0\"", 2.0),
            arch("3\" = 1'-0\"", 4.0),
            arch("1 1/2\" = 1'-0\"", 8.0),
            arch("1\" = 1'-0\"", 12.0),
            arch("3/4\" = 1'-0\"", 16.0),
            arch("1/2\" = 1'-0\"", 24.0),
            arch("3/8\" = 1'-0\"", 32.0),
            arch("1/4\" = 1'-0\"", 48.0),
            arch("3/16\" = 1'-0\"", 64.0),
            arch("1/8\" = 1'-0\"", 96.0),
            arch("3/32\" = 1'-0\"", 128.0),
            arch("1/16\" = 1'-0\"", 192.0),
            arch("1\" = 10'-0\"", 120.0),
            arch("1\" = 20'-0\"", 240.0),
            arch("1\" = 30'-0\"", 360.0),
            arch("1\" = 40'-0\"", 480.0),
            arch("1\" = 50'-0\"", 600.0),
            arch("1\" = 100'-0\"", 1200.0),
        ],
    }
}

/// The standards that ship, in their order.
pub fn shipped() -> Vec<DrawingStandard> {
    vec![iso(), us()]
}

impl DrawingStandard {
    /// How a drawing at 1:`ratio` writes its scale: the standard's own
    /// words when it offers that scale, else the ratio.
    pub fn scale_label(&self, ratio: f64) -> String {
        self.scales
            .iter()
            .find(|s| (s.ratio - ratio).abs() < 1e-6)
            .map_or_else(|| format!("1:{ratio:.0}"), |s| s.label.clone())
    }

    /// Does it offer 1:`ratio`?
    pub fn offers(&self, ratio: f64) -> bool {
        self.scales.iter().any(|s| (s.ratio - ratio).abs() < 1e-6)
    }

    /// Its size `name`.
    pub fn sheet(&self, name: &str) -> Option<&PaperSize> {
        self.sheets.iter().find(|p| p.name == name)
    }
}

/// The size, of every shipped standard, a `w` × `h` mm paper is, either
/// way up: the project's standard first.
pub fn size_name(first: &DrawingStandard, w: f64, h: f64) -> Option<String> {
    let (long, short) = (w.max(h), w.min(h));
    std::iter::once(first.clone())
        .chain(shipped())
        .flat_map(|s| s.sheets)
        .find(|p| (p.w - long).abs() < 0.5 && (p.h - short).abs() < 0.5)
        .map(|p| p.name)
}

/// Every shipped size by name, the project's standard first: for *Show
/// all*.
pub fn any_sheet(first: &DrawingStandard, name: &str) -> Option<PaperSize> {
    std::iter::once(first.clone())
        .chain(shipped())
        .flat_map(|s| s.sheets)
        .find(|p| p.name == name)
}

/// The project's standard: its style pack's, else the one its units
/// suggest, US for feet and inches, ISO for metric (MU136: a project in
/// ft-in listed 1:25, 1:50… and wrote 1:48 for 1/4" = 1'-0").
pub fn of(model: &Model) -> DrawingStandard {
    crate::styles::active_pack(model)
        .standard
        .clone()
        .unwrap_or_else(|| {
            if model.units().is_imperial() {
                us()
            } else {
                iso()
            }
        })
}

impl DrawingStandard {
    /// Its scale nearest 1:`ratio` (by how many times bigger or smaller).
    pub fn nearest(&self, ratio: f64) -> f64 {
        self.scales
            .iter()
            .map(|s| s.ratio)
            .min_by(|a, b| (a / ratio).ln().abs().total_cmp(&(b / ratio).ln().abs()))
            .unwrap_or(ratio)
    }
}

/// The views to move when the project goes from standard `from` to `to`:
/// each view drawn at one of `from`'s scales that `to` doesn't offer, to
/// `to`'s nearest (1:50 to 1/4" = 1'-0" going to US, and back). A view at a
/// scale of its own is left as it is. Before and after, for one
/// transaction with the standard's change (owner MU108: the views stayed
/// at 1:50 in a US project).
pub fn rescaled_views(
    model: &Model,
    from: &DrawingStandard,
    to: &DrawingStandard,
) -> Vec<(crate::Element, crate::Element)> {
    model
        .iter()
        .filter_map(|e| match &e.kind {
            crate::ElementKind::View(v) if from.offers(v.scale) && !to.offers(v.scale) => {
                let mut after = e.clone();
                if let crate::ElementKind::View(w) = &mut after.kind {
                    w.scale = to.nearest(v.scale);
                }
                Some((e.clone(), after))
            }
            _ => None,
        })
        .collect()
}

/// How the project writes a 1:`ratio` scale.
pub fn scale_label(model: &Model, ratio: f64) -> String {
    of(model).scale_label(ratio)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn going_to_us_moves_metric_views_to_the_nearest_us_scale_and_back() {
        let (iso, us) = (iso(), us());
        for (m, i) in [
            (50.0, 48.0),
            (100.0, 96.0),
            (20.0, 24.0),
            (200.0, 192.0),
            (10.0, 12.0),
        ] {
            assert_eq!(us.nearest(m), i, "1:{m}");
        }
        assert_eq!(iso.nearest(48.0), 50.0);
        let mut m = crate::sample::sample_house();
        let views: Vec<_> = m
            .iter()
            .filter(|e| matches!(&e.kind, crate::ElementKind::View(v) if v.scale == 50.0))
            .map(|e| e.id)
            .collect();
        assert!(!views.is_empty());
        // and a view at a scale of its own stays
        let own = views[0];
        let e = m.get(own).unwrap().clone();
        let mut a = e.clone();
        if let crate::ElementKind::View(v) = &mut a.kind {
            v.scale = 37.0;
        }
        crate::Transaction::new("own")
            .replace(e, a)
            .apply(&mut m)
            .unwrap();
        let moves = rescaled_views(&m, &iso, &us);
        let mut tx = crate::Transaction::new("to US");
        for (b, a) in moves {
            tx = tx.replace(b, a);
        }
        tx.apply(&mut m).unwrap();
        for id in &views[1..] {
            let crate::ElementKind::View(v) = &m.get(*id).unwrap().kind else {
                unreachable!()
            };
            assert_eq!(v.scale, 48.0);
        }
        let crate::ElementKind::View(v) = &m.get(own).unwrap().kind else {
            unreachable!()
        };
        assert_eq!(v.scale, 37.0);
    }

    #[test]
    fn us_scales_are_written_in_feet_and_inches_and_sheets_are_in_inches() {
        let us = us();
        assert_eq!(us.scale_label(48.0), "1/4\" = 1'-0\"");
        assert_eq!(us.scale_label(96.0), "1/8\" = 1'-0\"");
        // a scale it doesn't offer reads as a ratio
        assert_eq!(us.scale_label(50.0), "1:50");
        let d = us.sheet("ARCH D").unwrap();
        assert!((d.w - 914.4).abs() < 1e-9 && (d.h - 609.6).abs() < 1e-9);
        assert_eq!(size_name(&us, 609.6, 914.4).as_deref(), Some("ARCH D"));
        // an A3 sheet in a US project is still named
        assert_eq!(size_name(&us, 420.0, 297.0).as_deref(), Some("A3"));
    }

    #[test]
    fn iso_is_what_projects_drew_before_standards() {
        let iso = iso();
        assert_eq!(iso.scale_label(50.0), "1:50");
        assert_eq!(iso.default_sheet, "A3");
        let m = Model::default();
        assert_eq!(of(&m).name, "ISO");
        assert_eq!(scale_label(&m, 100.0), "1:100");
    }

    /// A project in feet and inches with no standard of its own lists and
    /// writes US scales; one its pack sets wins (MU136: ft-in showed 1:48).
    #[test]
    fn a_project_in_feet_and_inches_draws_to_us_scales() {
        let mut doc = crate::Document::from_model(Model::default());
        let ft = crate::Units {
            length: crate::units::LengthUnit::FeetInches,
            ..Default::default()
        };
        doc.commit(crate::units::set_units(doc.model(), ft))
            .unwrap();
        let m = doc.model();
        assert_eq!(of(m).name, "US");
        assert_eq!(scale_label(m, 48.0), "1/4\" = 1'-0\"");
    }

    #[test]
    fn a_pack_carries_its_standard() {
        let mut p = crate::styles::StylePack::standard();
        p.standard = Some(us());
        let back = crate::styles::StylePack::from_json(&p.to_json()).unwrap();
        assert_eq!(back.standard.unwrap().name, "US");
    }
}
