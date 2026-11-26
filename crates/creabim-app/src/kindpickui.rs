//! The door and window picker on screen (Mockup_Door_Window_Picker_REV00):
//! the tool strip's kind button, the picker's page, the kind block at the
//! top of a door's or window's Properties, and the browser's types group.
//! The kinds and their pictures are in `kindpick.rs`.

use super::*;
use crate::kindpick::{Pick, DOORS, WINDOWS};
use creabim_core::types::{self, TypeParams};
use creabim_core::{OpeningKind, TypeCategory};
use serde_json::json;

/// Who a pick goes to: the next door or window placed, or the selection.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Next,
    Selection,
}

fn cat_of(kind: OpeningKind) -> Option<TypeCategory> {
    match kind {
        OpeningKind::Door => Some(TypeCategory::Door),
        OpeningKind::Window => Some(TypeCategory::Window),
        OpeningKind::Hole => None,
    }
}

impl CreaBimApp {
    /// The kind the Door or Window tool places next: its type's, when one
    /// is picked, else the one picked in the picker, else the usual.
    pub(super) fn next_kind(&self, kind: OpeningKind) -> Pick {
        let model = self.doc.model();
        if let Some(t) = cat_of(kind)
            .and_then(|c| self.plan.types.get(&c))
            .and_then(|id| model.get(*id))
        {
            if let ElementKind::ElementType(t) = &t.kind {
                if let TypeParams::Door(o) | TypeParams::Window(o) = &t.params {
                    if let Some(p) = Pick::of_frame(kind, &o.frame) {
                        return p;
                    }
                }
            }
        }
        let own = match kind {
            OpeningKind::Door => self.plan.members.door_kind,
            _ => self.plan.members.window_kind,
        };
        own.unwrap_or_else(|| {
            Pick::of_frame(kind, &creabim_core::OpeningFrame::for_kind(kind)).unwrap_or(
                if kind == OpeningKind::Door {
                    Pick::Door(creabim_core::Operation::Standard)
                } else {
                    Pick::Window(creabim_core::cells::WindowStyle::Fixed)
                },
            )
        })
    }

    /// The tool strip's kind button, first for the Door and Window tools.
    pub(super) fn kind_control(&self) -> Option<serde_json::Value> {
        let kind = match self.plan.tool {
            Tool::Door => OpeningKind::Door,
            Tool::Window => OpeningKind::Window,
            _ => return None,
        };
        let p = self.next_kind(kind);
        let what = if kind == OpeningKind::Door {
            "door"
        } else {
            "window"
        };
        Some(json!({
            "kind": "menu", "key": format!("kinds:{what}:next"), "label": "", "value": p.label(), "svg": p.svg(),
            "hint": format!("The kind of {what} you place next. Click to choose another, or one of your types."),
        }))
    }

    /// The picker's page: its kinds by family, and the project's types.
    pub(super) fn kinds_popup_json(&self, key: &str) -> serde_json::Value {
        let mut parts = key.split(':').skip(1);
        let door = parts.next() == Some("door");
        let target = parts.next().unwrap_or("next");
        let kind = if door {
            OpeningKind::Door
        } else {
            OpeningKind::Window
        };
        let current = match target {
            "sel" => self.selection.all().into_iter().find_map(|id| {
                match self.doc.model().get(id).map(|e| &e.kind) {
                    Some(ElementKind::Opening(o)) if o.kind == kind => Pick::of(o),
                    _ => None,
                }
            }),
            _ => Some(self.next_kind(kind)),
        };
        let tile = |p: Pick| json!({"id": p.id(), "label": p.label(), "svg": p.svg(), "on": Some(p) == current});
        let families: Vec<serde_json::Value> = if door {
            DOORS.iter().map(|(n, ops)| json!({"name": n, "items": ops.iter().map(|o| tile(Pick::Door(*o))).collect::<Vec<_>>()})).collect()
        } else {
            WINDOWS.iter().map(|(n, ss)| json!({"name": n, "items": ss.iter().map(|s| tile(Pick::Window(*s))).collect::<Vec<_>>()})).collect()
        };
        let model = self.doc.model();
        let cat = cat_of(kind);
        let types: Vec<serde_json::Value> = types::types(model)
            .into_iter()
            .filter(|(_, _, t)| Some(t.category()) == cat)
            .filter_map(|(id, name, t)| {
                let (TypeParams::Door(o) | TypeParams::Window(o)) = &t.params else { return None };
                let p = Pick::of_frame(kind, &o.frame)?;
                let n = types::instances(model, id).len();
                Some(json!({
                    "id": format!("t:{}", id.0), "label": name, "svg": p.svg(),
                    "line": format!("{} · {n} placed", p.label()),
                    "on": target == "next" && cat.and_then(|c| self.plan.types.get(&c)) == Some(&id),
                }))
            })
            .collect();
        json!({
            "kind": if door { "door" } else { "window" },
            "target": target,
            "families": families,
            "types": types,
            "current": current.map(|p| if p.family() == p.label() { p.label().to_string() } else { format!("{} · {}", p.label(), p.family()) }),
        })
    }

