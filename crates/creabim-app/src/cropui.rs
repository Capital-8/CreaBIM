//! Crop boxes in the app (Revit's section boxes; see
//! [`creabim_core::cropbox`]): adding them, their frames and picking in the
//! 3D view, their list and fields in Properties, and the menu entries.
//!
//! The 3D window works with one set of crop boxes at a time: those of the
//! selected crop box's view, else of the 3D view open in the drawing pane,
//! else its own. So a 3D view's crop boxes are edited in the 3D window and
//! show on every sheet that view is placed on.

use super::*;
use crate::ui_model::Item;
use creabim_core::{cropbox, CropBox, CropScope};

const FRAME: Color32 = Color32::from_rgb(40, 110, 200);
/// A crop box switched off: its frame grey and dashed.
const OFF_FRAME: Color32 = Color32::from_gray(140);
fn frame_selected() -> Color32 {
    crate::theme::interaction()
}
/// How far a new crop box stands off what it is fitted round, metres.
const MARGIN: f64 = 0.3;

impl CreaBimApp {
    /// Whose crop boxes the 3D window shows and adds to: a 3D view, or
    /// `None` for its own.
    pub(super) fn crop_owner(&self) -> Option<ElementId> {
        let model = self.doc.model();
        let of_box = |id: ElementId| match model.get(id).map(|e| &e.kind) {
            Some(ElementKind::CropBox(c)) => Some(c.view),
            _ => None,
        };
        if let Some(owner) = self.crop_pick.and_then(of_box) {
            return owner;
        }
        if let Some(owner) = self.selection.active.and_then(of_box) {
            return owner;
        }
        self.active_view.filter(|v| {
            matches!(
                self.view(*v).map(|v| &v.kind),
                Some(ViewKind::Axonometric { .. })
            )
        })
    }

    /// Crop the 3D window's scene with the crop boxes it shows; returns the
    /// cut edges to draw.
    pub(super) fn crop_scene_3d(&mut self) -> Vec<(DVec3, DVec3)> {
        let boxes = creabim_geom::cropbox::active(self.doc.model(), self.crop_owner());
        creabim_geom::cropbox::crop_scene(self.doc.model(), &mut self.scene, &boxes)
    }

    fn owner_name(&self, owner: Option<ElementId>) -> String {
        match owner.and_then(|v| self.doc.model().get(v)) {
            Some(e) => format!("3D view “{}”", e.name),
            None => "the 3D window".into(),
        }
    }

