//! The open JSON format: the example builds, the export round-trips, the
//! schema and the docs match the format, and mistakes are reported where
//! they are.

use creabim_core::{ElementKind, Model};
use std::path::PathBuf;

fn repo(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn read(path: &str) -> String {
    std::fs::read_to_string(repo(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn build(text: &str) -> Model {
    let spec = creabim_spec::parse(text).unwrap_or_else(|e| panic!("{e:#?}"));
    match creabim_spec::build(&spec) {
        Ok(b) => b.model,
        Err(e) => panic!("{e:#?}"),
    }
}

/// The problems a file has, as "path: message" lines.
fn problems(text: &str) -> Vec<String> {
    let r = creabim_spec::parse(text).and_then(|s| creabim_spec::build(&s).map(|_| ()));
    match r {
        Ok(()) => vec![],
        Err(e) => e.iter().map(|p| p.to_string()).collect(),
    }
}

fn count(m: &Model, f: impl Fn(&ElementKind) -> bool) -> usize {
    m.iter().filter(|e| f(&e.kind)).count()
}

/// Export, build what was exported, export again: the same text.
fn round_trips(m: &Model) -> String {
    let first = creabim_spec::to_text(&creabim_spec::export(m).spec);
    let again = build(&first);
    let second = creabim_spec::to_text(&creabim_spec::export(&again).spec);
    assert_eq!(first, second, "export → build → export changed the file");
    first
}

#[test]
fn the_example_builds_a_small_house_with_sheets() {
    let m = build(&read("examples/small-house.json"));
    assert_eq!(count(&m, |k| matches!(k, ElementKind::Level(_))), 3);
    assert_eq!(count(&m, |k| matches!(k, ElementKind::Wall(_))), 7);
    assert_eq!(count(&m, |k| matches!(k, ElementKind::Opening(_))), 9);
    assert_eq!(count(&m, |k| matches!(k, ElementKind::Object(_))), 9);
    assert_eq!(count(&m, |k| matches!(k, ElementKind::Site(_))), 5);
    assert_eq!(count(&m, |k| matches!(k, ElementKind::Part(_))), 2);
    let sheets = creabim_geom::sheet::sheets(&m);
    assert_eq!(sheets.len(), 3);
    assert_eq!(count(&m, |k| matches!(k, ElementKind::Viewport(_))), 9);
    assert_eq!(count(&m, |k| matches!(k, ElementKind::Tag(_))), 8);
    assert_eq!(count(&m, |k| matches!(k, ElementKind::Note(_))), 2);
    // the exterior walls are of the layered type, as thick as its layers,
    // and run up to the roof level
    let south = m.iter().find(|e| e.name == "South wall").unwrap();
    let ElementKind::Wall(w) = &south.kind else {
        panic!()
    };
    assert!((w.thickness - 0.25).abs() < 1e-9);
    assert!((w.height - 2.7).abs() < 1e-9);
    assert!(south.of_type.is_some());
    // nothing stands in a doorway, and every sheet draws
    assert!(creabim_core::checks::objects_in_openings(&m).is_empty());
    let pdf = creabim_geom::sheet::sheets_pdf(
        &m,
        &creabim_geom::sheet::SheetInfo {
            project: "Small house".into(),
            date: "2026-11-04".into(),
            ..Default::default()
        },
    );
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn a_placed_symbol_round_trips() {
    // MU84: placed symbols were left out of the format
    let mut doc = creabim_core::Document::from_model(build(&read("examples/small-house.json")));
    let view = doc
        .model()
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::View(v) if matches!(v.kind, creabim_core::ViewKind::Plan { .. })))
        .unwrap()
        .id;
    let mut n = creabim_core::Note::symbol(view, glam::DVec2::new(1.0, 2.0), "slopes");
    n.text = "1:12".into();
    let s = n.symbol.as_mut().unwrap();
    s.style = Some("slopes.ramp_rise_arrow".into());
    s.angle = 30.0;
    doc.commit(
        creabim_core::Transaction::new("s").insert(creabim_core::Element::new(
            "Symbol",
            ElementKind::Note(n.clone()),
        )),
    )
    .unwrap();
    let text = round_trips(doc.model());
    assert!(text.contains("slopes.ramp_rise_arrow"));
    let again = build(&text);
    let back = again
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Note(m) if m.symbol.is_some() => Some(m.clone()),
            _ => None,
        })
        .expect("the symbol came back");
    assert_eq!(back.symbol, n.symbol);
    assert_eq!(back.text, "1:12");
}

#[test]
fn the_example_round_trips_through_export_json() {
    let m = build(&read("examples/small-house.json"));
    let text = round_trips(&m);
    // the export names elements by what they were called
    assert!(text.contains("\"id\": \"South wall\""));
    assert!(text.contains("\"wall\": \"Spine wall\""));
    assert!(text.contains("\"catalogue\": \"starter/sofa-3\""));
    let x = creabim_spec::export(&m);
    assert!(x.skipped.is_empty(), "{:?}", x.skipped);
    // and the rebuilt model has as much in it
    let again = build(&text);
    assert_eq!(again.len(), m.len());
}

#[test]
fn the_sample_houses_round_trip_for_what_the_format_covers() {
    for (name, m) in [
        ("starter", creabim_core::starter::starter_house()),
        ("typed", creabim_core::sample::typed_house()),
        ("layered", creabim_core::sample::layered_house()),
        ("furnished", creabim_core::sample::furnished_house()),
    ] {
        let x = creabim_spec::export(&m);
        let text = round_trips(&m);
        let again = build(&text);
        // the walls, doors and windows the format covers all come back
        for kind in ["Wall", "Opening", "Slab", "Roof", "Level", "View", "Sheet"] {
            let n = |m: &Model| {
                m.iter()
                    .filter(|e| e.kind.name() == kind)
                    .filter(|e| !matches!(&e.kind, ElementKind::Wall(w) if w.curtain.is_some()))
                    .count()
            };
            let skipped_hosts = x.skipped.iter().any(|s| s.contains("curtain"));
            if kind == "Opening" && skipped_hosts {
                continue;
            }
            assert_eq!(n(&again), n(&m), "{name}: {kind}");
        }
    }
}

#[test]
fn the_schema_file_is_the_one_the_format_makes() {
    let made = creabim_spec::schema().pretty();
    let path = repo("docs/creabim-model.schema.json");
    if std::env::var_os("CREABIM_BLESS").is_some() {
        std::fs::write(&path, &made).unwrap();
    }
    let file = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        file == made,
        "docs/creabim-model.schema.json is out of date: run \
         `CREABIM_BLESS=1 cargo test -p creabim-spec` to rewrite it"
    );
    // and it is JSON that describes every list of the format
    let v: serde_json::Value = serde_json::from_str(&file).unwrap();
    let props = &v["$defs"]["Model"]["properties"];
    for key in ["walls", "openings", "views", "sheets", "parts", "site"] {
        assert!(props.get(key).is_some(), "{key}");
    }
}

