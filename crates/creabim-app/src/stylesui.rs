//! Graphic styles in the app: the Graphic styles window, where the
//! project's style packs (pens, object styles, hatch patterns and view
//! templates) are edited, loaded from files and saved to them; and in a
//! view's Properties its view range, template, depth cues and graphic
//! overrides. Packs and views are project data, and every change is one
//! transaction, so it undoes like any edit.

use super::stylepages::weight_combo;
use super::*;
use creabim_core::graphics::{PlanRange, RangeLimit};
use creabim_core::styles::{
    self, DepthCues, HatchLine, HatchPattern, LinePattern, Override, PatternUnits, Rgb, Shadows,
    StylePack, Target, ViewTemplate,
};
use creabim_core::visibility::Category;

/// The window's page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Page {
    #[default]
    Objects,
    Pens,
    Palette,
    Templates,
    Marks,
    Symbols,
}

/// The window's own state, for the session.
#[derive(Default)]
pub(super) struct StylesUi {
    pub open: bool,
    pub page: Page,
    /// The template picked in its list.
    template: usize,
    /// The marker designer.
    designer: crate::choices::Designer,
    /// While an option is previewed on the plan, the pack as it was.
    pub(super) preview_base: Option<StylePack>,
    /// Find, the sample plan's scale and a set's name being typed.
    pages: super::stylepages::PageState,
    /// The Palette page's name being typed and its preview's eyes.
    palette: super::palettepage::PaletteState,
}

/// The undo label of a previewed option, taken back when the pointer
/// leaves it.
const PREVIEW: &str = "Preview style";

/// The categories object styles are set for: those with linework.
pub(super) const STYLED: [Category; 20] = [
    Category::Walls,
    Category::Slabs,
    Category::Roofs,
    Category::Columns,
    Category::Beams,
    Category::Footings,
    Category::Doors,
    Category::Windows,
    Category::Stairs,
    Category::Railings,
    Category::Ceilings,
    Category::Objects,
    Category::FreeMeshes,
    Category::Parts,
    Category::Site,
    Category::Planting,
    Category::Openings,
    Category::Rooms,
    Category::Grids,
    Category::SectionMarks,
];

fn pattern_combo(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    p: &mut Option<LinePattern>,
) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(salt)
        .icon(crate::props::combo_icon)
        .width(ui.spacing().combo_width.max(84.0).min(ui.available_width()))
        .selected_text(p.as_ref().map_or("As drawn", |p| p.label()))
        .show_ui(ui, |ui| {
            changed |= ui.selectable_value(p, None, "As drawn").changed();
            for n in LinePattern::NAMED {
                let label = n.label();
                changed |= ui.selectable_value(p, Some(n), label).changed();
            }
        });
    changed
}

/// A colour that can be left unset. A fill starts light, a line dark.
pub(super) fn colour_pick(ui: &mut egui::Ui, c: &mut Option<Rgb>, fill: bool, hover: &str) -> bool {
    let mut on = c.is_some();
    let mut changed = crate::props::tickbox(ui, &mut on, "")
        .on_hover_text(hover)
        .changed();
    if changed {
        let v = if fill { 0.85 } else { 0.2 };
        *c = on.then_some(Rgb([v; 3]));
    }
    if let Some(Rgb(rgb)) = c {
        // the picker rounds through its own colour space every frame: keep
        // what it gives back only when it was used
        let mut edit = *rgb;
        if crate::props::colour_button_rgb(ui, &mut edit).changed() {
            *rgb = edit;
            changed = true;
        }
    }
    changed
}

fn name_combo(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    names: &[String],
    v: &mut Option<String>,
    none: &str,
) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(salt)
        .icon(crate::props::combo_icon)
        .width(
            ui.spacing()
                .combo_width
                .max(108.0)
                .min(ui.available_width()),
        )
        .selected_text(v.as_deref().unwrap_or(none))
        .show_ui(ui, |ui| {
            changed |= ui.selectable_value(v, None, none).changed();
            for n in names {
                changed |= ui.selectable_value(v, Some(n.clone()), n).changed();
            }
        });
    changed
}

/// A level and an offset from it, or none.
fn limit_ui(
    ui: &mut egui::Ui,
    salt: &str,
    label: &str,
    none: &str,
    levels: &[(ElementId, String)],
    l: &mut Option<RangeLimit>,
) -> bool {
    let mut changed = false;
    crate::props::flow(ui, |ui| {
        ui.label(label);
        let current = l
            .and_then(|l| levels.iter().find(|x| x.0 == l.level))
            .map_or(none.to_string(), |x| x.1.clone());
        egui::ComboBox::from_id_salt(salt)
            .icon(crate::props::combo_icon)
            .width(ui.available_width().min(130.0))
            .truncate()
            .selected_text(current)
            .show_ui(ui, |ui| {
                if ui.selectable_label(l.is_none(), none).clicked() {
                    *l = None;
                    changed = true;
                }
                for (id, name) in levels {
                    let on = l.is_some_and(|l| l.level == *id);
                    if ui.selectable_label(on, name).clicked() {
                        *l = Some(RangeLimit {
                            level: *id,
                            offset: l.map_or(0.0, |l| l.offset),
                        });
                        changed = true;
                    }
                }
            });
        if let Some(l) = l {
            ui.label("+");
            changed |= ui
                .add(crate::units::drag(&mut l.offset).range(-100.0..=100.0))
                .changed();
        }
    });
    changed
}

/// A plan's view range: its own level, the levels within a range, or a
/// view from above. Returns the undo label of a change.
pub(super) fn range_ui(
    ui: &mut egui::Ui,
    levels: &[(ElementId, String)],
    r: &mut PlanRange,
) -> Option<&'static str> {
    let mut label = None;
    #[derive(PartialEq)]
    enum Mode {
        Own,
        Range,
        Above,
    }
    let mode = if r.from_above {
        Mode::Above
    } else if r.spans_levels() {
        Mode::Range
    } else {
        Mode::Own
    };
    let mut pick = mode;
    crate::props::flow(ui, |ui| {
        crate::props::radio_tick(ui, &mut pick, Mode::Own, "Its level")
            .on_hover_text("Only the elements of this plan's own level, as plans always drew");
        crate::props::radio_tick(ui, &mut pick, Mode::Range, "Levels in range")
            .on_hover_text("Every level's elements between the bottom and the top: a site level under the floor, roofs dashed overhead");
        crate::props::radio_tick(ui, &mut pick, Mode::Above, "From above")
            .on_hover_text("Look down from the top with nothing cut; roofs and upper slabs hide what is under them. A roof or site plan.");
    });
    let was = if r.from_above {
        2
    } else if r.spans_levels() {
        1
    } else {
        0
    };
    let now = match pick {
        Mode::Own => 0,
        Mode::Range => 1,
        Mode::Above => 2,
    };
    if now != was {
        let depth = r.view_depth;
        *r = PlanRange {
            view_depth: depth,
            ..Default::default()
        };
        match now {
            1 => r.overhead = true,
            2 => r.from_above = true,
            _ => {}
        }
        label = Some("View range");
    }
    match now {
        1 => {
            if limit_ui(ui, "range_top", "Top", "Next level up", levels, &mut r.top) {
                label = Some("View range");
            }
            if limit_ui(
                ui,
                "range_bottom",
                "Bottom",
                "This level",
                levels,
                &mut r.bottom,
            ) {
                label = Some("View range");
            }
            if limit_ui(
                ui,
                "range_beyond",
                "View depth",
                "None",
                levels,
                &mut r.beyond,
            ) {
                label = Some("View range");
            }
            if crate::props::switch(ui, &mut r.overhead, "Overhead dashed")
                .on_hover_text("Draw what is above the cut, up to the top, in dashed lines: roof edges, upper slabs")
                .changed()
            {
                label = Some("View range");
            }
            crate::props::info(
                ui,
                "Levels between the bottom and the cut are cut or seen below it; between the view depth and the bottom they draw lighter.",
                "",
            );
        }
        2 => {
            if limit_ui(
                ui,
                "above_top",
                "Top",
                "Above everything",
                levels,
                &mut r.top,
            ) {
                label = Some("View range");
            }
            if limit_ui(
                ui,
                "above_bottom",
                "Bottom",
                "Under everything",
                levels,
                &mut r.bottom,
            ) {
                label = Some("View range");
            }
        }
        _ => {}
    }
    label
}

