//! The permit set: ARCH D sheets (36 × 24 in) with the project's title
//! block down the right-hand edge.

use super::drawings::{detail_boxes, wall_at, Views};
use super::*;
use crate::papermark::{PaperMark, PaperMarkKind};
use crate::titleblock::{Corner, TextAlign, TitleItem};
use crate::*;
use glam::DVec2;

/// ARCH D, landscape, in millimetres.
const SHEET: (f64, f64) = (914.4, 609.6);

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

pub fn build(b: &mut Builder, views: &Views) {
    let mut tx = Transaction::new("Ridgeline House: title block");
    tx = b.keep(
        "Ridgeline title block",
        Element::new(
            "Ridgeline title block",
            ElementKind::SheetTemplate(title_block()),
        ),
        tx,
    );
    for (name, size, bold) in [
        ("Cover title", 14.0, true),
        ("Cover heading", 6.0, true),
        ("Cover text", 4.0, false),
    ] {
        let style = TextStyle {
            size,
            bold,
            purpose: StyleUse::Notes,
            ..Default::default()
        };
        tx = b.keep(name, Element::new(name, ElementKind::TextStyle(style)), tx);
    }
    b.commit(tx);
    let template = b.id("Ridgeline title block");

    type Place = (ElementId, DVec2, DVec2, Option<[DVec2; 2]>);
    let crop = |lo: DVec2, hi: DVec2| Some([lo, hi]);
    let plan_crop = crop(p(-27.0, -20.5), p(60.0, 37.0));
    let plan_at = (v(415.0, 310.0), p(23.5, 6.5));
    let sched = &views.schedules;
    let boxes = detail_boxes();
    let w = wall_at();
    let mid = |i: usize| {
        let (_, lo, hi) = boxes[i];
        v((lo.0 + hi.0) / 2.0, (lo.1 + hi.1) / 2.0)
    };
    let set: Vec<(&str, &str, Vec<Place>)> = vec![
        (
            "G-001",
            "Cover",
            vec![(b.id("3D of the lot"), v(545.0, 290.0), DVec2::ZERO, None)],
        ),
        (
            "C-101",
            "Site Plan",
            vec![
                (
                    views.site,
                    v(300.0, 345.0),
                    p(36.0, 0.0),
                    crop(p(-56.0, -74.0), p(128.0, 74.0)),
                ),
                (
                    views.site_section,
                    v(300.0, 80.0),
                    v(ft(70.0), 0.0),
                    crop(v(0.0, -3.5), v(ft(140.0), 9.5)),
                ),
                (b.id("BH1 log"), v(590.0, 590.0), DVec2::ZERO, None),
            ],
        ),
        (
            "A-100",
            "Foundation Plan",
            vec![(views.foundation, plan_at.0, plan_at.1, plan_crop)],
        ),
        (
            "A-101",
            "Ground Floor Plan",
            vec![(views.ground, plan_at.0, plan_at.1, plan_crop)],
        ),
        (
            "A-102",
            "First Floor Plan",
            vec![(views.first, plan_at.0, plan_at.1, plan_crop)],
        ),
        (
            "A-103",
            "Roof Plan",
            vec![(views.roof, plan_at.0, plan_at.1, plan_crop)],
        ),
        (
            "A-104",
            "Reflected Ceiling Plans",
            vec![
                (
                    views.ceilings[0],
                    v(215.0, 320.0),
                    p(24.0, 10.0),
                    crop(p(-10.0, -16.0), p(58.0, 34.0)),
                ),
                (
                    views.ceilings[1],
                    v(620.0, 320.0),
                    p(24.0, 10.0),
                    crop(p(-10.0, -16.0), p(58.0, 34.0)),
                ),
            ],
        ),
        (
            "A-201",
            "South and North Elevations",
            vec![
                (
                    views.elevations[0],
                    v(415.0, 455.0),
                    v(ft(36.0), 3.6),
                    crop(v(0.0, -0.45), v(ft(72.0), 9.0)),
                ),
                (
                    views.elevations[2],
                    v(415.0, 175.0),
                    v(ft(36.0), 3.6),
                    crop(v(0.0, -0.45), v(ft(72.0), 9.0)),
                ),
            ],
        ),
        (
            "A-202",
            "East and West Elevations",
            vec![
                (
                    views.elevations[1],
                    v(215.0, 320.0),
                    v(ft(28.0), 3.6),
                    crop(v(0.0, -0.45), v(ft(52.0), 9.0)),
                ),
                (
                    views.elevations[3],
                    v(620.0, 320.0),
                    v(ft(24.0), 3.6),
                    crop(v(0.0, -0.45), v(ft(52.0), 9.0)),
                ),
            ],
        ),
        (
            "A-301",
            "Building Sections",
            vec![
                (
                    views.sections[0],
                    v(205.0, 320.0),
                    v(ft(22.0), 3.2),
                    crop(v(0.0, -2.2), v(ft(48.0), 9.2)),
                ),
                (
                    views.sections[1],
                    v(600.0, 320.0),
                    v(ft(32.0), 3.2),
                    crop(v(0.0, -2.2), v(ft(64.0), 9.2)),
                ),
            ],
        ),
        (
            "A-401",
            "Wall Section and Details",
            vec![
                (
                    b.id("Section C-C"),
                    v(105.0, 310.0),
                    v(w + 0.1, 3.0),
                    crop(v(w - 1.5, -1.9), v(w + 2.0, 7.9)),
                ),
                (b.id(boxes[3].0), v(420.0, 470.0), mid(3), None),
                (b.id(boxes[1].0), v(420.0, 215.0), mid(1), None),
                (b.id(boxes[2].0), v(675.0, 422.0), mid(2), None),
                (b.id(boxes[0].0), v(675.0, 160.0), mid(0), None),
            ],
        ),
        (
            "A-601",
            "Door, Window and Room Schedules",
            vec![
                (sched[0], v(30.0, 560.0), DVec2::ZERO, None),
                (sched[1], v(30.0, 330.0), DVec2::ZERO, None),
                (sched[2], v(440.0, 560.0), DVec2::ZERO, None),
                (sched[6], v(440.0, 200.0), DVec2::ZERO, None),
            ],
        ),
        (
            "A-602",
            "Wall, Floor and Fitting Schedules",
            vec![
                (sched[3], v(30.0, 560.0), DVec2::ZERO, None),
                (sched[4], v(30.0, 285.0), DVec2::ZERO, None),
                (sched[7], v(440.0, 560.0), DVec2::ZERO, None),
                (sched[8], v(440.0, 360.0), DVec2::ZERO, None),
            ],
        ),
        (
            "A-603",
            "Take-off, Structure and Wall Type Schedules",
            vec![
                (sched[9], v(30.0, 560.0), DVec2::ZERO, None),
                (sched[10], v(30.0, 370.0), DVec2::ZERO, None),
                (sched[5], v(300.0, 560.0), DVec2::ZERO, None),
                (
                    b.id("Wall Type Schedule"),
                    v(300.0, 330.0),
                    DVec2::ZERO,
                    None,
                ),
            ],
        ),
        (
            "A-901",
            "3D Views",
            vec![
                (views.street, v(220.0, 320.0), DVec2::ZERO, None),
                (views.garden, v(620.0, 320.0), DVec2::ZERO, None),
            ],
        ),
    ];
    let mut tx = Transaction::new("Ridgeline House: sheets");
    let mut index = String::new();
    for (number, title, placed) in set {
        index.push_str(&format!("{number}   {title}\n"));
        let mut sheet = Sheet::a(number, "A1");
        sheet.width = SHEET.0;
        sheet.height = SHEET.1;
        sheet.template = Some(template);
        let e = Element::new(title, ElementKind::Sheet(sheet));
        let id = e.id;
        tx = b.keep(number, e, tx);
        for (view, center, target, crop) in placed {
            tx = tx.insert(Element::new(
                "Viewport",
                ElementKind::Viewport(Viewport {
                    sheet: id,
                    view,
                    center,
                    target,
                    crop,
                    fit: None,
                    pinned: false,
                }),
            ));
        }
    }
    b.commit(tx);
    // the hero views' sheet is added once they are traced (showcase_views_sheet)
    index.push_str(
        "A-902   Hero Views
",
    );

    // north arrows and scale bars on the plans
    let mut tx = Transaction::new("Ridgeline House: sheet symbols");
    for number in ["C-101", "A-100", "A-101", "A-102", "A-103"] {
        let mark = PaperMark {
            sheet: b.id(number),
            at: v(770.0, 70.0),
            size: 22.0,
            kind: PaperMarkKind::NorthArrow {
                rotation: None,
                style: None,
            },
            color: None,
            copy_of: None,
            measures: None,
        };
        tx = tx.insert(Element::new("North arrow", ElementKind::PaperMark(mark)));
    }
    b.commit(tx);

    // the cover: the project's name, the drawing list, the code summary
    let cover = b.id("G-001");
    let title = Some(b.id("Cover title"));
    let heading = Some(b.id("Cover heading"));
    let body = Some(b.id("Cover text"));
    let notes: Vec<(DVec2, String, Option<ElementId>)> = vec![
        (v(30.0, 575.0), PROJECT.to_uppercase(), title),
        (
            v(30.0, 552.0),
            format!("{ADDRESS}\nA new house for {CLIENT}  ·  Permit set, 21 November 2026"),
            heading,
        ),
        (v(30.0, 518.0), "DRAWING INDEX".into(), heading),
        (v(30.0, 506.0), index, None),
        (v(30.0, 392.0), "PROJECT DATA".into(), heading),
        (
            v(30.0, 380.0),
            "Occupancy: single-family dwelling (IRC R-3)\n\
             Construction: Type V-B, wood frame, two storeys\n\
             Conditioned floor area: 2,016 sq ft (1,152 ground, 864 first)\n\
             Building height: 28 ft to the ridge (35 ft allowed)\n\
             Lot: 130 ft × 120 ft, 15,600 sq ft; coverage 9.6% (30% allowed)\n\
             Setbacks: front 25 ft, sides 10 ft, rear 25 ft"
                .into(),
            None,
        ),
        (v(30.0, 322.0), "CLIMATE AND STRUCTURE".into(), heading),
        (
            v(30.0, 310.0),
            "Climate zone 6A (Duluth, Minnesota); 9,400 heating degree days\n\
             Frost depth 48 in: footings bear 50 in below finished grade\n\
             Ground snow 60 psf; roof design snow load 40 psf\n\
             Wind 115 mph, exposure B; seismic design category A\n\
             Soil bearing 3,000 psf on stiff glacial till (report GI-2026-118)"
                .into(),
            None,
        ),
        (v(30.0, 258.0), "ENVELOPE".into(), heading),
        (
            v(30.0, 246.0),
            "Walls R-31: cedar rainscreen, 2 in continuous mineral wool, 2×6 at R-23\n\
             Roof R-60: 14 in I-joist rafters dense-packed, 1 1/2 in polyiso inside\n\
             Slab R-20 under, R-10 at its edge and down the stem walls\n\
             Windows and doors: triple glazed, U-0.17, thermally broken frames\n\
             Air barrier continuous at the sheathing; target 1.0 ACH50\n\
             South glass shaded by 2'-6\" eaves and the terrace canopy\n\
             All plumbing in one heated chase: none in outside walls or floors"
                .into(),
            None,
        ),
        (v(30.0, 182.0), "GENERAL NOTES".into(), heading),
        (
            v(30.0, 170.0),
            "1. All work to the 2020 Minnesota Residential Code.\n\
             2. Dimensions are to grid lines and faces of framing; do not scale.\n\
             3. Verify all dimensions and conditions on site before work.\n\
             4. Every drawing in this set is drawn from one CreaBIM model:\n    \
             change the model and the sheets, schedules and take-off follow."
                .into(),
            None,
        ),
    ];
    let mut tx = Transaction::new("Ridgeline House: cover notes");
    for (at, text, style) in notes {
        let mut note = Note::new(cover, at, text);
        note.style = style.or(body);
        tx = tx.insert(Element::new("Note", ElementKind::Note(note)));
    }
    // the details sheet says, quietly, that its details are not finished:
    // they show what the model draws and the pieces that sit on it
    let remark = TextStyle {
        size: 2.6,
        italic: true,
        color: Some([0.45, 0.45, 0.45]),
        purpose: StyleUse::Notes,
        ..Default::default()
    };
    let remark = Element::new("Sheet remark", ElementKind::TextStyle(remark));
    let mut note = Note::new(
        b.id("A-401"),
        v(30.0, 24.0),
        "Details: work in progress. They show what the model draws, with the pieces drafted onto it so far.",
    );
    note.style = Some(remark.id);
    tx = tx.insert(remark);
    tx = tx.insert(Element::new("Note", ElementKind::Note(note)));
    b.commit(tx);
}