const START: &str =
    "<!-- reference:start (made by `CREABIM_BLESS=1 cargo test -p creabim-spec`) -->";
const END: &str = "<!-- reference:end -->";

#[test]
fn the_docs_reference_is_the_one_the_format_makes() {
    let path = repo("docs/json-format.md");
    let doc = std::fs::read_to_string(&path).unwrap();
    let (Some(a), Some(b)) = (doc.find(START), doc.find(END)) else {
        panic!("docs/json-format.md has no reference markers");
    };
    let made = format!("{START}\n\n{}{END}", creabim_spec::reference());
    let now = &doc[a..b + END.len()];
    if std::env::var_os("CREABIM_BLESS").is_some() && now != made {
        std::fs::write(&path, doc.replace(now, &made)).unwrap();
        return;
    }
    assert!(
        now == made,
        "the reference in docs/json-format.md is out of date: run \
         `CREABIM_BLESS=1 cargo test -p creabim-spec` to rewrite it"
    );
}

#[test]
fn the_example_in_the_docs_builds() {
    let doc = read("docs/json-format.md");
    let start = doc.find("```json\n").expect("a json example") + 8;
    let end = start + doc[start..].find("```").unwrap();
    let m = build(&doc[start..end]);
    assert!(count(&m, |k| matches!(k, ElementKind::Sheet(_))) >= 1);
    assert!(count(&m, |k| matches!(k, ElementKind::Opening(_))) >= 2);
}

const SMALL: &str = r#"{
  "version": 1,
  "levels": [{ "id": "L0", "elevation": 0 }],
  "walls": [
    { "id": "W1", "level": "L0", "start": [0, 0], "end": [5, 0], "thickness": 0.2 }
  ],
  "openings": [
    { "id": "D1", "kind": "door", "wall": "W1", "offset": 1, "width": 0.9, "height": 2.1 }
  ]
}"#;

#[test]
fn a_small_file_builds() {
    let m = build(SMALL);
    assert_eq!(m.len(), 3);
}

#[test]
fn unknown_fields_are_errors_with_their_path_and_a_suggestion() {
    let p = problems(&SMALL.replace("\"thickness\"", "\"thikness\""));
    assert!(
        p.iter().any(
            |p| p == "$.walls[0].thikness: unknown field `thikness`: did you mean `thickness`?"
        ),
        "{p:?}"
    );
    let p = problems(&SMALL.replace("\"door\"", "\"dor\""));
    assert!(
        p.iter()
            .any(|p| p.starts_with("$.openings[0].kind: unknown value `dor`: did you mean `door`")),
        "{p:?}"
    );
}