/// Every setting of a line style, in the five groups of the approved
/// mockup (Mockup_Drawing_Styles_REV00). True when one changed.
pub(super) fn line_style_fields(
    ui: &mut egui::Ui,
    s: &mut creabim_core::linestyle::LineStyle,
) -> bool {
    use creabim_core::linestyle::{LeaderRun, Lettering, LineEnd};
    let mut changed = false;
    let slide = |ui: &mut egui::Ui, name: &str, v: &mut f64, lo: f64, hi: f64, tip: &str| -> bool {
        crate::props::row(ui, name, |ui| {
            ui.spacing_mut().slider_width = (ui.available_width() - 56.0).max(40.0);
            ui.add(egui::Slider::new(v, lo..=hi).max_decimals(2))
                .on_hover_text(tip)
                .changed()
        })
    };
    crate::props::section(ui, "Line");
    changed |= slide(
        ui,
        "Overshoot",
        &mut s.overshoot,
        0.0,
        6.0,
        "How far strokes run past their corners",
    );
    changed |= slide(
        ui,
        "Wobble",
        &mut s.wobble,
        0.0,
        0.6,
        "A slight waver along a stroke; lines stay straight",
    );
    changed |= slide(
        ui,
        "Wobble length",
        &mut s.wobble_length,
        30.0,
        110.0,
        "How long each waver is",
    );
    let mut end = s.end;
    crate::app::helpui::combo(ui, "Ends")
        .selected_text(end.label())
        .show_ui(ui, |ui| {
            for e in LineEnd::ALL {
                ui.selectable_value(&mut end, e, e.label());
            }
        });
    if end != s.end {
        s.end = end;
        changed = true;
    }
    changed |= slide(
        ui,
        "End amount",
        &mut s.end_amount,
        0.0,
        1.0,
        "How strong the blob, taper or flick is",
    );
    changed |= slide(
        ui,
        "Pressure",
        &mut s.pressure,
        0.0,
        0.6,
        "How much a stroke swells and thins along its length",
    );
    changed |= slide(
        ui,
        "Double pass",
        &mut s.double_pass,
        0.0,
        0.8,
        "A second, finer stroke beside the first",
    );
    changed |= slide(
        ui,
        "Double weight",
        &mut s.double_strength,
        0.0,
        0.6,
        "How heavy the second stroke is; it stays black",
    );
    crate::props::section(ui, "Hatches");
    changed |= slide(
        ui,
        "Spacing jitter",
        &mut s.hatch_spacing_jitter,
        0.0,
        0.5,
        "Hatch lines a little closer or further apart",
    );
    changed |= slide(
        ui,
        "Angle jitter",
        &mut s.hatch_angle_jitter,
        0.0,
        8.0,
        "Hatch lines turned a few degrees, by hand",
    );
    changed |= slide(
        ui,
        "Broken lines",
        &mut s.hatch_broken,
        0.0,
        0.5,
        "How often a hatch line has a gap",
    );
    changed |= slide(
        ui,
        "Density",
        &mut s.hatch_density,
        0.6,
        1.6,
        "Hand hatches closer or looser (comes with hand hatches)",
    );
    changed |= crate::props::check(ui, "", &mut s.timber_x, "Timber X").changed();
    changed |= crate::props::check(ui, "", &mut s.earth_weave, "Earth weave").changed();
    crate::props::section(ui, "Colour");
    changed |= slide(
        ui,
        "Wash",
        &mut s.wash,
        0.0,
        1.0,
        "Flat colour under the ink; the ink stays black (comes with the palette)",
    );
    crate::props::section(ui, "Lettering");
    let mut letters = s.lettering;
    crate::app::helpui::combo(ui, "Lettering")
        .selected_text(letters.label())
        .show_ui(ui, |ui| {
            for l in Lettering::ALL {
                ui.selectable_value(&mut letters, l, l.label());
            }
        });
    if letters != s.lettering {
        s.lettering = letters;
        changed = true;
    }
    changed |= slide(
        ui,
        "Letter jitter",
        &mut s.letter_jitter,
        0.0,
        0.7,
        "Letters a little off the line, by hand",
    );
    crate::props::section(ui, "Leaders and ticks");
    let mut run = s.leader;
    crate::app::helpui::combo(ui, "Leader")
        .selected_text(run.label())
        .show_ui(ui, |ui| {
            for r in LeaderRun::ALL {
                ui.selectable_value(&mut run, r, r.label());
            }
        });
    if run != s.leader {
        s.leader = run;
        changed = true;
    }
    if let Some(shipped) = creabim_core::linestyle::shipped_style(&s.name) {
        if *s != shipped
            && crate::props::action(
                ui,
                "",
                "Reset to its factory values",
                "The values it shipped with",
                true,
            )
            .clicked()
        {
            *s = shipped;
            changed = true;
        }
    }
    ui.label(
        egui::RichText::new("Line, hatch, leader and tick settings draw now, on screen and on sheets. Lettering, stepped leaders, washes and the hand hatches' density come with the next stages.")
            .size(11.0)
            .weak(),
    );
    changed
}

