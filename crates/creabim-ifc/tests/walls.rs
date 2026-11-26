//! Wall behaviour through IFC: a wall's axis is its location line (with
//! the layer set usage measured from it), a top attached to a roof is
//! clipped by its underside, a top tied to a level gives the height the
//! level makes, and doors say which way they are hung.

use creabim_core::{
    sample::{layered_house, sample_house},
    Document, Element, ElementId, ElementKind, LocationLine, Model, Wall, WallTop,
};
use creabim_ifc::{export_ifc4, import_ifc, ExportOptions};

fn named(m: &Model, name: &str) -> ElementId {
    m.iter().find(|e| e.name == name).expect(name).id
}

fn edit(m: &Model, id: ElementId, f: impl FnOnce(&mut Element)) -> Model {
    let mut doc = Document::from_model(m.clone());
    doc.edit(id, "edit", f).unwrap();
    doc.model().clone()
}

fn wall(m: &Model, name: &str) -> Wall {
    match &m.get(named(m, name)).unwrap().kind {
        ElementKind::Wall(w) => w.clone(),
        _ => unreachable!(),
    }
}

#[test]
fn a_walls_axis_is_its_location_line() {
    let m = layered_house();
    let id = named(&m, "GF Wall 1");
    let m = edit(&m, id, |e| {
        if let ElementKind::Wall(w) = &mut e.kind {
            w.location = LocationLine::ExteriorFace;
        }
    });
    let w = wall(&m, "GF Wall 1");
    let t = w.thickness / 2.0;
    let ifc = export_ifc4(&m, &ExportOptions::default());
    // its layers start at the axis now: no offset to the exterior face
    assert!(
        ifc.contains(".AXIS2.,.POSITIVE.,0.,$"),
        "the usage starts at the axis"
    );
    let back = import_ifc(&ifc).unwrap().model;
    let b = wall(&back, "GF Wall 1");
    assert_eq!(b.location, LocationLine::ExteriorFace);
    assert!(b.start.distance(w.start) < 1e-6 && b.end.distance(w.end) < 1e-6);
    assert!((b.thickness - 2.0 * t).abs() < 1e-6);
    // the others still sit on their centrelines
    assert_eq!(wall(&back, "GF Wall 2").location, LocationLine::Centre);
}

#[test]
fn walls_attached_to_the_roof_are_clipped_by_it() {
    let m = layered_house();
    assert_eq!(
        m.iter()
            .filter(|e| matches!(&e.kind, ElementKind::Wall(w) if !w.attach_top.is_empty()))
            .count(),
        4
    );
    let ifc = export_ifc4(&m, &ExportOptions::default());
    assert!(ifc.contains("'Body','Clipping'"));
    assert!(ifc.matches("IFCPOLYGONALBOUNDEDHALFSPACE(").count() >= 4);
    // they still come back as walls, as tall as their gables
    let back = import_ifc(&ifc).unwrap();
    let gable = wall(&back.model, "FF Wall 2");
    assert!(gable.height > 4.5, "{}", gable.height);
}

#[test]
fn a_wall_another_tool_clipped_comes_in_with_its_clip() {
    // Highland Haven (Bonsai): gable walls clipped by the roof's planes
    // came in as native walls at their full height, through the roof.
    // From another tool the clip is applied and the wall is its shape, as
    // a mesh; CreaBIM's own walls still come back native (above)
    let m = layered_house();
    let ifc =
        export_ifc4(&m, &ExportOptions::default()).replacen("'CreaBIM','')", "'Bonsai','')", 1);
    assert!(
        ifc.contains("'Bonsai','')"),
        "the header names another tool"
    );
    let back = import_ifc(&ifc).unwrap().model;
    let e = back
        .iter()
        .find(|e| e.name == "FF Wall 2")
        .expect("FF Wall 2");
    let ElementKind::FreeMesh(f) = &e.kind else {
        panic!("came in as {}, its clip ignored", e.ifc_class());
    };
    // a gable: well under the box round it
    let mesh = f.evaluated().mesh;
    let (lo, hi) = mesh.vertices.iter().fold(
        (glam::DVec3::splat(f64::MAX), glam::DVec3::splat(f64::MIN)),
        |(a, b), v| (a.min(*v), b.max(*v)),
    );
    let s = hi - lo;
    assert!(
        mesh.volume() < 0.95 * s.x * s.y * s.z,
        "{} in a box of {s}",
        mesh.volume()
    );
}

