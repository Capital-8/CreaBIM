//! A phased project's schedules end with their totals split by status.

use creabim_core::phases;
use creabim_core::{Document, ElementKind, ScheduleCategory};
use creabim_geom::schedule::schedule;

#[test]
fn a_phased_schedule_splits_its_totals_by_status() {
    let plain = creabim_core::starter::starter_house();
    let before = schedule(&plain, ScheduleCategory::Walls);
    assert!(
        before.rows.iter().all(|r| !r.group),
        "no status rows without phases"
    );
    let mut doc = Document::from_model(plain);
    doc.commit(phases::start(doc.model(), true).unwrap())
        .unwrap();
    let m = doc.model();
    let wall = m
        .iter()
        .find(|e| matches!(e.kind, ElementKind::Wall(_)) && !m.dependents(e.id).is_empty())
        .unwrap()
        .id;
    let (tx, _) = phases::remove_from(m, &[wall], Some(3));
    doc.commit(tx).unwrap();
    let s = schedule(doc.model(), ScheduleCategory::Walls);
    let walls = s.rows.iter().filter(|r| !r.group).count();
    assert_eq!(walls, before.rows.len(), "every wall is still listed");
    assert_eq!(s.columns.last().unwrap().title, "Phase");
    let heads: Vec<&str> = s
        .rows
        .iter()
        .filter(|r| r.group)
        .map(|r| r.cells[0].as_str())
        .collect();
    assert_eq!(
        heads,
        [
            format!("Existing, kept ({})", walls - 1).as_str(),
            "Taken down (1)"
        ]
    );
    for r in s.rows.iter().filter(|r| r.group) {
        assert_eq!(r.cells.len(), s.columns.len());
        assert!(
            r.cells.iter().skip(1).any(|c| !c.is_empty()),
            "its sums are in their columns: {:?}",
            r.cells
        );
    }
    // doors: the ones in the wall came down with it
    let d = schedule(doc.model(), ScheduleCategory::Doors);
    assert!(d
        .rows
        .iter()
        .any(|r| r.group && r.cells[0].starts_with("Taken down")));
}
