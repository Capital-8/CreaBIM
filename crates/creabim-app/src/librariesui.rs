//! Manage › Libraries (Mockup_Libraries_REV00, owner 2026-11-18: "let's
//! experiment with Libraries as an area, and if it is sound we keep it"):
//! every library in one area. Kinds on the left, the items as tiles, the
//! picked one on the right with what can be done with it. Patterns,
//! palettes and masters are fully at home here; the other kinds list what
//! they hold and open where they are edited today.

use super::*;
use crate::timecard::{cut_label, mix};
use creabim_core::styles::HatchPattern;

/// The libraries, in the order the list shows them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LibKind {
    #[default]
    Patterns,
    Symbols,
    /// Tags and marks: the tag designer (from Graphic styles, REV00 §3).
    Tags,
    LineStyles,
    Palettes,
    Materials,
    Profiles,
    Masters,
}

impl LibKind {
    const ALL: [(&'static str, &'static [LibKind]); 3] = [
        (
            "Drawing",
            &[
                LibKind::Patterns,
                LibKind::Symbols,
                LibKind::Tags,
                LibKind::LineStyles,
                LibKind::Palettes,
            ],
        ),
        ("Building", &[LibKind::Materials, LibKind::Profiles]),
        ("Sheets", &[LibKind::Masters]),
    ];
    fn label(self) -> &'static str {
        match self {
            LibKind::Patterns => "Patterns",
            LibKind::Symbols => "Symbols",
            LibKind::Tags => "Tags",
            LibKind::LineStyles => "Line styles",
            LibKind::Palettes => "Palettes",
            LibKind::Materials => "Materials",
            LibKind::Profiles => "Profiles",
            LibKind::Masters => "Masters",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            LibKind::Patterns => "hatch",
            LibKind::Symbols => "north-arrow",
            LibKind::Tags => "tag",
            LibKind::LineStyles => "align-left",
            LibKind::Palettes => "palette",
            LibKind::Materials => "layers",
            LibKind::Profiles => "profile",
            LibKind::Masters => "sheet-template",
        }
    }

    /// Shown as the style pack's own page rather than tiles: the tag
    /// designer and the symbol picks, as Graphic styles had them.
    fn is_page(self) -> bool {
        matches!(self, LibKind::Tags | LibKind::Symbols)
    }
}

/// Which items a kind shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    #[default]
    All,
    InProject,
    Yours,
}

/// The Libraries area between frames.
#[derive(Default)]
pub struct LibUi {
    /// The Libraries window is open (owner 2026-11-18: a Manage window, not
    /// an area; areas are for what you edit).
    pub open: bool,
    pub kind: LibKind,
    pub scope: Scope,
    pub find: String,
    /// The picked item, by name.
    pub pick: Option<String>,
}

/// One item as the tiles show it.
struct Item {
    name: String,
    group: String,
    about: String,
    in_project: bool,
    yours: bool,
    look: Look,
}

enum Look {
    Pattern(HatchPattern),
    Colours(Vec<egui::Color32>),
    Symbol(usize, usize),
    Line(f64),
    /// A profile's outline.
    Profile(Vec<glam::DVec2>),
    /// A master: its sheet, border and title block.
    Master(Box<creabim_core::SheetTemplate>),
}

impl CreaBimApp {
    /// The Libraries area's name in its header: the kind shown.
    pub(super) fn libraries_title(&self) -> String {
        self.libs.kind.label().to_string()
    }

    /// Show Libraries in an area: one already showing it, else the area in
    /// front turns into it.
    pub(super) fn open_libraries(&mut self, kind: Option<LibKind>) {
        if let Some(k) = kind {
            self.libs.kind = k;
            self.libs.pick = None;
        }
        self.libs.open = true;
    }

