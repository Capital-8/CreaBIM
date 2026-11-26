//! Grid systems and per-view visibility in the app: the grid groups panel
//! and menus, grid lines with their bubbles in the 3D view, and what each
//! view (and the 3D view) leaves out.
//!
//! What a view leaves out is stored on the view ([`ViewHidden`]) and edited
//! through ordinary transactions, so it undoes, saves, and carries over to
//! the sheets the view is placed on. The 3D view is not a view element: the
//! categories it leaves out are kept in the user's settings, the single
//! elements it hides for the session.
//!
//! Each Visibility menu and panel starts with quick switches for the 2D
//! objects (grids, annotation, guides, sketches), the ones most often in
//! the way while modelling.

use super::*;
use crate::ui_model::Item;
use creabim_core::grids::{self, Bubbles, GridColour, GridGroup, LabelStyle, Shape};
use creabim_core::{Category, Grid, ViewHidden};
use serde_json::{json, Value};
use std::collections::HashSet;

/// A change to what a view leaves out, picked in a panel.
type HiddenEdit = Box<dyn FnOnce(&mut ViewHidden)>;

/// Where visibility changes go: a view element, or the 3D view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum VisTarget {
    View(ElementId),
    Window3d,
}

impl VisTarget {
    /// The prefix of its menu actions.
    fn prefix(self) -> &'static str {
        match self {
            VisTarget::View(_) => "vis",
            VisTarget::Window3d => "vis3",
        }
    }
}

/// The quick switches of a Visibility menu: groups of categories, by key.
pub(super) const QUICK: [(&str, &str, &str, &[Category]); 4] = [
    // the building's grid lines: not the 3D view's Ground grid, so
    // another name and another icon (MU118)
    ("grids", "Grid lines", "hash", &[Category::Grids]),
    (
        "annotation",
        "Annotation",
        "tag",
        &[
            Category::Dimensions,
            Category::Tags,
            Category::Notes,
            Category::Levels,
            Category::SectionMarks,
        ],
    ),
    ("guides", "Guides", "slash", &[Category::Guides]),
    ("sketches", "Sketches", "pen-tool", &[Category::Sketches]),
];

/// What a quick switch covers, with "all" for every 2D object.
fn quick_group(key: &str, in_3d: bool) -> Vec<Category> {
    if key == "all" {
        let mut all: Vec<Category> = QUICK.iter().flat_map(|q| q.3.iter().copied()).collect();
        if in_3d {
            all.push(Category::Openings);
        }
        return all;
    }
    if key == "cutters" {
        return vec![Category::Openings];
    }
    QUICK
        .iter()
        .find(|q| q.0 == key)
        .map(|q| q.3.to_vec())
        .unwrap_or_default()
}

/// The hint of a quick switch.
fn quick_hint(key: &str) -> &'static str {
    match key {
        "annotation" => "Dimensions, tags, notes, level lines and section marks",
        "guides" => "Construction guides; Shift+G hides or shows them in every view",
        "grids" => "Grid lines and their bubbles",
        _ => "",
    }
}

/// How far a grid bubble sits past the end of its line in 3D, metres.
const BUBBLE_OUT_3D: f64 = 1.0;
/// A grid bubble's radius in the 3D view, points, for one or two characters.
const BUBBLE_R_3D: f32 = 11.0;

pub(super) fn color32(c: [f32; 3]) -> Color32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(b(c[0]), b(c[1]), b(c[2]))
}

impl CreaBimApp {
    // ---- what a view leaves out -----------------------------------------

    /// What `t` leaves out now.
    pub(super) fn view_hidden(&self, t: VisTarget) -> Option<ViewHidden> {
        match t {
            VisTarget::View(id) => self.view(id).map(|v| v.hidden.clone()),
            VisTarget::Window3d => Some(self.vis3d.clone()),
        }
    }

    /// Change what `t` leaves out. For a view it is one undo step.
    pub(super) fn set_view_hidden(
        &mut self,
        t: VisTarget,
        label: &str,
        f: impl FnOnce(&mut ViewHidden),
    ) {
        match t {
            VisTarget::View(id) => {
                let r = self.doc.edit(id, label, |e| {
                    if let ElementKind::View(v) = &mut e.kind {
                        f(&mut v.hidden);
                    }
                });
                let name = self.doc.model().get(id).map_or("", |e| e.name.as_str());
                let msg = format!("{label} in {name}.");
                self.report(r, msg);
            }
            VisTarget::Window3d => {
                f(&mut self.vis3d);
                self.hidden_rev += 1;
                self.snaps_3d = None;
                // the Grids category is the 3D view's Grid lines switch too
                self.shell.settings.grids_3d = !self.vis3d.hides_category(Category::Grids);
                self.geometry_key = None;
                self.status = format!("{label} in the 3D view.");
                // the categories are kept for next time; single elements
                // are for the session
                let s = &mut self.shell.settings;
                if s.hidden_3d != self.vis3d.categories {
                    s.hidden_3d = self.vis3d.categories.clone();
                    s.store();
                }
            }
        }
    }

    /// The view a right-click "hide in view" or the Visibility menu acts on.
    pub(super) fn vis_target(&self, in_3d: bool) -> Option<VisTarget> {
        if in_3d {
            return Some(VisTarget::Window3d);
        }
        let id = self.active_view?;
        match &self.view(id)?.kind {
            ViewKind::Schedule { .. } => None,
            _ => Some(VisTarget::View(id)),
        }
    }

    /// Elements the 3D view leaves out by its settings, and the guides
    /// when they are hidden everywhere.
    pub(super) fn hidden_in_3d(&self) -> HashSet<ElementId> {
        let mut out = self.vis3d.hidden_set(self.doc.model());
        if self.shell.settings.guides_hidden {
            out.extend(guide_ids(self.doc.model()));
        }
        out
    }

    /// Hide or show the guides in every view, keeping them.
    pub(super) fn set_guides_hidden(&mut self, hidden: bool) {
        let s = &mut self.shell.settings;
        s.guides_hidden = hidden;
        s.store();
        self.hidden_rev += 1;
        self.snaps_3d = None;
        let n = guide_ids(self.doc.model()).len();
        self.status = if hidden {
            format!("Guides hidden in every view ({n} kept). Shift+G or View > Guides brings them back.")
        } else {
            format!("Guides shown ({n}). Shift+G hides them again.")
        };
    }

    /// Turn the 3D view's X-ray on or off.
    pub(super) fn set_xray(&mut self, on: bool) {
        let s = &mut self.shell.settings;
        s.xray = on;
        self.plan.hover = None;
        s.store();
        self.geometry_key = None;
        self.status = if on {
            "X-ray: walls, slabs, roofs and masses are see-through, so beams, columns and what is inside them show. Alt+Z turns it off.".into()
        } else {
            "X-ray off.".into()
        };
    }

    /// Does the view show elements hidden inside others, dashed?
    pub(super) fn shows_hidden_elements(&self, id: ElementId) -> bool {
        self.view(id).is_some_and(|v| v.show_hidden)
    }

    /// Show or stop showing, dashed, what is hidden inside other elements in
    /// view `id`. One undo step.
    pub(super) fn toggle_hidden_elements(&mut self, id: ElementId) {
        let on = !self.shows_hidden_elements(id);
        let label = if on {
            "Show hidden elements"
        } else {
            "Stop showing hidden elements"
        };
        let r = self.doc.edit(id, label, |e| {
            if let ElementKind::View(v) = &mut e.kind {
                v.show_hidden = on;
            }
        });
        self.report(
            r,
            if on {
                "Elements inside others (a beam in a wall) show dashed in this view and on its sheets."
            } else {
                "Hidden elements are left out again."
            },
        );
    }

    /// Does view `id` draw what is hidden inside others in its colours?
    pub(super) fn shows_hidden_colour(&self, id: ElementId) -> bool {
        self.view(id).is_some_and(|v| v.hidden_in_colour)
    }

    /// Draw, or stop drawing, what is hidden inside other elements in view
    /// `id` in its material colours, washed over what hides it (showing it
    /// dashed too). One undo step.
    pub(super) fn toggle_hidden_colour(&mut self, id: ElementId) {
        let on = !self.shows_hidden_colour(id);
        let label = if on {
            "Show hidden elements in colour"
        } else {
            "Show hidden elements dashed only"
        };
        let r = self.doc.edit(id, label, |e| {
            if let ElementKind::View(v) = &mut e.kind {
                v.hidden_in_colour = on;
                if on {
                    v.show_hidden = true;
                }
            }
        });
        self.report(
            r,
            if on {
                "Elements inside others show in their material colours, see-through over what hides them, in this view and on its sheets. Click inside one to pick it."
            } else {
                "Hidden elements are dashed only again."
            },
        );
    }

    /// Show, or stop showing, what is inside walls, slabs and roofs in the
    /// 3D view in its colours, faintly through what hides it.
    pub(super) fn set_hidden_colour_3d(&mut self, on: bool) {
        let s = &mut self.shell.settings;
        s.hidden_colour_3d = on;
        s.store();
        self.geometry_key = None;
        self.status = if on {
            "The 3D view shows beams, columns and what else is inside walls, slabs and roofs in their colours through them.".into()
        } else {
            "What is inside walls and slabs is hidden again in 3D.".into()
        };
    }

    /// Show the ground in the 3D view after its works, before them, or
    /// both (the ground as it was see-through over the works).
    pub(super) fn set_ground_3d(&mut self, g: creabim_core::GroundState) {
        let s = &mut self.shell.settings;
        s.ground_3d = g;
        s.store();
        self.geometry_key = None;
        self.status = match g {
            creabim_core::GroundState::After => "The 3D view shows the ground with its works.",
            creabim_core::GroundState::Before => {
                "The 3D view shows the ground as surveyed, before its works, with no roads."
            }
            creabim_core::GroundState::Both => {
                "The 3D view shows the ground as it was see-through over its works."
            }
        }
        .into();
    }

    /// Hide the selection in `t`: the elements themselves, their
    /// categories, or the grid groups of the selected grid lines.
    pub(super) fn hide_selected_in(&mut self, t: VisTarget, how: &str) {
        let ids = self.selection.all();
        if ids.is_empty() {
            self.status = "Select something to hide first.".into();
            return;
        }
        let model = self.doc.model();
        let cutters = creabim_core::visibility::cutters(model);
        let cats: Vec<Category> = ids
            .iter()
            .filter_map(|id| Category::of(model.get(*id)?, &cutters))
            .fold(vec![], |mut v, c| {
                if !v.contains(&c) {
                    v.push(c);
                }
                v
            });
        let groups: Vec<ElementId> = ids
            .iter()
            .filter_map(|id| match &model.get(*id)?.kind {
                ElementKind::Grid(g) => g.group,
                ElementKind::GridGroup(_) => Some(*id),
                _ => None,
            })
            .collect();
        match how {
            "category" if !cats.is_empty() => {
                let names: Vec<&str> = cats.iter().map(|c| c.label()).collect();
                let label = format!("Hide {}", names.join(", "));
                self.set_view_hidden(t, &label, |h| {
                    for c in cats {
                        h.set_category(c, true);
                    }
                });
            }
            "group" if !groups.is_empty() => {
                self.set_view_hidden(t, "Hide grid system", |h| {
                    for g in groups {
                        h.set_grid_group(g, true);
                    }
                });
            }
            // only the selection, and what it stands on, stays in this
            // view (MU34: a 3D view on a sheet showed the whole model)
            "others" => {
                let mut keep: std::collections::HashSet<ElementId> = ids.iter().copied().collect();
                for id in &ids {
                    if let Some(e) = model.get(*id) {
                        keep.extend(e.references());
                    }
                    keep.extend(model.dependents(*id));
                }
                let others: Vec<ElementId> = model
                    .iter()
                    .filter(|e| !keep.contains(&e.id) && crate::app::gridvis::shows_in_3d(&e.kind))
                    .map(|e| e.id)
                    .collect();
                let label = format!("Show only {} in this view", ids.len());
                self.set_view_hidden(t, &label, |h| {
                    for id in others {
                        h.set_element(id, true);
                    }
                });
            }
            "element" => {
                let label = match ids.len() {
                    1 => "Hide element".to_string(),
                    n => format!("Hide {n} elements"),
                };
                self.set_view_hidden(t, &label, |h| {
                    for id in ids {
                        h.set_element(id, true);
                    }
                });
            }
            _ => {
                self.status = "Nothing of that kind is selected.".into();
                return;
            }
        }
        self.selection.clear();
        self.face = None;
    }

