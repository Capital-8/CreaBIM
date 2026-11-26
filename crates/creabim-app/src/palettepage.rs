//! Graphic styles › Palette (pens stage 7, Mockup_Pens_REV05, A on all
//! four): the project's palette, its mode and five settings, the seven
//! colours it works out, the legibility rule's report, and the sample
//! plan drawn in it, seen as a colour-vision difference would see it.

use super::*;
use creabim_core::palette::{self, PaletteMode, Vision};
use creabim_core::styles::StylePack;

/// The page's own state, for the session.
#[derive(Clone, Debug, Default)]
pub(super) struct PaletteState {
    /// The name being typed for an own palette.
    pub name: Option<String>,
    /// How the preview is seen.
    pub vision: Vision,
}

/// A 0..100 (or 0..360) setting as a slider row.
fn slider(
    ui: &mut egui::Ui,
    label: &str,
    v: &mut f64,
    max: f64,
    suffix: &str,
    tip: &str,
    on: bool,
) -> bool {
    let mut changed = false;
    crate::props::row(ui, label, |ui| {
        ui.add_enabled_ui(on, |ui| {
            changed = crate::props::fill(
                ui,
                egui::Slider::new(v, 0.0..=max)
                    .suffix(suffix)
                    .max_decimals(0),
            )
            .on_hover_text(tip)
            .changed();
        });
    });
    changed
}