/// A view's template, depth cues and overrides. Returns the undo label of
/// a change.
pub(super) fn look_ui(
    ui: &mut egui::Ui,
    model: &creabim_core::Model,
    v: &mut View,
    edit_styles: &mut bool,
    adjusted: &mut Option<creabim_core::linestyle::LineStyle>,
) -> Option<&'static str> {
    let mut label = None;
    let pack = styles::active_pack(model);
    let names: Vec<String> = pack.templates.iter().map(|t| t.name.clone()).collect();
    let fallback = pack.default_templates.of(v).map(str::to_string);
    let none = match &fallback {
        Some(n) => format!("Pack default ({n})"),
        None => "None".to_string(),
    };
    let none = none.replace("Pack default", "Default");
    crate::props::row(ui, "Template", |ui| {
        ui.spacing_mut().combo_width = ui.available_width();
        if name_combo(ui, "view_template", &names, &mut v.look.template, &none) {
            label = Some("View template");
        }
    });
    // its pen set (pens stage 2): its own, its template's, else the project's
    let sets = pack.set_names();
    let from_template = {
        let mut bare = v.clone();
        bare.look.pen_set = None;
        bare.look.resolved = false;
        styles::resolved(model, &bare).look.pen_set.clone()
    };
    let none = match from_template {
        Some(t) => format!("Template's ({t})"),
        None => format!("Project's ({})", pack.default_set_name()),
    };
    crate::props::row(ui, "Pen set", |ui| {
        ui.spacing_mut().combo_width = ui.available_width();
        if name_combo(ui, "view_pen_set", &sets, &mut v.look.pen_set, &none) {
            label = Some("Pen set");
        }
    });
    // its line style (pens stage 4): how its lines are drawn, Ruled to
    // hand-made; its own, its template's, else the project's
    let ls_names = pack.line_style_names();
    let ls_template = {
        let mut bare = v.clone();
        bare.look.line_style = None;
        bare.look.resolved = false;
        styles::resolved(model, &bare).look.line_style.clone()
    };
    let project_ls = pack
        .line_style
        .clone()
        .unwrap_or_else(|| creabim_core::linestyle::RULED.to_string());
    let none = match &ls_template {
        Some(t) => format!("Template's ({t})"),
        None => format!("Project's ({project_ls})"),
    };
    crate::props::row(ui, "Line style", |ui| {
        ui.spacing_mut().combo_width = ui.available_width();
        if name_combo(
            ui,
            "view_line_style",
            &ls_names,
            &mut v.look.line_style,
            &none,
        ) {
            label = Some("Line style");
        }
    });
    // its palette (pens stage 7): the colours it draws in; its own, its
    // template's, else the project's
    let pal_names = pack.palette_names();
    let pal_template = {
        let mut bare = v.clone();
        bare.look.palette = None;
        bare.look.resolved = false;
        styles::resolved(model, &bare).look.palette.clone()
    };
    let pal_none = match &pal_template {
        Some(t) => format!("Template's ({t})"),
        None => format!(
            "Project's ({})",
            pack.palette.clone().unwrap_or_else(|| "Ink".into())
        ),
    };
    crate::props::row(ui, "Palette", |ui| {
        ui.spacing_mut().combo_width = ui.available_width();
        if name_combo(
            ui,
            "view_palette",
            &pal_names,
            &mut v.look.palette,
            &pal_none,
        ) {
            label = Some("Palette");
        }
    });
    let current = v
        .look
        .line_style
        .clone()
        .or(ls_template)
        .unwrap_or(project_ls);
    // Ruled has nothing to adjust: changing it would stop it being ruled
    if let Some(mut s) = pack
        .line_style_named(&current)
        .filter(|s| s.name != creabim_core::linestyle::RULED)
    {
        let title = format!("Adjust {}", s.name);
        if let Some(true) =
            crate::props::sub_box(ui, "view.linestyle.adjust", &title, None, false, |ui| {
                line_style_fields(ui, &mut s)
            })
        {
            *adjusted = Some(s);
        }
    }
    // the pack the template comes from, and where it is edited (MU22)
    // as Blender's data rows (MU30): the pack's name in a field, its edit
    // button beside it, together filling the field column
    crate::props::row(ui, "Style pack", |ui| {
        let w = ui.available_width();
        let gap = ui.spacing().item_spacing.x;
        ui.add_sized(
            [(w - 28.0 - gap).max(40.0), 22.0],
            // the fill of the fields and lists beside it, in every theme
            // (MU31: a hover-only button took the theme's darkest fill)
            egui::Button::new(egui::RichText::new(pack.name.clone()))
                .fill(ui.visuals().widgets.inactive.weak_bg_fill)
                .stroke(ui.visuals().widgets.inactive.bg_stroke)
                .sense(egui::Sense::hover()),
        )
        .on_hover_text("A named bundle of scale, style, hidden categories, pens, overrides and cues from the project's style pack. Change the template and every view using it changes.");
        if crate::props::icon_button(ui, "pencil", "Edit the style pack")
            .on_hover_text(
                "Graphic styles: the pack's pens, object styles, hatches and view templates",
            )
            .clicked()
        {
            *edit_styles = true;
        }
    });
    // depth cues
    let c = &mut v.look.cues;
    crate::props::row(ui, "Fade", |ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 56.0).max(40.0);
        if ui
            .add(egui::Slider::new(&mut c.fade, 0.0..=0.9).max_decimals(2))
            .on_hover_text("Lines and fills further from the cut or the viewer draw lighter, up to this much at the far end of the view. Sections, elevations and plans from above.")
            .changed()
        {
            label = Some("Depth cues");
        }
    });
    let mut sun = c.shadows.is_some();
    if crate::props::check(ui, "Shadows", &mut sun, "Sun")
        .on_hover_text("Shadows cast on the ground, in plans from above")
        .changed()
    {
        c.shadows = sun.then_some(Shadows {
            direction: -50.0,
            length: 0.6,
            opacity: 0.35,
        });
        label = Some("Depth cues");
    }
    if let Some(s) = &mut c.shadows {
        crate::props::row(ui, "Toward", |ui| {
            label = label.or(crate::props::fill(
                ui,
                egui::DragValue::new(&mut s.direction)
                    .speed(1.0)
                    .suffix("°"),
            )
            .changed()
            .then_some("Depth cues"));
        });
        crate::props::row(ui, "Length", |ui| {
            label = label.or(crate::props::fill(
                ui,
                egui::DragValue::new(&mut s.length)
                    .speed(0.05)
                    .range(0.0..=5.0),
            )
            .changed()
            .then_some("Depth cues"));
        });
        crate::props::row(ui, "Strength", |ui| {
            ui.spacing_mut().slider_width = (ui.available_width() - 56.0).max(40.0);
            label = label.or(ui
                .add(egui::Slider::new(&mut s.opacity, 0.05..=1.0))
                .changed()
                .then_some("Depth cues"));
        });
    }
    // overrides by category: the sub-container (MU60), at the end of its
    // section as AMD-003 asks, its count while shut
    let n = v.look.overrides.len();
    crate::props::sub_box(ui, "view.overrides", "Overrides", Some(n), true, |ui| {
        let mut remove = None;
        for (i, o) in v.look.overrides.iter_mut().enumerate() {
            let name = match o.target {
                Target::Category(c) => c.label().to_string(),
                Target::Element(id) => model
                    .get(id)
                    .map_or("(deleted element)".into(), |e| e.name.clone()),
            };
            ui.push_id(("override", i), |ui| {
                crate::props::flow(ui, |ui| {
                    ui.label(egui::RichText::new(name).strong());
                    if ui
                        .small_button("✕")
                        .on_hover_text("Remove this override")
                        .clicked()
                    {
                        remove = Some(i);
                    }
                });
                if override_fields(ui, o, &pack) {
                    label = Some("Graphic override");
                }
            });
        }
        if let Some(i) = remove {
            v.look.overrides.remove(i);
            label = Some("Graphic override");
        }
        let mut add = None;
        crate::props::row(ui, "Add", |ui| {
            egui::ComboBox::from_id_salt("add_override")
                .icon(crate::props::combo_icon)
                .selected_text("A category…")
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for c in STYLED {
                        let taken = v
                            .look
                            .overrides
                            .iter()
                            .any(|o| o.target == Target::Category(c));
                        if !taken && ui.selectable_label(false, c.label()).clicked() {
                            add = Some(c);
                        }
                    }
                });
        });
        if let Some(c) = add {
            v.look.override_mut(Target::Category(c)).halftone = true;
            label = Some("Graphic override");
        }
    });
    label
}

/// The fields of one override. True when one changed.
pub(super) fn override_fields(ui: &mut egui::Ui, o: &mut Override, pack: &StylePack) -> bool {
    let mut changed = false;
    // one setting a row, as Blender's (MU22)
    changed |= crate::props::check(ui, "Draw", &mut o.halftone, "Halftone")
        .on_hover_text("Drawn half-strength, mixed toward white")
        .changed();
    let mut hatch_off = o.hatch == Some(false);
    if crate::props::check(ui, "", &mut hatch_off, "No hatch")
        .on_hover_text("Leave out the hatch lines of its cut faces")
        .changed()
    {
        o.hatch = hatch_off.then_some(false);
        changed = true;
    }
    crate::props::row(ui, "Weight", |ui| {
        ui.spacing_mut().combo_width = ui.available_width();
        changed |= weight_combo(ui, "ov_weight", &mut o.weight, pack);
    });
    crate::props::row(ui, "Line", |ui| {
        ui.spacing_mut().combo_width = ui.available_width();
        changed |= pattern_combo(ui, "ov_pattern", &mut o.pattern);
    });
    crate::props::row(ui, "Colour", |ui| {
        changed |= colour_pick(ui, &mut o.colour, false, "A colour of its own in this view");
    });
    changed
}

