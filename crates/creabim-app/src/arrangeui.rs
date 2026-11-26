//! A window's Arrangement section in Properties (doors and windows part
//! 9): the ribbon or corner it is in, a window added beside it, the corner
//! turned at either end, the joint where two meet, and back on its own.
//! Every change is one transaction, so it undoes like any edit.

use super::*;
use creabim_core::{arrangement, ArrangementKind, CornerJoint, OpeningKind};

impl CreaBimApp {
    pub(super) fn arrangement_section(&mut self, ui: &mut egui::Ui, id: ElementId) {
        let Some(ElementKind::Opening(o)) = self.doc.model().get(id).map(|e| &e.kind) else {
            return;
        };
        if o.kind != OpeningKind::Window || o.roof_at.is_some() {
            return;
        }
        let o = o.clone();
        let model = self.doc.model();
        let corner_wall = |end| arrangement::wall_at_corner(model, o.host, end).is_some();
        let (at_start, at_end) = (corner_wall(false), corner_wall(true));
        let open = self.prop_header(
            ui,
            "element.arrange",
            "Arrangement",
            true,
            Some("doors-windows#arrangements"),
        );
        let mut act: Option<Result<creabim_core::Transaction, String>> = None;
        let mut done = "";
        crate::props::fold(ui, open, |ui| {
            let h = ui.spacing().interact_size.y;
            crate::props::row(ui, "In", |ui| {
                let text = o
                    .arrangement
                    .map_or_else(|| "On its own".to_string(), |a| a.label());
                ui.label(text);
            });
            let kind = o.arrangement.map(|a| a.kind);
            // side by side, in this wall
            if matches!(kind, None | Some(ArrangementKind::Ribbon)) {
                crate::props::row(ui, "Ribbon", |ui| {
                    let b = egui::Button::new("Add beside").truncate();
                    if ui
                        .add_sized([ui.available_width(), h], b)
                        .on_hover_text(
                            "The same window, frame to frame beside this one, coupled to it",
                        )
                        .clicked()
                    {
                        act = Some(arrangement::add_beside(self.doc.model(), id));
                        done = "Added a window to the ribbon. Ctrl+Z to undo.";
                    }
                });
            }
            // how windows meet: round a corner, in a bay or a bow
            if kind != Some(ArrangementKind::Ribbon) {
                let joint_id = egui::Id::new("arrange.joint");
                let mut joint = o.arrangement.map_or_else(
                    || {
                        ui.ctx()
                            .data(|d| d.get_temp(joint_id))
                            .unwrap_or(CornerJoint::Post)
                    },
                    |a| a.joint,
                );
                let before = joint;
                crate::props::choice(
                    ui,
                    "Joint",
                    &mut joint,
                    &[
                        (
                            CornerJoint::Post,
                            "Post",
                            "a corner post between the two frames",
                        ),
                        (
                            CornerJoint::Glass,
                            "Glass",
                            "glass to glass, no post: the panes meet at a sealed joint",
                        ),
                    ],
                );
                if joint != before {
                    if o.arrangement.is_some() {
                        act = Some(Ok(arrangement::set_joint(self.doc.model(), id, joint)));
                        done = "Changed the corner joint. Ctrl+Z to undo.";
                    } else {
                        ui.ctx().data_mut(|d| d.insert_temp(joint_id, joint));
                    }
                }
                let turned = |end| o.arrangement.is_some_and(|a| a.corner_at(end));
                if matches!(kind, None | Some(ArrangementKind::Corner)) {
                    crate::props::row(ui, "Corner", |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let w = ((ui.available_width() - 4.0) / 2.0).max(24.0);
                        for (end, text, there) in
                            [(false, "Start", at_start), (true, "End", at_end)]
                        {
                            let can = there && !turned(end);
                            let tip = if turned(end) {
                                "It turns this corner already"
                            } else if !there {
                                "No wall meets this one at that end"
                            } else if end {
                                "Take it to the corner at the wall's end, and the same window round it"
                            } else {
                                "Take it to the corner at the wall's start, and the same window round it"
                            };
                            let r = ui
                                .add_enabled(
                                    can,
                                    egui::Button::new(text)
                                        .truncate()
                                        .min_size(egui::vec2(w, h)),
                                )
                                .on_hover_text(tip)
                                .on_disabled_hover_text(tip);
                            if r.clicked() {
                                act = Some(arrangement::add_round_corner(
                                    self.doc.model(),
                                    id,
                                    end,
                                    joint,
                                ));
                                done = "Turned the window round the corner. Ctrl+Z to undo.";
                            }
                        }
                    });
                }
                // a bay or a bow, out from the wall (part 9b)
                if kind.is_none() {
                    let depth_id = egui::Id::new("arrange.depth");
                    let mut depth: f64 = ui.ctx().data(|d| d.get_temp(depth_id)).unwrap_or(0.6);
                    let before = depth;
                    crate::props::row(ui, "Depth", |ui| {
                        crate::props::fill(
                            ui,
                            egui::DragValue::new(&mut depth)
                                .range(0.2..=3.0)
                                .speed(0.01)
                                .custom_formatter(|v, _| format!("{:.0} mm", v * 1000.0))
                                .custom_parser(|t| {
                                    t.trim()
                                        .trim_end_matches("mm")
                                        .trim()
                                        .parse::<f64>()
                                        .ok()
                                        .map(|v| v / 1000.0)
                                }),
                        )
                        .on_hover_text(
                            "How far the bay comes out from the wall, to its walls' middle",
                        )
                    });
                    if depth != before {
                        ui.ctx().data_mut(|d| d.insert_temp(depth_id, depth));
                    }
                    // one menu: three buttons cut each other off at the
                    // narrowest panel
                    let mut make = None;
                    crate::app::helpui::combo(ui, "Bay")
                        .selected_text("Make one…")
                        .show_ui(ui, |ui| {
                            for (shape, text, tip) in [
                                (
                                    arrangement::BayShape::Box,
                                    "Box bay",
                                    "Square sides, a window in each",
                                ),
                                (
                                    arrangement::BayShape::Angled,
                                    "Angled bay",
                                    "Sides at 45 degrees, a window in each",
                                ),
                                (
                                    arrangement::BayShape::Bow(5),
                                    "Bow window",
                                    "A curve of five facets, a window in each",
                                ),
                            ] {
                                if ui
                                    .selectable_label(false, text)
                                    .on_hover_text(tip)
                                    .clicked()
                                {
                                    make = Some(shape);
                                }
                            }
                        });
                    if let Some(shape) = make {
                        act = Some(arrangement::make_bay(
                            self.doc.model(),
                            id,
                            shape,
                            depth,
                            joint,
                        ));
                        done = "Made the bay: the wall opens over the window and a window stands in each side. Ctrl+Z to undo.";
                    }
                }
            }
            if o.arrangement.is_some() {
                crate::props::row(ui, "Group", |ui| {
                    if ui
                        .add_sized(
                            [ui.available_width(), h],
                            egui::Button::new("Leave").truncate(),
                        )
                        .on_hover_text("On its own again, where it stands")
                        .clicked()
                    {
                        act = Some(Ok(arrangement::leave(self.doc.model(), id)));
                        done = "Took the window out of its group. Ctrl+Z to undo.";
                    }
                });
            }
        });
        if let Some(r) = act {
            let r = r.and_then(|tx| self.doc.commit(tx).map(|_| ()).map_err(|e| e.to_string()));
            self.report(r, done);
        }
    }
}