#[test]
fn wrong_types_and_missing_fields_say_where() {
    let p = problems(&SMALL.replace("[5, 0]", "[5, \"0\"]"));
    assert_eq!(p, ["$.walls[0].end[1]: expected a number, got \"0\""]);
    let p = problems(&SMALL.replace("\"start\": [0, 0], ", ""));
    assert_eq!(p, ["$.walls[0]: missing field `start`"]);
    let p = problems(&SMALL.replace("\"version\": 1", "\"version\": 7"));
    assert!(p[0].contains("version 7"), "{p:?}");
    let p = problems("{\"version\": 1, \"walls\": {}}");
    assert_eq!(p, ["$.walls: expected an array, got an object"]);
}

#[test]
fn references_are_checked_by_kind() {
    let p = problems(&SMALL.replace("\"wall\": \"W1\"", "\"wall\": \"W2\""));
    assert_eq!(
        p,
        ["$.openings[0].wall: there is no wall with the id \"W2\"; did you mean \"W1\"?"]
    );
    let p = problems(&SMALL.replace("\"wall\": \"W1\"", "\"wall\": \"L0\""));
    assert_eq!(p, ["$.openings[0].wall: \"L0\" is a level, not a wall"]);
    let p = problems(&SMALL.replace("\"id\": \"D1\"", "\"id\": \"W1\""));
    assert_eq!(
        p,
        ["$.openings[0].id: the id \"W1\" is already used by $.walls[0]"]
    );
    // a mistake in a wall isn't repeated by the doors in it
    let p = problems(&SMALL.replace("\"level\": \"L0\"", "\"level\": \"L9\""));
    assert_eq!(p.len(), 1, "{p:?}");
}

#[test]
fn elements_that_dont_fit_are_reported() {
    let p = problems(&SMALL.replace("\"offset\": 1", "\"offset\": 4.5"));
    assert_eq!(p.len(), 1);
    assert!(
        p[0].starts_with("$.openings[0]: it runs from 4.500 to 5.400 m"),
        "{p:?}"
    );
    let p = problems(&SMALL.replace(", \"thickness\": 0.2", ""));
    assert_eq!(
        p,
        ["$.walls[0]: a wall needs `thickness`, an `assembly` or a `type`"]
    );
    let p = problems(&SMALL.replace("\"thickness\": 0.2", "\"thickness\": -0.2"));
    assert_eq!(p, ["$.walls[0]: wall thickness must be positive, got -0.2"]);
}

#[test]
fn lengths_follow_the_units_field() {
    let mm = SMALL
        .replace("\"version\": 1", "\"version\": 1, \"units\": \"mm\"")
        .replace("[5, 0]", "[5000, 0]")
        .replace("\"thickness\": 0.2", "\"thickness\": 200")
        .replace("\"elevation\": 0", "\"elevation\": 0, \"height\": 3000")
        .replace(
            "\"offset\": 1, \"width\": 0.9, \"height\": 2.1",
            "\"offset\": 1000, \"width\": 900, \"height\": 2100",
        );
    let a = build(SMALL);
    let b = build(&mm);
    let text = |m: &Model| creabim_spec::to_text(&creabim_spec::export(m).spec);
    assert_eq!(text(&a), text(&b));
}

#[test]
fn fields_a_kind_does_not_use_are_errors() {
    let text = r#"{ "version": 1, "levels": [{ "id": "L0", "elevation": 0 }],
        "views": [{ "id": "S", "kind": "section", "level": "L0",
                    "start": [0, 0], "end": [1, 0], "depth": 5 }] }"#;
    assert_eq!(
        problems(text),
        ["$.views[0].level: `level` is not used by a section view"]
    );
}

#[test]
fn a_wall_drawn_by_a_layer_boundary_keeps_it() {
    let m = build(&SMALL.replace(
        "\"thickness\": 0.2 }",
        "\"thickness\": 0.2, \"location\": \"layer-boundary\", \"boundary\": 2 }",
    ));
    let w = m
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Wall(w) => Some(w.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(w.location, creabim_core::LocationLine::Boundary(2));
    let text = round_trips(&m);
    assert!(text.contains("\"boundary\": 2"), "{text}");
}

#[test]
fn a_part_drawn_on_a_face_keeps_its_plane() {
    let text = SMALL.replace(
        "\"openings\": [",
        r#""parts": [
    { "id": "V1", "level": "L0", "void": true, "outline": [[0, 0], [1, 0], [1, 1], [0, 1]],
      "base": -0.3, "height": 0.31,
      "plane": { "origin": [1, 0, 1], "x": [0, 1, 0], "y": [0, 0, 1] } }
  ],
  "openings": ["#,
    );
    let m = build(&text);
    let plane = m
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Part(p) => match &p.form {
                creabim_core::parts::Form::Extrusion { plane, .. } => *plane,
                _ => None,
            },
            _ => None,
        })
        .expect("a part on a plane");
    assert_eq!(plane.normal(), glam::DVec3::X);
    let out = round_trips(&m);
    assert!(out.contains("\"plane\""), "{out}");
    // axes that aren't square are an error where they are
    let p = problems(&text.replace("\"y\": [0, 0, 1]", "\"y\": [0, 1, 1]"));
    assert!(p.iter().any(|l| l.contains("plane")), "{p:?}");
}

