//! Spot levels and ground lines as elements of their own: the terrain
//! they lie on is shaped by them as it was by its own data.

use creabim_core::{
    Document, Element, ElementId, ElementKind, GroundLine, GroundLineKind, Level, Model, Site,
    SiteForm, SiteView, Transaction, Xform,
};
use glam::{DVec2, DVec3};

fn level(elevation: f64) -> Element {
    Element::new(
        "Ground Floor",
        ElementKind::Level(Level {
            elevation,
            height: 3.0,
        }),
    )
}

fn terrain(m: &Model) -> &Site {
    m.iter()
        .find_map(|e| match &e.kind {
            ElementKind::Site(s) if matches!(s.form, SiteForm::Terrain { .. }) => Some(s),
            _ => None,
        })
        .unwrap()
}

fn at(m: &Model, p: DVec2) -> f64 {
    creabim_geom::earth::shaped(m, terrain(m))
        .unwrap()
        .ground
        .height(p)
        .unwrap()
}

fn square(half: f64) -> Vec<DVec2> {
    creabim_core::polygon::rectangle(DVec2::splat(-half), DVec2::splat(half))
}

#[test]
fn a_terrain_is_the_same_ground_with_its_heights_as_elements() {
    let lv = level(0.0);
    let mut t = Site::terrain(lv.id, square(20.0), 0.0, 1.0);
    if let SiteForm::Terrain { points, data, .. } = &mut t.form {
        points.push(DVec3::new(-8.0, -6.0, 1.0));
        points.push(DVec3::new(6.0, 4.0, 2.5));
        data.lines.push(GroundLine {
            kind: GroundLineKind::Contour,
            points: vec![DVec3::new(-15.0, 12.0, 3.0), DVec3::new(15.0, 12.0, 3.0)],
            closed: false,
        });
    }
    let mut doc = Document::from_model(
        Model::from_elements([lv, Element::new("Terrain", ElementKind::Site(t))]).unwrap(),
    );
    let probes = [
        DVec2::new(-8.0, -6.0),
        DVec2::new(0.0, 0.0),
        DVec2::new(3.0, 12.0),
        DVec2::new(10.0, -10.0),
    ];
    let was: Vec<f64> = probes.iter().map(|p| at(doc.model(), *p)).collect();
    assert!((was[0] - 1.0).abs() < 1e-9 && (was[2] - 3.0).abs() < 1e-9);
    doc.commit(creabim_core::site::earlier_pads(doc.model()).expect("heights to convert"))
        .unwrap();
    assert_eq!(terrain(doc.model()).heights().count(), 0);
    for (p, z) in probes.iter().zip(&was) {
        assert!((at(doc.model(), *p) - z).abs() < 1e-9, "{p}");
    }
    // and a flat terrain with one spot level on it is no longer a slab
    let (m, _) = (doc.model(), ());
    assert!(creabim_core::site::surveyed(m, terrain(m)));
}

#[test]
fn a_spot_level_moved_or_relevelled_takes_the_ground_with_it() {
    let lv = level(0.0);
    let upper = level(2.0);
    let ground = Element::new(
        "Terrain",
        ElementKind::Site(Site::terrain(lv.id, square(20.0), 0.0, 1.0)),
    );
    let spot = Element::new(
        "Spot level",
        ElementKind::Site(Site::spot(lv.id, DVec2::new(-5.0, 0.0), 1.5)),
    );
    // on a level 2 m up, 0.5 above it: 2.5 above the terrain's level
    let high = Element::new(
        "Spot level",
        ElementKind::Site(Site::spot(upper.id, DVec2::new(8.0, 8.0), 0.5)),
    );
    // off the terrain: it shapes nothing
    let off = Element::new(
        "Spot level",
        ElementKind::Site(Site::spot(lv.id, DVec2::new(50.0, 0.0), 9.0)),
    );
    let id: ElementId = spot.id;
    let mut doc =
        Document::from_model(Model::from_elements([lv, upper, ground, spot, high, off]).unwrap());
    let (west, east) = (DVec2::new(-5.0, 0.0), DVec2::new(5.0, 0.0));
    assert!((at(doc.model(), west) - 1.5).abs() < 1e-9);
    assert!((at(doc.model(), DVec2::new(8.0, 8.0)) - 2.5).abs() < 1e-9);
    let (_, hi) = creabim_geom::earth::shaped(doc.model(), terrain(doc.model()))
        .unwrap()
        .ground
        .range();
    assert!(
        hi < 2.5 + 1e-6,
        "the spot off the terrain raised it to {hi}"
    );
    // moved 10 m east and 0.5 up
    let before = doc.model().get(id).unwrap().clone();
    let after = creabim_core::transform::transformed(
        doc.model(),
        &before,
        &Xform::Move(DVec3::new(10.0, 0.0, 0.5)),
    )
    .unwrap();
    doc.commit(Transaction::new("Move").replace(before, after))
        .unwrap();
    assert!((at(doc.model(), east) - 2.0).abs() < 1e-9);
    assert!((at(doc.model(), west) - 1.5).abs() > 0.05);
    // deleted, the ground lets go of it
    let tx = Transaction::new("Delete").remove_cascade(doc.model(), id);
    doc.commit(tx).unwrap();
    assert!((at(doc.model(), east) - 2.0).abs() > 0.05);
}

#[test]
fn a_spot_level_and_a_contour_draw_what_a_click_takes_them_by() {
    use creabim_geom::plan::PlanItem;
    let lv = level(100.0);
    let spot = Site::spot(lv.id, DVec2::new(1.0, 1.0), 0.45);
    let line = Site::contour(
        lv.id,
        vec![DVec2::ZERO, DVec2::X * 5.0, DVec2::new(8.0, 3.0)],
        1.0,
    );
    let m = Model::from_elements([lv]).unwrap();
    let id = ElementId::new();
    let on = SiteView::default();
    let items = creabim_geom::parts::site_plan_with(&m, id, &spot, &on);
    assert_eq!(
        items
            .iter()
            .filter(|i| matches!(i, PlanItem::Line { .. }))
            .count(),
        2,
        "a cross"
    );
    // and its level, a tag on the point it reads (MU142: one tag language)
    assert!(
        items.iter().any(|i| matches!(i, PlanItem::Tag { anchor, .. } if anchor.distance(DVec2::new(1.0, 1.0)) < 1e-9)),
        "and its level"
    );
    let items = creabim_geom::parts::site_plan_with(&m, id, &line, &on);
    assert_eq!(items.len(), 2, "its two stretches");
    // a view that shows neither levels nor contours shows neither
    let off = SiteView {
        levels: false,
        contours: false,
        ..SiteView::default()
    };
    assert!(creabim_geom::parts::site_plan_with(&m, id, &spot, &off).is_empty());
    assert!(creabim_geom::parts::site_plan_with(&m, id, &line, &off).is_empty());
}