/// The page. True when the pack changed.
pub(super) fn palette_page(
    ui: &mut egui::Ui,
    pack: &mut StylePack,
    st: &mut PaletteState,
    seen: f64,
    dark: bool,
) -> bool {
    let names = pack.palette_names();
    let cur = pack.palette.clone().unwrap_or_else(|| "Ink".to_string());
    let factory = palette::shipped(&cur);
    let mut p = pack.palette_named(&cur).unwrap_or_default();
    let before = p.clone();
    let mut changed = false;
    let mut pick: Option<String> = None;
    let mut save_as: Option<String> = None;
    let mut reset = false;
    let mut delete = false;
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        crate::props::info(
            ui,
            "A palette gives every drawing its colours from a few settings: paper, ink, the fill of what is cut, surfaces, context, an accent and shadow. A legibility rule keeps lines readable whatever you pick. Views and view templates can take their own, in their View tab.",
            "",
        );
        // the project's palette
        crate::props::row(ui, "Palette", |ui| {
            egui::ComboBox::from_id_salt("palette_pick")
                .icon(crate::props::combo_icon)
                .width(ui.available_width())
                .selected_text(&cur)
                .show_ui(ui, |ui| {
                    for n in &names {
                        let about = palette::about(n);
                        let r = ui.selectable_label(*n == cur, n);
                        let r = if about.is_empty() { r } else { r.on_hover_text(about) };
                        if r.clicked() {
                            pick = Some(n.clone());
                        }
                    }
                });
        });
        let about = palette::about(&cur);
        if !about.is_empty() {
            crate::props::plain_note(ui, about);
        }
        let mut mode = p.mode;
        crate::props::row(ui, "Mode", |ui| {
            if crate::props::segment(
                ui,
                &mut mode,
                &PaletteMode::ALL
                    .iter()
                    .map(|m| (*m, m.label(), m.hint()))
                    .collect::<Vec<_>>(),
            ) {
                p.mode = mode;
            }
        });
        let tuned = p.mode.tuned();
        slider(ui, "Hue", &mut p.hue, 360.0, "°", "The palette's colour, round the wheel", tuned);
        slider(ui, "Ink strength", &mut p.ink, 100.0, " %", "How dark the lines are (how light, on dark paper)", tuned);
        slider(ui, "Paper tint", &mut p.paper, 100.0, " %", "How much the paper takes the hue", tuned);
        slider(ui, "Fill strength", &mut p.fill, 100.0, " %", "How strong the fill of what is cut is: walls, slabs", tuned);
        slider(
            ui,
            "Accent",
            &mut p.accent,
            360.0,
            "°",
            "Duo: the accent's turn from the hue, for furniture and planting",
            p.mode == PaletteMode::Duo,
        );
        ui.add_enabled_ui(tuned, |ui| {
            crate::props::check(ui, "Paper", &mut p.dark, "Dark paper, light ink")
                .on_hover_text("Light lines on deep paper, as a blueprint");
        });
        // save as, factory values, delete
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let changed_from_factory = factory.as_ref().is_some_and(|f| *f != p);
            if let Some(name) = &mut st.name {
                crate::props::text_field(ui, egui::TextEdit::singleline(name).hint_text("Its name"), 150.0);
                let ok = !name.trim().is_empty() && !names.contains(&name.trim().to_string());
                if ui.add_enabled(ok, egui::Button::new("Save")).clicked() {
                    save_as = Some(name.trim().to_string());
                }
                if ui.button("Cancel").clicked() {
                    st.name = None;
                }
            } else {
                if ui
                    .button("Save as…")
                    .on_hover_text("Keep these settings as a palette of your own")
                    .clicked()
                {
                    st.name = Some(format!("{cur} 2"));
                }
                if factory.is_some()
                    && ui
                        .add_enabled(changed_from_factory, egui::Button::new("Factory values"))
                        .on_hover_text("This shipped palette as it comes with CreaBIM")
                        .clicked()
                {
                    reset = true;
                }
                if factory.is_none()
                    && crate::props::icon_button(ui, "trash-2", "Delete this palette of your own").clicked()
                {
                    delete = true;
                }
            }
        });
        let r = p.roles();
        // the seven colours
        ui.add_space(6.0);
        crate::props::section(ui, "Its colours");
        ui.horizontal_wrapped(|ui| {
            for (n, c) in [
                ("Paper", r.paper),
                ("Ink", r.ink),
                ("Fill", r.fill),
                ("Surface", r.surface),
                ("Context", r.context),
                ("Accent", r.accent),
                ("Shadow", r.shadow),
            ] {
                let (sw, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                ui.painter().rect(
                    sw,
                    3.0,
                    crate::paletteshapes::color32(c),
                    egui::Stroke::new(1.0, ui.visuals().widgets.inactive.bg_stroke.color),
                    egui::StrokeKind::Inside,
                );
                ui.label(egui::RichText::new(n).size(11.5));
                ui.add_space(4.0);
            }
        });
        // the legibility rule, always on (decision 2)
        crate::props::section(ui, "Kept readable");
        let (ink, fill) = r.contrasts();
        crate::props::readout(ui, "Ink on paper", format!("{ink:.1} : 1, readable"), "Lines keep at least 4.5 : 1 against the paper");
        crate::props::readout(ui, "Walls on paper", format!("{fill:.1} : 1, stands out"), "The fill of what is cut keeps a step from the paper");
        crate::props::readout(
            ui,
            "Selection",
            if r.selection_moved {
                "moved to the opposite hue"
            } else {
                "orange, as usual"
            },
            "The selection keeps its orange unless the palette is near orange itself",
        );
        if !r.note.is_empty() {
            crate::props::plain_note(ui, r.note);
        }

        let ui = &mut cols[1];
        // the sample plan in the palette, as someone would see it
        let mut vision = st.vision;
        crate::props::row(ui, "Seen as", |ui| {
            egui::ComboBox::from_id_salt("palette_vision")
                .icon(crate::props::combo_icon)
                .width(ui.available_width())
                .selected_text(vision.label())
                .show_ui(ui, |ui| {
                    for v in Vision::ALL {
                        ui.selectable_value(&mut vision, v, v.label());
                    }
                });
        });
        st.vision = vision;
        let layer = ui.layer_id();
        let from = ui.ctx().graphics(|g| g.get(layer).map(|l| l.next_idx()));
        // drawn on light paper, then given the palette's colours, as a
        // view is on screen
        super::stylepages::sample_plan(ui, pack, None, seen, dark && p.is_ink());
        if !p.is_ink() {
            crate::paletteshapes::apply(ui.ctx(), layer, from, &r);
        }
        if vision != Vision::Typical {
            crate::paletteshapes::see(ui.ctx(), layer, from, vision);
        }
        crate::props::plain_note(
            ui,
            "Colour is given as a drawing is painted: changing a palette redraws nothing else.",
        );
    });
    if let Some(n) = pick {
        pack.palette = (n != "Ink").then_some(n);
        st.name = None;
        return true;
    }
    if let Some(n) = save_as {
        let mut own = p.clone();
        own.name = n.clone();
        pack.palettes.push(own);
        pack.palette = Some(n);
        st.name = None;
        return true;
    }
    if reset {
        pack.palettes.retain(|x| x.name != cur);
        return true;
    }
    if delete {
        pack.palettes.retain(|x| x.name != cur);
        pack.palette = None;
        return true;
    }
    if p != before {
        pack.put_palette(p);
        changed = true;
    }
    changed
}