    // ---- menus ---------------------------------------------------------

    /// The items of the Visibility menu of `t`.
    pub(super) fn visibility_items(&self, t: VisTarget) -> Vec<Item> {
        let Some(h) = self.view_hidden(t) else {
            return vec![];
        };
        let p = t.prefix();
        let model = self.doc.model();
        let in_3d = t == VisTarget::Window3d;
        let shown = |key: &str| {
            quick_group(key, in_3d)
                .iter()
                .all(|c| !h.hides_category(*c))
        };
        let mut items = vec![
            Item::Header(match t {
                VisTarget::View(_) => "This view shows".into(),
                VisTarget::Window3d => "The 3D view shows".into(),
            }),
            Item::new(format!("{p}.quick:all"), "All 2D objects")
                .checked(shown("all"))
                .icon("eye")
                .keep_open()
                .hint("Grids, annotation, guides and sketches together"),
        ];
        for (key, label, icon, _) in QUICK {
            items.push(
                Item::new(format!("{p}.quick:{key}"), label)
                    .checked(shown(key))
                    .icon(icon)
                    .keep_open()
                    .hint(quick_hint(key)),
            );
        }
        if in_3d {
            let s = &self.shell.settings;
            items.extend([
                Item::new(format!("{p}.quick:cutters"), "Void cutters")
                    .checked(shown("cutters"))
                    .icon("square-dashed")
                    .keep_open()
                    .hint("The outlines of elements used to cut voids into others"),
                Item::new("ground.grid", "Ground grid")
                    .checked(s.ground_grid)
                    .icon("grid-3x3")
                    .keep_open(),
                Item::new("plan3d", "The 2D view in 3D space")
                    .checked(self.plan_in_3d)
                    .icon("map")
                    .keep_open(),
                Item::Separator,
                Item::new("xray", "X-ray")
                    .key("Alt+Z")
                    .checked(s.xray)
                    .icon("xray-ball")
                    .keep_open()
                    .hint(
                        "Walls, slabs, roofs and masses see-through, so what is inside them shows",
                    ),
                Item::new("hidden.colour3d", "Hidden elements in colour")
                    .checked(s.hidden_colour_3d)
                    .icon("hidden-colour")
                    .keep_open()
                    .hint("What is inside walls, slabs and roofs shows through them in its own colours"),
            ]);
            let has_ground = self.doc.model().iter().any(|e| {
                matches!(&e.kind, ElementKind::Site(st)
                    if matches!(st.form, creabim_core::SiteForm::Terrain { .. }))
            });
            if has_ground {
                items.push(Item::Separator);
                for (k, g) in creabim_core::GroundState::ALL.iter().enumerate() {
                    items.push(
                        Item::new(format!("ground3d:{k}"), format!("Ground: {}", g.label()))
                            .checked(s.ground_3d == *g)
                            .icon("terrain")
                            .keep_open(),
                    );
                }
            }
        } else if let VisTarget::View(id) = t {
            items.extend([
                Item::Separator,
                Item::new(
                    format!("vis.reveal:{}", id.0),
                    "Elements hidden inside others",
                )
                .checked(self.shows_hidden_elements(id))
                // the header's own icon for it, not X-ray's (MU23)
                .icon("hidden-elements")
                .keep_open()
                .hint(
                    "Draw what is inside another element, like a beam in a wall, with dashed lines",
                ),
                Item::new(format!("vis.colour:{}", id.0), "Hidden elements in colour")
                    .checked(self.shows_hidden_colour(id))
                    .icon("hidden-colour")
                    .keep_open()
                    .hint("For detailing: what is inside others in its material colours, see-through, as well as dashed. Sheets show it too"),
                Item::new(format!("vis.xray:{}", id.0), "X-ray while working")
                    .key("Alt+Z")
                    .checked(self.plan.xray.contains(&id))
                    .icon("xray-ball")
                    .keep_open()
                    .hint("Walls, slabs and roofs see-through here, what is inside them solid and clicked first. Sheets don't change"),
            ]);
        }
        if self.shell.settings.guides_hidden {
            items.push(
                Item::new("guides.toggle", "Guides are hidden everywhere: show them")
                    .key("Shift+G")
                    .icon("slash"),
            );
        }
        items.push(Item::Separator);
        items.push(Item::Header("Categories".into()));
        for (i, c) in Category::ALL.iter().enumerate() {
            items.push(
                Item::new(format!("{p}.cat:{i}"), c.label())
                    .checked(!h.hides_category(*c))
                    .keep_open(),
            );
        }
        let groups = grids::groups(model);
        if !groups.is_empty() {
            items.push(Item::Separator);
            items.push(Item::Header("Grid systems".into()));
            for (e, g) in groups {
                items.push(
                    Item::new(format!("{p}.grp:{}", e.id.0), &e.name)
                        .checked(!h.grid_groups.contains(&e.id))
                        .enabled(!h.hides_category(Category::Grids))
                        .hint(format!("Labels {}1, {}A…", g.prefix, g.prefix))
                        .keep_open(),
                );
            }
        }
        let hidden: Vec<&Element> = h.elements.iter().filter_map(|id| model.get(*id)).collect();
        if !hidden.is_empty() {
            items.push(Item::Separator);
            let mut list: Vec<Item> = hidden
                .iter()
                .map(|e| Item::new(format!("{p}.unhide:{}", e.id.0), format!("Show {}", e.name)))
                .collect();
            list.push(Item::Separator);
            list.push(Item::new(format!("{p}.unhideall"), "Show them all"));
            items.push(Item::sub(
                format!("Hidden elements ({})", hidden.len()),
                list,
            ));
        }
        items.push(Item::Separator);
        items.push(
            Item::new(format!("{p}.reset"), "Show everything")
                .icon("eye")
                .enabled(!h.is_empty()),
        );
        items
    }

    /// Right-click items that hide the selection in the view clicked.
    pub(super) fn hide_in_view_items(&self, in_3d: bool) -> Vec<Item> {
        if self.selection.is_empty() {
            return vec![];
        }
        let enabled = self.vis_target(in_3d).is_some();
        let where_ = if in_3d { "3D view" } else { "view" };
        let p = if in_3d { "vis3" } else { "vis" };
        let mut items = vec![
            Item::new(format!("{p}.hide:element"), format!("Hide in {where_}"))
                .icon("eye-off")
                .enabled(enabled)
                .hint("Leave the selection out of this view only; its Visibility settings bring it back"),
            Item::new(format!("{p}.hide:category"), format!("Hide category in {where_}"))
                .enabled(enabled)
                .hint("Leave out every element of the selection's kind (walls, windows…) in this view"),
            Item::new(format!("{p}.hide:others"), format!("Show only the selection in {where_}"))
                .enabled(enabled)
                .hint("Keep only what is selected in this view, saved with it and on its sheets; its Visibility settings bring the rest back"),
        ];
        let model = self.doc.model();
        let grouped = self.selection.all().iter().any(|id| {
            matches!(
                model.get(*id).map(|e| &e.kind),
                Some(ElementKind::Grid(Grid { group: Some(_), .. }) | ElementKind::GridGroup(_))
            )
        });
        if grouped {
            items.push(
                Item::new(
                    format!("{p}.hide:group"),
                    format!("Hide grid system in {where_}"),
                )
                .enabled(enabled),
            );
        }
        items.push(Item::Separator);
        items
    }

    /// The grid systems dropdown of the Grid tool.
    pub(super) fn grid_group_items(&self) -> Vec<Item> {
        let model = self.doc.model();
        let active = self.active_grid_group();
        let mut items = vec![
            Item::Header("New grid lines go in".into()),
            Item::new("gg.active:", "No grid system").checked(active.is_none()),
        ];
        for (e, g) in grids::groups(model) {
            let n = grid_count(model, Some(e.id));
            items.push(
                Item::new(format!("gg.active:{}", e.id.0), &e.name)
                    .checked(active == Some(e.id))
                    .hint(format!(
                        "{n} lines, labelled {}{} and {}{}",
                        g.prefix,
                        g.vertical.format(1),
                        g.prefix,
                        g.horizontal.format(1)
                    )),
            );
        }
        items.push(Item::Separator);
        items.push(
            Item::new("gg.new", "New grid system")
                .icon("plus")
                .hint("A group of grid lines with its own labels (A-1, B-1…) and colour, e.g. for a second building"),
        );
        items.push(
            Item::new("gg.renumber", "Renumber the lines in order")
                .hint("West to east and south to north"),
        );
        let selected_lines = self
            .selection
            .all()
            .iter()
            .filter(|id| matches!(model.get(**id).map(|e| &e.kind), Some(ElementKind::Grid(_))))
            .count();
        if selected_lines > 0 {
            let mut to = vec![Item::new("gg.adopt:", "No grid system")];
            to.extend(
                grids::groups(model)
                    .into_iter()
                    .map(|(e, _)| Item::new(format!("gg.adopt:{}", e.id.0), &e.name)),
            );
            items.push(Item::sub(
                format!("Move the {selected_lines} selected lines to"),
                to,
            ));
        }
        let arg = active.map_or(String::new(), |g| g.0.to_string());
        items.push(Item::new(format!("gg.select:{arg}"), "Select its lines"));
        if let Some(g) = active {
            items.push(Item::new(
                format!("gg.edit:{}", g.0),
                "Edit the grid system…",
            ));
        }
        items
    }

    /// The active grid group, if it still exists.
    pub(super) fn active_grid_group(&self) -> Option<ElementId> {
        self.plan.grid_group.filter(|g| {
            matches!(
                self.doc.model().get(*g).map(|e| &e.kind),
                Some(ElementKind::GridGroup(_))
            )
        })
    }

    pub(super) fn new_grid_group(&mut self) {
        self.new_grid_group_here(true);
    }

    /// A new grid system, active for the Grid tool; selected too when
    /// `select` (from a menu), else the tool's tab shows it (REV01).
    pub(super) fn new_grid_group_here(&mut self, select: bool) {
        let e = grids::new_group(self.doc.model());
        let (id, name) = (e.id, e.name.clone());
        let r = self
            .doc
            .commit(Transaction::new(format!("New grid system {name}")).insert(e));
        if r.is_ok() {
            self.plan.grid_group = Some(id);
            self.plan.tool = Tool::Grid;
            if select {
                self.selection.only(id);
                self.face = None;
            }
        }
        self.report(
            r,
            format!("New grid system {name}: grid lines you draw now go in it."),
        );
    }

    /// Put the selected grid lines in `group` (`None`: out of any). They
    /// keep their labels unless one is taken there already.
    pub(super) fn move_selected_grids(&mut self, group: Option<ElementId>) {
        let model = self.doc.model();
        let mut tx = Transaction::new("Move grid lines to another system");
        let mut probe = model.clone();
        for id in self.selection.all() {
            let Some(e) = probe.get(id).cloned() else {
                continue;
            };
            let ElementKind::Grid(g) = &e.kind else {
                continue;
            };
            if g.group == group {
                continue;
            }
            let taken = probe.iter().any(|o| {
                o.id != id
                    && matches!(&o.kind, ElementKind::Grid(og) if og.group == group && og.label == g.label)
            });
            let mut after = e.clone();
            if let ElementKind::Grid(ag) = &mut after.kind {
                if taken {
                    ag.label = grids::next_label(&probe, group, ag.start, ag.end);
                }
                ag.group = group;
            }
            let step = Transaction::new("").replace(e.clone(), after.clone());
            if step.apply(&mut probe).is_ok() {
                tx = tx.replace(e, after);
            }
        }
        if tx.ops.is_empty() {
            self.status = "Select grid lines to move first.".into();
            return;
        }
        let n = tx.ops.len();
        let r = self.doc.commit(tx);
        self.report(r, format!("Moved {n} grid lines."));
    }

