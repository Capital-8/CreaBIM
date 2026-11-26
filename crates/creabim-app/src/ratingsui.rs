//! Declared ratings (Common properties REV00, owner: A on all five): fire,
//! acoustic, U-value, load-bearing and external, for walls, slabs, roofs,
//! columns and beams, on their type with an own value per element; and
//! External for doors and windows, whose other ratings stay in their
//! construction. Information only: carried to schedules and IFC as given.

use super::*;
use creabim_core::common::{self, From};
use creabim_core::construction::Ratings;

/// A three-way switch: worked out, yes, no.
fn auto_yes_no(ui: &mut egui::Ui, label: &str, tip: &str, v: &mut Option<bool>) -> bool {
    let mut pick = match v {
        None => 0u8,
        Some(true) => 1,
        Some(false) => 2,
    };
    let options = [(0, "Auto", "Worked out"), (1, "Yes", ""), (2, "No", "")];
    // the switch needs room for "Auto" in each third: narrower, it goes
    // under its label at full width rather than read "…"
    let changed = if ui.available_width() < 250.0 {
        ui.vertical(|ui| {
            let muted = ui.visuals().widgets.noninteractive.fg_stroke.color;
            ui.add(egui::Label::new(egui::RichText::new(label).color(muted)).truncate())
                .on_hover_text(tip);
            crate::props::segment(ui, &mut pick, &options)
        })
        .inner
    } else {
        crate::props::row_tip(ui, label, tip, |ui| {
            crate::props::segment(ui, &mut pick, &options)
        })
    };
    if changed {
        *v = match pick {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        };
    }
    changed
}

/// A small line under a field, saying where a value comes from.
fn why(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add(egui::Label::new(egui::RichText::new(text.into()).small().weak()).wrap());
}

fn yes_no(v: bool) -> &'static str {
    if v {
        "yes"
    } else {
        "no"
    }
}

/// The text fields of a ratings block: fire, acoustic and U-value. `base`
/// is what an empty field stands for (the type's), shown as its hint.
fn declared_fields(
    ui: &mut egui::Ui,
    r: &mut Ratings,
    base: Option<&Ratings>,
) -> Option<&'static str> {
    let mut label = None;
    let hint = |own: &str, b: Option<&str>| -> String {
        match b.filter(|b| !b.trim().is_empty()) {
            Some(b) => b.to_string(),
            None => own.to_string(),
        }
    };
    let tip_of = |what: &str| match base {
        Some(_) => format!("{what}, as declared. Empty: its type's (shown grey)."),
        None => format!("{what}, as declared"),
    };
    let text = |ui: &mut egui::Ui, name: &str, tip: &str, v: &mut String, h: String| {
        crate::props::row_tip(ui, name, tip, |ui| {
            crate::props::text_field(
                ui,
                egui::TextEdit::singleline(v).hint_text(h),
                f32::INFINITY,
            )
            .changed()
        })
    };
    if text(
        ui,
        "Fire",
        &tip_of("Fire resistance"),
        &mut r.fire,
        hint("REI 60", base.map(|b| b.fire.as_str())),
    ) {
        label = Some("Fire rating");
    }
    if text(
        ui,
        "Acoustic",
        &tip_of("Sound reduction"),
        &mut r.acoustic,
        hint("Rw 52 dB", base.map(|b| b.acoustic.as_str())),
    ) {
        label = Some("Acoustic rating");
    }
    let mut u = r.u_value.map(|v| format!("{v}")).unwrap_or_default();
    let base_u = base.and_then(|b| b.u_value).map(|v| format!("{v} W/m²K"));
    let changed = crate::props::row_tip(
        ui,
        "U-value",
        &tip_of("Thermal transmittance, W/m²K"),
        |ui| {
            crate::props::text_field(
                ui,
                egui::TextEdit::singleline(&mut u)
                    .hint_text(base_u.unwrap_or_else(|| "0.28 W/m²K".into())),
                f32::INFINITY,
            )
            .changed()
        },
    );
    if changed {
        let t = u.trim().trim_end_matches("W/m²K").trim().replace(',', ".");
        r.u_value = t.parse::<f64>().ok().filter(|v| v.is_finite() && *v > 0.0);
        label = Some("U-value");
    }
    label
}

