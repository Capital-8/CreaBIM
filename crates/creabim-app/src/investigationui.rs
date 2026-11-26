//! The Properties of a borehole, trial pit or probe: what it is and where
//! its ground level comes from, then its log as the report gives it
//! (layers, water, tests), with a box to paste the report's rows into.
//! Laid out as the house Properties (AMD-018): ticks with a word, facts on
//! the Worked out plate, and each log a list with the picked row's fields
//! under it.

use crate::partstool::{pick, picked, soil_picture};
use crate::props::Worked;
use creabim_core::investigation::{
    FieldTest, LogLayer, ProbeKind, SoilKind, TestKind, WaterStrike,
};
use creabim_core::site::{Site, SiteForm};
use creabim_core::Model;
use eframe::egui;
use std::sync::Mutex;

/// An investigation point whose log was asked for, by its position.
static LOG: Mutex<Option<glam::DVec2>> = Mutex::new(None);

/// The investigation point whose log was asked for since last time.
pub(crate) fn take_log() -> Option<glam::DVec2> {
    LOG.lock().ok().and_then(|mut r| r.take())
}

/// A depth in metres as logs give them, in a full row.
fn depth(ui: &mut egui::Ui, name: &str, v: &mut f64, hi: f64, tip: &str) -> bool {
    crate::props::row(ui, name, |ui| {
        crate::props::fill(
            ui,
            egui::DragValue::new(v)
                .range(0.0..=hi)
                .speed(0.02)
                .fixed_decimals(2)
                .suffix(" m"),
        )
        .on_hover_text(tip)
        .changed()
    })
}