    /// The Libraries window: large and resizable, the area's layout in it.
    pub(super) fn libraries_window(&mut self, ctx: &egui::Context) {
        // a tag option previewed on the plan goes when Tags is left
        if !(self.libs.open && self.libs.kind == LibKind::Tags)
            && self.gstyles.preview_base.is_some()
        {
            self.end_preview();
        }
        if !self.libs.open {
            return;
        }
        let mut open = true;
        let area = self.mat.area.unwrap_or_else(|| ctx.content_rect());
        let w = (area.width() - 120.0).clamp(560.0, 1180.0);
        let h = (area.height() - 80.0).clamp(420.0, 720.0);
        crate::app::shell::window("Libraries")
            .open(&mut open)
            .default_pos(area.center() - egui::vec2(w, h) / 2.0)
            .default_size([w, h])
            .min_width(520.0)
            .min_height(360.0)
            .resizable(true)
            .constrain_to(area)
            .collapsible(false)
            .show(ctx, |ui| {
                let size = ui.available_size().max(egui::vec2(500.0, 340.0));
                let (r, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                self.libraries_area(ui, r);
            });
        self.libs.open = open;
    }

    fn lib_items(&self, kind: LibKind) -> Vec<Item> {
        let (_, pack) = self.edited_pack();
        let model = self.doc.model();
        match kind {
            LibKind::Patterns => {
                let mut out: Vec<Item> = pack
                    .patterns
                    .iter()
                    .filter(|p| !creabim_core::pattern::library().iter().any(|l| l.name == p.name))
                    .map(|p| Item {
                        name: p.name.clone(),
                        group: "Yours".into(),
                        about: String::new(),
                        in_project: true,
                        yours: true,
                        look: Look::Pattern(p.clone()),
                    })
                    .collect();
                out.extend(creabim_core::pattern::library().iter().map(|l| Item {
                    name: l.name.clone(),
                    group: l.group.clone(),
                    about: l.about.clone(),
                    in_project: pack.pattern(&l.name).is_some(),
                    yours: false,
                    look: Look::Pattern(l.pattern.clone()),
                }));
                out
            }
            LibKind::Palettes => pack
                .palette_names()
                .into_iter()
                .map(|n| {
                    let p = pack.palette_named(&n).unwrap_or_default();
                    let r = p.roles();
                    let c = |v: [f32; 3]| crate::paletteshapes::color32(v);
                    Item {
                        about: creabim_core::palette::about(&n).to_string(),
                        in_project: pack.palette.as_deref().unwrap_or("Ink") == n,
                        yours: creabim_core::palette::shipped(&n).is_none(),
                        group: if creabim_core::palette::shipped(&n).is_some() { "Shipped".into() } else { "Yours".into() },
                        look: Look::Colours(vec![c(r.paper), c(r.ink), c(r.fill), c(r.surface), c(r.accent)]),
                        name: n,
                    }
                })
                .collect(),
            LibKind::Symbols => creabim_core::symbols::library()
                .iter()
                .enumerate()
                .flat_map(|(k, kind)| {
                    kind.styles.iter().enumerate().map(move |(s, st)| Item {
                        name: format!("{} · {}", kind.name, st.name),
                        group: kind.name.clone(),
                        about: format!("{} style.", st.family),
                        in_project: false,
                        yours: false,
                        look: Look::Symbol(k, s),
                    })
                })
                .collect(),
            LibKind::Tags => vec![],
            LibKind::LineStyles => creabim_core::linestyle::shipped()
                .into_iter()
                .map(|l| Item {
                    in_project: pack.line_style.as_deref().unwrap_or(creabim_core::linestyle::RULED) == l.name,
                    group: "Shipped".into(),
                    about: "How lines are drawn: ruled, or by hand. Picked for the project in Project › Line style.".into(),
                    yours: false,
                    look: Look::Line(l.wobble),
                    name: l.name,
                })
                .collect(),
            LibKind::Materials => model
                .iter()
                .filter(|e| matches!(e.kind, ElementKind::Material(_)))
                .map(|e| {
                    let colour = match &e.kind {
                        ElementKind::Material(m) => {
                            let c = m.color;
                            egui::Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8)
                        }
                        _ => egui::Color32::GRAY,
                    };
                    Item {
                        name: e.name.clone(),
                        group: "In this project".into(),
                        about: "Edited in the Materials window.".into(),
                        in_project: true,
                        yours: true,
                        look: Look::Colours(vec![colour]),
                    }
                })
                .collect(),
            LibKind::Profiles => creabim_core::profiles::PRESETS
                .iter()
                .map(|n| Item {
                    name: n.to_string(),
                    group: "Shipped".into(),
                    about: "A section for sweeps: opened in the Profiles view.".into(),
                    in_project: false,
                    yours: false,
                    look: Look::Profile(creabim_core::profiles::preset(n).unwrap_or_default()),
                })
                .collect(),
            LibKind::Masters => {
                let mut out: Vec<Item> = creabim_core::titleblock::styles()
                    .into_iter()
                    .map(|(n, t)| Item {
                        name: n.to_string(),
                        group: "Shipped".into(),
                        about: "A master to start from: New master makes the project's own copy, drawn in the Masters view.".into(),
                        in_project: false,
                        yours: false,
                        look: Look::Master(Box::new(t)),
                    })
                    .collect();
                out.extend(model.iter().filter_map(|e| match &e.kind {
                    ElementKind::SheetTemplate(t) => Some(Item {
                        name: e.name.clone(),
                        group: "In this project".into(),
                        about: "The project's own master: Edit opens it in the Masters view.".into(),
                        in_project: true,
                        yours: true,
                        look: Look::Master(Box::new(t.clone())),
                    }),
                    _ => None,
                }));
                out
            }
        }
    }

