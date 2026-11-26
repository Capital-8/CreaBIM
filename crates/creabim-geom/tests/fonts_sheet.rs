//! A sheet whose text styles use a shipped font carries that font inside
//! its SVG and PDF (fonts 2026-11-16). With CREABIM_FONT_OUT set to a
//! folder, the sheet is written there for a look by eye.

use creabim_core::{Document, ElementKind, Font};

fn in_font(font: Font) -> Document {
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    let styles: Vec<_> = doc
        .model()
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::TextStyle(_)))
        .map(|e| e.id)
        .collect();
    for id in styles {
        doc.edit(id, "font", |e| {
            if let ElementKind::TextStyle(s) = &mut e.kind {
                s.font = font;
            }
        })
        .unwrap();
    }
    doc
}

#[test]
fn a_shipped_font_goes_inside_svg_and_pdf() {
    let doc = in_font(Font::Structa);
    let model = doc.model();
    let info = creabim_geom::sheet::SheetInfo::default();
    let (sid, _) = creabim_geom::sheet::sheets(model)
        .into_iter()
        .nth(1)
        .unwrap();
    let svg = creabim_geom::sheet::sheet_svg(model, sid, &info).unwrap();
    assert!(svg.contains("@font-face { font-family: 'CreaBIM Structa'"));
    assert!(svg.contains("font-family=\"'CreaBIM Structa'"));
    let pdf = creabim_geom::sheet::sheets_pdf_of(model, &info, &[sid]);
    let text = creabim_geom::paper::pdf_readable(&pdf);
    assert!(text.contains("/BaseFont /CreaBIMStructa"));
    assert!(text.contains("/FontFile2"));
    assert!(text.contains("/B1 "));
    assert!(!text.contains("{font:"), "every placeholder resolved");
    if let Ok(out) = std::env::var("CREABIM_FONT_OUT") {
        std::fs::write(format!("{out}/structa.svg"), &svg).unwrap();
        std::fs::write(format!("{out}/structa.pdf"), &pdf).unwrap();
        for f in [Font::TechHand, Font::Osifont, Font::Kalam, Font::Jost] {
            let d = in_font(f);
            let (s, _) = creabim_geom::sheet::sheets(d.model())
                .into_iter()
                .nth(1)
                .unwrap();
            let svg = creabim_geom::sheet::sheet_svg(d.model(), s, &info).unwrap();
            std::fs::write(format!("{out}/{f:?}.svg"), svg).unwrap();
        }
    }
}

#[test]
fn a_standard_font_embeds_nothing() {
    let doc = in_font(Font::Sans);
    let model = doc.model();
    let info = creabim_geom::sheet::SheetInfo::default();
    let (sid, _) = creabim_geom::sheet::sheets(model)
        .into_iter()
        .nth(1)
        .unwrap();
    let pdf = creabim_geom::sheet::sheets_pdf_of(model, &info, &[sid]);
    assert!(!String::from_utf8_lossy(&pdf).contains("/FontFile2"));
}
