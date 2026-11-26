//! An upright edge of a pit or a pad reads as a clean cut in 3D: one face
//! from the ground down to the floor, its corners closed, no fan of thin
//! slivers and no band of edges along it (owner, site polish 6).

use creabim_core::{Element, ElementKind, Level, Model, Site, SiteForm};
use glam::{DVec2, DVec3};

/// A sloping terrain, a pit dug 1.2 m into it with sides lying back 1 in
/// 0.5, its south edge (edge 0) cut upright.
fn model() -> (Model, Site) {
    let lv = Element::new(
        "Ground Floor",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let sq = |h: f64| creabim_core::polygon::rectangle(DVec2::splat(-h), DVec2::splat(h));
    let mut ground = Site::terrain(lv.id, sq(20.0), 0.0, 2.0);
    if let SiteForm::Terrain { points, .. } = &mut ground.form {
        // a slope rising north
        for x in [-20.0, -10.0, 0.0, 10.0, 20.0] {
            for y in [-20.0, -10.0, 0.0, 10.0, 20.0] {
                points.push(DVec3::new(x, y, 0.06 * y));
            }
        }
    }
    let outline = creabim_core::polygon::rectangle(DVec2::new(-2.0, -1.5), DVec2::new(2.0, 1.5));
    let mut pit = Site::pit(lv.id, outline, 1.2, 0.5);
    if let SiteForm::Pad { retained, .. } = &mut pit.form {
        retained.push(0);
    }
    let m = Model::from_elements([
        lv,
        Element::new("Terrain", ElementKind::Site(ground.clone())),
        Element::new("Pit", ElementKind::Site(pit)),
    ])
    .unwrap();
    (m, ground)
}

/// The ground's triangles near the pit's south edge's two corners.
fn near_corners(m: &Model, ground: &Site) -> Vec<[DVec3; 3]> {
    let sh = creabim_geom::earth::shaped(m, ground).unwrap();
    let corners = [DVec2::new(-2.0, -1.5), DVec2::new(2.0, -1.5)];
    sh.ground
        .triangles(0.0)
        .into_iter()
        .filter(|t| {
            let c = (t[0] + t[1] + t[2]) / 3.0;
            corners.iter().any(|k| c.truncate().distance(*k) < 1.6)
        })
        .collect()
}

/// A triangle's smallest angle, in degrees.
fn least_angle(t: &[DVec3; 3]) -> f64 {
    (0..3)
        .map(|i| {
            let (a, b, c) = (t[i], t[(i + 1) % 3], t[(i + 2) % 3]);
            let (u, v) = ((b - a).normalize_or_zero(), (c - a).normalize_or_zero());
            u.dot(v).clamp(-1.0, 1.0).acos().to_degrees()
        })
        .fold(180.0, f64::min)
}

#[test]
fn the_corners_of_an_upright_cut_make_no_slivers() {
    let (m, ground) = model();
    let tris = near_corners(&m, &ground);
    assert!(!tris.is_empty());
    // slivers: long thin triangles standing more or less upright, the fan
    // the ground made where the upright face met the sloping side
    let slivers: Vec<f64> = tris
        .iter()
        .filter(|t| {
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            let upright = n.length() > 1e-12 && n.normalize().z.abs() < 0.5;
            let long = (0..3).any(|i| t[i].distance(t[(i + 1) % 3]) > 0.6);
            upright && long && least_angle(t) < 3.0
        })
        .map(least_angle)
        .collect();
    eprintln!(
        "slivers near the corners: {} of {}",
        slivers.len(),
        tris.len()
    );
    assert!(slivers.len() <= 2, "{} slivers: {slivers:?}", slivers.len());
}

/// The 3D view's edges where the terrain creases (faces meeting at more
/// than 20°, as the 3D view draws them) near the pit, and their length.
fn creases(m: &Model, ground: &Site) -> (usize, f64, usize) {
    use std::collections::HashMap;
    let mesh = creabim_geom::parts::site_meshes(m, ground).remove(0).0;
    let key = |p: [f32; 3]| {
        (
            (p[0] * 1e4).round() as i64,
            (p[1] * 1e4).round() as i64,
            (p[2] * 1e4).round() as i64,
        )
    };
    let mut edges: HashMap<_, Vec<DVec3>> = HashMap::new();
    let mut ends: HashMap<_, (DVec3, DVec3)> = HashMap::new();
    for t in mesh.indices.chunks(3) {
        let p: Vec<DVec3> = t
            .iter()
            .map(|&i| glam::Vec3::from(mesh.positions[i as usize]).as_dvec3())
            .collect();
        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
        for k in 0..3 {
            let (a, b) = (
                mesh.positions[t[k] as usize],
                mesh.positions[t[(k + 1) % 3] as usize],
            );
            let (ka, kb) = (key(a), key(b));
            let id = if ka < kb { (ka, kb) } else { (kb, ka) };
            edges.entry(id).or_default().push(n);
            ends.insert(id, (p[k], p[(k + 1) % 3]));
        }
    }
    let near = |p: DVec3| p.x.abs() < 3.5 && p.y > -3.5 && p.y < -0.5 && p.z > -3.0;
    let (mut count, mut len, mut short) = (0, 0.0, 0);
    for (id, ns) in &edges {
        let crease = ns.len() == 2 && ns[0].dot(ns[1]) < 0.94;
        let (a, b) = ends[id];
        if crease && near(a) && near(b) {
            count += 1;
            len += a.distance(b);
            if a.distance(b) < 0.05 {
                short += 1;
            }
        }
    }
    (count, len, short)
}

#[test]
fn an_upright_cut_draws_few_edges_in_3d() {
    let (m, ground) = model();
    let (count, len, short) = creases(&m, &ground);
    eprintln!("creases by the south face: {count}, {len:.2} m long, {short} under 5 cm");
    assert_eq!(short, 0, "no bits of edge at the corners");
}

/// The 3D view's edges round the whole pit shorter than 15 cm: the teeth
/// the corners zigzagged in, where two banks or a bank and the upright
/// face met. Before the corners were mitred there were dozens.
#[test]
fn the_pits_corners_meet_in_single_creases() {
    let (m, ground) = model();
    let mesh = creabim_geom::parts::site_meshes(&m, &ground).remove(0).0;
    let item = creabim_geom::scene::SceneItem {
        id: creabim_core::ElementId::new(),
        mesh,
        color: [0.5, 0.6, 0.4],
        surface: [0.5, 0.0, 1.0],
        ghost: false,
        layer: None,
        finish: None,
    };
    let near = |p: DVec3| p.x.abs() < 4.0 && p.y.abs() < 4.0 && p.z > -2.0;
    let edges: Vec<(DVec3, DVec3)> = creabim_geom::section::feature_edges_over(&item, 0.94)
        .into_iter()
        .filter(|(a, b)| near(*a) && near(*b))
        .collect();
    // where the drawn lines turn: a zigzag turns at every tooth, a clean
    // cut only at its true corners
    use std::collections::HashMap;
    let key = |p: DVec3| {
        (
            (p.x * 1e3).round() as i64,
            (p.y * 1e3).round() as i64,
            (p.z * 1e3).round() as i64,
        )
    };
    let mut at: HashMap<_, Vec<DVec3>> = HashMap::new();
    for (a, b) in &edges {
        at.entry(key(*a))
            .or_default()
            .push((*b - *a).normalize_or_zero());
        at.entry(key(*b))
            .or_default()
            .push((*a - *b).normalize_or_zero());
    }
    let turns = at
        .values()
        .filter(|ds| ds.len() == 2 && ds[0].dot(ds[1]) > -0.87)
        .count();
    eprintln!("lines turn {turns} times round the pit");
    assert!(
        turns <= 2,
        "{turns} turns: a zigzag (13 before the corners were mitred)"
    );
}

/// Writes the terrain's mesh near the pit, and its 3D edges, for a
/// picture of it (CREABIM_DUMP=path).
#[test]
fn dump_for_a_picture() {
    let Some(path) = std::env::var_os("CREABIM_DUMP") else {
        return;
    };
    let (m, ground) = model();
    let mesh = creabim_geom::parts::site_meshes(&m, &ground).remove(0).0;
    let near = |p: [f32; 3]| p[0].abs() < 5.0 && p[1] > -5.0 && p[1] < 4.0 && p[2] > -2.5;
    let mut tris = vec![];
    for t in mesh.indices.chunks(3) {
        let p: Vec<[f32; 3]> = t.iter().map(|&i| mesh.positions[i as usize]).collect();
        if p.iter().all(|q| near(*q)) {
            tris.push(p);
        }
    }
    let item = creabim_geom::scene::SceneItem {
        id: creabim_core::ElementId::new(),
        mesh: mesh.clone(),
        color: [0.5, 0.6, 0.4],
        surface: [0.5, 0.0, 1.0],
        ghost: false,
        layer: None,
        finish: None,
    };
    let edges: Vec<[[f64; 3]; 2]> = creabim_geom::section::feature_edges_over(&item, 0.94)
        .into_iter()
        .filter(|(a, b)| near(a.as_vec3().to_array()) && near(b.as_vec3().to_array()))
        .map(|(a, b)| [a.to_array(), b.to_array()])
        .collect();
    let json = serde_json::json!({ "tris": tris, "edges": edges });
    std::fs::write(path, json.to_string()).unwrap();
}

#[test]
fn print_the_bank_near_a_corner() {
    if std::env::var_os("CREABIM_DUMP_BANK").is_none() {
        return;
    }
    let (m, ground) = model();
    let sh = creabim_geom::earth::shaped(&m, &ground).unwrap();
    for (a, d) in &sh.banks[0] {
        if a.x > 1.0 && a.y > 0.5 {
            eprintln!(
                "{:6.2} {:6.2} {:6.2} -> {:6.2} {:6.2} {:6.2}",
                a.x, a.y, a.z, d.x, d.y, d.z
            );
        }
    }
}

/// The audit file's pit, its south edge upright, on the file's own
/// surveyed ground (CREABIM_AUDIT=model.json, CREABIM_DUMP=out.json).
#[test]
fn dump_the_audit_pit() {
    let (Some(src), Some(out)) = (
        std::env::var_os("CREABIM_AUDIT"),
        std::env::var_os("CREABIM_DUMP"),
    ) else {
        return;
    };
    let mut m = Model::from_json(&std::fs::read_to_string(src).unwrap()).unwrap();
    let pit = m.iter().find(|e| e.name == "Audit pit").unwrap().clone();
    let mut e = pit.clone();
    let mut centre = DVec2::ZERO;
    if let ElementKind::Site(Site {
        form: SiteForm::Pad {
            retained, boundary, ..
        },
        ..
    }) = &mut e.kind
    {
        retained.push(0);
        centre = boundary.iter().copied().sum::<DVec2>() / boundary.len() as f64;
    }
    let mut doc = creabim_core::Document::from_model(m.clone());
    doc.commit(creabim_core::Transaction::new("upright").replace(pit, e))
        .unwrap();
    m = doc.model().clone();
    let ground = m
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Site(s) if matches!(s.form, SiteForm::Terrain { .. }) => Some(s.clone()),
            _ => None,
        })
        .unwrap();
    let mesh = creabim_geom::parts::site_meshes(&m, &ground).remove(0).0;
    let near =
        |p: [f32; 3]| (p[0] as f64 - centre.x).abs() < 4.5 && (p[1] as f64 - centre.y).abs() < 4.0;
    let mut tris = vec![];
    for t in mesh.indices.chunks(3) {
        let p: Vec<[f64; 3]> = t
            .iter()
            .map(|&i| {
                let q = mesh.positions[i as usize];
                [q[0] as f64 - centre.x, q[1] as f64 - centre.y, q[2] as f64]
            })
            .collect();
        if t.iter().all(|&i| near(mesh.positions[i as usize])) {
            tris.push(p);
        }
    }
    let item = creabim_geom::scene::SceneItem {
        id: creabim_core::ElementId::new(),
        mesh: mesh.clone(),
        color: [0.5, 0.6, 0.4],
        surface: [0.5, 0.0, 1.0],
        ghost: false,
        layer: None,
        finish: None,
    };
    let edges: Vec<[[f64; 3]; 2]> = creabim_geom::section::feature_edges_over(&item, 0.94)
        .into_iter()
        .filter(|(a, b)| near(a.as_vec3().to_array()) && near(b.as_vec3().to_array()))
        .map(|(a, b)| {
            [
                [a.x - centre.x, a.y - centre.y, a.z],
                [b.x - centre.x, b.y - centre.y, b.z],
            ]
        })
        .collect();
    let z: f64 = tris.iter().flatten().map(|p| p[2]).fold(f64::MIN, f64::max);
    eprintln!(
        "audit pit: centre {centre}, top {z:.2}, {} tris, {} edges",
        tris.len(),
        edges.len()
    );
    std::fs::write(
        out,
        serde_json::json!({ "tris": tris, "edges": edges }).to_string(),
    )
    .unwrap();
}

