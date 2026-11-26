//! Create similar, Pick up and Apply settings in the app
//! (Mockup_Create_Similar_And_Pick_Up REV01, all decided 2026-11-21):
//! Shift+C starts the tool that made the selected element, set like it;
//! Ctrl+Shift+C picks up an element's settings and Ctrl+Shift+V applies
//! them, to the selection at once or to what is clicked or boxed next.
//! What is held shows as a chip in the status bar, its groups ticked in
//! the chip's menu. Each apply is one undo step.

use super::*;
use creabim_core::likeness::{self, Group};
use creabim_core::OpeningKind;

/// What the next click or box does, while a pointer of ours is out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LikePointer {
    /// Holds the settings of what is clicked.
    PickUp,
    /// Gives what is clicked or boxed the settings held.
    Apply,
}

/// The settings held and how they are used, kept for the session.
#[derive(Default)]
pub(super) struct Likeness {
    /// The element picked up, as it was then.
    pub held: Option<Element>,
    /// The groups ticked, remembered per kind of element.
    pub ticked: HashMap<std::mem::Discriminant<ElementKind>, Vec<Group>>,
    /// Our pointer, and the selection to give back when it is done.
    pub pointer: Option<(LikePointer, Vec<ElementId>)>,
    /// The tool's own settings, as they were before Create similar set
    /// its bar like an element: given back when it lets go.
    pub own: Option<OwnSettings>,
}

/// What Create similar changes in a tool's bar.
pub(super) struct OwnSettings {
    wall_thickness: f64,
    wall_location: creabim_core::LocationLine,
    members: crate::memberstool::MemberSettings,
    types: HashMap<creabim_core::TypeCategory, ElementId>,
    objects: crate::objecttool::ObjectSettings,
    detail: (creabim_core::detail::DetailKind, f64),
    /// Notes' and tags' styles, the tag's material, a dimension's style,
    /// and whether notes get a leader.
    anno: (
        Option<ElementId>,
        Option<ElementId>,
        bool,
        Option<ElementId>,
        bool,
    ),
}

/// The tool that draws elements like `e`, if there is one.
pub(super) fn tool_for(e: &Element) -> Option<Tool> {
    Some(match &e.kind {
        // a curtain wall is the Wall tool drawing its curtain type
        ElementKind::Wall(w) if w.curtain.is_none() || e.of_type.is_some() => Tool::Wall,
        ElementKind::Opening(o) if o.roof_at.is_none() => match o.kind {
            OpeningKind::Door => Tool::Door,
            OpeningKind::Window => Tool::Window,
            OpeningKind::Hole => Tool::Opening,
        },
        ElementKind::Slab(_) => Tool::Slab,
        ElementKind::Column(_) => Tool::Column,
        ElementKind::Beam(_) => Tool::Beam,
        ElementKind::Footing(_) => Tool::Footing,
        // furniture and fixtures: the Object tool, set to their type
        // (MU135: Shift+C on a washbasin said it wasn't there yet)
        ElementKind::Object(_) if e.of_type.is_some() => Tool::Object,
        ElementKind::Roof(_) => Tool::Roof,
        ElementKind::Stair(_) => Tool::Stair,
        ElementKind::Railing(_) => Tool::Railing,
        ElementKind::Room(_) => Tool::Room,
        ElementKind::Ceiling(_) => Tool::Ceiling,
        ElementKind::Detail(_) => Tool::Detail,
        // a note drawn as a symbol is placed with the Symbol tool's own pick
        ElementKind::Note(n) if n.symbol.is_none() => Tool::Note,
        ElementKind::Tag(_) => Tool::Tag,
        ElementKind::Dimension(d) => {
            use creabim_core::DimensionKind as K;
            match d.kind {
                K::Linear => Tool::Dimension,
                K::Angular => Tool::Angle,
                K::Radius => Tool::Radius,
                K::Diameter => Tool::Diameter,
                K::ArcLength => Tool::ArcLength,
                K::Spot => Tool::SpotLevel,
            }
        }
        _ => return None,
    })
}

/// How an element is named in short: its mark, else its name.
fn short_name(model: &creabim_core::Model, e: &Element) -> String {
    creabim_core::marks::mark_of(model, e.id).unwrap_or_else(|| e.name.clone())
}

