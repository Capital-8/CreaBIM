//! The two value tables of the Graphic styles window (AMD-016, mockups
//! Pens REV05 and Graphic styles REV01): Object styles, a row per category
//! with its own cut sample, and Pens, a row per pen and a column per scale
//! in the project's pen set. Each has the same fixed sample plan beside
//! it, redrawn with every change, so a style is judged on paper.

use super::*;
use creabim_core::styles::{
    self, LinePattern, ObjectStyle, PenName, PenSet, PenTable, Rgb, StylePack, Weight, SHIPPED_SETS,
};
use creabim_core::visibility::Category;
use creabim_core::LineWeights;
use eframe::egui::{Color32, Pos2, Rect, Shape, Stroke};

/// What the two pages keep between frames.
#[derive(Clone, Debug)]
pub(super) struct PageState {
    /// Object styles' Find.
    pub find: String,
    /// The scale the sample plan is seen at.
    pub seen: f64,
    /// The name being typed for an own pen set.
    pub set_name: Option<String>,
    /// Object styles as a list and the picked category's fields (MU77),
    /// or as the table.
    pub as_list: bool,
    /// The category picked in the list.
    pub pick: usize,
    /// The row picked in the Symbols page's list of kinds.
    pub sym_pick: Option<usize>,
}

impl Default for PageState {
    fn default() -> Self {
        Self {
            find: String::new(),
            seen: 100.0,
            set_name: None,
            as_list: true,
            pick: 1,
            sym_pick: None,
        }
    }
}

/// The categories of the Object styles table, grouped (REV01 decision 2).
pub(super) const GROUPS: [(&str, &[Category]); 6] = [
    (
        "Structure",
        &[
            Category::Walls,
            Category::Slabs,
            Category::Roofs,
            Category::Columns,
            Category::Beams,
            Category::Footings,
        ],
    ),
    (
        "Openings",
        &[Category::Doors, Category::Windows, Category::Openings],
    ),
    ("Circulation", &[Category::Stairs, Category::Railings]),
    (
        "Interiors",
        &[
            Category::Ceilings,
            Category::Objects,
            Category::FreeMeshes,
            Category::Parts,
        ],
    ),
    ("Site", &[Category::Site, Category::Planting]),
    (
        "Annotation",
        &[Category::Rooms, Category::Grids, Category::SectionMarks],
    ),
];

const FIELD_W: f32 = 112.0;
const NUM_W: f32 = 58.0;

/// A weight picker: *As drawn*, the pack's pens by their names (its own
/// pens too), or millimetres.
pub(super) fn weight_combo(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    w: &mut Option<Weight>,
    pack: &StylePack,
) -> bool {
    let mut changed = false;
    let text = w.map_or("As drawn".to_string(), |w| pack.weight_label(w));
    egui::ComboBox::from_id_salt(salt)
        .icon(crate::props::combo_icon)
        .width(
            ui.spacing()
                .combo_width
                .max(FIELD_W)
                .min(ui.available_width()),
        )
        .selected_text(text)
        .show_ui(ui, |ui| {
            changed |= ui.selectable_value(w, None, "As drawn").changed();
            for p in PenName::ALL {
                changed |= ui
                    .selectable_value(w, Some(Weight::Pen(p)), pack.pen_label(p))
                    .changed();
            }
            if !pack.own_pens.is_empty() {
                ui.separator();
                for o in &pack.own_pens {
                    changed |= ui
                        .selectable_value(w, Some(Weight::Own { pen: o.id }), &o.name)
                        .changed();
                }
                ui.separator();
            }
            if ui
                .selectable_label(matches!(w, Some(Weight::Mm(_))), "Millimetres")
                .clicked()
                && !matches!(w, Some(Weight::Mm(_)))
            {
                *w = Some(Weight::Mm(0.25));
                changed = true;
            }
        });
    if let Some(Weight::Mm(mm)) = w {
        changed |= ui
            .add_sized(
                [NUM_W, ui.spacing().interact_size.y],
                egui::DragValue::new(mm)
                    .speed(0.01)
                    .range(0.02..=2.0)
                    .max_decimals(2)
                    .suffix(" mm"),
            )
            .changed();
    }
    changed
}

/// A colour that can be left unset, as a field (REV01 decision 3): its
/// swatch and value, or `unset` in italics; a click opens the picker,
/// where *Clear* sets it back.
pub(super) fn colour_cell(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    c: &mut Option<Rgb>,
    unset: &str,
    fill: bool,
    tip: &str,
) -> bool {
    let w = if ui.spacing().combo_width > FIELD_W {
        ui.spacing().combo_width
    } else {
        FIELD_W
    };
    colour_cell_w(ui, salt, c, unset, fill, tip, w)
}

fn colour_cell_w(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    c: &mut Option<Rgb>,
    unset: &str,
    fill: bool,
    tip: &str,
    width: f32,
) -> bool {
    let mut changed = false;
    let h = ui.spacing().interact_size.y;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, h), egui::Sense::click());
    let popup_id = ui.make_persistent_id(("colour_cell", salt));
    let open = egui::Popup::is_id_open(ui.ctx(), popup_id);
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        let stroke = if open {
            v.selection.stroke
        } else if resp.hovered() {
            v.widgets.hovered.bg_stroke
        } else {
            v.widgets.inactive.bg_stroke
        };
        let p = ui.painter();
        p.rect(
            rect,
            v.widgets.inactive.corner_radius,
            v.text_edit_bg_color(),
            stroke,
            egui::StrokeKind::Inside,
        );
        let font = egui::TextStyle::Body.resolve(ui.style());
        match c {
            Some(Rgb(rgb)) => {
                let col = rgb_of(*rgb);
                let sw = Rect::from_center_size(
                    egui::pos2(rect.left() + 12.0, rect.center().y),
                    egui::Vec2::splat(13.0),
                );
                p.rect(
                    sw,
                    3.0,
                    col,
                    Stroke::new(1.0, Color32::from_black_alpha(60)),
                    egui::StrokeKind::Inside,
                );
                p.text(
                    egui::pos2(rect.left() + 24.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!("#{:02X}{:02X}{:02X}", col.r(), col.g(), col.b()),
                    font,
                    v.text_color(),
                );
            }
            None => {
                // italics as the mockup: an unset value reads as a hint
                let mut job = egui::text::LayoutJob::default();
                job.append(
                    unset,
                    0.0,
                    egui::TextFormat {
                        font_id: font.clone(),
                        color: v.weak_text_color(),
                        italics: true,
                        ..Default::default()
                    },
                );
                let g = ui.painter().layout_job(job);
                p.galley(
                    egui::pos2(rect.left() + 8.0, rect.center().y - g.size().y / 2.0),
                    g,
                    v.weak_text_color(),
                );
            }
        }
    }
    let resp = resp.on_hover_text(tip);
    egui::Popup::menu(&resp)
        .id(popup_id)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            let start = c.map_or(if fill { [0.85; 3] } else { [0.2; 3] }, |c| c.0);
            let col = rgb_of(start);
            let mut srgb = [col.r(), col.g(), col.b()];
            // the house picker (MU101)
            if crate::props::colour_picker(ui, popup_id.with("p"), &mut srgb) {
                *c = Some(Rgb(srgb.map(|v| v as f32 / 255.0)));
                changed = true;
            }
            if c.is_some() && ui.button(format!("Clear: {unset}")).clicked() {
                *c = None;
                changed = true;
                ui.close();
            }
        });
    changed
}

fn rgb_of(c: [f32; 3]) -> Color32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(b(c[0]), b(c[1]), b(c[2]))
}

fn line_pattern_combo(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    p: &mut Option<LinePattern>,
) -> bool {
    line_pattern_combo_w(ui, salt, p, FIELD_W)
}

fn line_pattern_combo_w(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    p: &mut Option<LinePattern>,
    width: f32,
) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(salt)
        .icon(crate::props::combo_icon)
        .width(width)
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
                .max(FIELD_W)
                .min(ui.available_width().max(FIELD_W)),
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

/// Edit the pack's own style for category `c` (none for one material);
/// a style that sets nothing is taken out. True when it changed.
fn with_style(
    pack: &mut StylePack,
    c: Category,
    f: impl FnOnce(&mut ObjectStyle, &StylePack) -> bool,
) -> bool {
    let i = pack
        .objects
        .iter()
        .position(|s| s.category == c && s.material.is_none());
    let mut s = i.map_or_else(|| ObjectStyle::new(c), |i| pack.objects[i].clone());
    let before = s.clone();
    f(&mut s, pack);
    if s == before {
        return false;
    }
    put_style(pack, i, s);
    true
}

fn put_style(pack: &mut StylePack, i: Option<usize>, s: ObjectStyle) {
    let empty = s == ObjectStyle::new(s.category);
    match (i, empty) {
        (Some(i), true) => {
            pack.objects.remove(i);
        }
        (Some(i), false) => pack.objects[i] = s,
        (None, false) => pack.objects.push(s),
        (None, true) => {}
    }
}

/// The shipped pack of the same name: what *Reset to the pack's defaults*
/// goes back to. None for an office's own pack.
fn factory_pack(pack: &StylePack) -> Option<StylePack> {
    StylePack::shipped()
        .into_iter()
        .find(|p| p.name == pack.name)
}

fn default_style(factory: Option<&StylePack>, c: Category) -> ObjectStyle {
    factory
        .and_then(|f| f.object_style(c, None).filter(|s| s.material.is_none()))
        .cloned()
        .unwrap_or_else(|| ObjectStyle::new(c))
}