/// A borehole's, trial pit's or probe's fields. Returns the undo label of
/// a change, or "".
pub fn probe_fields(ui: &mut egui::Ui, model: &Model, s: &mut Site) -> &'static str {
    let mut label = "";
    let shown = s.clone();
    let SiteForm::Investigation(p) = &mut s.form else {
        return label;
    };
    crate::props::row(ui, "Kind", |ui| {
        egui::ComboBox::from_id_salt("probe-kind")
            .icon(crate::props::combo_icon)
            .selected_text(p.kind.label())
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for k in ProbeKind::ALL {
                    if ui.selectable_value(&mut p.kind, k, k.label()).changed() {
                        label = "Investigation kind";
                    }
                }
            });
    });
    // its ground level: the report's, or the survey's under it
    let survey = shown
        .as_probe()
        .and_then(|q| creabim_geom::investigation::survey_level(model, q));
    let mut own = p.ground.is_some();
    if crate::props::check(ui, "Ground", &mut own, "Report's")
        .on_hover_text("Type the ground level the report gives. Unticked, it takes the surveyed ground's height here")
        .changed()
    {
        p.ground = own.then(|| survey.unwrap_or(0.0));
        label = "Ground level";
    }
    if let Some(g) = p.ground.as_mut() {
        if crate::props::row(ui, "Ground level", |ui| {
            crate::props::fill(
                ui,
                egui::DragValue::new(g)
                    .speed(0.01)
                    .fixed_decimals(2)
                    .suffix(" m"),
            )
            .on_hover_text("As the report gives it, on the project's levels")
            .changed()
        }) {
            label = "Ground level";
        }
        if let Some(sv) = survey {
            let d = *g - sv;
            // the survey's level here, in the warn colour when far off
            crate::props::worked_later(
                ui,
                "site.element",
                &[Worked::new("Survey", creabim_core::site_level(model, sv))
                    .warn_if(d.abs() > 0.15)],
            );
            if d.abs() > 0.15 {
                crate::props::warning(
                    ui,
                    format!(
                        "The survey is {:.2} m {} here. A difference this big usually means the report uses another datum.",
                        d.abs(),
                        if d > 0.0 { "lower" } else { "higher" }
                    ),
                    None,
                );
            }
        }
    } else {
        match survey {
            Some(sv) => crate::props::worked_later(
                ui,
                "site.element",
                &[Worked::new("Ground", creabim_core::site_level(model, sv))],
            ),
            None => crate::props::info(
                ui,
                "No terrain under it: its ground level is its level's.",
                "site",
            ),
        }
    }
    if crate::props::row(ui, "Depth", |ui| {
        crate::props::fill(
            ui,
            egui::DragValue::new(&mut p.depth)
                .range(0.1..=300.0)
                .speed(0.05)
                .fixed_decimals(2)
                .suffix(" m"),
        )
        .on_hover_text("How deep it went")
        .changed()
    }) {
        label = "Investigation depth";
    }
    for (name, text, hint) in [
        (
            "Method",
            &mut p.method,
            "How it was made: cable percussion, window sample, machine dug",
        ),
        ("Date", &mut p.date, "When, as the report writes it"),
        (
            "Report",
            &mut p.report,
            "The report it comes from: its reference and the firm",
        ),
    ] {
        if crate::props::row(ui, name, |ui| {
            crate::props::text_field(
                ui,
                egui::TextEdit::singleline(text).hint_text(hint),
                f32::INFINITY,
            )
            .changed()
        }) {
            label = "Investigation details";
        }
    }
    if let Err(e) = p.validate() {
        crate::props::warning(ui, format!("Check the log: {e}."), None);
    }
    if crate::props::action(
        ui,
        "Log",
        "Borehole log",
        "Open its log: its layers drawn with their soil symbols and descriptions, its water and tests, to place on a sheet",
        true,
    )
    .clicked()
    {
        if let Ok(mut r) = LOG.lock() {
            *r = Some(p.position);
        }
    }

    // ---- layers: a list from the top down, the picked layer under it
    let title = match p.layers.len() {
        0 => "Layers".to_string(),
        n => format!("Layers ({n})"),
    };
    if crate::props::sub_fold(ui, "probe.layers", &title, true) {
        let rows: Vec<(Option<String>, String)> = p
            .layers
            .iter()
            .map(|l| {
                // its name (or soil) and the depth it goes down to; its
                // soil is the row's hatch
                let r = (
                    Some(format!("{:.2} m", l.to)),
                    if l.unit.is_empty() {
                        l.soil.label().to_string()
                    } else {
                        l.unit.clone()
                    },
                );
                r
            })
            .collect();
        let mut at = picked(ui, "probe.layers").min(rows.len().saturating_sub(1));
        let pic = |pa: &egui::Painter, r: egui::Rect, k: usize, ink: egui::Color32| {
            soil_picture(pa, r, k, ink);
        };
        let act = crate::props::item_list_drawn(
            ui,
            "probe.layers",
            &rows,
            &mut at,
            rows.len().clamp(1, 6),
            (true, false, !rows.is_empty()),
            Some(&pic),
        );
        if act.add {
            let from = p.layers.last().map_or(0.0, |l| l.to);
            let to = if p.depth > from + 0.05 {
                p.depth
            } else {
                from + 1.0
            };
            p.layers.push(LogLayer {
                to,
                soil: p
                    .layers
                    .last()
                    .map_or(SoilKind::Topsoil, |_| SoilKind::Clay),
                ..Default::default()
            });
            at = p.layers.len() - 1;
            label = "Add layer";
        } else if act.remove && at < p.layers.len() {
            p.layers.remove(at);
            at = at.saturating_sub(1);
            label = "Remove layer";
        }
        pick(ui, "probe.layers", at);
        if let Some(l) = p.layers.get_mut(at) {
            if depth(
                ui,
                "Down to",
                &mut l.to,
                300.0,
                "The foot of this layer, from the ground",
            ) {
                label = "Layer depth";
            }
            crate::props::row(ui, "Soil", |ui| {
                egui::ComboBox::from_id_salt("soil")
                    .icon(crate::props::combo_icon)
                    .selected_text(l.soil.label())
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for k in SoilKind::ALL {
                            if ui.selectable_value(&mut l.soil, k, k.label()).changed() {
                                label = "Layer soil";
                            }
                        }
                    });
            });
            if crate::props::row(ui, "Name", |ui| {
                crate::props::text_field(
                    ui,
                    egui::TextEdit::singleline(&mut l.unit).hint_text("GT1"),
                    f32::INFINITY,
                )
                .on_hover_text("The report's name for this layer across the site (GT1, Stratum B)")
                .changed()
            }) {
                label = "Layer name";
            }
            // its name on a line of its own and the words across the whole
            // block, as the paste box under it: in the field column a
            // description broke every two words (MU124)
            ui.label(egui::RichText::new("As logged").color(ui.visuals().weak_text_color()));
            let changed = crate::props::fill(
                ui,
                egui::TextEdit::multiline(&mut l.description)
                    .desired_rows(2)
                    .hint_text("As in the report"),
            )
            .changed();
            if changed {
                label = "Layer description";
                // the soil follows the words while it was the one they named
                if let Some(k) = SoilKind::from_description(&l.description) {
                    l.soil = k;
                }
            }
        } else {
            crate::props::empty(
                ui,
                "None yet: + adds a layer, or paste the report's rows below.",
            );
        }
    }
    // pasting the report's rows
    if crate::props::sub_fold(ui, "probe.paste", "Paste from the report", false) {
        let id = ui.id().with("probe-paste");
        let mut text: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_default());
        crate::props::info(
            ui,
            "One layer per line: its depths (\"0.25 1.05\" or just the foot \"1.05\"), then its description. A code like GT2 first becomes the layer's name; the soil is the word the description names.",
            "site",
        );
        crate::props::fill(
            ui,
            egui::TextEdit::multiline(&mut text)
                .desired_rows(4)
                .hint_text(
                "0.00  0.25  GT1 TOPSOIL: dark brown sandy loam\n0.25  1.05  GT2 MADE GROUND: …",
            ),
        );
        if crate::props::action(
            ui,
            "",
            "Read rows",
            "Replace the layers with the rows above",
            !text.trim().is_empty(),
        )
        .clicked()
        {
            let n = p.read_layers(&text);
            if n > 0 {
                label = "Read layers";
                text.clear();
            }
        }
        ui.data_mut(|d| d.insert_temp(id, text));
    }

    // ---- water: each strike, and where it rose to
    let title = match p.water.len() {
        0 => "Water".to_string(),
        n => format!("Water ({n})"),
    };
    if crate::props::sub_fold(ui, "probe.water", &title, !p.water.is_empty()) {
        let rows: Vec<(Option<String>, String)> = p
            .water
            .iter()
            .enumerate()
            .map(|(i, w)| {
                (
                    Some(format!("{:.2} m", w.depth)),
                    format!("Strike {}", i + 1),
                )
            })
            .collect();
        let mut at = picked(ui, "probe.water").min(rows.len().saturating_sub(1));
        let act = crate::props::item_list(
            ui,
            "probe.water",
            &rows,
            &mut at,
            rows.len().clamp(1, 4),
            (true, false, !rows.is_empty()),
        );
        if act.add {
            let d = p.water.last().map_or(p.depth / 2.0, |w| w.depth + 1.0);
            p.water.push(WaterStrike {
                depth: d.min(p.depth),
                ..Default::default()
            });
            at = p.water.len() - 1;
            label = "Add water strike";
        } else if act.remove && at < p.water.len() {
            p.water.remove(at);
            at = at.saturating_sub(1);
            label = "Remove water strike";
        }
        pick(ui, "probe.water", at);
        if let Some(w) = p.water.get_mut(at) {
            if depth(
                ui,
                "Met at",
                &mut w.depth,
                300.0,
                "Where it was struck, from the ground",
            ) {
                label = "Water strike";
            }
            let mut rose = w.rest.is_some();
            if crate::props::check(ui, "Rose", &mut rose, "After waiting")
                .on_hover_text("Where it stood after waiting, if the report says")
                .changed()
            {
                w.rest = rose.then_some(w.depth);
                label = "Water strike";
            }
            if let Some(r) = w.rest.as_mut() {
                if depth(ui, "Rose to", r, 300.0, "Where it stood, from the ground") {
                    label = "Water strike";
                }
            }
        } else {
            crate::props::empty(ui, "No water met.");
        }
    }

    // ---- tests: each at its depth, its result
    let title = match p.tests.len() {
        0 => "Tests".to_string(),
        n => format!("Tests ({n})"),
    };
    if crate::props::sub_fold(ui, "probe.tests", &title, !p.tests.is_empty()) {
        let rows: Vec<(Option<String>, String)> = p
            .tests
            .iter()
            .map(|t| {
                // the depth names it; the result as the report writes it
                let v = if t.text.is_empty() {
                    format!("{}", t.value)
                } else {
                    t.text.clone()
                };
                (Some(v), format!("{:.2} m", t.depth))
            })
            .collect();
        let mut at = picked(ui, "probe.tests").min(rows.len().saturating_sub(1));
        let act = crate::props::item_list(
            ui,
            "probe.tests",
            &rows,
            &mut at,
            rows.len().clamp(1, 5),
            (true, false, !rows.is_empty()),
        );
        if act.add {
            let (d, kind) = p
                .tests
                .last()
                .map_or((1.0, TestKind::Spt), |t| (t.depth + 1.0, t.kind));
            p.tests.push(FieldTest {
                depth: d.min(p.depth),
                kind,
                ..Default::default()
            });
            at = p.tests.len() - 1;
            label = "Add test";
        } else if act.remove && at < p.tests.len() {
            p.tests.remove(at);
            at = at.saturating_sub(1);
            label = "Remove test";
        }
        pick(ui, "probe.tests", at);
        if let Some(t) = p.tests.get_mut(at) {
            crate::props::row(ui, "Test", |ui| {
                egui::ComboBox::from_id_salt("test-kind")
                    .icon(crate::props::combo_icon)
                    .selected_text(t.kind.label())
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for k in TestKind::ALL {
                            if ui.selectable_value(&mut t.kind, k, k.label()).changed() {
                                label = "Test kind";
                            }
                        }
                    });
            });
            if depth(ui, "At", &mut t.depth, 300.0, "Its depth, from the ground") {
                label = "Test depth";
            }
            let unit = t.kind.unit();
            if crate::props::row(ui, "Result", |ui| {
                crate::props::fill(
                    ui,
                    egui::DragValue::new(&mut t.value)
                        .range(0.0..=1000.0)
                        .speed(0.5)
                        .max_decimals(1),
                )
                .on_hover_text(unit)
                .changed()
            }) {
                label = "Test result";
            }
            if crate::props::row(ui, "As written", |ui| {
                crate::props::text_field(
                    ui,
                    egui::TextEdit::singleline(&mut t.text).hint_text("50/220 mm"),
                    f32::INFINITY,
                )
                .on_hover_text(
                    "The result as the report writes it, when the number alone doesn't say it all",
                )
                .changed()
            }) {
                label = "Test result";
            }
        } else {
            crate::props::empty(ui, "No tests: + adds one, an SPT or a vane.");
        }
    }

    if crate::props::sub_fold(ui, "probe.notes", "Notes", !p.notes.is_empty())
        && crate::props::fill(
            ui,
            egui::TextEdit::multiline(&mut p.notes)
                .desired_rows(2)
                .hint_text("Anything else the report says about it"),
        )
        .changed()
    {
        label = "Investigation notes";
    }
    if label == "Layer depth" {
        // keep the layers in order as depths are dragged past each other
        p.tidy();
    }
    label
}