#[test]
fn a_top_tied_to_a_level_exports_the_height_it_makes() {
    let m = sample_house();
    let first = named(&m, "First Floor");
    let id = named(&m, "GF Wall 1");
    let m = edit(&m, id, |e| {
        if let ElementKind::Wall(w) = &mut e.kind {
            w.top = WallTop::UpToLevel {
                level: first,
                offset: -0.25,
            };
        }
    });
    let mut doc = Document::from_model(m);
    doc.commit(creabim_core::levels::set(doc.model(), first, 3.4, 3.0))
        .unwrap();
    let w = wall(doc.model(), "GF Wall 1");
    assert!((w.height - 3.15).abs() < 1e-9);
    let back = import_ifc(&export_ifc4(doc.model(), &ExportOptions::default()))
        .unwrap()
        .model;
    assert!((wall(&back, "GF Wall 1").height - 3.15).abs() < 1e-6);
}

#[test]
fn doors_say_which_way_they_are_hung() {
    let m = sample_house();
    let ifc = export_ifc4(&m, &ExportOptions::default());
    let left = ifc.matches(".SINGLE_SWING_LEFT.").count();
    let id = named(&m, "Front Door");
    let m = edit(&m, id, |e| {
        if let ElementKind::Opening(o) = &mut e.kind {
            o.flip_hand = true;
        }
    });
    let ifc = export_ifc4(&m, &ExportOptions::default());
    assert_eq!(ifc.matches(".SINGLE_SWING_LEFT.").count(), left - 1);
    assert_eq!(ifc.matches(".SINGLE_SWING_RIGHT.").count(), 1);
    // opening out as well, it is turned round: seen the way it swings
    // its hinge is on the left again
    let m = edit(&m, id, |e| {
        if let ElementKind::Opening(o) = &mut e.kind {
            o.flip_facing = true;
        }
    });
    let ifc = export_ifc4(&m, &ExportOptions::default());
    assert_eq!(ifc.matches(".SINGLE_SWING_LEFT.").count(), left);
    // and the door still comes back in its wall
    let back = import_ifc(&ifc).unwrap().model;
    let o = back
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Opening(o) if e.name == "Front Door" => Some(o.clone()),
            _ => None,
        })
        .unwrap();
    assert!((o.width - 1.0).abs() < 1e-6 && (o.offset - 1.6).abs() < 1e-6);
}

#[test]
fn a_double_door_goes_out_and_comes_back_as_a_pair() {
    let m = sample_house();
    let id = named(&m, "Front Door");
    let m = edit(&m, id, |e| {
        if let ElementKind::Opening(o) = &mut e.kind {
            o.frame
                .set_operation(o.kind, creabim_core::Operation::DoubleSwing);
            o.width = 1.6;
        }
    });
    let ifc = export_ifc4(&m, &ExportOptions::default());
    assert_eq!(ifc.matches(".DOUBLE_DOOR_SINGLE_SWING.").count(), 1);
    let back = import_ifc(&ifc).unwrap().model;
    let o = back
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Opening(o) if e.name == "Front Door" => Some(o.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        o.frame.operation(o.kind),
        creabim_core::Operation::DoubleSwing
    );
    assert!((o.width - 1.6).abs() < 1e-6);
}

#[test]
fn sliding_doors_go_out_with_their_ifc_operations() {
    use creabim_core::Operation as O;
    let m = sample_house();
    let id = named(&m, "Front Door");
    for (op, ifc_op, back_op) in [
        (O::SurfaceSliding, ".SLIDING_TO_", O::SurfaceSliding),
        (
            O::SurfaceSlidingPair,
            ".DOUBLE_DOOR_SLIDING.",
            O::SurfaceSlidingPair,
        ),
        // IFC has no pocket door: it comes back as the slider it is
        (O::Pocket, ".SLIDING_TO_", O::SurfaceSliding),
        (O::MultiSlide, "'MULTI_PANEL_SLIDING'", O::MultiSlide),
    ] {
        let m = edit(&m, id, |e| {
            if let ElementKind::Opening(o) = &mut e.kind {
                o.frame.set_operation(o.kind, op);
            }
        });
        let ifc = export_ifc4(&m, &ExportOptions::default());
        assert_eq!(ifc.matches(ifc_op).count(), 1, "{op:?}");
        let back = import_ifc(&ifc).unwrap().model;
        let o = back
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Opening(o) if e.name == "Front Door" => Some(o.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(o.frame.operation(o.kind), back_op);
    }
}

#[test]
fn windows_go_out_with_their_cells_and_come_back() {
    use creabim_core::cells::WindowStyle;
    let m = sample_house();
    let id = m
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Opening(o) if o.kind == creabim_core::OpeningKind::Window))
        .unwrap()
        .id;
    for (style, part) in [
        (WindowStyle::Casement, ".SINGLE_PANEL."),
        (WindowStyle::CasementPair, ".DOUBLE_PANEL_VERTICAL."),
        (WindowStyle::Awning, ".SINGLE_PANEL."),
        (WindowStyle::Sliding, ".DOUBLE_PANEL_VERTICAL."),
        (WindowStyle::Louvred, ".SINGLE_PANEL."),
    ] {
        let m = edit(&m, id, |e| {
            if let ElementKind::Opening(o) = &mut e.kind {
                o.frame.grid = style.grid(false, [1, 1]);
            }
        });
        let ifc = export_ifc4(&m, &ExportOptions::default());
        assert!(ifc.contains(part), "{style:?}");
        assert!(ifc.contains("'CreaBIM_Opening'"), "{style:?}");
        let back = import_ifc(&ifc).unwrap().model;
        let want = match &m.get(id).unwrap().kind {
            ElementKind::Opening(o) => o.frame.grid.clone(),
            _ => unreachable!(),
        };
        assert!(
            back.iter()
                .any(|e| matches!(&e.kind, ElementKind::Opening(o) if o.frame.grid == want)),
            "{style:?}"
        );
    }
}