/// The *Seen at* switch above a sample plan.
fn seen_at(ui: &mut egui::Ui, seen: &mut f64) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("As it draws").strong());
        ui.add_space((ui.available_width() - 228.0).max(8.0));
        ui.label("Seen at");
        ui.allocate_ui(egui::vec2(170.0, ui.spacing().interact_size.y), |ui| {
            let mut s = *seen as i32;
            crate::props::segment(
                ui,
                &mut s,
                &[
                    (50, "1:50", "The pens of drawings at 1:50"),
                    (100, "1:100", "The pens of drawings at 1:100"),
                    (200, "1:200", "The pens of drawings at 1:200"),
                ],
            );
            *seen = s as f64;
        });
    });
}

/// The two parts of a page side by side when there is room, else the
/// drawing under the table (REV01: the narrow window).
fn two_parts<S>(
    ui: &mut egui::Ui,
    state: &mut S,
    table: impl FnOnce(&mut egui::Ui, &mut S),
    drawing: impl FnOnce(&mut egui::Ui, &mut S),
) {
    // the drawing keeps 340 points on the right; the table scrolls
    // sideways in what is left (REV01: narrow, it drops under the table)
    let wide = ui.available_width() >= 820.0;
    if wide {
        ui.horizontal_top(|ui| {
            let w = ui.available_width() - 352.0;
            ui.allocate_ui(egui::vec2(w, 0.0), |ui| {
                ui.set_max_width(w);
                ui.vertical(|ui| {
                    egui::ScrollArea::horizontal()
                        .id_salt("table_side")
                        .max_width(w)
                        .show(ui, |ui| table(ui, state));
                });
            });
            ui.add_space(8.0);
            ui.vertical(|ui| drawing(ui, state));
        });
    } else {
        egui::ScrollArea::horizontal()
            .id_salt("table_side")
            .show(ui, |ui| table(ui, state));
        ui.add_space(8.0);
        ui.allocate_ui(egui::vec2(ui.available_width().min(560.0), 0.0), |ui| {
            ui.vertical(|ui| drawing(ui, state));
        });
    }
}

// ---------------------------------------------------------------------------
// Object styles

/// A category's name in the table, cut with … at a fixed width so long
/// names don't push the columns away (MU77); the whole name on hover.
fn name_cell(ui: &mut egui::Ui, name: &str) {
    const W: f32 = 150.0;
    let h = ui.spacing().interact_size.y;
    let (r, resp) = ui.allocate_exact_size(egui::vec2(W, h), egui::Sense::hover());
    let ink = ui.visuals().text_color();
    let mut job = egui::text::LayoutJob::single_section(
        name.to_string(),
        egui::TextFormat::simple(egui::TextStyle::Body.resolve(ui.style()), ink),
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(W - 4.0);
    let g = ui.painter().layout_job(job);
    let elided = g.elided;
    ui.painter().galley(
        egui::pos2(r.left(), r.center().y - g.size().y / 2.0),
        g,
        ink,
    );
    if elided {
        resp.on_hover_text(name);
    }
}

/// What a category's row says in the list: its cut pen, or *As drawn*.
fn summary(pack: &StylePack, c: Category) -> String {
    match pack.object_style(c, None).filter(|s| s.material.is_none()) {
        Some(s) if *s != ObjectStyle::new(c) => s
            .cut_weight
            .or(s.projection_weight)
            .map_or_else(|| "styled".to_string(), |w| pack.weight_label(w)),
        _ => "As drawn".into(),
    }
}

/// Object styles as a list (MU77, lists option A): the categories grouped
/// in a short list, the picked one's fields as rows beside it.
fn objects_list(
    ui: &mut egui::Ui,
    pack: &mut StylePack,
    st: &mut PageState,
    dark: bool,
    factory: Option<&StylePack>,
) -> bool {
    let mut changed = false;
    let find = st.find.trim().to_lowercase();
    let mut rows: Vec<(Option<String>, String)> = vec![];
    let mut cats: Vec<Option<Category>> = vec![];
    for (group, list) in GROUPS {
        let hits: Vec<Category> = list
            .iter()
            .copied()
            .filter(|c| find.is_empty() || c.label().to_lowercase().contains(&find))
            .collect();
        if hits.is_empty() {
            continue;
        }
        rows.push((None, group.to_string()));
        cats.push(None);
        for c in hits {
            rows.push((Some(summary(pack, c)), c.label().to_string()));
            cats.push(Some(c));
        }
    }
    if cats.iter().all(|c| c.is_none()) {
        crate::props::empty(ui, "No category by that name.");
        return false;
    }
    if cats.get(st.pick).copied().flatten().is_none() {
        st.pick = cats.iter().position(|c| c.is_some()).unwrap_or(0);
    }
    let names = pack.pattern_names();
    let pens = pack.pens_at(None, st.seen);
    let wide = ui.available_width() >= 820.0;
    let list = |ui: &mut egui::Ui, st: &mut PageState| {
        crate::props::item_list(
            ui,
            "object_styles",
            &rows,
            &mut st.pick,
            14,
            (false, false, false),
        );
    };
    let fields = |ui: &mut egui::Ui, pack: &mut StylePack, st: &mut PageState| -> bool {
        let Some(c) = cats.get(st.pick).copied().flatten() else {
            return false;
        };
        let def = default_style(factory, c);
        let snapshot = pack.clone();
        let mut reset = false;
        let k = c.label();
        let mut ch = with_style(pack, c, |s, pk| {
            ui.horizontal(|ui| {
                let look = look_of(&snapshot, &pens, c, Some(&*s));
                cut_sample(ui, &look, dark);
                ui.label(egui::RichText::new(k).strong());
                if *s != def {
                    ui.add_space((ui.available_width() - 24.0).max(4.0));
                    reset = crate::props::icon_button(
                        ui,
                        "undo-2",
                        &format!("{k} back to the pack's defaults"),
                    )
                    .clicked();
                }
            });
            crate::props::section(ui, "Lines");
            let full = |ui: &mut egui::Ui| ui.spacing_mut().combo_width = ui.available_width();
            crate::props::row(ui, "Cut", |ui| {
                full(ui);
                weight_combo(ui, (k, "l_cut"), &mut s.cut_weight, pk)
            });
            crate::props::row(ui, "Projection", |ui| {
                full(ui);
                weight_combo(ui, (k, "l_proj"), &mut s.projection_weight, pk)
            });
            crate::props::row(ui, "Line", |ui| {
                full(ui);
                line_pattern_combo_w(ui, (k, "l_line"), &mut s.pattern, ui.available_width())
            });
            crate::props::row(ui, "Colour", |ui| {
                let w = ui.available_width();
                colour_cell_w(
                    ui,
                    (k, "l_colour"),
                    &mut s.colour,
                    "As drawn",
                    false,
                    "A line colour of its own",
                    w,
                )
            });
            crate::props::section(ui, "Cut faces");
            crate::props::row(ui, "Fill", |ui| {
                let w = ui.available_width();
                colour_cell_w(
                    ui,
                    (k, "l_fill"),
                    &mut s.cut_fill,
                    "Material's",
                    true,
                    "A fill colour behind its cut hatch",
                    w,
                )
            });
            crate::props::row(ui, "Pattern", |ui| {
                full(ui);
                name_combo(
                    ui,
                    (k, "l_pattern"),
                    &names,
                    &mut s.cut_pattern,
                    "Material's",
                )
            });
            crate::props::section(ui, "Surfaces");
            crate::props::row(ui, "Fill", |ui| {
                let w = ui.available_width();
                colour_cell_w(
                    ui,
                    (k, "l_surface"),
                    &mut s.surface_fill,
                    "Material's",
                    true,
                    "A fill for its surfaces seen from above: roofs in a roof plan, hard landscape areas",
                    w,
                )
            });
            true
        });
        if reset {
            let i = pack
                .objects
                .iter()
                .position(|s| s.category == c && s.material.is_none());
            put_style(pack, i, def);
            ch = true;
        }
        ch
    };
    if wide {
        ui.horizontal_top(|ui| {
            ui.allocate_ui(egui::vec2(260.0, 0.0), |ui| {
                ui.set_width(260.0);
                ui.vertical(|ui| list(ui, st));
            });
            ui.add_space(12.0);
            let w = (ui.available_width() - 352.0).clamp(240.0, 360.0);
            ui.allocate_ui(egui::vec2(w, 0.0), |ui| {
                ui.set_width(w);
                ui.vertical(|ui| changed |= fields(ui, pack, st));
            });
            ui.add_space(12.0);
            ui.vertical(|ui| {
                seen_at(ui, &mut st.seen);
                sample_plan(ui, pack, None, st.seen, dark);
            });
        });
    } else {
        list(ui, st);
        ui.add_space(6.0);
        changed |= fields(ui, pack, st);
        ui.add_space(6.0);
        seen_at(ui, &mut st.seen);
        sample_plan(ui, pack, None, st.seen, dark);
    }
    changed
}

/// The object styles, a row per category, grouped, with Find, a reset per
/// row and the sample plan. True when one changed.
pub(super) fn objects_page(
    ui: &mut egui::Ui,
    pack: &mut StylePack,
    st: &mut PageState,
    dark: bool,
) -> bool {
    let mut changed = false;
    let factory = factory_pack(pack);
    let names = pack.pattern_names();
    let pens = pack.pens_at(None, st.seen);
    ui.horizontal(|ui| {
        crate::props::text_field(
            ui,
            egui::TextEdit::singleline(&mut st.find).hint_text("Find a category"),
            240.0,
        );
        crate::props::tickbox(ui, &mut st.as_list, "List view")
            .on_hover_text("The categories as a list with the picked one's fields beside it; untick for the table, every category in a row");
        ui.add_space((ui.available_width() - 200.0).max(8.0));
        let any = GROUPS.iter().flat_map(|g| g.1.iter()).any(|c| {
            pack.object_style(*c, None)
                .filter(|s| s.material.is_none())
                .cloned()
                .unwrap_or_else(|| ObjectStyle::new(*c))
                != default_style(factory.as_ref(), *c)
        });
        let tip = if factory.is_some() {
            "Every category back to how this shipped pack draws it"
        } else {
            "Every category back to As drawn: this pack is the office's own"
        };
        if ui
            .add_enabled(any, egui::Button::new("Reset to the pack's defaults"))
            .on_hover_text(tip)
            .clicked()
        {
            for (_, cats) in GROUPS {
                for c in cats {
                    let i = pack
                        .objects
                        .iter()
                        .position(|s| s.category == *c && s.material.is_none());
                    put_style(pack, i, default_style(factory.as_ref(), *c));
                }
            }
            changed = true;
        }
    });
    ui.add_space(4.0);
    if st.as_list {
        changed |= objects_list(ui, pack, st, dark, factory.as_ref());
        return changed;
    }
    let find = st.find.trim().to_lowercase();
    let seen = &mut st.seen;
    two_parts(
        ui,
        pack,
        |ui, pack| {
            egui::Grid::new("object_styles_table")
                .striped(true)
                .num_columns(10)
                .spacing(egui::vec2(6.0, 4.0))
                .show(ui, |ui| {
                    for h in [
                        "",
                        "Category",
                        "Cut",
                        "Projection",
                        "Line",
                        "Colour",
                        "Cut fill",
                        "Cut pattern",
                        "Surface fill",
                        "",
                    ] {
                        ui.label(egui::RichText::new(h).strong());
                    }
                    ui.end_row();
                    for (group, cats) in GROUPS {
                        let rows: Vec<Category> = cats
                            .iter()
                            .copied()
                            .filter(|c| find.is_empty() || c.label().to_lowercase().contains(&find))
                            .collect();
                        if rows.is_empty() {
                            continue;
                        }
                        ui.label("");
                        ui.label(
                            egui::RichText::new(group.to_uppercase())
                                .size(10.5)
                                .strong()
                                .color(ui.visuals().strong_text_color()),
                        );
                        ui.end_row();
                        for c in rows {
                            let k = c.label();
                            let snapshot = pack.clone();
                            let def = default_style(factory.as_ref(), c);
                            let mut reset = false;
                            changed |= with_style(pack, c, |s, _| {
                                let look = look_of(&snapshot, &pens, c, Some(&*s));
                                cut_sample(ui, &look, dark);
                                name_cell(ui, k);
                                weight_combo(ui, (k, "cut"), &mut s.cut_weight, &snapshot);
                                weight_combo(ui, (k, "proj"), &mut s.projection_weight, &snapshot);
                                line_pattern_combo(ui, (k, "line"), &mut s.pattern);
                                colour_cell(ui, (k, "colour"), &mut s.colour, "As drawn", false, "A line colour of its own");
                                colour_cell(ui, (k, "fill"), &mut s.cut_fill, "Material's", true, "A fill colour behind its cut hatch");
                                name_combo(ui, (k, "cut_pattern"), &names, &mut s.cut_pattern, "Material's");
                                colour_cell(
                                    ui,
                                    (k, "surface"),
                                    &mut s.surface_fill,
                                    "Material's",
                                    true,
                                    "A fill for its surfaces seen from above: roofs in a roof plan, hard landscape areas",
                                );
                                if *s != def {
                                    reset = crate::props::icon_button(
                                        ui,
                                        "undo-2",
                                        &format!("{k} back to the pack's defaults"),
                                    )
                                    .clicked();
                                } else {
                                    ui.label("");
                                }
                                ui.end_row();
                                true
                            });
                            if reset {
                                let i = pack
                                    .objects
                                    .iter()
                                    .position(|s| s.category == c && s.material.is_none());
                                put_style(pack, i, def);
                                changed = true;
                            }
                        }
                    }
                });
            crate::props::plain_note(ui, "A field that reads As drawn or Material's takes its value from the element or its material; click it to set your own. Styles for one material, surface patterns and draw order are set in the pack's file.");
        },
        |ui, pack| {
            seen_at(ui, seen);
            sample_plan(ui, pack, None, *seen, dark);
            crate::props::plain_note(ui, "A fixed sample plan, as on the Pens page. It redraws with every change, so a style is judged on paper, not by reading a row.");
        },
    );
    changed
}

// ---------------------------------------------------------------------------
// Pens

fn pen_mut(w: &mut LineWeights, p: PenName) -> &mut f64 {
    match p {
        PenName::Cut => &mut w.cut,
        PenName::Projection => &mut w.projection,
        PenName::Symbol => &mut w.symbol,
        PenName::Fine => &mut w.fine,
        PenName::Hatch => &mut w.hatch,
        PenName::Hidden => &mut w.hidden,
    }
}

/// What CreaBIM draws with a built-in pen, for a pen no category names.
fn drawn_by(p: PenName) -> &'static str {
    match p {
        PenName::Cut => "what is cut",
        PenName::Projection => "what is seen",
        PenName::Symbol => "door and window symbols",
        PenName::Fine => "furniture and fixtures",
        PenName::Hatch => "hatching",
        PenName::Hidden => "hidden lines",
    }
}