    /// The Libraries area.
    pub(super) fn libraries_area(&mut self, ui: &mut egui::Ui, body: Rect) {
        let p = self.shell.theme.palette();
        ui.painter().rect_filled(body, 0.0, p.panel2);
        let wide = body.width() >= 760.0;
        let kinds_w = if body.width() >= 520.0 { 160.0 } else { 0.0 };
        let det_w = if wide { 250.0 } else { 0.0 };
        let mid = Rect::from_min_max(
            egui::pos2(body.left() + kinds_w, body.top()),
            egui::pos2(body.right() - det_w, body.bottom()),
        );
        let mut run: Option<String> = None;
        // the kinds
        if kinds_w > 0.0 {
            let r = Rect::from_min_max(body.min, egui::pos2(body.left() + kinds_w, body.bottom()));
            ui.painter().line_segment(
                [r.right_top(), r.right_bottom()],
                egui::Stroke::new(1.0, p.border),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(r.shrink2(egui::vec2(6.0, 8.0)))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            for (group, kinds) in LibKind::ALL {
                child.add_space(4.0);
                child.label(
                    egui::RichText::new(group.to_uppercase())
                        .size(10.0)
                        .strong()
                        .color(p.muted),
                );
                for k in kinds {
                    let on = self.libs.kind == *k;
                    let (rr, resp) = child.allocate_exact_size(
                        egui::vec2(kinds_w - 12.0, 26.0),
                        egui::Sense::click(),
                    );
                    if on {
                        child
                            .painter()
                            .rect_filled(rr, 6.0, mix(p.panel2, p.accent, 0.14));
                    } else if resp.hovered() {
                        child.painter().rect_filled(rr, 6.0, p.raise);
                    }
                    let c = if on { p.accent } else { p.text };
                    crate::svgicon::paint(
                        child.painter(),
                        egui::Rect::from_center_size(
                            egui::pos2(rr.left() + 14.0, rr.center().y),
                            egui::vec2(14.0, 14.0),
                        ),
                        k.icon(),
                        c,
                    );
                    child.painter().text(
                        egui::pos2(rr.left() + 28.0, rr.center().y),
                        egui::Align2::LEFT_CENTER,
                        k.label(),
                        egui::FontId::proportional(12.5),
                        c,
                    );
                    if !k.is_page() {
                        let n = self.lib_items(*k).len();
                        child.painter().text(
                            egui::pos2(rr.right() - 6.0, rr.center().y),
                            egui::Align2::RIGHT_CENTER,
                            n.to_string(),
                            egui::FontId::proportional(11.0),
                            p.muted,
                        );
                    }
                    if resp.clicked() {
                        self.libs.kind = *k;
                        self.libs.pick = None;
                    }
                }
            }
        }
        // Tags and Symbols: the pack's page, whole (REV00, section 3)
        if self.libs.kind.is_page() {
            let r = Rect::from_min_max(egui::pos2(body.left() + kinds_w, body.top()), body.max)
                .shrink2(egui::vec2(10.0, 8.0));
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(r)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            if kinds_w == 0.0 {
                self.lib_kind_combo(&mut child);
                child.add_space(4.0);
            }
            let page = if self.libs.kind == LibKind::Tags {
                super::stylesui::Page::Marks
            } else {
                super::stylesui::Page::Symbols
            };
            self.pack_page_ui(&mut child, page);
            return;
        }
        // the items: a bar, then tiles in groups
        let items = self.lib_items(self.libs.kind);
        let f = self.libs.find.trim().to_lowercase();
        let shown: Vec<&Item> = items
            .iter()
            .filter(|i| match self.libs.scope {
                Scope::All => true,
                Scope::InProject => i.in_project,
                Scope::Yours => i.yours,
            })
            .filter(|i| {
                f.is_empty()
                    || i.name.to_lowercase().contains(&f)
                    || i.group.to_lowercase().contains(&f)
            })
            .collect();
        if self
            .libs
            .pick
            .as_ref()
            .is_none_or(|n| !items.iter().any(|i| &i.name == n))
        {
            self.libs.pick = shown.first().map(|i| i.name.clone());
        }
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(mid.shrink2(egui::vec2(10.0, 8.0)))
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        let ui2 = &mut child;
        ui2.horizontal(|ui| {
            if kinds_w == 0.0 {
                self.lib_kind_combo(ui);
            }
            let w = (ui.available_width() - 262.0).max(80.0);
            crate::props::text_field(
                ui,
                egui::TextEdit::singleline(&mut self.libs.find).hint_text("Find"),
                w,
            );
            let mut scope = self.libs.scope;
            crate::props::segment(
                ui,
                &mut scope,
                &[
                    (Scope::All, "All", "Everything this library holds"),
                    (Scope::InProject, "In project", "What this project uses"),
                    (Scope::Yours, "Yours", "Made or imported by you"),
                ],
            );
            self.libs.scope = scope;
        });
        ui2.add_space(6.0);
        let tile_w = 124.0;
        let pick = self.libs.pick.clone();
        let mut picked: Option<String> = None;
        egui::ScrollArea::vertical()
            .id_salt(("libs.items", self.libs.kind as u8))
            .auto_shrink(false)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show(ui2, |ui| {
                // sized inside the scroll area, its bar already taken off
                let avail = ui.available_width() - 2.0;
                let cols = ((avail + 8.0) / (tile_w + 8.0)).floor().max(2.0);
                let tw = ((avail - (cols - 1.0) * 8.0) / cols).floor() - 0.5;
                if shown.is_empty() {
                    crate::props::empty(ui, "Nothing here: change All, In project or Yours, or what you are looking for.");
                }
                let mut groups: Vec<&str> = shown.iter().map(|i| i.group.as_str()).collect();
                groups.dedup();
                let mut seen = std::collections::HashSet::new();
                for g in groups {
                    if !seen.insert(g) {
                        continue;
                    }
                    let in_g: Vec<&&Item> = shown.iter().filter(|i| i.group == g).collect();
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(g.to_uppercase()).size(10.5).strong().color(p.muted));
                        ui.label(egui::RichText::new(in_g.len().to_string()).size(10.5).color(p.muted));
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                        for it in in_g {
                            let on = pick.as_deref() == Some(it.name.as_str());
                            let (r, resp) = ui.allocate_exact_size(egui::vec2(tw, tw * 0.75 + 26.0), egui::Sense::click());
                            let pv = egui::Rect::from_min_size(r.min, egui::vec2(tw, tw * 0.75));
                            ui.painter().rect_filled(r, 9.0, p.panel);
                            self.lib_paint(ui, pv.shrink(1.0), &it.look);
                            if it.in_project && self.libs.kind != LibKind::Materials {
                                let t = if matches!(self.libs.kind, LibKind::Palettes | LibKind::LineStyles) { "in use" } else { "in project" };
                                let g = ui.painter().layout_no_wrap(t.into(), egui::FontId::proportional(9.5), p.text);
                                let pr = egui::Rect::from_min_size(egui::pos2(pv.right() - g.size().x - 14.0, pv.top() + 5.0), egui::vec2(g.size().x + 10.0, 16.0));
                                ui.painter().rect_filled(pr, 8.0, mix(p.panel, crate::props::Hue::Project.colour(p.dark), 0.3));
                                ui.painter().galley(pr.center() - g.size() / 2.0, g, p.text);
                            }
                            ui.painter().line_segment([pv.left_bottom(), pv.right_bottom()], egui::Stroke::new(1.0, p.border));
                            let name_r = egui::Rect::from_min_max(egui::pos2(r.left() + 7.0, pv.bottom()), egui::pos2(r.right() - 7.0, r.bottom()));
                            let g = ui.painter().layout(it.name.clone(), egui::FontId::proportional(11.5), p.text, f32::INFINITY);
                            ui.painter().with_clip_rect(name_r).galley(egui::pos2(name_r.left(), name_r.center().y - g.size().y / 2.0), g.clone(), p.text);
                            if g.size().x > name_r.width() {
                                // cut: a soft fade at its end, the name on hover
                                ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(name_r.right() - 12.0, name_r.top()), egui::pos2(r.right() - 1.0, r.bottom() - 1.0)), 0.0, p.panel);
                                ui.painter().text(egui::pos2(name_r.right() - 10.0, name_r.center().y), egui::Align2::LEFT_CENTER, "…", egui::FontId::proportional(11.5), p.muted);
                            }
                            let stroke = if on {
                                egui::Stroke::new(1.5, p.accent)
                            } else if resp.hovered() {
                                egui::Stroke::new(1.0, mix(p.border, p.accent, 0.4))
                            } else {
                                egui::Stroke::new(1.0, p.border)
                            };
                            ui.painter().rect_stroke(r, 9.0, stroke, egui::StrokeKind::Inside);
                            let resp = resp.on_hover_text(&it.name);
                            if resp.clicked() {
                                picked = Some(it.name.clone());
                            }
                            if resp.double_clicked() {
                                run = Some("libs.go".into());
                            }
                        }
                    });
                }
            });
        if let Some(n) = picked {
            self.libs.pick = Some(n);
        }
        // the picked one
        let it = self
            .libs
            .pick
            .as_ref()
            .and_then(|n| items.iter().find(|i| &i.name == n));
        if wide {
            let r = Rect::from_min_max(egui::pos2(body.right() - det_w, body.top()), body.max);
            ui.painter()
                .rect_filled(r, 0.0, mix(p.panel2, p.panel, 0.5));
            ui.painter().line_segment(
                [r.left_top(), r.left_bottom()],
                egui::Stroke::new(1.0, p.border),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(r.shrink2(egui::vec2(10.0, 10.0)))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            if let Some(it) = it {
                let w = child.available_width();
                let (pv, _) = child.allocate_exact_size(egui::vec2(w, w), egui::Sense::hover());
                child.painter().rect_filled(pv, 8.0, p.panel);
                self.lib_paint(&mut child, pv.shrink(1.0), &it.look);
                child.painter().rect_stroke(
                    pv,
                    8.0,
                    egui::Stroke::new(1.0, p.border),
                    egui::StrokeKind::Inside,
                );
                child.add_space(4.0);
                cut_label(
                    &mut child,
                    egui::RichText::new(&it.name).strong().size(14.0),
                    w,
                );
                child.label(
                    egui::RichText::new(format!(
                        "{} · {}",
                        it.group,
                        if it.in_project {
                            "in this project"
                        } else {
                            "not in this project"
                        }
                    ))
                    .size(11.5)
                    .color(p.muted),
                );
                if !it.about.is_empty() {
                    child.add(egui::Label::new(egui::RichText::new(&it.about).size(12.0)).wrap());
                }
                child.add_space(8.0);
                self.lib_actions(&mut child, it, &mut run);
            }
        } else if let Some(it) = it {
            // narrow: the picked one in a strip at the foot
            let r = Rect::from_min_max(egui::pos2(mid.left(), body.bottom() - 40.0), body.max);
            ui.painter().rect_filled(r, 0.0, p.panel);
            ui.painter().line_segment(
                [r.left_top(), r.right_top()],
                egui::Stroke::new(1.0, p.border),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(r.shrink2(egui::vec2(10.0, 8.0)))
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            let w = (child.available_width() - 190.0).max(60.0);
            cut_label(&mut child, egui::RichText::new(&it.name).strong(), w);
            child.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.lib_actions(ui, it, &mut run)
            });
        }
        if let Some(r) = run {
            let ctx = ui.ctx().clone();
            self.libs_do(&ctx, &r);
        }
    }

    /// The kinds as a drop-down, when the area is too narrow for the list.
    fn lib_kind_combo(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_id_salt("libs.kind")
            .icon(crate::props::combo_icon)
            .width(110.0)
            .selected_text(self.libs.kind.label())
            .show_ui(ui, |ui| {
                for (_, kinds) in LibKind::ALL {
                    for k in kinds {
                        if ui
                            .selectable_label(self.libs.kind == *k, k.label())
                            .clicked()
                        {
                            self.libs.kind = *k;
                            self.libs.pick = None;
                        }
                    }
                }
            });
    }

    /// What can be done with the picked item.
    fn lib_actions(&self, ui: &mut egui::Ui, it: &Item, run: &mut Option<String>) {
        ui.horizontal_wrapped(|ui| match self.libs.kind {
            LibKind::Patterns => {
                let text = if it.in_project { "Add a copy" } else { "Add to the project" };
                if crate::library::primary(ui, true, text).clicked() {
                    *run = Some("libs.add".into());
                }
                if ui.button("Edit").on_hover_text("Open it in the Patterns area to draw and change it (added to the project first)").clicked() {
                    *run = Some("libs.edit".into());
                }
            }
            LibKind::Palettes => {
                if crate::library::primary(ui, !it.in_project, "Use for the project").clicked() {
                    *run = Some("libs.add".into());
                }
                if ui.button("Edit").on_hover_text("Graphic styles › Palette").clicked() {
                    *run = Some("libs.edit".into());
                }
            }
            LibKind::Masters => {
                if it.in_project {
                    if crate::library::primary(ui, true, "Edit").clicked() {
                        *run = Some("libs.edit".into());
                    }
                } else if crate::library::primary(ui, true, "New master from it").clicked() {
                    *run = Some("libs.add".into());
                }
            }
            // their own page, no tiles
            LibKind::Symbols | LibKind::Tags => {}
            LibKind::LineStyles => {
                if crate::library::primary(ui, !it.in_project, "Use for the project").clicked() {
                    *run = Some("libs.add".into());
                }
            }
            LibKind::Materials => {
                if ui.button("Open in Materials").clicked() {
                    *run = Some("libs.edit".into());
                }
            }
            LibKind::Profiles => {
                if ui.button("Open in Profiles").clicked() {
                    *run = Some("libs.edit".into());
                }
            }
        });
    }

    /// Run what a library's button asked for, on the picked item.
    pub(super) fn libs_do(&mut self, _ctx: &egui::Context, what: &str) {
        let Some(name) = self.libs.pick.clone() else {
            return;
        };
        let what = if what == "libs.go" {
            if matches!(
                self.libs.kind,
                LibKind::Patterns | LibKind::Palettes | LibKind::LineStyles
            ) {
                "libs.add"
            } else {
                "libs.edit"
            }
        } else {
            what
        };
        match (self.libs.kind, what) {
            (LibKind::Patterns, "libs.add") => {
                let Some(l) = creabim_core::pattern::library()
                    .iter()
                    .find(|l| l.name == name)
                else {
                    self.status = "It is the project's already.".into();
                    return;
                };
                let (id, mut pack) = self.edited_pack();
                let mut p = l.pattern.clone();
                p.name = super::patternsui::free_name(&pack, &l.name);
                let label = p.name.clone();
                pack.patterns.push(p);
                self.commit_pack(id, pack, "Add pattern");
                self.status = format!("{label} added to the project: materials can use it now.");
            }
            (LibKind::Patterns, _) => {
                // a library pattern is the project's to change: added first
                let (id, mut pack) = self.edited_pack();
                let open = if pack.pattern(&name).is_some() {
                    name.clone()
                } else if let Some(l) = creabim_core::pattern::library()
                    .iter()
                    .find(|l| l.name == name)
                {
                    let mut p = l.pattern.clone();
                    p.name = super::patternsui::free_name(&pack, &l.name);
                    let n = p.name.clone();
                    pack.patterns.push(p);
                    self.commit_pack(id, pack, "Add pattern");
                    n
                } else {
                    name.clone()
                };
                // the area takes it; the window steps aside
                self.libs.open = false;
                self.open_patterns(Some(open));
            }
            (LibKind::Palettes, "libs.add") => {
                let (id, mut pack) = self.edited_pack();
                pack.palette = (name != "Ink").then(|| name.clone());
                self.commit_pack(id, pack, "Palette");
            }
            (LibKind::Palettes, _) => {
                self.gstyles.open = true;
                self.gstyles.page = super::stylesui::Page::Palette;
            }
            (LibKind::LineStyles, _) => {
                let (id, mut pack) = self.edited_pack();
                pack.line_style = (name != creabim_core::linestyle::RULED).then(|| name.clone());
                self.commit_pack(id, pack, "Line style");
            }
            (LibKind::Symbols | LibKind::Tags, _) => {}
            (LibKind::Masters, "libs.add") => {
                self.masters_action("master.new", &name);
            }
            (LibKind::Masters, _) => {
                let id = self
                    .doc
                    .model()
                    .iter()
                    .find(|e| e.name == name && matches!(e.kind, ElementKind::SheetTemplate(_)))
                    .map(|e| e.id);
                self.open_master(id);
            }
            (LibKind::Materials, _) => {
                let id = self
                    .doc
                    .model()
                    .iter()
                    .find(|e| e.name == name && matches!(e.kind, ElementKind::Material(_)))
                    .map(|e| e.id);
                self.open_materials(id);
            }
            (LibKind::Profiles, _) => {
                self.profiles_action("profiles.open", &name);
            }
        }
    }

    /// An item's picture in `r`. Drawings (patterns, symbols, line styles,
    /// profiles, masters) are on the drawings' paper, dark in a dark theme;
    /// a palette shows its own colours, paper included.
    fn lib_paint(&self, ui: &mut egui::Ui, r: Rect, look: &Look) {
        let p = self.shell.theme.palette();
        let paint = ui.painter().with_clip_rect(r);
        match look {
            Look::Pattern(pat) => {
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r));
                super::patternsui::paint_in(&mut child, r, pat);
            }
            Look::Colours(cs) => {
                let n = cs.len().max(1) as f32;
                if cs.len() == 1 {
                    paint.rect_filled(r.shrink(14.0), 8.0, cs[0]);
                } else {
                    // the paper behind, the rest as swatches on it
                    paint.rect_filled(r, 8.0, cs[0]);
                    let w = (r.width() - 24.0) / (n - 1.0);
                    for (k, c) in cs.iter().skip(1).enumerate() {
                        let rr = egui::Rect::from_min_size(
                            egui::pos2(r.left() + 12.0 + k as f32 * w, r.center().y - 14.0),
                            egui::vec2(w - 6.0, 28.0),
                        );
                        paint.rect_filled(rr, 5.0, *c);
                    }
                }
            }
            Look::Symbol(k, s) => super::paint_drawing(&paint, || {
                paint.rect_filled(r, 8.0, egui::Color32::WHITE);
                if let Some(st) = creabim_core::symbols::library()
                    .get(*k)
                    .and_then(|kd| kd.styles.get(*s))
                {
                    let ink = egui::Color32::from_rgb(30, 34, 40);
                    let sc = (r.height() * 0.6) / 20.0;
                    let to = |q: [f64; 2]| {
                        egui::pos2(
                            r.center().x + q[0] as f32 * sc,
                            r.center().y - q[1] as f32 * sc,
                        )
                    };
                    for f in &st.fills {
                        let pts: Vec<egui::Pos2> = f.pts.iter().map(|q| to(*q)).collect();
                        paint.add(egui::Shape::convex_polygon(pts, ink, egui::Stroke::NONE));
                    }
                    for c in &st.circles {
                        paint.circle_stroke(
                            to([c[0], c[1]]),
                            c[2] as f32 * sc,
                            egui::Stroke::new(1.2, ink),
                        );
                    }
                    for stroke in &st.strokes {
                        let pts: Vec<egui::Pos2> = stroke.pts.iter().map(|q| to(*q)).collect();
                        paint.add(egui::Shape::line(
                            pts,
                            egui::Stroke::new((stroke.weight as f32 * 0.4).max(0.8), ink),
                        ));
                    }
                }
            }),
            Look::Line(wobble) => super::paint_drawing(&paint, || {
                paint.rect_filled(r, 8.0, egui::Color32::WHITE);
                let ink = egui::Color32::from_rgb(30, 34, 40);
                for k in 0..3 {
                    let y = r.top() + r.height() * (0.3 + 0.2 * k as f32);
                    let pts: Vec<egui::Pos2> = (0..=40)
                        .map(|i| {
                            let t = i as f32 / 40.0;
                            let x = r.left() + 14.0 + t * (r.width() - 28.0);
                            let w = (*wobble as f32)
                                * 6.0
                                * ((t * 9.0 + k as f32).sin() * 0.6 + (t * 23.0).sin() * 0.4);
                            egui::pos2(x, y + w)
                        })
                        .collect();
                    paint.add(egui::Shape::line(pts, egui::Stroke::new(1.3, ink)));
                }
            }),
            Look::Profile(pts) => super::paint_drawing(&paint, || {
                // the section, filled as a cut, on paper
                paint.rect_filled(r, 8.0, egui::Color32::WHITE);
                if pts.len() >= 3 {
                    let lo = pts
                        .iter()
                        .fold(glam::DVec2::splat(f64::MAX), |a, p| a.min(*p));
                    let hi = pts
                        .iter()
                        .fold(glam::DVec2::splat(f64::MIN), |a, p| a.max(*p));
                    let span = (hi - lo).max_element().max(1e-6);
                    let k = (r.width().min(r.height()) as f64 * 0.62) / span;
                    let mid = (lo + hi) / 2.0;
                    let to = |p: glam::DVec2| {
                        r.center()
                            + egui::vec2(((p.x - mid.x) * k) as f32, (-(p.y - mid.y) * k) as f32)
                    };
                    let world: Vec<glam::DVec2> = pts.clone();
                    {
                        let tris = creabim_core::polygon::triangulate(&world);
                        let mut mesh = egui::Mesh::default();
                        for p in &world {
                            mesh.colored_vertex(to(*p), egui::Color32::from_gray(205));
                        }
                        for t in tris {
                            mesh.add_triangle(t[0] as u32, t[1] as u32, t[2] as u32);
                        }
                        paint.add(egui::Shape::mesh(mesh));
                    }
                    paint.add(egui::Shape::closed_line(
                        world.iter().map(|p| to(*p)).collect(),
                        egui::Stroke::new(1.4, egui::Color32::from_rgb(30, 34, 40)),
                    ));
                }
            }),
            Look::Master(t) => super::paint_drawing(&paint, || {
                // the sheet, its border and title block, fitted in the tile
                paint.rect_filled(r, 8.0, p.panel2);
                let size = t.paper.unwrap_or(glam::DVec2::new(420.0, 297.0));
                let k =
                    ((r.width() - 16.0) as f64 / size.x).min((r.height() - 16.0) as f64 / size.y);
                let sheet = egui::Rect::from_center_size(
                    r.center(),
                    egui::vec2((size.x * k) as f32, (size.y * k) as f32),
                );
                paint.rect_filled(sheet, 1.0, egui::Color32::WHITE);
                let to = |q: glam::DVec2| {
                    egui::pos2(
                        sheet.left() + (q.x * k) as f32,
                        sheet.bottom() - (q.y * k) as f32,
                    )
                };
                let ink = egui::Color32::from_rgb(30, 34, 40);
                if t.border {
                    let [ml, mb, mr, mt] = t.margins.unwrap_or([20.0, 10.0, 10.0, 10.0]);
                    let b = egui::Rect::from_min_max(
                        to(glam::DVec2::new(ml, size.y - mt)),
                        to(glam::DVec2::new(size.x - mr, mb)),
                    );
                    paint.rect_stroke(
                        b,
                        0.0,
                        egui::Stroke::new(1.0, ink),
                        egui::StrokeKind::Middle,
                    );
                }
                let at = t.corner.at(size.x, size.y);
                lib_master_items(&paint, &t.items, at, k, &to, ink);
            }),
        }
    }
}

