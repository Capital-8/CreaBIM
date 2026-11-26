//! The starter house's plan sheet with grid bubbles drawn as a CreaBIM
//! symbol, written as SVG for a look by eye:
//! `cargo test -p creabim-geom --test symbols_sheet -- --ignored` with
//! CREABIM_SYMBOL_OUT set to a folder (and CREABIM_SYMBOL to a style id).

use creabim_core::styles::{active_pack, apply_pack};
use creabim_core::Document;

#[test]
fn grid_symbols_replace_the_bubbles() {
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    let mut pack = active_pack(doc.model()).into_owned();
    pack.symbols
        .insert("grids".into(), "grids.hexagon_structural_node".into());
    doc.commit(apply_pack(doc.model(), &pack)).unwrap();
    let model = doc.model();
    let plan = model
        .iter()
        .find(|e| matches!(&e.kind, creabim_core::ElementKind::View(v) if matches!(v.kind, creabim_core::ViewKind::Plan { .. })))
        .unwrap()
        .id;
    let d = creabim_geom::section::view_drawing(model, plan).unwrap();
    let symbols = d
        .items
        .iter()
        .filter(|i| matches!(i, creabim_geom::plan::PlanItem::Symbol { .. }))
        .count();
    assert!(symbols > 0, "grid bubbles became symbols");
    // no grid bubble is left as a plain label
    assert!(!d.items.iter().any(
        |i| matches!(i, creabim_geom::plan::PlanItem::Label { id, text, .. }
        if matches!(model.get(*id).map(|e| &e.kind), Some(creabim_core::ElementKind::Grid(_)))
            && creabim_geom::plan::is_bubble(text))
    ));
}

/// Every mark kind given its third style.
fn all_picked() -> Document {
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    let mut pack = active_pack(doc.model()).into_owned();
    for k in creabim_core::symbols::MARK_KINDS {
        let kd = creabim_core::symbols::kind(k).unwrap();
        pack.symbols.insert(k.into(), kd.styles[2].id.clone());
    }
    doc.commit(apply_pack(doc.model(), &pack)).unwrap();
    doc
}

#[test]
fn section_marks_draw_with_their_symbol() {
    let doc = all_picked();
    let model = doc.model();
    let plan = model
        .iter()
        .find(|e| matches!(&e.kind, creabim_core::ElementKind::View(v) if matches!(v.kind, creabim_core::ViewKind::Plan { .. })))
        .unwrap()
        .id;
    let d = creabim_geom::section::view_drawing(model, plan).unwrap();
    let marks: Vec<_> = d
        .items
        .iter()
        .filter_map(|i| match i {
            creabim_geom::plan::PlanItem::Symbol { symbol, .. } => {
                Some(symbol.id.split('.').next().unwrap())
            }
            _ => None,
        })
        .collect();
    let has_sections = model.iter().any(|e| {
        matches!(&e.kind, creabim_core::ElementKind::View(v) if v.callout.is_none()
            && matches!(v.kind, creabim_core::ViewKind::Section { .. }))
    });
    if has_sections {
        assert!(
            marks.iter().any(|k| *k == "sections" || *k == "elevations"),
            "{marks:?}"
        );
    }
}

#[test]
fn a_placed_symbol_draws_its_style_turned() {
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    let plan = doc
        .model()
        .iter()
        .find(|e| matches!(&e.kind, creabim_core::ElementKind::View(v) if matches!(v.kind, creabim_core::ViewKind::Plan { .. })))
        .unwrap()
        .id;
    let mut n = creabim_core::Note::symbol(plan, glam::DVec2::new(2.0, 2.0), "slopes");
    n.text = "1:12".into();
    n.symbol.as_mut().unwrap().angle = 90.0;
    let e = creabim_core::Element::new("Symbol", creabim_core::ElementKind::Note(n));
    let id = e.id;
    doc.commit(creabim_core::Transaction::new("symbol").insert(e))
        .unwrap();
    let d = creabim_geom::section::view_drawing(doc.model(), plan).unwrap();
    let found = d.items.iter().find_map(|i| match i {
        creabim_geom::plan::PlanItem::Symbol {
            id: x,
            symbol,
            angle,
            text,
            ..
        } if *x == id => Some((symbol.id.clone(), *angle, text.clone())),
        _ => None,
    });
    let (style, angle, text) = found.expect("the placed symbol is drawn");
    assert!(style.starts_with("slopes."), "{style}");
    assert!((angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    assert_eq!(text, "1:12");
}

#[test]
#[ignore]
fn starter_sheets_with_every_mark_picked() {
    let Ok(out) = std::env::var("CREABIM_SYMBOL_OUT") else {
        return;
    };
    let mut doc = all_picked();
    // the sheet marks on the first plan sheet
    let (s1, _) = creabim_geom::sheet::sheets(doc.model())
        .into_iter()
        .nth(1)
        .unwrap();
    let mut t = creabim_core::Transaction::new("marks");
    for m in [
        creabim_core::PaperMark::north_arrow(s1, glam::DVec2::new(40.0, 60.0)),
        creabim_core::PaperMark::scale_bar(s1, glam::DVec2::new(30.0, 30.0), 50.0),
        creabim_core::PaperMark::cloud(s1, glam::DVec2::new(250.0, 150.0), "B"),
    ] {
        t = t.insert(creabim_core::Element::new(
            "mark",
            creabim_core::ElementKind::PaperMark(m),
        ));
    }
    doc.commit(t).unwrap();
    let model = doc.model();
    let info = creabim_geom::sheet::SheetInfo::default();
    for (i, (sid, _)) in creabim_geom::sheet::sheets(model).into_iter().enumerate() {
        let svg = creabim_geom::sheet::sheet_svg(model, sid, &info).unwrap();
        std::fs::write(format!("{out}/all_{i}.svg"), svg).unwrap();
    }
}

#[test]
#[ignore]
fn starter_sheet_with_symbols() {
    let Ok(out) = std::env::var("CREABIM_SYMBOL_OUT") else {
        return;
    };
    let id =
        std::env::var("CREABIM_SYMBOL").unwrap_or_else(|_| "grids.hexagon_structural_node".into());
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    let mut pack = active_pack(doc.model()).into_owned();
    pack.symbols.insert("grids".into(), id.clone());
    doc.commit(apply_pack(doc.model(), &pack)).unwrap();
    let model = doc.model();
    let info = creabim_geom::sheet::SheetInfo::default();
    let (sid, _) = creabim_geom::sheet::sheets(model)
        .into_iter()
        .nth(1)
        .unwrap();
    let svg = creabim_geom::sheet::sheet_svg(model, sid, &info).unwrap();
    std::fs::write(format!("{out}/{id}.svg"), svg).unwrap();
}

#[test]
fn tag_text_fits_inside_a_diamond() {
    // MU84: "W06" crowded a small diamond's edges
    let s = creabim_core::symbols::style("tags.diamond_opening_tag").unwrap();
    let (text_w, text_h, mm) = (6.0, 2.5, 1.0);
    let sh = creabim_geom::symbol::tag_shapes(s, glam::DVec2::ZERO, text_w, text_h, mm);
    // the outline's width where the text's top and bottom edges run
    let (lo, hi) = sh.bounds().unwrap();
    let step = (hi.x - lo.x) / creabim_geom::symbol::span(s).0;
    let slot = glam::DVec2::from(s.text.as_ref().unwrap().at);
    let inside = creabim_geom::symbol::inner_width(s, slot, text_h * 0.5 / step).unwrap() * step;
    assert!(inside >= text_w + text_h * 0.5, "{inside} for {text_w}");
}