#[test]
fn a_property_round_trips_with_its_edges_and_rules() {
    let mut m = creabim_core::starter::starter_house();
    let level = m.levels().next().unwrap().0.id;
    let mut site = creabim_core::Site::property(
        level,
        vec![
            glam::DVec2::new(-10.0, -10.0),
            glam::DVec2::new(20.0, -10.0),
            glam::DVec2::new(20.0, 25.0),
            glam::DVec2::new(-10.0, 25.0),
        ],
    );
    if let creabim_core::SiteForm::Property { edges, rules, .. } = &mut site.form {
        *edges = vec![
            creabim_core::LotEdge::Front,
            creabim_core::LotEdge::Open,
            creabim_core::LotEdge::Rear,
            creabim_core::LotEdge::Side,
        ];
        rules.height = 9.0;
        rules.coverage = 0.4;
        rules.angle = 45.0;
        rules.easements.push(creabim_core::Easement {
            name: "Drain".into(),
            edge: 2,
            width: 3.0,
        });
    }
    let mut tx = creabim_core::Transaction::new("Lot");
    tx = tx.insert(creabim_core::Element::new(
        "Lot",
        ElementKind::Site(site.clone()),
    ));
    let mut doc = creabim_core::Document::from_model(m);
    doc.commit(tx).unwrap();
    m = doc.model().clone();
    let text = round_trips(&m);
    assert!(text.contains("\"property\"") && text.contains("\"open\""));
    let again = build(&text);
    let lot = again.iter().find(|e| e.name == "Lot").unwrap();
    // ids are made afresh on building: the lot itself comes back
    let ElementKind::Site(back) = &lot.kind else {
        panic!("a site element")
    };
    assert_eq!(back.form, site.form);
}

#[test]
fn a_road_round_trips_with_its_levels_section_and_bays() {
    let mut m = creabim_core::starter::starter_house();
    let level = m.levels().next().unwrap().0.id;
    let mut site = creabim_core::Site::road(
        level,
        vec![
            glam::DVec2::new(-10.0, -10.0),
            glam::DVec2::new(20.0, -10.0),
            glam::DVec2::new(20.0, 25.0),
        ],
        6.0,
    );
    if let creabim_core::SiteForm::Road(r) = &mut site.form {
        r.path[2].z = 1.5;
        r.footpath = 1.8;
        r.crowned = false;
        r.parking.side = creabim_core::road::Side::Right;
        r.parking.angle = 60.0;
    }
    let mut doc = creabim_core::Document::from_model(m);
    doc.commit(
        creabim_core::Transaction::new("Road").insert(creabim_core::Element::new(
            "Drive",
            ElementKind::Site(site.clone()),
        )),
    )
    .unwrap();
    m = doc.model().clone();
    let text = round_trips(&m);
    assert!(
        text.contains("\"road\"") && text.contains("\"right\""),
        "{text}"
    );
    let again = build(&text);
    let drive = again.iter().find(|e| e.name == "Drive").unwrap();
    let ElementKind::Site(back) = &drive.kind else {
        panic!("a site element")
    };
    assert_eq!(back.form, site.form);
}

#[test]
fn drains_and_tanks_round_trip() {
    use creabim_core::services::{DrainUse, TankUse};
    let mut m = creabim_core::starter::starter_house();
    let level = m.levels().next().unwrap().0.id;
    let mut drain = creabim_core::Site::drain(
        level,
        vec![
            glam::DVec2::new(-2.0, -2.0),
            glam::DVec2::new(-2.0, -8.0),
            glam::DVec2::new(6.0, -8.0),
        ],
        DrainUse::Surface,
    );
    if let creabim_core::SiteForm::Drain(d) = &mut drain.form {
        d.diameter = 0.15;
        d.fall = 1.0;
        d.chambers = false;
    }
    let mut tank = creabim_core::Site::tank(level, glam::DVec2::new(8.0, -9.0), TankUse::Soakaway);
    if let creabim_core::SiteForm::Tank(t) = &mut tank.form {
        t.angle = 30.0;
        t.cover = 0.5;
    }
    let mut doc = creabim_core::Document::from_model(m);
    doc.commit(
        creabim_core::Transaction::new("Services")
            .insert(creabim_core::Element::new(
                "Run",
                ElementKind::Site(drain.clone()),
            ))
            .insert(creabim_core::Element::new(
                "Pit",
                ElementKind::Site(tank.clone()),
            )),
    )
    .unwrap();
    m = doc.model().clone();
    let text = round_trips(&m);
    assert!(
        text.contains("\"surface\"") && text.contains("\"soakaway\""),
        "{text}"
    );
    let again = build(&text);
    for (name, site) in [("Run", &drain), ("Pit", &tank)] {
        let e = again.iter().find(|e| e.name == name).unwrap();
        let ElementKind::Site(back) = &e.kind else {
            panic!("a site element")
        };
        assert_eq!(back.form, site.form);
    }
}

