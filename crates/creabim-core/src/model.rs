use crate::{Element, ElementId, ElementKind, Level, ModelError};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// The single source of truth every view reads from.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Model {
    elements: IndexMap<ElementId, Element>,
    /// Bumped on every change so views know when to rebuild. Revisions
    /// are unique across every model in the process (a clone shares its
    /// original's until either changes), so caches can key on the
    /// revision alone; 0 is a model never changed through these methods.
    #[serde(skip)]
    revision: u64,
    /// The revision of the last change a view's drawing could show: a
    /// note, a north arrow or a scale bar on a sheet, or a view moved on
    /// one, leaves it (MU23), so sheets don't redraw their views for them.
    #[serde(skip)]
    drawn: u64,
    /// For each view, the revision of the last change to its own look
    /// alone ([`look_only`]): its drawing is out of date, no other view's
    /// and not the 3D scene, so that change leaves `drawn` as it is.
    #[serde(skip)]
    looks: std::collections::HashMap<ElementId, u64>,
    /// The revision of the last change the 3D scene could show: an
    /// annotation or a text style (its font, its size) builds nothing in
    /// 3D, so leaves it (a font change rebuilt the scene and its snaps,
    /// two seconds each time, MU85 log).
    #[serde(skip)]
    built: u64,
    /// IFC data CreaBIM doesn't model, kept for the export (see
    /// [`crate::ifcdata`]); shared by the model's copies, as it is only
    /// set when a file is read.
    #[serde(skip)]
    ifc: std::sync::Arc<crate::ifcdata::IfcData>,
}

impl Model {
    /// The IFC data kept beside the elements (empty for most projects).
    pub fn ifc_data(&self) -> &crate::ifcdata::IfcData {
        &self.ifc
    }

    /// Keep `data` beside the elements: an import or a file being opened.
    pub fn set_ifc_data(&mut self, data: crate::ifcdata::IfcData) {
        self.ifc = std::sync::Arc::new(data);
    }
}

/// Builds nothing in the 3D scene or its snaps: annotations, their
/// styles, and what is only on sheets.
fn no_scene(e: &Element) -> bool {
    matches!(
        e.kind,
        ElementKind::Dimension(_)
            | ElementKind::Tag(_)
            | ElementKind::Note(_)
            | ElementKind::TextStyle(_)
            | ElementKind::Sheet(_)
            | ElementKind::Viewport(_)
            | ElementKind::SheetTemplate(_)
            | ElementKind::PaperMark(_)
            | ElementKind::MarkRules(_)
            // a crop box builds nothing: the 3D view cuts the scene with it
            // after (a face dragged rebuilt every join, 0.3–0.5 s a step)
            | ElementKind::CropBox(_)
    )
}

/// Is the change of `old` into `new` one to the project's notes alone
/// (its sections, drafts and to-dos)? Sheets show them; 3D does not, so
/// typing in the Notes area builds nothing.
fn notes_only(old: &Element, new: &Element) -> bool {
    let (ElementKind::Project(o), ElementKind::Project(n)) = (&old.kind, &new.kind) else {
        return false;
    };
    let mut as_new = o.clone();
    as_new.notes = n.notes.clone();
    as_new.pad = n.pad.clone();
    as_new.sections = n.sections.clone();
    as_new == *n
}

/// Is the change of `old` into `new` one to a style pack's patterns that
/// nothing draws with? Nothing drawn or built changes.
fn unused_patterns_only(model: &Model, old: &Element, new: &Element) -> bool {
    let (ElementKind::StylePack(o), ElementKind::StylePack(n)) = (&old.kind, &new.kind) else {
        return false;
    };
    let mut rest = old.clone();
    rest.kind = new.kind.clone();
    rest == *new && crate::styles::only_unused_patterns_changed(model, o, n)
}