#[test]
fn a_door_with_a_sidelight_and_transom_goes_out_as_its_door_and_comes_back_whole() {
    use creabim_core::cells::{Assembly, Cell};
    use creabim_core::{OpeningKind, Operation};
    let m = sample_house();
    let id = m
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Opening(o) if o.kind == OpeningKind::Door))
        .unwrap()
        .id;
    let m = edit(&m, id, |e| {
        if let ElementKind::Opening(o) = &mut e.kind {
            o.frame
                .set_operation(OpeningKind::Door, Operation::DoubleSwing);
            o.frame.grid = Assembly {
                left: Some(0.25),
                transom: Some((0.2, Cell::default())),
                ..Assembly::of(o.frame.grid.clone())
            }
            .grid();
        }
    });
    let ifc = export_ifc4(&m, &ExportOptions::default());
    // its door's operation, and the whole grid in CreaBIM's property set
    assert!(ifc.contains(".DOUBLE_DOOR_SINGLE_SWING."));
    assert!(ifc.contains("'CreaBIM_Opening'"));
    let back = import_ifc(&ifc).unwrap().model;
    let want = match &m.get(id).unwrap().kind {
        ElementKind::Opening(o) => o.frame.grid.clone(),
        _ => unreachable!(),
    };
    assert!(back
        .iter()
        .any(|e| matches!(&e.kind, ElementKind::Opening(o) if o.frame.grid == want)));
}

#[test]
fn an_arched_door_cuts_its_arch_and_comes_back_arched() {
    use creabim_core::cells::{Assembly, Cell, Outline};
    use creabim_core::OpeningKind;
    let m = sample_house();
    let id = m
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Opening(o) if o.kind == OpeningKind::Door))
        .unwrap()
        .id;
    let m = edit(&m, id, |e| {
        if let ElementKind::Opening(o) = &mut e.kind {
            o.frame.grid = Assembly {
                transom: Some((0.25, Cell::default())),
                ..Assembly::of(o.frame.grid.clone())
            }
            .grid();
            o.frame.grid.outline = Outline::Round;
        }
    });
    let ifc = export_ifc4(&m, &ExportOptions::default());
    // the opening and the head's frame and glass shaped in elevation
    let shaped = ifc.matches("IFCARBITRARYCLOSEDPROFILEDEF").count();
    let before = export_ifc4(&sample_house(), &ExportOptions::default())
        .matches("IFCARBITRARYCLOSEDPROFILEDEF")
        .count();
    assert!(shaped >= before + 3, "{shaped} {before}");
    assert!(ifc.contains(".SINGLE_SWING_"));
    let back = import_ifc(&ifc).unwrap().model;
    let name = &m.get(id).unwrap().name;
    let o = back
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Opening(o) if &e.name == name => Some(o.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(o.frame.grid.outline, Outline::Round);
}