/// Contours don't stack along an upright cut: in plan they would pile up
/// into a band on its line, in 3D stand up the face as fins.
#[test]
fn contours_stay_off_an_upright_cut() {
    let (m, ground) = model();
    let id = m
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Site(s) if matches!(s.form, SiteForm::Terrain { .. })))
        .unwrap()
        .id;
    let mut g = ground.clone();
    if let SiteForm::Terrain { contour, .. } = &mut g.form {
        *contour = 0.1;
    }
    let items = creabim_geom::parts::site_plan_with(&m, id, &g, &creabim_core::SiteView::default());
    // along the south edge's line, between its corners: at most once
    // over (one contour may run along its foot), never stacked
    let along: f64 = items
        .iter()
        .filter_map(|it| match it {
            creabim_geom::plan::PlanItem::Line {
                pen: creabim_geom::plan::Pen::Contour | creabim_geom::plan::Pen::ContourHeavy,
                a,
                b,
                ..
            } => {
                let m = (*a + *b) / 2.0;
                ((m.y + 1.5).abs() < 0.03 && m.x.abs() < 1.9).then(|| a.distance(*b))
            }
            _ => None,
        })
        .sum();
    eprintln!("contour length on the cut's line: {along:.2} m (the edge is 4 m)");
    assert!(
        along <= 4.0 + 1e-6,
        "contours stack along the cut: {along:.2} m"
    );
}