/// Is the change of `old` into `new` one to a view's own look alone: how
/// it draws what it shows (shaded or hidden line, its drawing style,
/// hidden things dashed or in colour, room colours)? No other view reads
/// these of it (they read its line, its name, its callout box and its
/// range), and nothing in 3D does, so only that view's drawing changes.
fn look_only(old: &Element, new: &Element) -> bool {
    let (ElementKind::View(o), ElementKind::View(n)) = (&old.kind, &new.kind) else {
        return false;
    };
    let mut as_new = o.clone();
    as_new.style = n.style;
    as_new.graphics.drawing_style = n.graphics.drawing_style;
    as_new.show_hidden = n.show_hidden;
    as_new.hidden_in_colour = n.hidden_in_colour;
    as_new.room_colours = n.room_colours;
    if as_new != *n {
        return false;
    }
    let mut rest = old.clone();
    rest.kind = new.kind.clone();
    rest == *new
}

/// The next revision handed out, to any model.
static NEXT_REVISION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_revision() -> u64 {
    NEXT_REVISION.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Two models are equal when they hold the same elements in the same order;
/// the revision counter is bookkeeping, not content.
impl PartialEq for Model {
    fn eq(&self, other: &Self) -> bool {
        self.elements.len() == other.elements.len() && self.iter().eq(other.iter())
    }
}

impl Model {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// As [`Model::revision`], but unchanged by what is only on a sheet's
    /// paper: what views' drawings are cached by.
    pub fn drawing_revision(&self) -> u64 {
        self.drawn
    }

    /// What the 3D scene and its snaps are cached by: as
    /// [`Model::drawing_revision`], but unchanged by annotations and text
    /// styles too, which build nothing in 3D.
    pub fn scene_revision(&self) -> u64 {
        self.built
    }

    /// What view `view`'s own drawing is cached by: as
    /// [`Model::drawing_revision`], and changed too when the view's own
    /// look was (its style), which changes no other view's drawing.
    pub fn drawing_revision_of(&self, view: ElementId) -> u64 {
        self.drawn.max(self.looks.get(&view).copied().unwrap_or(0))
    }

    /// Written on a sheet's paper only: no view draws it.
    fn paper_only(&self, e: &Element) -> bool {
        match &e.kind {
            ElementKind::PaperMark(_) | ElementKind::Viewport(_) => true,
            ElementKind::Note(n) => {
                matches!(
                    self.get(n.view).map(|v| &v.kind),
                    Some(ElementKind::Sheet(_))
                )
            }
            _ => false,
        }
    }

    /// Have everything drawn from the model made again, the model itself
    /// unchanged: for what was shown in a quicker way while an edit was
    /// live and is to be shown in full once it ends.
    pub fn redraw(&mut self) {
        self.bump(true);
    }

    fn bump(&mut self, drawn: bool) {
        self.bump_with(drawn, drawn);
    }

    fn bump_with(&mut self, drawn: bool, built: bool) {
        self.revision = next_revision();
        if drawn {
            self.drawn = self.revision;
        }
        if built {
            self.built = self.revision;
        }
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn get(&self, id: ElementId) -> Option<&Element> {
        self.elements.get(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Element> {
        self.elements.values()
    }

    pub fn levels(&self) -> impl Iterator<Item = (&Element, &Level)> {
        self.iter().filter_map(|e| match &e.kind {
            ElementKind::Level(l) => Some((e, l)),
            _ => None,
        })
    }

    pub fn level(&self, id: ElementId) -> Option<&Level> {
        match self.get(id).map(|e| &e.kind) {
            Some(ElementKind::Level(l)) => Some(l),
            _ => None,
        }
    }

    /// The project's information element, if it has one.
    pub fn project_info(&self) -> Option<(crate::ElementId, &crate::ProjectInfo)> {
        self.iter().find_map(|e| match &e.kind {
            ElementKind::Project(p) => Some((e.id, p)),
            _ => None,
        })
    }

    /// The project's units: from its units element, millimetres when it
    /// has none (as every file from before units were added).
    pub fn units(&self) -> crate::Units {
        self.iter()
            .find_map(|e| match e.kind {
                ElementKind::Units(u) => Some(u),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// Keep only the elements `keep` accepts, without checking references:
    /// for read-only copies such as a view's model with what it hides left
    /// out ([`crate::visibility::without`]). Never commit edits from one.
    pub fn retain(&mut self, mut keep: impl FnMut(&Element) -> bool) {
        self.elements.retain(|_, e| keep(e));
        self.bump(true);
    }

    /// Elements that directly reference `id`.
    pub fn dependents(&self, id: ElementId) -> Vec<ElementId> {
        self.iter()
            .filter(|e| e.references().contains(&id))
            .map(|e| e.id)
            .collect()
    }

    fn check_refs(&self, e: &Element) -> Result<(), ModelError> {
        e.validate().map_err(|m| ModelError::Invalid(e.id, m))?;
        for r in e.references() {
            if !self.elements.contains_key(&r) {
                return Err(ModelError::DanglingReference {
                    element: e.id,
                    missing: r,
                });
            }
        }
        if let ElementKind::Opening(o) = &e.kind {
            match self.get(o.host).map(|h| &h.kind) {
                Some(ElementKind::Wall(_)) => {}
                // a roof window: a window placed in plan on its roof
                Some(ElementKind::Roof(_))
                    if o.kind == crate::OpeningKind::Window && o.roof_at.is_some() => {}
                _ => {
                    return Err(ModelError::Invalid(
                        e.id,
                        "opening host must be a wall (or a roof, for a roof window)".into(),
                    ))
                }
            }
        }
        Ok(())
    }

    pub(crate) fn insert(&mut self, e: Element) -> Result<(), ModelError> {
        if self.elements.contains_key(&e.id) {
            return Err(ModelError::Duplicate(e.id));
        }
        self.check_refs(&e)?;
        let drawn = !self.paper_only(&e);
        let built = drawn && !no_scene(&e);
        self.elements.insert(e.id, e);
        self.bump_with(drawn, built);
        Ok(())
    }

    pub(crate) fn remove(&mut self, id: ElementId) -> Result<Element, ModelError> {
        if let Some(by) = self.dependents(id).first() {
            return Err(ModelError::StillReferenced {
                element: id,
                by: *by,
            });
        }
        let drawn = self.get(id).is_none_or(|e| !self.paper_only(e));
        let built = drawn && self.get(id).is_none_or(|e| !no_scene(e));
        let e = self
            .elements
            .shift_remove(&id)
            .ok_or(ModelError::NotFound(id))?;
        self.bump_with(drawn, built);
        Ok(e)
    }

    pub(crate) fn replace(&mut self, e: Element) -> Result<Element, ModelError> {
        if !self.elements.contains_key(&e.id) {
            return Err(ModelError::NotFound(e.id));
        }
        self.check_refs(&e)?;
        let look = self.get(e.id).is_some_and(|o| look_only(o, &e));
        let unused = self
            .get(e.id)
            .is_some_and(|o| unused_patterns_only(self, o, &e));
        let drawn = !unused
            && !look
            && (!self.paper_only(&e) || self.get(e.id).is_some_and(|o| !self.paper_only(o)));
        let built = drawn
            && !(no_scene(&e) && self.get(e.id).is_some_and(no_scene))
            && !self.get(e.id).is_some_and(|o| notes_only(o, &e));
        let old = self.elements.insert(e.id, e).expect("checked above");
        self.bump_with(drawn, built);
        if look {
            self.looks.insert(old.id, self.revision);
        }
        Ok(old)
    }

    /// Build a model from elements in the given order, checking references
    /// once all are present. Unlike [`Model::from_json`] this keeps the order
    /// exactly, which matters because an element may legitimately reference
    /// one inserted after it (a wall moved to a newer level).
    pub fn from_elements(elements: impl IntoIterator<Item = Element>) -> Result<Self, ModelError> {
        let mut m = Model::new();
        for e in elements {
            if m.elements.contains_key(&e.id) {
                return Err(ModelError::Duplicate(e.id));
            }
            m.elements.insert(e.id, e);
        }
        for e in m.elements.values() {
            m.check_refs(e)?;
        }
        m.bump(true);
        Ok(m)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("model is always serializable")
    }

    /// Load a saved model, re-checking every reference.
    pub fn from_json(s: &str) -> Result<Self, String> {
        let raw: Model = serde_json::from_str(s).map_err(|e| e.to_string())?;
        let mut m = Model::new();
        // Insert in dependency order: an element goes in once its references exist.
        let mut pending: Vec<Element> = raw.elements.into_values().collect();
        while !pending.is_empty() {
            let before = pending.len();
            let mut rest = Vec::new();
            for e in pending {
                if e.references().iter().all(|r| m.elements.contains_key(r)) {
                    m.insert(e).map_err(|e| e.to_string())?;
                } else {
                    rest.push(e);
                }
            }
            if rest.len() == before {
                return Err(format!("{} elements have missing references", rest.len()));
            }
            pending = rest;
        }
        m.drawn = m.revision;
        m.built = m.revision;
        Ok(m)
    }
}

#[cfg(test)]
mod scene_revision_tests {
    #[test]
    fn a_text_style_change_leaves_the_scene_revision() {
        let mut doc = crate::Document::from_model(crate::starter::starter_house());
        let built = doc.model().scene_revision();
        assert_ne!(built, 0, "the starter house has a scene revision");
        assert_eq!(built, doc.model().drawing_revision(), "{built}");
        let style = doc
            .model()
            .iter()
            .find(|e| matches!(e.kind, crate::ElementKind::TextStyle(_)))
            .unwrap()
            .id;
        doc.edit(style, "font", |e| {
            if let crate::ElementKind::TextStyle(s) = &mut e.kind {
                s.font = crate::Font::Structa;
            }
        })
        .unwrap();
        assert_eq!(
            doc.model().scene_revision(),
            built,
            "a font builds nothing in 3D"
        );
        assert_ne!(
            doc.model().drawing_revision(),
            built,
            "but drawings show it"
        );
    }

    #[test]
    fn drawing_a_pattern_no_material_uses_rebuilds_nothing() {
        let mut doc = crate::Document::from_model(crate::starter::starter_house());
        let pack = crate::styles::StylePack::standard();
        let tx = crate::styles::apply_pack(doc.model(), &pack);
        doc.commit(tx).unwrap();
        let id = crate::styles::packs(doc.model())[0].0;
        let (built, drawn) = (doc.model().scene_revision(), doc.model().drawing_revision());
        let set = |doc: &mut crate::Document, f: &dyn Fn(&mut crate::styles::StylePack)| {
            doc.edit(id, "pattern", |e| {
                if let crate::ElementKind::StylePack(p) = &mut e.kind {
                    f(p);
                }
            })
            .unwrap();
        };
        set(&mut doc, &|p| {
            p.patterns.push(crate::styles::HatchPattern {
                name: "Doodle".into(),
                ..Default::default()
            })
        });
        set(&mut doc, &|p| p.patterns.last_mut().unwrap().angle = 30.0);
        assert_eq!(doc.model().scene_revision(), built, "nothing uses Doodle");
        assert_eq!(
            doc.model().drawing_revision(),
            drawn,
            "nothing draws Doodle"
        );
        // a pattern a material uses: its drawings and 3D change
        let brick = crate::Hatch::Brick.label().to_string();
        assert!(
            !crate::styles::pattern_users(doc.model(), &brick).is_empty(),
            "the starter house has brick"
        );
        set(&mut doc, &|p| {
            p.patterns.push(crate::styles::HatchPattern {
                name: brick.clone(),
                ..Default::default()
            })
        });
        assert_ne!(doc.model().drawing_revision(), drawn, "brick is drawn");
    }

    #[test]
    fn a_crop_box_face_dragged_rebuilds_nothing_in_3d() {
        use glam::DVec3;
        let mut doc = crate::Document::from_model(crate::starter::starter_house());
        let c = crate::Element::new(
            "Crop box 1",
            crate::ElementKind::CropBox(crate::CropBox::new(
                None,
                DVec3::splat(-20.0),
                DVec3::splat(20.0),
            )),
        );
        let id = c.id;
        doc.commit(crate::Transaction::new("Add crop box").insert(c))
            .unwrap();
        let (built, drawn) = (doc.model().scene_revision(), doc.model().drawing_revision());
        // a face dragged: the box changes at every step of the drag
        for step in 1..=3 {
            doc.edit(id, "Crop face", |e| {
                if let crate::ElementKind::CropBox(b) = &mut e.kind {
                    b.max.x = 20.0 - step as f64;
                }
            })
            .unwrap();
        }
        assert_eq!(
            doc.model().scene_revision(),
            built,
            "the 3D scene is cut by it after, never built from it"
        );
        assert_ne!(
            doc.model().drawing_revision(),
            drawn,
            "sections and plans it crops change"
        );
    }
}
