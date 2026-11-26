//! A pad's edge held by a retaining wall stands upright; its other
//! edges keep their banks.

use creabim_core::{Element, ElementKind, Level, Model, Site, SiteForm};
use glam::DVec2;

#[test]
fn a_retained_edge_stands_upright_and_the_others_keep_their_banks() {
    let lv = Element::new(
        "Ground Floor",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let sq = |h: f64| creabim_core::polygon::rectangle(DVec2::splat(-h), DVec2::splat(h));
    let ground = Site::terrain(lv.id, sq(20.0), 0.0, 1.0);
    // dug 1 m into flat ground, banks 1 in 1; its south edge (edge 0) retained
    let mut pad = Site::pad(lv.id, sq(4.0), -1.0);
    if let SiteForm::Pad { retained, .. } = &mut pad.form {
        retained.push(0);
    }
    let m = Model::from_elements([
        lv,
        Element::new("Terrain", ElementKind::Site(ground.clone())),
        Element::new("Pad", ElementKind::Site(pad)),
    ])
    .unwrap();
    let sh = creabim_geom::earth::shaped(&m, &ground).unwrap();
    assert_eq!(sh.banks.len(), 1);
    let run = |on: &dyn Fn(DVec2) -> bool| {
        let v: Vec<f64> = sh.banks[0]
            .iter()
            .filter(|(a, _)| on(a.truncate()))
            .map(|(a, d)| a.truncate().distance(d.truncate()))
            .collect();
        assert!(!v.is_empty());
        v.iter().sum::<f64>() / v.len() as f64
    };
    let south = run(&|p| (p.y + 4.0).abs() < 1e-6 && p.x.abs() < 3.5);
    let north = run(&|p| (p.y - 4.0).abs() < 1e-6 && p.x.abs() < 3.5);
    assert!(south < 0.05, "the retained edge runs out {south} m");
    assert!(
        (north - 1.0).abs() < 0.05,
        "a 1 in 1 bank 1 m deep runs out {north} m"
    );
    // the ground just beyond the wall is where it was; beyond the bank's
    // top too, and half way down the bank it is half way down
    let at = |p: DVec2| sh.ground.height(p).unwrap();
    assert!(at(DVec2::new(0.0, -4.3)).abs() < 0.02);
    assert!((at(DVec2::new(0.0, 4.5)) + 0.5).abs() < 0.08);
    // less is dug than with banks all round
    let mut all = Site::pad(m.iter().next().unwrap().id, sq(4.0), -1.0);
    all.level = ground.level;
    let m2 = Model::from_elements([
        m.iter().next().unwrap().clone(),
        Element::new("Terrain", ElementKind::Site(ground.clone())),
        Element::new("Pad", ElementKind::Site(all)),
    ])
    .unwrap();
    let banked = creabim_geom::earth::shaped(&m2, &ground).unwrap();
    assert!(
        sh.cut < banked.cut - 3.0,
        "{} against {}",
        sh.cut,
        banked.cut
    );
}
