//! Placed on, for every sheet mark (Mockup_Copy_To_Sheets_REV00, A on
//! all): a tick per place. Ticking adds a copy there, unticking takes that
//! copy away. Copies are ordinary marks (moved and styled one by one),
//! listed together through `copy_of`. North arrows and scale bars are
//! listed by drawing (a scale bar measures one drawing), the rest by sheet.

use super::*;
use creabim_core::{PaperMark, PaperMarkKind};
use creabim_geom::sheet as gsheet;

/// What the Placed on fold asked for.
pub(super) enum MarkAct {
    /// Add a copy on this sheet, for this drawing.
    Add(Vec<(ElementId, Option<ElementId>)>),
    /// Take these copies away.
    Remove(Vec<ElementId>),
    /// Give every copy this one's look.
    SameOnAll,
}

/// One place a mark can go.
struct Place {
    sheet: ElementId,
    drawing: Option<ElementId>,
    label: String,
    meta: String,
    ok: bool,
}

/// Listed by drawing (north arrows by plan, scale bars by drawing with a
/// scale), or by sheet.
fn by_drawing(m: &PaperMark) -> bool {
    matches!(
        m.kind,
        PaperMarkKind::NorthArrow { .. } | PaperMarkKind::ScaleBar { .. }
    )
}

/// Whether a drawing can carry this mark.
fn fits(model: &creabim_core::Model, m: &PaperMark, vp: ElementId) -> bool {
    match m.kind {
        PaperMarkKind::NorthArrow { .. } => gsheet::is_plan(model, vp),
        _ => gsheet::drawing_scale(model, vp).is_some(),
    }
}

/// The drawing a mark belongs to: its own, else the nearest that fits.
fn drawing_of(model: &creabim_core::Model, m: &PaperMark) -> Option<ElementId> {
    if !by_drawing(m) {
        return None;
    }
    m.measures
        .filter(|v| model.get(*v).is_some())
        .or_else(|| gsheet::nearest_viewport(model, m.sheet, m.at, |v| fits(model, m, v)))
}

/// The mark and its copies: (id, mark).
fn family(
    model: &creabim_core::Model,
    id: ElementId,
    m: &PaperMark,
) -> Vec<(ElementId, PaperMark)> {
    let first = m.first(id);
    model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::PaperMark(p) if e.id == first || p.copy_of == Some(first) => {
                Some((e.id, p.clone()))
            }
            _ => None,
        })
        .collect()
}

fn places(model: &creabim_core::Model, m: &PaperMark) -> Vec<Place> {
    let mut out = vec![];
    for (sheet, s) in gsheet::sheets(model) {
        let sheet_name = format!(
            "{} {}",
            s.number,
            model.get(sheet).map(|e| e.name.as_str()).unwrap_or("")
        );
        if !by_drawing(m) {
            out.push(Place {
                sheet,
                drawing: None,
                label: sheet_name,
                meta: String::new(),
                ok: true,
            });
            continue;
        }
        let vps = gsheet::viewports_on(model, sheet);
        if vps.is_empty() {
            out.push(Place {
                sheet,
                drawing: None,
                label: sheet_name,
                meta: "no drawings".into(),
                ok: false,
            });
            continue;
        }
        for (vp, _, name) in vps {
            let ok = fits(model, m, vp);
            let meta = match (&m.kind, gsheet::drawing_scale(model, vp)) {
                (PaperMarkKind::NorthArrow { .. }, _) if ok => "plan".to_string(),
                (PaperMarkKind::NorthArrow { .. }, _) => "not a plan".to_string(),
                (_, Some(sc)) => creabim_core::standards::scale_label(model, sc),
                _ => "no scale".to_string(),
            };
            out.push(Place {
                sheet,
                drawing: Some(vp),
                label: format!("{} \u{b7} {name}", s.number),
                meta,
                ok,
            });
        }
    }
    out
}

