//! How the ground and a property are drawn is each view's (contour
//! interval, heights, shading, water; setback lines, bearings). A view
//! that has set nothing draws as the terrain and the property say, so
//! the plans of a file from before views carried this draw as they did.

use creabim_core::{
    Element, ElementKind, GroundAnalysis, GroundLook, Level, Model, Site, SiteForm, SiteView,
};
use creabim_geom::parts::site_plan_with;
use creabim_geom::plan::{Pen, PlanItem};
use glam::{DVec2, DVec3};

fn square(half: f64) -> Vec<DVec2> {
    creabim_core::polygon::rectangle(DVec2::splat(-half), DVec2::splat(half))
}

/// A sloping terrain as an older file kept it: its contour interval, its
/// heavier contours and their heights on the terrain itself.
fn older() -> (Model, Element) {
    let lv = Element::new(
        "Ground Floor",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let mut t = Site::terrain(lv.id, square(20.0), 0.0, 1.0);
    if let SiteForm::Terrain {
        points,
        contour,
        data,
        ..
    } = &mut t.form
    {
        points.push(DVec3::new(-18.0, -18.0, 0.0));
        points.push(DVec3::new(18.0, -18.0, 0.5));
        points.push(DVec3::new(18.0, 18.0, 4.0));
        points.push(DVec3::new(-18.0, 18.0, 3.0));
        *contour = 0.5;
        data.major = 2;
        data.labels = true;
        data.analysis = GroundAnalysis::Slope;
    }
    let te = Element::new("Terrain", ElementKind::Site(t));
    // through the file's own JSON, as it is read back
    let m = Model::from_elements([lv, te.clone()]).unwrap();
    let m = Model::from_json(&m.to_json()).unwrap();
    (m, te)
}

fn drawn(m: &Model, e: &Element, sv: &SiteView) -> Vec<PlanItem> {
    let ElementKind::Site(s) = &e.kind else {
        unreachable!()
    };
    site_plan_with(m, e.id, s, sv)
}

fn count(items: &[PlanItem], f: impl Fn(&PlanItem) -> bool) -> usize {
    items.iter().filter(|i| f(i)).count()
}

fn contours(items: &[PlanItem]) -> usize {
    count(items, |i| {
        matches!(
            i,
            PlanItem::Line {
                pen: Pen::Contour | Pen::ContourHeavy,
                ..
            }
        )
    })
}

#[test]
fn an_older_files_plan_draws_as_the_terrain_said() {
    let (m, e) = older();
    let ElementKind::Site(s) = &e.kind else {
        unreachable!()
    };
    let SiteForm::Terrain { contour, data, .. } = &s.form else {
        unreachable!()
    };
    // a view that has set nothing of its own, as every view of an older file
    let plain = SiteView::default();
    assert!(plain.look.is_none());
    let as_it_was = drawn(&m, &e, &plain);
    assert!(contours(&as_it_was) > 4, "its contours are drawn");
    assert!(
        count(&as_it_was, |i| matches!(i, PlanItem::Text { .. })) > 0,
        "its heavier contours carry their heights"
    );
    assert!(
        count(&as_it_was, |i| matches!(i, PlanItem::Tint { .. })) > 0,
        "it is shaded by slope"
    );
    // and exactly as a view that says the same thing itself
    let said = SiteView {
        look: Some(GroundLook::of_terrain(*contour, data)),
        ..SiteView::default()
    };
    assert_eq!(as_it_was, drawn(&m, &e, &said), "line for line the same");
}

#[test]
fn a_view_draws_the_ground_its_own_way() {
    let (m, e) = older();
    let base = drawn(&m, &e, &SiteView::default());
    // a wider interval in this view: fewer contours, the terrain untouched
    let wide = SiteView {
        look: Some(GroundLook {
            interval: 2.0,
            ..GroundLook::default()
        }),
        ..SiteView::default()
    };
    let d = drawn(&m, &e, &wide);
    assert!(contours(&d) > 0 && contours(&d) < contours(&base));
    // and no shading or heights it did not ask for
    assert_eq!(count(&d, |i| matches!(i, PlanItem::Tint { .. })), 0);
    assert_eq!(count(&d, |i| matches!(i, PlanItem::Text { .. })), 0);
    // none at all
    let none = SiteView {
        look: Some(GroundLook::default()),
        ..SiteView::default()
    };
    assert_eq!(contours(&drawn(&m, &e, &none)), 0);
}

#[test]
fn a_view_with_nothing_set_writes_nothing_new_to_the_file() {
    let sv = SiteView::default();
    let json = serde_json::to_string(&sv).unwrap();
    assert!(!json.contains("look") && !json.contains("setbacks") && !json.contains("bearings"));
    // and a view's settings from an older file read back with nothing set
    let old: SiteView = serde_json::from_str(r#"{"ground":"After","contours":true}"#).unwrap();
    assert!(old.look.is_none() && old.setbacks.is_none() && old.bearings.is_none());
}

#[test]
fn a_propertys_setbacks_and_bearings_are_the_views() {
    let lv = Element::new(
        "Ground Floor",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let mut p = Site::property(lv.id, square(15.0));
    // an older file with its bearings switched off on the property
    if let SiteForm::Property { rules, .. } = &mut p.form {
        rules.front = 5.0;
        rules.bearings = false;
    }
    let e = Element::new("Lot", ElementKind::Site(p));
    let m = Model::from_elements([lv, e.clone()]).unwrap();
    let texts = |sv: &SiteView| count(&drawn(&m, &e, sv), |i| matches!(i, PlanItem::Text { .. }));
    let lines = |sv: &SiteView| count(&drawn(&m, &e, sv), |i| matches!(i, PlanItem::Line { .. }));
    let plain = SiteView::default();
    // as the property said: setbacks drawn, no bearings
    let with = SiteView {
        bearings: Some(true),
        ..SiteView::default()
    };
    assert!(
        texts(&with) > texts(&plain),
        "this view writes the bearings"
    );
    let bare = SiteView {
        setbacks: Some(false),
        ..SiteView::default()
    };
    assert!(
        lines(&bare) < lines(&plain),
        "this view leaves the setback lines out"
    );
}