    /// Menu choices from this module's menus. False when `head` is not one.
    pub(super) fn grid_vis_action(&mut self, head: &str, arg: &str) -> bool {
        let model = self.doc.model();
        let by_uuid = || {
            model
                .iter()
                .find(|e| e.id.0.to_string() == arg)
                .map(|e| e.id)
        };
        let index = arg.parse::<usize>().ok();
        let (prefix, what) = head.split_once('.').unwrap_or((head, ""));
        match prefix {
            "vis" | "vis3" if what == "hide" => {
                if let Some(t) = self.vis_target(prefix == "vis3") {
                    self.hide_selected_in(t, arg);
                }
                true
            }
            "vis" | "vis3" => {
                let Some(t) = self.vis_target(prefix == "vis3") else {
                    self.status = "Open a plan, section or 3D view first.".into();
                    return true;
                };
                let Some(h) = self.view_hidden(t) else {
                    return true;
                };
                let id = by_uuid();
                match what {
                    "quick" => {
                        let group = quick_group(arg, t == VisTarget::Window3d);
                        if group.is_empty() {
                            return true;
                        }
                        let hide = group.iter().all(|c| !h.hides_category(*c));
                        let name = match QUICK.iter().find(|q| q.0 == arg) {
                            Some(q) => q.1.to_lowercase(),
                            None if arg == "cutters" => "cutting shapes".into(),
                            None => "2D objects".into(),
                        };
                        let label = format!("{} {name}", if hide { "Hide" } else { "Show" });
                        self.set_view_hidden(t, &label, |h| {
                            for c in group {
                                h.set_category(c, hide);
                            }
                        });
                        if arg == "all" && t == VisTarget::Window3d {
                            let s = &mut self.shell.settings;
                            s.ground_grid = !hide;
                            s.store();
                            self.geometry_key = None;
                        }
                    }
                    "reveal" => {
                        if let Some(v) = id {
                            self.toggle_hidden_elements(v);
                        }
                    }
                    "colour" => {
                        if let Some(v) = id {
                            self.toggle_hidden_colour(v);
                        }
                    }
                    "xray" => {
                        if let Some(v) = id {
                            self.toggle_view_xray(v);
                        }
                    }
                    "cat" => {
                        if let Some(c) = index.and_then(|i| Category::ALL.get(i)).copied() {
                            let hide = !h.hides_category(c);
                            let label =
                                format!("{} {}", if hide { "Hide" } else { "Show" }, c.label());
                            self.set_view_hidden(t, &label, |h| h.set_category(c, hide));
                        }
                    }
                    "grp" => {
                        if let Some(g) = id {
                            let hide = !h.grid_groups.contains(&g);
                            let label = if hide {
                                "Hide grid system"
                            } else {
                                "Show grid system"
                            };
                            self.set_view_hidden(t, label, |h| h.set_grid_group(g, hide));
                        }
                    }
                    "unhide" => {
                        if let Some(e) = id {
                            self.set_view_hidden(t, "Show hidden element", |h| {
                                h.set_element(e, false)
                            });
                        }
                    }
                    "unhideall" => {
                        self.set_view_hidden(t, "Show hidden elements", |h| h.elements.clear())
                    }
                    "reset" => {
                        self.set_view_hidden(t, "Show everything", |h| *h = ViewHidden::default())
                    }
                    _ => return false,
                }
                true
            }
            "gg" => {
                match what {
                    "active" => {
                        self.plan.grid_group = by_uuid();
                        self.plan.tool = Tool::Grid;
                        self.plan.cancel();
                        let name = self
                            .plan
                            .grid_group
                            .and_then(|g| self.doc.model().get(g))
                            .map_or("no grid system".to_string(), |e| e.name.clone());
                        self.status = format!("New grid lines go in {name}.");
                    }
                    "new" => self.new_grid_group(),
                    "adopt" => self.move_selected_grids(by_uuid()),
                    "renumber" => {
                        let g = self.active_grid_group();
                        let tx = grids::renumber(self.doc.model(), g);
                        if tx.ops.is_empty() {
                            self.status = "The lines are numbered in order already.".into();
                        } else {
                            let n = tx.ops.len();
                            let r = self.doc.commit(tx);
                            self.report(r, format!("Relabelled {n} grid lines in order."));
                        }
                    }
                    "select" => {
                        let ids = grid_ids(self.doc.model(), by_uuid());
                        self.selection.clear();
                        self.selection.extend(ids, false);
                        self.face = None;
                    }
                    "edit" => {
                        if let Some(g) = by_uuid() {
                            self.selection.only(g);
                            self.face = None;
                        }
                    }
                    _ => return false,
                }
                true
            }
            _ => false,
        }
    }

    /// Items for the top bar when they apply: the Grid tool's grid system,
    /// and the active view's Visibility menu.
    pub(super) fn grid_vis_options(&self) -> Vec<Value> {
        let mut out = vec![];
        if self.plan.tool == Tool::Grid {
            // a straight line, or an arc by its ends and a point on it (MU66)
            let arc = self.plan.grid_arc;
            out.push(json!({"kind": "seg", "key": "gridshape", "items": [
                {"id": "grid.shape:0", "label": "Line", "on": !arc, "hint": "Two clicks: its ends"},
                {"id": "grid.shape:1", "label": "Arc", "on": arc, "hint": "Three clicks: its two ends, then a point it passes through"},
            ]}));
            let name = self
                .active_grid_group()
                .and_then(|g| self.doc.model().get(g))
                .map_or("None".to_string(), |e| e.name.clone());
            // read only: the system is picked and set up in Properties
            // (REV01, decision 2)
            out.push(json!({
                "kind": "label", "key": "gridgroups",
                "label": if name == "None" { "No grid system".to_string() } else { format!("In {name}") },
            }));
            out.push(json!({"kind": "sep"}));
        }
        if self.vis_target(false).is_some() {
            out.push(json!({
                "kind": "menu", "key": "visibility", "label": "", "value": "Visibility",
                "icon": "eye",
                "hint": "What this view shows: grids, annotation, guides, categories, grid systems and hidden elements. Sheets show their views the same way.",
            }));
        }
        out
    }

    /// Items of this module's menus, by key.
    pub(super) fn grid_vis_menu(&self, key: &str) -> Option<Vec<Item>> {
        Some(match key {
            "gridgroups" => self.grid_group_items(),
            "visibility" => self.visibility_items(self.vis_target(false)?),
            "vis3d" => self.visibility_items(VisTarget::Window3d),
            _ => return None,
        })
    }

    /// Set the 3D ground grid's spacing to a typed length, in any unit.
    /// Returns what went wrong when it can't be read.
    pub(super) fn set_ground_spacing_text(&mut self, text: &str) -> Result<f64, String> {
        let (lo, hi) = GROUND_SPACING_RANGE;
        let v = crate::units::parse(text)
            .filter(|v| v.is_finite() && *v > 0.0)
            .ok_or_else(|| format!("Couldn't read \"{text}\" as a length."))?;
        if !(lo..=hi).contains(&v) {
            return Err(format!(
                "Pick a spacing between {} and {}.",
                crate::units::len(lo),
                crate::units::len(hi)
            ));
        }
        let s = &mut self.shell.settings;
        s.ground_grid_spacing = v;
        s.ground_grid = true;
        s.store();
        self.geometry_key = None;
        self.status = format!("Ground grid every {}.", crate::units::len(v));
        Ok(v)
    }

