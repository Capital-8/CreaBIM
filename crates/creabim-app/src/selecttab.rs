//! The Select tab of Properties (wave 3, decision 6, the owner's idea):
//! what the selection holds and how to refine it, shown at all times. Its
//! blocks: what is in the selection by category, each with a tick to take
//! it out; Select similar (same type, same category, and By its values:
//! the selected element's values, ticked to match, the matches counted
//! live); and All, None, Invert, with every one of a kind. The Select ▾
//! and right-click menus keep their entries as shortcuts. New ways of
//! selecting belong here.
//!
//! "Its values" are the columns its schedule shows (doors: type, level,
//! wall, width, height, sill…), so what matches here matches the schedule.

use super::*;
use creabim_core::visibility::{cutters, Category};
use creabim_core::{OpeningKind, ScheduleCategory};
use creabim_geom::schedule;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

/// The schedule an element is listed in, by its kind.
fn schedule_category(e: &Element) -> Option<ScheduleCategory> {
    Some(match &e.kind {
        ElementKind::Opening(o) if o.kind == OpeningKind::Door => ScheduleCategory::Doors,
        ElementKind::Opening(o) if o.kind == OpeningKind::Window => ScheduleCategory::Windows,
        ElementKind::Wall(_) => ScheduleCategory::Walls,
        ElementKind::Slab(_) => ScheduleCategory::Slabs,
        ElementKind::Column(_) | ElementKind::Beam(_) => ScheduleCategory::Structure,
        ElementKind::Room(_) => ScheduleCategory::Rooms,
        _ => return None,
    })
}

/// Columns that name one element alone: never shared, not offered.
const OWN: [&str; 4] = ["Mark", "Name", "Number", "Ref"];

/// A schedule's column titles and each listed element's cells.
type Table = Arc<(Vec<String>, HashMap<ElementId, Vec<String>>)>;

/// The schedule of `cat`, kept until the model changes.
fn table(ctx: &egui::Context, model: &Model, cat: ScheduleCategory) -> Table {
    let key = egui::Id::new(("seltab.schedule", format!("{cat:?}")));
    let rev = model.revision();
    if let Some((r, t)) = ctx.data(|d| d.get_temp::<(u64, Table)>(key)) {
        if r == rev {
            return t;
        }
    }
    let s = schedule::schedule_by(model, cat, false);
    let titles = s.columns.iter().map(|c| c.title.to_string()).collect();
    let rows = s
        .rows
        .into_iter()
        .filter(|r| !r.group)
        .map(|r| (r.id, r.cells))
        .collect();
    let t: Table = Arc::new((titles, rows));
    ctx.data_mut(|d| d.insert_temp(key, (rev, t.clone())));
    t
}

/// An element's values as its schedule lists them, (column, value), for
/// the columns it could share with others; None for an element no
/// schedule lists.
pub(super) fn values(
    ctx: &egui::Context,
    model: &Model,
    id: ElementId,
) -> Option<Vec<(String, String)>> {
    let cat = schedule_category(model.get(id)?)?;
    let t = table(ctx, model, cat);
    let mine = t.1.get(&id)?;
    Some(
        t.0.iter()
            .zip(mine)
            .filter(|(title, v)| {
                !OWN.contains(&title.as_str()) && !v.trim().is_empty() && v.as_str() != "-"
            })
            .map(|(t, v)| (t.clone(), v.clone()))
            .collect(),
    )
}

