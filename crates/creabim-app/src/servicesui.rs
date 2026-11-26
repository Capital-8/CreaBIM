//! The Properties of a drain or a tank: what it is, its size, how it is
//! laid, its levels as the ground gives them, and its dig.

use crate::partstool::{
    chamber_picture, metres, num, percent, pick, picked, sfold, sfold_d, sfold_h,
};
use creabim_core::services::{DrainUse, TankUse};
use creabim_core::site::{Site, SiteForm};
use creabim_core::Model;
use eframe::egui;
use std::sync::Mutex;

/// A drain whose long section was asked for, by its run.
static LONG_SECTION: Mutex<Option<Vec<glam::DVec2>>> = Mutex::new(None);

/// The drain whose long section was asked for since last time, if any.
pub(crate) fn take_long_section() -> Option<Vec<glam::DVec2>> {
    LONG_SECTION.lock().ok().and_then(|mut r| r.take())
}

/// A drain's or a tank's fields. Returns the undo label of a change, or "".
pub fn service_fields(ui: &mut egui::Ui, model: &Model, s: &mut Site) -> &'static str {
    let mut label = "";
    let units = model.units();
    let shown = s.clone();
    // what digging it in takes, from it as it stands
    let dig = match &shown.form {
        SiteForm::Drain(sd) => {
            dig_facts(model, creabim_geom::services::drain_dig(model, &shown, sd))
        }
        SiteForm::Tank(st) => dig_facts(model, creabim_geom::services::tank_dig(model, &shown, st)),
        _ => vec![],
    };
    match &mut s.form {
        SiteForm::Drain(d) => {
            sfold_h(
                ui,
                "drain.pipe",
                "Pipe",
                "Its dig is measured apart from the earthworks: the ground is put back as it was.",
                |ui| {
                    crate::props::Combo::salted("Carries", "drain-use")
                        .selected_text(d.service.label())
                        .show_ui(ui, |ui| {
                            for u in DrainUse::ALL {
                                if ui.selectable_value(&mut d.service, u, u.label()).changed() {
                                    label = "Drain use";
                                }
                            }
                        });
                    let mut mm = d.diameter * 1000.0;
                    if crate::props::row(ui, "Diameter", |ui| {
                        crate::props::fill(ui, crate::units::model_mm(&mut mm).range(50.0..=1500.0))
                            .on_hover_text(
                                "The pipe's bore: 100 mm for a house's foul drain, 150 mm for more",
                            )
                            .changed()
                    }) {
                        d.diameter = mm / 1000.0;
                        label = "Pipe size";
                    }
                    let ratio = format!(
                        "How steeply it falls all along. Now {}: 1.25% is 1 in 80, 2.5% is 1 in 40",
                        d.gradient_text()
                    );
                    if percent(ui, "Fall", &mut d.fall, 20.0, &ratio) {
                        label = "Drain fall";
                    }
                    if num(ui, "Depth", &mut d.depth, 0.0, 10.0) {
                        label = "Drain depth";
                    }
                    if num(ui, "Trench", &mut d.trench, 0.2, 5.0) {
                        label = "Trench width";
                    }
                    if num(ui, "Bed", &mut d.bed, 0.0, 1.0) {
                        label = "Drain bed";
                    }
                    crate::props::worked_later(ui, "drain.dig", &dig);
                },
            );
            // its levels, chamber by chamber: a list, and the picked one's
            let SiteForm::Drain(sd) = &shown.form else {
                return label;
            };
            let lv = creabim_geom::services::drain_levels(model, &shown, sd);
            let n = lv.cover.len();
            let title = format!("Chambers ({n})");
            sfold_d(ui, "drain.chambers", &title, true, |ui| {
                if crate::props::check(ui, "Chambers", &mut d.chambers, "At each bend")
                    .on_hover_text("An inspection chamber at its head, at each bend and at its end")
                    .changed()
                {
                    label = "Drain chambers";
                }
                if d.chambers && num(ui, "Size", &mut d.chamber, 0.3, 3.0) {
                    label = "Chamber size";
                }
                let name = |k: usize| format!("{}{}", sd.service.letter(), k + 1);
                let rows: Vec<(Option<String>, String)> = (0..n)
                    .map(|k| (Some(creabim_core::site_level(model, lv.invert[k])), name(k)))
                    .collect();
                let mut at = picked(ui, "drain.chambers").min(n.saturating_sub(1));
                let pic = |pa: &egui::Painter, r: egui::Rect, _k: usize, ink: egui::Color32| {
                    chamber_picture(pa, r, ink);
                };
                crate::props::item_list_drawn(
                    ui,
                    "drain.chambers",
                    &rows,
                    &mut at,
                    rows.len().clamp(1, 6),
                    (false, false, false),
                    Some(&pic),
                );
                pick(ui, "drain.chambers", at);
                if let Some((least, at)) = lv.least_cover(sd) {
                    if least < 0.0 {
                        crate::props::warning(
                            ui,
                            format!(
                                "It comes out of the ground at {}: lay it deeper or flatter.",
                                name(at)
                            ),
                            None,
                        );
                    } else if least < 0.6 {
                        crate::props::warning(
                            ui,
                            format!(
                                "Only {} over the pipe at {}: under a drive it may need protecting.",
                                units.length(least),
                                name(at)
                            ),
                            None,
                        );
                    }
                }
                if crate::props::action(
                    ui,
                    "Drawing",
                    "Long section",
                    "Open its long section: the run unfolded with the ground over it and its levels tabled, to place on a sheet",
                    true,
                )
                .clicked()
                {
                    if let Ok(mut r) = LONG_SECTION.lock() {
                        *r = Some(sd.path.clone());
                    }
                }
                // the picked chamber's levels: its sub-fold, its plate
                if at < n && crate::props::sub_fold(ui, "drain.chamber", &name(at), true) {
                    let (c, i) = (lv.cover[at], lv.invert[at]);
                    crate::props::worked_later(
                        ui,
                        "site.facts",
                        &[crate::props::Worked::new(
                            "Cover",
                            creabim_core::site_level(model, c),
                        )],
                    );
                    crate::props::worked_later(
                        ui,
                        "site.facts",
                        &[crate::props::Worked::new(
                            "Invert",
                            creabim_core::site_level(model, i),
                        )],
                    );
                    crate::props::worked_later(
                        ui,
                        "site.facts",
                        &[crate::props::Worked::new("Depth", units.length(c - i))],
                    );
                    crate::props::flush_later(ui);
                }
            });
        }
        SiteForm::Tank(t) => {
            let SiteForm::Tank(st) = &shown.form else {
                return label;
            };
            let lv = creabim_geom::services::tank_levels(model, &shown, st);
            let w = crate::props::Worked::new;
            let mut facts = vec![
                w("Holds", format!("{:.2} m³", st.capacity())),
                w("Ground", creabim_core::site_level(model, lv.ground)),
                w("Top", creabim_core::site_level(model, lv.top)),
                w("Floor", creabim_core::site_level(model, lv.floor)),
            ];
            facts.extend(dig.iter().cloned());
            sfold(ui, "tank.size", "Tank", |ui| {
                crate::props::Combo::salted("Kind", "tank-use")
                    .selected_text(t.service.label())
                    .show_ui(ui, |ui| {
                        for u in TankUse::ALL {
                            if ui.selectable_value(&mut t.service, u, u.label()).changed() {
                                label = "Tank use";
                            }
                        }
                    });
                if num(ui, "Cover", &mut t.cover, 0.0, 10.0) {
                    label = "Tank cover";
                }
                if num(ui, "Walls", &mut t.wall, 0.0, 1.0) {
                    label = "Tank walls";
                }
                if num(ui, "Space", &mut t.working, 0.0, 5.0) {
                    label = "Working space";
                }
                crate::props::worked_later(ui, "tank.facts", &facts);
            });
        }
        _ => {}
    }
    label
}