/// Every pad, pit and trench of a site (CREABIM_SITE=model.json): where
/// the 3D lines round it turn sharply (zigzag teeth) and how much contour
/// lies stacked on its upright edges. Writes each work's mesh beside it
/// for a picture (CREABIM_DUMP=folder).
#[test]
fn check_every_work_of_a_site() {
    let Some(src) = std::env::var_os("CREABIM_SITE") else {
        return;
    };
    let dump = std::env::var_os("CREABIM_DUMP");
    let m = Model::from_json(&std::fs::read_to_string(src).unwrap()).unwrap();
    let ground = m
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Site(s) if matches!(s.form, SiteForm::Terrain { .. }) => Some(s.clone()),
            _ => None,
        })
        .unwrap();
    let mesh = creabim_geom::parts::site_meshes(&m, &ground).remove(0).0;
    let item = creabim_geom::scene::SceneItem {
        id: creabim_core::ElementId::new(),
        mesh: mesh.clone(),
        color: [0.5, 0.6, 0.4],
        surface: [0.5, 0.0, 1.0],
        ghost: false,
        layer: None,
        finish: None,
    };
    let all_edges = creabim_geom::section::feature_edges_over(&item, 0.94);
    let id = m
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Site(s) if matches!(s.form, SiteForm::Terrain { .. })))
        .unwrap()
        .id;
    let plan =
        creabim_geom::parts::site_plan_with(&m, id, &ground, &creabim_core::SiteView::default());
    let mut works: Vec<(String, Vec<DVec2>, Vec<usize>)> = m
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Site(Site {
                form: SiteForm::Pad {
                    boundary, retained, ..
                },
                ..
            }) => Some((e.name.clone(), boundary.clone(), retained.clone())),
            _ => None,
        })
        .collect();
    works.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, outline, retained) in works {
        let (lo, hi) = outline.iter().fold(
            (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)),
            |(lo, hi), p| (lo.min(*p), hi.max(*p)),
        );
        let (lo, hi) = (lo - DVec2::splat(3.5), hi + DVec2::splat(3.5));
        let inside = |p: DVec3| p.x > lo.x && p.y > lo.y && p.x < hi.x && p.y < hi.y;
        use std::collections::HashMap;
        // creases one could see: points a centimetre apart are one, and
        // edges shorter than two are left out
        let key = |p: DVec3| {
            (
                (p.x * 1e2).round() as i64,
                (p.y * 1e2).round() as i64,
                (p.z * 1e2).round() as i64,
            )
        };
        let mut at: HashMap<_, Vec<DVec3>> = HashMap::new();
        for (a, b) in all_edges
            .iter()
            .filter(|(a, b)| inside(*a) && inside(*b) && a.distance(*b) > 0.02)
        {
            at.entry(key(*a))
                .or_default()
                .push((*b - *a).normalize_or_zero());
            at.entry(key(*b))
                .or_default()
                .push((*a - *b).normalize_or_zero());
        }
        let turns = at
            .values()
            .filter(|ds| ds.len() == 2 && ds[0].dot(ds[1]) > -0.87)
            .count();
        // contour along each upright edge, against the edge's length
        let n = outline.len();
        let mut stacked = 0.0;
        let mut length = 0.0;
        for e in &retained {
            let (a, b) = (outline[*e], outline[(*e + 1) % n]);
            length += a.distance(b);
            let d = (b - a).normalize();
            for it in &plan {
                if let creabim_geom::plan::PlanItem::Line {
                    pen: creabim_geom::plan::Pen::Contour | creabim_geom::plan::Pen::ContourHeavy,
                    a: p,
                    b: q,
                    ..
                } = it
                {
                    let mid = (*p + *q) / 2.0;
                    let t = (mid - a).dot(d);
                    let off = (mid - a).perp_dot(d).abs();
                    if off < 0.05 && t > 0.05 && t < a.distance(b) - 0.05 {
                        stacked += p.distance(*q);
                    }
                }
            }
        }
        eprintln!("{name:34} sharp turns {turns:3}   contour on upright edges {stacked:6.2} m of {length:5.2} m");
        if let Some(dir) = &dump {
            let c = (lo + hi) / 2.0;
            let mut tris = vec![];
            for t in mesh.indices.chunks(3) {
                let p: Vec<DVec3> = t
                    .iter()
                    .map(|&i| glam::Vec3::from(mesh.positions[i as usize]).as_dvec3())
                    .collect();
                if p.iter().all(|q| inside(*q)) {
                    tris.push(
                        p.iter()
                            .map(|q| [q.x - c.x, q.y - c.y, q.z])
                            .collect::<Vec<_>>(),
                    );
                }
            }
            let edges: Vec<[[f64; 3]; 2]> = all_edges
                .iter()
                .filter(|(a, b)| inside(*a) && inside(*b))
                .map(|(a, b)| [[a.x - c.x, a.y - c.y, a.z], [b.x - c.x, b.y - c.y, b.z]])
                .collect();
            let file =
                std::path::Path::new(dir).join(format!("{}.json", name.split(' ').next().unwrap()));
            std::fs::write(
                file,
                serde_json::json!({ "tris": tris, "edges": edges }).to_string(),
            )
            .unwrap();
        }
    }
}