#[test]
fn paths_gullies_and_the_datum_round_trip() {
    let text = r#"{
        "version": 1,
        "datum": 100.0,
        "levels": [{"id": "Ground", "elevation": 0}],
        "site": [
            {"id": "Street", "kind": "road", "level": "Ground",
             "path": [[0, 0, 0], [30, 0, 1]], "road": {"width": 5.5, "gullies": 12}},
            {"id": "Walk", "kind": "path", "level": "Ground",
             "walk": {"path": [[0, 5], [10, 5], [10, 9]], "width": 1.0, "surface": "paving", "edging": 0}}
        ]
    }"#;
    let m = build(text);
    assert_eq!(m.project_info().map(|(_, p)| p.elevation), Some(100.0));
    let again = round_trips(&m);
    assert!(
        again.contains("\"datum\"") && again.contains("\"gullies\""),
        "{again}"
    );
    let walk = m.iter().find(|e| e.name == "Walk").unwrap();
    let ElementKind::Site(s) = &walk.kind else {
        panic!("a site element")
    };
    let creabim_core::SiteForm::Walk(w) = &s.form else {
        panic!("a path")
    };
    assert_eq!(w.surface, creabim_core::walk::Surface::Paving);
    assert_eq!(w.edging, 0.0);
    assert_eq!(creabim_core::site_level(&m, -0.5), "99.50");
}

#[test]
fn a_borehole_round_trips_with_its_log_and_its_log_view() {
    use creabim_core::investigation::{
        FieldTest, LogLayer, ProbeKind, SoilKind, TestKind, WaterStrike,
    };
    let mut m = creabim_core::starter::starter_house();
    let level = m.levels().next().unwrap().0.id;
    let mut bh = creabim_core::Site::probe(level, glam::DVec2::new(4.0, -6.0), ProbeKind::Borehole);
    if let creabim_core::SiteForm::Investigation(p) = &mut bh.form {
        p.ground = Some(-0.15);
        p.depth = 8.0;
        p.layers = vec![
            LogLayer {
                to: 0.3,
                soil: SoilKind::Topsoil,
                unit: "GT1".into(),
                description: "TOPSOIL".into(),
            },
            LogLayer {
                to: 8.0,
                soil: SoilKind::Clay,
                unit: String::new(),
                description: "Firm brown CLAY".into(),
            },
        ];
        p.water = vec![WaterStrike {
            depth: 2.5,
            rest: Some(2.1),
            note: "after 20 min".into(),
        }];
        p.tests = vec![FieldTest {
            depth: 1.5,
            kind: TestKind::Spt,
            value: 12.0,
            text: String::new(),
        }];
        p.method = "Window sample".into();
        p.report = "GI-2026-031".into();
    }
    let e = creabim_core::Element::new("BH1", ElementKind::Site(bh.clone()));
    let id = e.id;
    let mut doc = creabim_core::Document::from_model(m);
    doc.commit(
        creabim_core::Transaction::new("Ground investigation")
            .insert(e)
            .insert(creabim_core::Element::new(
                "BH1 log",
                ElementKind::View(creabim_core::View::borehole_log(id)),
            )),
    )
    .unwrap();
    m = doc.model().clone();
    let text = round_trips(&m);
    assert!(
        text.contains("\"investigation\"") && text.contains("\"borehole_log\""),
        "{text}"
    );
    let again = build(&text);
    let e = again.iter().find(|e| e.name == "BH1").unwrap();
    let ElementKind::Site(back) = &e.kind else {
        panic!("a site element")
    };
    assert_eq!(back.form, bh.form);
    assert!(again.iter().any(|v| matches!(&v.kind,
        ElementKind::View(v) if v.kind == creabim_core::ViewKind::BoreholeLog { probe: e.id })));
}

#[test]
fn how_a_stair_is_built_round_trips() {
    let text = r#"{
        "version": 1,
        "levels": [{"id": "Ground", "elevation": 0, "height": 3}],
        "stairs": [
            {"id": "S1", "level": "Ground", "start": [0, 0], "width": 1.0,
             "build": "cut_stringers"},
            {"id": "S2", "level": "Ground", "start": [5, 0], "width": 1.0}
        ]
    }"#;
    let m = build(text);
    let build_of = |name: &str| {
        let e = m.iter().find(|e| e.name == name).unwrap();
        let ElementKind::Stair(s) = &e.kind else {
            panic!("a stair")
        };
        s.build
    };
    use creabim_core::members::StairBuild;
    assert_eq!(build_of("S1"), StairBuild::CutStringers);
    assert_eq!(build_of("S2"), StairBuild::Concrete);
    let again = round_trips(&m);
    // concrete, the default, is left out
    assert_eq!(again.matches("\"build\"").count(), 1, "{again}");
}