#[test]
fn a_doors_make_and_declared_ratings_go_out_and_come_back() {
    use creabim_core::construction::{Construction, LeafStyle, Ratings, Trim};
    use creabim_core::OpeningKind;
    let m = sample_house();
    let id = m
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Opening(o) if o.kind == OpeningKind::Door))
        .unwrap()
        .id;
    let built = Construction {
        leaf: LeafStyle::PartGlazed,
        fittings: Default::default(),
        panes: None,
        trim: Trim {
            lining: 0.018,
            architrave: 0.07,
            threshold: 0.02,
        },
        ratings: Ratings {
            load_bearing: None,
            external: None,
            fire: "FD30".into(),
            acoustic: "Rw 32 dB".into(),
            security: "RC2".into(),
            u_value: Some(1.4),
            accessible: true,
        },
    };
    let m = edit(&m, id, |e| {
        if let ElementKind::Opening(o) = &mut e.kind {
            o.frame.construction = built.clone();
        }
    });
    let ifc = export_ifc4(&m, &ExportOptions::default());
    // where IFC keeps ratings, for other programs to read
    assert!(ifc.contains("'Pset_DoorCommon'"));
    assert!(ifc.contains("'FireRating',$,IFCLABEL('FD30')"));
    assert!(ifc.contains("IFCTHERMALTRANSMITTANCEMEASURE(1.4"));
    let back = import_ifc(&ifc).unwrap().model;
    let name = &m.get(id).unwrap().name;
    let o = back
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Opening(o) if &e.name == name => Some(o.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(o.frame.construction, built);
    // from another program, without CreaBIM's own set: the ratings alone
    let other = ifc.replace("'Construction'", "'Elsewhere'");
    let back = import_ifc(&other).unwrap().model;
    let o = back
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Opening(o) if &e.name == name => Some(o.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(o.frame.construction.ratings, built.ratings);
}

/// Pivot, double action, folding, sectional, roller and revolving doors
/// go out as the operation types IFC has for them (a pivot or a sectional
/// door as user defined) and come back as they were, a folding door with
/// its panels.
#[test]
fn pivot_double_action_and_folding_doors_go_out_and_come_back() {
    use creabim_core::{OpeningKind, Operation};
    let m = sample_house();
    let id = m
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Opening(o) if o.kind == OpeningKind::Door))
        .unwrap()
        .id;
    let name = m.get(id).unwrap().name.clone();
    for (op, ifc_type) in [
        (Operation::Pivot, ".USERDEFINED.,'OFFSET_PIVOT'"),
        (Operation::CentrePivot, ".USERDEFINED.,'CENTRE_PIVOT'"),
        (Operation::DoubleAction, ".DOUBLE_SWING_"),
        (Operation::DoubleActionPair, ".DOUBLE_DOOR_DOUBLE_SWING."),
        (Operation::Bifold, ".FOLDING_TO_"),
        (Operation::BifoldPair, ".DOUBLE_DOOR_FOLDING."),
        (Operation::Folding, ".FOLDING_TO_"),
        (Operation::Sectional, ".USERDEFINED.,'SECTIONAL_OVERHEAD'"),
        (Operation::Roller, ".ROLLINGUP."),
        (Operation::Revolving, ".REVOLVING."),
    ] {
        let m = edit(&m, id, |e| {
            if let ElementKind::Opening(o) = &mut e.kind {
                o.frame.set_operation(OpeningKind::Door, op);
                if op == Operation::Folding {
                    o.frame.set_folding_panels(OpeningKind::Door, 4);
                }
            }
        });
        let ifc = export_ifc4(&m, &ExportOptions::default());
        assert!(ifc.contains(ifc_type), "{op:?}");
        let back = import_ifc(&ifc).unwrap().model;
        let o = back
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Opening(o) if e.name == name => Some(o.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(o.frame.operation(OpeningKind::Door), op, "{op:?}");
        if op == Operation::Folding {
            assert_eq!(o.frame.panels(), 4);
        }
    }
}

/// Tilt and turn, hung, pivot, bifold and guillotine windows go out with their grid
/// and come back as they were.
#[test]
fn tilt_turn_hung_pivot_and_bifold_windows_go_out_and_come_back() {
    use creabim_core::cells::WindowStyle;
    use creabim_core::OpeningKind;
    let m = sample_house();
    let id = m
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Opening(o) if o.kind == OpeningKind::Window))
        .unwrap()
        .id;
    let name = m.get(id).unwrap().name.clone();
    for style in [
        WindowStyle::TiltTurn,
        WindowStyle::SingleHung,
        WindowStyle::DoubleHung,
        WindowStyle::HorizontalPivot,
        WindowStyle::VerticalPivot,
        WindowStyle::Bifold,
        WindowStyle::Guillotine,
    ] {
        let m = edit(&m, id, |e| {
            if let ElementKind::Opening(o) = &mut e.kind {
                o.frame.grid = style.grid(false, [1, 1]);
            }
        });
        let ifc = export_ifc4(&m, &ExportOptions::default());
        let back = import_ifc(&ifc).unwrap().model;
        let o = back
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Opening(o) if e.name == name => Some(o.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(WindowStyle::of(&o.frame.grid), Some(style), "{style:?}");
        // hung sashes are two panels one above the other
        if matches!(style, WindowStyle::SingleHung | WindowStyle::DoubleHung) {
            assert!(ifc.contains(".DOUBLE_PANEL_HORIZONTAL."));
        }
    }
}

