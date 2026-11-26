//! Phases in the app (Mockup_Phases REV04): the small tag beside each
//! view's name that says which phase it stands at and opens its menu, the
//! Phases tab of Properties, and the Time rows of the Element and View
//! tabs. The model is `creabim_core::phases`.
//!
//! Owner's decisions: Demolition is a phase of its own; nothing is added
//! along a view's edge (no time strip), only the tag.

use super::*;
use creabim_core::phases::{self, PhaseDef, Status};

/// What a tag, a menu or a Time row sets the phase of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PhaseOf {
    /// A view of the model: it keeps its own phase.
    View(ElementId),
    /// The 3D window, whose phase is the session's.
    Window3d,
}

/// A row of a list in the Phases tab: its name, a quiet value at its
/// right, lit when `on`, marked when `shown` only. True when clicked.
fn list_row(ui: &mut egui::Ui, name: &str, right: &str, on: bool, shown: bool) -> bool {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::click());
    let v = ui.visuals();
    if on || shown || resp.hovered() {
        let fill = if on {
            v.selection.bg_fill.gamma_multiply(0.35)
        } else if shown {
            // shown in the view in front, not worked in: a fainter tint
            v.selection.bg_fill.gamma_multiply(0.14)
        } else {
            v.widgets.hovered.weak_bg_fill
        };
        ui.painter().rect_filled(r, 4.0, fill);
    }
    let ink = v.text_color();
    let right = right.to_string();
    // the value at the right first, at most half the row; the name has
    // the rest and is cut with … (shown whole on hover)
    let mut job = egui::text::LayoutJob::single_section(
        right.clone(),
        egui::TextFormat::simple(egui::FontId::proportional(11.5), v.weak_text_color()),
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width((w * 0.5).max(20.0));
    let rg = ui.painter().layout_job(job);
    let room = (w - rg.size().x - 24.0).max(24.0);
    let text = if on {
        egui::RichText::new(name).strong().color(ink)
    } else {
        egui::RichText::new(name).color(ink)
    };
    let g = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        room,
        egui::TextStyle::Body,
    );
    ui.painter()
        .galley(r.left_center() + Vec2::new(8.0, -g.size().y / 2.0), g, ink);
    ui.painter().galley(
        egui::pos2(
            r.right() - 8.0 - rg.size().x,
            r.center().y - rg.size().y / 2.0,
        ),
        rg,
        v.weak_text_color(),
    );
    let whole = if right.is_empty() {
        name.to_string()
    } else {
        format!("{name}  ·  {right}")
    };
    resp.on_hover_text(whole).clicked()
}

use crate::phaselens::{lens_of, lensed, set_lens};

/// The green of a phase that builds: the demolition red's strength.
const BUILDS_GREEN: [f32; 3] = [0.16, 0.50, 0.31];

/// A tag's ink and ground from its colour (none is grey), for the theme.
fn tag_colours(hue: Option<[f32; 3]>, dark: bool) -> (Color32, Color32) {
    let Some(c) = hue else {
        return if dark {
            (Color32::from_gray(200), Color32::from_gray(64))
        } else {
            (Color32::from_gray(80), Color32::from_gray(228))
        };
    };
    // toward the paper for the ground, toward its ink for the words
    let mix = |to: f32, by: f32| {
        let k = |v: f32| ((v + (to - v) * by).clamp(0.0, 1.0) * 255.0) as u8;
        Color32::from_rgb(k(c[0]), k(c[1]), k(c[2]))
    };
    if dark {
        (mix(1.0, 0.45), mix(0.0, 0.62))
    } else {
        (mix(0.0, 0.12), mix(1.0, 0.82))
    }
}

impl CreaBimApp {
    /// The project's phases; empty when it has none.
    pub(super) fn phase_list(&self) -> Vec<PhaseDef> {
        phases::list(self.doc.model()).to_vec()
    }

    /// The phase `of` is set to; none is the project's last.
    pub(super) fn phase_set(&self, of: PhaseOf) -> Option<u32> {
        match of {
            PhaseOf::View(v) => lensed(v, self.view(v).and_then(|v| v.phase)),
            PhaseOf::Window3d => self.phase3d,
        }
    }

    /// The view's own phase, whatever an area shows it at: what its
    /// sheets print.
    pub(super) fn view_phase(&self, v: ElementId) -> Option<u32> {
        self.view(v).and_then(|v| v.phase)
    }

    /// The options `of` shows, where not each question's pick.
    pub(super) fn options_set(&self, of: PhaseOf) -> Vec<u32> {
        match of {
            PhaseOf::View(v) => self.view(v).map(|v| v.options.clone()).unwrap_or_default(),
            PhaseOf::Window3d => self.options3d.clone(),
        }
    }

    /// The options `of` ghosts.
    pub(super) fn ghost_set(&self, of: PhaseOf) -> Vec<u32> {
        match of {
            PhaseOf::View(v) => self.view(v).map(|v| v.ghost.clone()).unwrap_or_default(),
            PhaseOf::Window3d => self.ghost3d.clone(),
        }
    }

    /// Ghost in `of` every option it does not show, or none.
    pub(super) fn set_ghost(&mut self, of: PhaseOf, on: bool) {
        let shown = self.options_shown(of);
        let ghost: Vec<u32> = if on {
            phases::questions(self.doc.model())
                .iter()
                .flat_map(|q| q.options.iter().map(|o| o.id))
                .filter(|o| !shown.contains(o))
                .collect()
        } else {
            vec![]
        };
        match of {
            PhaseOf::View(v) => {
                let _ = self.doc.edit(v, "Ghost options", |e| {
                    if let ElementKind::View(view) = &mut e.kind {
                        view.ghost = ghost;
                    }
                });
            }
            PhaseOf::Window3d => {
                self.ghost3d = ghost;
                self.hidden_rev += 1;
            }
        }
        self.status = if on {
            "The other options show as blue ghosts here: seen, not clicked.".into()
        } else {
            "Ghosts off.".into()
        };
    }

    /// The option `of` shows for each open question.
    pub(super) fn options_shown(&self, of: PhaseOf) -> Vec<u32> {
        phases::chosen(self.doc.model(), &self.options_set(of))
    }

    /// The option worked in, when `of` shows it: what is drawn in `of` is
    /// that option's alone. None is what every option shares.
    pub(super) fn option_worked(&self, of: PhaseOf) -> Option<u32> {
        self.work_option
            .filter(|o| self.options_shown(of).contains(o))
    }

    /// Show option `id` in `of`, in place of the others of its question,
    /// and work in it (`work`) or in what all options share.
    pub(super) fn set_option(&mut self, of: PhaseOf, id: u32, work: bool) {
        let model = self.doc.model();
        let Some((q, o)) = phases::option(model, id) else {
            return;
        };
        let (name, siblings): (String, Vec<u32>) =
            (o.name.clone(), q.options.iter().map(|o| o.id).collect());
        let pick = q.pick;
        let mut set = self.options_set(of);
        set.retain(|x| !siblings.contains(x));
        if id != pick {
            set.push(id);
        }
        match of {
            PhaseOf::View(v) => {
                let _ = self.doc.edit(v, "View's option", |e| {
                    if let ElementKind::View(view) = &mut e.kind {
                        view.options = set;
                    }
                });
            }
            PhaseOf::Window3d => {
                self.options3d = set;
                self.hidden_rev += 1;
                self.snaps_3d = None;
            }
        }
        // what is ghosted follows: always the options not shown
        if !self.ghost_set(of).is_empty() {
            self.set_ghost(of, true);
        }
        self.drop_absent_selection(of);
        self.work_option = work.then_some(id);
        self.status = if work {
            format!("Working in {name}: what you draw here is {name}'s alone, and Delete takes things out of {name} only.")
        } else {
            format!("Showing {name}, working in what all options share.")
        };
    }

    /// The phase `of` stands at, by id; none for a project without phases.
    pub(super) fn phase_at(&self, of: PhaseOf) -> Option<u32> {
        phases::stands_at_id(phases::list(self.doc.model()), self.phase_set(of))
    }

