//! Review harness: writes OBJ meshes and SVG drawings of sample trees.
use creabim_core::tree::*;
use creabim_core::{Element, ElementKind, Level, Model, Site, Transaction};
use creabim_geom::mesh::TriMesh;
use creabim_geom::plan::PlanView;
use glam::DVec2;
use std::io::Write;

fn obj(path: &str, parts: &[(&TriMesh, &str)]) {
    let mut f = std::fs::File::create(path).unwrap();
    let mut base = 1;
    for (m, name) in parts {
        writeln!(f, "o {name}").unwrap();
        for p in &m.positions {
            writeln!(f, "v {} {} {}", p[0], p[1], p[2]).unwrap();
        }
        for t in m.indices.chunks(3) {
            writeln!(f, "f {} {} {}", t[0] + base, t[1] + base, t[2] + base).unwrap();
        }
        base += m.positions.len() as u32;
    }
}

fn model_of(trees: &[(TreeShape, f64, f64, DVec2)]) -> Model {
    let lv = Element::new(
        "Site",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let l = lv.id;
    let mut m = Model::from_elements([lv]).unwrap();
    let mut tx = Transaction::new("plant");
    for (s, c, h, at) in trees {
        tx = tx.insert(Element::new(
            "Tree",
            ElementKind::Site(Site::tree(l, *at, s.clone(), *c, *h)),
        ));
    }
    tx.apply(&mut m).unwrap();
    m
}

fn main() {
    let out = std::env::args().nth(1).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    let presets = TreePreset::ALL;
    // 3D: each preset in leaf and bare, each limb setting bare
    let mut cases: Vec<(String, TreeShape, f64, f64)> = vec![];
    for p in presets {
        let (s, c, h) = p.shape();
        cases.push((format!("{:?}", p), s.clone(), c, h));
        cases.push((
            format!("{:?}-bare", p),
            TreeShape {
                leaves: LeafHabit::Deciduous,
                month: 1,
                own_month: true,
                ..s
            },
            c,
            h,
        ));
    }
    for b in Branching::ALL {
        let (s, c, h) = TreePreset::Shade.shape();
        cases.push((
            format!("Shade-{:?}-bare", b),
            TreeShape {
                branching: b,
                leaves: LeafHabit::Deciduous,
                month: 1,
                own_month: true,
                ..s
            },
            c,
            h,
        ));
    }
    for f in ClumpForm::ALL {
        let (s, c, h) = TreePreset::Shade.shape();
        cases.push((format!("Form-{:?}", f), TreeShape { form: f, ..s }, c, h));
    }
    for facets in 1..=5u8 {
        let (s, c, h) = TreePreset::Shade.shape();
        cases.push((
            format!("LowPoly-{facets}"),
            TreeShape {
                form: ClumpForm::LowPoly,
                facets,
                ..s
            },
            c,
            h,
        ));
    }
    for d in TreeDetail::ALL {
        let (s, c, h) = TreePreset::Shade.shape();
        cases.push((
            format!("Detail-{:?}", d),
            TreeShape { detail: d, ..s },
            c,
            h,
        ));
    }
    for (name, s, c, h) in &cases {
        let g = creabim_geom::tree::grow(s, *c, *h);
        let leaf = s.leaf_state(false).leaf;
        let fol = g.foliage_at(leaf);
        obj(
            &format!("{out}/{name}.obj"),
            &[
                (&g.wood, "wood"),
                (&g.twigs, if leaf < 0.75 { "wood" } else { "skip" }),
                (&fol, "foliage"),
            ],
        );
    }
    // 2D: plan symbols of every style, scale 1:100
    for p in TreePlan::ALL {
        for bold in [false, true] {
            let (s, c, h) = TreePreset::Shade.shape();
            let s = TreeShape {
                plan: p,
                plan_bold: bold,
                ..s
            };
            let g = creabim_geom::tree::grow(&s, c, h);
            let mut items = vec![];
            creabim_geom::tree::plan_symbol(
                &mut items,
                creabim_core::ElementId::new(),
                DVec2::ZERO,
                &g,
                &s,
            );
            let svg = PlanView { items }.to_svg(100.0);
            std::fs::write(
                format!("{out}/plan-{:?}{}.svg", p, if bold { "-bold" } else { "" }),
                svg,
            )
            .unwrap();
        }
    }
    // elevations and sections of every style, each preset in a row
    for e in TreeElevation::ALL {
        for limbs in [false, true] {
            let trees: Vec<_> = presets
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let (s, c, h) = p.shape();
                    (
                        TreeShape {
                            elevation: e,
                            elevation_limbs: limbs,
                            ..s
                        },
                        c,
                        h,
                        DVec2::new(i as f64 * 12.0, 0.0),
                    )
                })
                .collect();
            let m = model_of(&trees);
            let el = creabim_geom::section::section_view(
                &m,
                DVec2::new(-7.0, -8.0),
                DVec2::new(55.0, -8.0),
                20.0,
                false,
                false,
            );
            std::fs::write(
                format!(
                    "{out}/elev-{:?}{}.svg",
                    e,
                    if limbs { "-limbs" } else { "" }
                ),
                el.to_svg(100.0),
            )
            .unwrap();
            if !limbs {
                for cut in TreeCut::ALL {
                    let trees: Vec<_> = trees
                        .iter()
                        .map(|(s, c, h, at)| (TreeShape { cut, ..s.clone() }, *c, *h, *at))
                        .collect();
                    let m = model_of(&trees);
                    let sec = creabim_geom::section::section_view(
                        &m,
                        DVec2::new(-7.0, -0.05),
                        DVec2::new(55.0, -0.05),
                        10.0,
                        true,
                        false,
                    );
                    std::fs::write(
                        format!("{out}/sect-{:?}-{:?}.svg", e, cut),
                        sec.to_svg(100.0),
                    )
                    .unwrap();
                }
            }
        }
    }
    // timing
    let t0 = std::time::Instant::now();
    let trees: Vec<_> = (0..100)
        .map(|i| {
            let (s, c, h) = TreePreset::Shade.shape();
            let at = DVec2::new((i % 10) as f64 * 9.0, (i / 10) as f64 * 9.0);
            (
                TreeShape {
                    seed: seed_at(at),
                    elevation: TreeElevation::Wavy,
                    ..s
                },
                c,
                h,
                at,
            )
        })
        .collect();
    let m = model_of(&trees);
    let _ = creabim_geom::scene::build_scene(&m);
    let t1 = t0.elapsed();
    let t0 = std::time::Instant::now();
    let el = creabim_geom::section::section_view(
        &m,
        DVec2::new(-7.0, -8.0),
        DVec2::new(90.0, -8.0),
        100.0,
        false,
        false,
    );
    let t2 = t0.elapsed();
    let t0 = std::time::Instant::now();
    let sec = creabim_geom::section::section_view(
        &m,
        DVec2::new(-7.0, 40.3),
        DVec2::new(90.0, 40.3),
        50.0,
        true,
        false,
    );
    let t3 = t0.elapsed();
    println!(
        "100 trees: scene {:.1} ms, elevation {:.1} ms ({} items), section {:.1} ms ({} items)",
        t1.as_secs_f64() * 1e3,
        t2.as_secs_f64() * 1e3,
        el.items.len(),
        t3.as_secs_f64() * 1e3,
        sec.items.len()
    );
}
