//! A note on a sheet showing a section of the project's notes (Properties
//! REV04, decision 9): it draws the section, its fields filled, follows
//! every edit, and typing notes builds nothing in 3D.

use creabim_core::project::{set_project, NoteSection, PadItem, TODOS};
use creabim_core::{Document, Element, ElementKind, Note, Sheet, Transaction};
use creabim_geom::sheet::{sheet_svg, SheetInfo};
use glam::DVec2;

#[test]
fn a_linked_note_shows_its_section_and_follows_it() {
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    let mut info = doc
        .model()
        .project_info()
        .map(|(_, p)| p.clone())
        .unwrap_or_default();
    info.name = "Maple Lane House".into();
    let mut general = NoteSection::new("general", "General notes");
    general
        .text
        .push_str("1. Do not scale from this drawing.\n2. Read with {sheets} for {project}.\n");
    info.sections = vec![general];
    info.pad = vec![PadItem {
        text: "Check the drain fall".into(),
        todo: true,
        ..Default::default()
    }];
    doc.commit(set_project(doc.model(), info)).unwrap();

    let sheet = Element::new("Plan", ElementKind::Sheet(Sheet::a("A101", "A3")));
    let sid = sheet.id;
    let general = Note::linked(sid, DVec2::new(290.0, 270.0), "general", 110.0);
    let todos = Note::linked(sid, DVec2::new(290.0, 150.0), TODOS, 110.0);
    doc.commit(
        Transaction::new("Place notes")
            .insert(sheet)
            .insert(Element::new("General notes", ElementKind::Note(general)))
            .insert(Element::new("To-dos", ElementKind::Note(todos))),
    )
    .unwrap();
    let svg = sheet_svg(doc.model(), sid, &SheetInfo::default()).unwrap();
    assert!(svg.contains("GENERAL NOTES"), "its heading");
    assert!(svg.contains("Do not scale from this drawing."), "its text");
    assert!(
        svg.contains("Maple Lane House") && svg.contains("A101"),
        "its fields filled"
    );
    assert!(svg.contains("Check the drain fall"), "the to-dos");

    // an edit in the Notes area: the sheet follows, 3D is left as it is
    let built = doc.model().scene_revision();
    let drawn = doc.model().drawing_revision();
    let mut info = doc.model().project_info().unwrap().1.clone();
    info.sections[0].text.push_str("3. Concrete C25/30.\n");
    doc.commit(set_project(doc.model(), info)).unwrap();
    assert_eq!(
        doc.model().scene_revision(),
        built,
        "notes build nothing in 3D"
    );
    assert_ne!(
        doc.model().drawing_revision(),
        drawn,
        "but sheets show them"
    );
    let svg = sheet_svg(doc.model(), sid, &SheetInfo::default()).unwrap();
    assert!(
        svg.contains("Concrete C25/30."),
        "the sheet follows the section"
    );

    // a section that is gone: the note falls back to its own text
    let mut info = doc.model().project_info().unwrap().1.clone();
    info.sections = vec![NoteSection::new("own-1", "Mine")];
    doc.commit(set_project(doc.model(), info)).unwrap();
    let svg = sheet_svg(doc.model(), sid, &SheetInfo::default()).unwrap();
    assert!(!svg.contains("Do not scale"));
}