impl CreaBimApp {
    /// The Placed on fold of a sheet mark.
    pub(super) fn mark_placed_fold(&mut self, ui: &mut egui::Ui, id: ElementId, m: &PaperMark) {
        let model = self.doc.model();
        let fam = family(model, id, m);
        let title = if fam.len() > 1 {
            format!("Placed on \u{b7} {}", fam.len())
        } else {
            "Placed on".to_string()
        };
        let open = self.prop_header(
            ui,
            "mark_placed",
            &title,
            true,
            Some(
                "say:A tick per place: ticking adds a copy there, unticking takes that copy away. \
                 Each copy is an ordinary mark: drag it, turn it, style it on its own. \
                 North arrows are listed by plan, scale bars by drawing, since each reads its own scale.",
            ),
        );
        let mut act: Option<MarkAct> = None;
        crate::props::fold(ui, open, |ui| {
            let model = self.doc.model();
            let list = places(model, m);
            // which place each copy is on
            let at: Vec<(ElementId, ElementId, Option<ElementId>)> = fam
                .iter()
                .map(|(i, p)| (*i, p.sheet, drawing_of(model, p)))
                .collect();
            let here = (m.sheet, drawing_of(model, m));
            let on = |p: &Place| {
                at.iter()
                    .find(|(_, s, d)| *s == p.sheet && *d == p.drawing)
                    .map(|x| x.0)
            };
            // what you do first sits first (owner: the buttons were under a
            // long list): Suggested or All, None, Same on all copies
            let free: Vec<(ElementId, Option<ElementId>)> = list
                .iter()
                .filter(|p| p.ok && (p.sheet, p.drawing) != here && on(p).is_none())
                .map(|p| (p.sheet, p.drawing))
                .collect();
            // two buttons sharing the row, each on one line: Suggested (by
            // drawing) or All (by sheet), and None
            let others: Vec<ElementId> = fam.iter().map(|(i, _)| *i).filter(|i| *i != id).collect();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let w = ((ui.available_width() - 4.0) / 2.0).max(40.0);
                let h = ui.spacing().interact_size.y;
                let (text, tip) = if !by_drawing(m) {
                    ("All", "A copy on every sheet")
                } else if matches!(m.kind, PaperMarkKind::NorthArrow { .. }) {
                    ("Suggested", "A copy on every plan")
                } else {
                    (
                        "Suggested",
                        "A copy under every plan, section, elevation and detail",
                    )
                };
                if ui
                    .add_enabled_ui(!free.is_empty(), |ui| {
                        ui.add_sized([w, h], egui::Button::new(text))
                    })
                    .inner
                    .on_hover_text(tip)
                    .clicked()
                {
                    act = Some(MarkAct::Add(free.clone()));
                }
                if ui
                    .add_enabled_ui(!others.is_empty(), |ui| {
                        ui.add_sized([w, h], egui::Button::new("None"))
                    })
                    .inner
                    .on_hover_text("Take every copy away but this one")
                    .clicked()
                {
                    act = Some(MarkAct::Remove(others.clone()));
                }
            });
            if fam.len() > 1 {
                // the row's full width, as Suggested and None together
                // (owner: it sat cramped behind an empty label)
                let h = ui.spacing().interact_size.y;
                let r = ui
                    .add_sized(
                        [ui.available_width(), h],
                        egui::Button::new("Same on all copies"),
                    )
                    .on_hover_text(
                        "Give every copy this one's style, size and colour; where each sits stays",
                    );
                if r.clicked() {
                    act = Some(MarkAct::SameOnAll);
                }
            }
            ui.add_space(4.0);
            // its scroll kept under a fixed key: while the fold opens or
            // shuts its body sits in another Ui, whose id would start the
            // list at the top and jump back (owner: a flicker after
            // scrolling to the end, then folding and opening)
            let key = egui::Id::new(("mark_placed.scroll", id));
            let off: f32 = ui.ctx().data(|d| d.get_temp(key)).unwrap_or(0.0);
            let out = egui::ScrollArea::vertical()
                .id_salt(("mark_placed", id))
                .max_height(260.0)
                .vertical_scroll_offset(off)
                .show(ui, |ui| {
                    for p in &list {
                        let mine = (p.sheet, p.drawing) == here;
                        let copy = on(p);
                        let mut ticked = mine || copy.is_some();
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            ui.add_enabled_ui(p.ok && !mine, |ui| {
                                let tip = if mine {
                                    "This one: delete it as usual to take it away"
                                } else if !p.ok {
                                    "It can't carry this mark"
                                } else if ticked {
                                    "Untick to take this copy away"
                                } else {
                                    "Tick to put a copy here"
                                };
                                if crate::props::tickbox(ui, &mut ticked, "")
                                    .on_hover_text(tip)
                                    .changed()
                                {
                                    act = Some(if ticked {
                                        MarkAct::Add(vec![(p.sheet, p.drawing)])
                                    } else {
                                        MarkAct::Remove(copy.into_iter().collect())
                                    });
                                }
                            });
                            // the meta at the row's end on one line; the
                            // name cut to the room left
                            // the bold name says which is this one
                            let meta = p.meta.clone();
                            let meta = egui::RichText::new(meta).small().weak();
                            let mw = egui::WidgetText::from(meta.clone())
                                .into_galley(
                                    ui,
                                    Some(egui::TextWrapMode::Extend),
                                    f32::INFINITY,
                                    egui::TextStyle::Small,
                                )
                                .size()
                                .x;
                            let rich = egui::RichText::new(&p.label);
                            let rich = if mine {
                                rich.strong()
                            } else if p.ok {
                                rich
                            } else {
                                rich.weak()
                            };
                            let h = ui.spacing().interact_size.y;
                            let w = (ui.available_width() - mw - 8.0).max(30.0);
                            ui.allocate_ui(egui::vec2(w, h), |ui| {
                                ui.set_width(w);
                                ui.add(egui::Label::new(rich).truncate())
                                    .on_hover_text(&p.label);
                            });
                            ui.add(egui::Label::new(meta).extend());
                        });
                    }
                });
            ui.ctx()
                .data_mut(|d| d.insert_temp(key, out.state.offset.y));
        });
        if let Some(a) = act {
            self.mark_placed_act(id, a);
        }
    }

    fn mark_placed_act(&mut self, id: ElementId, act: MarkAct) {
        let model = self.doc.model();
        let Some(me) = model.get(id).cloned() else {
            return;
        };
        let ElementKind::PaperMark(m) = me.kind.clone() else {
            return;
        };
        let m = &m;
        let fam = family(model, id, m);
        let first = m.first(id);
        let (tx, said) = match act {
            MarkAct::Add(to) => {
                let src_vp = drawing_of(model, m);
                let centre =
                    |vp: Option<ElementId>| match vp.and_then(|v| model.get(v)).map(|e| &e.kind) {
                        Some(ElementKind::Viewport(v)) => Some(v.center),
                        _ => None,
                    };
                let mut tx = Transaction::new("Place on sheets");
                for (sheet, drawing) in &to {
                    let mut c = m.clone();
                    c.sheet = *sheet;
                    c.copy_of = Some(first);
                    c.measures = *drawing;
                    // under its drawing's title (a north arrow by its top
                    // right corner); else the same place on the sheet
                    let north = matches!(m.kind, PaperMarkKind::NorthArrow { .. });
                    if let Some(at) =
                        drawing.and_then(|d| gsheet::mark_spot(model, d, north, m.size))
                    {
                        c.at = at;
                    } else if let (Some(a), Some(b)) = (centre(src_vp), centre(*drawing)) {
                        c.at = b + (m.at - a);
                    }
                    if let Some(s) = drawing.and_then(|d| gsheet::drawing_scale(model, d)) {
                        c = c.at_scale(s);
                    }
                    tx = tx.insert(Element::new(&me.name, ElementKind::PaperMark(c)));
                }
                (tx, format!("Placed on {} more.", to.len()))
            }
            MarkAct::Remove(gone) => {
                let mut tx = Transaction::new("Take copies away");
                for g in &gone {
                    tx = tx.remove_cascade(model, *g);
                }
                // the first gone: the next left becomes the first
                if gone.contains(&first) {
                    let left: Vec<&(ElementId, PaperMark)> =
                        fam.iter().filter(|(i, _)| !gone.contains(i)).collect();
                    if let Some((new_first, _)) = left.first() {
                        for (i, _) in &left {
                            let Some(was) = model.get(*i).cloned() else {
                                continue;
                            };
                            let mut after = was.clone();
                            if let ElementKind::PaperMark(q) = &mut after.kind {
                                q.copy_of = (*i != *new_first).then_some(*new_first);
                            }
                            tx = tx.replace(was, after);
                        }
                    }
                }
                (tx, format!("Took {} away.", gone.len()))
            }
            MarkAct::SameOnAll => {
                let mut tx = Transaction::new("Same on all copies");
                for (i, _) in fam.iter().filter(|(i, _)| *i != id) {
                    let Some(was) = model.get(*i).cloned() else {
                        continue;
                    };
                    let mut after = was.clone();
                    if let ElementKind::PaperMark(q) = &mut after.kind {
                        q.size = m.size;
                        q.color = m.color;
                        q.kind = match (&m.kind, &q.kind) {
                            (
                                PaperMarkKind::ScaleBar { parts, style, .. },
                                PaperMarkKind::ScaleBar {
                                    scale,
                                    length,
                                    feet,
                                    ..
                                },
                            ) => PaperMarkKind::ScaleBar {
                                scale: *scale,
                                length: *length,
                                parts: *parts,
                                feet: *feet,
                                style: style.clone(),
                            },
                            _ => m.kind.clone(),
                        };
                    }
                    tx = tx.replace(was, after);
                }
                (tx, "Every copy looks like this one now.".to_string())
            }
        };
        let r = self.doc.commit(tx);
        self.report(r, said);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scale_bar_lists_drawings_and_greys_schedules() {
        let model = creabim_core::sample::sample_house();
        let Some((sheet, _)) = gsheet::sheets(&model).into_iter().next() else {
            return;
        };
        let m = PaperMark::scale_bar(sheet, glam::DVec2::new(50.0, 50.0), 100.0);
        let list = places(&model, &m);
        assert!(!list.is_empty());
        for p in &list {
            if let Some(d) = p.drawing {
                assert_eq!(p.ok, gsheet::drawing_scale(&model, d).is_some());
            }
        }
    }
}