    /// A pick from the picker: "kind.pick" with "next:d:Pivot",
    /// "sel:w:Casement" or "next:t:<type id>".
    pub(super) fn kinds_action(&mut self, head: &str, arg: &str) -> bool {
        if head != "kind.pick" {
            return false;
        }
        let (target, id) = arg.split_once(':').unwrap_or(("next", arg));
        let target = if target == "sel" {
            Target::Selection
        } else {
            Target::Next
        };
        if let Some(t) = id.strip_prefix("t:") {
            let Some((tid, cat)) = self
                .doc
                .model()
                .iter()
                .find(|e| e.id.0.to_string() == t)
                .and_then(|e| match &e.kind {
                    ElementKind::ElementType(ty) => Some((e.id, ty.category())),
                    _ => None,
                })
            else {
                return true;
            };
            match target {
                Target::Next => {
                    self.set_tool_type(cat, Some(tid));
                }
                Target::Selection => {
                    let model = self.doc.model();
                    let mut tx = Transaction::new("Door or window type");
                    for sid in self.selection.all() {
                        let Some(e) = model.get(sid) else { continue };
                        if TypeCategory::of(&e.kind) != Some(cat) {
                            continue;
                        }
                        let mut after = e.clone();
                        types::stamp(model, &mut after, tid);
                        tx = tx.replace(e.clone(), after);
                    }
                    let r = self.doc.commit(tx);
                    self.report(r, "The selection takes that type now.");
                }
            }
            return true;
        }
        let Some(p) = Pick::parse(id) else {
            return true;
        };
        match target {
            Target::Next => {
                match p {
                    Pick::Door(_) => self.plan.members.door_kind = Some(p),
                    Pick::Window(_) => self.plan.members.window_kind = Some(p),
                }
                // a kind of its own: no type
                if let Some(c) = cat_of(p.kind()) {
                    self.set_tool_type(c, None);
                }
                self.plan.tool = if p.kind() == OpeningKind::Door {
                    Tool::Door
                } else {
                    Tool::Window
                };
                self.status = format!(
                    "{}: click a wall to place it. The button in the bar changes it.",
                    p.label()
                );
            }
            Target::Selection => {
                let model = self.doc.model();
                let mut tx = Transaction::new(format!("Make it {}", p.label().to_lowercase()));
                for sid in self.selection.all() {
                    let Some(e) = model.get(sid) else { continue };
                    let ElementKind::Opening(o) = &e.kind else {
                        continue;
                    };
                    if o.kind != p.kind() || o.roof_at.is_some() {
                        continue;
                    }
                    let mut after = e.clone();
                    if let ElementKind::Opening(o) = &mut after.kind {
                        p.apply_frame(model, &mut o.frame);
                    }
                    tx = tx.replace(e.clone(), after);
                }
                let r = self.doc.commit(tx);
                self.report(
                    r,
                    format!("Now {}. Ctrl+Z to undo.", p.label().to_lowercase()),
                );
            }
        }
        true
    }

    /// The browser's group of door and window types, with how many of
    /// each are placed.
    pub(super) fn kinds_group(&self) -> serde_json::Value {
        let model = self.doc.model();
        let rows: Vec<serde_json::Value> = types::types(model)
            .into_iter()
            .filter(|(_, _, t)| matches!(t.category(), TypeCategory::Door | TypeCategory::Window))
            .map(|(id, name, t)| {
                let kind = if t.category() == TypeCategory::Door { OpeningKind::Door } else { OpeningKind::Window };
                let what = match &t.params {
                    TypeParams::Door(o) | TypeParams::Window(o) => Pick::of_frame(kind, &o.frame).map_or("custom", |p| p.label()),
                    _ => "",
                };
                let n = types::instances(model, id).len();
                json!({
                    "id": format!("kindtype:{}", id.0), "name": format!("{name} · {} · {n}", what.to_lowercase()),
                    "icon": if kind == OpeningKind::Door { "door" } else { "window" },
                    "hint": format!("{name}: {what}, {n} placed. Click to see the type."),
                })
            })
            .collect();
        json!({
            "key": "kindtypes", "title": "Opening types", "icon": "door", "open": !rows.is_empty(),
            "note": if rows.is_empty() { "none yet".to_string() } else { String::new() },
            "rows": rows,
            "acts": [
                {"id": "kinds.create:door", "icon": "door", "hint": "Place a door: choose its kind first"},
                {"id": "kinds.create:window", "icon": "window", "hint": "Place a window: choose its kind first"},
            ],
        })
    }
}