/// The site plan's lines round each work of a site, for a picture:
/// CREABIM_SITE=model.json CREABIM_PLAN=folder writes `<letter>.plan.json`.
#[test]
fn plan_of_every_work() {
    let (Some(src), Some(dir)) = (
        std::env::var_os("CREABIM_SITE"),
        std::env::var_os("CREABIM_PLAN"),
    ) else {
        return;
    };
    use creabim_geom::plan::PlanItem;
    let m = Model::from_json(&std::fs::read_to_string(src).unwrap()).unwrap();
    let (id, ground) = m
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Site(s) if matches!(s.form, SiteForm::Terrain { .. }) => {
                Some((e.id, s.clone()))
            }
            _ => None,
        })
        .unwrap();
    let view = creabim_core::SiteView {
        banks: true,
        ..creabim_core::SiteView::default()
    };
    let mut plan = creabim_geom::parts::site_plan_with(&m, id, &ground, &view);
    for e in m.iter() {
        if matches!(&e.kind, ElementKind::Site(s) if matches!(s.form, SiteForm::Pad { .. })) {
            if let ElementKind::Site(s) = &e.kind {
                plan.extend(creabim_geom::parts::site_plan_with(&m, e.id, s, &view));
            }
        }
    }
    for e in m.iter() {
        let ElementKind::Site(Site {
            form: SiteForm::Pad { boundary, .. },
            ..
        }) = &e.kind
        else {
            continue;
        };
        let (lo, hi) = boundary.iter().fold(
            (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)),
            |(lo, hi), p| (lo.min(*p), hi.max(*p)),
        );
        let (lo, hi) = (lo - DVec2::splat(4.0), hi + DVec2::splat(4.0));
        let ins = |p: DVec2| p.x > lo.x && p.y > lo.y && p.x < hi.x && p.y < hi.y;
        let mut items = vec![];
        for it in &plan {
            match it {
                PlanItem::Line { pen, a, b, .. } if ins(*a) || ins(*b) => {
                    items.push(serde_json::json!({"k": "line", "pen": format!("{pen:?}"), "p": [[a.x, a.y], [b.x, b.y]]}));
                }
                PlanItem::Polygon { pen, points, .. } if points.iter().any(|p| ins(*p)) => {
                    items.push(serde_json::json!({"k": "poly", "pen": format!("{pen:?}"), "p": points.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>()}));
                }
                PlanItem::Fill { points, .. } if points.iter().any(|p| ins(*p)) => {
                    items.push(serde_json::json!({"k": "fill", "pen": "Fill", "p": points.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>()}));
                }
                _ => {}
            }
        }
        let sh = creabim_geom::earth::shaped(&m, &ground).unwrap();
        let banks: Vec<Vec<[f64; 6]>> = sh
            .banks
            .iter()
            .filter(|b| b.iter().any(|(a, _)| ins(a.truncate())))
            .map(|b| {
                b.iter()
                    .map(|(a, d)| [a.x, a.y, a.z, d.x, d.y, d.z])
                    .collect()
            })
            .collect();
        let file = std::path::Path::new(&dir)
            .join(format!("{}.plan.json", e.name.split(' ').next().unwrap()));
        std::fs::write(file, serde_json::json!({"lo": [lo.x, lo.y], "hi": [hi.x, hi.y], "outline": boundary.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>(), "items": items, "banks": banks}).to_string()).unwrap();
    }
}