/// A roof window goes out as a skylight filling an opening in its roof,
/// and comes back set in that roof where it was, opening as it did.
#[test]
fn a_roof_window_goes_out_and_comes_back() {
    use creabim_core::cells::WindowStyle;
    use creabim_core::{Document, Opening, OpeningFrame, OpeningKind, Transaction};
    let m = layered_house();
    let (roof_id, at) = m
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Roof(r) => {
                let f = r.faces().into_iter().find(|f| f.g.length() > 0.1)?;
                let mid = f.outline.iter().copied().sum::<glam::DVec2>() / f.outline.len() as f64;
                Some((e.id, mid))
            }
            _ => None,
        })
        .expect("a sloped roof");
    for style in WindowStyle::ROOF {
        let mut frame = OpeningFrame::for_kind(OpeningKind::Window);
        frame.grid = style.grid(false, [1, 1]);
        let w = Element::new(
            "Roof window",
            ElementKind::Opening(Opening {
                host: roof_id,
                kind: OpeningKind::Window,
                offset: 0.0,
                sill: 0.0,
                width: 0.78,
                height: 1.4,
                material: None,
                frame,
                flip_hand: false,
                flip_facing: false,
                roof_at: Some(at),
                roof_build: Default::default(),
                arrangement: None,
            }),
        );
        let mut doc = Document::from_model(m.clone());
        doc.commit(Transaction::new("roof window").insert(w))
            .unwrap();
        let ifc = export_ifc4(doc.model(), &ExportOptions::default());
        assert!(ifc.contains(".SKYLIGHT."), "{style:?}");
        let back = import_ifc(&ifc).unwrap().model;
        let o = back
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Opening(o) if e.name == "Roof window" => Some(o.clone()),
                _ => None,
            })
            .expect("the roof window came back");
        assert_eq!(o.host, roof_id, "{style:?}");
        assert!(o.roof_at.unwrap().distance(at) < 1e-6, "{style:?}");
        assert!((o.width - 0.78).abs() < 1e-6 && (o.height - 1.4).abs() < 1e-6);
        assert_eq!(WindowStyle::of(&o.frame.grid), Some(style), "{style:?}");
    }
}

#[test]
fn glass_door_leaves_go_out_and_come_back() {
    use creabim_core::construction::{Construction, GlassFittings, LeafStyle};
    use creabim_core::OpeningKind;
    let m = sample_house();
    let id = m
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Opening(o) if o.kind == OpeningKind::Door))
        .unwrap()
        .id;
    let name = m.get(id).unwrap().name.clone();
    for built in [
        Construction {
            leaf: LeafStyle::AllGlass,
            fittings: GlassFittings::Rails,
            ..Construction::default()
        },
        Construction {
            leaf: LeafStyle::French,
            panes: Some([2, 5]),
            ..Construction::default()
        },
        Construction {
            leaf: LeafStyle::SlimSteel,
            ..Construction::default()
        },
    ] {
        let m = edit(&m, id, |e| {
            if let ElementKind::Opening(o) = &mut e.kind {
                o.frame.construction = built.clone();
            }
        });
        let ifc = export_ifc4(&m, &ExportOptions::default());
        let back = import_ifc(&ifc).unwrap().model;
        let o = back
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Opening(o) if e.name == name => Some(o.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(o.frame.construction, built);
    }
}

#[test]
fn french_and_slim_steel_windows_go_out_and_come_back() {
    use creabim_core::construction::{Construction, LeafStyle};
    use creabim_core::OpeningKind;
    let m = sample_house();
    let id = m
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Opening(o) if o.kind == OpeningKind::Window))
        .unwrap()
        .id;
    let name = m.get(id).unwrap().name.clone();
    for built in [
        Construction {
            leaf: LeafStyle::French,
            panes: Some([2, 3]),
            ..Construction::default()
        },
        Construction {
            leaf: LeafStyle::SlimSteel,
            ..Construction::default()
        },
    ] {
        let m = edit(&m, id, |e| {
            if let ElementKind::Opening(o) = &mut e.kind {
                o.frame.construction = built.clone();
            }
        });
        let back = import_ifc(&export_ifc4(&m, &ExportOptions::default()))
            .unwrap()
            .model;
        let o = back
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Opening(o) if e.name == name => Some(o.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(o.frame.construction, built);
    }
}
