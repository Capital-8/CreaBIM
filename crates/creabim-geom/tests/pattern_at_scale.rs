//! A paving's pattern is opened up where a drawing's scale would print it
//! as a black patch (owner's markup of Ridgeline House's site plan, where
//! the block-paved court was solid black at 1:120).

use creabim_core::site::Pattern;
use creabim_core::{ElementKind, View, ViewKind};
use creabim_geom::parts::{pattern_at_scale, PATTERN_MIN_MM};
use creabim_geom::plan::{Pen, PlanItem};
use glam::DVec2;

fn step(p: &Pattern) -> f64 {
    match *p {
        Pattern::Units { size, .. } => size.min_element(),
        Pattern::Lines { spacing, .. } | Pattern::Stipple { spacing } => spacing,
        Pattern::None => f64::INFINITY,
    }
}

/// Every pattern, at every scale a plan is drawn at, stands at least the
/// minimum apart on paper, and one that already does is left as it is.
#[test]
fn a_pattern_never_prints_closer_than_the_minimum() {
    let block = Pattern::Units {
        size: DVec2::new(0.2, 0.1),
        stagger: true,
        angle: 0.0,
    };
    let boards = Pattern::Lines {
        spacing: 0.14,
        angle: 90.0,
    };
    let gravel = Pattern::Stipple { spacing: 0.25 };
    for p in [&block, &boards, &gravel] {
        for scale in [20.0, 48.0, 50.0, 96.0, 100.0, 120.0, 200.0, 500.0] {
            let shown = pattern_at_scale(p, scale);
            let on_paper = step(&shown) * 1000.0 / scale;
            assert!(
                on_paper >= PATTERN_MIN_MM - 1e-9,
                "{p:?} at 1:{scale}: {on_paper} mm"
            );
            // a whole number of the pattern's own steps, so joints stay on joints
            let n = step(&shown) / step(p);
            assert!((n - n.round()).abs() < 1e-9, "{p:?} at 1:{scale}: {n}");
            if step(p) * 1000.0 / scale >= PATTERN_MIN_MM {
                assert_eq!(&shown, p, "at 1:{scale}");
            }
        }
    }
    // the proportions of a unit are kept: a 2:1 block stays 2:1
    let Pattern::Units { size, stagger, .. } = pattern_at_scale(&block, 120.0) else {
        panic!()
    };
    assert!((size.x / size.y - 2.0).abs() < 1e-9 && stagger);
}

/// On screen a pattern is painted as drawn while its lines stand eight
/// pixels apart; zoomed out it is opened up to that, by whole steps; and
/// a surface too small to hold two courses keeps no lines at all.
#[test]
fn a_pattern_opens_up_on_screen_as_the_zoom_goes_out() {
    use creabim_geom::parts::{pattern_on_screen, pattern_step, PATTERN_MIN_PX};
    let block = Pattern::Units {
        size: DVec2::new(0.6, 0.3),
        stagger: true,
        angle: 0.0,
    };
    let court = vec![vec![
        DVec2::new(0.0, 0.0),
        DVec2::new(12.0, 0.0),
        DVec2::new(12.0, 10.0),
        DVec2::new(0.0, 10.0),
    ]];
    let own = creabim_geom::parts::pattern_lines(&block, &court).len();
    // zoomed in: the drawing's own lines
    assert!(pattern_on_screen(&block, &court, 60.0).is_none());
    let mut last = own;
    for px in [20.0, 10.0, 5.0, 2.5] {
        let lines = pattern_on_screen(&block, &court, px).expect("opened up");
        assert!(
            !lines.is_empty() && lines.len() < last,
            "{} lines at {px} px a metre",
            lines.len()
        );
        last = lines.len();
        // no two parallel lines closer than the minimum: the courses
        let mut ys: Vec<f64> = lines
            .iter()
            .filter(|l| (l.0.y - l.1.y).abs() < 1e-9)
            .map(|l| l.0.y)
            .collect();
        ys.sort_by(f64::total_cmp);
        ys.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        assert!(
            ys.windows(2)
                .all(|w| (w[1] - w[0]) * px >= PATTERN_MIN_PX - 1e-6),
            "at {px} px a metre"
        );
    }
    // a thumbnail of the plan: the tint alone
    assert_eq!(pattern_on_screen(&block, &court, 0.5), Some(vec![]));
    assert!(pattern_step(&block) == 0.3);
}

/// Ridgeline House's site plan at 1:120: the parking court's block paving
/// is drawn with far fewer lines than the model's own 200 by 100 blocks
/// would make, none closer than the minimum, and the court's outline and
/// everything else on the plan stays.
#[test]
fn the_parking_court_is_not_a_black_patch_on_the_site_plan() {
    let m = creabim_core::ridgeline::ridgeline_house();
    let court = m.iter().find(|e| e.name == "Parking court").unwrap();
    let ElementKind::Site(s) = &court.kind else {
        panic!()
    };
    let creabim_core::SiteForm::Area {
        boundary, pattern, ..
    } = &s.form
    else {
        panic!()
    };
    let view = m
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::View(v) if e.name == "Site plan" => Some(v.clone()),
            _ => None,
        })
        .unwrap();
    let ViewKind::Plan { level, cut_height } = view.kind else {
        panic!()
    };
    let fine = |v: &View| {
        creabim_geom::ranges::plan(&m, v, level, cut_height)
            .items
            .iter()
            .filter(
                |i| matches!(i, PlanItem::Line { id, pen: Pen::Pattern, .. } if *id == court.id),
            )
            .count()
    };
    let own = creabim_geom::parts::pattern_lines(pattern, std::slice::from_ref(boundary)).len();
    let shown = fine(&view);
    assert!(own > 200, "the model's own pattern: {own} lines");
    assert!(
        shown > 0 && shown * 3 < own,
        "{shown} of {own} lines at 1:{}",
        view.scale
    );
    // at 1:48 every block is drawn
    let mut close = view.clone();
    close.scale = 48.0;
    assert_eq!(fine(&close), own);
}