/// A section across a site, its lines for a picture: CREABIM_SITE=model.json
/// CREABIM_SECTION="x0,y0,x1,y1,depth,out.json".
#[test]
fn section_of_a_site() {
    let (Some(src), Some(spec)) = (
        std::env::var_os("CREABIM_SITE"),
        std::env::var("CREABIM_SECTION").ok(),
    ) else {
        return;
    };
    let v: Vec<&str> = spec.split(',').collect();
    let f = |i: usize| v[i].parse::<f64>().unwrap();
    let m = Model::from_json(&std::fs::read_to_string(src).unwrap()).unwrap();
    let d = creabim_geom::section::section_view(
        &m,
        DVec2::new(f(0), f(1)),
        DVec2::new(f(2), f(3)),
        f(4),
        true,
        false,
    );
    let mut items = vec![];
    for it in &d.items {
        match it {
            creabim_geom::plan::PlanItem::Line { pen, a, b, .. } => {
                items.push(serde_json::json!({"k": "line", "pen": format!("{pen:?}"), "p": [[a.x, a.y], [b.x, b.y]]}))
            }
            creabim_geom::plan::PlanItem::Fill { points, .. } => {
                items.push(serde_json::json!({"k": "fill", "pen": "Fill", "p": points.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>()}))
            }
            _ => {}
        }
    }
    std::fs::write(v[5], serde_json::json!({ "items": items }).to_string()).unwrap();
}

