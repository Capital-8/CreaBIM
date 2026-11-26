//! A curtain wall set in a gable of the starter house and attached to its
//! roof (a public review, 2026-11-16: "Attach to roof" did nothing for a
//! curtain wall, so it ran through the gable; and at full height it left
//! the gable above it open to the roof).

use creabim_core::*;
use glam::DVec2;

/// The starter house with a curtain wall in the middle of a first-floor
/// gable, `attached` to the main roof or not, as tall as the gable's eaves.
fn gable_with_curtain(attached: bool) -> (Model, ElementId, ElementId) {
    gable_with(attached, (0.25, 0.75), 0.0, MullionProfile::Box)
}

/// As [`gable_with_curtain`], from `span.0` to `span.1` along the gable,
/// bowed by `bulge` (a curved curtain wall, out of the gable's line and
/// under the roof's slopes), with mullions of `profile`.
fn gable_with(
    attached: bool,
    span: (f64, f64),
    bulge: f64,
    profile: MullionProfile,
) -> (Model, ElementId, ElementId) {
    let mut doc = Document::from_model(starter::starter_house());
    let model = doc.model();
    // a gable: a first-floor wall whose roof-following top bends
    let (host_id, host) = model
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Wall(w) if w.curtain.is_none() && !w.attach_top.is_empty() => {
                // its roof well above its middle, its ends at the eaves
                let p = walls::top_profile(model, w)?;
                let top = walls::base_z(model, w) + w.height;
                let mid = p.z_at((w.start + w.end) / 2.0)?;
                (mid > top + 1.0).then(|| (e.id, w.clone()))
            }
            _ => None,
        })
        .expect("a gable in the starter house");
    let d = host.direction();
    let len = host.length();
    let mut cw = Wall::new(
        host.level,
        host.start + d * (len * span.0),
        host.start + d * (len * span.1),
        0.15,
        host.height,
    );
    cw.base_offset = host.base_offset;
    cw.bulge = bulge;
    cw.curtain = Some(CurtainGrid {
        mullion_depth: 0.15,
        profile,
        transom_depth: (profile != MullionProfile::Box).then_some(0.1),
        ..Default::default()
    });
    if attached {
        cw.attach_top = host.attach_top.clone();
    }
    let cw_el = Element::new("Gable glazing", ElementKind::Wall(cw));
    let cw_id = cw_el.id;
    doc.commit(Transaction::new("Gable glazing").insert(cw_el))
        .unwrap();
    (doc.model().clone(), host_id, cw_id)
}

#[test]
fn two_curtain_walls_at_a_corner_share_one_post() {
    use creabim_geom::curtain::{curtain_boxes_under, setting, CurtainPart, EndJoin};
    let mut doc = Document::from_model(starter::starter_house());
    let level = doc
        .model()
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Wall(w) => Some(w.level),
            _ => None,
        })
        .unwrap();
    // an L in the garden, clear of the house
    let mk = |a: DVec2, b: DVec2| {
        let mut w = Wall::new(level, a, b, 0.15, 2.8);
        w.curtain = Some(CurtainGrid {
            vertical: GridRule::Count(3),
            ..Default::default()
        });
        w
    };
    let a = mk(DVec2::new(30.0, 30.0), DVec2::new(33.0, 30.0));
    let b = mk(DVec2::new(33.0, 30.0), DVec2::new(33.0, 33.0));
    doc.commit(
        Transaction::new("L")
            .insert(Element::new("A", ElementKind::Wall(a.clone())))
            .insert(Element::new("B", ElementKind::Wall(b.clone()))),
    )
    .unwrap();
    let model = doc.model();
    let (sa, sb) = (setting(model, &a), setting(model, &b));
    let joins = [sa.ends[1], sb.ends[0]];
    // one carries the post, the other stops against it
    assert!(
        joins.iter().any(|j| matches!(j, EndJoin::Post(_))),
        "{joins:?}"
    );
    assert!(
        joins.iter().any(|j| matches!(j, EndJoin::Butt(_))),
        "{joins:?}"
    );
    assert!(sa.ends[0] == EndJoin::Free && sb.ends[1] == EndJoin::Free);
    let g = a.curtain.clone().unwrap();
    let ba = curtain_boxes_under(&a, &g, &[], &sa);
    let bb = curtain_boxes_under(&b, &g, &[], &sb);
    let uprights = |bs: &[creabim_geom::curtain::CurtainBox], at: f64| {
        bs.iter()
            .filter(|x| x.part == CurtainPart::Mullion && x.hi.z - x.lo.z > 2.0)
            .filter(|x| x.lo.x <= at + 0.08 && x.hi.x >= at - 0.08)
            .count()
    };
    // at the corner: A's end (3.0 along it) and B's start (0.0)
    let (post, butt) = if matches!(sa.ends[1], EndJoin::Post(_)) {
        (uprights(&ba, 3.0), uprights(&bb, 0.0))
    } else {
        (uprights(&bb, 0.0), uprights(&ba, 3.0))
    };
    assert_eq!(
        (post > 0, butt),
        (true, 0),
        "one post, not two edge mullions"
    );
}