/// Keep `set` in the pack: a shipped set at its factory values is not
/// kept (it is read from CreaBIM), anything else is.
fn store_set(p: &mut StylePack, set: PenSet) {
    let factory = styles::shipped_set(&set.name);
    let i = p.pen_sets.iter().position(|s| s.name == set.name);
    if factory.as_ref() == Some(&set) {
        if let Some(i) = i {
            p.pen_sets.remove(i);
        }
        return;
    }
    match i {
        Some(i) => p.pen_sets[i] = set,
        None => p.pen_sets.push(set),
    }
}

fn unique(p: &StylePack, base: &str) -> String {
    let names = p.set_names();
    if !names.iter().any(|n| n == base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|n| !names.contains(n))
        .expect("a free name")
}

fn width_field(ui: &mut egui::Ui, v: &mut f64, tip: &str, on: bool) -> bool {
    let mut x = *v;
    let r = ui
        .add_sized(
            [NUM_W, ui.spacing().interact_size.y],
            egui::DragValue::new(&mut x)
                .speed(0.01)
                .range(0.02..=2.0)
                .fixed_decimals(2),
        )
        .on_hover_text(tip);
    if on {
        let a = crate::theme::Palette::of(ui.ctx()).accent;
        ui.painter().line_segment(
            [
                r.rect.left_bottom() + egui::vec2(3.0, 1.0),
                r.rect.right_bottom() + egui::vec2(-3.0, 1.0),
            ],
            Stroke::new(1.5, a),
        );
    }
    if r.changed() && x != *v {
        *v = x;
        return true;
    }
    false
}

/// A pen's sample: a short line at its width on paper.
fn pen_sample(ui: &mut egui::Ui, mm: f64, dashed: bool, dark: bool) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(46.0, 20.0), egui::Sense::hover());
    let paper = paper_rgb(dark);
    ui.painter().rect_filled(r, 3.0, paper);
    let ink = ink_rgb(dark);
    let w = (mm as f32 * 6.0).max(0.6);
    let (a, b) = (
        r.left_center() + egui::vec2(5.0, 0.0),
        r.right_center() - egui::vec2(5.0, 0.0),
    );
    if dashed {
        ui.painter()
            .extend(Shape::dashed_line(&[a, b], Stroke::new(w, ink), 5.0, 3.0));
    } else {
        ui.painter().line_segment([a, b], Stroke::new(w, ink));
    }
}