/// The elements like `id` on every ticked column, in `within` if given,
/// `id` among them.
pub(super) fn matching(
    ctx: &egui::Context,
    model: &Model,
    id: ElementId,
    ticked: &BTreeSet<String>,
    within: Option<&HashSet<ElementId>>,
) -> Vec<ElementId> {
    let Some(cat) = model.get(id).and_then(schedule_category) else {
        return vec![];
    };
    let t = table(ctx, model, cat);
    let Some(mine) = t.1.get(&id) else {
        return vec![];
    };
    let cols: Vec<usize> =
        t.0.iter()
            .enumerate()
            .filter(|(_, title)| ticked.contains(*title))
            .map(|(i, _)| i)
            .collect();
    if cols.is_empty() {
        return vec![];
    }
    let mut out: Vec<ElementId> =
        t.1.iter()
            .filter(|(other, _)| within.is_none_or(|w| w.contains(other)))
            .filter(|(_, cells)| cols.iter().all(|i| cells.get(*i) == mine.get(*i)))
            .map(|(id, _)| *id)
            .collect();
    out.sort_by_key(|id| id.0);
    out
}

/// What the tab keeps between frames: the columns ticked, and whether the
/// whole project is searched rather than this view.
#[derive(Clone, Default)]
struct Ask {
    ticked: BTreeSet<String>,
    project: bool,
    /// The schedule the ticks are for: picking another kind of element
    /// starts with none ticked (first look: a door's Width tick lit
    /// windows and read "None match" on a wall).
    kind: Option<ScheduleCategory>,
}

fn ask_id() -> egui::Id {
    egui::Id::new("seltab.ask")
}

/// Buttons sharing a row in whole pixels, so none shifts by a hair as the
/// panel is dragged (MU58). Returns which was clicked.
fn button_row(ui: &mut egui::Ui, buttons: &[(&str, &str, bool)]) -> Option<usize> {
    button_row_in(ui, buttons, 0.0)
}

/// [`button_row`] kept `inset` in from both sides (inside a sub-box).
fn button_row_in(ui: &mut egui::Ui, buttons: &[(&str, &str, bool)], inset: f32) -> Option<usize> {
    let mut hit = None;
    // across the whole column, not in the value column: three buttons in
    // a value column don't fit a narrow panel (first look, wave 3)
    let full = (ui.available_width() - inset).floor();
    ui.horizontal(|ui| {
        ui.add_space(inset);
        ui.allocate_ui_with_layout(
            egui::vec2(full, 24.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                let gap = ui.spacing().item_spacing.x;
                let n = buttons.len() as f32;
                let w = ((full - gap * (n - 1.0)) / n).floor();
                for (i, (label, tip, on)) in buttons.iter().enumerate() {
                    if ui
                        .add_enabled(*on, egui::Button::new(*label).min_size(egui::vec2(w, 24.0)))
                        .on_hover_text(*tip)
                        .clicked()
                    {
                        hit = Some(i);
                    }
                }
            },
        );
    });
    hit
}