    /// The little window the ground grid's "Custom…" spacing is typed in.
    pub(super) fn ground_spacing_window(&mut self, ctx: &egui::Context) {
        let Some(mut text) = self.shell.ground_custom.take() else {
            return;
        };
        let mut open = true;
        let mut done = false;
        let mut error = None;
        let area = self.mat.area.unwrap_or_else(|| ctx.content_rect());
        crate::app::shell::window("Ground grid spacing")
            .open(&mut open)
            .default_pos(area.min + Vec2::new(60.0, 80.0))
            .constrain_to(area)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                crate::props::info(
                    ui,
                    "Any spacing, in the project's units or another (1250, 1.25 m, 4'-0\").",
                    "",
                );
                crate::props::flow(ui, |ui| {
                    ui.label("Every");
                    let r =
                        crate::props::text_field(ui, egui::TextEdit::singleline(&mut text), 110.0);
                    if !r.has_focus() && !r.lost_focus() && ui.memory(|m| m.focused().is_none()) {
                        r.request_focus();
                    }
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.button("Apply").clicked() || enter {
                        done = true;
                    }
                });
                crate::props::flow(ui, |ui| {
                    for s in GROUND_SPACINGS {
                        if ui.small_button(crate::units::len(s)).clicked() {
                            text = crate::units::len(s);
                            done = true;
                        }
                    }
                });
            });
        if done {
            match self.set_ground_spacing_text(&text) {
                Ok(_) => return,
                Err(e) => error = Some(e),
            }
        }
        if let Some(e) = error {
            self.status = e;
        }
        if open {
            self.shell.ground_custom = Some(text);
        }
    }

    // ---- properties panel ---------------------------------------------

    /// The Visibility section of a view's properties.
    /// `heading` names the section (the view's name, "3D view"), or it is
    /// just "Visibility".
    pub(super) fn visibility_panel(
        &mut self,
        ui: &mut egui::Ui,
        t: VisTarget,
        heading: Option<&str>,
    ) {
        let Some(h) = self.view_hidden(t) else {
            return;
        };
        let mut change: Option<(String, HiddenEdit)> = None;
        // switches that are not the view's own: run as menu actions
        let mut extra: Option<&str> = None;
        let mut ground_3d: Option<creabim_core::GroundState> = None;
        let mut ground_3d_was: Option<f32> = None;
        let mut envelope_3d: Option<bool> = None;
        let mut lot_lines_3d: Option<bool> = None;
        let mut reveal = None;
        let mut colour_of = None;
        let mut xray_of = None;
        let head = heading.unwrap_or("Visibility");
        let title = if h.is_empty() {
            head.to_string()
        } else {
            let n = h.categories.len() + h.elements.len() + h.grid_groups.len();
            format!("{head} ({n} hidden)")
        };
        let (key, help) = match t {
            VisTarget::View(_) => ("visibility.view", "visibility"),
            VisTarget::Window3d => ("visibility.3d", "navigate"),
        };
        // folded or open as it was left, between sessions too
        let open = self.prop_header(ui, key, &title, t != VisTarget::Window3d, Some(help));
        crate::props::fold(ui, open, |ui| {
            // flush in its box, as Blender's (MU22)
            ui.vertical(|ui| {
                // as Blender's (MU22): check boxes in the value column, each
                // group named once in the label column
                let in_3d = t == VisTarget::Window3d;
                {
                    for (n, (key, label, _, _)) in QUICK.iter().enumerate() {
                        let group = quick_group(key, in_3d);
                        let mut shown = group.iter().all(|c| !h.hides_category(*c));
                        if crate::props::check(ui, if n == 0 { "Show" } else { "" }, &mut shown, *label)
                            .on_hover_text(quick_hint(key))
                            .changed()
                        {
                            let label = format!(
                                "{} {}",
                                if shown { "Show" } else { "Hide" },
                                label.to_lowercase()
                            );
                            change = Some((
                                label,
                                Box::new(move |h| {
                                    for c in group {
                                        h.set_category(c, !shown);
                                    }
                                }),
                            ));
                        }
                    }
                }
                if in_3d {
                    ui.vertical(|ui| {
                        let mut ground = self.shell.settings.ground_grid;
                        if crate::props::check(ui, "Also", &mut ground, "Ground grid").changed() {
                            extra = Some("ground.grid");
                        }
                        let mut xray = self.shell.settings.xray;
                        if crate::props::switch(ui, &mut xray, "X-ray")
                            .on_hover_text("Walls, slabs, roofs and masses see-through (Alt+Z)")
                            .changed()
                        {
                            extra = Some("xray");
                        }
                        let has_ground = self.doc.model().iter().any(|e| {
                            matches!(&e.kind, ElementKind::Site(s)
                                if matches!(s.form, creabim_core::SiteForm::Terrain { .. }))
                        });
                        if has_ground {
                            let now = self.shell.settings.ground_3d;
                            let mut g = now;
                            crate::props::row(ui, "Ground", |ui| {
                                egui::ComboBox::from_id_salt("vis3d.ground")
                                    .icon(crate::props::combo_icon)
                                    .selected_text(g.label())
                                    .show_ui(ui, |ui| {
                                        for o in creabim_core::GroundState::ALL {
                                            ui.selectable_value(&mut g, o, o.label());
                                        }
                                    })
                                    .response
                                    .on_hover_text("Before shows the ground as surveyed, with no roads. Both shows the ground as it was see-through over the works where they changed it")
                            });
                            if g != now {
                                ground_3d = Some(g);
                            }
                            // between the two grounds: how much of the
                            // ground as it was shows over the works
                            if g == creabim_core::GroundState::Both {
                                let mut was = self.shell.settings.ground_3d_was * 100.0;
                                let moved = crate::props::row(ui, "As it was", |ui| {
                                    crate::props::fill(
                                        ui,
                                        egui::Slider::new(&mut was, 5.0..=95.0)
                                            .fixed_decimals(0)
                                            .suffix(" %"),
                                    )
                                    .on_hover_text("How solid the ground as surveyed is over the works: slide down to see the works, up to see the ground they replaced")
                                    .changed()
                                });
                                if moved {
                                    ground_3d_was = Some(was / 100.0);
                                }
                            }
                        }
                        let has_lot = self.doc.model().iter().any(|e| {
                            matches!(&e.kind, ElementKind::Site(s) if s.is_property())
                        });
                        if has_lot {
                            let mut lines = !self.shell.settings.lot_lines_3d_off;
                            if crate::props::check(ui, "Property", &mut lines, "Lines on the ground")
                                .on_hover_text("The property's boundary and its setback lying on the ground, in the plan's colours. Never printed or exported")
                                .changed()
                            {
                                lot_lines_3d = Some(lines);
                            }
                            let mut on = !self.shell.settings.envelope_3d_off;
                            if crate::props::check(ui, "Envelope", &mut on, "Of the property")
                                .on_hover_text("The space the planning rules leave to build in, outlined: the buildable area up to the height limit, under the angle planes. Never printed or exported")
                                .changed()
                            {
                                envelope_3d = Some(on);
                            }
                        }
                        let mut colour = self.shell.settings.hidden_colour_3d;
                        if crate::props::switch(ui, &mut colour, "Hidden elements in colour")
                            .on_hover_text("What is inside walls, slabs and roofs shows through them in its own colours")
                            .changed()
                        {
                            extra = Some("hidden.colour3d");
                        }
                    });
                } else if let VisTarget::View(id) = t {
                    let mut on = self.shows_hidden_elements(id);
                    if crate::props::check(ui, "Hidden inside", &mut on, "Dashed")
                        .on_hover_text("A beam in a wall, a stud wall in a cavity: drawn with dashed hidden lines")
                        .changed()
                    {
                        reveal = Some(id);
                    }
                    let mut colour = self.shows_hidden_colour(id);
                    if crate::props::check(ui, "", &mut colour, "In material colours")
                        .on_hover_text("For detailing: washed in their own colours over what hides them, as well as dashed")
                        .changed()
                    {
                        colour_of = Some(id);
                    }
                    let mut xray = self.plan.xray.contains(&id);
                    if crate::props::check(ui, "While working", &mut xray, "X-ray")
                        .on_hover_text("Walls, slabs and roofs see-through here, what is inside them solid and clicked first (Alt+Z). Sheets don't change")
                        .changed()
                    {
                        xray_of = Some(id);
                    }
                }
                let mut guides = !self.shell.settings.guides_hidden;
                if crate::props::check(ui, "Guides", &mut guides, "In every view")
                    .on_hover_text("Hide all guides without deleting them, and bring them back (Shift+G)")
                    .changed()
                {
                    extra = Some("guides.toggle");
                }
            });
        });
        // the categories a section of their own, after the view's: inside
        // it the view's section ran too long (owner MU107)
        let cat_key = match t {
            VisTarget::View(_) => "visibility.categories",
            VisTarget::Window3d => "visibility.categories3d",
        };
        let open = self.prop_header(
            ui,
            cat_key,
            "Categories",
            t != VisTarget::Window3d,
            Some(help),
        );
        crate::props::fold(ui, open, |ui| {
            ui.vertical(|ui| {
                {
                    // two compact columns, as Blender's lists of toggles
                    let cols = if ui.available_width() >= 180.0 { 2 } else { 1 };
                    let cw = (ui.available_width() / cols as f32 - 4.0).max(60.0);
                    egui::Grid::new(("vis-cats", format!("{t:?}")))
                        .num_columns(cols)
                        .spacing(egui::vec2(4.0, 1.0))
                        .show(ui, |ui| {
                            for (i, c) in Category::ALL.iter().enumerate() {
                                let mut shown = !h.hides_category(*c);
                                if crate::props::tick(ui, &mut shown, c.label(), cw).changed() {
                                    let c = *c;
                                    let label = format!(
                                        "{} {}",
                                        if shown { "Show" } else { "Hide" },
                                        c.label()
                                    );
                                    change =
                                        Some((label, Box::new(move |h| h.set_category(c, !shown))));
                                }
                                if i % cols == cols - 1 {
                                    ui.end_row();
                                }
                            }
                        });
                }
                let groups: Vec<(ElementId, String, [f32; 3])> = grids::groups(self.doc.model())
                    .into_iter()
                    .map(|(e, g)| (e.id, e.name.clone(), g.color))
                    .collect();
                if !groups.is_empty() {
                    crate::props::sub_box(ui, "vis.grids", "Grid systems", None, true, |ui| {
                        ui.add_enabled_ui(!h.hides_category(Category::Grids), |ui| {
                            for (id, name, color) in groups {
                                crate::props::flow(ui, |ui| {
                                    swatch(ui, color);
                                    let mut shown = !h.grid_groups.contains(&id);
                                    let w = ui.available_width();
                                    if crate::props::tick(ui, &mut shown, &name, w).changed() {
                                        let label = if shown {
                                            "Show grid system"
                                        } else {
                                            "Hide grid system"
                                        };
                                        change = Some((
                                            label.into(),
                                            Box::new(move |h| h.set_grid_group(id, !shown)),
                                        ));
                                    }
                                });
                            }
                        });
                    });
                    let hidden: Vec<(ElementId, String)> = h
                        .elements
                        .iter()
                        .filter_map(|id| self.doc.model().get(*id))
                        .map(|e| (e.id, e.name.clone()))
                        .collect();
                    if !hidden.is_empty() {
                        let n = hidden.len();
                        crate::props::sub_box(
                            ui,
                            "vis.hidden",
                            "Hidden elements",
                            Some(n),
                            true,
                            |ui| {
                                for (id, name) in hidden {
                                    crate::props::flow(ui, |ui| {
                                        ui.label(name);
                                        if ui.small_button("Unhide").clicked() {
                                            change = Some((
                                                "Show hidden element".into(),
                                                Box::new(move |h| h.set_element(id, false)),
                                            ));
                                        }
                                    });
                                }
                            },
                        );
                    }
                }
                ui.add_space(3.0);
                if crate::props::row(ui, "", |ui| {
                    ui.add_enabled(!h.is_empty(), egui::Button::new("Show everything"))
                        .on_hover_text(
                            "Right-click an element in a view to hide it or its category there.",
                        )
                        .clicked()
                }) {
                    change = Some((
                        "Show everything".into(),
                        Box::new(|h| *h = ViewHidden::default()),
                    ));
                }
            });
        });
        if let Some((label, f)) = change {
            self.set_view_hidden(t, &label, f);
        }
        if let Some(id) = reveal {
            self.toggle_hidden_elements(id);
        }
        if let Some(id) = colour_of {
            self.toggle_hidden_colour(id);
        }
        if let Some(id) = xray_of {
            self.toggle_view_xray(id);
        }
        if let Some(g) = ground_3d {
            self.set_ground_3d(g);
        }
        if let Some(on) = envelope_3d {
            let s = &mut self.shell.settings;
            s.envelope_3d_off = !on;
            s.store();
            self.geometry_key = None;
        }
        if let Some(on) = lot_lines_3d {
            let s = &mut self.shell.settings;
            s.lot_lines_3d_off = !on;
            s.store();
            self.geometry_key = None;
        }
        if let Some(v) = ground_3d_was {
            let s = &mut self.shell.settings;
            s.ground_3d_was = v;
            s.store();
            self.geometry_key = None;
        }
        match extra {
            Some("guides.toggle") => self.set_guides_hidden(!self.shell.settings.guides_hidden),
            Some("xray") => self.set_xray(!self.shell.settings.xray),
            Some("hidden.colour3d") => {
                self.set_hidden_colour_3d(!self.shell.settings.hidden_colour_3d)
            }
            Some("ground.grid") => {
                let s = &mut self.shell.settings;
                s.ground_grid = !s.ground_grid;
                s.store();
                self.geometry_key = None;
            }
            _ => {}
        }
    }

    /// A grid system's settings when it is selected: under Settings, its
    /// groups boxed (the Grid tool's tab shows them as sections, MU68).
    pub(super) fn grid_group_properties(
        &mut self,
        ui: &mut egui::Ui,
        id: ElementId,
        g: &mut GridGroup,
        in_tool: bool,
    ) -> &'static str {
        crate::props::info(
            ui,
            "A grid system: its lines are labelled on their own, with the prefix in front.",
            "",
        );
        let a = grid_group_head(ui, g);
        let b = self.grid_group_rest(ui, id, g, in_tool);
        if b.is_empty() {
            a
        } else {
            b
        }
    }

    /// A group of a grid system's settings: a full section in the Grid
    /// tool's tab, where it is top level; a box under Settings.
    fn grid_section(
        &mut self,
        ui: &mut egui::Ui,
        in_tool: bool,
        key: &str,
        title: &str,
        open: bool,
        add: impl FnOnce(&mut egui::Ui),
    ) {
        if in_tool {
            let open = self.prop_header(ui, key, title, open, Some("grids"));
            crate::props::fold(ui, open, add);
        } else {
            crate::props::sub_box(ui, key, title, None, open, add);
        }
    }

    /// Numbering, new lines, laying out, and its lines.
    fn grid_group_rest(
        &mut self,
        ui: &mut egui::Ui,
        id: ElementId,
        g: &mut GridGroup,
        in_tool: bool,
    ) -> &'static str {
        let mut label = "";
        let curved = g.shape == Shape::Curved;
        // set once: folded at first (REV01)
        self.grid_section(ui, in_tool, "gridnumbering", "Numbering", false, |ui| {
            let style = |ui: &mut egui::Ui, text: &str, s: &mut LabelStyle| {
                let mut changed = false;
                crate::app::helpui::combo(ui, text)
                    .selected_text(s.label())
                    .show_ui(ui, |ui| {
                        for o in LabelStyle::ALL {
                            changed |= ui.selectable_value(s, o, o.label()).changed();
                        }
                    });
                changed
            };
            let (a, b) = if curved {
                ("Spokes", "Rings")
            } else {
                ("Across", "Along")
            };
            if style(ui, a, &mut g.vertical) | style(ui, b, &mut g.horizontal) {
                label = "Grid label style";
            }
            // a third set, for lines that run neither way (decision 3)
            if !curved {
                if style(ui, "Diagonal", &mut g.diagonal) {
                    label = "Grid label style";
                }
                crate::props::row(ui, "Mark", |ui| {
                    if crate::props::text_field(
                        ui,
                        egui::TextEdit::singleline(&mut g.diagonal_mark),
                        f32::INFINITY,
                    )
                    .on_hover_text("After a diagonal's label: A', B'…")
                    .changed()
                    {
                        label = "Grid label style";
                    }
                });
            }
        });
        // what a new line gets (decision 2)
        self.grid_section(ui, in_tool, "gridnewlines", "New lines", false, |ui| {
            // four choices don't fit a segmented row: a list (MU66 look)
            let text = |b: Bubbles| match b {
                Bubbles::Start => "At the start",
                Bubbles::End => "At the end",
                Bubbles::Both => "At both ends",
                Bubbles::None => "None",
            };
            let mut pick = None;
            crate::app::helpui::combo(ui, "Bubbles")
                .selected_text(text(g.bubbles))
                .show_ui(ui, |ui| {
                    for b in Bubbles::ALL {
                        if ui.selectable_label(g.bubbles == b, text(b)).clicked() {
                            pick = Some(b);
                        }
                    }
                });
            if let Some(b) = pick.filter(|b| *b != g.bubbles) {
                g.bubbles = b;
                label = "Grid bubbles for new lines";
            }
        });
        // a whole grid from the engineer's spacing (decision 5)
        let mut add = false;
        self.grid_section(ui, in_tool, "gridlayout", "Lay out lines", false, |ui| {
            let (a, b) = if curved {
                ("Spokes", "Rings")
            } else {
                ("Across", "Along")
            };
            if spacing_row(ui, id, a, &mut g.spacing_across, curved) {
                label = "Grid spacing";
            }
            if spacing_row(ui, id, b, &mut g.spacing_along, false) {
                label = "Grid spacing";
            }
            crate::props::row(ui, "Run past", |ui| {
                if crate::props::fill(ui, crate::units::drag(&mut g.overhang).range(0.0..=50.0))
                    .on_hover_text("How far the lines run past the last line crossing them")
                    .changed()
                {
                    label = "Grid spacing";
                }
            });
            let some = !g.spacing_across.is_empty() || !g.spacing_along.is_empty();
            // on the rows' own edges (REV01, decision 4)
            if full_button(ui, some, "Add lines")
                .on_hover_text(
                    "Lines at the spacing above, from the origin; lines already there are kept",
                )
                .clicked()
            {
                add = true;
            }
        });
        if add {
            let tx = grids::lay_out(self.doc.model(), id, g);
            let n = tx.ops.len();
            if n == 0 {
                self.status = "Those lines are all there already.".into();
            } else {
                let r = self.doc.commit(tx);
                self.report(r, format!("Added {n} grid lines, one undo step."));
            }
        }
        let its_lines = |this: &mut Self, ui: &mut egui::Ui| {
            let n = grid_count(this.doc.model(), Some(id));
            crate::props::worked_out(
                ui,
                "grid-system",
                &[
                    crate::props::Worked::new("Lines", n.to_string()),
                    crate::props::Worked::new(
                        "Labels",
                        format!(
                            "{}{}, {}{}…",
                            g.prefix,
                            g.vertical.format(1),
                            g.prefix,
                            g.horizontal.format(1)
                        ),
                    ),
                ],
            );
            // selected, it offers to draw in it; in the tool, that is now
            if !in_tool {
                let active = this.active_grid_group() == Some(id) && this.plan.tool == Tool::Grid;
                if full_button(ui, !active, "Draw lines in it").clicked() {
                    this.plan.grid_group = Some(id);
                    this.plan.tool = Tool::Grid;
                    this.plan.cancel();
                    this.status = Tool::Grid.hint().into();
                }
            }
            let (sel, renum) = button_pair(ui, "Select lines", "Renumber");
            if sel {
                let ids = grid_ids(this.doc.model(), Some(id));
                this.selection.clear();
                this.selection.extend(ids, false);
            }
            if renum {
                let tx = grids::renumber(this.doc.model(), Some(id));
                if tx.ops.is_empty() {
                    this.status = "The lines are numbered in order already.".into();
                } else {
                    let r = this.doc.commit(tx);
                    this.report(
                        r,
                        "Relabelled each direction in order, in the system's own axes.",
                    );
                }
            }
            if !in_tool {
                crate::props::info(
                    ui,
                    "Deleting the system keeps its lines, ungrouped and with their labels.",
                    "",
                );
            }
        };
        if in_tool {
            let open = self.prop_header(ui, "gridlines", "Its lines", true, Some("grids"));
            if open {
                its_lines(self, ui);
            }
        } else {
            its_lines(self, ui);
        }
        label
    }

    /// The Grid tool's own tab (REV01, decision 1): the grid system new
    /// lines go in, picked or made here, and its settings, so nothing has
    /// to be found or selected first.
    pub(super) fn grid_tool_panel(&mut self, ui: &mut egui::Ui) {
        let model = self.doc.model();
        let groups: Vec<(ElementId, String)> = grids::groups(model)
            .into_iter()
            .map(|(e, _)| (e.id, e.name.clone()))
            .collect();
        let active = self.active_grid_group();
        let current = active
            .and_then(|g| model.get(g))
            .map_or("No grid system".to_string(), |e| e.name.clone());
        let before: Option<GridGroup> = active.and_then(|id| match &model.get(id)?.kind {
            ElementKind::GridGroup(g) => Some(g.clone()),
            _ => None,
        });
        let mut g = before.clone();
        let mut pick: Option<Option<ElementId>> = None;
        let mut new = false;
        let mut head = "";
        // the system and its own settings, a section as Graphics or View
        // range are (MU68), then its groups as sections of their own
        let open = self.prop_header(ui, "gridtool", "Grid system", true, Some("grids"));
        crate::props::fold(ui, open, |ui| {
            crate::props::row(ui, "System", |ui| {
                let h = ui.spacing().interact_size.y;
                // the + on the fields' right edge, the list filling the rest up
                // to it (MU68: the + sat off the edge, then short of it)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_sized([h, h], egui::Button::new("+"))
                        .on_hover_text("A new grid system: new lines go in it")
                        .clicked()
                    {
                        new = true;
                    }
                    let w = ui.available_width().floor().max(40.0);
                    let pad = ui.spacing().button_padding.x * 2.0 + ui.spacing().icon_width;
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_max_width(w);
                        egui::ComboBox::from_id_salt("gridtool.system")
                            .icon(crate::props::combo_icon)
                            .width((w - pad).max(20.0))
                            .truncate()
                            .selected_text(current.as_str())
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(active.is_none(), "No grid system")
                                    .clicked()
                                {
                                    pick = Some(None);
                                }
                                for (gid, name) in &groups {
                                    if ui.selectable_label(active == Some(*gid), name).clicked() {
                                        pick = Some(Some(*gid));
                                    }
                                }
                            })
                            // its whole name when cut short (MU69)
                            .response
                            .on_hover_text(current.as_str());
                    });
                });
            });
            match g.as_mut() {
                Some(g) => head = grid_group_head(ui, g),
                None => crate::props::plain_note(
                    ui,
                    "Lines drawn now are numbered on their own. + makes a grid system: its own prefix, numbering, turn and spacing.",
                ),
            }
        });
        let mut rest = "";
        if let (Some(id), Some(g)) = (active, g.as_mut()) {
            rest = self.grid_group_rest(ui, id, g, true);
        }
        if let (Some(id), Some(g), Some(before)) = (active, &g, &before) {
            let label = if rest.is_empty() { head } else { rest };
            if !label.is_empty() && g != before {
                let g = g.clone();
                let r = self.doc.edit_merged(id, label, |e| {
                    e.kind = ElementKind::GridGroup(g.clone());
                });
                if let Err(e) = r {
                    self.status = format!("Couldn't do that: {e}");
                }
            }
        }
        if let Some(to) = pick {
            self.plan.grid_group = to;
            let name = to
                .and_then(|g| self.doc.model().get(g))
                .map_or("no grid system".to_string(), |e| e.name.clone());
            self.status = format!("New grid lines go in {name}.");
        }
        if new {
            self.new_grid_group_here(false);
        }
    }

    /// The grid system part of a grid line's properties, then its own
    /// settings: bubbles, elbows, shape, levels, and its ends in this plan.
    pub(super) fn grid_properties(
        &mut self,
        ui: &mut egui::Ui,
        id: ElementId,
        g: &mut Grid,
    ) -> &'static str {
        let mut label = "";
        let model = self.doc.model();
        let groups: Vec<(ElementId, String)> = grids::groups(model)
            .into_iter()
            .map(|(e, _)| (e.id, e.name.clone()))
            .collect();
        let current = grids::group_of(model, g).map(|(e, _)| e.name.clone());
        let mut pick = None;
        crate::app::helpui::combo(ui, "System")
            .selected_text(current.unwrap_or_else(|| "None".into()))
            .show_ui(ui, |ui| {
                if ui.selectable_label(g.group.is_none(), "None").clicked() {
                    pick = Some(None);
                }
                for (gid, name) in &groups {
                    if ui.selectable_label(g.group == Some(*gid), name).clicked() {
                        pick = Some(Some(*gid));
                    }
                }
            });
        if let Some(to) = pick.filter(|to| *to != g.group) {
            // it takes the next label of its new group
            g.group = to;
            g.label = grids::next_label_for(model, g);
            label = "Move grid line to another system";
        }
        let scheme = grids::group_of(model, g)
            .map(|(_, s)| s.clone())
            .unwrap_or_default();
        let dir = scheme.direction(g);
        crate::props::read_rows(
            ui,
            &[
                crate::props::ReadRow::new(
                    "Shows as",
                    crate::props::Read::Text(grids::display_label(model, g)),
                    "Its system's prefix, then its label",
                ),
                crate::props::ReadRow::new(
                    "Runs",
                    crate::props::Read::Text(format!(
                        "{} · {}",
                        dir.label(),
                        scheme.style(dir).0.label()
                    )),
                    "Which way it runs in its system, which sets how it is counted. A line more than 20° off both directions is a diagonal, counted on its own",
                ),
            ],
        );
        // which ends have a bubble, and how far each is moved aside
        let (mut s, mut e) = (g.bubbles.at_start(), g.bubbles.at_end());
        if crate::props::check(ui, "Bubbles", &mut s, "At its start").changed() {
            g.bubbles = g.bubbles.with(false, s);
            label = "Grid bubbles";
        }
        if crate::props::check(ui, "", &mut e, "At its end").changed() {
            g.bubbles = g.bubbles.with(true, e);
            label = "Grid bubbles";
        }
        // a box with one full-width row per bubble, as In this plan (MU66:
        // two fields in one row grew while typed in and pushed the panel)
        let ends: Vec<(usize, &str)> = [(0, s, "Start"), (1, e, "End")]
            .into_iter()
            .filter(|(_, on, _)| *on)
            .map(|(k, _, t)| (k, t))
            .collect();
        if !ends.is_empty() {
            crate::props::sub_box(ui, "grid.elbows", "Elbows", None, true, |ui| {
                for (k, name) in &ends {
                    crate::props::row(ui, name, |ui| {
                        if crate::props::fill(
                            ui,
                            crate::units::drag(&mut g.elbows[*k]).range(-20.0..=20.0),
                        )
                        .on_hover_text(
                            "How far the bubble steps aside on a jog, to the left as the line runs out; 0 keeps it on the line. Or drag the bubble's square grip sideways",
                        )
                        .changed()
                        {
                            label = "Grid bubble elbow";
                        }
                    });
                }
            });
        }
        // a straight line or an arc
        let mut arc = g.bulge != 0.0;
        if crate::props::choice(
            ui,
            "Shape",
            &mut arc,
            &[
                (false, "Line", "A straight line"),
                (true, "Arc", "An arc through its two ends"),
            ],
        ) {
            g.bulge = if arc { 0.2 } else { 0.0 };
            label = "Grid line shape";
        }
        if arc {
            // how far its middle stands off the straight line
            let half = g.start.distance(g.end) / 2.0;
            let mut rise = g.bulge * half;
            crate::props::row(ui, "Rise", |ui| {
                if crate::props::fill(ui, crate::units::drag(&mut rise).range(-half..=half))
                    .on_hover_text("How far its middle stands off the straight line between its ends; negative bows the other way")
                    .changed()
                    && half > 1e-6
                {
                    g.bulge = (rise / half).clamp(-1.0, 1.0);
                    if g.bulge == 0.0 {
                        g.bulge = 1e-3;
                    }
                    label = "Grid line shape";
                }
            });
        }
        // the levels it shows on (decision 6)
        let levels: Vec<(ElementId, String)> = model
            .levels()
            .map(|(e, _)| (e.id, e.name.clone()))
            .collect();
        let mut some = !g.levels.is_empty();
        if crate::props::choice(
            ui,
            "Levels",
            &mut some,
            &[
                (false, "All", "It shows on every level"),
                (true, "Some", "It shows on the levels ticked"),
            ],
        ) {
            g.levels = if some {
                // start from the level being worked on
                self.work_level()
                    .map(|(l, _)| vec![l])
                    .or_else(|| levels.first().map(|(l, _)| vec![*l]))
                    .unwrap_or_default()
            } else {
                vec![]
            };
            label = "Grid line levels";
        }
        if some {
            for (i, (lid, name)) in levels.iter().enumerate() {
                let mut on = g.levels.contains(lid);
                if crate::props::check(ui, if i == 0 { "On" } else { "" }, &mut on, name.as_str())
                    .changed()
                {
                    if on {
                        g.levels.push(*lid);
                    } else {
                        g.levels.retain(|l| l != lid);
                    }
                    label = "Grid line levels";
                }
            }
        }
        // shorter in the plan in front only
        let plan = self
            .active_view
            .filter(|v| matches!(self.view(*v).map(|v| &v.kind), Some(ViewKind::Plan { .. })));
        if let Some(v) = plan {
            let mut t = self
                .view(v)
                .and_then(|view| view.grid_ends.get(&id).copied())
                .unwrap_or([0.0, 0.0]);
            let before = t;
            let mut reset = false;
            crate::props::sub_box(ui, "grid.inplan", "In this plan", None, true, |ui| {
                for (name, k) in [("Start in", 0), ("End in", 1)] {
                    crate::props::row(ui, name, |ui| {
                        crate::props::fill(ui, crate::units::drag(&mut t[k]).range(0.0..=1e4))
                            .on_hover_text(
                                "How much shorter it is drawn at this end, in this plan only",
                            );
                    });
                }
                if t != [0.0, 0.0]
                    && ui
                        .add(
                            egui::Button::new("Draw it whole here")
                                .min_size(egui::vec2(ui.available_width() - 6.0, 24.0)),
                        )
                        .clicked()
                {
                    reset = true;
                }
            });
            if reset {
                t = [0.0, 0.0];
            }
            if t != before {
                let r = self.doc.edit_merged(v, "Grid ends in this plan", |e| {
                    if let ElementKind::View(view) = &mut e.kind {
                        if t == [0.0, 0.0] {
                            view.grid_ends.remove(&id);
                        } else {
                            view.grid_ends.insert(id, t);
                        }
                    }
                });
                if let Err(e) = r {
                    self.status = format!("Couldn't do that: {e}");
                }
            }
        }
        label
    }

    // ---- 3D ------------------------------------------------------------

    /// Grid lines the 3D view shows, with their colour.
    fn grids_3d(&self) -> Vec<(ElementId, Grid, [f32; 3])> {
        if self.vis3d.hides_category(Category::Grids) {
            return vec![];
        }
        self.grids_3d_all()
    }

    /// Every grid line the 3D view could draw, the Grids switch aside.
    fn grids_3d_all(&self) -> Vec<(ElementId, Grid, [f32; 3])> {
        let model = self.doc.model();
        let hidden = self.hidden_in_3d();
        model
            .iter()
            .filter(|e| !hidden.contains(&e.id) && !self.hidden.contains(&e.id))
            .filter_map(|e| match &e.kind {
                ElementKind::Grid(g) => {
                    // the plans' colour (MU67): Auto is the grid ink
                    // (Auto reads on the view's background: lighter on a
                    // dark one, MU70)
                    let auto = if self.paper_is_dark {
                        Color32::from_rgb(142, 166, 200)
                    } else {
                        crate::planview::GRID_INK
                    };
                    let c = crate::planview::grid_colour(model, g)
                        .unwrap_or(auto)
                        .to_array();
                    let rgb = [
                        c[0] as f32 / 255.0,
                        c[1] as f32 / 255.0,
                        c[2] as f32 / 255.0,
                    ];
                    Some((e.id, g.clone(), rgb))
                }
                _ => None,
            })
            .collect()
    }

    /// The heights grid lines span in 3D: the lowest level to the highest
    /// (at least one storey), and every level between.
    fn grid_heights_3d(&self) -> Vec<f64> {
        let mut zs: Vec<f64> = self
            .doc
            .model()
            .levels()
            .map(|(_, l)| l.elevation)
            .collect();
        zs.sort_by(f64::total_cmp);
        zs.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        let lo = zs.first().copied().unwrap_or(0.0);
        let hi = zs.last().copied().unwrap_or(0.0);
        if hi - lo < 0.5 {
            let storey = self
                .doc
                .model()
                .levels()
                .map(|(_, l)| l.height)
                .fold(0.0, f64::max);
            zs = vec![lo, lo + if storey > 0.0 { storey } else { 3.0 }];
        }
        zs
    }

    /// Grid lines on or off in the 3D view (MU42).
    pub(super) fn toggle_grids_3d(&mut self) {
        // one switch with the 3D view's Grids category (MU69: two switches
        // had to agree, so ticking Grids alone showed nothing)
        let on = self.vis3d.hides_category(Category::Grids);
        self.vis3d.set_category(Category::Grids, !on);
        self.hidden_rev += 1;
        self.snaps_3d = None;
        let s = &mut self.shell.settings;
        s.grids_3d = on;
        s.hidden_3d = self.vis3d.categories.clone();
        s.store();
        self.geometry_key = None;
        self.status = if on {
            "Grid lines in 3D: flat on the working level. Alt+G hides them."
        } else {
            "Grid lines hidden in 3D; a selected one still shows. Alt+G shows them all."
        }
        .into();
    }

    /// The grid lines the 3D view draws now: all of them when they are
    /// on, else only the selected ones (MU42).
    fn shown_grids_3d(&self) -> Vec<(ElementId, Grid, [f32; 3])> {
        let on = !self.vis3d.hides_category(Category::Grids);
        // only those shown on the working level (decision 6)
        let level = self.work_level().map(|(l, _)| l);
        self.grids_3d_all()
            .into_iter()
            .filter(|(id, ..)| on || self.selection.contains(*id))
            .filter(|(_, g, _)| level.is_none_or(|l| grids::on_level(g, l)))
            .collect()
    }

    /// Where grid lines lie in 3D: flat on the working level (MU42: they
    /// stood up the whole height before, through roofs and floors).
    fn grid_z_3d(&self) -> f64 {
        self.work_level()
            .map_or_else(|| self.grid_heights_3d()[0], |(_, z)| z)
    }

    /// Depth-tested lines for the grid lines in 3D: each along its line on
    /// the working level, faint, out to its bubbles; a selected one in the
    /// selection colour. The model draws over them where it is in front.
    pub(super) fn grid_lines_3d(&self) -> Vec<Vertex> {
        self.grid_lines_3d_where(false)
    }

    /// The selected grid lines, for the selection's pass drawn through
    /// everything (MU72: depth-tested, the ground hid most of it).
    pub(super) fn selected_grid_lines_3d(&self) -> Vec<Vertex> {
        // drawn bold over the view by draw_grid_bubbles instead (MU74: a
        // one-pixel line in 3D against the plans' bold one)
        vec![]
    }

    /// What the 3D grid lines are built from: the grids, their colours,
    /// the working level and the background, as one number.
    pub(super) fn grid_lines_3d_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for (id, g, c) in self.shown_grids_3d() {
            id.hash(&mut h);
            format!("{g:?}{c:?}").hash(&mut h);
        }
        self.grid_z_3d().to_bits().hash(&mut h);
        self.paper_is_dark.hash(&mut h);
        h.finish()
    }

    /// A grid line drawn over the 3D view as the plans draw a lit one:
    /// bold and dashed, out to each bubble's ring.
    fn paint_grid_3d(
        &self,
        painter: &egui::Painter,
        project: &dyn Fn(DVec3) -> Option<egui::Pos2>,
        g: &Grid,
        colour: Color32,
    ) {
        let z = self.grid_z_3d() + 0.03;
        let stroke = egui::Stroke::new(2.0, colour);
        let line: Vec<egui::Pos2> = grids::points(g)
            .iter()
            .filter_map(|q| project(q.extend(z)))
            .collect();
        if line.len() > 1 {
            painter.extend(egui::Shape::dashed_line(&line, stroke, 12.0, 4.0));
        }
        let r = BUBBLE_R_3D
            * creabim_geom::plan::bubble_scale(&grids::display_label(self.doc.model(), g)) as f32;
        for (end, on) in [(false, g.bubbles.at_start()), (true, g.bubbles.at_end())] {
            if !on {
                continue;
            }
            let (at, leader) = grids::bubble(g, end, BUBBLE_OUT_3D);
            let mut pts: Vec<egui::Pos2> = leader
                .iter()
                .chain(std::iter::once(&at))
                .filter_map(|q| project(q.extend(z)))
                .collect();
            // up to the ring, not into the label
            if let [.., a, b] = pts.as_slice() {
                let (a, b) = (*a, *b);
                let d = b - a;
                if d.length() > r {
                    let n = pts.len();
                    pts[n - 1] = b - d.normalized() * r;
                } else {
                    pts.pop();
                }
            }
            if pts.len() > 1 {
                painter.extend(egui::Shape::dashed_line(&pts, stroke, 12.0, 4.0));
            }
        }
    }

    fn grid_lines_3d_where(&self, selected: bool) -> Vec<Vertex> {
        // clear of the working level: at 5 mm they fought the ground and
        // the ground grid for depth and broke up (MU72)
        let z = self.grid_z_3d() + 0.03;
        let mut out = Vec::new();
        for (id, g, color) in self.shown_grids_3d() {
            if self.selection.contains(id) != selected {
                continue;
            }
            // their own colour (MU70: half way to the background they
            // vanished on a dark one)
            // selected: the selection's colour, as in the drawings (MU72:
            // a fixed orange ignored the theme's)
            let c = if self.selection.contains(id) {
                let a = crate::planview::accent_selected();
                [
                    a.r() as f32 / 255.0,
                    a.g() as f32 / 255.0,
                    a.b() as f32 / 255.0,
                ]
            } else {
                color
            };
            // the line (an arc in pieces), and out to each bubble it has
            let mut runs = vec![grids::points(&g)];
            for (end, on) in [(false, g.bubbles.at_start()), (true, g.bubbles.at_end())] {
                if on {
                    let (at, mut leader) = grids::bubble(&g, end, BUBBLE_OUT_3D);
                    leader.push(at);
                    runs.push(leader);
                }
            }
            // dashed as the plans draw it (MU74: a long dash and a dot
            // here, a plain dash there), three of dash to one of gap, the
            // pattern running on along the whole line
            const PATTERN: [(f64, bool); 2] = [(0.75, true), (0.25, false)];
            for run in runs {
                let mut k = 0usize;
                let mut left = PATTERN[0].0;
                for w in run.windows(2) {
                    let (a, b) = (w[0], w[1]);
                    let len = a.distance(b);
                    let dir = (b - a) / len.max(1e-9);
                    let mut at = 0.0;
                    while at < len - 1e-9 {
                        let step = left.min(len - at);
                        if PATTERN[k].1 {
                            let p0 = a + dir * at;
                            let p1 = a + dir * (at + step);
                            out.push(line_vertex(p0.extend(z).as_vec3().to_array(), c));
                            out.push(line_vertex(p1.extend(z).as_vec3().to_array(), c));
                        }
                        at += step;
                        left -= step;
                        if left <= 1e-9 {
                            k = (k + 1) % PATTERN.len();
                            left = PATTERN[k].0;
                        }
                    }
                }
            }
        }
        out
    }

    /// Where a grid line's bubbles are in 3D: at its ends, on the
    /// working level.
    fn bubbles_3d(&self) -> Vec<(ElementId, String, [f32; 3], DVec3)> {
        let z = self.grid_z_3d();
        let model = self.doc.model();
        let mut out = vec![];
        for (id, g, color) in self.shown_grids_3d() {
            let text = grids::display_label(model, &g);
            for (end, on) in [(false, g.bubbles.at_start()), (true, g.bubbles.at_end())] {
                if on {
                    let (at, _) = grids::bubble(&g, end, BUBBLE_OUT_3D);
                    out.push((id, text.clone(), color, at.extend(z)));
                }
            }
        }
        out
    }

    /// The grid bubbles over the 3D view: a circle with the label, facing
    /// the viewer, at both ends of each line.
    pub(super) fn draw_grid_bubbles(
        &self,
        painter: &egui::Painter,
        project: &dyn Fn(DVec3) -> Option<egui::Pos2>,
    ) {
        // the grid line under the pointer, lit along its length (MU72: a
        // 3D highlight tints solids only, so a line showed nothing)
        let lit = painter.ctx().pointer_hover_pos().and_then(|p| {
            self.plan
                .hover
                .as_ref()
                .and_then(|h| h.at(crate::hoverpick::Pane::Window3d, p))
        });
        if let Some((_, g, _)) = lit
            .filter(|id| !self.selection.contains(*id))
            .and_then(|id| {
                self.shown_grids_3d()
                    .into_iter()
                    .find(|(gid, ..)| *gid == id)
            })
        {
            // the pointing colour, as the drawings' (blue), not the
            // selection's
            self.paint_grid_3d(painter, project, &g, crate::hoverpick::HOVER);
        }
        // selected: bold in the selection's colour, as the plans and the
        // section marks show it (MU74)
        for (id, g, _) in self.shown_grids_3d() {
            if self.selection.contains(id) {
                self.paint_grid_3d(painter, project, &g, crate::planview::accent_selected());
            }
        }
        for (id, text, color, at) in self.bubbles_3d() {
            let Some(c) = project(at) else { continue };
            if !painter.clip_rect().expand(20.0).contains(c) {
                continue;
            }
            let selected = self.selection.contains(id);
            let pointed = lit == Some(id) && !selected;
            let ring = if selected {
                crate::planview::accent_selected()
            } else if pointed {
                crate::hoverpick::HOVER
            } else {
                color32(color)
            };
            let r = BUBBLE_R_3D * creabim_geom::plan::bubble_scale(&text) as f32;
            // filled with the view's own background, so it follows the
            // theme (MU70: always white)
            let bg = if self.paper_is_dark {
                BACKGROUND_3D_DARK
            } else {
                BACKGROUND_3D
            };
            let fill = Color32::from_rgba_unmultiplied(
                (bg[0] * 255.0) as u8,
                (bg[1] * 255.0) as u8,
                (bg[2] * 255.0) as u8,
                235,
            );
            painter.circle(
                c,
                r,
                fill,
                egui::Stroke::new(if selected || pointed { 2.5 } else { 1.5 }, ring),
            );
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                &text,
                egui::FontId::proportional(12.0),
                ring,
            );
        }
    }

    /// The grid line under `p` in the 3D view (a bubble, or its line at a
    /// level), with the point on it under the pointer, for picking and
    /// dragging it.
    /// The grid line within a few pixels of `p` when it lies in front of
    /// what the ray hits (`hit_t`), or nothing is hit: the ground or a floor
    /// under it no longer takes the click (MU71).
    pub(super) fn grid_line_in_front(
        &self,
        p: egui::Pos2,
        project: &dyn Fn(DVec3) -> Option<egui::Pos2>,
        o: DVec3,
        d: DVec3,
        hit_t: Option<f32>,
    ) -> Option<ElementId> {
        let z = self.grid_z_3d();
        let mut best: Option<(f32, ElementId)> = None;
        for (id, g, _) in self.shown_grids_3d() {
            for w in grids::points(&g).windows(2) {
                let (a, b) = (w[0].extend(z), w[1].extend(z));
                let (Some(sa), Some(sb)) = (project(a), project(b)) else {
                    continue;
                };
                let ab = sb - sa;
                let t = ((p - sa).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
                let px = (sa + ab * t).distance(p);
                if px >= 6.0 {
                    continue;
                }
                // the point seen under the pointer (MU137: not as far along
                // the line as on the screen, which parts in perspective)
                let at = crate::snaps::point_seen_at(a, b, sa + ab * t, project);
                // in the ray's own measure, as the hit's
                let len2 = d.length_squared().max(1e-12);
                let along = ((at - o).dot(d) / len2) as f32;
                let tol = (0.05 / len2.sqrt()) as f32;
                let in_front = hit_t.is_none_or(|h| along <= h + tol);
                if in_front && best.is_none_or(|(bp, _)| px < bp) {
                    best = Some((px, id));
                }
            }
        }
        best.map(|b| b.1)
    }

    pub(super) fn grid_line_near(
        &self,
        p: egui::Pos2,
        project: &dyn Fn(DVec3) -> Option<egui::Pos2>,
    ) -> Option<(ElementId, DVec3)> {
        self.grid_hit_3d(p, project, None)
    }

    /// Where the pointer holds grid line `id` in the 3D view, if it is on
    /// that line or its bubble (other grids crossing there don't count).
    pub(super) fn grid_grab(
        &self,
        p: egui::Pos2,
        id: ElementId,
        project: &dyn Fn(DVec3) -> Option<egui::Pos2>,
    ) -> Option<DVec3> {
        self.grid_hit_3d(p, project, Some(id)).map(|h| h.1)
    }

    fn grid_hit_3d(
        &self,
        p: egui::Pos2,
        project: &dyn Fn(DVec3) -> Option<egui::Pos2>,
        only: Option<ElementId>,
    ) -> Option<(ElementId, DVec3)> {
        let zs = self.grid_heights_3d();
        let hi = zs[zs.len() - 1];
        // a bubble is the deliberate target: it wins over lines passing by
        let bubble = self
            .bubbles_3d()
            .into_iter()
            .filter(|(id, ..)| only.is_none_or(|o| o == *id))
            .filter_map(|(id, text, _, at)| {
                let c = project(at)?;
                let r = BUBBLE_R_3D * creabim_geom::plan::bubble_scale(&text) as f32;
                let d = c.distance(p);
                (d <= r + 2.0).then_some((id, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id);
        if let Some(id) = bubble {
            let g = match self.doc.model().get(id).map(|e| &e.kind) {
                Some(ElementKind::Grid(g)) => g.clone(),
                _ => return None,
            };
            // the bubble end nearest the pointer
            let near = [g.start, g.end]
                .into_iter()
                .filter_map(|e| Some((e, project(e.extend(hi))?.distance(p))))
                .min_by(|a, b| a.1.total_cmp(&b.1))?;
            return Some((id, near.0.extend(hi)));
        }
        let mut best: Option<(f32, ElementId, DVec3)> = None;
        for (id, g, _) in self.grids_3d() {
            if only.is_some_and(|o| o != id) {
                continue;
            }
            // each piece of it: an arc is many
            let pts = grids::points(&g);
            for z in &zs {
                for w in pts.windows(2) {
                    let (a, b) = (w[0].extend(*z), w[1].extend(*z));
                    let (Some(sa), Some(sb)) = (project(a), project(b)) else {
                        continue;
                    };
                    let ab = sb - sa;
                    let t = ((p - sa).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
                    let d = (sa + ab * t).distance(p);
                    if d < 6.0 && best.is_none_or(|(bd, _, _)| d < bd) {
                        let at = crate::snaps::point_seen_at(a, b, sa + ab * t, project);
                        best = Some((d, id, at));
                    }
                }
            }
        }
        best.map(|(_, id, at)| (id, at))
    }

    /// The grid line whose bubble is under `p` in the 3D view.
    pub(super) fn grid_bubble_near(
        &self,
        p: egui::Pos2,
        project: &dyn Fn(DVec3) -> Option<egui::Pos2>,
    ) -> Option<ElementId> {
        self.bubbles_3d()
            .into_iter()
            .filter_map(|(id, text, _, at)| {
                let c = project(at)?;
                let r = BUBBLE_R_3D * creabim_geom::plan::bubble_scale(&text) as f32;
                let d = c.distance(p);
                (d <= r + 2.0).then_some((id, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }
}

/// How many grid lines are in `group` (`None`: ungrouped).
fn grid_count(model: &Model, group: Option<ElementId>) -> usize {
    grid_ids(model, group).len()
}

/// Every guide in the model.
pub(super) fn guide_ids(model: &Model) -> Vec<ElementId> {
    model
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Guide(_)))
        .map(|e| e.id)
        .collect()
}

fn grid_ids(model: &Model, group: Option<ElementId>) -> Vec<ElementId> {
    model
        .iter()
        .filter(|e| matches!(&e.kind, ElementKind::Grid(g) if g.group == group))
        .map(|e| e.id)
        .collect()
}

/// A small square of colour.
fn swatch(ui: &mut egui::Ui, c: [f32; 3]) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), Sense::hover());
    ui.painter().rect_filled(r, 2.0, color32(c));
}

/// Elements that have a body to hide (not views, styles, levels...).
pub(super) fn shows_in_3d(kind: &ElementKind) -> bool {
    !matches!(
        kind,
        ElementKind::View(_)
            | ElementKind::Sheet(_)
            | ElementKind::Viewport(_)
            | ElementKind::Level(_)
            | ElementKind::Material(_)
            | ElementKind::Assembly(_)
            | ElementKind::TextStyle(_)
            | ElementKind::StylePack(_)
            | ElementKind::Units(_)
            | ElementKind::Project(_)
            | ElementKind::ElementType(_)
    )
}

/// A spacing list as shown: each step in the project's lengths (degrees
/// for a curved system's spokes), runs of the same step as "3 × 4000".
fn spacing_text(v: &[f64], degrees: bool) -> String {
    let one = |x: f64| {
        if degrees {
            format!("{}°", (x * 100.0).round() / 100.0)
        } else {
            crate::units::len(x)
        }
    };
    let mut parts: Vec<String> = vec![];
    let mut i = 0;
    while i < v.len() {
        let mut n = 1;
        while i + n < v.len() && (v[i + n] - v[i]).abs() < 1e-9 {
            n += 1;
        }
        parts.push(if n > 1 {
            format!("{n} × {}", one(v[i]))
        } else {
            one(v[i])
        });
        i += n;
    }
    parts.join(", ")
}

/// A spacing row of a grid system's Lay out lines: what is typed is kept
/// while it is edited and read on every change; true when the list
/// changed.
fn spacing_row(
    ui: &mut egui::Ui,
    group: ElementId,
    name: &str,
    v: &mut Vec<f64>,
    degrees: bool,
) -> bool {
    let key = egui::Id::new(("grid.spacing", group, name));
    let shown = spacing_text(v, degrees);
    let mut text = ui
        .data(|d| d.get_temp::<String>(key))
        .unwrap_or_else(|| shown.clone());
    let mut changed = false;
    crate::props::row(ui, name, |ui| {
        let hint = if degrees {
            "30°, 30°, 30°"
        } else {
            "4000, 3400, 5000"
        };
        let r = crate::props::text_field(
            ui,
            egui::TextEdit::singleline(&mut text).hint_text(hint),
            f32::INFINITY,
        )
        .on_hover_text(if degrees {
            "The angle from each spoke to the next, anticlockwise from east: 30, 30, 30 or 3 x 30"
        } else {
            "The distance from each line to the next: 4000, 3400, 5000 or 3 x 4000"
        });
        if r.changed() {
            let read = |s: &str| -> Option<f64> {
                if degrees {
                    s.trim().trim_end_matches('°').trim().parse().ok()
                } else {
                    crate::units::parse(s)
                }
            };
            if let Some(p) = grids::parse_spacing(&text, read) {
                *v = p;
                changed = true;
            }
        }
        if !r.has_focus() {
            text = spacing_text(v, degrees);
        }
    });
    ui.data_mut(|d| d.insert_temp(key, text));
    changed
}

/// A button on the rows' own edges, at the house height (REV01, decision 4).
pub(crate) fn full_button(ui: &mut egui::Ui, enabled: bool, text: &str) -> egui::Response {
    let w = ui.available_width().floor();
    ui.add_enabled(
        enabled,
        egui::Button::new(text).min_size(egui::vec2(w, 24.0)),
    )
}

/// Two buttons sharing the rows' edges evenly; which was clicked.
pub(crate) fn button_pair(ui: &mut egui::Ui, a: &str, b: &str) -> (bool, bool) {
    let gap = ui.spacing().item_spacing.x;
    let w = ((ui.available_width() - gap) / 2.0).floor();
    ui.horizontal(|ui| {
        let x = ui.add_sized([w, 24.0], egui::Button::new(a)).clicked();
        let y = ui.add_sized([w, 24.0], egui::Button::new(b)).clicked();
        (x, y)
    })
    .inner
}

/// A grid system's own settings, first: prefix, shape, turn, origin,
/// colour (no app state needed, so the Grid tool's section can hold them).
fn grid_group_head(ui: &mut egui::Ui, g: &mut GridGroup) -> &'static str {
    let mut label = "";
    crate::props::row(ui, "Prefix", |ui| {
        if crate::props::text_field(ui, egui::TextEdit::singleline(&mut g.prefix), f32::INFINITY)
            .changed()
        {
            label = "Grid prefix";
        }
    });
    // straight in two directions, or rings and spokes (decision 4)
    if crate::props::choice(
        ui,
        "Shape",
        &mut g.shape,
        &[
            (
                Shape::Straight,
                "Lines",
                "Straight lines across and along, and diagonals",
            ),
            (
                Shape::Curved,
                "Rings",
                "Rings round its origin, and spokes from it",
            ),
        ],
    ) {
        label = "Grid system shape";
    }
    crate::props::row(ui, "Turn", |ui| {
        if crate::props::fill(
            ui,
            egui::DragValue::new(&mut g.turn)
                .speed(0.5)
                .range(-180.0..=180.0)
                .suffix("°"),
        )
        .on_hover_text(
            "Its own north, anticlockwise: a turned building counts in its own directions",
        )
        .changed()
        {
            label = "Grid system turn";
        }
    });
    let origin_tip = if g.shape == Shape::Curved {
        "Its centre: rings go round it, spokes come out of it"
    } else {
        "Where Add lines lays out its first lines"
    };
    for (name, v) in [("Origin X", &mut g.origin.x), ("Origin Y", &mut g.origin.y)] {
        crate::props::row(ui, name, |ui| {
            if crate::props::fill(ui, crate::units::drag(v).range(-1e5..=1e5))
                .on_hover_text(origin_tip)
                .changed()
            {
                label = "Grid system origin";
            }
        });
    }
    // where its colour comes from (MU67), used in plans and 3D
    // a list: three segments were cut to "…" at the narrow panel
    let name = |c: GridColour| match c {
        GridColour::Auto => "Auto",
        GridColour::Annotation => "Annotation",
        GridColour::Custom => "Own colour",
    };
    let mut pick = None;
    crate::app::helpui::combo(ui, "Colour")
        .selected_text(name(g.colour_mode))
        .show_ui(ui, |ui| {
            for (c, tip) in [
                (
                    GridColour::Auto,
                    "A grid colour that reads on the theme and on dark or light paper",
                ),
                (
                    GridColour::Annotation,
                    "The annotation colour, so grids sit with dimensions and tags",
                ),
                (
                    GridColour::Custom,
                    "Its own colour, to tell one building's grid from another's",
                ),
            ] {
                if ui
                    .selectable_label(g.colour_mode == c, name(c))
                    .on_hover_text(tip)
                    .clicked()
                {
                    pick = Some(c);
                }
            }
        });
    if let Some(c) = pick.filter(|c| *c != g.colour_mode) {
        g.colour_mode = c;
        label = "Grid colour";
    }
    if g.colour_mode == GridColour::Custom {
        crate::props::row(ui, "", |ui| {
            // the picker works in 8-bit sRGB, as the colour is stored
            let mut c = g.color.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
            if crate::props::colour_button_srgb(ui, &mut c).changed() {
                g.color = c.map(|v| v as f32 / 255.0);
                label = "Grid colour";
            }
        });
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The app round the sample house, with a guide added.
    fn app() -> CreaBimApp {
        let mut app = CreaBimApp::blank(None);
        app.load_model(sample_house());
        let guide: creabim_core::Guide = serde_json::from_value(json!({
            "level": null, "a": [0.0, -2.0, 0.0], "b": [4.0, -2.0, 0.0],
        }))
        .unwrap();
        let e = Element::new("Guide", ElementKind::Guide(guide));
        app.doc.commit(Transaction::new("Guide").insert(e)).unwrap();
        app
    }

    fn checked(items: &[Item], id: &str) -> Option<bool> {
        items.iter().find_map(|i| match i {
            Item::Action { id: i, checked, .. } if i == id => *checked,
            _ => None,
        })
    }

    #[test]
    fn the_3d_views_quick_switches_hide_grids_annotation_and_guides() {
        let mut app = app();
        let ctx = egui::Context::default();
        let model = app.doc.model().clone();
        let grids: Vec<ElementId> = model
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::Grid(_)))
            .map(|e| e.id)
            .collect();
        assert!(!grids.is_empty());
        let items = app.visibility_items(VisTarget::Window3d);
        assert_eq!(checked(&items, "vis3.quick:grids"), Some(true));
        app.run_action(&ctx, "vis3.quick:grids");
        assert!(grids.iter().all(|g| app.hidden_in_3d().contains(g)));
        // kept for the next session
        assert!(app.shell.settings.hidden_3d.contains(&Category::Grids));
        let items = app.visibility_items(VisTarget::Window3d);
        assert_eq!(checked(&items, "vis3.quick:grids"), Some(false));
        assert_eq!(checked(&items, "vis3.quick:all"), Some(false));
        // annotation goes as a group
        app.run_action(&ctx, "vis3.quick:annotation");
        for c in [Category::Dimensions, Category::Tags, Category::Notes] {
            assert!(app.vis3d.hides_category(c));
        }
        // everything back, ground grid included
        app.run_action(&ctx, "vis3.quick:all");
        assert!(app.vis3d.categories.is_empty());
        app.run_action(&ctx, "vis3.quick:all");
        assert!(app.vis3d.hides_category(Category::Guides));
        assert!(!app.shell.settings.ground_grid);
        // the 3D view's Visibility is on its navigation bar
        let nav = app.nav_state();
        assert_eq!(nav["menus"][0]["key"], "vis3d");
        assert!(nav["buttons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["id"] == "xray"));
    }

    #[test]
    fn guides_hide_everywhere_and_come_back() {
        let mut app = app();
        let ctx = egui::Context::default();
        let guides = guide_ids(app.doc.model());
        assert_eq!(guides.len(), 1);
        app.run_action(&ctx, "guides.toggle");
        assert!(app.shell.settings.guides_hidden);
        assert!(app.hidden_in_3d().contains(&guides[0]));
        // still in the model
        assert!(app.doc.model().get(guides[0]).is_some());
        // picking the Guide tool brings them back
        let i = Tool::ALL.iter().position(|t| *t == Tool::Guide).unwrap();
        app.run_action(&ctx, &format!("tool:{i}"));
        assert!(!app.shell.settings.guides_hidden);
        assert!(!app.hidden_in_3d().contains(&guides[0]));
    }

    #[test]
    fn xray_and_hidden_elements_switch_on_and_off() {
        let mut app = app();
        let ctx = egui::Context::default();
        app.run_action(&ctx, "xray");
        assert!(app.shell.settings.xray);
        app.run_action(&ctx, "xray");
        assert!(!app.shell.settings.xray);
        let plan = app
            .doc
            .model()
            .iter()
            .find(|e| matches!(&e.kind, ElementKind::View(v) if matches!(v.kind, ViewKind::Plan { .. })))
            .unwrap()
            .id;
        app.active_view = Some(plan);
        app.run_action(&ctx, "view.reveal");
        assert!(app.shows_hidden_elements(plan));
        // one undo step
        app.run_command(&ctx, crate::commands::Command::Undo, None);
        assert!(!app.shows_hidden_elements(plan));
        let items = app.visibility_items(VisTarget::View(plan));
        assert_eq!(
            checked(&items, &format!("vis.reveal:{}", plan.0)),
            Some(false)
        );
    }

    #[test]
    fn a_custom_ground_grid_spacing_is_typed_in_any_unit() {
        let mut app = app();
        crate::units::set(creabim_core::Units::metric());
        assert_eq!(app.set_ground_spacing_text("1250"), Ok(1.25));
        assert_eq!(app.shell.settings.ground_grid_spacing, 1.25);
        assert!((app.set_ground_spacing_text("4'").unwrap() - 1.2192).abs() < 1e-9);
        assert!(app.set_ground_spacing_text("nonsense").is_err());
        assert!(app.set_ground_spacing_text("1 mm").is_err());
        // the menu shows it as the custom one
        let view = app.view_menu();
        let ground = view
            .iter()
            .find_map(|i| match i {
                Item::Sub { label, items, .. } if label == "Ground grid" => Some(items.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(checked(&ground, "ground.custom"), Some(true));
    }

    #[test]
    fn roofs_columns_and_beams_take_tags() {
        let model = creabim_core::sample::layered_house();
        for e in model.iter() {
            let member = creabim_core::members::is_member(&e.kind);
            if member || matches!(e.kind, ElementKind::Wall(_) | ElementKind::Slab(_)) {
                assert!(crate::planview::is_taggable(&model, e.id), "{}", e.name);
            }
            if matches!(e.kind, ElementKind::View(_) | ElementKind::Level(_)) {
                assert!(!crate::planview::is_taggable(&model, e.id));
            }
        }
    }

    /// Every icon a menu names, for the check below.
    fn icons(items: &[Item], out: &mut Vec<(String, &'static str)>) {
        for i in items {
            match i {
                Item::Action { label, icon, .. } => out.push((label.clone(), icon)),
                Item::Sub {
                    label, icon, items, ..
                } => {
                    out.push((label.clone(), icon));
                    icons(items, out);
                }
                _ => {}
            }
        }
    }

    #[test]
    fn every_menu_entry_has_an_icon_that_exists() {
        let app = app();
        let known = include_str!("../ui/icons.js");
        for key in [
            "menu:File",
            "menu:Edit",
            "menu:View",
            "menu:Create",
            "menu:Materials",
            "menu:Help",
        ] {
            let items = app.menu_items(key);
            // entries on the menu itself, and the submenus they open; check
            // marks stand in for the icon of on/off entries
            for i in &items {
                match i {
                    Item::Action {
                        label,
                        icon,
                        checked: None,
                        ..
                    } => assert!(!icon.is_empty(), "{key}: {label} has no icon"),
                    Item::Sub { label, icon, .. } => {
                        assert!(!icon.is_empty(), "{key}: {label} has no icon")
                    }
                    _ => {}
                }
            }
            let mut all = vec![];
            icons(&items, &mut all);
            for (label, icon) in all.into_iter().filter(|(_, i)| !i.is_empty()) {
                assert!(
                    known.contains(&format!("\"{icon}\":")),
                    "{key}: {label} names {icon}, which icons.js lacks"
                );
            }
        }
        // the Create menu's schedules too (under Views and levels since
        // the tool families, REV04)
        fn find_sub(items: &[Item], name: &str) -> Option<Vec<Item>> {
            items.iter().find_map(|i| match i {
                Item::Sub { label, items, .. } if label == name => Some(items.clone()),
                Item::Sub { items, .. } => find_sub(items, name),
                _ => None,
            })
        }
        let create = app.menu_items("menu:Create");
        let schedule = find_sub(&create, "Schedule").unwrap();
        assert!(schedule
            .iter()
            .all(|i| matches!(i, Item::Action { icon, .. } if !icon.is_empty())));
    }

    #[test]
    fn x_ray_and_colours_work_per_drawing_view_and_alt_z_follows_the_pointer() {
        let mut app = CreaBimApp::blank(None);
        let (model, beam) = creabim_core::sample::sample_with_buried_beam();
        app.load_model(model);
        let ctx = egui::Context::default();
        let sec = app
            .doc
            .model()
            .iter()
            .find(|e| e.name == "Section A")
            .unwrap()
            .id;
        app.active_view = Some(sec);
        let is_dashed = |i: &PlanItem| matches!(i, PlanItem::Line { id, pen: creabim_geom::plan::Pen::Hidden, .. } if *id == beam);
        assert!(!app.drawing(sec).unwrap().items.iter().any(is_dashed));
        // X-ray is a working mode: no edit, nothing to undo
        let rev = app.doc.model().revision();
        app.run_action(&ctx, "view.xray");
        assert!(app.plan.xray.contains(&sec));
        assert_eq!(app.doc.model().revision(), rev);
        assert!(app.drawing(sec).unwrap().items.iter().any(is_dashed));
        // sheets draw the view as it is
        let printed = creabim_geom::section::view_drawing(app.doc.model(), sec).unwrap();
        assert!(!printed.items.iter().any(is_dashed));
        let items = app.visibility_items(VisTarget::View(sec));
        assert_eq!(checked(&items, &format!("vis.xray:{}", sec.0)), Some(true));
        app.run_action(&ctx, &format!("vis.xray:{}", sec.0));
        assert!(!app.plan.xray.contains(&sec));

        // Alt+Z: in the 2D pane when the pointer was last over it
        app.rect_2d = Some(Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(100.0, 100.0),
        ));
        app.last_in_2d = true;
        assert_eq!(app.xray_view_target(), Some(sec));
        app.last_in_2d = false;
        assert_eq!(app.xray_view_target(), None);

        // colours: shows hidden elements too, one undo step
        app.run_action(&ctx, "view.colour");
        assert!(app.shows_hidden_elements(sec) && app.shows_hidden_colour(sec));
        assert!(app
            .drawing(sec)
            .unwrap()
            .items
            .iter()
            .any(|i| matches!(i, PlanItem::Tint { id, .. } if *id == beam)));
        app.run_command(&ctx, crate::commands::Command::Undo, None);
        assert!(!app.shows_hidden_elements(sec) && !app.shows_hidden_colour(sec));

        // in 3D
        app.run_action(&ctx, "hidden.colour3d");
        assert!(app.shell.settings.hidden_colour_3d);
        app.scene = (*creabim_geom::shared_scene(app.doc.model())).clone();
        let buried = app.buried_vertices();
        assert!(!buried.is_empty() && buried.len().is_multiple_of(3));
        app.run_action(&ctx, "hidden.colour3d");
        assert!(!app.shell.settings.hidden_colour_3d);
    }

    #[test]
    fn in_3d_x_ray_a_click_takes_the_beam_inside_the_wall() {
        let mut app = CreaBimApp::blank(None);
        let (model, beam) = creabim_core::sample::sample_with_buried_beam();
        app.load_model(model);
        app.scene = (*creabim_geom::shared_scene(app.doc.model())).clone();
        let wall = app
            .doc
            .model()
            .iter()
            .find(|e| e.name == "GF Wall 1")
            .unwrap()
            .id;
        // from the south, through the wall at the beam's height
        let (o, d) = (glam::DVec3::new(2.5, -5.0, 2.45), glam::DVec3::Y);
        assert_eq!(app.first_hit_3d(o, d).map(|h| h.id), Some(wall));
        app.shell.settings.xray = true;
        assert_eq!(app.first_hit_3d(o, d).map(|h| h.id), Some(beam));
    }
}