/// The Pens page: the project's pen set picked, saved as, reset or
/// deleted; one table of its pens, a column per scale; the sample plan and
/// who uses which pen. True when something changed.
#[allow(clippy::needless_range_loop)]
pub(super) fn pens_page(
    ui: &mut egui::Ui,
    pack: &mut StylePack,
    st: &mut PageState,
    dark: bool,
) -> bool {
    // a pack from before pen sets: its tables become a set only once it is
    // edited, so opening the page changes nothing
    let base = pack.normalised();
    let mut p = base.clone();
    let cur = p.default_set_name();
    let shipped = SHIPPED_SETS.contains(&cur.as_str());
    let factory = styles::shipped_set(&cur);
    let mut set = p.set(&cur).expect("the project's set");
    set.tables.sort_by(|a, b| a.up_to.total_cmp(&b.up_to));
    let set_changed = factory
        .as_ref()
        .is_some_and(|f| f.built_in_part() != set.built_in_part());
    let names = p.set_names();
    let mut pick: Option<String> = None;
    let mut save_as = false;
    let mut reset_set = false;
    let mut delete_set = false;
    let mut rename: Option<String> = None;
    // the set row
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Pen set").strong());
        egui::ComboBox::from_id_salt("pen_set_pick")
            .icon(crate::props::combo_icon)
            .width(170.0)
            .selected_text(&cur)
            .show_ui(ui, |ui| {
                ui.label(egui::RichText::new("Shipped").weak().size(10.5));
                for n in &names {
                    if SHIPPED_SETS.contains(&n.as_str()) && ui.selectable_label(*n == cur, n).clicked() {
                        pick = Some(n.clone());
                    }
                }
                if names.len() > SHIPPED_SETS.len() {
                    ui.label(egui::RichText::new("Yours").weak().size(10.5));
                    for n in &names {
                        if !SHIPPED_SETS.contains(&n.as_str())
                            && ui.selectable_label(*n == cur, n).clicked()
                        {
                            pick = Some(n.clone());
                        }
                    }
                }
            })
            .response
            .on_hover_text("The project draws with this set. A view or a view template can pick another (Properties › View).");
        if !shipped {
            let mut name = st.set_name.clone().unwrap_or_else(|| cur.clone());
            let r = crate::props::text_field(ui, egui::TextEdit::singleline(&mut name), 150.0)
                .on_hover_text("Your set's name");
            if r.changed() {
                st.set_name = Some(name.clone());
            }
            if r.lost_focus() {
                let n = name.trim().to_string();
                if !n.is_empty() && n != cur && !names.contains(&n) {
                    rename = Some(n);
                }
                st.set_name = None;
            }
        }
        if ui
            .button("Duplicate")
            .on_hover_text("A copy of this set to tweak, as a set of your own: the set you start from stays as it is")
            .clicked()
        {
            save_as = true;
        }
        let tip = if !shipped {
            "Your own set has no factory values"
        } else if set_changed {
            "Reset this set to its factory values"
        } else {
            "This set is at its factory values"
        };
        if ui
            .add_enabled_ui(shipped && set_changed, |ui| {
                crate::props::icon_button(ui, "undo-2", tip)
            })
            .inner
            .clicked()
        {
            reset_set = true;
        }
        let tip = if shipped { "Shipped sets stay" } else { "Delete this set" };
        if ui
            .add_enabled_ui(!shipped, |ui| crate::props::icon_button(ui, "trash-2", tip))
            .inner
            .clicked()
        {
            delete_set = true;
        }
        let chip = match (shipped, set_changed) {
            (true, true) => "shipped · changed",
            (true, false) => "shipped",
            _ => "yours",
        };
        crate::props::chip(ui, chip, shipped && set_changed);
    });
    let about = styles::shipped_set_about(&cur);
    crate::props::plain_note(
        ui,
        if about.is_empty() {
            "Your own set. Thicknesses are on paper, in mm."
        } else {
            about
        },
    );
    ui.add_space(4.0);
    let seen = st.seen;
    let band = set
        .tables
        .iter()
        .position(|t| seen <= t.up_to)
        .unwrap_or(set.tables.len().saturating_sub(1));
    let mut new_pen = false;
    let mut remove_pen: Option<u32> = None;
    let mut add_scale = false;
    let mut remove_scale: Option<usize> = None;
    let snapshot = p.clone();
    let n_tables = set.tables.len();
    let mut table_part = |ui: &mut egui::Ui, set: &mut PenSet, p: &mut StylePack| {
        egui::Grid::new("pen_table")
            .striped(true)
            .num_columns(5 + n_tables)
            .spacing(egui::vec2(6.0, 4.0))
            .show(ui, |ui| {
                ui.label("");
                ui.label(egui::RichText::new("Pen").strong());
                let last = n_tables - 1;
                for i in 0..n_tables {
                    let on = i == band;
                    let ink = if on {
                        crate::theme::Palette::of(ui.ctx()).accent
                    } else {
                        ui.visuals().strong_text_color()
                    };
                    let r = ui
                        .horizontal(|ui| {
                            if i < last {
                                ui.label(egui::RichText::new("up to 1:").color(ink).strong());
                                let mut u = set.tables[i].up_to;
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut u)
                                            .range(1.0..=100000.0)
                                            .max_decimals(0),
                                    )
                                    .on_hover_text(
                                        "This column draws drawings at this scale and larger",
                                    )
                                    .changed()
                                {
                                    set.tables[i].up_to = u;
                                }
                            } else {
                                ui.label(egui::RichText::new("smaller").color(ink).strong())
                                    .on_hover_text("Drawings smaller than the other columns");
                                if crate::props::icon_button(
                                    ui,
                                    "plus",
                                    "Add a scale: a column between the last two",
                                )
                                .clicked()
                                {
                                    add_scale = true;
                                }
                            }
                        })
                        .response;
                    if n_tables > 1 {
                        r.context_menu(|ui| {
                            if ui.button("Remove this scale").clicked() {
                                remove_scale = Some(i);
                                ui.close();
                            }
                        });
                    }
                }
                ui.label(egui::RichText::new("Used by").strong());
                ui.label("");
                ui.label("");
                ui.end_row();
                // built in
                ui.label("");
                ui.label(
                    egui::RichText::new("BUILT IN")
                        .size(10.5)
                        .color(ui.visuals().weak_text_color()),
                );
                ui.end_row();
                for pen in PenName::ALL {
                    pen_sample(
                        ui,
                        pen.width(&set.tables[band].weights),
                        pen == PenName::Hidden,
                        dark,
                    );
                    let mut name = p.pen_label(pen);
                    if crate::props::text_field(ui, egui::TextEdit::singleline(&mut name), 140.0)
                        .on_hover_text(format!(
                            "Built in: {}. Rename it if your office calls it otherwise.",
                            drawn_by(pen)
                        ))
                        .changed()
                    {
                        let n = name.trim();
                        if n.is_empty() || n == pen.label() {
                            p.pen_names.remove(&pen);
                        } else {
                            p.pen_names.insert(pen, n.to_string());
                        }
                    }
                    for (i, t) in set.tables.iter_mut().enumerate() {
                        width_field(
                            ui,
                            pen_mut(&mut t.weights, pen),
                            &format!("{} on paper, mm", p.pen_label(pen)),
                            i == band,
                        );
                    }
                    let users =
                        snapshot.pen_users(Weight::Pen(pen), super::stylesui::STYLED.as_slice());
                    let text = match users {
                        0 => drawn_by(pen).to_string(),
                        1 => "1 category".to_string(),
                        n => format!("{n} categories"),
                    };
                    crate::props::chip(ui, &text, users > 0).on_hover_text(
                        "Categories whose cut or projection lines take this pen (Object styles)",
                    );
                    // its own reset, when its widths or name left the factory
                    let factory_w = factory
                        .as_ref()
                        .filter(|f| f.tables.len() == set.tables.len())
                        .map(|f| {
                            f.tables
                                .iter()
                                .map(|t| pen.width(&t.weights))
                                .collect::<Vec<_>>()
                        });
                    let now: Vec<f64> = set.tables.iter().map(|t| pen.width(&t.weights)).collect();
                    let off = factory_w.as_ref().is_some_and(|f| *f != now)
                        || p.pen_names.contains_key(&pen);
                    if off {
                        if crate::props::icon_button(
                            ui,
                            "undo-2",
                            "Back to this pen's factory values",
                        )
                        .clicked()
                        {
                            if let Some(f) = &factory_w {
                                for (t, v) in set.tables.iter_mut().zip(f) {
                                    *pen_mut(&mut t.weights, pen) = *v;
                                }
                            }
                            p.pen_names.remove(&pen);
                        }
                    } else {
                        ui.label("");
                    }
                    ui.add_enabled_ui(false, |ui| {
                        crate::props::icon_button(
                            ui,
                            "trash-2",
                            "Built-in pens stay, so old projects open unchanged",
                        )
                    });
                    ui.end_row();
                }
                if !p.own_pens.is_empty() {
                    ui.label("");
                    ui.label(
                        egui::RichText::new("YOURS")
                            .size(10.5)
                            .color(ui.visuals().weak_text_color()),
                    );
                    ui.end_row();
                }
                let own: Vec<(u32, String)> =
                    p.own_pens.iter().map(|o| (o.id, o.name.clone())).collect();
                for (id, name) in own {
                    let t0 = &set.tables[band];
                    let w0 = t0.own.get(&id).copied().unwrap_or(t0.weights.projection);
                    pen_sample(ui, w0, false, dark);
                    let mut n = name.clone();
                    if crate::props::text_field(ui, egui::TextEdit::singleline(&mut n), 140.0)
                        .on_hover_text("Your own pen: assign it in Object styles or on the right")
                        .changed()
                    {
                        if let Some(o) = p.own_pens.iter_mut().find(|o| o.id == id) {
                            o.name = n.clone();
                        }
                    }
                    for (i, t) in set.tables.iter_mut().enumerate() {
                        let mut v = t.own.get(&id).copied().unwrap_or(t.weights.projection);
                        if width_field(ui, &mut v, &format!("{n} on paper, mm"), i == band) {
                            t.own.insert(id, v);
                        }
                    }
                    let users = snapshot
                        .pen_users(Weight::Own { pen: id }, super::stylesui::STYLED.as_slice());
                    let text = match users {
                        0 => "not used".to_string(),
                        1 => "1 category".to_string(),
                        n => format!("{n} categories"),
                    };
                    crate::props::chip(ui, &text, users > 0);
                    ui.label("");
                    if crate::props::icon_button(
                        ui,
                        "trash-2",
                        "Delete this pen from every set; what used it draws As drawn",
                    )
                    .clicked()
                    {
                        remove_pen = Some(id);
                    }
                    ui.end_row();
                }
            });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let a = crate::theme::Palette::of(ui.ctx()).accent;
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("+ New pen").color(crate::theme::ink_on(a)),
                    )
                    .fill(a),
                )
                .on_hover_text("A pen of your own, in every set: name it, then assign it")
                .clicked()
            {
                new_pen = true;
            }
            crate::props::plain_note(
                ui,
                "Thicknesses are on paper, in mm. A drawing uses the first column its scale fits.",
            );
        });
    };
    let st_seen = &mut st.seen;
    let mut both = (p.clone(), set.clone());
    two_parts(
        ui,
        &mut both,
        |ui, b| table_part(ui, &mut b.1, &mut b.0),
        |ui, b| {
            // the drawing shows the set as edited this frame
            let mut shown = b.0.clone();
            store_set(&mut shown, b.1.clone());
            seen_at(ui, st_seen);
            sample_plan(ui, &shown, Some(&cur), *st_seen, dark);
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Who uses which pen (Object styles)").strong());
            egui::Grid::new("pen_assign")
                .num_columns(2)
                .spacing(egui::vec2(8.0, 4.0))
                .show(ui, |ui| {
                    for (label, c, cut) in [
                        ("Walls, cut", Category::Walls, true),
                        ("Columns, cut", Category::Columns, true),
                        ("Doors, cut", Category::Doors, true),
                        ("Windows, cut", Category::Windows, true),
                        ("Furniture", Category::Objects, false),
                        ("Slab edges", Category::Slabs, false),
                    ] {
                        ui.label(label);
                        with_style(&mut b.0, c, |s, pk| {
                            let w = if cut {
                                &mut s.cut_weight
                            } else {
                                &mut s.projection_weight
                            };
                            weight_combo(ui, ("assign", label), w, pk)
                        });
                        ui.end_row();
                    }
                });
        },
    );
    (p, set) = both;
    // structural edits, after the table
    if add_scale {
        let n = set.tables.len();
        let last = set.tables[n - 1].clone();
        let up = if n >= 2 {
            set.tables[n - 2].up_to * 2.0
        } else {
            200.0
        };
        set.tables.insert(n - 1, PenTable { up_to: up, ..last });
    }
    if let Some(i) = remove_scale {
        if set.tables.len() > 1 {
            set.tables.remove(i);
            let n = set.tables.len();
            set.tables[n - 1].up_to = set.tables[n - 1].up_to.max(100000.0);
        }
    }
    if reset_set {
        if let Some(f) = &factory {
            let mut fresh = f.clone();
            // the user's own pens keep their widths (pens decision 5)
            for (t, old) in fresh.tables.iter_mut().zip(&set.tables) {
                t.own = old.own.clone();
            }
            set = fresh;
        }
    }
    let set_name = set.name.clone();
    store_set(&mut p, set.clone());
    if save_as {
        let n = unique(&p, &format!("{cur} copy"));
        p.pen_sets.push(PenSet {
            name: n.clone(),
            tables: set.tables.clone(),
        });
        p.pen_set = Some(n);
    }
    if let Some(n) = rename {
        if let Some(s) = p.pen_sets.iter_mut().find(|s| s.name == set_name) {
            s.name = n.clone();
        }
        for t in &mut p.templates {
            if t.pen_set.as_deref() == Some(set_name.as_str()) {
                t.pen_set = Some(n.clone());
            }
        }
        p.pen_set = Some(n);
    }
    if delete_set {
        p.pen_sets.retain(|s| s.name != set_name);
        for t in &mut p.templates {
            if t.pen_set.as_deref() == Some(set_name.as_str()) {
                t.pen_set = None;
            }
        }
        p.pen_set = None;
    }
    if let Some(n) = pick {
        if n != p.default_set_name() {
            p.pen_set = Some(n);
        }
    }
    if new_pen {
        let n = (1..)
            .map(|i| format!("Pen {i}"))
            .find(|n| !p.own_pens.iter().any(|o| &o.name == n))
            .expect("a free name");
        p.add_own_pen(&n);
    }
    if let Some(id) = remove_pen {
        p.remove_own_pen(id);
    }
    if p != base {
        *pack = p;
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// The sample plan

/// How one category draws in the sample.
struct SampleLook {
    cut: f32,
    proj: f32,
    ink: Color32,
    dashes: Option<Vec<f64>>,
    fill: Color32,
    hatch: Hatching,
    surface: Option<Color32>,
}

#[derive(Clone, Copy, PartialEq)]
enum Hatching {
    None,
    Solid,
    Diagonal,
    Dots,
}

pub(crate) fn paper_rgb(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(34)
    } else {
        Color32::WHITE
    }
}