impl CreaBimApp {
    /// The groups ticked for `e`'s kind: those it offers, phase off at first.
    pub(super) fn like_ticked(&self, e: &Element) -> Vec<Group> {
        let offered = likeness::groups(e);
        match self.like.ticked.get(&std::mem::discriminant(&e.kind)) {
            Some(on) => offered.into_iter().filter(|g| on.contains(g)).collect(),
            None => offered.into_iter().filter(|g| g.on_at_first()).collect(),
        }
    }

    /// Shift+C: the tool that made the selected element, every new one
    /// like it until the tool changes.
    pub(super) fn create_similar(&mut self, ctx: &egui::Context) {
        let Some(e) = self
            .selection
            .active
            .and_then(|id| self.doc.model().get(id))
            .cloned()
        else {
            self.status = "Create similar: select the element to make more like it first.".into();
            return;
        };
        let Some(tool) = tool_for(&e) else {
            self.status = format!("Create similar isn't there yet for {}.", e.name);
            return;
        };
        let name = short_name(self.doc.model(), &e);
        self.pick_tool(ctx, tool);
        // the tool's own settings, kept to give back (once: a second
        // Create similar in a row keeps the first's)
        if self.like.own.is_none() {
            let a = &self.plan.anno;
            self.like.own = Some(OwnSettings {
                wall_thickness: self.plan.wall_thickness,
                wall_location: self.plan.wall_location,
                members: self.plan.members.clone(),
                types: self.plan.types.clone(),
                objects: self.plan.objects.clone(),
                detail: (self.plan.details.kind, self.plan.details.width),
                anno: (
                    a.note_style,
                    a.tag_style,
                    a.tag_material,
                    a.dim_style,
                    a.leader,
                ),
            });
        }
        // the tool's bar says the same as what it will place
        match &e.kind {
            ElementKind::Wall(w) => {
                self.plan.wall_thickness = w.thickness;
                self.plan.wall_location = w.location;
            }
            ElementKind::Opening(o) => {
                let m = &mut self.plan.members;
                let size = [o.width, o.height, o.sill];
                match o.kind {
                    OpeningKind::Door => m.door = size,
                    OpeningKind::Window => m.window = size,
                    OpeningKind::Hole => m.hole = size,
                }
            }
            ElementKind::Object(o) => {
                // its own type as the catalogue entry placed, its turn and
                // height above the floor
                let t = e.of_type.as_ref().map(|t| t.id);
                let key = t
                    .and_then(|t| creabim_core::objects::type_of_id(self.doc.model(), t))
                    .and_then(|ty| ty.catalogue.clone())
                    .or_else(|| t.map(|t| format!("project/{t:?}")))
                    .unwrap_or_default();
                let entry =
                    t.and_then(|t| creabim_core::objects::from_type(self.doc.model(), t, &key));
                let s = &mut self.plan.objects;
                if entry.is_some() {
                    s.current = entry;
                    s.project_type = t;
                }
                s.rotation = o.rotation;
                s.offset = Some(o.offset);
            }
            ElementKind::Roof(r) => {
                let m = &mut self.plan.members;
                m.roof_shape = r.shape;
                m.roof_pitch = r.pitch;
            }
            ElementKind::Stair(st) => {
                let m = &mut self.plan.members;
                m.stair_width = st.width;
                m.stair_drawn = st.drawn;
            }
            ElementKind::Railing(r) => {
                let m = &mut self.plan.members;
                m.railing_height = r.height;
                m.railing_section = r.rail_profile.clone();
            }
            ElementKind::Detail(d) => {
                self.plan.details.kind = d.kind;
                self.plan.details.width = d.width;
            }
            ElementKind::Note(n) => {
                self.plan.anno.note_style = n.style;
                self.plan.anno.leader = n.leader.is_some();
            }
            ElementKind::Tag(t) => {
                self.plan.anno.tag_style = t.style;
                self.plan.anno.tag_material = t.material;
            }
            ElementKind::Dimension(d) => self.plan.anno.dim_style = d.style,
            _ => {}
        }
        if let Some(cat) = creabim_core::TypeCategory::of(&e.kind) {
            match &e.of_type {
                Some(t) => {
                    self.plan.types.insert(cat, t.id);
                }
                None => {
                    self.plan.types.remove(&cat);
                }
            }
        }
        self.plan.like = Some((e, tool));
        self.status = format!(
            "Create similar: the {} tool is set like {name}; everything you place with it is the same.",
            tool.label()
        );
    }