impl CreaBimApp {
    /// The pack being edited: the project's default, else the Standard
    /// pack it draws with until it has one.
    pub(super) fn edited_pack(&self) -> (Option<ElementId>, StylePack) {
        let model = self.doc.model();
        let all = styles::packs(model);
        let found = all.iter().find(|p| p.1.default).or(all.first());
        match found {
            Some((id, p)) => (Some(*id), (*p).clone()),
            None => (None, StylePack::standard()),
        }
    }

    /// Commit an edited pack: in place, or as the project's first pack.
    pub(super) fn commit_pack(&mut self, id: Option<ElementId>, pack: StylePack, label: &str) {
        let r = match id {
            Some(id) => self.doc.edit_merged(id, label, |e| {
                e.name = pack.name.clone();
                e.kind = ElementKind::StylePack(Box::new(pack.clone()));
            }),
            None => {
                let tx = styles::apply_pack(self.doc.model(), &pack);
                self.doc.commit(Transaction {
                    label: label.into(),
                    ..tx
                })
            }
        };
        self.report(r, format!("{label}."));
    }

    /// Properties of a view: its range (plans) and its graphics.
    /// A view's View fold: its scale, underlay, room colours, style and
    /// 3D angle (Mockup_View_Tabs_REV00: these were on the element tab).
    pub(super) fn view_main_section(&mut self, ui: &mut egui::Ui, id: ElementId) {
        let Some(ElementKind::View(v)) = self.doc.model().get(id).map(|e| &e.kind) else {
            return;
        };
        if matches!(v.kind, ViewKind::Schedule { .. }) {
            return;
        }
        let mut v = v.clone();
        let mut label = None;
        // how it's shown, first: its scale and what it's drawn over
        // (Mockup_View_Tabs_REV00: these were on the element tab)
        let mut rescale = None;
        let open = self.prop_header(
            ui,
            "view.main",
            "View",
            true,
            Some("views#the-views-settings"),
        );
        crate::props::fold(ui, open, |ui| {
            if let Some(sc) = crate::sheetview::scale_picker(ui, id, v.scale) {
                rescale = Some(sc);
            }
            if let ViewKind::Plan { level, .. } = &v.kind {
                let choices = self.underlay_choices(*level);
                let current = choices
                    .iter()
                    .find(|(u, _)| *u == v.underlay)
                    .map_or("None".to_string(), |(_, l)| l.clone());
                crate::app::helpui::combo(ui, "Underlay")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for (u, l) in &choices {
                            if ui.selectable_value(&mut v.underlay, *u, l).changed() {
                                label = Some("View underlay");
                            }
                        }
                    })
                    .response
                    .on_hover_text("Another level drawn faintly under this one, to trace and snap to. Not printed.");
            }
            if matches!(
                v.kind,
                ViewKind::Plan { .. } | ViewKind::Section { cut: true, .. }
            ) && crate::props::switch(ui, &mut v.room_colours, "Colour rooms")
                .on_hover_text("Fill each room with a colour of its own, the same in every view")
                .changed()
            {
                label = Some("Colour rooms");
            }
            if matches!(
                v.kind,
                ViewKind::Section { .. } | ViewKind::Axonometric { .. }
            ) && super::view_style_combo(ui, "view_style", &mut v.style)
            {
                label = Some("View style");
            }
            if matches!(v.kind, ViewKind::Section { .. }) {
                crate::props::row(ui, "Drawing style", |ui| {
                    egui::ComboBox::from_id_salt("section.drawing_style")
                        .icon(crate::props::combo_icon)
                        .selected_text(v.graphics.drawing_style.label())
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for st in creabim_core::graphics::DrawingStyle::ALL {
                                if ui
                                    .selectable_value(&mut v.graphics.drawing_style, st, st.label())
                                    .on_hover_text(st.hint())
                                    .changed()
                                {
                                    label = Some("Drawing style");
                                }
                            }
                        });
                });
            }
            if let ViewKind::Axonometric { azimuth, altitude } = &mut v.kind {
                for (text, val, lo, hi) in [
                    ("Azimuth", azimuth, 0.0, 360.0),
                    ("Altitude", altitude, -89.0, 89.0),
                ] {
                    if crate::props::row(ui, text, |ui| {
                        crate::props::fill(
                            ui,
                            egui::DragValue::new(val)
                                .speed(0.5)
                                .range(lo..=hi)
                                .suffix("\u{b0}"),
                        )
                        .changed()
                    }) {
                        label = Some("Orbit 3D view");
                    }
                }
            }
        });
        if let Some(sc) = rescale {
            self.set_view_scale(id, sc);
            if let Some(ElementKind::View(x)) = self.doc.model().get(id).map(|e| &e.kind) {
                v.scale = x.scale;
            }
        }
        if let Some(label) = label {
            let r = self.doc.edit_merged(id, label, |e| {
                if let ElementKind::View(x) = &mut e.kind {
                    *x = v.clone();
                }
            });
            self.report(r, "View changed.");
        }
    }

    pub(super) fn view_graphics_sections(&mut self, ui: &mut egui::Ui, id: ElementId) {
        let mut cut_box: Option<bool> = None;
        self.view_graphics_inner(ui, id, &mut cut_box);
        if let Some(up) = cut_box {
            self.cut_box(id, up);
        }
    }

    fn view_graphics_inner(
        &mut self,
        ui: &mut egui::Ui,
        id: ElementId,
        cut_box: &mut Option<bool>,
    ) {
        let Some(ElementKind::View(v)) = self.doc.model().get(id).map(|e| &e.kind) else {
            return;
        };
        if matches!(v.kind, ViewKind::Schedule { .. }) {
            return;
        }
        let mut v = v.clone();
        let mut label = None;
        // a saved 3D view's crop boxes, how much of it is shown
        if matches!(v.kind, ViewKind::Axonometric { .. }) {
            self.crop_list(ui, Some(id));
        }
        // a section's range: whether it cuts and how far it looks
        if matches!(v.kind, ViewKind::Section { .. }) {
            let open = self.prop_header(ui, "view.srange", "View range", true, Some("sections"));
            crate::props::fold(ui, open, |ui| {
                if let ViewKind::Section { depth, cut, .. } = &mut v.kind {
                    if crate::props::switch(ui, cut, "Cut through the model (section)").changed() {
                        label = Some("Section or elevation");
                    }
                    if crate::props::row(ui, "Depth", |ui| {
                        crate::props::fill(ui, crate::units::drag(depth).range(0.1..=500.0))
                            .changed()
                    }) {
                        label = Some("View depth");
                    }
                }
            });
        }
        if matches!(v.kind, ViewKind::Plan { .. }) {
            let open = self.prop_header(ui, "view.range", "View range", true, Some("view-range"));
            crate::props::fold(ui, open, |ui| {
                let levels: Vec<(ElementId, String)> =
                    creabim_core::levels::sorted(self.doc.model())
                        .into_iter()
                        .map(|(id, name, _)| (id, name))
                        .collect();
                // which way it looks, beside the cut it looks from (MU93: it
                // was only on the view's element tab)
                let mut up = v.ceiling_plan;
                if crate::props::row(ui, "Looks", |ui| {
                    crate::props::segment(
                        ui,
                        &mut up,
                        &[
                            (
                                false,
                                "Down",
                                "A floor plan: cut, then looking down to the floor",
                            ),
                            (
                                true,
                                "Up",
                                "A reflected ceiling plan: cut, then looking up to the ceilings, \
                                 what hangs from them and the beams, up to the level above",
                            ),
                        ],
                    )
                }) {
                    if let Some(said) = super::set_looks_up(&mut v, up) {
                        self.status = said;
                    }
                    label = Some("Ceiling plan");
                }
                // its ceiling height tags, only when it looks up
                if v.ceiling_plan {
                    let mut show = !v.graphics.hide_ceiling_heights;
                    if crate::props::row(ui, "Heights", |ui| {
                        crate::props::segment(
                            ui,
                            &mut show,
                            &[
                                (
                                    true,
                                    "Show",
                                    "A CH tag in each ceiling: its height above the floor",
                                ),
                                (false, "Hide", "No ceiling height tags"),
                            ],
                        )
                    }) {
                        v.graphics.hide_ceiling_heights = !show;
                        label = Some("Ceiling heights");
                    }
                }
                // the cut, as in the header (MU32: only the header had it)
                if let ViewKind::Plan { cut_height, .. } = &mut v.kind {
                    crate::props::row(ui, "Cut height", |ui| {
                        let r = crate::props::fill(
                            ui,
                            crate::units::drag(cut_height).range(0.0..=100.0),
                        )
                        .on_hover_text("How high above its level the plan is cut");
                        if r.changed() {
                            label = Some("Cut height");
                        }
                    });
                }
                if let Some(l) = range_ui(ui, &levels, &mut v.graphics.range) {
                    label = Some(l);
                }
                // the cut as a section box in 3D (MU34 A2)
                let (down, up) = crate::props::actions2(
                    ui,
                    "Section box",
                    (
                        "Below the cut",
                        "Cut the 3D view as this plan sees it",
                        true,
                    ),
                    (
                        "Above (ceilings)",
                        "Keep what is above the cut, to look up at the ceilings",
                        true,
                    ),
                );
                if down || up {
                    *cut_box = Some(up);
                }
            });
        }
        // a section's cut as a section box in 3D too (MU34 A2)
        if matches!(v.kind, ViewKind::Section { .. })
            && crate::props::action(
                ui,
                "Section box",
                "In 3D",
                "Cut the 3D view as this section sees it: from its line to its depth",
                true,
            )
            .clicked()
        {
            *cut_box = Some(false);
        }
        let open = self.prop_header(
            ui,
            "view.look",
            "Template and overrides",
            true,
            Some("graphic-styles#view-templates"),
        );
        crate::props::fold(ui, open, |ui| {
            let mut edit = false;
            let mut adjusted = None;
            if let Some(l) = look_ui(ui, self.doc.model(), &mut v, &mut edit, &mut adjusted) {
                label = Some(l);
            }
            if edit {
                self.gstyles.open = true;
            }
            // an adjusted line style is the pack's, for every view using it
            if let Some(s) = adjusted {
                let (pid, mut pack) = self.edited_pack();
                pack.put_line_style(s);
                self.commit_pack(pid, pack, "Adjust line style");
            }
        });
        if let Some(label) = label {
            v.look.tidy();
            let r = self.doc.edit_merged(id, label, |e| {
                if let ElementKind::View(x) = &mut e.kind {
                    *x = v.clone();
                }
            });
            self.report(r, "View changed.");
            // turned to look up: each ceiling's height tagged (MU142)
            self.tag_ceilings_of(id);
        }
    }

    /// The selected element's override in the open view: "this boundary
    /// wall dashed here".
    pub(super) fn element_override_section(&mut self, ui: &mut egui::Ui, id: ElementId) {
        let Some(view) = self.active_view else { return };
        let model = self.doc.model();
        let Some(ElementKind::View(v)) = model.get(view).map(|e| &e.kind) else {
            return;
        };
        if matches!(v.kind, ViewKind::Schedule { .. }) || id == view {
            return;
        }
        let cutters = creabim_core::visibility::cutters(model);
        if model
            .get(id)
            .and_then(|e| Category::of(e, &cutters))
            .is_none()
        {
            return;
        }
        let view_name = model.get(view).map_or("", |e| e.name.as_str()).to_string();
        let pack = styles::active_pack(model).into_owned();
        let mut v = v.clone();
        let open = self.prop_header(
            ui,
            "element.override",
            &format!("In {view_name}"),
            false,
            Some("graphic-styles#overrides"),
        );
        crate::props::fold(ui, open, |ui| {
            let mut o = v
                .look
                .overrides
                .iter()
                .find(|o| o.target == Target::Element(id))
                .cloned()
                .unwrap_or_else(|| Override::new(Target::Element(id)));
            if override_fields(ui, &mut o, &pack) {
                *v.look.override_mut(Target::Element(id)) = o;
                v.look.tidy();
                let r = self.doc.edit_merged(view, "Graphic override", |e| {
                    if let ElementKind::View(x) = &mut e.kind {
                        *x = v.clone();
                    }
                });
                self.report(r, "Override changed.");
            }
        });
    }

    /// The Graphic styles window, when open.
    pub(super) fn graphic_styles_window(&mut self, ctx: &egui::Context) {
        if !self.gstyles.open {
            return;
        }
        let mut open = true;
        let (id, in_model) = self.edited_pack();
        // while an option is previewed, the window shows the pack as it was
        let before = self
            .gstyles
            .preview_base
            .clone()
            .unwrap_or(in_model.clone());
        let mut pack = before.clone();
        let mut action: Option<&'static str> = None;
        let mut label = "Graphic styles";
        // tags, symbols and patterns moved to Libraries (Mockup_Libraries_
        // REV00, section 3, owner 2026-11-18)
        if matches!(self.gstyles.page, Page::Marks | Page::Symbols) {
            self.gstyles.page = Page::Objects;
        }
        let mut go: Option<&'static str> = None;
        let mut make_default = None;
        let area = self.mat.area.unwrap_or_else(|| ctx.content_rect());
        let model = self.doc.model();
        let packs: Vec<(ElementId, String, bool)> = styles::packs(model)
            .iter()
            .map(|(i, p)| (*i, p.name.clone(), p.default))
            .collect();
        let views: Vec<ElementId> = self
            .selection
            .all()
            .iter()
            .copied()
            .filter(|i| matches!(model.get(*i).map(|e| &e.kind), Some(ElementKind::View(_))))
            .collect();
        let mut give_template: Option<String> = None;
        let dark = self.dark_paper(ctx);
        crate::app::shell::window("Graphic styles")
            .open(&mut open)
            .default_pos(area.min + Vec2::new(40.0, 64.0))
            .default_size(Vec2::new(1180.0, 640.0))
            .constrain_to(area)
            .collapsible(false)
            .show(ctx, |ui| {
                crate::props::info(ui, "How the project's drawings look: object styles, pens, palette and view templates, kept together as a style pack with the tags, symbols and patterns it uses. Load an office's pack to restyle every view and sheet; save this one to use in another project.", "");
                crate::props::flow(ui, |ui| {
                    ui.label("Style pack");
                    egui::ComboBox::from_id_salt("pack_pick").icon(crate::props::combo_icon)
                        .selected_text(if id.is_some() {
                            pack.name.clone()
                        } else {
                            format!("{} (built in)", pack.name)
                        })
                        .show_ui(ui, |ui| {
                            for (pid, name, default) in &packs {
                                if ui.selectable_label(*default, name).clicked() {
                                    make_default = Some(*pid);
                                }
                            }
                        });
                    ui.label("Name");
                    if crate::props::text_field(ui, egui::TextEdit::singleline(&mut pack.name), 140.0)
                        .changed()
                    {
                        label = "Rename style pack";
                    }
                    let add = crate::props::tool_button(ui, "plus", "Add a shipped pack");
                    egui::Popup::menu(&add).show(|ui| {
                        for p in StylePack::shipped() {
                            if ui.button(&p.name).on_hover_text(&p.description).clicked() {
                                action = Some(if p.name == "Standard" { "standard" } else { "light" });
                                ui.close();
                            }
                        }
                    });
                    if crate::props::tool_button(ui, "folder-open", "Load a pack from a file…").clicked() {
                        action = Some("load");
                    }
                    if crate::props::tool_button(ui, "save", "Save this pack as…").clicked() {
                        action = Some("save");
                    }
                });
                if !pack.description.is_empty() {
                    ui.label(egui::RichText::new(&pack.description).weak());
                }
                // what else the pack carries, one row each, picked and made
                // in Libraries and the Patterns area (REV00, section 3)
                ui.add_space(2.0);
                let d = StylePack::standard();
                let tags = match &pack.marker_style {
                    Some(n) => n.clone(),
                    None if pack.markers == d.markers && pack.room_tags == d.room_tags => "CreaBIM's own".into(),
                    None => "The pack's own".into(),
                };
                let symbols = match pack.symbols.len() {
                    0 => "CreaBIM's own".to_string(),
                    1 => "1 kind picked".to_string(),
                    n => format!("{n} kinds picked"),
                };
                let patterns = match pack.patterns.len() {
                    0 => "Built in only".to_string(),
                    1 => "1 of the project's own".to_string(),
                    n => format!("{n} of the project's own"),
                };
                for (what, value, link, tip, ask) in [
                    ("Tag style", tags, "Libraries", "Tags and marks: picked and designed in Libraries › Tags", "tags"),
                    ("Symbol set", symbols, "Libraries", "The symbol each kind of mark draws with: Libraries › Symbols", "symbols"),
                    ("Patterns", patterns, "Patterns", "Hatch and surface patterns: drawn in the Patterns area, more in Libraries › Patterns", "patterns"),
                ] {
                    ui.horizontal(|ui| {
                        ui.add_sized([96.0, 22.0], egui::Label::new(egui::RichText::new(what).color(ui.visuals().weak_text_color())));
                        ui.add_sized([150.0, 22.0], egui::Label::new(value).truncate());
                        if ui.small_button(format!("{link} ›")).on_hover_text(tip).clicked() {
                            go = Some(ask);
                        }
                    });
                }
                ui.add_space(4.0);
                // the four pages as one segmented control (DSM05)
                ui.allocate_ui(egui::vec2(ui.available_width().min(760.0), ui.spacing().interact_size.y), |ui| {
                    crate::props::segment(
                        ui,
                        &mut self.gstyles.page,
                        &[
                            (Page::Objects, "Object styles", "How each category draws"),
                            (Page::Pens, "Pens", "Pen sets and the pens' thickness per scale"),
                            (Page::Palette, "Palette", "The colours drawings take: paper, ink, fills, from a few settings"),
                            (Page::Templates, "Templates", "View templates"),
                        ],
                    );
                });
                ui.add_space(6.0);
                egui::ScrollArea::vertical().max_height(ui.available_height().max(420.0)).show(ui, |ui| match self.gstyles.page {
                    Page::Objects => {
                        if super::stylepages::objects_page(ui, &mut pack, &mut self.gstyles.pages, dark) {
                            label = "Object style";
                        }
                    }
                    Page::Pens => {
                        if super::stylepages::pens_page(ui, &mut pack, &mut self.gstyles.pages, dark) {
                            label = "Pens";
                        }
                    }
                    Page::Palette => {
                        let seen = self.gstyles.pages.seen;
                        if super::palettepage::palette_page(ui, &mut pack, &mut self.gstyles.palette, seen, dark) {
                            label = "Palette";
                        }
                    }
                    // in Libraries and the Patterns area now
                    Page::Marks | Page::Symbols => {}
                    Page::Templates => {
                        if templates_page(ui, &mut pack, &mut self.gstyles.template, &views, &mut give_template) {
                            label = "View template";
                        }
                    }
                });
            });
        self.gstyles.open = open;
        if pack != before {
            self.end_preview();
            let (id, _) = self.edited_pack();
            self.commit_pack(id, pack.clone(), label);
        }
        match go {
            Some("tags") => self.open_libraries(Some(super::librariesui::LibKind::Tags)),
            Some("symbols") => self.open_libraries(Some(super::librariesui::LibKind::Symbols)),
            Some("patterns") => self.open_patterns(None),
            _ => {}
        }
        if let Some(p) = make_default {
            let tx = styles::make_default(self.doc.model(), p);
            let r = self.doc.commit(tx);
            self.report(r, "Default style pack changed.");
        }
        if let Some(name) = give_template {
            let tx = styles::set_template(self.doc.model(), &views, Some(&name));
            let n = tx.ops.len();
            let r = self.doc.commit(tx);
            self.report(r, format!("{n} views use the template {name}."));
        }
        match action {
            Some("standard") | Some("light") => {
                let p = StylePack::shipped()
                    .into_iter()
                    .find(|p| (p.name == "Standard") == (action == Some("standard")))
                    .expect("shipped");
                self.apply_style_pack(p);
            }
            Some("save") => self.save_style_pack(&pack),
            Some("load") => self.load_style_pack(),
            _ => {}
        }
    }

    /// One of the style pack's pages outside Graphic styles: Libraries'
    /// Tags and Symbols (REV00, section 3). Edits are saved; a tag option
    /// under the pointer is previewed on the plan until it leaves.
    pub(super) fn pack_page_ui(&mut self, ui: &mut egui::Ui, page: Page) {
        let (id, in_model) = self.edited_pack();
        let before = self
            .gstyles
            .preview_base
            .clone()
            .unwrap_or(in_model.clone());
        let mut pack = before.clone();
        let mut hover: Option<StylePack> = None;
        let mut label = "";
        egui::ScrollArea::vertical()
            .id_salt(("pack_page", matches!(page, Page::Marks)))
            .auto_shrink(false)
            .show(ui, |ui| match page {
                Page::Marks => {
                    let (changed, h) =
                        crate::choices::marks_page(ui, &mut pack, &mut self.gstyles.designer);
                    if changed {
                        label = "Tags and marks";
                    }
                    hover = h;
                }
                Page::Symbols
                    if super::stylepages::symbols_page(ui, &mut pack, &mut self.gstyles.pages) =>
                {
                    label = "Symbols";
                }
                _ => {}
            });
        if pack != before {
            self.end_preview();
            let (id, _) = self.edited_pack();
            self.commit_pack(
                id,
                pack,
                if label.is_empty() {
                    "Graphic styles"
                } else {
                    label
                },
            );
        } else if let Some(mut h) = hover {
            h.default = in_model.default;
            // the option under the pointer, shown on the plan until it leaves
            if h != in_model {
                if self.gstyles.preview_base.is_none() {
                    self.gstyles.preview_base = Some(before.clone());
                }
                self.commit_pack(id, h, PREVIEW);
                self.status = "Previewing on the drawings: click to keep it.".into();
            }
        } else {
            self.end_preview();
        }
    }

    /// Take back an option previewed on the plan.
    pub(super) fn end_preview(&mut self) {
        if self.gstyles.preview_base.take().is_some() {
            // a first preview adds the project's pack, and the next ones
            // edit it: each its own step
            while self.doc.undo_label() == Some(PREVIEW) {
                self.doc.discard_last();
            }
            self.status.clear();
        }
    }

    pub(super) fn apply_style_pack(&mut self, p: StylePack) {
        let name = p.name.clone();
        let tx = styles::apply_pack(self.doc.model(), &p);
        let r = self.doc.commit(tx);
        self.report(
            r,
            format!("Style pack {name} applied: every view and sheet draws with it."),
        );
    }

    fn save_style_pack(&mut self, pack: &StylePack) {
        let file = format!("{}.json", pack.name.to_lowercase().replace(' ', "-"));
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save style pack")
            .set_file_name(file)
            .add_filter("Style pack", &["json"])
            .save_file()
        else {
            return;
        };
        let mut p = pack.clone();
        p.default = false;
        let r = std::fs::write(&path, p.to_json());
        self.report(r, format!("Saved {}.", path.display()));
    }

    fn load_style_pack(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Load style pack")
            .add_filter("Style pack", &["json"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| StylePack::from_json(&t))
        {
            Ok(p) => self.apply_style_pack(p),
            Err(e) => self.status = format!("Couldn't read {}: {e}", path.display()),
        }
    }
}