pub(crate) fn ink_rgb(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(225)
    } else {
        Color32::from_gray(27)
    }
}

/// The pen CreaBIM cuts or sees a category with when its style leaves it
/// *As drawn*.
fn default_pens(c: Category, w: &LineWeights) -> (f64, f64) {
    match c {
        Category::Doors | Category::Windows => (w.symbol, w.symbol),
        Category::Objects => (w.fine, w.fine),
        Category::Beams | Category::Ceilings | Category::Footings => (w.hidden, w.hidden),
        _ => (w.cut, w.projection),
    }
}

fn look_of(
    pack: &StylePack,
    pens: &styles::PenWidths,
    c: Category,
    style: Option<&ObjectStyle>,
) -> SampleLook {
    let s = style.or_else(|| pack.object_style(c, None).filter(|s| s.material.is_none()));
    let (dc, dp) = default_pens(c, &pens.weights);
    let cut = s.and_then(|s| s.cut_weight).map_or(dc, |w| pens.mm(w)) as f32;
    let proj = s
        .and_then(|s| s.projection_weight)
        .map_or(dp, |w| pens.mm(w)) as f32;
    let ink = s
        .and_then(|s| s.colour)
        .map_or(Color32::from_gray(27), |c| rgb_of(c.0));
    let dashes = s.and_then(|s| s.pattern.as_ref()).and_then(|p| p.dashes());
    let hatch = match s.and_then(|s| s.cut_pattern.as_deref()) {
        None => match c {
            Category::Walls => Hatching::Diagonal,
            Category::Columns | Category::Slabs | Category::Footings => Hatching::Dots,
            _ => Hatching::None,
        },
        Some(n) => {
            let n = n.to_lowercase();
            if n.contains("solid") || n.contains("poch") {
                Hatching::Solid
            } else if n.contains("concrete") || n.contains("dot") || n.contains("gravel") {
                Hatching::Dots
            } else if n.contains("none") || n.contains("empty") {
                Hatching::None
            } else {
                Hatching::Diagonal
            }
        }
    };
    let fill = s
        .and_then(|s| s.cut_fill)
        .map_or(Color32::from_gray(236), |c| rgb_of(c.0));
    let surface = s.and_then(|s| s.surface_fill).map(|c| rgb_of(c.0));
    SampleLook {
        cut,
        proj,
        ink,
        dashes,
        fill,
        hatch,
        surface,
    }
}

/// A row's cut sample: a short wall in its cut fill, hatch and outline.
fn cut_sample(ui: &mut egui::Ui, l: &SampleLook, dark: bool) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(54.0, 22.0), egui::Sense::hover());
    let pp = Paper {
        rect: r,
        k: 1.0,
        z: 4.0,
        dark,
        painter: ui.painter().clone(),
    };
    pp.painter.rect_filled(r, 3.0, paper_rgb(dark));
    let wall = Rect::from_min_max(r.min + egui::vec2(6.0, 6.0), r.max - egui::vec2(6.0, 6.0));
    pp.fill_rect(wall, l);
    pp.outline(
        &[
            wall.left_top(),
            wall.right_top(),
            wall.right_bottom(),
            wall.left_bottom(),
        ],
        true,
        l.ink,
        l.cut,
        &l.dashes,
    );
}

/// Where the sample plan is drawn: its 420 × 300 units laid into `rect`,
/// `z` pixels per paper mm of pen width at 1:1 of the units.
struct Paper {
    rect: Rect,
    k: f32,
    z: f32,
    dark: bool,
    painter: egui::Painter,
}

