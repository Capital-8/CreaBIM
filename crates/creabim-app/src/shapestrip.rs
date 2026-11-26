//! The strip beside what is selected (MU43): the modelling tools that
//! apply to the selection, at the top right of the view being worked in.
//! It shows with the Select tool while nothing else is under way, and
//! lists only what works. The right-click menu carries the same tools.

use super::actionbar::Act;
use super::*;
use crate::commands::Command;
use creabim_core::pushpull;

/// A row of the strip: this wide, this high.
const WIDTH: f32 = 148.0;
const ROW: f32 = 24.0;
/// The strip folded to its icons, in a view too narrow for the names.
const NARROW: f32 = 26.0;

impl CreaBimApp {
    /// What the strip lists for the selection: its heading and its tools.
    /// None when it does not show.
    /// The edge picked in Edit mode on the selected outline, if what is
    /// picked is one edge.
    fn picked_edge(&self) -> Option<(ElementId, handles::Ring, usize)> {
        let id = self
            .plan
            .edit
            .filter(|id| self.selection.active == Some(*id))?;
        let model = self.doc.model();
        let e = model.get(id)?;
        let (ring, i) = self
            .plan
            .grips
            .one_edge(|ring| editmode::ring_of(model, e, ring).map_or(0, |r| r.corners.len()))?;
        Some((id, ring, i))
    }

    /// In Edit mode, what the strip does with the picked edge (they were
    /// a small menu under the pointer, in the way of dragging the edge:
    /// MU46).
    fn edge_acts(&self) -> Vec<Act> {
        if self.plan.edit.is_none() {
            return vec![];
        }
        let on = self.picked_edge().is_some();
        vec![
            Act::new("strip:offset-edge", "Offset edge", "offset")
                .hint("Push the picked edge out or pull it in, square to itself. Click an edge first")
                .enabled(on),
            Act::new("strip:insert-point", "Insert point", "diamond")
                .hint("A corner in the middle of the picked edge, then moved with the pointer. Click an edge first")
                .enabled(on),
        ]
    }

    pub(super) fn strip_actions(&self) -> Option<(String, Vec<Act>)> {
        if self.plan.tool != Tool::Select
            || self.plan.modal.is_some()
            || self.plan.armed.is_some()
            || self.plan.busy()
        {
            return None;
        }
        let ids = self.selection.all();
        let model = self.doc.model();
        let kinds: Vec<&ElementKind> = ids
            .iter()
            .filter_map(|i| model.get(*i))
            .map(|e| &e.kind)
            .collect();
        if kinds.is_empty() || kinds.len() != ids.len() {
            return None;
        }
        let pull = || {
            Act::new("strip:pull", "Pull", "pull")
                .key("P")
                .hint("Point at a face, click, then move out or in or type a distance")
        };
        let mv = || {
            Act::new("strip:move", "Move", "move")
                .key("M")
                .hint("By a base point, then where it goes")
        };
        let offset = || {
            Act::new("strip:offset", "Offset", "offset")
                .hint("A parallel copy, to the side the pointer is on")
        };
        let editing = self.plan.edit.is_some();
        let edit = || Act {
            on: editing,
            ..Act::new("strip:edit", "Edit", "edit-outline")
                .key("Tab")
                .hint("Edit mode: its points, edges and faces. Esc finishes")
        };
        // a shape and what it works on: Add, Subtract, Intersect, Reach
        if let Some((shape, targets)) = self.solid_op_pair() {
            let n = targets.len();
            return Some((
                format!("A shape and {n} {}", plural("element", n)),
                self.solid_op_acts(shape, &targets),
            ));
        }
        if kinds.len() > 1 {
            // several: only shapes have something of their own, Join
            if !kinds.iter().all(|k| matches!(k, ElementKind::FreeMesh(_))) {
                return None;
            }
            let join = Act::new("strip:join", "Join", "shape-join")
                .hint("The selected shapes become one, each a piece of it");
            return Some((format!("{} shapes", kinds.len()), vec![join, mv()]));
        }
        Some(match kinds[0] {
            ElementKind::Sketch(s) if s.closed => {
                // named for what a click does (MU47)
                let face = if s.wire {
                    Act::new("strip:face", "Make face", "shape-face")
                        .hint("Fill the outline: a face, which Pull makes a solid")
                } else {
                    Act::new("strip:face", "Remove face", "shape-wire").hint(
                        "Keep the lines only: a closed wire, which Pull makes walls, open at both ends",
                    )
                };
                let head = if s.wire {
                    "A closed wire"
                } else {
                    "A closed outline"
                };
                let mut acts = vec![pull(), offset(), edit(), face];
                acts.extend(self.edge_acts());
                (head.into(), acts)
            }
            ElementKind::Sketch(s) => (
                "An open line".into(),
                vec![
                    pull(),
                    offset(),
                    edit(),
                    Act::new("strip:close", "Close it", "shape-close")
                        .hint(
                            "Join its last point to its first: a closed outline pulls into a solid",
                        )
                        .enabled(s.can_close()),
                ]
                .into_iter()
                .chain(self.edge_acts())
                .collect(),
            ),
            ElementKind::FreeMesh(m) => {
                let face = self.face.filter(|f| *f < m.mesh.faces.len());
                let picked = self.plan.grips.mesh.len();
                match (face, editing && picked > 0) {
                    (Some(_), false) => (
                        "A face".into(),
                        vec![
                            pull(),
                            Act::new("strip:inset", "Inset", "shape-inset").key("I").hint(
                                "A smaller face inside its edges: move the pointer or type the thickness, Ctrl sets a depth",
                            ),
                            mv(),
                        ],
                    ),
                    (_, true) => (
                        match picked {
                            1 => "1 point".into(),
                            n => format!("{n} points"),
                        },
                        vec![
                            mv(),
                            Act::new("strip:linked", "Whole piece", "shape-piece")
                                .key("L")
                                .hint("Grow what is picked to the whole piece it belongs to"),
                            edit(),
                        ],
                    ),
                    _ => {
                        let mut acts = vec![pull(), mv(), edit()];
                        if m.mesh.pieces().len() > 1 {
                            acts.push(
                                Act::new("strip:separate", "Separate", "shape-separate")
                                    .hint("Each piece becomes a shape of its own"),
                            );
                        }
                        ("A shape".into(), acts)
                    }
                }
            }
            k if pushpull::can_pull(k) => (format!("A {}", kind_word(k)), vec![pull(), mv()]),
            _ => return None,
        })
    }