/// A line pattern's sizes, background and line families. True when one
/// changed.
pub(super) fn line_families(ui: &mut egui::Ui, p: &mut HatchPattern) -> bool {
    let mut changed = false;
    crate::props::flow(ui, |ui| {
        ui.label("Sizes");
        changed |= crate::props::radio_tick(ui, &mut p.units, PatternUnits::Paper, "on paper (mm)")
            .changed();
        changed |= crate::props::radio_tick(ui, &mut p.units, PatternUnits::Model, "true size (m)")
            .changed();
    });
    crate::props::flow(ui, |ui| {
        ui.label("Background");
        changed |= colour_pick(
            ui,
            &mut p.fill,
            true,
            "A fill behind the lines; none is white",
        );
    });
    let mut remove = None;
    for (j, l) in p.lines.iter_mut().enumerate() {
        ui.push_id(("line", j), |ui| {
            crate::props::flow(ui, |ui| {
                ui.label("Angle");
                changed |= ui
                    .add(egui::DragValue::new(&mut l.angle).speed(1.0).suffix("°"))
                    .changed();
                ui.label("Spacing");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut l.spacing)
                            .speed(0.01)
                            .range(0.001..=100.0),
                    )
                    .changed();
                ui.label("Offset");
                changed |= ui
                    .add(egui::DragValue::new(&mut l.offset).speed(0.01))
                    .changed();
                ui.label("Stagger");
                changed |= ui
                    .add(egui::DragValue::new(&mut l.shift).speed(0.01))
                    .changed();
                let mut dashes = l
                    .dashes
                    .iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                ui.label("Dashes");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut dashes)
                            .vertical_align(egui::Align::Center)
                            .desired_width(80.0),
                    )
                    .on_hover_text("Dash, gap, dash, gap…, spaced; 0 is a dot. Empty is solid.")
                    .changed()
                {
                    l.dashes = dashes
                        .split_whitespace()
                        .filter_map(|t| t.parse::<f64>().ok())
                        .filter(|d| *d >= 0.0)
                        .collect();
                    changed = true;
                }
                if crate::props::icon_button(ui, "trash-2", "Remove this line family").clicked() {
                    remove = Some(j);
                }
            });
        });
    }
    if let Some(j) = remove {
        p.lines.remove(j);
        changed = true;
    }
    if ui.button("Add a line family").clicked() {
        p.lines.push(HatchLine {
            angle: 45.0,
            spacing: if p.units == PatternUnits::Model {
                0.1
            } else {
                1.5
            },
            offset: 0.0,
            shift: 0.0,
            phase: 0.0,
            dashes: vec![],
            along_longest: false,
        });
        changed = true;
    }
    changed
}