/// A tank's overall size and which way it is turned: its Element rows.
pub fn tank_size(ui: &mut egui::Ui, s: &mut Site) -> &'static str {
    let mut label = "";
    let SiteForm::Tank(t) = &mut s.form else {
        return label;
    };
    if num(ui, "Length", &mut t.length, 0.3, 50.0) {
        label = "Tank length";
    }
    if num(ui, "Width", &mut t.width, 0.3, 50.0) {
        label = "Tank width";
    }
    if num(ui, "Depth", &mut t.depth, 0.2, 20.0) {
        label = "Tank depth";
    }
    if crate::props::row(ui, "Turn", |ui| {
        crate::props::fill(
            ui,
            egui::DragValue::new(&mut t.angle)
                .range(-360.0..=360.0)
                .speed(1.0)
                .max_decimals(1)
                .suffix("°"),
        )
        .changed()
    }) {
        label = "Tank turn";
    }
    label
}

/// What digging it in takes, kept apart from the earthworks: rows of the
/// plate at the foot of the block that lays it (AMD-018).
fn dig_facts(model: &Model, d: creabim_geom::services::Dig) -> Vec<crate::props::Worked> {
    let units = model.units();
    let w = crate::props::Worked::new;
    vec![
        w("Dug", units.volume(d.dug)),
        w("Deepest", units.length(d.deepest)),
        w("Bed", units.volume(d.bed)),
        w("Backfill", units.volume(d.backfill)),
        w("Take away", units.volume(d.dug - d.backfill)),
    ]
}

/// A path's fields: its surface, width, edging and make-up, and what
/// laying it takes.
pub fn walk_fields(ui: &mut egui::Ui, model: &Model, s: &mut Site) -> &'static str {
    let mut label = "";
    let units = model.units();
    let SiteForm::Walk(w) = &mut s.form else {
        return label;
    };
    sfold_h(
        ui,
        "path.section",
        "Section",
        "It lies on the ground as the works leave it, and is dug only as deep as its make-up.",
        |ui| {
            crate::props::Combo::salted("Surface", "path-surface")
                .selected_text(w.surface.label())
                .show_ui(ui, |ui| {
                    for u in creabim_core::walk::Surface::ALL {
                        if ui.selectable_value(&mut w.surface, u, u.label()).changed() {
                            label = "Path surface";
                        }
                    }
                });
            if num(ui, "Width", &mut w.width, 0.3, 10.0) {
                label = "Path width";
            }
            if num(ui, "Edging", &mut w.edging, 0.0, 0.5) {
                label = "Path edging";
            }
            if num(ui, "Depth", &mut w.thickness, 0.0, 1.0) {
                label = "Path depth";
            }
            let mut q = vec![crate::props::Worked::new("Surface", units.area(w.area()))];
            if w.edging > 0.0 {
                q.push(crate::props::Worked::new(
                    "Edging",
                    metres(2.0 * w.length()),
                ));
            }
            q.push(crate::props::Worked::new(
                "Dug",
                units.volume(w.area() * w.thickness),
            ));
            crate::props::worked_later(ui, "path.facts", &q);
        },
    );
    sfold(ui, "path.line", "Line", |ui| {
        if num(ui, "Radius", &mut w.radius, 0.0, 200.0) {
            label = "Path bend radius";
        }
    });
    label
}