/// Writes both houses to look at in the app:
/// `CREABIM_OUT=dir cargo test --test curtain_roof -- --ignored`.
#[test]
#[ignore]
fn write_the_gable_houses() {
    let dir = std::env::var("CREABIM_OUT").expect("CREABIM_OUT");
    for (attached, name) in [(true, "gable-attached"), (false, "gable-free")] {
        let (m, ..) = gable_with_curtain(attached);
        std::fs::write(format!("{dir}/{name}.creabim.json"), m.to_json()).unwrap();
    }
    for p in MullionProfile::ALL {
        let (m, ..) = gable_with(true, (0.25, 0.75), 0.0, p);
        let name = format!("gable-{}", p.label().to_lowercase());
        std::fs::write(format!("{dir}/{name}.creabim.json"), m.to_json()).unwrap();
    }
    let (m, ..) = gable_with(true, (0.2, 0.6), 0.6, MullionProfile::Stick);
    std::fs::write(format!("{dir}/gable-curved.creabim.json"), m.to_json()).unwrap();
}

/// Every vertex of `cw`'s parts lies under the roof it is attached to,
/// and the highest reach it.
fn under_and_reaching(model: &Model, cw_id: ElementId) {
    under_and_reaching_within(model, cw_id, 0.02)
}

/// As [`under_and_reaching`], reaching within `reach` of it (a curved
/// wall's flat panes keep a little under the roof over their arc).
fn under_and_reaching_within(model: &Model, cw_id: ElementId, reach: f64) {
    let ElementKind::Wall(cw) = &model.get(cw_id).unwrap().kind else {
        unreachable!()
    };
    let roof = walls::top_profile(model, cw).expect("the roof over it");
    let scene = creabim_geom::scene::build_scene(model);
    let mut gap = f64::MAX;
    let mut n = 0;
    for it in scene.iter().filter(|i| i.id == cw_id) {
        for p in &it.mesh.positions {
            let xy = DVec2::new(p[0] as f64, p[1] as f64);
            if let Some(z) = roof.z_at(xy) {
                n += 1;
                assert!(
                    p[2] as f64 <= z + 0.02,
                    "a part pokes {:.3} above the roof",
                    p[2] as f64 - z
                );
                gap = gap.min(z - p[2] as f64);
            }
        }
    }
    assert!(n > 0);
    assert!(gap < reach, "it stops {gap:.3} short of the roof");
}

#[test]
fn a_curved_curtain_wall_under_the_gable_follows_both_slopes() {
    // off-centre, so its arc meets the ridge at a slant
    for bulge in [0.6, -0.6] {
        let (model, _, cw_id) = gable_with(true, (0.2, 0.6), bulge, MullionProfile::Stick);
        under_and_reaching(&model, cw_id);
    }
}