#[test]
fn a_stairs_finishes_round_trip() {
    let text = r#"{
        "version": 1,
        "materials": [{"id": "Oak", "hatch": "timber"}],
        "levels": [{"id": "Ground", "elevation": 0, "height": 3}],
        "stairs": [
            {"id": "S1", "level": "Ground", "start": [0, 0], "width": 1.0,
             "finish": {"tread": 0.025, "tread_material": "Oak", "riser": 0.015, "nosing": 0.02}}
        ]
    }"#;
    let m = build(text);
    let e = m.iter().find(|e| e.name == "S1").unwrap();
    let ElementKind::Stair(s) = &e.kind else {
        panic!("a stair")
    };
    assert_eq!(s.finish.tread, 0.025);
    assert_eq!(s.finish.nosing, 0.02);
    let oak = m.iter().find(|e| e.name == "Oak").unwrap().id;
    assert_eq!(s.finish.tread_material, Some(oak));
    assert_eq!(s.finish.riser_material, None);
    let again = round_trips(&m);
    assert!(again.contains("\"tread_material\": \"Oak\""), "{again}");
    // a material that isn't there is reported where it is named
    let bad = text.replace("\"tread_material\": \"Oak\"", "\"tread_material\": \"Elm\"");
    assert!(
        problems(&bad)
            .iter()
            .any(|p| p.contains("finish.tread_material")),
        "{:?}",
        problems(&bad)
    );
}

#[test]
fn a_guards_infill_height_and_newels_round_trip() {
    let text = r#"{
        "version": 1,
        "levels": [{"id": "Ground", "elevation": 0, "height": 3}],
        "railings": [
            {"id": "G1", "level": "Ground", "path": [[0, 0, 0], [3, 0, 0]], "height": 0.9,
             "infill": "balusters", "guard_height": 1.07, "newels": true},
            {"id": "G2", "level": "Ground", "path": [[0, 2, 0], [3, 2, 0]], "infill": "glass"}
        ]
    }"#;
    let m = build(text);
    let rail = |n: &str| match &m.iter().find(|e| e.name == n).unwrap().kind {
        ElementKind::Railing(r) => r.clone(),
        _ => panic!("a railing"),
    };
    use creabim_core::members::RailInfill;
    let g1 = rail("G1");
    assert_eq!(g1.infill, RailInfill::Balusters);
    assert_eq!(g1.guard_height, Some(1.07));
    assert!(g1.newels);
    assert_eq!(rail("G2").infill, RailInfill::Glass);
    let again = round_trips(&m);
    assert!(
        again.contains("\"balusters\"") && again.contains("\"glass\""),
        "{again}"
    );
}

#[test]
fn a_stairs_rule_set_round_trips_and_ontario_is_left_out() {
    let text = r#"{
        "version": 1,
        "levels": [{"id": "Ground", "elevation": 0, "height": 3}],
        "stairs": [
            {"id": "S1", "level": "Ground", "start": [0, 0], "width": 1.0, "rules": "uk"},
            {"id": "S2", "level": "Ground", "start": [5, 0], "width": 1.0},
            {"id": "S3", "level": "Ground", "start": [9, 0], "width": 1.0, "rules": "Office stair"}
        ]
    }"#;
    let m = build(text);
    use creabim_core::stairrules::StairCode;
    let code = |n: &str| match &m.iter().find(|e| e.name == n).unwrap().kind {
        ElementKind::Stair(s) => s.rules.clone(),
        _ => panic!("a stair"),
    };
    assert_eq!(code("S1"), StairCode::Uk);
    assert_eq!(code("S2"), StairCode::Ontario);
    assert_eq!(code("S3"), StairCode::Own("Office stair".into()));
    let again = round_trips(&m);
    assert!(
        again.contains("\"uk\"") && again.contains("\"Office stair\""),
        "{again}"
    );
    assert!(!again.contains("\"ontario\""), "{again}");
}

#[test]
fn a_walk_line_section_round_trips_with_its_stair() {
    let text = r#"{
        "version": 1,
        "levels": [{"id": "Ground", "elevation": 0, "height": 3}],
        "stairs": [{"id": "S1", "level": "Ground", "start": [0, 0], "width": 1.0}],
        "views": [{"id": "S1 walk", "kind": "walk_section", "stair": "S1", "scale": 50}]
    }"#;
    let m = build(text);
    let s1 = m.iter().find(|e| e.name == "S1").unwrap().id;
    assert!(m.iter().any(|v| matches!(&v.kind,
        ElementKind::View(v) if v.kind == creabim_core::ViewKind::WalkSection { stair: s1 })));
    let again = round_trips(&m);
    assert!(again.contains("\"walk_section\""), "{again}");
    // it needs its stair
    let bad = text.replace(", \"stair\": \"S1\"", "");
    assert!(
        problems(&bad).iter().any(|p| p.contains("needs `stair`")),
        "{:?}",
        problems(&bad)
    );
}

