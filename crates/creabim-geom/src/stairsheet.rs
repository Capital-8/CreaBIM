//! The Stair sheet: one sheet that documents a stair. The plans of the
//! floor it starts on and the floor it reaches, both at 1:50 and cropped
//! to the stair; its walk-line section; a 3D view cut out round it; and the
//! stair schedule. Every view on it is an ordinary view, so it stays live
//! and can be moved, cropped or deleted like any other.

use crate::section::view_drawing;
use crate::sheet::{title_block_bounds, BINDING, MARGIN};
use creabim_core::cropbox::{CropBox, Outside};
use creabim_core::{
    Element, ElementId, ElementKind, Model, ScheduleCategory, Sheet, Transaction, View, ViewKind,
    Viewport,
};
use glam::DVec2;

/// Room round the stair in its plans, metres.
const AROUND: f64 = 0.8;
/// Space between views, and under each for its title, paper mm.
const GAP: f64 = 12.0;
const TITLE: f64 = 12.0;

/// A view to place: its id, the part of its drawing to show (none: all of
/// it) and that part's size on paper.
struct Slot {
    view: ElementId,
    fit: bool,
    crop: Option<[DVec2; 2]>,
    target: DVec2,
    size: DVec2,
}

/// The part of a plan a viewport following stair `id` shows: the stair's
/// footprint with room round it; none if it isn't a stair any more.
pub fn fitted(model: &Model, id: ElementId) -> Option<[DVec2; 2]> {
    let Some(ElementKind::Stair(s)) = model.get(id).map(|e| &e.kind) else {
        return None;
    };
    let (lo, hi) = s.footprint(model)?;
    Some([lo - DVec2::splat(AROUND), hi + DVec2::splat(AROUND)])
}

/// The level stair `s` arrives at, if one is there.
fn arrival(model: &Model, s: &creabim_core::Stair) -> Option<ElementId> {
    let top = s.base(model) + s.rise(model);
    model
        .levels()
        .filter(|(_, l)| (l.elevation - top).abs() < 0.05)
        .map(|(e, _)| e.id)
        .next()
}

