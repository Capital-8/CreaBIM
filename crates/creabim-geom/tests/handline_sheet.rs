//! The starter house's first sheet in a hand line style, written as SVG for
//! a look by eye: `cargo test -p creabim-geom --test handline_sheet -- --ignored`
//! with CREABIM_HAND_OUT set to a folder (and CREABIM_HAND_STYLE to a style
//! name, Hand by default).

use creabim_core::styles::{active_pack, apply_pack};
use creabim_core::Document;

#[test]
#[ignore]
fn starter_sheet_in_a_hand_style() {
    let Ok(out) = std::env::var("CREABIM_HAND_OUT") else {
        return;
    };
    let style = std::env::var("CREABIM_HAND_STYLE").unwrap_or_else(|_| "Hand".into());
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    let mut pack = active_pack(doc.model()).into_owned();
    pack.line_style = Some(style.clone());
    doc.commit(apply_pack(doc.model(), &pack)).unwrap();
    let model = doc.model();
    let info = creabim_geom::sheet::SheetInfo::default();
    for (i, (id, _)) in creabim_geom::sheet::sheets(model)
        .into_iter()
        .enumerate()
        .take(2)
    {
        let svg = creabim_geom::sheet::sheet_svg(model, id, &info).unwrap();
        let path = format!("{out}/{}_{i}.svg", style.replace(' ', "_"));
        std::fs::write(&path, svg).unwrap();
        println!("{path}");
    }
}