#[test]
fn a_viewport_following_a_stair_round_trips() {
    let text = r#"{
        "version": 1,
        "levels": [{"id": "Ground", "elevation": 0, "height": 3}],
        "stairs": [{"id": "S1", "level": "Ground", "start": [0, 0], "width": 1.0}],
        "views": [{"id": "Stair plan", "kind": "plan", "level": "Ground", "scale": 50}],
        "sheets": [{"id": "A501", "size": "A2", "viewports": [
            {"view": "Stair plan", "at": [100, 100], "crop": {"min": [-1, -1], "max": [6, 2]}, "fit": "S1"}
        ]}]
    }"#;
    let m = build(text);
    let s1 = m.iter().find(|e| e.name == "S1").unwrap().id;
    assert!(m.iter().any(|v| matches!(&v.kind,
        ElementKind::Viewport(v) if v.fit == Some(s1))));
    let again = round_trips(&m);
    assert!(again.contains("\"fit\": \"S1\""), "{again}");
}

#[test]
fn a_ramp_round_trips_with_its_slope() {
    let text = r#"{
        "version": 1,
        "levels": [{"id": "Ground", "elevation": 0, "height": 3}],
        "stairs": [{"id": "R1", "level": "Ground", "start": [0, 0], "line": [[0, 0], [9, 0]],
                    "width": 1.5, "height": 0.75, "ramp": 0.0833333}]
    }"#;
    let m = build(text);
    let r = m
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Stair(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap();
    assert!(r.is_ramp());
    assert!((r.tread - creabim_core::members::RAMP_SLICE).abs() < 1e-9);
    let again = round_trips(&m);
    assert!(again.contains("\"ramp\": 0.0833"), "{again}");
    // a flight set by hand keeps its length
    let mut m2 = m.clone();
    let id = m2
        .iter()
        .find(|e| matches!(e.kind, ElementKind::Stair(_)))
        .unwrap()
        .id;
    let before = m2.get(id).unwrap().clone();
    let mut e = before.clone();
    if let ElementKind::Stair(s) = &mut e.kind {
        s.line = vec![
            glam::DVec2::ZERO,
            glam::DVec2::new(5.0, 0.0),
            glam::DVec2::new(5.0, 5.0),
        ];
        s.pieces.set_treads(0, Some(60));
    }
    creabim_core::Transaction::new("t")
        .replace(before, e)
        .apply(&mut m2)
        .unwrap();
    let back = build(&round_trips(&m2));
    let s = back
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Stair(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(s.pieces.treads(0), Some(60));
}

#[test]
fn a_stacked_stair_round_trips_with_its_foot() {
    // the copy may come before its foot in the file
    let text = r#"{
        "version": 1,
        "levels": [{"id": "Ground", "elevation": 0, "height": 3},
                   {"id": "First", "elevation": 3, "height": 3}],
        "stairs": [{"id": "S1 first", "level": "First", "start": [0, 0], "width": 1.0, "typical": "S1"},
                   {"id": "S1", "level": "Ground", "start": [0, 0], "width": 1.0}]
    }"#;
    let m = build(text);
    let s1 = m.iter().find(|e| e.name == "S1").unwrap().id;
    let copy = m.iter().find(|e| e.name == "S1 first").unwrap().id;
    assert_eq!(creabim_core::stairstack::stack(&m, copy), vec![s1, copy]);
    let again = round_trips(&m);
    assert!(again.contains("\"typical\": \"S1\""), "{again}");
    let bad = text.replace("\"typical\": \"S1\"", "\"typical\": \"Nope\"");
    assert!(!problems(&bad).is_empty());
}

#[test]
fn a_split_stair_round_trips() {
    let text = r#"{
        "version": 1,
        "levels": [{"id": "Ground", "elevation": 0, "height": 3}],
        "stairs": [{"id": "S", "level": "Ground", "start": [0, 0], "width": 1.0,
                    "line": [[0, 0], [2.75, 0], [2.75, 1.5], [0, 1.5]], "split": true}]
    }"#;
    let m = build(text);
    let s = m
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Stair(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap();
    assert!(s.split && s.mirror(3.0).is_some());
    let again = round_trips(&m);
    assert!(again.contains("\"split\": true"), "{again}");
}