    /// Draw the strip at the top right of `rect`, `inset` in from its
    /// right edge (clear of the view's own buttons), and run what is
    /// clicked on it.
    pub(super) fn shape_strip(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        inset: f32,
        pane: egui::Id,
    ) {
        // in the view being worked in only
        if ui.rect_contains_pointer(rect) {
            self.strip_pane = Some(pane);
        }
        if self.strip_pane.is_some_and(|p| p != pane) {
            return;
        }
        let Some((head, acts)) = self.strip_actions() else {
            return;
        };
        // the whole strip where there is room; in a narrow view its icons
        // in a column, in a short one its icons in a row, each named when
        // pointed at; smaller still, nothing, and the right-click menu has
        // the tools (MU47, MU49: a short view showed none)
        let n = acts.len() as f32;
        let full = egui::vec2(WIDTH, 20.0 + n * ROW + 10.0);
        let column = egui::vec2(NARROW, n * ROW + 10.0);
        let row = egui::vec2(n * NARROW, ROW + 10.0);
        let fits = |size: egui::Vec2, room: f32| {
            rect.width() >= size.x + inset + room && rect.height() >= size.y + 80.0
        };
        let (narrow, across) = if fits(full, 220.0) {
            (false, false)
        } else if fits(column, 160.0) {
            (true, false)
        } else if rect.width() >= row.x + inset + 120.0 && rect.height() >= row.y + 40.0 {
            (true, true)
        } else {
            return;
        };
        let width = if narrow { NARROW } else { WIDTH };
        let wide = if across { row.x } else { width };
        let pos = egui::pos2(rect.right() - inset - wide - 12.0, rect.top() + 10.0);
        let mut clicked: Option<String> = None;
        egui::Area::new(pane.with("shape strip"))
            .order(crate::app::overlay_order())
            .fixed_pos(pos)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::same(5))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 1.0);
                        ui.set_width(wide);
                        if !narrow {
                            let weak = ui.visuals().weak_text_color();
                            ui.label(
                                egui::RichText::new(head.to_uppercase())
                                    .size(9.5)
                                    .color(weak),
                            );
                        }
                        let mut rows = |ui: &mut egui::Ui| {
                            for a in &acts {
                                if strip_row(ui, a, width) {
                                    clicked = Some(a.id.clone());
                                }
                            }
                        };
                        if across {
                            ui.horizontal(rows);
                        } else {
                            rows(ui);
                        }
                    });
            });
        if let Some(id) = clicked {
            let ctx = ui.ctx().clone();
            self.run_strip_action(&ctx, &id);
        }
    }

    /// The strip's own tools for the right-click menu (Pull, Move and
    /// Offset are there already as commands).
    pub(super) fn strip_menu_items(&self) -> Vec<crate::ui_model::Item> {
        use crate::ui_model::Item;
        let Some((_, acts)) = self.strip_actions() else {
            return vec![];
        };
        let mut items: Vec<Item> = acts
            .into_iter()
            .filter(|a| !matches!(a.id.as_str(), "strip:pull" | "strip:move" | "strip:offset"))
            .filter(|a| !a.id.starts_with("head:"))
            .map(|a| {
                Item::new(a.id.clone(), a.label.clone())
                    .icon(a.icon)
                    .key(a.key)
                    .hint(a.hint.clone())
                    .enabled(a.enabled)
            })
            .collect();
        if !items.is_empty() {
            items.push(Item::Separator);
        }
        items
    }

    /// Run a tool of the strip, by its id.
    pub(super) fn run_strip_action(&mut self, ctx: &egui::Context, id: &str) {
        let active = self.selection.active;
        match id {
            "strip:pull" => self.run_command(ctx, Command::Pull, None),
            "strip:move" => self.run_command(ctx, Command::Move, None),
            "strip:offset" => self.run_command(ctx, Command::Offset, None),
            "strip:edit" => self.toggle_edit_mode(),
            "strip:linked" => {
                if let Some(s) = crate::handles::pick_linked(&self.doc, &mut self.plan.grips) {
                    self.status = s;
                }
            }
            s if s.starts_with("solid:") => self.run_solid_op(s),
            "strip:separate" => {
                if let Some(id) = active {
                    self.separate_shape(id);
                }
            }
            "strip:join" => {
                if let Some(id) = active {
                    self.join_shapes(id);
                }
            }
            "strip:offset-edge" => {
                if let Some((_, ring, i)) = self.picked_edge() {
                    self.plan.grips.request = Some(handles::GripRequest::OffsetEdge(ring, i));
                    // typed distances go to the edit that starts
                    crate::modal::unfocus_soon(ctx);
                }
            }
            "strip:insert-point" => {
                if let Some((id, ring, i)) = self.picked_edge() {
                    let model = self.doc.model();
                    let mid = model
                        .get(id)
                        .and_then(|e| editmode::ring_of(model, e, ring))
                        .map(|r| {
                            let n = r.corners.len();
                            (r.corners[i % n] + r.corners[(i + 1) % n]) / 2.0
                        });
                    if let Some(mid) = mid {
                        self.plan.grips.request =
                            Some(handles::GripRequest::InsertPoint(ring, i, mid));
                    }
                }
            }
            "strip:face" => {
                let Some(id) = active else {
                    return;
                };
                let mut wire = false;
                let r = self.doc.edit(id, "Face of an outline", |e| {
                    if let ElementKind::Sketch(s) = &mut e.kind {
                        s.wire = !s.wire;
                        wire = s.wire;
                    }
                });
                self.report(
                    r,
                    if wire {
                        "Face removed: a closed wire. Pull makes it walls, open at both ends."
                    } else {
                        "Face made: Pull makes it a solid."
                    },
                );
            }
            "strip:close" => {
                let Some(id) = active else {
                    return;
                };
                let r = self.doc.edit(id, "Close outline", |e| {
                    if let ElementKind::Sketch(s) = &mut e.kind {
                        if s.can_close() {
                            s.set_closed(true);
                        }
                    }
                });
                self.report(r, "Closed: Pull makes it a solid.");
            }
            "strip:inset" => self.start_inset(),
            _ => {}
        }
    }

    /// Inset, following the pointer (MU49): on the face clicked, or on
    /// the faces picked whole in Edit mode, each on its own.
    pub(super) fn start_inset(&mut self) {
        let Some(id) = self.selection.active.filter(|_| self.selection.len() == 1) else {
            self.status = "Inset: select one shape and pick a face of it first.".into();
            return;
        };
        let mut faces = handles::whole_faces(self.doc.model(), id);
        if let Some(f) = self.face {
            if !faces.contains(&f) {
                faces = vec![f];
            }
        }
        if faces.is_empty() {
            self.status =
                "Inset: pick a face of the shape first (a click on it; Shift+click for more in Edit mode)."
                    .into();
            return;
        }
        handles::set_inset_start(faces);
        self.plan.armed = Some((crate::modal::Mode::Inset, false));
        self.status =
            "Inset: move over the face to set the thickness, or type it. Ctrl and move sets a depth."
                .into();
    }

    /// Separate: each piece of shape `id` becomes a shape of its own.
    pub(super) fn separate_shape(&mut self, id: ElementId) {
        let Some((tx, ids)) = creabim_core::shape::separate(self.doc.model(), id) else {
            self.status = "It is one piece: there is nothing to separate.".into();
            return;
        };
        let n = ids.len();
        let r = self.doc.commit(tx);
        if r.is_ok() {
            self.face = None;
            self.plan.edit = None;
            self.selection.clear();
            self.selection.extend(ids, false);
        }
        self.report(r, format!("Separated into {n} shapes, all selected."));
    }

    /// Join: the selected shapes become one. `keep` keeps its name and
    /// settings.
    pub(super) fn join_shapes(&mut self, keep: ElementId) {
        let mut ids = vec![keep];
        ids.extend(self.selection.all().into_iter().filter(|i| *i != keep));
        let applied = ids.iter().any(|i| {
            matches!(self.doc.model().get(*i).map(|e| &e.kind),
                Some(ElementKind::FreeMesh(f)) if !f.modifiers.is_empty())
        });
        match creabim_core::shape::join(self.doc.model(), &ids) {
            Ok((tx, kept)) => {
                let r = self.doc.commit(tx);
                if r.is_ok() {
                    self.face = None;
                    self.selection.only(kept);
                }
                let note = if applied {
                    " Their modifiers were applied first."
                } else {
                    ""
                };
                self.report(r, format!("Joined {} shapes into one.{note}", ids.len()));
            }
            Err(e) => self.status = e,
        }
    }
}