/// The drawn ground's heights near a point, for checking its rim:
/// CREABIM_SITE=model.json CREABIM_RIM="x,y0,y1".
#[test]
fn ground_heights_along() {
    let (Some(src), Some(spec)) = (
        std::env::var_os("CREABIM_SITE"),
        std::env::var("CREABIM_RIM").ok(),
    ) else {
        return;
    };
    let v: Vec<f64> = spec.split(',').map(|t| t.parse().unwrap()).collect();
    let m = Model::from_json(&std::fs::read_to_string(src).unwrap()).unwrap();
    let ground = m
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Site(s) if matches!(s.form, SiteForm::Terrain { .. }) => Some(s.clone()),
            _ => None,
        })
        .unwrap();
    let mesh = creabim_geom::parts::site_meshes(&m, &ground).remove(0).0;
    let all: Vec<DVec3> = mesh
        .positions
        .iter()
        .map(|p| glam::Vec3::from(*p).as_dvec3())
        .collect();
    let mut pts: Vec<_> = all
        .iter()
        .filter(|p| (p.x - v[0]).abs() < 0.2 && p.y >= v[1] && p.y <= v[2])
        .collect();
    eprintln!("v {v:?}");
    eprintln!(
        "{} points, {} near, x {:?} y {:?}",
        all.len(),
        pts.len(),
        all.iter()
            .fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.x), a.1.max(p.x))),
        all.iter()
            .fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.y), a.1.max(p.y)))
    );
    pts.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap());
    for p in pts {
        eprintln!("{:7.3} {:7.3} {:7.3}", p.x, p.y, p.z);
    }
}

#[test]
fn ground_rim_heights() {
    let Some(src) = std::env::var_os("CREABIM_SITE") else {
        return;
    };
    let m = Model::from_json(&std::fs::read_to_string(src).unwrap()).unwrap();
    let ground = m
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Site(s) if matches!(s.form, SiteForm::Terrain { .. }) => Some(s.clone()),
            _ => None,
        })
        .unwrap();
    if let SiteForm::Terrain {
        data,
        points,
        boundary,
        ..
    } = &ground.form
    {
        eprintln!(
            "survey {} points {} boundary {:?}",
            data.survey,
            points.len(),
            boundary
        );
    }
    let g = creabim_geom::ground::Ground::build(&ground).unwrap();
    let sh = creabim_geom::earth::shaped(&m, &ground).unwrap();
    for y in [20.5, 21.25, 22.0] {
        eprintln!(
            "y {y}: built {:?} existing {:?} shaped {:?}",
            g.height(DVec2::new(0.0, y)),
            sh.existing.height(DVec2::new(0.0, y)),
            sh.ground.height(DVec2::new(0.0, y))
        );
    }
}