impl Paper {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.rect.min + egui::vec2(x * self.k, y * self.k)
    }

    fn col(&self, c: Color32) -> Color32 {
        if self.dark {
            super::paper_dark(c)
        } else {
            c
        }
    }

    fn px(&self, mm: f32) -> f32 {
        (mm * self.z * self.k).max(0.6)
    }

    fn line(&self, pts: &[Pos2], ink: Color32, mm: f32, dashes: &Option<Vec<f64>>) {
        let stroke = Stroke::new(self.px(mm), self.col(ink));
        match dashes {
            Some(d) if d.len() >= 2 => {
                let s = self.z * self.k * 0.9;
                let dash = (d[0] as f32 * s).max(1.0);
                let gap = (d[1] as f32 * s).max(1.0);
                self.painter
                    .extend(Shape::dashed_line(pts, stroke, dash, gap));
            }
            _ => {
                self.painter.add(Shape::line(pts.to_vec(), stroke));
            }
        }
    }

    fn outline(
        &self,
        pts: &[Pos2],
        closed: bool,
        ink: Color32,
        mm: f32,
        dashes: &Option<Vec<f64>>,
    ) {
        let mut v = pts.to_vec();
        if closed {
            v.push(pts[0]);
        }
        self.line(&v, ink, mm, dashes);
    }

    fn rect_u(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(self.at(x0, y0), self.at(x1, y1))
    }

    /// A cut piece's fill and hatch, clipped to it.
    fn fill_rect(&self, r: Rect, l: &SampleLook) {
        let ink = self.col(l.ink);
        if l.hatch == Hatching::Solid {
            self.painter.rect_filled(r, 0.0, ink);
            return;
        }
        self.painter.rect_filled(r, 0.0, self.col(l.fill));
        let clip = self
            .painter
            .with_clip_rect(r.intersect(self.painter.clip_rect()));
        let step = (5.0 * self.k).max(3.0);
        let hw = Stroke::new(0.7, ink.gamma_multiply(0.85));
        match l.hatch {
            Hatching::Diagonal => {
                let span = r.width() + r.height();
                let mut t = -r.height();
                while t < span {
                    let a = egui::pos2(r.left() + t, r.bottom());
                    let b = egui::pos2(r.left() + t + r.height(), r.top());
                    clip.line_segment([a, b], hw);
                    t += step;
                }
            }
            Hatching::Dots => {
                let mut y = r.top() + step * 0.5;
                let mut row = 0;
                while y < r.bottom() {
                    let mut x = r.left() + if row % 2 == 0 { step * 0.4 } else { step * 1.1 };
                    while x < r.right() {
                        clip.circle_filled(egui::pos2(x, y), 0.7, ink);
                        x += step * 1.4;
                    }
                    y += step;
                    row += 1;
                }
            }
            _ => {}
        }
    }
}

/// The fixed sample plan (pens decision 3): a room with its walls,
/// a door, two windows, a column, a stair and rail, a table and chairs,
/// the slab edge and roof overhang, drawn with `pack`'s styles and pen set
/// `set` (none: the project's) as seen at 1:`seen`.
pub(super) fn sample_plan(
    ui: &mut egui::Ui,
    pack: &StylePack,
    set: Option<&str>,
    seen: f64,
    dark: bool,
) {
    let w = ui.available_width().clamp(240.0, 560.0);
    let h = w * 300.0 / 420.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let z = match seen as i32 {
        ..=50 => 10.0,
        51..=100 => 6.0,
        _ => 4.0,
    };
    let pp = Paper {
        rect,
        k: w / 420.0,
        z,
        dark,
        painter: ui.painter().with_clip_rect(rect),
    };
    pp.painter.rect_filled(rect, 4.0, paper_rgb(dark));
    let pens = pack.pens_at(set, seen);
    let look = |c| look_of(pack, &pens, c, None);
    let (walls, slabs, roofs, doors, windows, columns, beams, stairs, rails, furniture, ceilings) = (
        look(Category::Walls),
        look(Category::Slabs),
        look(Category::Roofs),
        look(Category::Doors),
        look(Category::Windows),
        look(Category::Columns),
        look(Category::Beams),
        look(Category::Stairs),
        look(Category::Railings),
        look(Category::Objects),
        look(Category::Ceilings),
    );
    // surfaces under everything
    if let Some(s) = slabs.surface {
        pp.painter.rect_filled(
            pp.rect_u(54.0, 54.0, 366.0, 246.0),
            0.0,
            pp.col(s).gamma_multiply(0.55),
        );
    }
    // roof overhang and slab edge
    let roof_dash = roofs.dashes.clone().or(Some(vec![3.0, 1.4]));
    pp.outline(
        &[
            pp.at(20.0, 20.0),
            pp.at(400.0, 20.0),
            pp.at(400.0, 280.0),
            pp.at(20.0, 280.0),
        ],
        true,
        roofs.ink,
        roofs.proj,
        &roof_dash,
    );
    pp.outline(
        &[
            pp.at(28.0, 28.0),
            pp.at(392.0, 28.0),
            pp.at(392.0, 272.0),
            pp.at(28.0, 272.0),
        ],
        true,
        slabs.ink,
        slabs.proj,
        &slabs.dashes,
    );
    // ceiling over the dining corner, above the cut
    let ceil_dash = ceilings.dashes.clone().or(Some(vec![1.0, 1.0]));
    pp.outline(
        &[
            pp.at(236.0, 170.0),
            pp.at(356.0, 170.0),
            pp.at(356.0, 236.0),
            pp.at(236.0, 236.0),
        ],
        true,
        ceilings.ink,
        ceilings.proj,
        &ceil_dash,
    );
    // stair, rail
    if let Some(s) = stairs.surface {
        pp.painter.rect_filled(
            pp.rect_u(60.0, 150.0, 130.0, 240.0),
            0.0,
            pp.col(s).gamma_multiply(0.55),
        );
    }
    pp.outline(
        &[
            pp.at(60.0, 150.0),
            pp.at(130.0, 150.0),
            pp.at(130.0, 240.0),
            pp.at(60.0, 240.0),
        ],
        true,
        stairs.ink,
        stairs.proj,
        &stairs.dashes,
    );
    for i in 1..9 {
        let y = 150.0 + i as f32 * 10.0;
        pp.line(
            &[pp.at(60.0, y), pp.at(130.0, y)],
            stairs.ink,
            stairs.proj,
            &stairs.dashes,
        );
    }
    pp.line(
        &[pp.at(95.0, 236.0), pp.at(95.0, 160.0)],
        stairs.ink,
        pens.weights.fine as f32,
        &None,
    );
    pp.line(
        &[pp.at(90.0, 168.0), pp.at(95.0, 160.0), pp.at(100.0, 168.0)],
        stairs.ink,
        pens.weights.fine as f32,
        &None,
    );
    pp.line(
        &[pp.at(136.0, 150.0), pp.at(136.0, 240.0)],
        rails.ink,
        rails.proj * 1.4,
        &rails.dashes,
    );
    // beams over, hidden
    let beam_dash = beams.dashes.clone().or(Some(vec![2.0, 1.0]));
    pp.line(
        &[pp.at(224.0, 144.0), pp.at(286.0, 144.0)],
        beams.ink,
        beams.proj,
        &beam_dash,
    );
    pp.line(
        &[pp.at(224.0, 152.0), pp.at(286.0, 152.0)],
        beams.ink,
        beams.proj,
        &beam_dash,
    );
    // furniture: a table and six chairs
    let f = |x0: f32, y0: f32, x1: f32, y1: f32| {
        if let Some(s) = furniture.surface {
            pp.painter.rect_filled(
                pp.rect_u(x0, y0, x1, y1),
                0.0,
                pp.col(s).gamma_multiply(0.55),
            );
        }
        pp.outline(
            &[pp.at(x0, y0), pp.at(x1, y0), pp.at(x1, y1), pp.at(x0, y1)],
            true,
            furniture.ink,
            furniture.proj,
            &furniture.dashes,
        );
    };
    f(260.0, 70.0, 340.0, 110.0);
    for (x, y) in [
        (270.0, 62.0),
        (300.0, 62.0),
        (330.0, 62.0),
        (270.0, 118.0),
        (300.0, 118.0),
        (330.0, 118.0),
    ] {
        f(x - 8.0, y - 5.0, x + 8.0, y + 5.0);
    }
    // walls: the cut pieces, their fill and hatch, then the outline
    let pieces = [
        (40.0, 40.0, 100.0, 54.0),
        (170.0, 40.0, 380.0, 54.0),
        (366.0, 54.0, 380.0, 110.0),
        (366.0, 190.0, 380.0, 260.0),
        (250.0, 246.0, 366.0, 260.0),
        (40.0, 246.0, 190.0, 260.0),
        (40.0, 54.0, 54.0, 246.0),
        (210.0, 54.0, 224.0, 150.0),
    ];
    for (x0, y0, x1, y1) in pieces {
        pp.fill_rect(pp.rect_u(x0, y0, x1, y1), &walls);
    }
    let ring: Vec<Pos2> = [
        (40.0, 40.0),
        (380.0, 40.0),
        (380.0, 260.0),
        (250.0, 260.0),
        (250.0, 246.0),
        (366.0, 246.0),
        (366.0, 54.0),
        (54.0, 54.0),
        (54.0, 246.0),
        (190.0, 246.0),
        (190.0, 260.0),
        (40.0, 260.0),
    ]
    .iter()
    .map(|(x, y)| pp.at(*x, *y))
    .collect();
    pp.outline(&ring, true, walls.ink, walls.cut, &walls.dashes);
    pp.outline(
        &[
            pp.at(210.0, 54.0),
            pp.at(210.0, 150.0),
            pp.at(224.0, 150.0),
            pp.at(224.0, 54.0),
        ],
        false,
        walls.ink,
        walls.cut,
        &walls.dashes,
    );
    // windows: a frame across the wall, the glass line
    let paper = paper_rgb(dark);
    for (x0, y0, x1, y1, horizontal) in [
        (100.0, 40.0, 170.0, 54.0, true),
        (366.0, 110.0, 380.0, 190.0, false),
    ] {
        pp.painter
            .rect_filled(pp.rect_u(x0, y0, x1, y1), 0.0, paper);
        pp.outline(
            &[pp.at(x0, y0), pp.at(x1, y0), pp.at(x1, y1), pp.at(x0, y1)],
            true,
            windows.ink,
            windows.cut,
            &windows.dashes,
        );
        let (a, b) = if horizontal {
            (pp.at(x0, (y0 + y1) / 2.0), pp.at(x1, (y0 + y1) / 2.0))
        } else {
            (pp.at((x0 + x1) / 2.0, y0), pp.at((x0 + x1) / 2.0, y1))
        };
        pp.line(&[a, b], windows.ink, windows.proj, &None);
    }
    // the door: jambs, leaf and swing
    pp.line(
        &[pp.at(190.0, 246.0), pp.at(190.0, 260.0)],
        doors.ink,
        doors.cut,
        &None,
    );
    pp.line(
        &[pp.at(250.0, 246.0), pp.at(250.0, 260.0)],
        doors.ink,
        doors.cut,
        &None,
    );
    pp.line(
        &[pp.at(190.0, 260.0), pp.at(190.0, 200.0)],
        doors.ink,
        doors.cut,
        &doors.dashes,
    );
    let arc: Vec<Pos2> = (0..=24)
        .map(|i| {
            let a = std::f32::consts::FRAC_PI_2 * i as f32 / 24.0;
            pp.at(190.0 + 60.0 * a.sin(), 260.0 - 60.0 * a.cos())
        })
        .collect();
    pp.line(&arc, doors.ink, doors.proj, &doors.dashes);
    // the column
    let col = pp.rect_u(286.0, 140.0, 302.0, 156.0);
    pp.fill_rect(col, &columns);
    pp.outline(
        &[
            col.left_top(),
            col.right_top(),
            col.right_bottom(),
            col.left_bottom(),
        ],
        true,
        columns.ink,
        columns.cut,
        &columns.dashes,
    );
    pp.painter.text(
        pp.at(296.0, 206.0),
        egui::Align2::CENTER_CENTER,
        "ceiling over",
        egui::FontId::proportional((9.0 * pp.k).max(8.0)),
        pp.col(Color32::from_gray(90)),
    );
}