/// A strip 78 mm wide down the right-hand edge, as North American sets
/// carry it, with the project's own fields.
fn title_block() -> SheetTemplate {
    let (x0, x1, y0, y1) = (-88.0, -10.0, 10.0, SHEET.1 - 10.0);
    let line = |pts: &[(f64, f64)], width: f64| TitleItem::Line {
        points: pts.iter().map(|&(x, y)| v(x, y)).collect(),
        width,
    };
    let text = |x: f64, y: f64, size: f64, t: &str| TitleItem::Text {
        at: v(x, y),
        size,
        text: t.into(),
        align: TextAlign::Start,
        angle: 0.0,
        colour: None,
        font: None,
    };
    let mut items = vec![
        line(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)], 0.6),
        TitleItem::Fill {
            points: vec![v(x0, y0), v(x1, y0), v(x1, y0 + 52.0), v(x0, y0 + 52.0)],
            gray: 0.92,
        },
    ];
    for y in [52.0, 80.0, 104.0, 128.0, 152.0, 200.0, 300.0, 480.0] {
        items.push(line(&[(x0, y0 + y), (x1, y0 + y)], 0.25));
    }
    items.push(line(
        &[(x0 + 39.0, y0 + 80.0), (x0 + 39.0, y0 + 152.0)],
        0.18,
    ));
    let label = 1.8;
    let l = x0 + 3.0;
    items.extend([
        text(l, y1 - 12.0, 5.5, "RIDGELINE"),
        text(l, y1 - 20.0, 5.5, "HOUSE"),
        text(l, y1 - 30.0, 2.2, "418 Ridgeline Road"),
        text(l, y1 - 34.5, 2.2, "Duluth, Minnesota"),
        text(l, y1 - 50.0, label, "CLIENT"),
        text(l, y1 - 55.0, 2.6, "{client}"),
        text(l, y1 - 68.0, label, "ARCHITECT"),
        text(l, y1 - 73.0, 2.6, "CreaBIM Studio"),
        text(l, y1 - 77.5, 2.0, "Modelled and drawn in CreaBIM 1.2"),
        text(l, y0 + 474.0, label, "NOTES"),
        text(l, y0 + 468.0, 2.0, "Do not scale drawings."),
        text(l, y0 + 463.5, 2.0, "Verify dimensions on site."),
        text(l, y0 + 459.0, 2.0, "Climate zone 6A."),
        text(l, y0 + 454.5, 2.0, "Frost depth 48 in."),
        text(l, y0 + 450.0, 2.0, "Roof snow load 40 psf."),
        text(l, y0 + 294.0, label, "ISSUES"),
        text(l, y0 + 288.0, 2.0, "A   21 Nov 2026   Permit set"),
        text(l, y0 + 194.0, label, "STATUS"),
        text(l, y0 + 186.0, 3.2, "{status}"),
        text(l, y0 + 146.0, label, "PROJECT NO."),
        text(l, y0 + 136.0, 3.0, "2026-014"),
        text(x0 + 42.0, y0 + 146.0, label, "DATE"),
        text(x0 + 42.0, y0 + 136.0, 2.6, "{date}"),
        text(l, y0 + 122.0, label, "SCALE"),
        text(l, y0 + 112.0, 2.6, "{scale}"),
        text(x0 + 42.0, y0 + 122.0, label, "REVISION"),
        text(x0 + 42.0, y0 + 112.0, 3.0, "{revision}"),
        text(l, y0 + 98.0, label, "DRAWN"),
        text(l, y0 + 88.0, 2.6, "{drawn}"),
        text(x0 + 42.0, y0 + 98.0, label, "PAPER"),
        text(x0 + 42.0, y0 + 88.0, 2.6, "ARCH D"),
        text(l, y0 + 74.0, label, "SHEET TITLE"),
        text(l, y0 + 62.0, 3.6, "{sheet}"),
        text(l, y0 + 44.0, label, "SHEET"),
        text(l, y0 + 16.0, 13.0, "{number}"),
    ]);
    SheetTemplate {
        corner: Corner::BottomRight,
        items,
        border: true,
        fields: vec![
            ("job".into(), PROJECT.into()),
            ("address".into(), ADDRESS.into()),
            ("client".into(), CLIENT.into()),
            ("status".into(), "PERMIT SET".into()),
            ("revision".into(), "A".into()),
            ("drawn".into(), "CreaBIM".into()),
        ],
        paper: None,
        guides: vec![],
        margins: None,
        stock: Default::default(),
    }
}