/// A new sheet documenting stair `id`, with the views it needs: the
/// transaction that makes it all, and the sheet's id. The walk-line
/// section and the stair schedule are used again if the project has them.
pub fn stair_sheet(model: &Model, id: ElementId) -> Result<(Transaction, ElementId), String> {
    let Some(e) = model.get(id) else {
        return Err("that stair is gone".into());
    };
    let ElementKind::Stair(s) = &e.kind else {
        return Err("that isn't a stair".into());
    };
    let name = e.name.clone();
    let level_name = |l: ElementId| model.get(l).map(|e| e.name.clone()).unwrap_or_default();
    let mut scratch = model.clone();
    let mut tx = Transaction::new(format!("Stair sheet for {name}"));
    let add = |scratch: &mut Model, tx: &mut Transaction, e: Element| -> Result<(), String> {
        Transaction::new("")
            .insert(e.clone())
            .apply(scratch)
            .map_err(|err| err.to_string())?;
        *tx = std::mem::replace(tx, Transaction::new("")).insert(e);
        Ok(())
    };
    let mut views: Vec<(ElementId, bool)> = vec![];
    // the plans, at 1:50, cropped to the stair
    let mut levels = vec![s.level];
    levels.extend(arrival(model, s));
    for l in levels {
        let mut v = View::plan(l);
        v.scale = 50.0;
        let e = Element::new(
            format!("{name}, {} plan", level_name(l)),
            ElementKind::View(v),
        );
        views.push((e.id, true));
        add(&mut scratch, &mut tx, e)?;
    }
    // the walk-line section
    let walk = model.iter().find(|e| {
        matches!(&e.kind, ElementKind::View(v) if v.kind == ViewKind::WalkSection { stair: id })
    });
    match walk {
        Some(w) => views.push((w.id, false)),
        None => {
            let e = Element::new(
                format!("{name} walk-line section"),
                ElementKind::View(View::walk_section(id)),
            );
            views.push((e.id, false));
            add(&mut scratch, &mut tx, e)?;
        }
    }
    // a 3D view, looking up the first flight from its side, cut out round
    // the stair so the walls in front of it don't hide it
    if let Some((lo, hi)) = s.cut_out(model) {
        let mut axo = View::axonometric(s.direction - 50.0, 30.0);
        axo.scale = 50.0;
        let e = Element::new(format!("{name} 3D"), ElementKind::View(axo));
        let v = e.id;
        views.push((v, false));
        add(&mut scratch, &mut tx, e)?;
        let mut crop = CropBox::new(Some(v), lo, hi);
        crop.outside = Outside::Hidden;
        crop.frame_hidden = true;
        crop.fit = Some(id);
        let c = Element::new(format!("{name} 3D crop"), ElementKind::CropBox(crop));
        add(&mut scratch, &mut tx, c)?;
    }
    // the stair schedule
    let schedule = model.iter().find(|e| {
        matches!(&e.kind, ElementKind::View(v)
            if matches!(v.kind, ViewKind::Schedule { category: ScheduleCategory::Stairs, .. }))
    });
    match schedule {
        Some(v) => views.push((v.id, false)),
        None => {
            let e = Element::new(
                ScheduleCategory::Stairs.schedule_name(),
                ElementKind::View(View::schedule(ScheduleCategory::Stairs)),
            );
            views.push((e.id, false));
            add(&mut scratch, &mut tx, e)?;
        }
    }
    // how big each is on paper
    let mut slots = vec![];
    let stair_box = fitted(model, id);
    for (v, crop_to_stair) in views {
        let (Some(d), Some(view)) = (
            view_drawing(&scratch, v),
            scratch.get(v).and_then(|e| match &e.kind {
                ElementKind::View(v) => Some(v.clone()),
                _ => None,
            }),
        ) else {
            continue;
        };
        let k = 1000.0 / view.paper_scale();
        // the plans show the stair's footprint, and keep showing it
        let crop = stair_box.filter(|_| crop_to_stair);
        let part = match (crop, d.bounds()) {
            (Some(c), _) => (c[0], c[1]),
            (None, Some(b)) => b,
            (None, None) => continue,
        };
        slots.push(Slot {
            view: v,
            fit: crop.is_some(),
            crop,
            target: (part.0 + part.1) / 2.0,
            size: (part.1 - part.0) * k,
        });
    }
    // the smallest sheet they fit on, shelf by shelf from the top left
    let used: Vec<String> = crate::sheet::sheets(model)
        .into_iter()
        .map(|(_, s)| s.number.clone())
        .collect();
    let number = (501..)
        .map(|n| format!("A{n}"))
        .find(|n| !used.contains(n))
        .expect("unbounded");
    let mut placed = None;
    for size in ["A2", "A1", "A0"] {
        let sheet = Sheet::a(number.clone(), size);
        if let Some(at) = pack(&scratch, &sheet, &slots) {
            placed = Some((sheet, at));
            break;
        }
    }
    let (sheet, at) = placed.unwrap_or_else(|| {
        // too big for any: stack them down the middle of an A0
        let sheet = Sheet::a(number.clone(), "A0");
        let mid = DVec2::new(sheet.width, sheet.height) / 2.0;
        (sheet, vec![mid; slots.len()])
    });
    let sheet_e = Element::new(format!("{name} details"), ElementKind::Sheet(sheet));
    let sheet_id = sheet_e.id;
    tx = tx.insert(sheet_e);
    for (slot, c) in slots.iter().zip(at) {
        tx = tx.insert(Element::new(
            "Viewport",
            ElementKind::Viewport(Viewport {
                sheet: sheet_id,
                view: slot.view,
                center: c,
                target: slot.target,
                crop: slot.crop,
                fit: slot.fit.then_some(id),
                pinned: false,
            }),
        ));
    }
    Ok((tx, sheet_id))
}