/// A symbol painted in `rect`: its strokes, fills and rings, its text slot
/// filled with `text`, in `ink`.
pub(super) fn paint_symbol(
    painter: &egui::Painter,
    rect: egui::Rect,
    s: &creabim_core::symbols::SymbolStyle,
    text: &str,
    ink: egui::Color32,
) {
    use glam::DVec2;
    // the symbol's 32-step grid fills the rect; y up in the symbol
    let unit = (rect.width().min(rect.height()) / 34.0) as f64;
    let c = rect.center();
    let at = DVec2::ZERO;
    let sh = creabim_geom::symbol::shapes(s, at, unit);
    let origin = s
        .text
        .as_ref()
        .map(|t| DVec2::from(t.at))
        .unwrap_or(DVec2::from(s.anchor.at));
    // shapes() puts the text slot on `at`: shift back so the symbol's own
    // centre sits in the rect's
    let shift = origin * unit;
    let to = |p: DVec2| egui::pos2(c.x + (p.x + shift.x) as f32, c.y - (p.y + shift.y) as f32);
    for f in &sh.fills {
        let pts: Vec<egui::Pos2> = f.iter().map(|p| to(*p)).collect();
        if pts.len() >= 3 {
            painter.add(egui::Shape::convex_polygon(pts, ink, egui::Stroke::NONE));
        }
    }
    for (l, w) in &sh.lines {
        let pts: Vec<egui::Pos2> = l.iter().map(|p| to(*p)).collect();
        painter.add(egui::Shape::line(
            pts,
            egui::Stroke::new((w / 0.18 * 1.2) as f32, ink),
        ));
    }
    if !text.is_empty() && s.text.is_some() {
        painter.text(
            to(sh.text_at),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional((unit * 4.2) as f32),
            ink,
        );
    }
}

/// What a kind's text slot shows in the page's samples.
fn sample_text(kind: &str) -> &'static str {
    match kind {
        "sections" => "1",
        "elevations" => "2",
        "north" => "N",
        "callouts" => "3",
        "grids" => "B",
        "levels" => "+3.0",
        "tags" => "D01",
        "slopes" => "1:12",
        "revisions" => "2",
        "scales" => "1:50",
        "centerlines" => "CL",
        "mep" => "S",
        _ => "",
    }
}

/// The Symbols page (Mockup_Symbols_REV00, A on all three, 2026-11-16):
/// the kinds in two groups, the picked kind's ten styles as tiles with
/// CreaBIM's own mark first, and whether it follows the line style.
/// True when the pack changed.
pub(super) fn symbols_page(ui: &mut egui::Ui, pack: &mut StylePack, st: &mut PageState) -> bool {
    use creabim_core::symbols::{kind, MARK_KINDS, TOOL_KINDS};
    let mut changed = false;
    // the list: two groups of kinds, each with its pick
    let mut rows: Vec<(Option<String>, String)> = vec![];
    let mut ids: Vec<Option<&str>> = vec![];
    for (title, kinds) in [
        ("Marks CreaBIM draws", &MARK_KINDS[..]),
        ("With the Symbol tool", &TOOL_KINDS[..]),
    ] {
        rows.push((None, title.into()));
        ids.push(None);
        for k in kinds {
            let Some(kd) = kind(k) else { continue };
            let style = pack
                .symbols
                .get(*k)
                .and_then(|id| creabim_core::symbols::style(id))
                .map_or("CreaBIM's own".to_string(), |s| s.name.clone());
            rows.push((Some(style), kd.name.clone()));
            ids.push(Some(k));
        }
    }
    let default_row = ids.iter().position(|k| *k == Some("grids")).unwrap_or(1);
    let mut pick = st.sym_pick.unwrap_or(default_row);
    ui.horizontal_top(|ui| {
        ui.allocate_ui(egui::vec2(260.0, 460.0), |ui| {
            ui.set_width(260.0);
            crate::props::item_list(ui, "symbol_kinds", &rows, &mut pick, 17, (false, false, false));
        });
        ui.add_space(10.0);
        if ids.get(pick).copied().flatten().is_none() {
            pick = default_row;
        }
        let k = ids[pick].unwrap_or("grids");
        let Some(kd) = kind(k) else { return };
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(&kd.name).strong().size(14.0));
            // where the pick shows, so it can be found
            let shows = match k {
                "arrows" => "Shows at the ends of note and tag leaders.",
                "sections" => "Shows at both ends of section lines in plans.",
                "elevations" => "Shows as elevation marks in plans.",
                "north" => "Shows as the north arrows placed on sheets.",
                "callouts" => "Shows on the corner of detail callouts.",
                "grids" => "Shows as grid bubbles in plans.",
                "levels" => "Shows as level marks in sections and elevations.",
                "tags" => "Shows as the outline of door, window and other tags.",
                "revisions" => "Shows beside revision clouds on sheets.",
                "scales" => "Shows as the scale bars placed on sheets, stretched to their length.",
                _ => "Placed with the Symbol tool in Annotate.",
            };
            ui.label(egui::RichText::new(shows).weak().size(11.5));
            ui.add_space(6.0);
            let current = pack.symbols.get(k).cloned();
            let tile = egui::vec2(112.0, 118.0);
            let per_row = ((ui.available_width() + 8.0) / (tile.x + 8.0)).floor().max(1.0) as usize;
            let v = ui.visuals().clone();
            let ink = v.strong_text_color();
            // CreaBIM's own first, then the ten styles
            let mut choices: Vec<Option<&creabim_core::symbols::SymbolStyle>> = vec![None];
            choices.extend(kd.styles.iter().map(Some));
            for chunk in choices.chunks(per_row) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    for c in chunk {
                        let on = match c {
                            None => current.is_none(),
                            Some(s) => current.as_deref() == Some(s.id.as_str()),
                        };
                        let (r, resp) = ui.allocate_exact_size(tile, egui::Sense::click());
                        let p = ui.painter();
                        p.rect(
                            r,
                            6.0,
                            v.text_edit_bg_color(),
                            egui::Stroke::new(
                                if on { 2.0 } else { 1.0 },
                                if on {
                                    v.selection.stroke.color
                                } else if resp.hovered() {
                                    v.widgets.hovered.bg_stroke.color
                                } else {
                                    v.widgets.inactive.bg_stroke.color
                                },
                            ),
                            egui::StrokeKind::Inside,
                        );
                        let art = egui::Rect::from_min_size(r.min + egui::vec2(16.0, 6.0), egui::vec2(tile.x - 32.0, 72.0));
                        let (name, family) = match c {
                            Some(s) => {
                                paint_symbol(p, art, s, sample_text(k), ink);
                                (s.name.clone(), s.family.to_uppercase())
                            }
                            None => {
                                p.text(
                                    art.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "as today",
                                    egui::FontId::proportional(11.0),
                                    v.weak_text_color(),
                                );
                                ("CreaBIM's own".to_string(), "DEFAULT".to_string())
                            }
                        };
                        let galley = p.layout(
                            name.clone(),
                            egui::FontId::proportional(11.0),
                            v.text_color(),
                            tile.x - 10.0,
                        );
                        p.galley(egui::pos2(r.center().x - galley.size().x / 2.0, r.top() + 80.0), galley, v.text_color());
                        p.text(
                            egui::pos2(r.center().x, r.bottom() - 7.0),
                            egui::Align2::CENTER_CENTER,
                            family,
                            egui::FontId::proportional(9.0),
                            v.weak_text_color(),
                        );
                        let resp = resp.on_hover_text(&name);
                        if resp.clicked() && !on {
                            match c {
                                Some(s) => {
                                    pack.symbols.insert(k.to_string(), s.id.clone());
                                }
                                None => {
                                    pack.symbols.remove(k);
                                }
                            }
                            changed = true;
                        }
                    }
                });
                ui.add_space(8.0);
            }
            let mut follow = pack.symbols_follow.iter().any(|x| x == k);
            if crate::props::check(ui, "", &mut follow, "Follow the line style")
                .on_hover_text("When the view's line style is Hand or Loose, this kind draws its Hand-family symbol instead")
                .changed()
            {
                pack.symbols_follow.retain(|x| x != k);
                if follow {
                    pack.symbols_follow.push(k.to_string());
                }
                changed = true;
            }
        });
    });
    st.sym_pick = Some(pick);
    changed
}