impl CreaBimApp {
    /// Properties, after the Material fold: the element's declared ratings,
    /// its own over its type's. A door or window gets External here; its
    /// other ratings are in its construction.
    pub(super) fn ratings_section(
        &mut self,
        ui: &mut egui::Ui,
        e: &mut Element,
    ) -> Option<&'static str> {
        if !common::rated(&e.kind) {
            return None;
        }
        let model = self.doc.model();
        let opening = matches!(e.kind, ElementKind::Opening(_));
        let base = common::type_ratings(model, e).cloned();
        // what Auto gives: worked out, with the element's own switches off
        let mut bare = e.clone();
        if let Some(r) = bare.ratings.as_deref_mut() {
            r.external = None;
            r.load_bearing = None;
        }
        let auto = common::common(model, &bare);
        let mut own = e.ratings.as_deref().cloned().unwrap_or_default();
        let open = self.prop_header(ui, "ratings", "Declared ratings", false, None);
        let mut label = None;
        crate::props::fold(ui, open, |ui| {
            if !opening {
                label = declared_fields(ui, &mut own, base.as_ref());
                let column_or_beam =
                    matches!(e.kind, ElementKind::Column(_) | ElementKind::Beam(_));
                if !matches!(e.kind, ElementKind::Roof(_)) {
                    if auto_yes_no(ui, "Load-bearing", "Load-bearing: it carries what is above. Auto: its type's, or from its layers", &mut own.load_bearing) {
                        label = Some("Load-bearing");
                    }
                    if own.load_bearing.is_none() {
                        match auto.load_bearing {
                            Some((v, From::Type)) => {
                                why(ui, format!("Auto: its type says {}.", yes_no(v)))
                            }
                            Some((v, _)) if column_or_beam => why(
                                ui,
                                format!(
                                    "Auto: {}, as a {}.",
                                    yes_no(v),
                                    if matches!(e.kind, ElementKind::Column(_)) {
                                        "column"
                                    } else {
                                        "beam"
                                    }
                                ),
                            ),
                            Some((true, _)) => {
                                why(ui, "Auto: yes, its core is a bearing material.")
                            }
                            Some((false, _)) => {
                                why(ui, "Auto: no, its core is not a bearing material.")
                            }
                            None => why(
                                ui,
                                "Auto: not known for a frame, so not written: set Yes or No.",
                            ),
                        }
                    }
                }
            }
            if auto_yes_no(
                ui,
                "External",
                "External: one side faces the outside. Auto: worked out from the rooms",
                &mut own.external,
            ) {
                label = Some("External");
            }
            if own.external.is_none() {
                match (&e.kind, auto.external) {
                    (ElementKind::Roof(_), _) => why(ui, "Auto: a roof is external."),
                    (ElementKind::Opening(_), Some((v, _))) => {
                        why(ui, format!("Auto: {}, as the wall it is in.", yes_no(v)))
                    }
                    (ElementKind::Wall(_), Some((true, _))) => {
                        why(ui, "Auto: yes, the outside on one side.")
                    }
                    (ElementKind::Wall(_), Some((false, _))) => {
                        why(ui, "Auto: no, rooms on both sides.")
                    }
                    (_, Some((v, _))) => why(ui, format!("Auto: {}.", yes_no(v))),
                    (_, None) => why(ui, "Auto: not known until rooms are drawn, so not written."),
                }
            }
            crate::props::info(
                ui,
                "Information only: carried to schedules and IFC as given, never checked.",
                "ifc",
            );
        });
        if label.is_some() {
            e.ratings = (!own.is_none()).then(|| Box::new(own));
        }
        label
    }

    /// A type's declared ratings, in its Properties: what every instance
    /// declares unless it sets its own.
    pub(super) fn type_ratings_fields(
        &self,
        ui: &mut egui::Ui,
        t: &mut creabim_core::types::ElementType,
    ) -> Option<&'static str> {
        use creabim_core::types::TypeParams;
        if !matches!(
            t.params,
            TypeParams::Wall(_)
                | TypeParams::Slab(_)
                | TypeParams::Roof(_)
                | TypeParams::Column(_)
                | TypeParams::Beam(_)
        ) {
            return None;
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Declared ratings").strong())
            .on_hover_text("What every element of this type declares, unless it sets its own. Information only: carried to schedules and IFC as given, never checked.");
        let mut label = declared_fields(ui, &mut t.ratings, None);
        if !matches!(t.params, TypeParams::Roof(_))
            && auto_yes_no(
                ui,
                "Load-bearing",
                "Load-bearing. Auto: from the layers of each element (a bearing core)",
                &mut t.ratings.load_bearing,
            )
        {
            label = Some("Load-bearing");
        }
        label
    }
}