    /// What is in front: the 3D window, or the view shown.
    pub(super) fn phase_front(&self) -> Option<PhaseOf> {
        if self.model_in_front() {
            return Some(PhaseOf::Window3d);
        }
        let v = self.active_view?;
        self.view(v).map(|_| PhaseOf::View(v))
    }

    /// Let go of what is selected and not there in `of` any more: after
    /// the view moved to another phase or option, an outline round
    /// something it no longer shows would be left (seen in the app).
    fn drop_absent_selection(&mut self, of: PhaseOf) {
        if self.phase_front() != Some(of) {
            return;
        }
        let gone = phases::absent(
            self.doc.model(),
            self.phase_set(of),
            &self.options_set(of),
            &[],
        );
        if gone.is_empty() {
            return;
        }
        let before = self.selection.len();
        self.selection.retain(|id| !gone.contains(&id));
        if self.selection.len() != before {
            self.face = None;
        }
    }

    /// Stand `of` at phase `id`.
    pub(super) fn set_phase(&mut self, of: PhaseOf, id: u32) {
        let name = self
            .phase_list()
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        match of {
            // in an area: that area alone moves through time, as its zoom
            // does; the view, its other areas and its sheets stay (MU133)
            PhaseOf::View(v) if self.plan.pane_now.is_some() => {
                let list = self.phase_list();
                let own = phases::stands_at_id(&list, self.view_phase(v));
                let i = self.area_now;
                if own == Some(id) {
                    self.set_area_phase(i, None);
                    set_lens(None);
                    self.status = format!("This area is back at the view's phase, {name}.");
                } else {
                    self.set_area_phase(i, Some(id));
                    set_lens(Some((v, id)));
                    let view_name = own
                        .and_then(|o| list.iter().find(|p| p.id == o))
                        .map(|p| p.name.clone())
                        .unwrap_or_default();
                    self.status = format!(
                        "This area stands at {name}: what you draw in it is built in {name}. The view and its sheets stay at {view_name}."
                    );
                }
            }
            PhaseOf::View(v) => {
                let r = self.doc.edit(v, "View's phase", |e| {
                    if let ElementKind::View(view) = &mut e.kind {
                        view.phase = Some(id);
                    }
                });
                self.report(
                    r,
                    format!("This view stands at {name}: what you draw in it is built in {name}."),
                );
            }
            PhaseOf::Window3d => {
                self.phase3d = Some(id);
                self.hidden_rev += 1;
                self.snaps_3d = None;
                self.status = format!(
                    "The 3D view stands at {name}: what you draw in it is built in {name}."
                );
            }
        }
        self.drop_absent_selection(of);
    }

    /// What is drawn from now on is built in the phase `of` stands at.
    pub(super) fn work_in_phase(&mut self, of: PhaseOf) {
        let at = self.phase_at(of);
        if self.doc.working_phase() != at {
            self.doc.set_working_phase(at);
        }
        let option = self.option_worked(of);
        if self.doc.working_option() != option {
            self.doc.set_working_option(option);
        }
        self.doc.set_hold_shared(self.hold_shared);
        self.doc.set_alter_existing(self.alter_existing);
        // say so when a change has just altered existing work
        let n = self.doc.take_altered();
        if n > 0 {
            let at = self
                .phase_at(of)
                .and_then(|a| self.phase_list().into_iter().find(|p| p.id == a));
            let name = at.map(|p| p.name).unwrap_or_default();
            self.status = format!(
                "Altered: the existing one comes down and the changed one is built in {name}. Ctrl+Z twice puts it back."
            );
        }
    }

    /// How a change to existing work is taken, from a later phase: as a
    /// correction of the survey, or as an alteration there.
    fn existing_choice(&mut self, ui: &mut egui::Ui) {
        let mut alter = self.alter_existing;
        if crate::props::choice(
            ui,
            "Changes",
            &mut alter,
            &[
                (false, "Correct", "Correct the survey: the element was always like this, in every phase"),
                (true, "Alter", "Alter it here: the element as it was comes down, and the changed one is built in the phase the view stands at"),
            ],
        ) {
            self.alter_existing = alter;
            self.doc.set_alter_existing(alter);
            self.status = if alter {
                "Changing existing work now alters it: the old comes down, the changed is built in the view's phase.".into()
            } else {
                "Changing existing work now corrects the survey: it changes in every phase.".into()
            };
        }
    }

