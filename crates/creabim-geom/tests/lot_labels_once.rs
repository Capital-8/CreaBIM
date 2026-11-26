//! A property's bearing and setback labels are written once on each
//! edge in a plan, whatever its view sets (owner, MU98_02 and MU98_06:
//! "why do we see double or overlapping info?").

use creabim_core::{Element, ElementKind, Level, Model, Site, SiteForm, SiteView, View, ViewKind};
use creabim_geom::plan::PlanItem;
use glam::DVec2;

fn model() -> (Model, Element, Element) {
    let lv = Element::new(
        "Ground Floor",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let mut p = Site::property(
        lv.id,
        creabim_core::polygon::rectangle(DVec2::new(0.0, 0.0), DVec2::new(18.32, 22.88)),
    );
    if let SiteForm::Property { rules, .. } = &mut p.form {
        rules.front = 6.0;
        rules.side = 3.2;
        rules.rear = 6.0;
        rules.set_note(3, "asdfasdf");
    }
    let lot = Element::new("Property", ElementKind::Site(p));
    let m = Model::from_elements([lv.clone(), lot.clone()]).unwrap();
    (m, lv, lot)
}

fn texts(m: &Model, lv: &Element, lot: &Element, sv: SiteView) -> Vec<String> {
    let mut v = View::plan(lv.id);
    v.site = sv;
    let ViewKind::Plan { level, cut_height } = v.kind else {
        unreachable!()
    };
    creabim_geom::ranges::plan(m, &v, level, cut_height)
        .items
        .into_iter()
        .filter_map(|it| match it {
            PlanItem::Text { id, text, .. } if id == lot.id => Some(text),
            _ => None,
        })
        .collect()
}

fn places(m: &Model, lv: &Element, lot: &Element, sv: SiteView) -> Vec<(DVec2, String)> {
    let mut v = View::plan(lv.id);
    v.site = sv;
    let ViewKind::Plan { level, cut_height } = v.kind else {
        unreachable!()
    };
    creabim_geom::ranges::plan(m, &v, level, cut_height)
        .items
        .into_iter()
        .filter_map(|it| match it {
            PlanItem::Text { id, text, at, .. } if id == lot.id => Some((at, text)),
            _ => None,
        })
        .collect()
}

#[test]
fn each_edge_is_labelled_once_whatever_the_view_sets() {
    let (m, lv, lot) = model();
    let views = [
        SiteView::default(),
        SiteView {
            bearings: Some(true),
            ..SiteView::default()
        },
        SiteView {
            setbacks: Some(true),
            bearings: Some(true),
            ..SiteView::default()
        },
        SiteView {
            levels: false,
            ..SiteView::default()
        },
    ];
    for sv in views {
        let t = texts(&m, &lv, &lot, sv.clone());
        // four edges: a bearing each, a setback each, and the one note
        let bearings = t.iter().filter(|s| s.contains('°')).count();
        let setbacks = t
            .iter()
            .filter(|s| !s.contains('°') && s.ends_with(" m"))
            .count();
        let notes = t.iter().filter(|s| s.as_str() == "asdfasdf").count();
        assert_eq!(bearings, 4, "{sv:?}: {t:?}");
        assert_eq!(setbacks, 4, "{sv:?}: {t:?}");
        assert_eq!(notes, 1, "{sv:?}: {t:?}");
        // and no label twice in one place
        let at = places(&m, &lv, &lot, sv.clone());
        for (i, (p, a)) in at.iter().enumerate() {
            for (q, b) in &at[i + 1..] {
                assert!(p.distance(*q) > 1e-6 || a != b, "{sv:?}: {a} twice at {p}");
            }
        }
    }
}

#[test]
fn a_view_without_bearings_writes_none() {
    let (m, lv, lot) = model();
    let t = texts(
        &m,
        &lv,
        &lot,
        SiteView {
            bearings: Some(false),
            ..SiteView::default()
        },
    );
    assert_eq!(t.iter().filter(|s| s.contains('°')).count(), 0, "{t:?}");
    assert_eq!(t.iter().filter(|s| s.ends_with(" m")).count(), 4, "{t:?}");
}

/// While a site element is dragged, the plan shows that element alone
/// drawn fresh over the drawing held from before (MU37, MU98: a lot's
/// grip rebuilt the whole plan each frame).
#[test]
fn a_dragged_site_element_is_previewed_alone() {
    let (m, lv, lot) = model();
    let v = View::plan(lv.id);
    let preview = creabim_geom::ranges::site_preview(&m, &v, &[lot.id]).unwrap();
    assert!(!preview.is_empty());
    assert!(
        preview.iter().all(|it| it.id() == lot.id),
        "nothing but the lot"
    );
    // as the plan draws it: the same labels, sized for its scale
    let ViewKind::Plan { level, cut_height } = v.kind else {
        unreachable!()
    };
    let whole = creabim_geom::ranges::plan(&m, &v, level, cut_height);
    let ours: Vec<&PlanItem> = whole.items.iter().filter(|it| it.id() == lot.id).collect();
    let texts = |items: &mut dyn Iterator<Item = &PlanItem>| -> Vec<String> {
        let mut t: Vec<String> = items
            .filter_map(|it| match it {
                PlanItem::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        t.sort();
        t
    };
    assert_eq!(texts(&mut preview.iter()), texts(&mut ours.into_iter()));
    // not a site element, or not of the plan's level: the plan is drawn whole
    assert!(creabim_geom::ranges::site_preview(&m, &v, &[lv.id]).is_none());
    let other = View::plan(creabim_core::ElementId::new());
    assert!(creabim_geom::ranges::site_preview(&m, &other, &[lot.id]).is_none());
}

/// With "Along the line" (on by default) a lot's labels run parallel to
/// their edge, read from below or from the right, never upside down; off,
/// they are level (MU98_03).
#[test]
fn a_lots_labels_run_along_their_edges() {
    use std::f64::consts::FRAC_PI_2;
    let (m, lv, lot) = model();
    let angles = |sv: SiteView| -> Vec<(String, f64)> {
        let mut v = View::plan(lv.id);
        v.site = sv;
        let ViewKind::Plan { level, cut_height } = v.kind else {
            unreachable!()
        };
        creabim_geom::ranges::plan(&m, &v, level, cut_height)
            .items
            .into_iter()
            .filter_map(|it| match it {
                PlanItem::Text {
                    id, text, angle, ..
                } if id == lot.id => Some((text, angle)),
                _ => None,
            })
            .collect()
    };
    let along = angles(SiteView::default());
    assert_eq!(along.len(), 9);
    // the lot is a rectangle: its labels lie level or upright, and every
    // upright one reads from the right (turned a quarter anticlockwise)
    for (t, a) in &along {
        assert!(
            a.abs() < 1e-9 || (a - FRAC_PI_2).abs() < 1e-9,
            "{t} at {a} would read upside down or askew"
        );
    }
    // the two side edges' bearings and setbacks, and the note on one
    assert_eq!(
        along.iter().filter(|(_, a)| a.abs() > 1e-9).count(),
        5,
        "{along:?}"
    );
    let level = angles(SiteView {
        along: false,
        ..SiteView::default()
    });
    assert!(level.iter().all(|(_, a)| *a == 0.0), "{level:?}");
    // never upside down, whichever way an edge runs
    for deg in (0..360).step_by(15) {
        let d = glam::DVec2::from_angle((deg as f64).to_radians());
        let a = creabim_geom::parts::readable(d);
        assert!(a > -FRAC_PI_2 - 1e-9 && a <= FRAC_PI_2 + 1e-9, "{deg}: {a}");
        // and along the edge, one way or the other
        assert!(glam::DVec2::from_angle(a).perp_dot(d).abs() < 1e-9, "{deg}");
    }
}

/// An edge's note is its own label, the line below its setback at every
/// scale, and where it was dragged to after that, with a leader back
/// (MU98_06).
#[test]
fn an_edge_note_sits_below_its_setback_and_where_it_is_dragged() {
    let (mut m, lv, lot) = model();
    let find = |m: &Model, scale: f64| -> (DVec2, DVec2, f64) {
        let mut v = View::plan(lv.id);
        v.scale = scale;
        let ViewKind::Plan { level, cut_height } = v.kind else {
            unreachable!()
        };
        let items = creabim_geom::ranges::plan(m, &v, level, cut_height).items;
        let at = |want: &dyn Fn(&str) -> bool| {
            items
                .iter()
                .find_map(|it| match it {
                    PlanItem::Text {
                        id,
                        text,
                        at,
                        angle,
                        ..
                    } if *id == lot.id && want(text) => Some((*at, *angle)),
                    _ => None,
                })
                .unwrap()
        };
        let (note, a) = at(&|t| t == "asdfasdf");
        // the setback on the note's own edge: the nearest Side label
        let side = items
            .iter()
            .filter_map(|it| match it {
                PlanItem::Text { id, text, at, .. }
                    if *id == lot.id && text.starts_with("Side") =>
                {
                    Some(*at)
                }
                _ => None,
            })
            .min_by(|p, q| p.distance(note).total_cmp(&q.distance(note)))
            .unwrap();
        (note, side, a)
    };
    for scale in [100.0, 500.0] {
        let (note, side, a) = find(&m, scale);
        // the line below: down the text's own up, a text height and a bit
        let down = DVec2::new(a.sin(), -a.cos());
        let gap = (note - side).dot(down);
        let want = creabim_geom::parts::LOT_TEXT_MM * 1.25 * scale / 1000.0;
        assert!(
            (gap - want).abs() < 1e-6,
            "at 1:{scale}: {gap} below, want {want}"
        );
        assert!((note - side).perp_dot(down).abs() < 1e-6, "straight below");
    }
    // dragged: it stays where it was put, a leader back to the setback
    let before = m.get(lot.id).unwrap().clone();
    let mut e = before.clone();
    if let ElementKind::Site(Site {
        form: SiteForm::Property { rules, .. },
        ..
    }) = &mut e.kind
    {
        rules.set_note_off(3, Some(DVec2::new(-4.0, 2.0)));
    }
    let mut tx = creabim_core::Transaction::new("drag note");
    tx = tx.replace(before, e);
    let mut doc = creabim_core::Document::from_model(m.clone());
    doc.commit(tx).unwrap();
    m = doc.model().clone();
    let (moved, side, a) = find(&m, 100.0);
    let down = DVec2::new(a.sin(), -a.cos());
    let want =
        side + DVec2::new(-4.0, 2.0) + down * (creabim_geom::parts::LOT_TEXT_MM * 1.25 * 0.1);
    assert!(moved.distance(want) < 1e-6, "{moved} not {want}");
}

/// With View › Site › Tints off, the site's tints and fills are greys of
/// the same lightness: a plan in black and white (Site polish 5).
#[test]
fn a_plan_without_tints_draws_the_site_in_greys() {
    let (m, lv, lot) = model();
    let tints = |sv: SiteView| -> Vec<[f32; 3]> {
        let mut v = View::plan(lv.id);
        v.site = sv;
        let ViewKind::Plan { level, cut_height } = v.kind else {
            unreachable!()
        };
        creabim_geom::ranges::plan(&m, &v, level, cut_height)
            .items
            .into_iter()
            .filter_map(|it| match it {
                PlanItem::Tint { id, rgb, .. } | PlanItem::Shade { id, rgb, .. }
                    if id == lot.id =>
                {
                    Some(rgb)
                }
                _ => None,
            })
            .collect()
    };
    let coloured = tints(SiteView::default());
    assert!(!coloured.is_empty());
    assert!(
        coloured.iter().any(|c| (c[0] - c[1]).abs() > 0.01),
        "{coloured:?}"
    );
    let grey = tints(SiteView {
        tints: false,
        ..SiteView::default()
    });
    assert_eq!(grey.len(), coloured.len());
    for (g, c) in grey.iter().zip(&coloured) {
        assert!(
            (g[0] - g[1]).abs() < 1e-6 && (g[1] - g[2]).abs() < 1e-6,
            "{g:?}"
        );
        let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        assert!((g[0] - l).abs() < 1e-6, "same lightness");
    }
}