/// A fitting's own light (Visuals REV02) is written with its objects and
/// read back onto their type; with none stated, none is kept.
#[test]
fn a_fittings_own_light_round_trips() {
    use creabim_spec::format as f;
    let m = build(&read("examples/small-house.json"));
    let mut spec = creabim_spec::export(&m).spec;
    assert!(spec.objects.iter().all(|o| o.light.is_none()));
    let key = spec.objects[0].catalogue.clone();
    let own = f::ObjectLight {
        on: true,
        lumens: 2400.0,
        kelvin: 4000.0,
        beam: f::LightBeam::Narrow,
        group: Some("Kitchen downlights".into()),
    };
    spec.objects[0].light = Some(own.clone());
    let text = creabim_spec::to_text(&spec);
    assert!(
        text.contains("\"lumens\": 2400") && text.contains("\"narrow\""),
        "{text}"
    );
    let m2 = build(&text);
    let again = creabim_spec::export(&m2).spec;
    // every object of that catalogue key has it now; no other has any
    for o in &again.objects {
        assert_eq!(
            o.light.as_ref(),
            (o.catalogue == key).then_some(&own),
            "{}",
            o.id
        );
    }
    round_trips(&m2);
}

#[test]
fn a_roof_window_round_trips_set_in_its_roof() {
    let mut m = creabim_core::sample::layered_house();
    let roof = m
        .iter()
        .find(|e| matches!(e.kind, ElementKind::Roof(_)))
        .unwrap()
        .id;
    let o = creabim_core::roofwindow::new_on(&m, roof).unwrap();
    creabim_core::Transaction::new("t")
        .insert(creabim_core::Element::new(
            "Roof window",
            ElementKind::Opening(o.clone()),
        ))
        .apply(&mut m)
        .unwrap();
    let text = round_trips(&m);
    assert!(text.contains("\"roof_at\""), "{text}");
    let again = build(&text);
    let back = again
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Opening(w) if w.roof_at.is_some() => Some(w.clone()),
            _ => None,
        })
        .expect("the roof window came back");
    assert!(back.roof_at.unwrap().distance(o.roof_at.unwrap()) < 1e-6);
    assert_eq!(back.frame.grid, o.frame.grid);
    assert!(matches!(
        again.get(back.host).map(|e| &e.kind),
        Some(ElementKind::Roof(_))
    ));
}

#[test]
fn glass_door_leaves_round_trip_with_their_fittings_and_panes() {
    use creabim_core::construction::{GlassFittings, LeafStyle};
    let mut m = creabim_core::sample::sample_house();
    let doors: Vec<_> = m
        .iter()
        .filter(|e| matches!(&e.kind, ElementKind::Opening(o) if o.kind == creabim_core::OpeningKind::Door))
        .map(|e| e.id)
        .collect();
    assert!(doors.len() >= 2);
    let mut tx = creabim_core::Transaction::new("t");
    for (k, id) in doors.iter().take(2).enumerate() {
        let before = m.get(*id).unwrap().clone();
        let mut e = before.clone();
        if let ElementKind::Opening(o) = &mut e.kind {
            let c = &mut o.frame.construction;
            if k == 0 {
                c.leaf = LeafStyle::AllGlass;
                c.fittings = GlassFittings::BottomRail;
            } else {
                c.leaf = LeafStyle::SlimSteel;
                c.panes = Some([3, 4]);
            }
        }
        tx = tx.replace(before, e);
    }
    tx.apply(&mut m).unwrap();
    let text = round_trips(&m);
    assert!(
        text.contains("\"all-glass\"") && text.contains("\"bottom-rail\""),
        "{text}"
    );
    assert!(text.contains("\"leaf_panes_across\""), "{text}");
    let again = build(&text);
    let made: Vec<_> = again
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Opening(o) => Some(o.frame.construction.clone()),
            _ => None,
        })
        .collect();
    assert!(made
        .iter()
        .any(|c| (c.leaf, c.fittings) == (LeafStyle::AllGlass, GlassFittings::BottomRail)));
    assert!(made
        .iter()
        .any(|c| (c.leaf, c.panes) == (LeafStyle::SlimSteel, Some([3, 4]))));
}

#[test]
fn windows_round_a_corner_round_trip_with_their_arrangement() {
    use creabim_core::{arrangement, CornerJoint, OpeningKind};
    let mut m = creabim_core::sample::layered_house();
    let tx = m
        .iter()
        .filter(|e| matches!(&e.kind, ElementKind::Opening(o) if o.kind == OpeningKind::Window && o.roof_at.is_none()))
        .find_map(|e| {
            [true, false]
                .into_iter()
                .find_map(|end| arrangement::add_round_corner(&m, e.id, end, CornerJoint::Post).ok())
        })
        .expect("a window by a corner");
    tx.apply(&mut m).unwrap();
    let text = round_trips(&m);
    assert!(
        text.contains("\"arrangement\"") && text.contains("\"corner\""),
        "{text}"
    );
    let back = build(&text);
    let corners = back
        .iter()
        .filter(|e| matches!(&e.kind, ElementKind::Opening(o) if o.arrangement.is_some_and(|a| a.corner_start || a.corner_end)))
        .count();
    assert_eq!(corners, 2);
}