#[test]
fn every_profile_stays_under_the_roof() {
    for p in MullionProfile::ALL {
        let (model, _, cw_id) = gable_with(true, (0.25, 0.75), 0.0, p);
        under_and_reaching(&model, cw_id);
    }
}

#[test]
fn an_attached_curtain_wall_stays_under_the_roof_and_reaches_it() {
    let (model, host_id, cw_id) = gable_with_curtain(true);
    let ElementKind::Wall(cw) = &model.get(cw_id).unwrap().kind else {
        unreachable!()
    };
    let roof = walls::top_profile(&model, cw).expect("the roof over it");
    let scene = creabim_geom::scene::build_scene(&model);
    let parts: Vec<_> = scene.iter().filter(|i| i.id == cw_id).collect();
    assert!(!parts.is_empty());
    let mut highest_gap = f64::MAX;
    for it in &parts {
        for p in &it.mesh.positions {
            let xy = DVec2::new(p[0] as f64, p[1] as f64);
            if let Some(z) = roof.z_at(xy) {
                assert!(
                    p[2] as f64 <= z + 2e-3,
                    "a part pokes {} above the roof",
                    p[2] as f64 - z
                );
                highest_gap = highest_gap.min(z - p[2] as f64);
            }
        }
    }
    // it reaches the roof (the head mullion's top meets it)
    assert!(highest_gap < 2e-3, "{highest_gap}");
    // and the gable around it opens up to the roof for it
    let hole = curtain::void_of(
        &model,
        host_id,
        match &model.get(host_id).unwrap().kind {
            ElementKind::Wall(w) => w,
            _ => unreachable!(),
        },
        cw,
    )
    .expect("it sits in the gable");
    let ElementKind::Wall(host) = &model.get(host_id).unwrap().kind else {
        unreachable!()
    };
    assert!((hole.sill + hole.height - host.height).abs() < 1e-9);
}

#[test]
fn a_curtain_wall_not_attached_leaves_the_gable_closed_over_it() {
    let (model, host_id, cw_id) = gable_with_curtain(false);
    let ElementKind::Wall(host) = &model.get(host_id).unwrap().kind else {
        unreachable!()
    };
    let ElementKind::Wall(cw) = &model.get(cw_id).unwrap().kind else {
        unreachable!()
    };
    // as tall as the gable's own height: the hole stops under it, so the
    // gable's top row, which follows the roof, stays wall
    let hole = curtain::void_of(&model, host_id, host, cw).expect("it sits in the gable");
    assert!(hole.sill + hole.height < host.height);
    // the gable's mesh closes over the curtain wall: something of the
    // host stands over its middle, above its top
    let scene = creabim_geom::scene::build_scene(&model);
    let mid = (cw.start + cw.end) / 2.0;
    let top = walls::base_z(&model, cw) + cw.height;
    let over = scene.iter().filter(|i| i.id == host_id).any(|i| {
        i.mesh.positions.iter().any(|p| {
            let xy = DVec2::new(p[0] as f64, p[1] as f64);
            (xy - mid).length() < 0.6 && p[2] as f64 > top + 0.3
        })
    });
    let near: Vec<f64> = scene
        .iter()
        .filter(|i| i.id == host_id)
        .flat_map(|i| i.mesh.positions.iter())
        .filter(|p| (DVec2::new(p[0] as f64, p[1] as f64) - mid).length() < 0.6)
        .map(|p| p[2] as f64)
        .collect();
    assert!(
        over,
        "the gable is open over the curtain wall: hole {hole:?} host h {} base {} cw top {top} near zs {:?} pieces {}",
        host.height,
        walls::base_z(&model, host),
        near.iter().cloned().fold(f64::MIN, f64::max),
        scene.iter().filter(|i| i.id == host_id).count()
    );
}