/// Where each slot's middle goes on `sheet`, laid in rows from the top
/// left, clear of the title block; none if they don't all fit.
fn pack(model: &Model, sheet: &Sheet, slots: &[Slot]) -> Option<Vec<DVec2>> {
    let block = title_block_bounds(model, sheet);
    let (left, right) = (BINDING + 5.0, sheet.width - MARGIN - 5.0);
    let bottom = MARGIN + 5.0;
    let mut out = vec![];
    let (mut x, mut top, mut row) = (left, sheet.height - MARGIN - 5.0, 0.0_f64);
    let clear = |a: DVec2, b: DVec2| b.x < block.0.x || a.x > block.1.x || a.y > block.1.y;
    for s in slots {
        let h = s.size.y + TITLE;
        let mut tries = 0;
        loop {
            tries += 1;
            if tries > 50 {
                return None;
            }
            if x + s.size.x > right {
                // the next row
                x = left;
                top -= row + GAP;
                row = 0.0;
            }
            if s.size.x > right - left || top - h < bottom {
                return None;
            }
            let (a, b) = (DVec2::new(x, top - h), DVec2::new(x + s.size.x, top));
            if clear(a, b) {
                out.push(DVec2::new(x + s.size.x / 2.0, top - s.size.y / 2.0));
                x += s.size.x + GAP;
                row = row.max(h);
                break;
            }
            // past the title block's left edge: the next row
            x = right + 1.0;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::PlanItem;

    #[test]
    fn a_stair_sheet_places_both_plans_the_section_a_3d_view_and_the_schedule() {
        let level = |name: &str, z: f64| {
            Element::new(
                name,
                ElementKind::Level(creabim_core::Level {
                    elevation: z,
                    height: 3.0,
                }),
            )
        };
        let (ground, first) = (level("Ground", 0.0), level("First", 3.0));
        let st = creabim_core::Stair::from_line(
            ground.id,
            &[DVec2::new(0.0, 0.0), DVec2::new(6.0, 0.0)],
            creabim_core::members::StairLine::Walk,
        )
        .unwrap();
        let stair = Element::new("Main stair", ElementKind::Stair(st));
        let sid = stair.id;
        let mut doc = creabim_core::Document::new();
        doc.commit(
            Transaction::new("setup")
                .insert(ground)
                .insert(first)
                .insert(stair),
        )
        .unwrap();
        let mut model = doc.model().clone();
        let st_max = match &model.get(sid).unwrap().kind {
            ElementKind::Stair(s) => s.max_riser,
            _ => unreachable!(),
        };
        let (tx, sheet) = stair_sheet(&model, sid).unwrap();
        tx.apply(&mut model).unwrap();
        let ports: Vec<&Viewport> = model
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Viewport(v) if v.sheet == sheet => Some(v),
                _ => None,
            })
            .collect();
        let kinds: Vec<&str> = ports
            .iter()
            .filter_map(|p| match model.get(p.view).map(|e| &e.kind) {
                Some(ElementKind::View(v)) => Some(match v.kind {
                    ViewKind::Plan { .. } => "plan",
                    ViewKind::WalkSection { .. } => "walk",
                    ViewKind::Axonometric { .. } => "3d",
                    ViewKind::Schedule { .. } => "schedule",
                    _ => "other",
                }),
                _ => None,
            })
            .collect();
        assert_eq!(kinds, ["plan", "plan", "walk", "3d", "schedule"]);
        // the plans are cropped round the stair, and nothing overlaps
        assert!(ports[0].crop.is_some() && ports[1].crop.is_some());
        let placed = crate::sheet::placed_views(&model, sheet, &mut |v| view_drawing(&model, v));
        for (i, a) in placed.iter().enumerate() {
            for b in &placed[i + 1..] {
                let apart = a.bounds.1.x < b.bounds.0.x
                    || b.bounds.1.x < a.bounds.0.x
                    || a.bounds.1.y < b.bounds.0.y
                    || b.bounds.1.y < a.bounds.0.y;
                assert!(apart, "{} overlaps {}", a.title, b.title);
            }
        }
        // a 3D view cut out round the stair
        assert!(model.iter().any(|e| matches!(&e.kind,
            ElementKind::CropBox(c) if c.view == Some(ports[3].view))));
        // the stair is numbered riser by riser in its 1:50 plan
        let plan = view_drawing(&model, ports[0].view).unwrap();
        let numbers: Vec<String> = plan
            .items
            .iter()
            .filter_map(|it| match it {
                PlanItem::Text { id, text, .. } if *id == sid => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(numbers.first().map(String::as_str), Some("1"));
        // and listed in the stair schedule
        let table = crate::schedule::schedule(&model, ScheduleCategory::Stairs);
        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0].cells[0], "Main stair");
        assert!(table.rows[0].cells[2].starts_with(&format!("{} × ", numbers.len())));
        assert_eq!(table.totals[0], "1 stair");
        assert_eq!(
            numbers.len(),
            creabim_core::members::riser_count(3.0, st_max) as usize
        );
    }

    #[test]
    fn a_stair_sheet_keeps_following_its_stair_until_cropped_by_hand() {
        let level = |name: &str, z: f64| {
            Element::new(
                name,
                ElementKind::Level(creabim_core::Level {
                    elevation: z,
                    height: 3.0,
                }),
            )
        };
        let (ground, first) = (level("Ground", 0.0), level("First", 3.0));
        let st = creabim_core::Stair::from_line(
            ground.id,
            &[DVec2::new(0.0, 0.0), DVec2::new(6.0, 0.0)],
            creabim_core::members::StairLine::Walk,
        )
        .unwrap();
        let stair = Element::new("Main stair", ElementKind::Stair(st));
        let sid = stair.id;
        let mut doc = creabim_core::Document::new();
        doc.commit(
            Transaction::new("setup")
                .insert(ground)
                .insert(first)
                .insert(stair),
        )
        .unwrap();
        let (tx, sheet) = stair_sheet(doc.model(), sid).unwrap();
        doc.commit(tx).unwrap();
        let plan = |doc: &creabim_core::Document| {
            doc.model()
                .iter()
                .find_map(|e| match &e.kind {
                    ElementKind::Viewport(v) if v.sheet == sheet && v.fit.is_some() => {
                        Some((e.id, v.clone()))
                    }
                    _ => None,
                })
                .unwrap()
        };
        let cut = |doc: &creabim_core::Document| {
            doc.model()
                .iter()
                .find_map(|e| match &e.kind {
                    ElementKind::CropBox(c) => Some((e.id, c.clone())),
                    _ => None,
                })
                .unwrap()
        };
        let (_, vp) = plan(&doc);
        let (crop0, target0) = crate::sheet::viewport_frame(doc.model(), &vp);
        let box0 = cut(&doc).1;
        // the stair moves 5 m east: its plans and its 3D cut-out go with it
        doc.edit(sid, "Move", |e| {
            if let ElementKind::Stair(s) = &mut e.kind {
                s.start.x += 5.0;
            }
        })
        .unwrap();
        let (vid, vp) = plan(&doc);
        let (crop1, target1) = crate::sheet::viewport_frame(doc.model(), &vp);
        assert!((target1 - target0 - DVec2::new(5.0, 0.0)).length() < 1e-6);
        assert!((crop1.unwrap()[0] - crop0.unwrap()[0] - DVec2::new(5.0, 0.0)).length() < 1e-6);
        let (bid, box1) = cut(&doc);
        assert!((box1.min.x - box0.min.x - 5.0).abs() < 1e-6);
        assert_eq!(box1.min.y, box0.min.y);
        // a crop box resized by hand stays as it was put
        doc.edit(bid, "Resize", |e| {
            if let ElementKind::CropBox(c) = &mut e.kind {
                c.set_face(0, c.min.x - 1.0);
            }
        })
        .unwrap();
        assert!(cut(&doc).1.fit.is_none());
        // and a viewport let go of keeps its own crop
        doc.edit(vid, "Crop", |e| {
            if let ElementKind::Viewport(v) = &mut e.kind {
                v.fit = None;
            }
        })
        .unwrap();
        doc.edit(sid, "Move back", |e| {
            if let ElementKind::Stair(s) = &mut e.kind {
                s.start.x -= 5.0;
            }
        })
        .unwrap();
        let vp = match &doc.model().get(vid).unwrap().kind {
            ElementKind::Viewport(v) => v.clone(),
            _ => unreachable!(),
        };
        assert_eq!(crate::sheet::viewport_frame(doc.model(), &vp).0, vp.crop);
    }
}