/// A placed symbol's rows in Properties: its kind, its style, its turn
/// and its text (Mockup_Symbols_REV00). The undo label when one changed.
pub(super) fn placed_symbol_ui(
    ui: &mut egui::Ui,
    model: &creabim_core::Model,
    n: &mut creabim_core::Note,
) -> Option<&'static str> {
    use creabim_core::symbols::{kind, TOOL_KINDS};
    let mut label = None;
    let view = match model.get(n.view).map(|e| &e.kind) {
        Some(creabim_core::ElementKind::View(v)) => Some(v.clone()),
        _ => None,
    };
    let shown = n
        .symbol
        .as_ref()
        .and_then(|p| creabim_core::symbols::placed_style(model, view.as_ref(), p));
    let p = n.symbol.as_mut()?;
    // its picture, as the Symbols page draws it
    if let Some(s) = shown {
        let (r, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 64.0), egui::Sense::hover());
        let art = egui::Rect::from_center_size(r.center(), egui::vec2(60.0, 60.0));
        paint_symbol(
            ui.painter(),
            art,
            s,
            &n.text,
            ui.visuals().strong_text_color(),
        );
    }
    crate::props::row(ui, "Kind", |ui| {
        let now = kind(&p.kind).map_or(p.kind.clone(), |k| k.name.clone());
        egui::ComboBox::from_id_salt("placed_symbol_kind")
            .icon(crate::props::combo_icon)
            .selected_text(now)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for k in TOOL_KINDS {
                    let Some(kd) = kind(k) else { continue };
                    if ui.selectable_label(p.kind == k, &kd.name).clicked() && p.kind != k {
                        p.kind = k.to_string();
                        p.style = None;
                        // a new kind starts clean: its own text, its own
                        // turn (MU84: a slope's 1:12 stayed on a point)
                        n.text.clear();
                        p.angle = 0.0;
                        label = Some("Symbol kind");
                    }
                }
            });
    });
    crate::props::row(ui, "Style", |ui| {
        let Some(kd) = kind(&p.kind) else { return };
        let now = p
            .style
            .as_deref()
            .and_then(creabim_core::symbols::style)
            .map_or("The kind's pick".to_string(), |s| s.name.clone());
        egui::ComboBox::from_id_salt("placed_symbol_style")
            .icon(crate::props::combo_icon)
            .selected_text(now)
            .width(ui.available_width())
            .height(420.0)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(p.style.is_none(), "The kind's pick")
                    .on_hover_text("As Libraries › Symbols picks for this kind")
                    .clicked()
                    && p.style.is_some()
                {
                    p.style = None;
                    label = Some("Symbol style");
                }
                for s in &kd.styles {
                    let on = p.style.as_deref() == Some(s.id.as_str());
                    if ui.selectable_label(on, &s.name).clicked() && !on {
                        p.style = Some(s.id.clone());
                        if s.text.is_none() {
                            n.text.clear();
                        }
                        label = Some("Symbol style");
                    }
                }
            });
    });
    crate::props::row(ui, "Turn", |ui| {
        if crate::props::fill(
            ui,
            egui::DragValue::new(&mut p.angle)
                .speed(1.0)
                .range(-360.0..=360.0)
                .suffix("°"),
        )
        .changed()
        {
            label = Some("Turn symbol");
        }
    });
    if shown.is_some_and(|s| s.text.is_some()) {
        crate::props::row(ui, "Text", |ui| {
            let w = ui.available_width();
            if crate::props::text_field(ui, egui::TextEdit::singleline(&mut n.text), w).changed() {
                label = Some("Symbol text");
            }
        });
    }
    label
}

/// A sheet mark's Style row (Mockup_Sheet_Symbols_REV00): its picture and
/// name, opening the bank's styles of `kind_id` as a grid, with the
/// project's pick first and CreaBIM's own drawing second. True when
/// changed.
pub(super) fn mark_style_ui(
    ui: &mut egui::Ui,
    model: &creabim_core::Model,
    kind_id: &str,
    style: &mut Option<String>,
) -> bool {
    use creabim_core::symbols::{kind, style as find};
    let Some(kd) = kind(kind_id) else {
        return false;
    };
    let project = creabim_core::styles::active_pack(model)
        .symbols
        .get(kind_id)
        .and_then(|id| find(id));
    let shown = match style.as_deref() {
        Some("creabim") => None,
        Some(id) => find(id),
        None => project,
    };
    let name = match style.as_deref() {
        None => match project {
            Some(p) => format!("Project's pick \u{b7} {}", p.name),
            None => "Project's pick \u{b7} CreaBIM's".to_string(),
        },
        Some("creabim") => "CreaBIM's own".to_string(),
        Some(_) => shown.map_or("?".to_string(), |s| s.name.clone()),
    };
    let ink = ui.visuals().strong_text_color();
    let mut changed = false;
    crate::props::row(ui, "Style", |ui| {
        // its name only, on one line, as the house dropdowns (MU91: the
        // picture made the row tall and wrapped the name)
        let h = ui.spacing().interact_size.y;
        let w = ui.available_width().max(60.0);
        let (r, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
        let v = ui.visuals();
        let fill = if resp.hovered() {
            v.widgets.hovered.bg_fill
        } else {
            v.widgets.inactive.weak_bg_fill
        };
        ui.painter().rect(
            r,
            v.widgets.inactive.corner_radius,
            fill,
            v.widgets.inactive.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let mut job = egui::text::LayoutJob::single_section(
            name.clone(),
            egui::TextFormat::simple(egui::TextStyle::Button.resolve(ui.style()), v.text_color()),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width((w - 26.0).max(20.0));
        let g = ui.painter().layout_job(job);
        ui.painter().galley(
            egui::pos2(r.left() + 6.0, r.center().y - g.size().y / 2.0),
            g,
            v.text_color(),
        );
        crate::svgicon::paint(
            ui.painter(),
            egui::Rect::from_center_size(
                egui::pos2(r.right() - 10.0, r.center().y),
                egui::vec2(9.0, 9.0),
            ),
            "chevron-down",
            v.weak_text_color(),
        );
        let _ = (shown, ink);
        let resp = resp.on_hover_text("Its look, from the symbol bank (Libraries › Symbols)");
        egui::Popup::menu(&resp)
            .width(260.0)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_width(260.0);
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                        let mut cells: Vec<(
                            Option<String>,
                            String,
                            Option<&'static creabim_core::symbols::SymbolStyle>,
                            String,
                        )> = vec![
                            (
                                None,
                                "Project's pick".into(),
                                project,
                                "Graphic styles".into(),
                            ),
                            (
                                Some("creabim".into()),
                                "CreaBIM's own".into(),
                                None,
                                "Built in".into(),
                            ),
                        ];
                        for s in &kd.styles {
                            cells.push((
                                Some(s.id.clone()),
                                s.name.clone(),
                                Some(s),
                                s.family.clone(),
                            ));
                        }
                        ui.horizontal_wrapped(|ui| {
                            for (id, label, s, fam) in cells {
                                let on = *style == id;
                                let (cr, c) = ui.allocate_exact_size(
                                    egui::vec2(80.0, 86.0),
                                    egui::Sense::click(),
                                );
                                let v = ui.visuals();
                                if on || c.hovered() {
                                    ui.painter().rect(
                                        cr,
                                        6.0,
                                        v.widgets.hovered.bg_fill,
                                        if on {
                                            v.selection.stroke
                                        } else {
                                            egui::Stroke::NONE
                                        },
                                        egui::StrokeKind::Inside,
                                    );
                                }
                                let art = egui::Rect::from_center_size(
                                    cr.center_top() + egui::vec2(0.0, 26.0),
                                    egui::vec2(40.0, 40.0),
                                );
                                match s {
                                    Some(s) => paint_symbol(ui.painter(), art, s, "N", ink),
                                    None => crate::svgicon::paint(
                                        ui.painter(),
                                        art.shrink(8.0),
                                        if kind_id == "north" {
                                            "north-arrow"
                                        } else {
                                            "scale-bar"
                                        },
                                        ink,
                                    ),
                                }
                                let g = ui.painter().layout(
                                    label.clone(),
                                    egui::FontId::proportional(10.0),
                                    v.text_color(),
                                    76.0,
                                );
                                let at = egui::pos2(
                                    cr.center().x - g.size().x / 2.0,
                                    art.bottom() + 3.0,
                                );
                                ui.painter()
                                    .with_clip_rect(cr)
                                    .galley(at, g, v.text_color());
                                let c = c.on_hover_text(format!("{label} \u{b7} {fam}"));
                                if c.clicked() && !on {
                                    *style = id;
                                    changed = true;
                                    ui.close();
                                }
                            }
                        });
                    });
            });
    });
    changed
}
