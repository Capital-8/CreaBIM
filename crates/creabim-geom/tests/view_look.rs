//! A change to a view's own look (its graphic style) redraws that view
//! and nothing else: not the other views, not the 3D scene. Before, every
//! edit counted as a change to the whole model, so a click on a drawing
//! style rebuilt every open drawing and the scene (40 to 160 ms each).

use creabim_core::{
    graphics::DrawingStyle, Document, ElementId, ElementKind, View, ViewKind, ViewStyle,
};

/// One change to a view.
type Change = fn(&mut View);

fn view(doc: &Document, kind: fn(&ViewKind) -> bool) -> ElementId {
    doc.model()
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::View(v) if kind(&v.kind) && v.callout.is_none() => Some(e.id),
            _ => None,
        })
        .expect("the starter house has one")
}

fn edit(doc: &mut Document, id: ElementId, f: impl FnOnce(&mut View)) {
    doc.edit(id, "View", |e| {
        if let ElementKind::View(v) = &mut e.kind {
            f(v);
        }
    })
    .unwrap();
}

#[test]
fn a_views_style_redraws_that_view_alone() {
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    let plan = view(&doc, |k| matches!(k, ViewKind::Plan { .. }));
    let section = view(&doc, |k| matches!(k, ViewKind::Section { .. }));
    let looks: [Change; 5] = [
        |v| v.graphics.drawing_style = DrawingStyle::Poche,
        |v| v.style = ViewStyle::Shaded,
        |v| v.show_hidden = !v.show_hidden,
        |v| v.hidden_in_colour = !v.hidden_in_colour,
        |v| v.room_colours = !v.room_colours,
    ];
    for look in looks {
        let m = doc.model();
        let (all, own, other, rev) = (
            m.drawing_revision(),
            m.drawing_revision_of(plan),
            m.drawing_revision_of(section),
            m.revision(),
        );
        let scene = creabim_geom::shared_scene(m);
        edit(&mut doc, plan, look);
        let m = doc.model();
        assert_ne!(m.revision(), rev, "the model did change");
        assert_ne!(
            m.drawing_revision_of(plan),
            own,
            "its own drawing is out of date"
        );
        assert_eq!(m.drawing_revision_of(section), other, "no other view's is");
        assert_eq!(m.drawing_revision(), all);
        assert!(
            std::sync::Arc::ptr_eq(&creabim_geom::shared_scene(m), &scene),
            "the 3D scene is not built again"
        );
    }
    // taking a look back is a change of look too
    let own = doc.model().drawing_revision_of(plan);
    let other = doc.model().drawing_revision_of(section);
    assert!(doc.undo());
    assert_ne!(doc.model().drawing_revision_of(plan), own);
    assert_eq!(doc.model().drawing_revision_of(section), other);
}

#[test]
fn what_other_views_read_of_a_view_still_redraws_them() {
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    let plan = view(&doc, |k| matches!(k, ViewKind::Plan { .. }));
    let section = view(&doc, |k| matches!(k, ViewKind::Section { .. }));
    // a section's line is drawn in plans; a plan's range, in sections; a
    // name is on marks and sheets; a scale sizes text
    let changes: [(ElementId, Change); 4] = [
        (section, |v| {
            if let ViewKind::Section { start, .. } = &mut v.kind {
                start.x += 0.5;
            }
        }),
        (plan, |v| {
            v.graphics.range.from_above = !v.graphics.range.from_above
        }),
        (plan, |v| v.scale *= 2.0),
        (plan, |v| {
            v.style = ViewStyle::Shaded;
            v.scale *= 2.0;
        }),
    ];
    for (id, change) in changes {
        let all = doc.model().drawing_revision();
        edit(&mut doc, id, change);
        assert_ne!(doc.model().drawing_revision(), all);
    }
    // and so does a rename, or any change to what is built
    let all = doc.model().drawing_revision();
    doc.edit(section, "Rename", |e| e.name.push('x')).unwrap();
    assert_ne!(doc.model().drawing_revision(), all);
    let scene = creabim_geom::shared_scene(doc.model());
    let wall = doc
        .model()
        .iter()
        .find(|e| matches!(e.kind, ElementKind::Wall(_)))
        .unwrap()
        .id;
    doc.edit(wall, "Wall", |e| {
        if let ElementKind::Wall(w) = &mut e.kind {
            w.end.x += 0.25;
        }
    })
    .unwrap();
    assert!(!std::sync::Arc::ptr_eq(
        &creabim_geom::shared_scene(doc.model()),
        &scene
    ));
}