    /// Create similar lets go: the tool's own settings back in its bar.
    pub(super) fn like_own(&mut self) {
        self.plan.like = None;
        if let Some(o) = self.like.own.take() {
            self.plan.wall_thickness = o.wall_thickness;
            self.plan.wall_location = o.wall_location;
            // the drawing in hand (a stair's line, a railing's path) stays
            let mut m = o.members;
            m.stair_line = std::mem::take(&mut self.plan.members.stair_line);
            m.path = std::mem::take(&mut self.plan.members.path);
            m.last_beam = self.plan.members.last_beam;
            self.plan.members = m;
            self.plan.types = o.types;
            self.plan.objects = o.objects;
            (self.plan.details.kind, self.plan.details.width) = o.detail;
            let a = &mut self.plan.anno;
            (
                a.note_style,
                a.tag_style,
                a.tag_material,
                a.dim_style,
                a.leader,
            ) = o.anno;
        }
    }

    /// The option bar's chip while a tool is set like an element: lit, a
    /// click lets go.
    pub(super) fn like_chip(&self) -> Option<serde_json::Value> {
        let (e, tool) = self.plan.like.as_ref()?;
        (*tool == self.plan.tool).then(|| {
            serde_json::json!({
                "kind": "toggle", "id": "like.own", "on": true, "icon": "create-similar", "wide": true,
                "label": format!("Like {}", short_name(self.doc.model(), e)),
                "hint": "Everything this tool places is like it. Click to go back to the tool's own settings.",
            })
        })
    }

    /// Ctrl+Shift+C: hold the selected element's settings, or with
    /// nothing selected, the pointer picks what to take them from.
    pub(super) fn pick_up_settings(&mut self) {
        match self.selection.active {
            Some(id) if self.selection.len() == 1 => self.hold(id),
            _ => self.arm_like(LikePointer::PickUp),
        }
    }

    /// Ctrl+Shift+V: give the selection the settings held, or with nothing
    /// selected, the pointer gives them to what is clicked or boxed.
    pub(super) fn apply_settings(&mut self) {
        if self.like.held.is_none() {
            self.status = "Pick up settings first (Ctrl+Shift+C).".into();
            return;
        }
        if self.selection.is_empty() {
            self.arm_like(LikePointer::Apply);
        } else {
            let ids = self.selection.all();
            self.apply_held_to(&ids);
        }
    }

    fn hold(&mut self, id: ElementId) {
        let Some(e) = self.doc.model().get(id).cloned() else {
            return;
        };
        let name = short_name(self.doc.model(), &e);
        let n = likeness::groups(&e).len();
        self.like.held = Some(e);
        self.status = format!(
            "Holding the settings of {name} ({n} groups). Ctrl+Shift+V gives them to what you select or click; the chip in the status bar chooses which."
        );
    }

    fn arm_like(&mut self, p: LikePointer) {
        let prev = self.selection.all();
        self.plan.tool = Tool::Select;
        self.like_own();
        self.selection.clear();
        self.like.pointer = Some((p, prev));
        self.status = match p {
            LikePointer::PickUp => "Pick up: click the element to take the settings from. Esc stops.".into(),
            LikePointer::Apply => "Apply: click elements, or drag a box over them, to give them the settings held. Esc stops.".into(),
        };
    }

    /// Give `ids` the settings held, in one step.
    pub(super) fn apply_held_to(&mut self, ids: &[ElementId]) {
        let Some(src) = self.like.held.clone() else {
            return;
        };
        let on = self.like_ticked(&src);
        let (tx, n) = likeness::apply_to(self.doc.model(), &src, ids, &on);
        let name = short_name(self.doc.model(), &src);
        if n == 0 {
            self.status = format!(
                "Nothing to change: they are already like {name}, or take none of its settings."
            );
            return;
        }
        let r = self.doc.commit(tx);
        self.report(
            r,
            format!(
                "Gave {n} element{} the settings of {name}. Ctrl+Z to undo.",
                if n == 1 { "" } else { "s" }
            ),
        );
    }