    /// The tag beside a view's name: the phase it stands at, and its menu.
    /// `x` is where the name ends, from the area's left edge. Nothing is
    /// drawn for a project without phases.
    pub(super) fn phase_tag(&mut self, ui: &mut egui::Ui, rect: Rect, x: f32, of: PhaseOf) {
        let list = self.phase_list();
        if list.is_empty() {
            return;
        }
        let at = phases::stands_at(&list, self.phase_set(of));
        let font = egui::FontId::proportional(12.5);
        let dark = ui.visuals().dark_mode;
        // grey on the first phase, where everything is as surveyed; the
        // project's demolition colour on a phase that takes things down;
        // green, of the same strength, on one that builds (owner)
        let hue = if at == 0 {
            None
        } else if list[at].demolition {
            Some(phases::look(self.doc.model()).down)
        } else {
            Some(BUILDS_GREEN)
        };
        let (ink, fill) = tag_colours(hue, dark);
        // with a question open: the option worked in, or that the work is
        // shared and which option shows
        let shown = self.options_shown(of);
        let option_name = |id: u32| {
            phases::option(self.doc.model(), id)
                .map(|(_, o)| o.name.clone())
                .unwrap_or_default()
        };
        // an area standing at a phase of its own: a dot after the name
        let own_phase = match of {
            PhaseOf::View(v) => {
                lens_of(v).filter(|p| phases::stands_at_id(&list, self.view_phase(v)) != Some(*p))
            }
            PhaseOf::Window3d => None,
        };
        let mut text = list[at].name.clone();
        if let Some(first) = shown.first() {
            match self.option_worked(of) {
                Some(w) => text = format!("{text}  ·  {}", option_name(w)),
                None => text = format!("{text}  ·  {} (shared)", option_name(*first)),
            }
        }
        let questions = phases::questions(self.doc.model()).to_vec();
        let worked = self.option_worked(of);
        let galley = ui.painter().layout_no_wrap(text, font, ink);
        // room for the dot of an area at a phase of its own
        let dot = if own_phase.is_some() { 10.0 } else { 0.0 };
        let size = Vec2::new(galley.size().x + 30.0 + dot, 21.0);
        let min = rect.left_top() + Vec2::new(x, 5.0);
        let r = Rect::from_min_size(min, size);
        if r.right() > rect.right() - 52.0 {
            return;
        }
        let resp = ui
            .interact(r, ui.id().with(("phase-tag", of == PhaseOf::Window3d, rect.min.x as i32, rect.min.y as i32)), Sense::click())
            .on_hover_text(if own_phase.is_some() {
                "This area stands at a phase of its own; the view and its sheets stay at theirs. Click to move this area through time."
            } else {
                "The phase this area stands at. What you draw here is built in it. Click to move it through time; the view's sheets stay as they are."
            });
        let p = ui.painter_at(rect);
        p.rect_filled(
            r,
            5.0,
            if resp.hovered() {
                fill.gamma_multiply(0.85)
            } else {
                fill
            },
        );
        let gw = galley.size().x;
        p.galley(
            r.left_top() + Vec2::new(9.0, (size.y - galley.size().y) / 2.0),
            galley,
            ink,
        );
        if own_phase.is_some() {
            p.circle_filled(
                egui::pos2(r.left() + 9.0 + gw + 7.0, r.center().y),
                2.6,
                ink,
            );
        }
        let c = egui::pos2(r.right() - 13.0, r.center().y);
        p.line_segment(
            [c + Vec2::new(-3.5, -1.5), c + Vec2::new(0.0, 2.0)],
            egui::Stroke::new(1.4, ink),
        );
        p.line_segment(
            [c + Vec2::new(0.0, 2.0), c + Vec2::new(3.5, -1.5)],
            egui::Stroke::new(1.4, ink),
        );
        let mut pick = None;
        let mut pick_option = None;
        let (mut make_view, mut back) = (false, false);
        let in_area = matches!(of, PhaseOf::View(_)) && self.plan.pane_now.is_some();
        let view_name = match of {
            PhaseOf::View(v) => phases::stands_at_id(&list, self.view_phase(v))
                .and_then(|o| list.iter().find(|p| p.id == o))
                .map(|p| p.name.clone())
                .unwrap_or_default(),
            PhaseOf::Window3d => String::new(),
        };
        let (ghosted, hold) = (!self.ghost_set(of).is_empty(), self.hold_shared);
        let (mut ghost_to, mut hold_to) = (None, None);
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(210.0);
            ui.label(egui::RichText::new("STANDS AT").small().weak());
            for (i, ph) in list.iter().enumerate() {
                if ui.selectable_label(i == at, &ph.name).clicked() {
                    pick = Some(ph.id);
                    ui.close();
                }
            }
            // this area at a phase of its own (MU133, Phase per area REV00)
            if in_area {
                ui.separator();
                if ui
                    .add_enabled(own_phase.is_some(), egui::Button::new("Make it the view's phase"))
                    .on_hover_text("The view stands here from now on, and its sheets print it")
                    .clicked()
                {
                    make_view = true;
                    ui.close();
                }
                if ui
                    .add_enabled(own_phase.is_some(), egui::Button::new(format!("Back to the view's phase, {view_name}")))
                    .on_hover_text("This area shows the view as its sheets do")
                    .clicked()
                {
                    back = true;
                    ui.close();
                }
            }
            for q in &questions {
                ui.separator();
                ui.label(egui::RichText::new(format!("WORKING IN  ·  {}", q.name.to_uppercase())).small().weak());
                for o in &q.options {
                    if ui.selectable_label(worked == Some(o.id), &o.name).clicked() {
                        pick_option = Some((o.id, true));
                        ui.close();
                    }
                }
                let showing = q.options.iter().map(|o| o.id).find(|o| shown.contains(o));
                if ui
                    .selectable_label(worked.is_none(), "What all options share")
                    .on_hover_text("Draw and change what every option has in common. The view keeps showing the option it shows.")
                    .clicked()
                {
                    pick_option = showing.map(|o| (o, false));
                    ui.close();
                }
            }
            if !questions.is_empty() {
                ui.separator();
                let mut g = ghosted;
                if ui.checkbox(&mut g, "Ghost the other options").on_hover_text("Show what the other options hold as blue ghosts in this view").changed() {
                    ghost_to = Some(g);
                }
                let mut h = hold;
                if ui.checkbox(&mut h, "Hold shared parts").on_hover_text("While you work in an option, what all options share can't be moved or changed by accident").changed() {
                    hold_to = Some(h);
                }
            }
            ui.separator();
            if ui.button("Phases…").on_hover_text("The Phases tab of Properties").clicked() {
                self.prop_tab = PropTab::Phases;
                ui.close();
            }
        });
        if let Some(id) = pick {
            self.set_phase(of, id);
        }
        if let (PhaseOf::View(v), Some(p)) = (of, own_phase) {
            if make_view || back {
                let i = self.area_now;
                self.set_area_phase(i, None);
                set_lens(None);
                if make_view {
                    let name = list
                        .iter()
                        .find(|x| x.id == p)
                        .map(|x| x.name.clone())
                        .unwrap_or_default();
                    let r = self.doc.edit(v, "View's phase", |e| {
                        if let ElementKind::View(view) = &mut e.kind {
                            view.phase = Some(p);
                        }
                    });
                    self.report(
                        r,
                        format!("The view stands at {name} now, and its sheets print it."),
                    );
                } else {
                    self.status = format!("This area is back at the view's phase, {view_name}.");
                }
                self.drop_absent_selection(of);
            }
        }
        if let Some((id, work)) = pick_option {
            self.set_option(of, id, work);
        }
        if let Some(g) = ghost_to {
            self.set_ghost(of, g);
        }
        if let Some(h) = hold_to {
            self.set_hold(h);
        }
    }

    /// Hold what all options share while an option is worked in, or not.
    pub(super) fn set_hold(&mut self, hold: bool) {
        self.hold_shared = hold;
        self.doc.set_hold_shared(hold);
        self.status = if hold {
            "Shared parts are held while you work in an option.".into()
        } else {
            "Shared parts are free: a change to one changes every option.".into()
        };
    }

    /// The Phases tab's Taken down list: everything a phase or an option
    /// has taken down, each with a way back. An accidental delete a few
    /// steps ago is found and undone here without undoing what came after.
    fn taken_down_fields(&mut self, ui: &mut egui::Ui) {
        let model = self.doc.model();
        let list = phases::list(model);
        let rows: Vec<(ElementId, String, String)> = phases::taken_down(model)
            .into_iter()
            .filter(|id| {
                // a door or window that came down with its wall is under the wall
                !matches!(model.get(*id).map(|e| &e.kind), Some(ElementKind::Opening(o))
                    if model.get(o.host).is_some_and(|h| h.phase.gone.is_some()))
            })
            .filter_map(|id| {
                let e = model.get(id)?;
                let phase = e
                    .phase
                    .gone
                    .and_then(|g| list.iter().find(|p| p.id == g))
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                let by = e
                    .phase
                    .gone_option
                    .and_then(|o| phases::option(model, o))
                    .map(|(_, o)| o.name.clone());
                let hung = model
                    .dependents(id)
                    .iter()
                    .filter(|d| {
                        model.get(**d).is_some_and(|x| {
                            matches!(x.kind, ElementKind::Opening(_))
                                && x.phase.gone == e.phase.gone
                        })
                    })
                    .count();
                let mut name = e.name.clone();
                if hung > 0 {
                    name = format!("{name} (+{hung})");
                }
                Some((
                    id,
                    name,
                    by.map_or(phase.clone(), |o| format!("{phase}, {o}")),
                ))
            })
            .collect();
        if rows.is_empty() {
            return;
        }
        let good = phases::making_good(model);
        let good_note = (!good.is_empty()).then(|| {
            let area: f64 = good.iter().map(|g| g.2).sum();
            format!(
                "Making good: {} opening{} in walls that stay, {} to fill.",
                good.len(),
                if good.len() == 1 { "" } else { "s" },
                model.units().area(area)
            )
        });
        let mut back: Option<Vec<ElementId>> = None;
        let mut select = None;
        self.down_pick.retain(|id| rows.iter().any(|r| r.0 == *id));
        let picked: Vec<ElementId> = self.down_pick.clone();
        crate::partstool::sfold_h(
            ui,
            "phases.down",
            "Taken down",
            "Everything a phase or an option has taken down. Click one to select it, then Bring back: it stands again in every phase and option, with the doors and windows that came down with it. Nothing drawn since is undone.",
            |ui| {
                egui::ScrollArea::vertical().max_height(156.0).id_salt("phases.down.list").show(ui, |ui| {
                    for (id, name, when) in &rows {
                        if list_row(ui, name, when, picked.contains(id), false) {
                            select = Some(*id);
                        }
                    }
                });
                if let Some(n) = &good_note {
                    crate::props::plain_note(ui, n);
                }
                let gap = ui.spacing().item_spacing.x;
                let w = ((ui.available_width() - gap) / 2.0).floor().max(40.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled_ui(!picked.is_empty(), |ui| ui.add_sized([w, 24.0], egui::Button::new("Bring back")))
                        .inner
                        .on_hover_text("Bring back what is picked in the list")
                        .clicked()
                    {
                        back = Some(picked.clone());
                    }
                    if ui.add_sized([w, 24.0], egui::Button::new("Bring all")).on_hover_text("Bring everything back: nothing is taken down any more").clicked() {
                        back = Some(rows.iter().map(|r| r.0).collect());
                    }
                });
            },
        );
        if let Some(id) = select {
            // click again to let go; Ctrl is not needed to pick several
            if let Some(at) = self.down_pick.iter().position(|x| *x == id) {
                self.down_pick.remove(at);
            } else {
                self.down_pick.push(id);
            }
            self.status = "Picked in the list. Bring back restores what is picked; click a row again to let it go.".into();
        }
        if let Some(ids) = back {
            self.bring_back(&ids);
        }
    }

    /// Bring `ids` back from having been taken down.
    pub(super) fn bring_back(&mut self, ids: &[ElementId]) {
        let tx = phases::bring_back(self.doc.model(), ids);
        let n = tx.ops.len();
        // a shared wall comes back whatever is held
        let hold = self.hold_shared;
        self.doc.set_hold_shared(false);
        let r = self.doc.commit(tx);
        self.doc.set_hold_shared(hold);
        self.report(r, format!("Brought back {n} element(s): standing again in every phase and option. Ctrl+Z to undo."));
    }

    /// The Phases tab's Options: the questions the design has open, the
    /// options answering each, which one the view in front shows and
    /// which is worked in.
    fn options_fields(&mut self, ui: &mut egui::Ui) {
        let questions = phases::questions(self.doc.model()).to_vec();
        let front = self.phase_front();
        let shown = front.map(|f| self.options_shown(f)).unwrap_or_default();
        let worked = front.and_then(|f| self.option_worked(f));
        let model = self.doc.model();
        let counts: Vec<Vec<(usize, usize)>> = questions
            .iter()
            .map(|q| {
                q.options
                    .iter()
                    .map(|o| phases::option_uses(model, o.id))
                    .collect()
            })
            .collect();
        // (the option to show and whether to work in it), a new question,
        // (a new option for), the pick, the one to settle with, a rename
        let mut go: Option<(u32, bool)> = None;
        let (mut new_q, mut new_o, mut pick, mut settle) = (false, None, None, None);
        let mut renamed: Option<(u32, String)> = None;
        let mut move_sel: Option<Option<u32>> = None;
        let selected = self.selection.all().len();
        let shelved = phases::shelf(self.doc.model()).to_vec();
        let (ghosted, hold) = (
            front.is_some_and(|f| !self.ghost_set(f).is_empty()),
            self.hold_shared,
        );
        let (mut ghost_to, mut hold_to, mut reopen) = (None, None, None);
        crate::partstool::sfold_h(
            ui,
            "phases.options",
            "Options",
            "Ways of answering an open question, kept side by side in one model. An option holds only what differs: what is built in it alone, and what it alone takes down. Click an option to show it in the view in front and work in it.",
            |ui| {
                if questions.is_empty() {
                    crate::props::plain_note(ui, "Trying two ways of doing something? Open a question: it starts with Option A and Option B.");
                }
                for (qi, q) in questions.iter().enumerate() {
                    let mut name = q.name.clone();
                    let changed = crate::props::row(ui, "Question", |ui| {
                        let w = ui.available_width();
                        crate::props::text_field(ui, egui::TextEdit::singleline(&mut name), w).changed()
                    });
                    if changed {
                        renamed = Some((q.id, name));
                    }
                    let showing = q.options.iter().map(|o| o.id).find(|o| shown.contains(o));
                    for (oi, o) in q.options.iter().enumerate() {
                        let (b, d) = counts[qi][oi];
                        let mut right = match (b, d) {
                            (0, 0) => "empty".to_string(),
                            (b, 0) => format!("{b} built"),
                            (0, d) => format!("{d} down"),
                            (b, d) => format!("{b} built, {d} down"),
                        };
                        if q.pick == o.id {
                            right = format!("pick  ·  {right}");
                        }
                        let on = worked == Some(o.id);
                        if list_row(ui, &o.name, &right, on, showing == Some(o.id) && !on) {
                            go = Some((o.id, true));
                        }
                    }
                    if list_row(ui, "Shared by all", "", worked.is_none(), false) {
                        go = showing.map(|o| (o, false));
                    }
                    if let Some(sh) = showing {
                        let mut oname = q.options.iter().find(|o| o.id == sh).map(|o| o.name.clone()).unwrap_or_default();
                        let changed = crate::props::row(ui, "Name", |ui| {
                            let w = ui.available_width();
                            crate::props::text_field(ui, egui::TextEdit::singleline(&mut oname), w).changed()
                        });
                        if changed {
                            renamed = Some((sh, oname));
                        }
                        let gap = ui.spacing().item_spacing.x;
                        let w = ((ui.available_width() - gap) / 2.0).floor().max(40.0);
                        ui.horizontal(|ui| {
                            if ui.add_sized([w, 24.0], egui::Button::new("+ Option")).on_hover_text("Another option for this question").clicked() {
                                new_o = Some(q.id);
                            }
                            if ui
                                .add_enabled_ui(q.pick != sh, |ui| ui.add_sized([w, 24.0], egui::Button::new("Pick it")))
                                .inner
                                .on_hover_text("What every view shows unless it says otherwise. The other options stay.")
                                .clicked()
                            {
                                pick = Some(sh);
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled_ui(selected > 0, |ui| ui.add_sized([w, 24.0], egui::Button::new("Move here")))
                                .inner
                                .on_hover_text("Put what is selected in what is worked in: this option, or what all options share")
                                .clicked()
                            {
                                move_sel = Some(worked);
                            }
                            if ui
                                .add_sized([w, 24.0], egui::Button::new("Settle on it"))
                                .on_hover_text("Close the question with the option shown: its work becomes the project's own. What the other options hold is kept in the file, out of every view.")
                                .clicked()
                            {
                                settle = Some(sh);
                            }
                        });
                    }
                    ui.add_space(6.0);
                }
                if !questions.is_empty() {
                    let mut g = ghosted;
                    crate::props::row(ui, "", |ui| {
                        if crate::props::switch(ui, &mut g, "Ghost others")
                            .on_hover_text("Show what the other options hold as blue ghosts in the view in front: seen, not clicked")
                            .changed()
                        {
                            ghost_to = Some(g);
                        }
                    });
                    let mut h = hold;
                    crate::props::row(ui, "", |ui| {
                        if crate::props::switch(ui, &mut h, "Hold shared")
                            .on_hover_text("While you work in an option, what all options share can't be moved or changed by accident. An option can still take a shared thing down.")
                            .changed()
                        {
                            hold_to = Some(h);
                        }
                    });
                }
                let w = ui.available_width();
                if ui.add_sized([w, 24.0], egui::Button::new("+ Question")).on_hover_text("An open question with two options, A and B").clicked() {
                    new_q = true;
                }
                for q in &shelved {
                    let kept: usize = q.options.iter().map(|o| phases::option_uses(model, o.id).0).sum();
                    let settled = q.options.iter().find(|o| o.id == q.pick).map(|o| o.name.as_str()).unwrap_or("");
                    let _ = kept;
                    if list_row(ui, &format!("{}: {settled}", q.name), "reopen", false, false) {
                        reopen = Some(q.id);
                    }
                }
            },
        );
        if let Some(g) = ghost_to {
            if let Some(f) = front {
                self.set_ghost(f, g);
            }
        }
        if let Some(h) = hold_to {
            self.set_hold(h);
        }
        if let Some(q) = reopen {
            if let Some(tx) = phases::reopen(self.doc.model(), q) {
                let r = self.doc.commit(tx);
                self.hidden_rev += 1;
                self.report(r, "Question open again, with what its options still hold.");
            }
        }
        if let Some((id, name)) = renamed {
            if let Some(tx) = phases::rename_option(self.doc.model(), id, &name) {
                let r = self.doc.commit_merged(tx);
                self.report(r, "Renamed.");
            }
        }
        if new_q {
            if let Some((tx, [a, _])) = phases::add_question(self.doc.model(), "Question") {
                let r = self.doc.commit(tx);
                self.report(r, "Question opened with Option A and Option B.");
                if let Some(f) = front {
                    self.set_option(f, a, true);
                }
            }
        }
        if let Some(q) = new_o {
            if let Some((tx, id)) = phases::add_option(self.doc.model(), q) {
                let r = self.doc.commit(tx);
                self.report(r, "Option added.");
                if let Some(f) = front {
                    self.set_option(f, id, true);
                }
            }
        }
        if let Some(id) = pick {
            if let Some(tx) = phases::set_pick(self.doc.model(), id) {
                let r = self.doc.commit(tx);
                self.hidden_rev += 1;
                self.report(
                    r,
                    "It is the pick: views show it unless they say otherwise.",
                );
            }
        }
        if let Some(to) = move_sel {
            let ids = self.selection.all();
            let tx = phases::move_to_option(self.doc.model(), &ids, to);
            let r = self.doc.commit(tx);
            self.report(
                r,
                if to.is_some() {
                    "The selection is this option's alone."
                } else {
                    "The selection is shared by all options."
                },
            );
        }
        if let Some(id) = settle {
            if let Some((tx, shelved)) = phases::settle(self.doc.model(), id) {
                let r = self.doc.commit(tx);
                self.work_option = None;
                self.hidden_rev += 1;
                self.report(
                    r,
                    format!("Question settled. {shelved} element(s) of the other options are kept in the file, out of every view. Ctrl+Z brings the question back."),
                );
            }
        }
        if let (Some((id, work)), Some(f)) = (go, front) {
            self.set_option(f, id, work);
        }
    }

    /// The Phases tab: starting phases, then the project through time.
    pub(super) fn phases_tab(&mut self, ui: &mut egui::Ui) {
        let list = self.phase_list();
        if list.is_empty() {
            crate::props::empty(ui, "This project has no phases yet.");
            crate::props::plain_note(
                ui,
                "With phases, everything remembers when it is built and when it comes down. One model then gives the existing, demolition and proposed drawings. Say what you have drawn so far:",
            );
            ui.add_space(4.0);
            let w = ui.available_width();
            let mut start = None;
            if ui
                .add_sized([w, 26.0], egui::Button::new("It is the existing building"))
                .on_hover_text("Everything drawn so far is Existing. Step to Proposed to draw what is new, and to Demolition to take things down.")
                .clicked()
            {
                start = Some(true);
            }
            if ui
                .add_sized([w, 26.0], egui::Button::new("It is the proposed building"))
                .on_hover_text("Everything drawn so far is new, built in Proposed. Step to Existing to draw what was there before.")
                .clicked()
            {
                start = Some(false);
            }
            if let Some(existing) = start {
                if let Some(tx) = phases::start(self.doc.model(), existing) {
                    let r = self.doc.commit(tx);
                    let home = phases::standard()[if existing { 0 } else { 2 }].id;
                    self.phase3d = Some(home);
                    self.hidden_rev += 1;
                    self.report(r, "Phases started: Existing, Demolition, Proposed. The tag beside a view's name moves it through time.");
                }
            }
            return;
        }
        let front = self.phase_front();
        let at = front.map(|f| phases::stands_at(&list, self.phase_set(f)));
        let model = self.doc.model();
        // what each phase builds and takes down
        let mut built = vec![0usize; list.len()];
        let mut down = vec![0usize; list.len()];
        for e in model.iter().filter(|e| phases::phased(e)) {
            built[e
                .phase
                .built
                .and_then(|b| phases::position(&list, b))
                .unwrap_or(0)] += 1;
            if let Some(g) = e.phase.gone.and_then(|g| phases::position(&list, g)) {
                down[g] += 1;
            }
        }
        // which view the tab speaks for, at its top: everything below
        // follows the view in front, and with two views side by side it
        // was not plain which one (MU100)
        let front_name = |app: &Self, f: PhaseOf| match f {
            PhaseOf::Window3d => "The 3D view".to_string(),
            PhaseOf::View(v) => app
                .doc
                .model()
                .get(v)
                .map(|e| e.name.clone())
                .unwrap_or_default(),
        };
        match (front, at.and_then(|i| list.get(i))) {
            (Some(f), Some(_)) => crate::props::readout(
                ui,
                "View",
                front_name(self, f),
                "The view this tab speaks for: the one you last clicked. Its phase is lit in the list below; click another view to see its own.",
            ),
            _ => crate::props::readout(ui, "View", "none: click a view", "This tab speaks for the view you last clicked."),
        }
        let mut pick = None;
        crate::partstool::sfold_h(
            ui,
            "phases.list",
            "Phases",
            "The project through time, first to last. Click one to stand the view in front at it: the view shows the project as it is then, and what you draw is built in that phase.",
            |ui| {
                for (i, p) in list.iter().enumerate() {
                    let count = match (built[i], down[i]) {
                        (0, 0) => "empty".to_string(),
                        (b, 0) => format!("{b} built"),
                        (0, d) => format!("{d} taken down"),
                        (b, d) => format!("{b} built, {d} down"),
                    };
                    let w = ui.available_width();
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::click());
                    let v = ui.visuals();
                    let on = at == Some(i);
                    if on || resp.hovered() {
                        let fill = if on { v.selection.bg_fill.gamma_multiply(0.35) } else { v.widgets.hovered.weak_bg_fill };
                        ui.painter().rect_filled(r, 4.0, fill);
                    }
                    let ink = v.text_color();
                    let name = if on {
                        egui::RichText::new(&p.name).strong().color(ink)
                    } else {
                        egui::RichText::new(&p.name).color(ink)
                    };
                    let g = egui::WidgetText::from(name).into_galley(ui, None, w, egui::TextStyle::Body);
                    ui.painter().galley(r.left_center() + Vec2::new(8.0, -g.size().y / 2.0), g, ink);
                    ui.painter().text(
                        r.right_center() - Vec2::new(8.0, 0.0),
                        egui::Align2::RIGHT_CENTER,
                        count,
                        egui::FontId::proportional(11.5),
                        v.weak_text_color(),
                    );
                    if resp.clicked() {
                        pick = Some(p.id);
                    }
                }
            },
        );
        self.phase_tools(ui, &list, at);
        crate::partstool::sfold_h(
            ui,
            "phases.existing",
            "Existing work",
            "What happens when you change something existing while a view stands at a later phase. Correct: the survey was wrong, so it changes in every phase. Alter: the building is being changed, so the old one comes down and the changed one is new.",
            |_ui| {},
        );
        self.existing_choice(ui);
        self.options_fields(ui);
        self.taken_down_fields(ui);
        self.phase_look_fields(ui);
        if let Some(f) = front {
            if let Some(id) = pick {
                self.set_phase(f, id);
            }
        }
    }

    /// How each status draws, for the whole project: the colour and dashes
    /// of what comes down, existing drawn fainter, a colour for new work.
    fn phase_look_fields(&mut self, ui: &mut egui::Ui) {
        let was = phases::look(self.doc.model());
        let mut look = was;
        let bytes = |c: [f32; 3]| c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
        let floats = |c: [u8; 3]| c.map(|v| v as f32 / 255.0);
        crate::partstool::sfold_h(
            ui,
            "phases.look",
            "How they draw",
            "The office's convention, for every view, sheet and the 3D view of this project. Existing: what was built before the phase a view stands at. Taken down: what that phase removes. New: what it builds.",
            |ui| {
                let mut c = bytes(look.down);
                if crate::props::colour_field(ui, "Down", &mut c, "The colour of what a phase takes down, in drawings and in 3D") {
                    look.down = floats(c);
                }
                crate::props::row(ui, "", |ui| {
                    crate::props::switch(ui, &mut look.down_dashed, "Dashed")
                        .on_hover_text("Draw what comes down in dashes")
                });
                crate::props::row(ui, "Existing", |ui| {
                    crate::props::switch(ui, &mut look.existing_faint, "Fainter")
                        .on_hover_text("Draw what was there before fainter and without its hatching, so the new work stands out")
                });
                let mut tint = look.new.is_some();
                crate::props::row(ui, "New", |ui| {
                    crate::props::switch(ui, &mut tint, "One colour")
                        .on_hover_text("Draw what a phase builds in one colour instead of its own")
                });
                if tint != look.new.is_some() {
                    look.new = tint.then_some(BUILDS_GREEN);
                }
                if let Some(n) = look.new {
                    let mut c = bytes(n);
                    if crate::props::colour_field(ui, "", &mut c, "The colour of what a phase builds") {
                        look.new = Some(floats(c));
                    }
                }
                if look != phases::PhaseLook::default() {
                    let back = crate::props::action(ui, "", "Reset", "Back to the usual: red dashes for what comes down, existing fainter, new in its own colours", true);
                    if back.clicked() {
                        look = phases::PhaseLook::default();
                    }
                }
            },
        );
        if look != was {
            if let Some(tx) = phases::set_look(self.doc.model(), |l| *l = look) {
                let r = self.doc.commit_merged(tx);
                self.hidden_rev += 1;
                self.report(r, "How phases draw changed, in every view.");
            }
        }
    }

    /// Under the list: the fields of the phase the view in front stands
    /// at (its name, whether things are taken down in it), and the list's
    /// tools: add after it, move it earlier or later, remove it.
    fn phase_tools(&mut self, ui: &mut egui::Ui, list: &[PhaseDef], at: Option<usize>) {
        let Some(p) = at.and_then(|i| list.get(i)).cloned() else {
            return;
        };
        let i = at.unwrap_or(0);
        ui.add_space(4.0);
        let mut name = p.name.clone();
        let changed = crate::props::row(ui, "Name", |ui| {
            let w = ui.available_width();
            crate::props::text_field(ui, egui::TextEdit::singleline(&mut name), w).changed()
        });
        if changed {
            if let Some(tx) = phases::rename(self.doc.model(), p.id, &name) {
                let r = self.doc.commit_merged(tx);
                self.report(r, format!("Phase renamed {}.", name.trim()));
            }
        }
        let mut down = p.demolition;
        let ticked = crate::props::row(ui, "", |ui| {
            crate::props::switch(ui, &mut down, "Takes down")
                .on_hover_text("A phase things are taken down in. A view standing at it shows what comes down in it, in red dashes. Delete from a later phase takes things down in the latest such phase before it.")
                .changed()
        });
        if ticked {
            if let Some(tx) = phases::set_demolition(self.doc.model(), p.id, down) {
                let r = self.doc.commit(tx);
                self.report(
                    r,
                    if down {
                        format!("Things are taken down in {}.", p.name)
                    } else {
                        format!("{} takes nothing down.", p.name)
                    },
                );
            }
        }
        ui.add_space(2.0);
        let gap = ui.spacing().item_spacing.x;
        let w = ((ui.available_width() - gap) / 2.0).floor().max(40.0);
        let (mut add, mut up, mut later, mut remove) = (false, false, false, false);
        ui.horizontal(|ui| {
            add = ui
                .add_sized([w, 24.0], egui::Button::new("+ Phase"))
                .on_hover_text(format!("A new phase after {}", p.name))
                .clicked();
            remove = ui
                .add_enabled_ui(list.len() > 1, |ui| {
                    ui.add_sized([w, 24.0], egui::Button::new("Remove"))
                })
                .inner
                .on_hover_text(
                    "Remove it. Only an empty phase can go: nothing built or taken down in it.",
                )
                .clicked();
        });
        ui.horizontal(|ui| {
            up = ui
                .add_enabled_ui(i > 0, |ui| {
                    ui.add_sized([w, 24.0], egui::Button::new("Earlier"))
                })
                .inner
                .on_hover_text("Move it one place earlier in time")
                .clicked();
            later = ui
                .add_enabled_ui(i + 1 < list.len(), |ui| {
                    ui.add_sized([w, 24.0], egui::Button::new("Later"))
                })
                .inner
                .on_hover_text("Move it one place later in time")
                .clicked();
        });
        if add {
            if let Some((tx, id)) = phases::add(self.doc.model(), "New phase", Some(p.id)) {
                let r = self.doc.commit(tx);
                self.report(r, "Phase added. Type its name.");
                if let Some(f) = self.phase_front() {
                    self.set_phase(f, id);
                    self.status =
                        "Phase added, and the view in front stands at it. Type its name.".into();
                }
            }
        }
        if up || later {
            match phases::shift(self.doc.model(), p.id, up) {
                Ok(tx) => {
                    let r = self.doc.commit(tx);
                    self.hidden_rev += 1;
                    self.report(
                        r,
                        format!("{} moved {}.", p.name, if up { "earlier" } else { "later" }),
                    );
                }
                Err(why) => self.status = why,
            }
        }
        if remove {
            match phases::remove(self.doc.model(), p.id) {
                Ok(tx) => {
                    let r = self.doc.commit(tx);
                    if self.phase3d == Some(p.id) {
                        self.phase3d = None;
                    }
                    self.hidden_rev += 1;
                    self.report(r, format!("{} removed.", p.name));
                }
                Err(why) => self.status = why,
            }
        }
    }

    /// The Element tab's Time rows: the phase the element is built in and
    /// the one it is taken down in, and how it reads from the view in
    /// front. Nothing for a project without phases, or what is not built.
    pub(super) fn element_time(&mut self, ui: &mut egui::Ui, id: ElementId) {
        let list = self.phase_list();
        let Some(e) = self
            .doc
            .model()
            .get(id)
            .filter(|e| phases::phased(e))
            .cloned()
        else {
            return;
        };
        if list.is_empty() {
            return;
        }
        let ids = {
            let mut ids: Vec<ElementId> = self
                .selection
                .all()
                .into_iter()
                .filter(|i| self.doc.model().get(*i).is_some_and(phases::phased))
                .collect();
            if !ids.contains(&id) {
                ids.push(id);
            }
            ids
        };
        let built = e
            .phase
            .built
            .and_then(|b| phases::position(&list, b))
            .unwrap_or(0);
        let gone = e.phase.gone.and_then(|g| phases::position(&list, g));
        let reads = self.phase_front().map(|f| {
            phases::reads(
                &list,
                e.phase,
                phases::stands_at(&list, self.phase_set(f)),
                &self.options_shown(f),
            )
        });
        let belongs = {
            let name = |o: u32| phases::option(self.doc.model(), o).map(|(_, o)| o.name.clone());
            match (
                e.phase.option.and_then(name),
                e.phase.gone_option.and_then(name),
            ) {
                (Some(o), _) => Some(o),
                (None, Some(o)) => Some(format!("Shared; {o} takes it down")),
                (None, None) if !phases::questions(self.doc.model()).is_empty() => {
                    Some("Shared by all options".into())
                }
                _ => None,
            }
        };
        let (mut new_built, mut new_gone) = (None, None);
        let mut new_option: Option<Option<u32>> = None;
        let mut back = false;
        let mut existing_here = false;
        let all_options: Vec<(String, u32)> = phases::questions(self.doc.model())
            .iter()
            .flat_map(|q| {
                let many = phases::questions(self.doc.model()).len() > 1;
                q.options.iter().map(move |o| {
                    (
                        if many {
                            format!("{}: {}", q.name, o.name)
                        } else {
                            o.name.clone()
                        },
                        o.id,
                    )
                })
            })
            .collect();
        crate::partstool::sfold_h(
            ui,
            "phases.element",
            "Phase",
            "When it is built and when it comes down. Whether it reads as existing, to take down or new is worked out from these and from the phase the view stands at.",
            |ui| {
                crate::props::row(ui, "Built in", |ui| {
                    egui::ComboBox::from_id_salt("phase.built")
                        .icon(crate::props::combo_icon)
                        .width(ui.available_width() - 8.0)
                        .selected_text(list[built].name.as_str())
                        .show_ui(ui, |ui| {
                            for (i, p) in list.iter().enumerate() {
                                // not after the phase it comes down in
                                let ok = gone.is_none_or(|g| i <= g);
                                if ui.add_enabled(ok, egui::Button::selectable(i == built, p.name.as_str())).clicked() {
                                    new_built = Some(p.id);
                                }
                            }
                        });
                });
                crate::props::row(ui, "Down in", |ui| {
                    egui::ComboBox::from_id_salt("phase.gone")
                        .icon(crate::props::combo_icon)
                        .width(ui.available_width() - 8.0)
                        .selected_text(gone.map_or("Never", |g| list[g].name.as_str()))
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(gone.is_none(), "Never").clicked() {
                                new_gone = Some(None);
                            }
                            for (i, p) in list.iter().enumerate() {
                                let ok = i >= built;
                                if ui.add_enabled(ok, egui::Button::selectable(gone == Some(i), p.name.as_str())).clicked() {
                                    new_gone = Some(Some(p.id));
                                }
                            }
                        });
                });
                if let Some(b) = &belongs {
                    crate::props::row(ui, "Option", |ui| {
                        egui::ComboBox::from_id_salt("phase.option")
                            .icon(crate::props::combo_icon)
                            .width(ui.available_width() - 8.0)
                            .selected_text(b.as_str())
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(e.phase.option.is_none(), "Shared by all options").clicked() {
                                    new_option = Some(None);
                                }
                                for (q, o) in &all_options {
                                    if ui.selectable_label(e.phase.option == Some(*o), q.as_str()).clicked() {
                                        new_option = Some(Some(*o));
                                    }
                                }
                            })
                            .response
                            .on_hover_text("The option it is built in alone, or shared by all of them. Drawn in the wrong option? Pick the right one here: the whole selection moves.");
                    });
                }
                self.groups_rows(ui, e.id);
                if e.phase.gone.is_some() {
                    let r = crate::props::action(ui, "", "Bring back", "It stands again in every phase and option, with the doors and windows that came down with it", true);
                    if r.clicked() {
                        back = true;
                    }
                }
                if reads == Some(Status::Existing) {
                    existing_here = true;
                }
                if let Some(s) = reads {
                    // "new" at the first phase read as a contradiction of
                    // Built in: Existing (MU100): say what it is there
                    let at = self.phase_front().map_or(0, |f| phases::stands_at(&list, self.phase_set(f)));
                    let text = match s {
                        Status::Absent => "not here in the view in front".to_string(),
                        Status::New if at == 0 => "as surveyed".to_string(),
                        Status::New => format!("new in {}", list[at].name),
                        s => s.label().to_string(),
                    };
                    let text = text.as_str();
                    crate::props::readout(
                        ui,
                        "In view",
                        text,
                        "Worked out from the two phases above and the phase the view in front stands at.",
                    );
                }
            },
        );
        if let Some(b) = new_built {
            let tx = phases::set(self.doc.model(), &ids, "Built in", |p| p.built = Some(b));
            let r = self.doc.commit(tx);
            self.report(r, "Built in changed.");
        }
        if let Some(g) = new_gone {
            let tx = phases::set(self.doc.model(), &ids, "Taken down in", |p| p.gone = g);
            let r = self.doc.commit(tx);
            self.report(r, "Taken down in changed.");
        }
        if let Some(to) = new_option {
            let tx = phases::move_to_option(self.doc.model(), &ids, to);
            let n = tx.ops.len();
            let r = self.doc.commit(tx);
            let whose = to
                .and_then(|o| phases::option(self.doc.model(), o).map(|(_, o)| o.name.clone()))
                .unwrap_or_else(|| "all options".into());
            self.report(
                r,
                format!("{n} element(s) now belong to {whose}. Nothing was redrawn."),
            );
        }
        if back {
            self.bring_back(&ids);
        }
        // existing, seen from a later phase: how a change to it is taken
        if existing_here {
            self.existing_choice(ui);
        }
    }

    /// The View tab's Time row: the phase the view (or the 3D window)
    /// stands at.
    pub(super) fn view_time(&mut self, ui: &mut egui::Ui, of: PhaseOf) {
        let list = self.phase_list();
        if list.is_empty() {
            return;
        }
        let at = phases::stands_at(&list, self.phase_set(of));
        let mut pick = None;
        crate::partstool::sfold_h(
            ui,
            "phases.view",
            "Phase",
            "The phase this view stands at: it shows the project as it is then. The tag beside the view's name sets the same thing.",
            |ui| {
                crate::props::row(ui, "View at", |ui| {
                    egui::ComboBox::from_id_salt("phase.view")
                        .icon(crate::props::combo_icon)
                        .width(ui.available_width() - 8.0)
                        .selected_text(list[at].name.as_str())
                        .show_ui(ui, |ui| {
                            for (i, p) in list.iter().enumerate() {
                                if ui.selectable_label(i == at, p.name.as_str()).clicked() {
                                    pick = Some(p.id);
                                }
                            }
                        });
                });
            },
        );
        if let Some(id) = pick {
            self.set_phase(of, id);
        }
        // the area in front standing at a phase of its own (MU133)
        if let PhaseOf::View(v) = of {
            let f = self.areas.focus;
            let here = self
                .area_phase(f)
                .filter(|p| phases::stands_at_id(&list, self.view_phase(v)) != Some(*p));
            let shows_it = self.areas.root.areas().get(f) == Some(&crate::areas::Shows::View(v));
            if let (Some(p), true) = (here, shows_it) {
                let name = list
                    .iter()
                    .find(|x| x.id == p)
                    .map(|x| x.name.clone())
                    .unwrap_or_default();
                // a drop-down as View at's above it: the name in full, and
                // back to the view's among its entries (a label and a
                // button ran past the card, then cut the name to "…")
                let own = phases::stands_at_id(&list, self.view_phase(v));
                let mut to: Option<Option<u32>> = None;
                crate::props::row(ui, "This area", |ui| {
                    egui::ComboBox::from_id_salt("phase.area")
                        .icon(crate::props::combo_icon)
                        .width(ui.available_width() - 8.0)
                        .selected_text(name.as_str())
                        .show_ui(ui, |ui| {
                            for x in &list {
                                if ui.selectable_label(x.id == p, x.name.as_str()).clicked() {
                                    to = Some((Some(x.id) != own).then_some(x.id));
                                }
                            }
                            ui.separator();
                            if ui.selectable_label(false, "As the view").clicked() {
                                to = Some(None);
                            }
                        })
                        .response
                        .on_hover_text("The area in front stands at a phase of its own. The sheets print the view's.");
                });
                if let Some(t) = to {
                    self.set_area_phase(f, t);
                    self.status = match t {
                        Some(_) => {
                            "This area moved through time; the view and its sheets stay.".into()
                        }
                        None => "This area is back at the view's phase.".into(),
                    };
                }
            }
        }
    }

    /// After a plain delete in a project with phases: how to take it down
    /// instead, when a later phase takes things down. Empty otherwise.
    pub(super) fn phase_delete_hint(&self) -> String {
        let list = self.phase_list();
        let Some(f) = self.phase_front() else {
            return String::new();
        };
        let at = phases::stands_at(&list, self.phase_set(f));
        match list.iter().skip(at + 1).find(|p| p.demolition) {
            Some(d) => format!(
                " It was built in {}, the phase this view stands at, so it is gone, not taken down (Ctrl+Z brings it back). To show it coming down in red dashes, stand the view at {} and delete it there.",
                list[at].name, d.name
            ),
            None => String::new(),
        }
    }

    /// Delete the selection from the phase the view in front stands at:
    /// what was built earlier is taken down, not lost. `None` when the
    /// project has no phases, so the plain delete goes on.
    pub(super) fn delete_in_phase(&mut self, ids: &[ElementId], name: &str) -> Option<()> {
        let list = self.phase_list();
        if list.is_empty() {
            return None;
        }
        let front = self.phase_front();
        let at = front.and_then(|f| self.phase_at(f));
        let option = front.and_then(|f| self.option_worked(f));
        let (mut tx, taken) = phases::remove_from_option(self.doc.model(), ids, at, option);
        if taken == 0 {
            return None;
        }
        tx.label = format!("Take down {name}");
        let stands = at
            .and_then(|a| list.iter().find(|p| p.id == a))
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let r = self.doc.commit(tx);
        let from = match option.and_then(|o| phases::option(self.doc.model(), o).map(|(_, o)| o.name.clone())) {
            Some(o) => format!("{o} takes down {name}: the other options keep it. Ctrl+Z to undo."),
            None => format!("Took down {name}: still there in the earlier phases, gone from {stands}. Ctrl+Z to undo."),
        };
        self.report(r, from);
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall_with_openings(app: &CreaBimApp, not: Option<ElementId>) -> ElementId {
        let m = app.doc.model();
        m.iter()
            .find(|e| {
                matches!(e.kind, ElementKind::Wall(_))
                    && Some(e.id) != not
                    && !m.dependents(e.id).is_empty()
            })
            .expect("a wall with something in it")
            .id
    }

    #[test]
    fn two_areas_of_one_plan_stand_at_phases_of_their_own() {
        // MU133: "i wanted to compare demolition with proposed of the same
        // floor plan but it didn't work. it switched to show me the same
        // thing" (Phase per area REV00, all A)
        let mut app = test_app();
        let tx = phases::start(app.doc.model(), true).unwrap();
        app.doc.commit(tx).unwrap();
        let view = app.active_view.expect("a view in front");
        app.areas.root = crate::areas::Node::split(
            true,
            0.5,
            crate::areas::Node::Area(crate::areas::Shows::View(view)),
            crate::areas::Node::Area(crate::areas::Shows::View(view)),
        );
        app.set_phase(PhaseOf::View(view), 3);
        let n = app.doc.model().len();
        // the existing wall comes down in Demolition
        let wall = wall_with_openings(&app, None);
        app.set_phase(PhaseOf::View(view), 2);
        assert!(app.delete_in_phase(&[wall], "wall").is_some());
        app.set_phase(PhaseOf::View(view), 3);
        assert_eq!(app.doc.model().len(), n);
        // the second area's tag picks Demolition: that area alone
        app.plan.pane_now = Some(1);
        app.area_now = 1;
        app.set_phase(PhaseOf::View(view), 2);
        assert_eq!(
            app.phase_at(PhaseOf::View(view)),
            Some(2),
            "seen at its own while it draws"
        );
        app.plan.pane_now = None;
        set_lens(None);
        assert_eq!(
            app.view(view).unwrap().phase,
            Some(3),
            "the view, and its sheets, stay"
        );
        assert_eq!((app.area_phase(0), app.area_phase(1)), (None, Some(2)));
        assert_eq!(
            app.phase_at(PhaseOf::View(view)),
            Some(3),
            "outside the area, the view's"
        );
        // each area draws its own: the wall coming down only in the second
        let drawn = |app: &mut CreaBimApp, area: usize| {
            set_lens(app.area_phase(area).map(|p| (view, p)));
            let d = app.drawing_live(view).unwrap();
            set_lens(None);
            d.items.iter().filter(|i| i.id() == wall).count()
        };
        assert_eq!(drawn(&mut app, 0), 0, "gone at Proposed");
        assert!(drawn(&mut app, 1) > 0, "coming down at Demolition");
        assert_eq!(
            drawn(&mut app, 0),
            0,
            "the first area's drawing is its own still"
        );
        // kept with the layout
        assert_eq!(app.shell.settings.area_phases.len(), 1);
        // its tag's own pick of the view's phase brings it back
        app.plan.pane_now = Some(1);
        app.area_now = 1;
        set_lens(Some((view, 2)));
        app.set_phase(PhaseOf::View(view), 3);
        app.plan.pane_now = None;
        assert_eq!(app.area_phase(1), None);
        assert_eq!(crate::phaselens::lens_of(view), None);
        // an area that shows something else starts at its view's phase
        app.plan.pane_now = Some(1);
        app.set_phase(PhaseOf::View(view), 1);
        app.plan.pane_now = None;
        set_lens(None);
        if let Some(s) = app.areas.root.get_mut(1) {
            *s = crate::areas::Shows::Model;
        }
        if let Some(s) = app.areas.root.get_mut(1) {
            *s = crate::areas::Shows::View(view);
        }
        app.prune_area_phases();
        assert_eq!(app.area_phase(1), Some(1), "the same view again: kept");
        if let Some(s) = app.areas.root.get_mut(1) {
            *s = crate::areas::Shows::Model;
        }
        app.prune_area_phases();
        assert!(app.areas.phases.is_empty(), "showing the 3D view: dropped");
    }

    #[test]
    fn a_view_stands_at_a_phase_and_delete_takes_down() {
        let mut app = test_app();
        assert!(
            app.phase_list().is_empty(),
            "no phases until they are started"
        );
        assert_eq!(app.phase_at(PhaseOf::Window3d), None);
        let wall = wall_with_openings(&app, None);
        assert!(
            app.delete_in_phase(&[wall], "wall").is_none(),
            "no phases: a plain delete"
        );
        let tx = phases::start(app.doc.model(), true).unwrap();
        app.doc.commit(tx).unwrap();
        // the 3D view at Proposed: what is drawn there is built there
        app.set_phase(PhaseOf::Window3d, 3);
        assert_eq!(app.phase_at(PhaseOf::Window3d), Some(3));
        app.work_in_phase(PhaseOf::Window3d);
        assert_eq!(app.doc.working_phase(), Some(3));
        // a view keeps its own
        let view = app.active_view.expect("a view in front");
        assert_eq!(app.phase_front(), Some(PhaseOf::View(view)));
        assert_eq!(
            app.phase_at(PhaseOf::View(view)),
            Some(1),
            "views start where the drawing is"
        );
        app.set_phase(PhaseOf::View(view), 2);
        assert_eq!(app.view(view).unwrap().phase, Some(2));
        assert!(app.doc.undo());
        assert_eq!(app.view(view).unwrap().phase, Some(1));
        // the existing wall, deleted from a view at Proposed, comes down in
        // Demolition
        app.set_phase(PhaseOf::View(view), 3);
        let n = app.doc.model().len();
        assert!(app.delete_in_phase(&[wall], "wall").is_some());
        assert_eq!(app.doc.model().len(), n, "nothing is lost");
        assert_eq!(app.doc.model().get(wall).unwrap().phase.gone, Some(2));
        assert!(app.status.contains("Took down"), "{}", app.status);
        // from Existing, where it was built, delete is delete
        app.set_phase(PhaseOf::View(view), 1);
        let other = wall_with_openings(&app, Some(wall));
        assert!(app.delete_in_phase(&[other], "wall").is_none());
    }

    #[test]
    fn a_view_shows_an_option_and_work_goes_into_it() {
        let mut app = test_app();
        let tx = phases::start(app.doc.model(), true).unwrap();
        app.doc.commit(tx).unwrap();
        let view = app.active_view.expect("a view in front");
        let of = PhaseOf::View(view);
        app.set_phase(of, 3);
        let (tx, [a, b]) = phases::add_question(app.doc.model(), "Extension").unwrap();
        app.doc.commit(tx).unwrap();
        assert_eq!(app.options_shown(of), [a], "the pick");
        assert_eq!(
            app.option_worked(of),
            None,
            "shared until an option is worked in"
        );
        // B shown and worked in: the view keeps it, what is drawn is B's
        app.set_option(of, b, true);
        assert_eq!(app.view(view).unwrap().options, [b]);
        assert_eq!(app.option_worked(of), Some(b));
        app.work_in_phase(of);
        assert_eq!(app.doc.working_option(), Some(b));
        // another view still shows the pick, so B is not worked in there
        assert_eq!(app.option_worked(PhaseOf::Window3d), None);
        // B takes a shared wall down; with A shown it still stands
        let wall = wall_with_openings(&app, None);
        assert!(app.delete_in_phase(&[wall], "wall").is_some());
        assert_eq!(
            app.doc.model().get(wall).unwrap().phase.gone_option,
            Some(b)
        );
        assert!(
            app.status.contains("the other options keep it"),
            "{}",
            app.status
        );
        // what is selected and not in the option shown is let go
        let own = wall_with_openings(&app, Some(wall));
        let tx = phases::move_to_option(app.doc.model(), &[own], Some(b));
        app.doc.set_hold_shared(false);
        app.doc.commit(tx).unwrap();
        app.selection.extend(vec![own], false);
        // back to the pick: nothing is kept on the view
        app.set_option(of, a, false);
        assert!(
            !app.selection.contains(own),
            "B's wall is not in a view showing A"
        );
        assert!(app.view(view).unwrap().options.is_empty());
        assert_eq!(app.option_worked(of), None);
    }
}
