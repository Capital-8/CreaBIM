//! Part 9: windows in a ribbon or round a corner keep their arrangement
//! through IFC, named for other programs and whole for CreaBIM.

use creabim_core::{arrangement, sample::sample_house, CornerJoint, ElementKind, OpeningKind};
use creabim_ifc::{export_ifc4, import_ifc, ExportOptions};

#[test]
fn ribbons_and_corners_round_trip_through_ifc() {
    let mut m = sample_house();
    let windows: Vec<_> = m
        .iter()
        .filter(|e| matches!(&e.kind, ElementKind::Opening(o) if o.kind == OpeningKind::Window && o.roof_at.is_none()))
        .map(|e| e.id)
        .collect();
    // one turned round a corner, glass to glass; another in a ribbon
    let (corner, tx) = windows
        .iter()
        .find_map(|id| {
            [true, false].into_iter().find_map(|end| {
                arrangement::add_round_corner(&m, *id, end, CornerJoint::Glass)
                    .ok()
                    .map(|tx| (*id, tx))
            })
        })
        .expect("a window by a corner");
    tx.apply(&mut m).unwrap();
    let (ribbon, tx) = windows
        .iter()
        .filter(|id| **id != corner)
        .find_map(|id| arrangement::add_beside(&m, *id).ok().map(|tx| (*id, tx)))
        .expect("a window with room beside it");
    tx.apply(&mut m).unwrap();
    let ifc = export_ifc4(&m, &ExportOptions::default());
    assert!(
        ifc.contains("'Corner 1, glass'"),
        "the corner's name for other programs"
    );
    assert!(ifc.contains("'Ribbon 2'"));
    let back = import_ifc(&ifc).expect("our own export imports").model;
    let arranged = |m: &creabim_core::Model| {
        let mut v: Vec<_> = m
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Opening(o) => o.arrangement.map(|a| (e.id, a)),
                _ => None,
            })
            .collect();
        v.sort_by_key(|(id, _)| id.to_string());
        v
    };
    let (was, now) = (arranged(&m), arranged(&back));
    assert_eq!(was.len(), 4, "two round the corner, two in the ribbon");
    assert_eq!(was, now);
    assert!(was.iter().any(|(id, _)| *id == ribbon));
}