    /// Each frame: Shift+C, our pointers taking what is clicked or boxed,
    /// and the preview beside the pointer.
    pub(super) fn likeness_frame(&mut self, ctx: &egui::Context) {
        if !ctx.egui_wants_keyboard_input()
            && self.plan.modal.is_none()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::C))
        {
            self.create_similar(ctx);
        }
        // Create similar lasts while its tool does
        if let Some((_, t)) = &self.plan.like {
            if *t != self.plan.tool {
                self.like_own();
            }
        }
        let Some((p, _)) = self.like.pointer else {
            return;
        };
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) || self.plan.tool != Tool::Select {
            if let Some((_, prev)) = self.like.pointer.take() {
                if p == LikePointer::PickUp && self.selection.is_empty() {
                    self.selection.extend(prev, false);
                }
            }
            return;
        }
        // what a click or a box took, once the button is up
        if !self.selection.is_empty() && !ctx.input(|i| i.pointer.any_down()) {
            let took = self.selection.all();
            match p {
                LikePointer::PickUp => {
                    let first = self.selection.active.unwrap_or(took[0]);
                    let (_, prev) = self.like.pointer.take().unwrap_or((p, vec![]));
                    self.hold(first);
                    self.selection.clear();
                    self.selection.extend(prev, false);
                }
                LikePointer::Apply => {
                    self.apply_held_to(&took);
                    self.selection.clear();
                }
            }
        }
        // the pointer's picture, and what a click would change
        let Some(at) = ctx.pointer_hover_pos() else {
            return;
        };
        let icon = match p {
            LikePointer::PickUp => "settings-pick",
            LikePointer::Apply => "settings-apply",
        };
        let pal = crate::theme::Palette::of(ctx);
        let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("like.pointer"));
        let painter = ctx.layer_painter(layer);
        let r = egui::Rect::from_min_size(at + egui::vec2(14.0, 14.0), egui::vec2(22.0, 22.0));
        painter.rect(
            r.expand(3.0),
            5.0,
            pal.panel,
            egui::Stroke::new(1.0, pal.border),
            egui::StrokeKind::Inside,
        );
        crate::svgicon::paint(&painter, r, icon, pal.text);
        let target = self
            .plan
            .hover
            .as_ref()
            .and_then(|h| h.under.get(h.index).copied())
            .and_then(|id| self.doc.model().get(id));
        let Some(target) = target else { return };
        let model = self.doc.model();
        let lines: Vec<String> = match (p, &self.like.held) {
            (LikePointer::Apply, Some(src)) => {
                let on = self.like_ticked(src);
                let c = likeness::changes(model, src, target, &on);
                let mut out = vec![format!("Apply to {}", short_name(model, target))];
                if c.is_empty() {
                    out.push("Nothing changes".into());
                }
                for (label, now, after) in c.iter().take(4) {
                    // (the app's font has no arrow)
                    out.push(format!("{label}: {now} to {after}"));
                }
                if c.len() > 4 {
                    out.push(format!("{} more", c.len() - 4));
                }
                out
            }
            _ => vec![
                format!("Pick up from {}", short_name(model, target)),
                target.name.clone(),
            ],
        };
        egui::Area::new(egui::Id::new("like.tip"))
            .order(egui::Order::Tooltip)
            .interactable(false)
            .fixed_pos(at + egui::vec2(46.0, 10.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    for (k, l) in lines.iter().enumerate() {
                        let t = egui::RichText::new(l).size(11.5);
                        ui.label(if k == 0 { t.strong() } else { t });
                    }
                });
            });
    }

    /// The status bar's chip while settings are held.
    pub(super) fn held_chip(&self) -> Option<Vec<serde_json::Value>> {
        let src = self.like.held.as_ref()?;
        let model = self.doc.model();
        let on = self.like_ticked(src).len();
        let all = likeness::groups(src).len();
        let name = short_name(model, src);
        Some(vec![
            serde_json::json!({"kind": "menu", "key": "held", "label": "", "value": format!("{name} · {on} of {all}"),
                "icon": "settings-pick", "join": "l",
                "hint": "The settings held (Ctrl+Shift+C): which go across, apply them, or make a type of them"}),
            serde_json::json!({"kind": "button", "id": "held.clear", "label": "", "icon": "x", "join": "r",
                "hint": "Let go of the settings held"}),
        ])
    }

    /// The chip's menu: apply, make a type, and a tick for each group.
    pub(super) fn held_items(&self) -> Vec<crate::ui_model::Item> {
        use crate::ui_model::Item;
        let Some(src) = &self.like.held else {
            return vec![];
        };
        let model = self.doc.model();
        let on = self.like_ticked(src);
        let mut items = vec![
            Item::Header(format!("Holding {}", short_name(model, src))),
            Item::new("held.apply", "Apply to the selection")
                .icon("settings-apply")
                .key("Ctrl+Shift+V")
                .enabled(!self.selection.is_empty())
                .hint("Give the selected elements these settings, in one step"),
            Item::new("held.arm", "Apply by clicking…")
                .hint("Click elements, or drag a box over them, to give them these settings"),
            Item::new("held.type", "Make a type of these…")
                .enabled(
                    model.get(src.id).is_some()
                        && creabim_core::TypeCategory::of(&src.kind).is_some(),
                )
                .hint("A type with these settings, to keep them"),
            Item::Separator,
            Item::Header("What goes across".into()),
        ];
        for g in likeness::groups(src) {
            let v = likeness::value(model, src, g);
            let label = if v.is_empty() {
                g.label(&src.kind).to_string()
            } else {
                format!("{}  ·  {v}", g.label(&src.kind))
            };
            items.push(
                Item::new(format!("held.g:{}", g as usize), label)
                    .checked(on.contains(&g))
                    .keep_open(),
            );
        }
        items.push(Item::Separator);
        items.push(Item::new("held.clear", "Let go").icon("x"));
        items
    }

    /// The chip's and the menus' actions. True when `id` was one of ours.
    pub(super) fn held_action(&mut self, ctx: &egui::Context, id: &str) -> bool {
        let (head, arg) = id.split_once(':').unwrap_or((id, ""));
        match head {
            "like.similar" => self.create_similar(ctx),
            "like.own" => {
                self.like_own();
                self.status = "Back to the tool's own settings.".into();
            }
            "like.pick" => self.pick_up_settings(),
            "like.apply" | "held.arm" => {
                if head == "held.arm" {
                    self.arm_like(LikePointer::Apply);
                } else {
                    self.apply_settings();
                }
            }
            "held.apply" => {
                let ids = self.selection.all();
                self.apply_held_to(&ids);
            }
            "held.clear" => {
                self.like.held = None;
                self.like.pointer = None;
                self.status = "Let go of the settings held.".into();
            }
            "held.type" => {
                if let Some(src) = self.like.held.as_ref().map(|e| e.id) {
                    self.run_types(super::typesui::TypeAsk::FromElement(src));
                }
            }
            "held.g" => {
                let Some(src) = self.like.held.clone() else {
                    return true;
                };
                let Some(g) = arg
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| Group::ALL.iter().copied().find(|g| *g as usize == i))
                else {
                    return true;
                };
                let mut on = self.like_ticked(&src);
                if let Some(k) = on.iter().position(|x| *x == g) {
                    on.remove(k);
                } else {
                    on.push(g);
                }
                self.like
                    .ticked
                    .insert(std::mem::discriminant(&src.kind), on);
            }
            _ => return false,
        }
        true
    }

    /// The right-click menu's group for the selection.
    pub(super) fn like_menu_items(&self) -> Vec<crate::ui_model::Item> {
        use crate::ui_model::Item;
        let one = self.selection.len() == 1;
        let similar = self
            .selection
            .active
            .and_then(|id| self.doc.model().get(id))
            .is_some_and(|e| tool_for(e).is_some());
        let mut items = vec![];
        if one {
            items.push(
                Item::new("like.similar", "Create similar")
                    .icon("create-similar")
                    .key("Shift+C")
                    .enabled(similar)
                    .hint("Its tool, set like it: what you place next is the same"),
            );
            items.push(
                Item::new("like.pick", "Pick up settings")
                    .icon("settings-pick")
                    .key("Ctrl+Shift+C")
                    .hint("Hold how it is made and how it looks, to give to others"),
            );
        }
        if self.like.held.is_some() {
            items.push(
                Item::new("like.apply", "Apply settings")
                    .icon("settings-apply")
                    .key("Ctrl+Shift+V")
                    .hint("Give the selection the settings held"),
            );
        }
        if !items.is_empty() {
            items.push(Item::Separator);
        }
        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_similar_has_a_tool_for_what_the_tools_draw() {
        let (a, b) = (
            creabim_core::sample::sample_house(),
            creabim_core::starter::starter_house(),
        );
        for e in a.iter().chain(b.iter()) {
            // everything placed or drawn in a view has its tool (MU135:
            // rooms, roofs, stairs, railings, curtain walls, ceilings,
            // details and annotations said "isn't there yet")
            let want = matches!(
                &e.kind,
                ElementKind::Wall(_)
                    | ElementKind::Slab(_)
                    | ElementKind::Opening(_)
                    | ElementKind::Roof(_)
                    | ElementKind::Stair(_)
                    | ElementKind::Railing(_)
                    | ElementKind::Room(_)
                    | ElementKind::Ceiling(_)
                    | ElementKind::Detail(_)
                    | ElementKind::Tag(_)
                    | ElementKind::Dimension(_)
                    | ElementKind::Column(_)
                    | ElementKind::Beam(_)
                    | ElementKind::Footing(_)
            ) || matches!(&e.kind, ElementKind::Note(n) if n.symbol.is_none())
                || matches!(&e.kind, ElementKind::Object(_) if e.of_type.is_some());
            if want {
                let ok = tool_for(e).is_some()
                    || matches!(&e.kind, ElementKind::Opening(o) if o.roof_at.is_some());
                assert!(ok, "{}", e.name);
            }
        }
    }

    /// Shift+C on a placed object (the starter house's washbasin) sets
    /// the Object tool to its type, turn and height; a click places one
    /// more of the same type; letting go gives the tool its own settings
    /// back (MU135: it said "Create similar isn't there yet").
    #[test]
    fn create_similar_places_more_of_an_objects_type() {
        let mut app = CreaBimApp::blank(None);
        app.load_model(creabim_core::starter::starter_house());
        let m = app.doc.model();
        let basin = m
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Object(_)) && e.name.contains("Washbasin"))
            .or_else(|| {
                m.iter()
                    .find(|e| matches!(e.kind, ElementKind::Object(_)) && e.of_type.is_some())
            })
            .expect("a placed object")
            .clone();
        let ty = basin.of_type.as_ref().unwrap().id;
        let ElementKind::Object(o) = &basin.kind else {
            unreachable!()
        };
        let (level, at) = (o.level, o.position);
        app.selection.only(basin.id);
        app.create_similar(&egui::Context::default());
        assert_eq!(app.plan.tool, Tool::Object, "{}", app.status);
        assert_eq!(app.plan.objects.project_type, Some(ty));
        assert!(app.plan.objects.current.is_some());
        // a click away from it places one more of its type
        let before = app
            .doc
            .model()
            .iter()
            .filter(|e| e.of_type.as_ref().map(|t| t.id) == Some(ty))
            .count();
        let mut sel = Selection::default();
        crate::objecttool::click(
            &mut app.doc,
            level,
            at + glam::DVec2::new(0.0, -1.5),
            &mut app.plan,
            &mut sel,
        )
        .unwrap();
        let after = app
            .doc
            .model()
            .iter()
            .filter(|e| e.of_type.as_ref().map(|t| t.id) == Some(ty))
            .count();
        assert_eq!(after, before + 1, "one more of the same type");
        app.like_own();
        assert_eq!(
            app.plan.objects.project_type, None,
            "the tool's own settings back"
        );
    }

    /// Shift+C on one element of each kind in the starter house starts
    /// its tool, set like it, and letting go gives the tool back its own
    /// settings (MU135: the rest said "isn't there yet").
    #[test]
    fn create_similar_starts_a_tool_for_every_kind_placed() {
        let mut app = CreaBimApp::blank(None);
        app.load_model(creabim_core::starter::starter_house());
        let ctx = egui::Context::default();
        let mut seen = std::collections::BTreeSet::new();
        let all: Vec<Element> = app.doc.model().iter().cloned().collect();
        for e in all {
            let Some(tool) = tool_for(&e) else { continue };
            if !seen.insert(tool.label()) {
                continue;
            }
            app.selection.only(e.id);
            app.create_similar(&ctx);
            assert_eq!(app.plan.tool, tool, "{}: {}", e.name, app.status);
            assert!(!app.status.contains("isn't there yet"), "{}", app.status);
            match &e.kind {
                ElementKind::Roof(r) => assert_eq!(app.plan.members.roof_pitch, r.pitch),
                ElementKind::Railing(r) => assert_eq!(app.plan.members.railing_height, r.height),
                ElementKind::Stair(st) => assert_eq!(app.plan.members.stair_width, st.width),
                ElementKind::Tag(t) => assert_eq!(app.plan.anno.tag_style, t.style),
                ElementKind::Detail(d) => assert_eq!(app.plan.details.kind, d.kind),
                _ => {}
            }
            app.like_own();
            assert!(app.plan.like.is_none());
        }
        for want in ["Roof", "Stair", "Railing", "Room", "Ceiling", "Tag", "Note"] {
            assert!(
                seen.iter().any(|l| l.contains(want)),
                "{want} not tried: {seen:?}"
            );
        }
    }
}
