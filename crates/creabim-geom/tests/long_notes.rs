//! A long note shared out across blocks (Mockup_Long_Notes_REV00): the
//! first block stops at its height when Continued, the next carries on
//! under its title, Whole folds it back, and what no block holds counts as
//! overflow.

use creabim_core::project::{set_project, NoteSection};
use creabim_core::{
    Document, Element, ElementKind, Note, NoteFollow, PartTitle, Sheet, Transaction,
};
use creabim_geom::annotate::{chain_overflow, chain_share, note_runs_past};
use glam::DVec2;

#[test]
fn a_long_note_continues_and_folds_back() {
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    let mut info = doc
        .model()
        .project_info()
        .map(|(_, p)| p.clone())
        .unwrap_or_default();
    let mut general = NoteSection::new("general", "General notes");
    for i in 1..=10 {
        general.text.push_str(&format!("{i}. Line {i}.\n"));
    }
    info.sections = vec![general];
    doc.commit(set_project(doc.model(), info)).unwrap();

    let sheet = Element::new("Plan", ElementKind::Sheet(Sheet::a("A101", "A3")));
    let sid = sheet.id;
    // 2.5 mm text steps 3.75 mm a line: 20 mm holds 5 lines
    let mut first = Note::linked(sid, DVec2::new(300.0, 270.0), "general", 110.0);
    first.height = Some(20.0);
    let first_e = Element::new("General notes", ElementKind::Note(first));
    let fid = first_e.id;
    let mut part = Note::linked(sid, DVec2::new(180.0, 270.0), "general", 110.0);
    part.height = Some(200.0);
    part.follows = Some(NoteFollow {
        first: fid,
        part: 1,
        title: PartTitle::Continued,
    });
    let part_e = Element::new("General notes (part 2)", ElementKind::Note(part));
    let pid = part_e.id;
    doc.commit(
        Transaction::new("Place")
            .insert(sheet)
            .insert(first_e)
            .insert(part_e),
    )
    .unwrap();

    // Whole: the first shows all, the part nothing, nothing left over
    assert!(
        chain_share(doc.model(), fid).is_none(),
        "a Whole first block lays out as before"
    );
    assert_eq!(chain_share(doc.model(), pid).unwrap().0.len(), 0);
    assert!(
        note_runs_past(doc.model(), fid) > 0,
        "it runs past its 20 mm"
    );

    // Continued: five lines here, the rest under "(continued)" there
    let mut e = doc.model().get(fid).unwrap().clone();
    if let ElementKind::Note(n) = &mut e.kind {
        n.continued = true;
    }
    let before = doc.model().get(fid).unwrap().clone();
    doc.commit(Transaction::new("Continued").replace(before, e))
        .unwrap();
    let (mine, _) = chain_share(doc.model(), fid).unwrap();
    assert_eq!(mine.len(), 5, "the first block holds what fits its height");
    let (rest, left) = chain_share(doc.model(), pid).unwrap();
    assert_eq!(rest[0], "(continued)");
    assert!(
        rest.iter().any(|l| l.contains("Line 10.")),
        "the part carries on to the end"
    );
    assert!(!rest.iter().any(|l| l.contains("Line 2.")), "no line twice");
    assert_eq!(left, 0);
    assert_eq!(chain_overflow(doc.model(), pid), 0);
}

#[test]
fn bold_italic_and_date_reach_the_sheet() {
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    let mut info = doc
        .model()
        .project_info()
        .map(|(_, p)| p.clone())
        .unwrap_or_default();
    let mut general = NoteSection::new("general", "General notes");
    general
        .text
        .push_str("1. Use **C25/30** concrete, *unless noted*.\n2. Issued {date}.\n");
    info.sections = vec![general];
    doc.commit(set_project(doc.model(), info)).unwrap();
    let sheet = Element::new("Plan", ElementKind::Sheet(Sheet::a("A101", "A3")));
    let sid = sheet.id;
    let n = Note::linked(sid, DVec2::new(290.0, 270.0), "general", 110.0);
    doc.commit(
        Transaction::new("Place")
            .insert(sheet)
            .insert(Element::new("General notes", ElementKind::Note(n))),
    )
    .unwrap();
    let svg = creabim_geom::sheet::sheet_svg(
        doc.model(),
        sid,
        &creabim_geom::sheet::SheetInfo::default(),
    )
    .unwrap();
    assert!(
        svg.contains("font-weight=\"bold\"") && svg.contains("C25/30"),
        "bold prints bold"
    );
    assert!(
        svg.contains("font-style=\"italic\""),
        "italic prints italic"
    );
    assert!(!svg.contains("**"), "no stars on paper");
    assert!(
        svg.contains(&creabim_core::project::today()),
        "{{date}} filled"
    );
}