    /// The bounds of what the model shows, or of `only` of it.
    fn shown_bounds(&self, only: Option<&[ElementId]>) -> Option<(DVec3, DVec3)> {
        creabim_geom::scene::drawn_scene(self.doc.model())
            .iter()
            .filter(|s| only.is_none_or(|ids| ids.contains(&s.id)))
            .filter(|s| !self.hidden.contains(&s.id))
            .filter_map(|s| s.mesh.bounds())
            .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))
            .map(|(a, b)| (a.as_dvec3(), b.as_dvec3()))
    }

    /// Selected building elements a crop box can be fitted round.
    fn selected_croppable(&self) -> Vec<ElementId> {
        self.selection
            .all()
            .into_iter()
            .filter(|id| {
                !matches!(
                    self.doc.model().get(*id).map(|e| &e.kind),
                    Some(ElementKind::CropBox(_) | ElementKind::View(_)) | None
                )
            })
            .collect()
    }

    /// Add a crop box to the 3D window's current set. `around` fits it round
    /// the selection; `only` makes it crop just the selection.
    pub(super) fn add_crop_box(&mut self, around: bool, only: bool) {
        let owner = self.crop_owner();
        let picked = self.selected_croppable();
        if (around || only) && picked.is_empty() {
            self.status = "Select the elements to fit the crop box round first.".into();
            return;
        }
        let bounds = if around || only {
            self.shown_bounds(Some(&picked))
        } else {
            self.shown_bounds(None)
        };
        let (lo, hi) = bounds.unwrap_or((DVec3::new(-5.0, -5.0, 0.0), DVec3::new(5.0, 5.0, 3.0)));
        let mut c = CropBox::new(owner, lo - DVec3::splat(MARGIN), hi + DVec3::splat(MARGIN));
        if only {
            c.scope = CropScope::Only(picked);
        }
        let name = cropbox::next_name(self.doc.model(), owner);
        let e = Element::new(name.clone(), ElementKind::CropBox(c));
        let id = e.id;
        let r = self
            .doc
            .commit(Transaction::new(format!("Add {name}")).insert(e));
        if r.is_ok() {
            self.selection.only(id);
            self.face = None;
        }
        let what = if only {
            "it crops only the selected elements"
        } else {
            "it crops everything in the view"
        };
        self.report(
            r,
            format!(
                "Added {name} to {}: {what}. Drag the handles on its faces to crop.",
                self.owner_name(owner)
            ),
        );
    }

    /// A plan's or section's cut as a section box in the 3D window (MU34
    /// A2): a plan keeps what is under its cut, down to its view depth
    /// (`up`: what is above it, looking at the ceilings); a section keeps
    /// what it looks at, from its line to its depth. One box per view,
    /// replaced when asked again.
    pub(super) fn cut_box(&mut self, view: ElementId, up: bool) {
        let model = self.doc.model();
        let Some(e) = model.get(view) else { return };
        let ElementKind::View(v) = &e.kind else {
            return;
        };
        let name = format!("{} cut", e.name);
        let (mut lo, mut hi) = self
            .shown_bounds(None)
            .unwrap_or((DVec3::new(-5.0, -5.0, 0.0), DVec3::new(5.0, 5.0, 3.0)));
        lo -= DVec3::splat(MARGIN);
        hi += DVec3::splat(MARGIN);
        match v.kind {
            ViewKind::Plan { level, cut_height } => {
                let z = model.level(level).map_or(0.0, |l| l.elevation);
                let below = model
                    .levels()
                    .map(|(_, x)| x.elevation)
                    .filter(|x| *x < z - 1e-6)
                    .fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.max(x))));
                if up {
                    lo.z = z + cut_height;
                } else {
                    hi.z = z + cut_height;
                    lo.z = z - v.graphics.depth(z, below);
                }
            }
            ViewKind::Section {
                start, end, depth, ..
            } => {
                let look = (end - start).normalize_or_zero().perp();
                let (a, b) = (start, start + look * depth);
                if look.x.abs() > 0.999 {
                    lo.x = a.x.min(b.x);
                    hi.x = a.x.max(b.x);
                } else if look.y.abs() > 0.999 {
                    lo.y = a.y.min(b.y);
                    hi.y = a.y.max(b.y);
                } else {
                    self.status = "A section box stays square to the grid: this section runs at an angle, so it can't be one.".into();
                    return;
                }
            }
            _ => {
                self.status = "Only a plan's or a section's cut makes a section box.".into();
                return;
            }
        }
        let existing = model
            .iter()
            .find(|x| x.name == name && matches!(x.kind, ElementKind::CropBox(_)))
            .map(|x| x.id);
        let c = CropBox::new(None, lo, hi);
        let r = match existing {
            Some(id) => self.doc.edit(id, &format!("Update {name}"), |x| {
                if let ElementKind::CropBox(old) = &mut x.kind {
                    old.min = c.min;
                    old.max = c.max;
                    old.enabled = true;
                }
            }),
            None => self.doc.commit(
                Transaction::new(format!("Add {name}"))
                    .insert(Element::new(name.clone(), ElementKind::CropBox(c))),
            ),
        };
        self.report(
            r,
            format!(
                "3D is cut as {} sees it{}: drag the box's faces to change it, or switch it off in the browser.",
                e_name(&name),
                if up { ", looking up at the ceilings" } else { "" }
            ),
        );
    }

    fn crop_box(&self, id: ElementId) -> Option<(&Element, &CropBox)> {
        let e = self.doc.model().get(id)?;
        match &e.kind {
            ElementKind::CropBox(c) => Some((e, c)),
            _ => None,
        }
    }

    fn edit_crop(&mut self, id: ElementId, label: &str, f: impl FnOnce(&mut CropBox)) {
        let r = self.doc.edit(id, label, |e| {
            if let ElementKind::CropBox(c) = &mut e.kind {
                f(c)
            }
        });
        self.report(r, format!("{label}. Ctrl+Z to undo."));
    }

    /// Menu and browser actions for crop boxes; `false` for other actions.
    pub(super) fn crop_action(&mut self, head: &str, arg: &str) -> bool {
        let id = self
            .doc
            .model()
            .iter()
            .find(|e| e.id.0.to_string() == arg)
            .map(|e| e.id);
        match (head, id) {
            ("crop.add", _) => self.add_crop_box(false, false),
            ("crop.around", _) => self.add_crop_box(true, false),
            ("crop.only", _) => self.add_crop_box(true, true),
            ("crop.toggle", Some(id)) => {
                let on = self.crop_box(id).is_some_and(|(_, c)| c.enabled);
                self.edit_crop(id, if on { "Crop box off" } else { "Crop box on" }, |c| {
                    c.enabled = !on
                });
            }
            ("crop.frame", Some(id)) => {
                let hidden = self.crop_box(id).is_some_and(|(_, c)| c.frame_hidden);
                self.edit_crop(
                    id,
                    if hidden {
                        "Show crop box"
                    } else {
                        "Hide crop box"
                    },
                    |c| c.frame_hidden = !hidden,
                );
            }
            ("crop.select", Some(id)) => {
                self.selection.only(id);
                self.face = None;
            }
            _ => return false,
        }
        true
    }

    /// The View menu's crop box entries.
    pub(super) fn crop_menu(&self) -> Vec<Item> {
        let owner = self.crop_owner();
        let some = !self.selected_croppable().is_empty();
        let mut items = vec![
            Item::new("crop.add", "New crop box")
                .icon("crop-box")
                .hint("A box round the whole model that crops everything in the 3D view; drag its faces in"),
            Item::new("crop.around", "New crop box round the selection")
                .enabled(some)
                .hint("Crops everything in the view to the selected elements"),
            Item::new("crop.only", "New crop box for the selection only")
                .enabled(some)
                .hint("Crops only the selected elements; everything else stays whole"),
        ];
        let boxes = cropbox::crop_boxes(self.doc.model(), owner);
        if !boxes.is_empty() {
            items.push(Item::Separator);
            items.push(Item::Header(format!(
                "Crop boxes of {}",
                self.owner_name(owner)
            )));
            for (e, c) in boxes {
                items.push(
                    Item::new(format!("crop.toggle:{}", e.id.0), e.name.clone())
                        .checked(c.enabled)
                        .keep_open()
                        .hint("Crop with this box, or switch it off"),
                );
            }
        }
        items
    }

    /// The crop boxes of `owner`, each with its switches, in Properties.
    pub(super) fn crop_list(&mut self, ui: &mut egui::Ui, owner: Option<ElementId>) {
        let title = format!("Crop boxes of {}", self.owner_name(owner));
        let open = self.prop_header(ui, "crops", &title, true, Some("crop-boxes"));
        crate::props::fold(ui, open, |ui| {
            let rows: Vec<(ElementId, String, bool, bool)> =
                cropbox::crop_boxes(self.doc.model(), owner)
                    .into_iter()
                    .map(|(e, c)| (e.id, e.name.clone(), c.enabled, !c.frame_hidden))
                    .collect();
            if rows.is_empty() {
                crate::props::empty(
                    ui,
                    "None yet. A crop box cuts the 3D view down to what is inside it.",
                );
            }
            let mut act: Option<(&str, ElementId)> = None;
            // a box each, titled by its name, with the house rows inside:
            // the key in the label column, the tick, its value (MU71: no
            // bare ticks, no small button)
            for (id, name, on, shown) in &rows {
                crate::props::sub_box(ui, &format!("crop.list.{}", id.0), name, None, true, |ui| {
                    let mut v = *on;
                    if crate::props::check(ui, "Crop box", &mut v, "Crop")
                        .on_hover_text("Crop the view with it")
                        .changed()
                    {
                        act = Some(("crop.toggle", *id));
                    }
                    let mut s = *shown;
                    if crate::props::check(ui, "", &mut s, "Show the box")
                        .on_hover_text("Its frame and handles; hidden, it still crops")
                        .changed()
                    {
                        act = Some(("crop.frame", *id));
                    }
                    // in from the card's edges alike (MU145)
                    let (sel, del) = crate::props::card_inset(ui, |ui| {
                        super::gridvis::button_pair(ui, "Select", "Delete")
                    });
                    if sel {
                        act = Some(("crop.select", *id));
                    }
                    if del {
                        act = Some(("delete", *id));
                    }
                });
            }
            if let Some((what, id)) = act {
                if what == "delete" {
                    let name = self
                        .crop_box(id)
                        .map_or(String::new(), |(e, _)| e.name.clone());
                    let r = self.doc.commit(
                        Transaction::new(format!("Delete {name}"))
                            .remove_all_cascade(self.doc.model(), &[id]),
                    );
                    self.report(r, format!("Deleted {name}. Ctrl+Z brings it back."));
                    self.drop_stale_selection();
                } else {
                    self.crop_action(what, &id.0.to_string());
                }
            }
            // the house buttons, on the rows' edges (MU69)
            let some = !self.selected_croppable().is_empty();
            if super::gridvis::full_button(ui, true, "Add crop box").clicked() {
                self.add_crop_box(false, false);
            }
            if super::gridvis::full_button(ui, some, "Round the selection")
                .on_disabled_hover_text("Select what it should go round first")
                .clicked()
            {
                self.add_crop_box(true, false);
            }
        });
    }

    /// Properties of a crop box; returns the undo label when `c` changed.
    pub(super) fn crop_properties(
        &mut self,
        ui: &mut egui::Ui,
        id: ElementId,
        c: &mut CropBox,
    ) -> Option<&'static str> {
        let mut label = None;
        crate::props::worked_out(
            ui,
            "crop",
            &[crate::props::Worked::new("Crops", self.owner_name(c.view))],
        );
        crate::props::howto(
            ui,
            "Crop box: drag the round handles on its faces to resize it, the square one to move it.",
        );
        // the house rows (MU69: this panel had kept its own radios, a
        // table running past the edge and a short button)
        if crate::props::check(ui, "Crop box", &mut c.enabled, "Crop").changed() {
            label = Some("Crop box on or off");
        }
        let mut shown = !c.frame_hidden;
        if crate::props::check(ui, "", &mut shown, "Show the box")
            .on_hover_text(
                "Hidden, it still crops; its frame shows again only while it is selected",
            )
            .changed()
        {
            c.frame_hidden = !shown;
            label = Some("Show or hide crop box");
        }
        crate::props::row(ui, "Outside", |ui| {
            let w = ui.available_width().floor().max(40.0);
            let pad = ui.spacing().button_padding.x * 2.0 + ui.spacing().icon_width;
            egui::ComboBox::from_id_salt(("crop.outside", id))
                .icon(crate::props::combo_icon)
                .width((w - pad).max(20.0))
                .truncate()
                .selected_text(c.outside.label())
                .show_ui(ui, |ui| {
                    for o in cropbox::Outside::ALL {
                        if ui.selectable_label(c.outside == o, o.label()).clicked() && c.outside != o {
                            c.outside = o;
                            label = Some("What shows outside the crop box");
                        }
                    }
                })
                .response
                .on_hover_text(format!(
                    "{}: what shows of what the box crops away whole: nothing, a ghost of what is selected, or a ghost of all of it",
                    c.outside.label()
                ))
        });
        let n = match &c.scope {
            CropScope::Only(ids) => ids.len(),
            CropScope::All => 0,
        };
        let was = matches!(c.scope, CropScope::Only(_));
        let mut only = was;
        let chosen = format!("Picked ({n})");
        crate::props::choice(
            ui,
            "Crops",
            &mut only,
            &[
                (false, "All", "every element in the view"),
                (true, chosen.as_str(), "only the elements you pick"),
            ],
        );
        if only != was {
            if only {
                c.scope = CropScope::Only(vec![]);
                label = Some("Crop chosen elements");
                self.start_crop_pick(id);
            } else {
                c.scope = CropScope::All;
                label = Some("Crop everything");
            }
        }
        if only {
            let picking = self.crop_pick == Some(id);
            let pick = if picking {
                "Done picking"
            } else {
                "Pick elements…"
            };
            if n > 0 {
                let (p, clear) = super::gridvis::button_pair(ui, pick, "Clear");
                if p {
                    if picking {
                        self.end_crop_pick();
                    } else {
                        self.start_crop_pick(id);
                    }
                }
                if clear {
                    c.scope = CropScope::Only(vec![]);
                    label = Some("Crop chosen elements");
                }
            } else if super::gridvis::full_button(ui, true, pick).clicked() {
                if picking {
                    self.end_crop_pick();
                } else {
                    self.start_crop_pick(id);
                }
            }
        }
        // where its faces are: one full-width row per axis, a box for each
        // side, as Elbows (MU66: two fields in a row pushed the panel)
        for (side, key, title) in [(0, "crop.from", "From"), (1, "crop.to", "To")] {
            crate::props::sub_box(ui, key, title, None, true, |ui| {
                for (axis, name) in ["X", "Y", "Z"].iter().enumerate() {
                    let mut v = if side == 0 { c.min[axis] } else { c.max[axis] };
                    crate::props::row(ui, name, |ui| {
                        if crate::props::fill(ui, crate::units::drag(&mut v).range(-1e5..=1e5))
                            .on_hover_text(
                                "Where this face of the box is; or drag its round handle",
                            )
                            .changed()
                        {
                            c.set_face(axis * 2 + side, v);
                            label = Some("Crop box extents");
                        }
                    });
                }
            });
        }
        // the unit once, at the end, so it fits the row (MU69)
        let s = c.size();
        let bare = |v: f64| {
            let t = crate::units::len(v);
            t.rsplit_once(' ').map_or(t.clone(), |(n, _)| n.to_string())
        };
        let unit = crate::units::len(s.z)
            .rsplit_once(' ')
            .map_or(String::new(), |(_, u)| format!(" {u}"));
        crate::props::readout(
            ui,
            "Size",
            format!("{} × {} × {}{unit}", bare(s.x), bare(s.y), bare(s.z)),
            "Width × depth × height of the box",
        );
        let fit = |me: &Self, c: &mut CropBox| -> bool {
            let only = match &c.scope {
                CropScope::Only(ids) if !ids.is_empty() => Some(ids.clone()),
                _ => None,
            };
            match me.shown_bounds(only.as_deref()) {
                Some((lo, hi)) => {
                    c.min = lo - DVec3::splat(MARGIN);
                    c.max = hi + DVec3::splat(MARGIN);
                    c.fit = None;
                    true
                }
                None => false,
            }
        };
        let what = if n > 0 {
            "Fit to its elements"
        } else {
            "Fit to the model"
        };
        if super::gridvis::full_button(ui, true, what).clicked() && fit(self, c) {
            label = Some("Fit crop box");
        }
        label
    }

    fn start_crop_pick(&mut self, id: ElementId) {
        self.crop_pick = Some(id);
        let picked = match self.crop_box(id).map(|(_, c)| &c.scope) {
            Some(CropScope::Only(ids)) => ids.clone(),
            _ => vec![],
        };
        self.selection.clear();
        self.selection.extend(picked, false);
        self.status = "Click elements in the 3D view to crop them with this box, again to leave them out. Esc or right-click when done.".into();
    }

    fn end_crop_pick(&mut self) {
        if let Some(id) = self.crop_pick.take() {
            self.selection.only(id);
            let n = match self.crop_box(id).map(|(_, c)| &c.scope) {
                Some(CropScope::Only(ids)) => ids.len(),
                _ => 0,
            };
            self.status = format!("The crop box crops {n} chosen elements; the rest stay whole.");
        }
    }

    /// Over Properties while a crop box's elements are being picked.
    pub(super) fn crop_pick_banner(&mut self, ui: &mut egui::Ui) {
        let Some((name, n)) = self.crop_pick.and_then(|id| {
            let (e, c) = self.crop_box(id)?;
            let n = match &c.scope {
                CropScope::Only(ids) => ids.len(),
                CropScope::All => 0,
            };
            Some((e.name.clone(), n))
        }) else {
            return;
        };
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(egui::RichText::new(format!("Picking what {name} crops")).strong());
            ui.label(format!(
                "{n} chosen. Click elements in the 3D view to add them, or click one again to leave it whole."
            ));
            if ui.button("Done picking").clicked() {
                self.end_crop_pick();
            }
        });
        ui.add_space(6.0);
    }

    /// While picking a crop box's elements: a click adds or takes away
    /// what it lands on, Esc or right-click ends. `true` while picking.
    pub(super) fn crop_pick_3d(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        ray: &dyn Fn(egui::Pos2) -> (DVec3, DVec3),
    ) -> bool {
        let Some(id) = self.crop_pick else {
            return false;
        };
        if self.crop_box(id).is_none() {
            self.crop_pick = None;
            return false;
        }
        if response.dragged_by(egui::PointerButton::Primary) {
            self.orbit_3d(response.drag_delta());
        }
        let esc = response.hovered() && ui.input(|i| i.key_pressed(egui::Key::Escape));
        if esc || response.secondary_clicked() {
            self.end_crop_pick();
            return true;
        }
        if response.clicked() {
            if let Some(p) = response.interact_pointer_pos() {
                let (o, d) = ray(p);
                // what shows, as cropped: what is cut away can't be clicked
                if let Some(h) = pick::pick(&self.scene, o.as_vec3(), d.as_vec3()) {
                    // a door or window is cropped with its wall
                    let hit = match self.doc.model().get(h.id).map(|e| &e.kind) {
                        Some(ElementKind::Opening(op)) => op.host,
                        _ => h.id,
                    };
                    let name = self
                        .doc
                        .model()
                        .get(hit)
                        .map_or(String::new(), |e| e.name.clone());
                    let mut added = false;
                    let r = self.doc.edit(id, "Crop box elements", |e| {
                        if let ElementKind::CropBox(c) = &mut e.kind {
                            let mut ids = match std::mem::take(&mut c.scope) {
                                CropScope::Only(ids) => ids,
                                CropScope::All => vec![],
                            };
                            match ids.iter().position(|i| *i == hit) {
                                Some(at) => {
                                    ids.remove(at);
                                }
                                None => {
                                    ids.push(hit);
                                    added = true;
                                }
                            }
                            c.scope = CropScope::Only(ids);
                        }
                    });
                    if added {
                        self.selection.extend([hit], true);
                    } else {
                        self.selection.retain(|s| s != hit);
                    }
                    self.report(
                        r,
                        if added {
                            format!("{name} is cropped by the box. Esc or right-click when done.")
                        } else {
                            format!("{name} left whole. Esc or right-click when done.")
                        },
                    );
                }
            }
        }
        true
    }

    /// The frames of the crop boxes the 3D window shows, for its line
    /// buffers: drawn with the model's depth, so what stands in front of a
    /// frame hides it, as it hides the model's own edges. A frame painted
    /// over the view crossed whatever lay in front of its far edges (MU143,
    /// owner: "a line from the side ... looking like it's crossing the
    /// foundation"). A box switched off has its frame dashed and grey.
    pub(super) fn crop_frame_lines(&self) -> Vec<(DVec3, DVec3, [f32; 3])> {
        // the 3D window's line shader raises its colours to the power 2.2
        // (`fs_flat`): given so the frame shows in the colour its name and
        // handles are in (seen in the app: a dull navy at first)
        let rgb = |c: Color32| [c.r(), c.g(), c.b()].map(|k| (k as f32 / 255.0).powf(1.0 / 2.2));
        let mut out = vec![];
        for (_, c) in cropbox::crop_boxes(self.doc.model(), self.crop_owner()) {
            if c.frame_hidden {
                continue;
            }
            for (a, b) in c.edges() {
                if c.enabled {
                    out.push((a, b, rgb(FRAME)));
                    continue;
                }
                let (len, dash, gap) = (a.distance(b), 0.3, 0.2);
                let mut t = 0.0;
                while t < len {
                    let e = (t + dash).min(len);
                    out.push((
                        a + (b - a) * (t / len),
                        a + (b - a) * (e / len),
                        rgb(OFF_FRAME),
                    ));
                    t = e + gap;
                }
            }
        }
        out
    }

    /// The crop boxes whose frames are drawn over the view, whole: the one
    /// selected or picked, which is being edited with its handles. The
    /// others are drawn with the model ([`Self::crop_frame_lines`]).
    pub(super) fn crop_frames_on_top(&self) -> Vec<ElementId> {
        cropbox::crop_boxes(self.doc.model(), self.crop_owner())
            .into_iter()
            .map(|(e, _)| e.id)
            .filter(|id| self.selection.contains(*id) || self.crop_pick == Some(*id))
            .collect()
    }

    /// Draw the names of the crop boxes the 3D window shows, and the frame
    /// of the selected one over the view, whole, in the selection colour
    /// (its handles come with it). The other frames are drawn with the
    /// model, where it hides them.
    pub(super) fn draw_crop_frames(
        &self,
        painter: &egui::Painter,
        project: &dyn Fn(DVec3) -> Option<egui::Pos2>,
    ) {
        let owner = self.crop_owner();
        let on_top = self.crop_frames_on_top();
        for (e, c) in cropbox::crop_boxes(self.doc.model(), owner) {
            let selected = on_top.contains(&e.id);
            if c.frame_hidden && !selected {
                continue;
            }
            let color = match (selected, c.enabled) {
                (true, _) => frame_selected(),
                (false, true) => FRAME,
                (false, false) => OFF_FRAME,
            };
            let stroke = egui::Stroke::new(2.0, color);
            for (a, b) in c.edges().into_iter().filter(|_| selected) {
                let (Some(pa), Some(pb)) = (project(a), project(b)) else {
                    continue;
                };
                if c.enabled {
                    painter.line_segment([pa, pb], stroke);
                } else {
                    painter.add(egui::Shape::dashed_line(&[pa, pb], stroke, 6.0, 4.0));
                }
            }
            if let Some(at) = project(c.max) {
                let text = if c.enabled {
                    e.name.clone()
                } else {
                    format!("{} (off)", e.name)
                };
                painter.text(
                    at + Vec2::new(4.0, -4.0),
                    egui::Align2::LEFT_BOTTOM,
                    text,
                    egui::FontId::proportional(11.0),
                    color,
                );
            }
        }
    }

    /// Selected elements an active crop box takes away whole (A5-4): each
    /// one's bounds, and the first box that misses it.
    pub(super) fn outside_crop(&mut self) -> Vec<(ElementId, (DVec3, DVec3), ElementId)> {
        let model = self.doc.model();
        let boxes = cropbox::crop_boxes(model, self.crop_owner());
        if boxes.iter().all(|(_, c)| !c.enabled) {
            return vec![];
        }
        let mut out = vec![];
        // a box that always ghosts takes in everything; the others only
        // the selection, unless they hide it (MU26)
        let always = boxes
            .iter()
            .any(|(_, c)| c.enabled && c.outside == cropbox::Outside::AlwaysGhost);
        let ids: Vec<ElementId> = if always {
            model.iter().map(|e| e.id).collect()
        } else {
            self.selection.all()
        };
        // every element's real extent, from its 3D body (MU27: windows,
        // doors, masses, furniture, columns and beams had none), kept
        // while the model and the list stay the same
        let rev = model.revision();
        let fresh = self
            .outside_bounds
            .as_ref()
            .is_some_and(|(r, l, _)| *r == rev && *l == ids);
        if !fresh {
            let only: std::collections::HashSet<ElementId> = ids.iter().copied().collect();
            let mut b: std::collections::HashMap<ElementId, (DVec3, DVec3)> = Default::default();
            for item in creabim_geom::scene::build_scene_of(model, Some(&only)) {
                if let Some((lo, hi)) = item.mesh.bounds() {
                    let (lo, hi) = (lo.as_dvec3(), hi.as_dvec3());
                    b.entry(item.id)
                        .and_modify(|e| *e = (e.0.min(lo), e.1.max(hi)))
                        .or_insert((lo, hi));
                }
            }
            self.outside_bounds = Some((rev, ids.clone(), b));
        }
        let bounds = self
            .outside_bounds
            .as_ref()
            .map(|c| c.2.clone())
            .unwrap_or_default();
        let model = self.doc.model();
        for id in ids {
            let selected = self.selection.contains(id);
            let Some(e) = model.get(id) else { continue };
            if matches!(e.kind, ElementKind::CropBox(_) | ElementKind::View(_)) {
                continue;
            }
            let Some((lo, hi)) = bounds
                .get(&id)
                .copied()
                .or_else(|| creabim_core::parts::element_bounds(model, e))
            else {
                continue;
            };
            let host = match &e.kind {
                ElementKind::Opening(o) => Some(o.host),
                _ => None,
            };
            if let Some((b, _)) = boxes.iter().find(|(_, c)| {
                c.enabled
                    && c.crops(id, host)
                    && c.misses(lo, hi)
                    && match c.outside {
                        cropbox::Outside::Hidden => false,
                        cropbox::Outside::GhostSelected => selected,
                        cropbox::Outside::AlwaysGhost => true,
                    }
            }) {
                out.push((id, (lo, hi), b.id));
            }
        }
        out
    }

    /// What lies outside the crop drawn as a ghost: a dashed box and a faint
    /// fill in the crop colour, so nothing vanishes without a word (A5-4).
    pub(super) fn draw_outside_crop(
        &self,
        painter: &egui::Painter,
        project: &dyn Fn(DVec3) -> Option<egui::Pos2>,
        outside: &[(ElementId, (DVec3, DVec3), ElementId)],
    ) {
        let stroke = egui::Stroke::new(1.4, FRAME);
        for (_, (lo, hi), _) in outside {
            let c = CropBox::new(None, *lo, *hi);
            let corners: Vec<egui::Pos2> = (0..8)
                .filter_map(|i| {
                    project(DVec3::new(
                        if i & 1 == 0 { lo.x } else { hi.x },
                        if i & 2 == 0 { lo.y } else { hi.y },
                        if i & 4 == 0 { lo.z } else { hi.z },
                    ))
                })
                .collect();
            if corners.len() == 8 {
                painter.add(egui::Shape::convex_polygon(
                    hull(&corners),
                    FRAME.gamma_multiply(0.12),
                    egui::Stroke::NONE,
                ));
            }
            for (a, b) in c.edges() {
                if let (Some(pa), Some(pb)) = (project(a), project(b)) {
                    painter.add(egui::Shape::dashed_line(&[pa, pb], stroke, 5.0, 4.0));
                }
            }
        }
    }

    /// The chip at the top of the 3D area for what the crop takes away:
    /// extend the box, show it anyway, or turn the crop off (A5-4).
    pub(super) fn outside_crop_chip(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        outside: &[(ElementId, (DVec3, DVec3), ElementId)],
    ) {
        let picked: Vec<(ElementId, (DVec3, DVec3), ElementId)> = outside
            .iter()
            .filter(|o| self.selection.contains(o.0))
            .copied()
            .collect();
        let outside = &picked[..];
        let Some((first, _, b)) = outside.first() else {
            self.outside_said = None;
            return;
        };
        let b = *b;
        // said once in the status bar too, as a new element lands outside
        if self.outside_said != Some(*first) {
            self.outside_said = Some(*first);
            let name = self
                .doc
                .model()
                .get(*first)
                .map_or("It".into(), |e| e.name.clone());
            let bx = self
                .doc
                .model()
                .get(b)
                .map_or("the crop box".into(), |e| e.name.clone());
            self.status = format!(
                "{name} is outside {bx}, so the 3D view crops it away: extend the box, show it anyway or turn the crop off (the chip at the top)."
            );
        }
        let name = self
            .doc
            .model()
            .get(b)
            .map_or("the crop box".into(), |e| e.name.clone());
        let mut pick = None;
        egui::Area::new(ui.id().with("outside-crop"))
            .fixed_pos(rect.center_top() + Vec2::new(-200.0, 10.0))
            .order(crate::app::overlay_order())
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    crate::props::flow(ui, |ui| {
                        let n = outside.len();
                        let text = if n == 1 {
                            format!("Outside {name}")
                        } else {
                            format!("{n} selected outside {name}")
                        };
                        ui.label(egui::RichText::new(text).color(FRAME).strong());
                        if ui
                            .button("Extend the box")
                            .on_hover_text("Grow the box to take it in: one undo step")
                            .clicked()
                        {
                            pick = Some(0);
                        }
                        if ui
                            .button("Show anyway")
                            .on_hover_text("Leave it whole: the box no longer crops it")
                            .clicked()
                        {
                            pick = Some(1);
                        }
                        if ui
                            .button("Turn crop off")
                            .on_hover_text("Switch the box off; it keeps its place")
                            .clicked()
                        {
                            pick = Some(2);
                        }
                    });
                });
            });
        let mine = outside.iter().filter(|o| o.2 == b);
        match pick {
            Some(0) => {
                let (lo, hi) = mine.fold(
                    (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)),
                    |(a, z), o| (a.min(o.1 .0), z.max(o.1 .1)),
                );
                self.edit_crop(b, "Extend the crop box", |c| {
                    c.min = c.min.min(lo - DVec3::splat(MARGIN));
                    c.max = c.max.max(hi + DVec3::splat(MARGIN));
                    c.fit = None;
                });
            }
            Some(1) => {
                let ids: Vec<ElementId> = mine.map(|o| o.0).collect();
                self.edit_crop(b, "Show it anyway", |c| c.except.extend(ids));
            }
            Some(2) => self.edit_crop(b, "Crop box off", |c| c.enabled = false),
            _ => {}
        }
    }

    /// The crop box whose frame is near screen point `p`, to click it.
    pub(super) fn crop_frame_near(
        &self,
        p: egui::Pos2,
        project: &dyn Fn(DVec3) -> Option<egui::Pos2>,
    ) -> Option<ElementId> {
        cropbox::crop_boxes(self.doc.model(), self.crop_owner())
            .into_iter()
            .filter(|(_, c)| !c.frame_hidden)
            .filter_map(|(e, c)| {
                let d = c
                    .edges()
                    .iter()
                    .filter_map(|(a, b)| {
                        let (a, b) = (project(*a)?, project(*b)?);
                        let ab = b - a;
                        let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
                        Some((a + ab * t).distance(p))
                    })
                    .fold(f32::MAX, f32::min);
                (d < 6.0).then_some((e.id, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }
}

/// The convex hull of `p`.
fn hull(p: &[egui::Pos2]) -> Vec<egui::Pos2> {
    let mut v: Vec<egui::Pos2> = p.to_vec();
    v.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    let cross =
        |o: egui::Pos2, a: egui::Pos2, b: egui::Pos2| (a - o).x * (b - o).y - (a - o).y * (b - o).x;
    let mut h: Vec<egui::Pos2> = vec![];
    for pass in 0..2 {
        let start = h.len();
        let it: Vec<egui::Pos2> = if pass == 0 {
            v.clone()
        } else {
            v.iter().rev().copied().collect()
        };
        for q in it {
            while h.len() >= start + 2 && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 {
                h.pop();
            }
            h.push(q);
        }
        h.pop();
    }
    h
}

fn e_name(cut_name: &str) -> &str {
    cut_name.trim_end_matches(" cut")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In a crop box's card, Select and Delete stand in from its two edges
    /// alike (MU145, owner: "the select button isn't fitting well with its
    /// left padding": it sat on the card's left edge, Delete 6 px in from
    /// its right).
    #[test]
    fn the_crop_cards_buttons_stand_in_from_both_edges_alike() {
        let mut app = CreaBimApp::blank(None);
        app.load_model(sample_house());
        app.add_crop_box(false, false);
        let ctx = egui::Context::default();
        let mut card_fill = egui::Color32::TRANSPARENT;
        let mut shapes = vec![];
        // the folds open over a few frames
        for k in 0..30 {
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(260.0, 700.0),
                )),
                time: Some(k as f64 * 0.05),
                ..Default::default()
            };
            let mut out = ctx.run_ui(raw, |ui| {
                card_fill = ui.visuals().extreme_bg_color;
                app.crop_list(ui, None)
            });
            out.textures_delta.clear();
            shapes = out.shapes;
        }
        fn rects(s: &egui::Shape, out: &mut Vec<(egui::Rect, egui::Color32)>) {
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| rects(s, out)),
                egui::Shape::Rect(r) => out.push((r.rect, r.fill)),
                _ => {}
            }
        }
        let mut all = vec![];
        for c in &shapes {
            rects(&c.shape, &mut all);
        }
        let cards: Vec<egui::Rect> = all
            .iter()
            .filter(|(_, f)| *f == card_fill)
            .map(|(r, _)| *r)
            .collect();
        // the two buttons: 24 px high, side by side in a card
        let pair: Vec<egui::Rect> = all
            .iter()
            .map(|(r, _)| *r)
            .filter(|r| (r.height() - 24.0).abs() < 1.0)
            .filter(|r| {
                cards
                    .iter()
                    .any(|c| c.contains_rect(*r) && r.width() < c.width() * 0.7)
            })
            .collect();
        assert!(
            pair.len() >= 2,
            "Select and Delete drawn in the card: {pair:?} in {cards:?}"
        );
        let card = cards
            .iter()
            .filter(|c| pair.iter().all(|r| c.contains_rect(*r)))
            .min_by(|a, b| a.area().total_cmp(&b.area()))
            .expect("one card holds both");
        let left = pair.iter().map(|r| r.left()).fold(f32::INFINITY, f32::min) - card.left();
        let right = card.right()
            - pair
                .iter()
                .map(|r| r.right())
                .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            left >= 4.0 && (left - right).abs() < 0.5,
            "the buttons stand {left} px in from the card's left edge and {right} px from its right"
        );
    }

    #[test]
    fn a_crop_frame_is_hidden_by_what_stands_in_front_of_it() {
        // MU143: the frames were painted over the view, so a far edge of
        // the box crossed the piers and footings in front of it. They go
        // to the 3D window's lines, with the model's depth; only the box
        // being edited is drawn over the view, whole
        let mut app = CreaBimApp::blank(None);
        app.load_model(sample_house());
        app.add_crop_box(false, false);
        let id = app.selection.active.unwrap();
        let c = app.crop_box(id).unwrap().1.clone();
        app.selection.clear();
        app.crop_pick = None;
        assert!(
            app.crop_frames_on_top().is_empty(),
            "nothing painted over the model"
        );
        let lines = app.crop_frame_lines();
        assert_eq!(lines.len(), 12, "its twelve edges, with the model's depth");
        for (a, b) in c.edges() {
            assert!(lines.iter().any(|l| l.0 == a && l.1 == b));
        }
        // being edited, it is drawn over the view, whole
        app.crop_pick = Some(id);
        assert_eq!(app.crop_frames_on_top(), vec![id]);
        app.crop_pick = None;
        // switched off, dashed and grey; hidden, not drawn
        app.edit_crop(id, "off", |c| c.enabled = false);
        let off = app.crop_frame_lines();
        assert!(off.len() > 12, "dashed");
        assert!(
            off.iter()
                .all(|l| l.2 == off[0].2 && l.2[0] == l.2[1] && l.2[1] == l.2[2]),
            "grey"
        );
        // the 3D window builds again when a frame changes, even one
        // switched off: the frame is in its buffers
        let before = app.geometry_key_at(0);
        app.edit_crop(id, "pull", |c| c.max.z -= 1.0);
        assert_ne!(app.geometry_key_at(0), before);
        app.edit_crop(id, "hide", |c| c.frame_hidden = true);
        assert!(app.crop_frame_lines().is_empty());
    }

    #[test]
    fn crop_boxes_are_added_toggled_and_undone_from_the_app() {
        let mut app = CreaBimApp::blank(None);
        app.load_model(sample_house());
        let n = app.doc.model().len();
        app.add_crop_box(false, false);
        assert_eq!(app.doc.model().len(), n + 1);
        let id = app.selection.active.unwrap();
        let (e, c) = app.crop_box(id).unwrap();
        assert_eq!(e.name, "Crop box 1");
        assert_eq!(c.view, None, "the 3D window's own");
        assert!(c.enabled && c.scope == CropScope::All);
        // the 3D window's scene is cropped once the box is pulled in
        app.edit_crop(id, "Crop box extents", |c| c.max.z = c.min.z + 1.5);
        app.scene = (*creabim_geom::shared_scene(app.doc.model())).clone();
        let edges = app.crop_scene_3d();
        assert!(!edges.is_empty(), "the cut walls show their cut edges");
        let top = app
            .scene
            .iter()
            .filter_map(|s| s.mesh.bounds())
            .map(|b| b.1.z as f64)
            .fold(f64::MIN, f64::max);
        let (_, c) = app.crop_box(id).unwrap();
        assert!(top <= c.max.z + 1e-4, "nothing above the box: {top}");
        // switched off it crops nothing
        assert!(app.crop_action("crop.toggle", &id.0.to_string()));
        app.scene = (*creabim_geom::shared_scene(app.doc.model())).clone();
        assert!(app.crop_scene_3d().is_empty());
        assert!(app.doc.undo());
        assert!(app.crop_box(id).unwrap().1.enabled);
        // a box for the selection only, and a 3D view's own boxes
        let wall = app
            .doc
            .model()
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Wall(_)))
            .unwrap()
            .id;
        app.selection.only(wall);
        app.add_crop_box(true, true);
        let only = app.selection.active.unwrap();
        assert_eq!(
            app.crop_box(only).unwrap().1.scope,
            CropScope::Only(vec![wall])
        );
        let view = app
            .doc
            .model()
            .iter()
            .find(|e| matches!(&e.kind, ElementKind::View(v) if matches!(v.kind, ViewKind::Axonometric { .. })))
            .unwrap()
            .id;
        app.selection.clear();
        app.active_view = Some(view);
        assert_eq!(app.crop_owner(), Some(view));
        app.add_crop_box(false, false);
        let mine = app.selection.active.unwrap();
        assert_eq!(app.crop_box(mine).unwrap().1.view, Some(view));
        assert_eq!(app.crop_box(mine).unwrap().0.name, "Crop box 1");
    }

    #[test]
    fn a_crop_face_dragged_recrops_3d_without_building_the_scene_again() {
        let mut app = CreaBimApp::blank(None);
        app.load_model(sample_house());
        app.add_crop_box(false, false);
        let id = app.selection.active.unwrap();
        let built = app.doc.model().scene_revision();
        let before = app.geometry_key_at(built);
        // one step of a face drag
        app.edit_crop(id, "Crop face", |c| c.max.x -= 1.0);
        assert_eq!(
            app.doc.model().scene_revision(),
            built,
            "the joined scene is not built again for it (0.3–0.5 s a step)"
        );
        assert_ne!(
            app.geometry_key_at(built),
            before,
            "but the 3D view cuts its scene again with the moved box"
        );
        // and a box switched off shows the whole scene again
        let on = app.geometry_key_at(built);
        assert!(app.crop_action("crop.toggle", &id.0.to_string()));
        assert_ne!(app.geometry_key_at(built), on);
    }
}