/// One row of the strip: its icon, its name, its key at the right.
fn strip_row(ui: &mut egui::Ui, a: &Act, width: f32) -> bool {
    let narrow = width < WIDTH;
    // a heading inside the strip (Reach), not a tool
    if a.id.starts_with("head:") {
        let (r, _) = ui.allocate_exact_size(
            egui::vec2(width, if narrow { 6.0 } else { 18.0 }),
            egui::Sense::hover(),
        );
        let v = ui.visuals();
        if narrow {
            ui.painter().hline(
                r.x_range().shrink(4.0),
                r.center().y,
                v.widgets.noninteractive.bg_stroke,
            );
        } else {
            ui.painter().text(
                egui::pos2(r.left() + 4.0, r.bottom() - 3.0),
                egui::Align2::LEFT_BOTTOM,
                a.label.to_uppercase(),
                egui::FontId::proportional(9.5),
                v.weak_text_color(),
            );
        }
        return false;
    }
    let (r, resp) = ui.allocate_exact_size(
        egui::vec2(width, ROW),
        if a.enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    let v = ui.visuals();
    let mut ink = v.text_color();
    if !a.enabled {
        ink = ink.gamma_multiply(0.4);
    }
    if a.on {
        ui.painter()
            .rect_filled(r, 5.0, v.selection.bg_fill.gamma_multiply(0.45));
    } else if resp.hovered() && a.enabled {
        ui.painter()
            .rect_filled(r, 5.0, v.widgets.hovered.weak_bg_fill);
    }
    let icon = egui::Rect::from_center_size(
        egui::pos2(r.left() + 13.0, r.center().y),
        egui::vec2(16.0, 16.0),
    );
    crate::svgicon::paint(ui.painter(), icon, a.icon, ink);
    if narrow {
        // folded: the name comes with the hint
        return resp
            .on_hover_text(a.tip())
            .on_disabled_hover_text(a.tip())
            .clicked();
    }
    ui.painter().text(
        egui::pos2(r.left() + 27.0, r.center().y),
        egui::Align2::LEFT_CENTER,
        &a.label,
        egui::FontId::proportional(12.5),
        ink,
    );
    if !a.key.is_empty() {
        ui.painter().text(
            egui::pos2(r.right() - 6.0, r.center().y),
            egui::Align2::RIGHT_CENTER,
            a.key,
            egui::FontId::monospace(10.0),
            v.weak_text_color(),
        );
    }
    resp.on_hover_text(a.tip()).clicked()
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_strip_tool_has_an_icon() {
        let js = include_str!("../ui/icons.js");
        for icon in [
            "pull",
            "move",
            "offset",
            "edit-outline",
            "shape-join",
            "shape-separate",
            "shape-close",
            "shape-face",
            "shape-wire",
            "shape-inset",
            "shape-piece",
            "shape-line",
            "shape-rect",
        ] {
            assert!(js.contains(&format!("\n\"{icon}\":")), "no icon {icon}");
        }
    }
}
