//! Drafting settings in the app: the Annotation styles manager, the
//! pickers for the styles new dimensions, notes and tags get, the
//! Dimension and Align tools' options, the guide line settings, and the
//! Sheet templates window.
//!
//! Styles and templates are elements, so every change here is an ordinary
//! transaction: it undoes, saves, and shows at once on everything that uses
//! the style or template.

use super::*;
use crate::ui_model::Item;
use creabim_core::titleblock::{Corner, SheetTemplate, TitleItem, BUILT_IN_FIELDS};
use creabim_core::{DimAxis, StyleUse, TextStyle};
use creabim_geom::plan::PlanItem;
use serde_json::{json, Value};

/// The open drafting windows and what they show.
#[derive(Default)]
pub(super) struct DraftingUi {
    pub styles_open: bool,
    pub templates_open: bool,
    /// The style the manager shows.
    style: Option<ElementId>,
    /// The kind of style the manager lists (Any lists them all).
    filter: StyleUse,
    /// The template the templates window shows.
    template: Option<ElementId>,
    /// A path typed for importing a title block or a project's templates.
    pub(super) path: String,
}

/// The guide line angle steps offered, degrees (0 is none).
const STEPS: [f64; 4] = [0.0, 15.0, 30.0, 45.0];

impl CreaBimApp {
    /// The tool options strip entries for the drafting tools.
    pub(super) fn drafting_options(&self) -> Vec<Value> {
        let seg = |items: Vec<Value>| json!({"kind": "seg", "items": items});
        let opt = |id: String, label: &str, on: bool, hint: &str| json!({"id": id, "label": label, "on": on, "hint": hint});
        let tool = self.plan.tool;
        let mut out = vec![];
        if tool == Tool::Dimension {
            let now = self.plan.dim_axis;
            let mut items = vec![opt(
                "dim.axis:0".into(),
                "Auto",
                now.is_none(),
                "Horizontal, vertical or aligned from where you drag the line; hold Shift to keep the one shown",
            )];
            for (i, a) in [DimAxis::Aligned, DimAxis::Horizontal, DimAxis::Vertical]
                .into_iter()
                .enumerate()
            {
                items.push(opt(
                    format!("dim.axis:{}", i + 1),
                    a.label(),
                    now == Some(a),
                    match a {
                        DimAxis::Aligned => "Along the two points: their true distance",
                        DimAxis::Horizontal => "Across the drawing: the distance in X only",
                        DimAxis::Vertical => "Up the drawing: the distance in Y only",
                    },
                ));
            }
            out.push(seg(items));
            // continuous, and the spacing rows land on (MU23 board)
            out.push(seg(vec![opt(
                "dim.chain".into(),
                "Continuous",
                self.plan.dim_chain,
                "After the first dimension, each click adds the next on the same line; Esc or right-click ends it",
            )]));
            // spacing the user types, not presets (MU25): off, lines go
            // where clicked
            out.push(seg(vec![opt(
                "dim.step".into(),
                "Even spacing",
                self.plan.dim_step_mm > 0.0,
                "Dimension lines land on steps of the spacing (on paper) from what they measure, so rows stay even. Off: where you click. They snap onto other dimensions' lines either way",
            )]));
            out.push(serde_json::json!({
                "kind": "number", "key": "dim.spacing", "label": "Spacing",
                "value": self.plan.dim_step_size, "unit": "mm", "step": 0.5, "min": 0.5, "max": 100.0,
                "hint": "The step between rows of dimensions, millimetres on paper. Typing one turns Even spacing on",
            }));
        }
        if tool == Tool::Tag {
            out.push(seg(vec![
                opt(
                    "tag.reads:0".into(),
                    "Mark",
                    !self.plan.anno.tag_material,
                    "The tag reads the element's mark, or its size",
                ),
                opt(
                    "tag.reads:1".into(),
                    "Material",
                    self.plan.anno.tag_material,
                    "A material tag: the element's material, or its layers one line each, outside to inside. It follows the model",
                ),
            ]));
        }
        let style_menu = match tool {
            Tool::Dimension
            | Tool::Angle
            | Tool::Radius
            | Tool::Diameter
            | Tool::ArcLength
            | Tool::SpotLevel => Some(("dimstyle", self.plan.anno.dim_style)),
            Tool::Note => Some(("notestyle", self.plan.anno.note_style)),
            Tool::Tag => Some(("tagstyle", self.plan.anno.tag_style)),
            _ => None,
        };
        if let Some((key, id)) = style_menu {
            out.push(json!({
                "kind": "menu", "key": key, "label": "Style", "value": self.style_name(id),
                "icon": "anno-styles",
                "hint": "The annotation style new ones get: text, ends, tag shape and colour",
            }));
        }
        if tool == Tool::Align {
            out.push(seg(crate::aligntool::AlignMode::ALL
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    opt(
                        format!("align.mode:{i}"),
                        m.label(),
                        self.plan.align.mode == *m,
                        m.hint(),
                    )
                })
                .collect()));
        }
        if tool != Tool::Select {
            out.push(json!({
                "kind": "menu", "key": "guides", "label": "",
                "value": if self.plan.snap.references { "Guides" } else { "Guides off" },
                "icon": "snap-guides",
                "hint": "Temporary guide lines: rest on a point or edge (or press Q) to get its extension, parallels, perpendiculars and angles. Esc clears them, K keeps the one under the pointer.",
            }));
        }
        out
    }

    fn style_name(&self, id: Option<ElementId>) -> String {
        id.and_then(|id| self.doc.model().get(id))
            .map_or("Default".into(), |e| e.name.clone())
    }

    /// Items of this module's menus, by key.
    pub(super) fn drafting_menu(&self, key: &str) -> Option<Vec<Item>> {
        let style_items = |purpose: StyleUse, now: Option<ElementId>| {
            let mut items =
                vec![Item::new(format!("{key}:none"), "Default").checked(now.is_none())];
            for (id, name) in crate::annotate::styles_for(self.doc.model(), purpose) {
                items.push(Item::new(format!("{key}:{}", id.0), name).checked(now == Some(id)));
            }
            items.push(Item::Separator);
            items.push(
                Item::new("styles", "Annotation styles…")
                    .icon("anno-styles")
                    .hint("Make, change and delete styles"),
            );
            items
        };
        Some(match key {
            "dimstyle" => style_items(StyleUse::Dimensions, self.plan.anno.dim_style),
            "notestyle" => style_items(StyleUse::Notes, self.plan.anno.note_style),
            "tagstyle" => style_items(StyleUse::Tags, self.plan.anno.tag_style),
            "guides" => {
                let mut items = vec![
                    Item::new("guides.refs", "Guide lines from rested points and edges")
                        .checked(self.plan.snap.references)
                        .icon("snap-guides"),
                    Item::Header("Angle steps".into()),
                ];
                for (i, s) in STEPS.iter().enumerate() {
                    let label = if *s == 0.0 {
                        "Level and plumb only".to_string()
                    } else {
                        format!("Every {s}°")
                    };
                    items.push(
                        Item::new(format!("guides.step:{i}"), label)
                            .checked((self.plan.guide_step - s).abs() < 0.1),
                    );
                }
                items.push(Item::Separator);
                items.push(
                    Item::new("guides.clear", "Clear guide lines")
                        .key("Esc")
                        .hint("Forget every rested point and edge"),
                );
                items
            }
            _ => return None,
        })
    }

    /// Run one of this module's actions; false when it is not one.
    pub(super) fn drafting_action(&mut self, head: &str, arg: &str) -> bool {
        let index = arg.parse::<usize>().ok();
        let id = self
            .doc
            .model()
            .iter()
            .find(|e| e.id.0.to_string() == arg)
            .map(|e| e.id);
        match head {
            "dim.axis" => {
                self.plan.dim_axis = match index {
                    Some(1) => Some(DimAxis::Aligned),
                    Some(2) => Some(DimAxis::Horizontal),
                    Some(3) => Some(DimAxis::Vertical),
                    _ => None,
                };
            }
            "tag.reads" => self.plan.anno.tag_material = index == Some(1),
            "dim.chain" => {
                self.plan.dim_chain = !self.plan.dim_chain;
                self.plan.chain_line = None;
            }
            "dim.step" => {
                self.plan.dim_step_mm = if self.plan.dim_step_mm > 0.0 {
                    0.0
                } else {
                    self.plan.dim_step_size
                };
            }
            "grid.shape" => {
                self.plan.grid_arc = index == Some(1);
                self.plan.grid_arc_ends = None;
                self.status = if self.plan.grid_arc {
                    "Grid arc: click its two ends, then a point it passes through."
                } else {
                    "Grid line: click its two ends."
                }
                .into();
            }
            "align.mode" => {
                if let Some(m) = index.and_then(|i| crate::aligntool::AlignMode::ALL.get(i)) {
                    self.plan.align.mode = *m;
                    self.status = m.hint().into();
                }
            }
            "dimstyle" => self.plan.anno.dim_style = id,
            "notestyle" => self.plan.anno.note_style = id,
            "tagstyle" => self.plan.anno.tag_style = id,
            "guides.refs" => self.plan.snap.references = !self.plan.snap.references,
            "guides.step" => {
                if let Some(s) = index.and_then(|i| STEPS.get(i)) {
                    self.plan.guide_step = *s;
                }
            }
            "guides.clear" => {
                let n = self.plan.clear_references();
                self.status = format!("Cleared {n} guide references.");
            }
            "styles" => self.open_styles(None),
            // the Masters view, which took over from the old window (MU40;
            // owner 2026-11-18: the window had no design pass, ran off screen)
            "templates" => self.open_master(None),
            "align" => self.use_align_tool(),
            "styles.select" => {
                if let Some(id) = id {
                    self.select_style_users(id);
                }
            }
            _ => return false,
        }
        true
    }

    /// Align in Corner mode (J): trim or extend two to a corner.
    pub(super) fn use_corner_tool(&mut self) {
        self.plan.tool = Tool::Align;
        self.plan.cancel();
        self.plan.align.mode = crate::aligntool::AlignMode::Corner;
        self.plan.align.corner = None;
        self.status = crate::aligntool::hint(&self.plan.align).into();
    }

    /// Switch to the Align tool.
    pub(super) fn use_align_tool(&mut self) {
        self.plan.tool = Tool::Align;
        self.plan.cancel();
        self.status = Tool::Align.hint().into();
    }

    pub(super) fn open_styles(&mut self, style: Option<ElementId>) {
        self.drafting.styles_open = true;
        if style.is_some() {
            self.drafting.style = style;
        }
    }

    /// Select every dimension, tag and note using a style.
    pub(super) fn select_style_users(&mut self, id: ElementId) {
        let ids = crate::annotate::style_user_ids(self.doc.model(), id);
        self.selection.clear();
        self.selection.extend(ids.iter().copied(), true);
        self.status = format!("Selected the {} annotations using this style.", ids.len());
    }

    // ---- the Annotation styles window ----------------------------------

    pub(super) fn styles_window(&mut self, ctx: &egui::Context) {
        if !self.drafting.styles_open {
            return;
        }
        let mut open = true;
        let area = self.mat.area.unwrap_or_else(|| ctx.content_rect());
        // fits the window, its contents scrolling (MU85: taller than a
        // laptop's screen, its last rows out of reach)
        crate::app::shell::window("Annotation styles")
            .open(&mut open)
            .default_pos(area.min + Vec2::new(40.0, 70.0))
            .constrain_to(area)
            .min_width(620.0)
            .max_height((area.height() - 40.0).max(240.0))
            .vscroll(true)
            .collapsible(false)
            .show(ctx, |ui| self.styles_contents(ui));
        self.drafting.styles_open = open;
    }

    fn styles_contents(&mut self, ui: &mut egui::Ui) {
        let model = self.doc.model();
        let list: Vec<(ElementId, String, StyleUse, usize)> = model
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::TextStyle(s) => Some((
                    e.id,
                    e.name.clone(),
                    s.purpose,
                    crate::annotate::style_users(model, e.id),
                )),
                _ => None,
            })
            .filter(|(_, _, p, _)| {
                self.drafting.filter.is_any() || *p == self.drafting.filter || p.is_any()
            })
            .collect();
        if self
            .drafting
            .style
            .is_none_or(|s| !list.iter().any(|l| l.0 == s))
        {
            self.drafting.style = list.first().map(|l| l.0);
        }
        crate::props::flow(ui, |ui| {
            crate::app::helpui::combo(ui, "Show")
                .selected_text(self.drafting.filter.label())
                .show_ui(ui, |ui| {
                    for k in StyleUse::ALL {
                        ui.selectable_value(&mut self.drafting.filter, k, k.label());
                    }
                });
            ui.separator();
            if ui
                .button("New")
                .on_hover_text("A new style for what the list shows")
                .clicked()
            {
                let s = TextStyle {
                    purpose: self.drafting.filter,
                    ..TextStyle::default()
                };
                self.add_style(new_style_name(self.doc.model(), "Style"), s);
            }
            let current = self
                .drafting
                .style
                .and_then(|id| self.doc.model().get(id).cloned());
            // the starter styles, for a project from before they shipped
            let missing: Vec<_> = creabim_core::annotation::starter_text_styles()
                .into_iter()
                .filter(|(n, _)| !self.doc.model().iter().any(|e| e.name == *n))
                .collect();
            if !missing.is_empty()
                && ui
                    .button("Starter styles")
                    .on_hover_text(
                        "Add a style for dimensions, one for notes and one for tags, to start from",
                    )
                    .clicked()
            {
                let mut tx = Transaction::new("Starter styles");
                for (name, s) in missing {
                    tx = tx.insert(Element::new(name, ElementKind::TextStyle(s)));
                }
                let r = self.doc.commit(tx);
                self.report(r, "Starter styles added: duplicate one to make it yours.");
            }
            if let Some(e) = &current {
                if ui.button("Duplicate").clicked() {
                    if let ElementKind::TextStyle(s) = &e.kind {
                        let name = new_style_name(self.doc.model(), &e.name);
                        self.add_style(name, *s);
                    }
                }
                if ui
                    .button("Delete")
                    .on_hover_text("Its users go back to the default style")
                    .clicked()
                {
                    self.delete_style(e.id);
                }
            }
        });
        ui.separator();
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(190.0);
                egui::ScrollArea::vertical()
                    .id_salt("style_list")
                    .max_height(360.0)
                    .show(ui, |ui| {
                        if list.is_empty() {
                            ui.label(egui::RichText::new("No styles yet: press New.").weak());
                        }
                        for (id, name, purpose, users) in &list {
                            let text = format!("{name}  ({users})");
                            let r = ui
                                .selectable_label(self.drafting.style == Some(*id), text)
                                .on_hover_text(format!(
                                    "For {}; used by {users}",
                                    purpose.label().to_lowercase()
                                ));
                            if r.clicked() {
                                self.drafting.style = Some(*id);
                            }
                        }
                    });
            });
            ui.separator();
            ui.vertical(|ui| {
                if let Some(id) = self.drafting.style {
                    self.style_editor(ui, id);
                }
            });
        });
    }

    fn style_editor(&mut self, ui: &mut egui::Ui, id: ElementId) {
        let Some(e) = self.doc.model().get(id).cloned() else {
            return;
        };
        let ElementKind::TextStyle(mut s) = e.kind.clone() else {
            return;
        };
        let mut name = e.name.clone();
        // a Properties row like every other (MU85)
        let renamed = crate::props::row(ui, "Name", |ui| {
            let w = ui.available_width();
            crate::props::text_field(ui, egui::TextEdit::singleline(&mut name), w).changed()
        });
        if renamed && !name.trim().is_empty() {
            let r = self
                .doc
                .edit_merged(id, "Rename style", |e| e.name = name.clone());
            self.report(r, "Style renamed.");
        }
        if let Some(label) = crate::annotate::style_ui(ui, &mut s) {
            let r = self
                .doc
                .edit_merged(id, label, |e| e.kind = ElementKind::TextStyle(s));
            self.report(r, format!("{label}: every user shows it."));
        }
        // its font for every style and for what has none (MU85: a font
        // changed here left the tags in theirs)
        crate::props::row(ui, "", |ui| {
            if ui
                .add_sized(
                    [ui.available_width(), ui.spacing().interact_size.y],
                    egui::Button::new("Use this font in every style").truncate(),
                )
                .on_hover_text("Every annotation style takes this font, and so do dimensions, notes and tags with no style")
                .clicked()
            {
                self.font_everywhere(s.font);
            }
        });
        ui.add_space(4.0);
        let from = ui
            .ctx()
            .graphics(|g| g.get(ui.layer_id()).map(|l| l.next_idx()));
        style_preview(ui, &s);
        // as the views draw: on dark paper with the theme (MU85)
        if self.dark_paper(ui.ctx()) {
            super::darken_shapes(ui.ctx(), ui.layer_id(), from);
        }
        let users = crate::annotate::style_users(self.doc.model(), id);
        crate::props::flow(ui, |ui| {
            if ui
                .add_enabled(
                    users > 0,
                    egui::Button::new(format!("Select all {users} using it")),
                )
                .clicked()
            {
                self.select_style_users(id);
            }
            let picked: Vec<ElementId> = self
                .selection
                .all()
                .into_iter()
                .filter(|i| {
                    self.doc.model().get(*i).is_some_and(|e| {
                        matches!(
                            e.kind,
                            ElementKind::Dimension(_) | ElementKind::Tag(_) | ElementKind::Note(_)
                        )
                    })
                })
                .collect();
            if ui
                .add_enabled(
                    !picked.is_empty(),
                    egui::Button::new(format!("Apply to {} selected", picked.len())),
                )
                .on_hover_text("Give the selected dimensions, tags and notes this style")
                .clicked()
            {
                let model = self.doc.model();
                let mut tx = Transaction::new("Apply style");
                for i in &picked {
                    if let Some(before) = model.get(*i) {
                        let mut after = before.clone();
                        crate::annotate::set_style(&mut after.kind, id);
                        tx = tx.replace(before.clone(), after);
                    }
                }
                let r = self.doc.commit(tx);
                self.report(r, format!("{} now use {}.", picked.len(), e.name));
            }
        });
        crate::props::flow(ui, |ui| {
            ui.label("New ones get it:");
            let anno = &mut self.plan.anno;
            for (slot, text) in [
                (&mut anno.dim_style, "Dimensions"),
                (&mut anno.note_style, "Notes"),
                (&mut anno.tag_style, "Tags"),
            ] {
                let mut on = *slot == Some(id);
                if crate::props::tickbox(ui, &mut on, text).changed() {
                    *slot = on.then_some(id);
                }
            }
        });
    }

    /// Font `f` for every text style, and for annotations with none.
    fn font_everywhere(&mut self, f: creabim_core::Font) {
        let model = self.doc.model();
        let mut tx = Transaction::new("Font in every style");
        for e in model.iter() {
            if let ElementKind::TextStyle(s) = &e.kind {
                if s.font != f {
                    let mut after = e.clone();
                    after.kind = ElementKind::TextStyle(TextStyle { font: f, ..*s });
                    tx = tx.replace(e.clone(), after);
                }
            }
        }
        let mut pack = creabim_core::styles::active_pack(model).into_owned();
        pack.text_font = (f != creabim_core::Font::Sans).then_some(f);
        let r = self.doc.commit(tx).and_then(|_| {
            let t = creabim_core::styles::apply_pack(self.doc.model(), &pack);
            self.doc.commit(t)
        });
        self.report(r, format!("{} in every style.", f.label()));
    }

    fn add_style(&mut self, name: String, s: TextStyle) {
        let e = Element::new(name, ElementKind::TextStyle(s));
        let id = e.id;
        let r = self.doc.commit(Transaction::new("New style").insert(e));
        self.drafting.style = Some(id);
        self.report(r, "New style: set its text, ends and tag shape here.");
    }

    fn delete_style(&mut self, id: ElementId) {
        let model = self.doc.model();
        let Some(e) = model.get(id) else { return };
        let mut tx = Transaction::new("Delete style");
        // its users go back to the default look, in the same undo step
        for u in crate::annotate::style_user_ids(model, id) {
            if let Some(before) = model.get(u) {
                let mut after = before.clone();
                match &mut after.kind {
                    ElementKind::Dimension(d) => d.style = None,
                    ElementKind::Tag(t) => t.style = None,
                    ElementKind::Note(n) => n.style = None,
                    _ => {}
                }
                tx = tx.replace(before.clone(), after);
            }
        }
        let tx = tx.remove_cascade(model, e.id);
        let r = self.doc.commit(tx);
        for slot in [
            &mut self.plan.anno.dim_style,
            &mut self.plan.anno.note_style,
            &mut self.plan.anno.tag_style,
        ] {
            if *slot == Some(id) {
                *slot = None;
            }
        }
        self.drafting.style = None;
        self.report(r, "Style deleted; what used it has the default look.");
    }

    // ---- the Sheet templates window ------------------------------------

    pub(super) fn templates_window(&mut self, ctx: &egui::Context) {
        if !self.drafting.templates_open {
            return;
        }
        let mut open = true;
        let area = self.mat.area.unwrap_or_else(|| ctx.content_rect());
        crate::app::shell::window("Sheet templates")
            .open(&mut open)
            .default_pos(area.min + Vec2::new(60.0, 80.0))
            .constrain_to(area)
            .min_width(640.0)
            .collapsible(false)
            .show(ctx, |ui| self.templates_contents(ui));
        self.drafting.templates_open = open;
    }

    fn templates_contents(&mut self, ui: &mut egui::Ui) {
        let list = templates(self.doc.model());
        if self
            .drafting
            .template
            .is_none_or(|t| !list.iter().any(|l| l.0 == t))
        {
            self.drafting.template = list.first().map(|l| l.0);
        }
        crate::props::info(ui, "A template is a title block every sheet using it draws, with {fields} the sheet fills in.", "");
        crate::props::flow(ui, |ui| {
            if ui
                .button("Copy of the built-in block")
                .on_hover_text("Start from the usual title block and change it")
                .clicked()
            {
                let name = new_style_name(self.doc.model(), "Title block");
                self.add_template(name, sheets::builtin_template());
            }
            if ui
                .button("Import DXF or SVG…")
                .on_hover_text(
                    "A title block drawn elsewhere. Text such as {client} becomes a field.",
                )
                .clicked()
            {
                if let Some(p) = rfd::FileDialog::new()
                    .set_title("Import a title block")
                    .add_filter("Drawings", &["dxf", "svg", "creabim"])
                    .pick_file()
                {
                    self.drafting.path = p.display().to_string();
                    self.import_templates();
                }
            }
            if let Some(id) = self.drafting.template {
                if ui.button("Duplicate").clicked() {
                    if let Some(e) = self.doc.model().get(id).cloned() {
                        if let ElementKind::SheetTemplate(t) = e.kind {
                            let name = new_style_name(self.doc.model(), &e.name);
                            self.add_template(name, t);
                        }
                    }
                }
                if ui
                    .button("Delete")
                    .on_hover_text("Its sheets go back to the built-in block")
                    .clicked()
                {
                    self.delete_template(id);
                }
            }
        });
        crate::props::flow(ui, |ui| {
            ui.label("From a path");
            crate::props::text_field(
                ui,
                egui::TextEdit::singleline(&mut self.drafting.path)
                    .hint_text(".dxf, .svg or another .creabim project"),
                360.0,
            );
            if ui
                .add_enabled(
                    !self.drafting.path.trim().is_empty(),
                    egui::Button::new("Import"),
                )
                .on_hover_text("A .creabim brings its sheet templates and annotation styles")
                .clicked()
            {
                self.import_templates();
            }
        });
        ui.separator();
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(170.0);
                if list.is_empty() {
                    ui.label(egui::RichText::new("No templates yet.").weak());
                }
                for (id, name) in &list {
                    let users = template_users(self.doc.model(), *id).len();
                    if ui
                        .selectable_label(
                            self.drafting.template == Some(*id),
                            format!("{name}  ({users})"),
                        )
                        .clicked()
                    {
                        self.drafting.template = Some(*id);
                    }
                }
            });
            ui.separator();
            ui.vertical(|ui| {
                if let Some(id) = self.drafting.template {
                    self.template_editor(ui, id);
                }
            });
        });
    }

    fn template_editor(&mut self, ui: &mut egui::Ui, id: ElementId) {
        let Some(e) = self.doc.model().get(id).cloned() else {
            return;
        };
        let ElementKind::SheetTemplate(mut t) = e.kind.clone() else {
            return;
        };
        let mut label = None;
        let mut name = e.name.clone();
        crate::props::flow(ui, |ui| {
            ui.label("Name");
            if ui.text_edit_singleline(&mut name).changed() && !name.trim().is_empty() {
                let r = self
                    .doc
                    .edit_merged(id, "Rename template", |e| e.name = name.clone());
                self.report(r, "Template renamed.");
            }
        });
        crate::app::helpui::combo(ui, "Sits in the")
            .selected_text(t.corner.label())
            .show_ui(ui, |ui| {
                for c in Corner::ALL {
                    if ui.selectable_value(&mut t.corner, c, c.label()).changed() {
                        label = Some("Title block corner");
                    }
                }
            });
        if crate::props::switch(ui, &mut t.border, "Draw the usual border too")
            .on_hover_text("Off for a template that brings its own frame")
            .changed()
        {
            label = Some("Template border");
        }
        template_preview(ui, &t);
        crate::props::section(ui, "Its own fields");
        let mut remove = None;
        for (i, (k, v)) in t.fields.iter_mut().enumerate() {
            crate::props::flow(ui, |ui| {
                crate::props::text_field(ui, egui::TextEdit::singleline(k), 110.0);
                if crate::props::text_field(ui, egui::TextEdit::singleline(v), 200.0).changed() {
                    label = Some("Field value");
                }
                if ui.small_button("Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            t.fields.remove(i);
            label = Some("Remove field");
        }
        if ui.button("Add field").clicked() {
            t.fields
                .push((format!("field{}", t.fields.len() + 1), String::new()));
            label = Some("Add field");
        }
        ui.label(
            egui::RichText::new(format!(
                "Every sheet also fills in {}.",
                BUILT_IN_FIELDS
                    .iter()
                    .map(|(k, _)| format!("{{{k}}}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
            .weak(),
        );
        // the same 200 ms as every fold (AMD-017)
        ui.style_mut().animation_time = 0.2;
        egui::CollapsingHeader::new("Text")
            .id_salt(("template_text", id))
            .show(ui, |ui| {
                for item in t.items.iter_mut() {
                    if let TitleItem::Text { text, size, .. } = item {
                        crate::props::flow(ui, |ui| {
                            if crate::props::text_field(ui, egui::TextEdit::singleline(text), 260.0)
                                .changed()
                            {
                                label = Some("Template text");
                            }
                            if ui
                                .add(crate::units::paper_small(size).range(0.5..=40.0))
                                .changed()
                            {
                                label = Some("Template text size");
                            }
                        });
                    }
                }
            });
        if let Some(l) = label {
            if let Err(why) = t.validate() {
                self.status = why;
            } else {
                let r = self
                    .doc
                    .edit_merged(id, l, |e| e.kind = ElementKind::SheetTemplate(t.clone()));
                self.report(r, format!("{l}: its sheets show it."));
            }
        }
        ui.separator();
        crate::props::flow(ui, |ui| {
            let sheet = self.active_view.filter(|v| self.is_sheet(*v));
            if ui
                .add_enabled(sheet.is_some(), egui::Button::new("Use on this sheet"))
                .clicked()
            {
                if let Some(s) = sheet {
                    self.set_sheet_template(&[s], Some(id));
                }
            }
            if ui.button("Use on every sheet").clicked() {
                let all: Vec<ElementId> = sheets::sheets(self.doc.model())
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect();
                self.set_sheet_template(&all, Some(id));
            }
            if ui.button("New sheet from it").clicked() {
                self.new_sheet_from(id);
            }
        });
    }

    pub(super) fn add_template(&mut self, name: String, t: SheetTemplate) {
        let e = Element::new(name, ElementKind::SheetTemplate(t));
        let id = e.id;
        let r = self
            .doc
            .commit(Transaction::new("New sheet template").insert(e));
        self.drafting.template = Some(id);
        self.report(r, "New sheet template. Use it on a sheet below.");
    }

    pub(super) fn delete_template(&mut self, id: ElementId) {
        let users = template_users(self.doc.model(), id);
        let mut tx = sheet_template_tx(self.doc.model(), &users, None, "Delete sheet template");
        tx = tx.remove_cascade(self.doc.model(), id);
        let r = self.doc.commit(tx);
        self.drafting.template = None;
        self.report(r, "Template deleted; its sheets have the built-in block.");
    }

    pub(super) fn set_sheet_template(&mut self, sheets: &[ElementId], t: Option<ElementId>) {
        let tx = sheet_template_tx(self.doc.model(), sheets, t, "Sheet template");
        let n = tx.ops.len();
        let r = self.doc.commit(tx);
        self.report(r, format!("{n} sheet(s) use the template."));
    }

    pub(super) fn new_sheet_from(&mut self, template: ElementId) {
        let Some(ElementKind::SheetTemplate(t)) =
            self.doc.model().get(template).map(|e| e.kind.clone())
        else {
            return;
        };
        let used: Vec<String> = sheets::sheets(self.doc.model())
            .into_iter()
            .map(|(_, s)| s.number.clone())
            .collect();
        let number = (301..)
            .map(|n| format!("A{n}"))
            .find(|n| !used.contains(n))
            .expect("unbounded");
        let std = creabim_core::standards::of(self.doc.model());
        let paper = std
            .sheet(&std.default_sheet)
            .cloned()
            .unwrap_or_else(|| creabim_core::standards::iso().sheets[3].clone());
        let mut s = Sheet::sized(number, &paper);
        if let Some(p) = t.paper {
            (s.width, s.height) = (p.x, p.y);
        }
        s.template = Some(template);
        let e = Element::new("New Sheet", ElementKind::Sheet(s));
        let id = e.id;
        let r = self.doc.commit(Transaction::new("New sheet").insert(e));
        self.active_view = Some(id);
        self.selection.only(id);
        self.report(r, "New sheet with the template's title block.");
    }

    /// Import from the typed path: a title block from a DXF or SVG, or the
    /// templates and annotation styles of another project.
    pub(super) fn import_templates(&mut self) {
        let path = self.drafting.path.trim().to_string();
        let lower = path.to_lowercase();
        if lower.ends_with(".creabim") || lower.ends_with(".json") {
            match creabim_store::load(&path) {
                Ok(other) => {
                    let mut tx = Transaction::new("Import templates and styles");
                    let mut n = 0;
                    for e in other.model().iter() {
                        if !matches!(
                            e.kind,
                            ElementKind::SheetTemplate(_) | ElementKind::TextStyle(_)
                        ) {
                            continue;
                        }
                        let taken = self
                            .doc
                            .model()
                            .iter()
                            .any(|m| m.name == e.name && m.kind == e.kind);
                        if !taken {
                            tx = tx.insert(Element::new(e.name.clone(), e.kind.clone()));
                            n += 1;
                        }
                    }
                    let r = self.doc.commit(tx);
                    self.report(r, format!("Brought in {n} sheet templates and styles."));
                }
                Err(e) => self.status = format!("Couldn't read {path}: {e}"),
            }
            return;
        }
        match creabim_exchange::titleblock::read(&path) {
            Ok(t) => {
                let stem = std::path::Path::new(&path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Title block")
                    .to_string();
                let name = new_style_name(self.doc.model(), &stem);
                self.add_template(name, t);
            }
            Err(e) => self.status = format!("Couldn't import {path}: {e}"),
        }
    }

    /// A sheet's template picker and its fields, in the sheet's properties.
    pub(super) fn sheet_template_ui(
        &mut self,
        ui: &mut egui::Ui,
        sh: &mut Sheet,
    ) -> Option<&'static str> {
        let mut label = None;
        let list = templates(self.doc.model());
        let now = sh
            .template
            .and_then(|t| list.iter().find(|l| l.0 == t))
            .map_or("Built-in".to_string(), |l| l.1.clone());
        crate::props::row(ui, "Master", |ui| {
            egui::ComboBox::from_id_salt("sheet.master")
                .icon(crate::props::combo_icon)
                .width(ui.available_width())
                .selected_text(now)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(&mut sh.template, None, "Built-in")
                        .changed()
                    {
                        label = Some("Sheet template");
                    }
                    for (id, name) in &list {
                        if ui
                            .selectable_value(&mut sh.template, Some(*id), name)
                            .changed()
                        {
                            label = Some("Sheet template");
                        }
                    }
                })
        })
        .response
        .on_hover_text("The master (title block and frame) this sheet is drawn on");
        // the paper it gets from its master, said (pens stage 7)
        let paper_says = match sheets::template_of(self.doc.model(), sh).map(|t| t.stock) {
            Some(st) if !st.is_plain() => {
                use creabim_core::titleblock::{PaperColour, PaperTexture};
                let colour = match st.colour {
                    PaperColour::White => "White".to_string(),
                    // short: the row is narrow (screen check: cut with …)
                    PaperColour::Own(_) => "Own".to_string(),
                    PaperColour::Palette => {
                        let p = creabim_core::styles::project_palette(self.doc.model());
                        if p.is_ink() {
                            "White".to_string()
                        } else {
                            p.name.clone()
                        }
                    }
                };
                if st.texture == PaperTexture::None {
                    colour
                } else {
                    format!("{colour}, {}", st.texture.label())
                }
            }
            _ => "White".to_string(),
        };
        crate::props::readout(
            ui,
            "Paper",
            &paper_says,
            "From its master: its colour and texture. Edit the master to change it",
        );
        // its master in the Masters view, to change (MU40: a view, not a
        // window)
        // with the masters' icon, so it reads as a button among fields
        // (owner, as Edit in Notes)
        if crate::props::action_icon(
            ui,
            "",
            "sheet-template",
            "Edit the master",
            "Open its master in a Masters view: what you change shows on every sheet on it",
            true,
        )
        .clicked()
        {
            self.open_master(sh.template);
        }
        let fields: Vec<(String, String)> = sheets::template_of(self.doc.model(), sh)
            .map(|t| {
                let mut f = t.fields.clone();
                for k in t.used_fields() {
                    let builtin = BUILT_IN_FIELDS.iter().any(|(b, _)| *b == k);
                    if !builtin && !f.iter().any(|(n, _)| *n == k) {
                        f.push((k, String::new()));
                    }
                }
                f
            })
            .unwrap_or_default();
        // what the project fills in (its Project tab): over a master's
        // default, under the sheet's own value
        let project: Vec<(&'static str, String)> = self
            .doc
            .model()
            .project_info()
            .map(|(_, p)| p.fields())
            .unwrap_or_default();
        for (k, default) in fields {
            let from_project = project
                .iter()
                .find(|(p, _)| *p == k)
                .map(|(_, v)| v.clone());
            let mut v = sh.fields.get(&k).cloned().unwrap_or_default();
            let mut word = k.replace('_', " ");
            if let Some(f) = word.get_mut(0..1) {
                f.make_ascii_uppercase();
            }
            if crate::props::row(ui, &word, |ui| {
                crate::props::fill(
                    ui,
                    egui::TextEdit::singleline(&mut v)
                        .vertical_align(egui::Align::Center)
                        .hint_text(match from_project {
                            Some(p) => p,
                            None if default.is_empty() => "From the master".to_string(),
                            None => default,
                        }),
                )
                .on_hover_text("Empty, the sheet shows the project's value, else its master's. Type to give this sheet its own")
            })
            .changed()
            {
                {
                    if v.is_empty() {
                        sh.fields.remove(&k);
                    } else {
                        sh.fields.insert(k.clone(), v);
                    }
                    label = Some("Title block field");
                }
            }
        }
        label
    }
}

/// The sheet templates in the model, by name.
pub(super) fn templates(model: &creabim_core::Model) -> Vec<(ElementId, String)> {
    let mut v: Vec<(ElementId, String)> = model
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::SheetTemplate(_)))
        .map(|e| (e.id, e.name.clone()))
        .collect();
    v.sort_by(|a, b| a.1.cmp(&b.1));
    v
}

/// The sheets using a template.
pub(super) fn template_users(model: &creabim_core::Model, id: ElementId) -> Vec<ElementId> {
    sheets::sheets(model)
        .into_iter()
        .filter(|(_, s)| s.template == Some(id))
        .map(|(i, _)| i)
        .collect()
}

/// Point `sheets` at template `t` (none: the built-in block).
fn sheet_template_tx(
    model: &creabim_core::Model,
    sheets: &[ElementId],
    t: Option<ElementId>,
    label: &str,
) -> Transaction {
    let mut tx = Transaction::new(label);
    for id in sheets {
        let Some(before) = model.get(*id) else {
            continue;
        };
        let ElementKind::Sheet(s) = &before.kind else {
            continue;
        };
        if s.template == t {
            continue;
        }
        let mut after = before.clone();
        if let ElementKind::Sheet(s) = &mut after.kind {
            s.template = t;
        }
        tx = tx.replace(before.clone(), after);
    }
    tx
}

/// `base`, or `base 2`, `base 3`… whichever no element has.
pub(super) fn new_style_name(model: &creabim_core::Model, base: &str) -> String {
    let base = base
        .trim_end_matches(|c: char| c.is_ascii_digit())
        .trim_end();
    let taken = |n: &str| model.iter().any(|e| e.name == n);
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|i| format!("{base} {i}"))
        .find(|n| !taken(n))
        .expect("unbounded")
}

/// A dimension and a tag drawn in a style, on white paper.
fn style_preview(ui: &mut egui::Ui, s: &TextStyle) {
    // as wide as the fields above it (MU85)
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 90.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::WHITE);
    // paper mm, four pixels to the mm
    let xf = planview::Xf {
        rect,
        center: DVec2::new(45.0, 10.0),
        scale: 4.0,
    };
    let look: creabim_geom::annotation::TextLook = (*s).into();
    let items = [
        PlanItem::Dimension {
            id: ElementId::new(),
            a: DVec2::new(4.0, 4.0),
            b: DVec2::new(44.0, 4.0),
            offset: 6.0,
            text: "4000".into(),
            look,
            witness: None,
            breaks: vec![],
            label: Default::default(),
        },
        PlanItem::Tag {
            id: ElementId::new(),
            anchor: DVec2::new(62.0, 2.0),
            at: DVec2::new(74.0, 12.0),
            elbow: None,
            text: "D01".into(),
            round: false,
            look,
        },
    ];
    for item in &items {
        planview::draw_item(&painter, &xf, item, false, 1.0);
    }
}

/// A template's title block in the corner of an outline of A3 paper.
fn template_preview(ui: &mut egui::Ui, t: &SheetTemplate) {
    let paper = t.paper.unwrap_or(DVec2::new(420.0, 297.0));
    let k = 300.0 / paper.x as f32;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(300.0, paper.y as f32 * k), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, Color32::WHITE);
    let to = |p: DVec2| rect.left_bottom() + Vec2::new(p.x as f32 * k, -(p.y as f32) * k);
    let ink = Color32::BLACK;
    if t.border {
        let m = 10.0;
        let pts = [
            DVec2::new(m, m),
            DVec2::new(paper.x - m, m),
            DVec2::new(paper.x - m, paper.y - m),
            DVec2::new(m, paper.y - m),
            DVec2::new(m, m),
        ];
        painter.add(egui::Shape::line(
            pts.iter().map(|p| to(*p)).collect(),
            egui::Stroke::new(1.0, ink),
        ));
    }
    let o = t.corner.at(paper.x, paper.y);
    for item in &t.items {
        match item {
            TitleItem::Shape { points, closed, .. } => {
                let mut pts: Vec<egui::Pos2> = points.iter().map(|p| to(o + *p)).collect();
                if *closed && !pts.is_empty() {
                    pts.push(pts[0]);
                }
                painter.add(egui::Shape::line(pts, egui::Stroke::new(0.8, ink)));
            }
            TitleItem::Image { lo, hi, .. } | TitleItem::Logo { lo, hi, .. } => {
                painter.rect_stroke(
                    egui::Rect::from_two_pos(to(o + *lo), to(o + *hi)),
                    0.0,
                    egui::Stroke::new(0.8, ink),
                    egui::StrokeKind::Middle,
                );
            }
            TitleItem::Line { points, .. } => {
                painter.add(egui::Shape::line(
                    points.iter().map(|p| to(o + *p)).collect(),
                    egui::Stroke::new(0.8, ink),
                ));
            }
            TitleItem::Fill { points, gray } => {
                let g = (gray.clamp(0.0, 1.0) * 255.0) as u8;
                painter.add(egui::Shape::convex_polygon(
                    points.iter().map(|p| to(o + *p)).collect(),
                    Color32::from_gray(g),
                    egui::Stroke::NONE,
                ));
            }
            TitleItem::Text {
                at,
                size,
                text,
                align,
                ..
            } => {
                let anchor = match align {
                    creabim_core::titleblock::TextAlign::Start => egui::Align2::LEFT_CENTER,
                    creabim_core::titleblock::TextAlign::Middle => egui::Align2::CENTER_CENTER,
                    creabim_core::titleblock::TextAlign::End => egui::Align2::RIGHT_CENTER,
                };
                painter.text(
                    to(o + *at),
                    anchor,
                    text,
                    egui::FontId::proportional((*size as f32 * k * 1.2).max(5.0)),
                    ink,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::sample::sample_house;

    #[test]
    fn new_names_count_up_past_the_taken_ones() {
        let mut doc = Document::from_model(sample_house());
        let style = |name: &str| Element::new(name, ElementKind::TextStyle(TextStyle::default()));
        assert_eq!(new_style_name(doc.model(), "Style"), "Style");
        doc.commit(Transaction::new("s").insert(style("Style")))
            .unwrap();
        assert_eq!(new_style_name(doc.model(), "Style"), "Style 2");
        // duplicating "Style 2" makes "Style 3", not "Style 2 2"
        doc.commit(Transaction::new("s").insert(style("Style 2")))
            .unwrap();
        assert_eq!(new_style_name(doc.model(), "Style 2"), "Style 3");
    }

    #[test]
    fn a_template_goes_on_every_sheet_and_comes_off_in_one_undo() {
        let mut doc = Document::from_model(sample_house());
        let t = Element::new(
            "Practice block",
            ElementKind::SheetTemplate(sheets::builtin_template()),
        );
        let tid = t.id;
        doc.commit(Transaction::new("New sheet template").insert(t))
            .unwrap();
        let all: Vec<ElementId> = sheets::sheets(doc.model())
            .into_iter()
            .map(|(i, _)| i)
            .collect();
        assert!(!all.is_empty());
        let tx = sheet_template_tx(doc.model(), &all, Some(tid), "Sheet template");
        doc.commit(tx).unwrap();
        assert_eq!(template_users(doc.model(), tid).len(), all.len());
        // pointing them at it again changes nothing
        assert!(
            sheet_template_tx(doc.model(), &all, Some(tid), "Sheet template")
                .ops
                .is_empty()
        );
        assert!(doc.undo());
        assert!(template_users(doc.model(), tid).is_empty());
    }
}