/// A master's title block items drawn small: lines, fills and shapes as
/// they are, text as text while it can be read, pictures as boxes.
fn lib_master_items(
    paint: &egui::Painter,
    items: &[creabim_core::titleblock::TitleItem],
    at: glam::DVec2,
    k: f64,
    to: &dyn Fn(glam::DVec2) -> egui::Pos2,
    ink: egui::Color32,
) {
    use creabim_core::titleblock::{TextAlign, TitleItem};
    let w = |mm: f64| ((mm * k) as f32).clamp(0.6, 2.0);
    for it in items {
        match it {
            TitleItem::Line { points, width } => {
                let pts: Vec<egui::Pos2> = points.iter().map(|q| to(at + *q)).collect();
                paint.add(egui::Shape::line(pts, egui::Stroke::new(w(*width), ink)));
            }
            TitleItem::Fill { points, gray } => {
                let pts: Vec<egui::Pos2> = points.iter().map(|q| to(at + *q)).collect();
                let g = (gray.clamp(0.0, 1.0) * 255.0) as u8;
                paint.add(egui::Shape::convex_polygon(
                    pts,
                    egui::Color32::from_gray(g),
                    egui::Stroke::NONE,
                ));
            }
            TitleItem::Shape {
                points,
                closed,
                pen,
                colour,
                fill,
                ..
            } => {
                let pts: Vec<egui::Pos2> = points.iter().map(|q| to(at + *q)).collect();
                let c = |v: [f32; 3]| {
                    egui::Color32::from_rgb(
                        (v[0] * 255.0) as u8,
                        (v[1] * 255.0) as u8,
                        (v[2] * 255.0) as u8,
                    )
                };
                if let (Some(f), true) = (fill, *closed) {
                    paint.add(egui::Shape::convex_polygon(
                        pts.clone(),
                        c(*f),
                        egui::Stroke::NONE,
                    ));
                }
                let st = egui::Stroke::new(w(pen.unwrap_or(0.25)), c(*colour));
                paint.add(if *closed {
                    egui::Shape::closed_line(pts, st)
                } else {
                    egui::Shape::line(pts, st)
                });
            }
            TitleItem::Text {
                at: tp,
                size,
                text,
                align,
                ..
            } => {
                let px = (size * k) as f32;
                let p0 = to(at + *tp);
                let anchor = match align {
                    TextAlign::Start => egui::Align2::LEFT_CENTER,
                    TextAlign::Middle => egui::Align2::CENTER_CENTER,
                    TextAlign::End => egui::Align2::RIGHT_CENTER,
                };
                if px >= 4.0 {
                    paint.text(p0, anchor, text, egui::FontId::proportional(px * 1.3), ink);
                } else {
                    // too small to read: a grey bar where it sits
                    let wd = (size * 0.55 * text.chars().count().max(1) as f64 * k) as f32;
                    let x0 = match align {
                        TextAlign::Start => p0.x,
                        TextAlign::Middle => p0.x - wd / 2.0,
                        TextAlign::End => p0.x - wd,
                    };
                    let h = px.max(1.0);
                    paint.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(x0, p0.y - h / 2.0),
                            egui::vec2(wd, h),
                        ),
                        0.0,
                        egui::Color32::from_gray(150),
                    );
                }
            }
            TitleItem::Image { lo, hi, .. } | TitleItem::Logo { lo, hi, .. } => {
                let b = egui::Rect::from_two_pos(to(at + *lo), to(at + *hi));
                paint.rect_stroke(
                    b,
                    0.0,
                    egui::Stroke::new(0.8, egui::Color32::from_gray(150)),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }
}