impl CreaBimApp {
    /// The Select tab (see the module's notes).
    pub(super) fn select_tab(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let ids = self.selection.all();
        let mut act: Option<String> = None;

        // what is in the selection, by category; untick one to take it out
        let open = self.prop_header(ui, "selin", "In the selection", true, Some("select"));
        let mut drop: Option<Category> = None;
        crate::props::fold(ui, open, |ui| {
            if ids.is_empty() {
                crate::props::empty(ui, "Nothing selected.");
                return;
            }
            let cats = selectsimilar::categories_of(self.doc.model(), &ids);
            for (i, (c, n)) in cats.iter().enumerate() {
                let mut on = true;
                let group = if i == 0 { "Holds" } else { "" };
                if crate::props::check(ui, group, &mut on, format!("{}  ({n})", c.label()))
                    .on_hover_text("Untick to take these out of the selection")
                    .changed()
                {
                    drop = Some(*c);
                }
            }
            let other = ids.len() - cats.iter().map(|c| c.1).sum::<usize>();
            if other > 0 {
                crate::props::plain_note(ui, &format!("and {other} of no category"));
            }
        });
        if let Some(c) = drop {
            let model = self.doc.model();
            let cut = cutters(model);
            let gone: HashSet<ElementId> = ids
                .iter()
                .copied()
                .filter(|id| model.get(*id).and_then(|e| Category::of(e, &cut)) == Some(c))
                .collect();
            self.selection.retain(|id| !gone.contains(&id));
            self.face = None;
            self.status = format!(
                "{} taken out of the selection: {} left.",
                c.label(),
                self.selection.len()
            );
        }

        // select similar: same type, same category, by its values
        let open = self.prop_header(ui, "selsimilar", "Select similar", true, Some("select"));
        let mut ask: Ask = ctx.data(|d| d.get_temp(ask_id())).unwrap_or_default();
        let mut by_values: Option<Vec<ElementId>> = None;
        let active = self.selection.active;
        crate::props::fold(ui, open, |ui| {
            if ids.is_empty() {
                crate::props::empty(ui, "Select something, then pick more like it here.");
                return;
            }
            crate::props::choice(
                ui,
                "Look in",
                &mut ask.project,
                &[
                    (false, "View", "Only what this view shows"),
                    (true, "Project", "Every level and view"),
                ],
            );
            let scope = if ask.project { "project" } else { "view" };
            match button_row(
                ui,
                &[
                    (
                        "Type",
                        "Every element of the same type (Shift+A in the view)",
                        true,
                    ),
                    (
                        "Category",
                        "Every wall, door, slab… like the selection",
                        true,
                    ),
                ],
            ) {
                Some(0) => act = Some(format!("similar:type.{scope}")),
                Some(_) => act = Some(format!("similar:cat.{scope}")),
                None => {}
            }
            let Some(id) = active else {
                return;
            };
            let kind = self.doc.model().get(id).and_then(schedule_category);
            if kind != ask.kind {
                ask.ticked.clear();
                ask.kind = kind;
            }
            let vals = values(&ctx, self.doc.model(), id);
            let name = self
                .doc
                .model()
                .get(id)
                .map_or(String::new(), |e| e.name.clone());
            let within = (!ask.project && !ask.ticked.is_empty()).then(|| self.ids_in_view());
            let (_, help_at) = crate::props::sub_box_help(
                ui,
                "selvalues",
                "By its values",
                None,
                true,
                true,
                |ui| {
                    let Some(vals) = vals else {
                        crate::props::plain_note(
                        ui,
                        "Values are what a schedule lists: doors, windows, walls, slabs, columns, beams and rooms.",
                    );
                        return;
                    };
                    for (title, v) in &vals {
                        let mut on = ask.ticked.contains(title);
                        if crate::props::check(ui, title, &mut on, v.as_str()).changed() {
                            if on {
                                ask.ticked.insert(title.clone());
                            } else {
                                ask.ticked.remove(title);
                            }
                        }
                    }
                    let found = matching(&ctx, self.doc.model(), id, &ask.ticked, within.as_ref());
                    // lit in the plans and the 3D view as they are ticked
                    if found != self.plan.spotlight {
                        ui.ctx().request_repaint();
                    }
                    self.match_preview = found.clone();
                    let label = match found.len() {
                        _ if ask.ticked.is_empty() => "Tick a value to match".to_string(),
                        0 => "None match here".to_string(),
                        n => format!("Select {n}"),
                    };
                    let tip = "Select every one that matches on the ticked values";
                    if button_row_in(ui, &[(label.as_str(), tip, !found.is_empty())], 6.0).is_some()
                    {
                        by_values = Some(found);
                    }
                },
            );
            // what to do, in a ? beside the heading (MU64: the hint line
            // took a row of its own)
            if let Some(r) = help_at {
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r));
                self.quick_help_button(
                    &mut child,
                    &format!(
                        "say:Tick the values of {name} that must match: everything alike on all of them lights up in the views, and the button selects it."
                    ),
                    "By its values",
                );
            }
        });
        if let Some(found) = by_values {
            let n = found.len();
            let what: Vec<String> = ask.ticked.iter().map(|t| t.to_lowercase()).collect();
            self.selection.extend(found, false);
            self.face = None;
            self.status = format!(
                "{n} selected, alike by {} ({}).",
                what.join(", "),
                if ask.project {
                    "whole project"
                } else {
                    "this view"
                }
            );
        }
        ctx.data_mut(|d| d.insert_temp(ask_id(), ask));

        // select: all, none, invert, and every one of a kind
        let open = self.prop_header(ui, "selall", "Select", true, Some("select"));
        let mut kinds: Vec<(&'static str, usize)> = vec![];
        if open {
            for id in self.selectable_ids() {
                if let Some(e) = self.doc.model().get(id) {
                    let k = kind_word(&e.kind);
                    match kinds.iter_mut().find(|(w, _)| *w == k) {
                        Some((_, n)) => *n += 1,
                        None => kinds.push((k, 1)),
                    }
                }
            }
            kinds.sort_by_key(|k| k.0);
        }
        let some = !ids.is_empty();
        crate::props::fold(ui, open, |ui| {
            match button_row(
                ui,
                &[
                    ("All", "Everything that can be picked (Ctrl+A)", true),
                    ("None", "Clear the selection (Esc)", some),
                    (
                        "Invert",
                        "Select what is not selected, and drop what is",
                        true,
                    ),
                ],
            ) {
                Some(0) => act = Some("cmd:SelectAll".into()),
                Some(1) => act = Some("cmd:SelectNone".into()),
                Some(_) => act = Some("sel.invert".into()),
                None => {}
            }
            crate::props::row(ui, "All of", |ui| {
                egui::ComboBox::from_id_salt("seltab.kind")
                    .icon(crate::props::combo_icon)
                    .width(ui.available_width())
                    .selected_text("A kind…")
                    .show_ui(ui, |ui| {
                        for (k, n) in &kinds {
                            let word = plural(k, 2);
                            let mut c = word.chars();
                            let label: String = c
                                .next()
                                .map_or(String::new(), |f| f.to_uppercase().chain(c).collect());
                            if ui
                                .selectable_label(false, format!("{label}  ({n})"))
                                .clicked()
                            {
                                act = Some(format!("sel.kind:{k}"));
                            }
                        }
                    });
            });
        });
        if let Some(a) = act {
            self.run_action(&ctx, &a);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn by_its_values_picks_the_doors_alike() {
        let model = creabim_core::starter::starter_house();
        let ctx = egui::Context::default();
        let width = |e: &Element| match &e.kind {
            ElementKind::Opening(o) => o.width,
            _ => f64::NAN,
        };
        let doors: Vec<&Element> = model
            .iter()
            .filter(|e| matches!(&e.kind, ElementKind::Opening(o) if o.kind == OpeningKind::Door))
            .collect();
        let d = doors[0];
        let vals = values(&ctx, &model, d.id).expect("a door has values");
        assert!(vals.iter().any(|(t, _)| t == "Width"), "{vals:?}");
        assert!(!vals.iter().any(|(t, _)| t == "Mark"), "a mark is its own");
        // by width: every door as wide, the door itself among them
        let ticked: BTreeSet<String> = ["Width".to_string()].into();
        let found = matching(&ctx, &model, d.id, &ticked, None);
        assert!(found.contains(&d.id));
        let same = doors
            .iter()
            .filter(|o| (width(o) - width(d)).abs() < 1e-6)
            .count();
        assert_eq!(found.len(), same);
        // within a set: only those in it
        let only: HashSet<ElementId> = [d.id].into();
        assert_eq!(
            matching(&ctx, &model, d.id, &ticked, Some(&only)),
            vec![d.id]
        );
        // nothing ticked: nothing matches
        assert!(matching(&ctx, &model, d.id, &BTreeSet::new(), None).is_empty());
        // a furniture piece or a level has no values here
        let level = model
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Level(_)))
            .unwrap();
        assert!(values(&ctx, &model, level.id).is_none());
    }
}