/// The pack's view templates (MU77): the list with its +, duplicate and
/// bin, what each kind of view takes without one under it, and the picked
/// template's settings beside it in sections. True when one changed.
fn templates_page(
    ui: &mut egui::Ui,
    pack: &mut StylePack,
    pick: &mut usize,
    views: &[ElementId],
    give: &mut Option<String>,
) -> bool {
    let mut changed = false;
    let names: Vec<String> = pack.templates.iter().map(|t| t.name.clone()).collect();
    let sets = pack.set_names();
    let pal_names = pack.palette_names();
    let ls_names = pack.line_style_names();
    let wide = ui.available_width() >= 720.0;
    let left_w = if wide { 300.0 } else { ui.available_width() };
    let left = |ui: &mut egui::Ui, pack: &mut StylePack, pick: &mut usize| -> bool {
        let mut changed = false;
        crate::props::section(ui, "Templates");
        let rows: Vec<(Option<String>, String)> = pack
            .templates
            .iter()
            .map(|t| {
                (
                    Some(t.scale.map_or(String::new(), |s| format!("1:{s:.0}"))),
                    t.name.clone(),
                )
            })
            .collect();
        let some = !rows.is_empty();
        let act = crate::props::item_list(ui, "templates", &rows, pick, 7, (true, some, some));
        if act.add {
            pack.templates.push(ViewTemplate::new(&format!(
                "Template {}",
                pack.templates.len() + 1
            )));
            *pick = pack.templates.len() - 1;
            changed = true;
        }
        if act.duplicate && *pick < pack.templates.len() {
            let mut t = pack.templates[*pick].clone();
            t.name = format!("{} copy", t.name);
            pack.templates.insert(*pick + 1, t);
            *pick += 1;
            changed = true;
        }
        if act.remove && *pick < pack.templates.len() {
            pack.templates.remove(*pick);
            *pick = pick.saturating_sub(1);
            changed = true;
        }
        ui.add_space(6.0);
        crate::props::section(ui, "Views without a template take");
        let d = &mut pack.default_templates;
        for (text, slot) in [
            ("Plans", &mut d.plan),
            ("Roof and site", &mut d.plan_from_above),
            ("Sections", &mut d.section),
            ("Elevations", &mut d.elevation),
            ("3D views", &mut d.axonometric),
            ("Details", &mut d.detail),
        ] {
            crate::props::row(ui, text, |ui| {
                ui.spacing_mut().combo_width = ui.available_width();
                changed |= name_combo(ui, ("kind", text), &names, slot, "None");
            });
        }
        changed
    };
    let right = |ui: &mut egui::Ui,
                 pack: &mut StylePack,
                 pick: &mut usize,
                 give: &mut Option<String>|
     -> bool {
        let mut changed = false;
        if pack.templates.is_empty() {
            crate::props::empty(ui, "No view templates yet: + beside the list makes one.");
            return false;
        }
        *pick = (*pick).min(pack.templates.len() - 1);
        let t = &mut pack.templates[*pick];
        crate::props::section(ui, "Template");
        crate::props::row(ui, "Name", |ui| {
            let w = ui.available_width();
            changed |=
                crate::props::text_field(ui, egui::TextEdit::singleline(&mut t.name), w).changed();
        });
        crate::props::section(ui, "Drawing");
        crate::props::row(ui, "Scale", |ui| {
            let mut on = t.scale.is_some();
            if crate::props::tickbox(ui, &mut on, "Its own")
                .on_hover_text("The template sets its views' scale")
                .changed()
            {
                t.scale = on.then_some(100.0);
                changed = true;
            }
            if let Some(s) = &mut t.scale {
                changed |= ui
                    .add_sized(
                        [ui.available_width(), ui.spacing().interact_size.y],
                        egui::DragValue::new(s).range(1.0..=5000.0).prefix("1:"),
                    )
                    .changed();
            }
        });
        crate::props::row(ui, "Style", |ui| {
            let mut on = t.style.is_some();
            if crate::props::tickbox(ui, &mut on, "Its own")
                .on_hover_text("The template sets its views' drawing style")
                .changed()
            {
                t.style = on.then_some(creabim_core::ViewStyle::HiddenLine);
                changed = true;
            }
            if let Some(s) = &mut t.style {
                ui.spacing_mut().combo_width = ui.available_width();
                changed |= view_style_combo(ui, "template_style", s);
            }
        });
        crate::props::row(ui, "Pen set", |ui| {
            ui.spacing_mut().combo_width = ui.available_width();
            changed |= name_combo(
                ui,
                "template_pen_set",
                &sets,
                &mut t.pen_set,
                "The project's",
            );
        });
        // its line style (pens stage 4), the views using it draw in it
        crate::props::row(ui, "Line style", |ui| {
            ui.spacing_mut().combo_width = ui.available_width();
            changed |= name_combo(
                ui,
                "template_line_style",
                &ls_names,
                &mut t.line_style,
                "The project's",
            );
        });
        // its palette (pens stage 7)
        crate::props::row(ui, "Palette", |ui| {
            ui.spacing_mut().combo_width = ui.available_width();
            changed |= name_combo(
                ui,
                "template_palette",
                &pal_names,
                &mut t.palette,
                "The project's",
            );
        });
        // a row as Scale's and Style's, so its tick lines up under theirs (MU78)
        crate::props::row(ui, "Pens", |ui| {
            let mut weights = t.weights.is_some();
            if crate::props::tickbox(ui, &mut weights, "Fixed widths of its own")
                .on_hover_text("Fixed pen widths whatever the scale, instead of the pen set's")
                .changed()
            {
                t.weights = weights.then(|| creabim_core::LineWeights::for_scale(100.0));
                changed = true;
            }
        });
        if let Some(w) = &mut t.weights {
            for (name, v) in w.fields_mut() {
                crate::props::row(ui, name, |ui| {
                    changed |= crate::props::fill(
                        ui,
                        egui::DragValue::new(v)
                            .speed(0.01)
                            .range(0.02..=2.0)
                            .suffix(" mm"),
                    )
                    .changed();
                });
            }
        }
        // its own symbol picks over the project's (Mockup_Symbols_REV00):
        // the marks a view draws; sheet marks follow the project's
        if crate::props::block(ui, "template_symbols", "Symbols", false) {
            for (k, short) in [
                ("arrows", "Leader ends"),
                ("sections", "Sections"),
                ("elevations", "Elevations"),
                ("callouts", "Callouts"),
                ("grids", "Grids"),
                ("levels", "Levels"),
                ("tags", "Tags"),
            ] {
                let Some(kd) = creabim_core::symbols::kind(k) else {
                    continue;
                };
                crate::props::row(ui, short, |ui| {
                    let now = t
                        .symbols
                        .get(k)
                        .and_then(|id| creabim_core::symbols::style(id))
                        .map_or("The project's".to_string(), |s| s.name.clone());
                    egui::ComboBox::from_id_salt(("template_symbol", k))
                        .icon(crate::props::combo_icon)
                        .selected_text(now)
                        .width(ui.available_width())
                        .height(420.0)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(!t.symbols.contains_key(k), "The project's")
                                .clicked()
                                && t.symbols.remove(k).is_some()
                            {
                                changed = true;
                            }
                            for s in &kd.styles {
                                let on =
                                    t.symbols.get(k).map(String::as_str) == Some(s.id.as_str());
                                if ui.selectable_label(on, &s.name).clicked() && !on {
                                    t.symbols.insert(k.to_string(), s.id.clone());
                                    changed = true;
                                }
                            }
                        });
                });
            }
        }
        crate::props::section(ui, "Leaves out");
        egui::Grid::new("leaves_out")
            .num_columns(3)
            .spacing(egui::vec2(12.0, 2.0))
            .show(ui, |ui| {
                for (k, c) in STYLED.iter().enumerate() {
                    let mut on = t.hidden.contains(c);
                    if crate::props::tickbox(ui, &mut on, c.label()).changed() {
                        t.hidden.retain(|x| x != c);
                        if on {
                            t.hidden.push(*c);
                        }
                        changed = true;
                    }
                    if k % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
        crate::props::section(ui, "Depth cues");
        let mut cues = t.cues.unwrap_or_default();
        crate::props::row(ui, "Fade", |ui| {
            ui.spacing_mut().slider_width = (ui.available_width() - 56.0).max(40.0);
            changed |= ui
                .add(egui::Slider::new(&mut cues.fade, 0.0..=0.9).max_decimals(2))
                .on_hover_text("Lines and fills further from the cut or the viewer draw lighter")
                .changed();
        });
        let mut sun = cues.shadows.is_some();
        if crate::props::check(ui, "Shadows", &mut sun, "Sun, in plans from above").changed() {
            cues.shadows = sun.then_some(Shadows {
                direction: -50.0,
                length: 0.6,
                opacity: 0.35,
            });
            changed = true;
        }
        t.cues = (cues != DepthCues::default()).then_some(cues);
        crate::props::worked_out(
            ui,
            "style-pack",
            &[crate::props::Worked::new(
                "Overrides",
                format!("{}, set in the pack's file", t.overrides.len()),
            )],
        );
        if !views.is_empty()
            && crate::props::action(
                ui,
                "Give",
                &format!("To the {} selected views", views.len()),
                "Every selected view takes this template",
                true,
            )
            .clicked()
        {
            *give = Some(t.name.clone());
        }
        changed
    };
    if wide {
        ui.horizontal_top(|ui| {
            ui.allocate_ui(egui::vec2(left_w, 0.0), |ui| {
                ui.set_width(left_w);
                ui.vertical(|ui| changed |= left(ui, pack, pick));
            });
            ui.add_space(16.0);
            let w = ui.available_width().min(520.0);
            ui.allocate_ui(egui::vec2(w, 0.0), |ui| {
                ui.set_width(w);
                ui.vertical(|ui| changed |= right(ui, pack, pick, give));
            });
        });
    } else {
        changed |= left(ui, pack, pick);
        ui.add_space(8.0);
        changed |= right(ui, pack, pick, give);
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shipped_pack_applies_and_undoes_in_one_step() {
        let mut app = crate::app::test_app();
        let before = app.doc.model().clone();
        let light = StylePack::shipped().remove(1);
        app.apply_style_pack(light);
        assert_eq!(styles::active_pack(app.doc.model()).name, "Light linework");
        // editing the pack edits the project's copy, merged per label
        let (id, mut pack) = app.edited_pack();
        let mut set = pack.set("Fine line").unwrap();
        set.tables[0].weights.cut = 0.6;
        pack.pen_sets = vec![set.clone()];
        app.commit_pack(id, pack.clone(), "Pens");
        set.tables[0].weights.cut = 0.65;
        pack.pen_sets = vec![set];
        app.commit_pack(id, pack, "Pens");
        assert_eq!(
            styles::active_pack(app.doc.model()).weights_at(50.0).cut,
            0.65
        );
        app.doc.undo();
        assert_eq!(
            styles::active_pack(app.doc.model()).weights_at(50.0).cut,
            0.5
        );
        app.doc.undo();
        assert_eq!(app.doc.model(), &before);
    }
}
