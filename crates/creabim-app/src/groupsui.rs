//! The browser's Groups section (Options from IFC REV00, owner: A on all
//! five): the project's groups, kept from IFC, each with its count; a
//! click selects what it holds, the eye hides and shows it, and groups
//! that look like design options are offered as options, which is only
//! done when asked. Ctrl+click picks groups to make options of by hand.

use super::*;
use creabim_core::groups;
use creabim_core::phases;
use serde_json::json;

/// What the Groups section remembers for the session.
#[derive(Default)]
pub(super) struct GroupsUi {
    /// The offer was answered "Keep as groups".
    pub dismissed: bool,
    /// Groups picked with Ctrl+click, to make options of.
    pub picked: Vec<u32>,
    /// Groups hidden from the views with their eye.
    pub hidden: Vec<u32>,
}

fn count(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

impl CreaBimApp {
    /// The browser's Groups section; none when the project has no groups.
    pub(super) fn groups_group(&self) -> Option<serde_json::Value> {
        let model = self.doc.model();
        let all = groups::groups(model);
        if all.is_empty() {
            return None;
        }
        let ui = &self.groups_ui;
        let rows: Vec<serde_json::Value> = all
            .iter()
            .map(|g| {
                let n = groups::members(model, g.id).len();
                let option = g.option.and_then(|o| phases::option(model, o)).map(|(q, o)| format!("{} › {}", q.name, o.name));
                let what = match (&option, g.is_search()) {
                    (Some(o), _) => format!("Made into the option {o}"),
                    (None, true) => "A Bonsai saved search: the members it found when the file was saved".to_string(),
                    (None, false) => "A group from the IFC file".to_string(),
                };
                json!({
                    "id": format!("group:{}", g.id),
                    "name": format!("{} · {}{}", g.name, count(n), if g.is_search() { " · search" } else { "" }),
                    "icon": if option.is_some() { "split" } else if g.is_search() { "search" } else { "layers-3" },
                    "sel": ui.picked.contains(&g.id),
                    "eye": !ui.hidden.contains(&g.id),
                    "eyeAct": format!("groups.eye:{}", g.id),
                    "hint": format!("{}: {what}. Click to select its {} pieces; Ctrl+click to pick it for Make them options.", g.name, count(n)),
                })
            })
            .collect();
        let offer = if ui.picked.len() >= 2 {
            let ids = ui
                .picked
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",");
            Some(json!({
                "text": format!("{} groups picked.", ui.picked.len()),
                "acts": [
                    {"id": format!("groups.make:{ids}"), "label": "Make them options", "primary": true},
                    {"id": "groups.unpick", "label": "Clear"},
                ],
            }))
        } else if !ui.dismissed {
            groups::option_candidates(model).first().map(|set| {
                let names: Vec<String> = set
                    .iter()
                    .filter_map(|id| all.iter().find(|g| g.id == *id))
                    .map(|g| g.name.clone())
                    .collect();
                let ids = set.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
                json!({
                    "text": format!("These {} look like options of one design: {}. They share most of their pieces.", names.len(), names.join(", ")),
                    "acts": [
                        {"id": format!("groups.make:{ids}"), "label": "Make them options", "primary": true},
                        {"id": "groups.keep", "label": "Keep as groups"},
                    ],
                })
            })
        } else {
            None
        };
        Some(json!({
            "key": "groups", "title": "Groups", "icon": "layers-3", "open": offer.is_some(),
            "rows": rows, "offer": offer,
        }))
    }

    /// Properties › Phase: the groups `id` is in, and, for a copy made
    /// for an option, the piece it stands for.
    pub(super) fn groups_rows(&self, ui: &mut egui::Ui, id: ElementId) {
        let model = self.doc.model();
        if groups::groups(model).is_empty() {
            return;
        }
        let names = groups::of(model, id);
        if !names.is_empty() {
            crate::props::row(ui, "Groups", |ui| {
                ui.add(egui::Label::new(names.join(", ")).truncate())
                    .on_hover_text(format!(
                        "In the groups {}: they go back to IFC with it.",
                        names.join(", ")
                    ));
            });
        }
        if let Some(c) = groups::copies(model).iter().find(|c| c.copy == id) {
            let of = model.get(c.of).map_or_else(
                || "a piece now deleted".to_string(),
                |e| format!("'{}'", e.name),
            );
            let same = groups::stands_for(model, id) == c.of;
            let text = if same {
                format!(
                    "A copy of {of} for this option. Unchanged, it goes back to IFC as that piece."
                )
            } else {
                format!("A copy of {of} for this option, changed here: it goes back to IFC as a piece of its own.")
            };
            ui.add(egui::Label::new(egui::RichText::new(text).small().weak()).wrap());
        }
    }

    /// A click on a group's row: select what it holds, or (`add`, Ctrl)
    /// pick it for Make them options.
    pub(super) fn pick_group(&mut self, id: u32, add: bool) {
        if add {
            let p = &mut self.groups_ui.picked;
            match p.iter().position(|x| *x == id) {
                Some(i) => {
                    p.remove(i);
                }
                None => p.push(id),
            }
            return;
        }
        let model = self.doc.model();
        let Some(g) = groups::groups(model).iter().find(|g| g.id == id) else {
            return;
        };
        let name = g.name.clone();
        let ids = groups::members(model, id);
        self.selection.clear();
        for m in &ids {
            self.selection.toggle(*m);
        }
        self.face = None;
        self.status = format!("Selected the {} pieces of {name}.", count(ids.len()));
    }

    /// Run a Groups action. False when the action isn't one.
    pub(super) fn groups_action(&mut self, head: &str, arg: &str) -> bool {
        match head {
            "groups.keep" => {
                self.groups_ui.dismissed = true;
                self.status = "Kept as groups. Ctrl+click two or more in the browser to make them options whenever you like.".into();
            }
            "groups.unpick" => self.groups_ui.picked.clear(),
            "groups.eye" => {
                let Ok(id) = arg.parse::<u32>() else {
                    return true;
                };
                let ids = groups::members(self.doc.model(), id);
                let with = creabim_core::transform::copy_set(self.doc.model(), &ids);
                let h = &mut self.groups_ui.hidden;
                if let Some(i) = h.iter().position(|x| *x == id) {
                    h.remove(i);
                    for e in with {
                        self.hidden.remove(&e);
                    }
                } else {
                    h.push(id);
                    self.hidden.extend(with);
                }
                self.hidden_rev += 1;
            }
            "groups.make" => {
                let ids: Vec<u32> = arg.split(',').filter_map(|s| s.parse().ok()).collect();
                self.make_options_from(&ids);
            }
            _ => return false,
        }
        true
    }

    /// Make groups `ids` into a question, one option per group, starting
    /// the project's phases first if it has none (what is drawn becomes
    /// Proposed).
    pub(super) fn make_options_from(&mut self, ids: &[u32]) {
        if phases::list(self.doc.model()).is_empty() {
            if let Some(tx) = phases::start(self.doc.model(), false) {
                let r = self.doc.commit(tx);
                self.report(r, "Phases started, for the options.");
            }
        }
        let Some((tx, made)) = groups::make_options(self.doc.model(), ids, "") else {
            self.status =
                "Those groups can't be made options: pick two or more groups not yet options."
                    .into();
            return;
        };
        let r = self.doc.commit(tx);
        self.groups_ui.picked.clear();
        self.groups_ui.dismissed = true;
        let model = self.doc.model();
        let q = phases::questions(model)
            .iter()
            .find(|q| q.id == made.question);
        let (qname, names) = q
            .map(|q| {
                (
                    q.name.clone(),
                    q.options
                        .iter()
                        .map(|o| o.name.clone())
                        .collect::<Vec<_>>()
                        .join(", "),
                )
            })
            .unwrap_or_default();
        self.hidden_rev += 1;
        let copies = if made.copies > 0 {
            format!(
                " {} pieces in some options but not all were copied into each ({} copies); unchanged, they go back to IFC as one piece.",
                count(made.copies),
                count(made.copies)
            )
        } else {
            String::new()
        };
        self.report(
            r,
            format!(
                "Made the question {qname}: {names}. {} pieces are shared by all, {} are in one option alone.{copies} Pick an option from the tag beside a view's phase.",
                count(made.shared),
                count(made.alone)
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::groups::GroupDef;
    use creabim_core::Transaction;

    fn with_groups(app: &mut CreaBimApp) -> Vec<ElementId> {
        let m = app.doc.model();
        let walls: Vec<ElementId> = m
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::Wall(_)))
            .map(|e| e.id)
            .take(3)
            .collect();
        let g = |id: u32, name: &str, k: &[usize]| GroupDef {
            id,
            name: name.into(),
            members: k.iter().map(|i| walls[*i]).collect(),
            ifc: None,
            option: None,
        };
        let tx = match m.project_info() {
            Some((pid, _)) => {
                let before = m.get(pid).unwrap().clone();
                let mut after = before.clone();
                if let ElementKind::Project(p) = &mut after.kind {
                    p.groups = vec![
                        g(1, "1_A", &[0, 1]),
                        g(2, "1_B", &[0, 2]),
                        g(3, "Framing", &[1]),
                    ];
                }
                Transaction::new("g").replace(before, after)
            }
            None => {
                let info = creabim_core::ProjectInfo {
                    groups: vec![
                        g(1, "1_A", &[0, 1]),
                        g(2, "1_B", &[0, 2]),
                        g(3, "Framing", &[1]),
                    ],
                    ..Default::default()
                };
                Transaction::new("g").insert(Element::new("Project", ElementKind::Project(info)))
            }
        };
        app.doc.commit(tx).unwrap();
        walls
    }

    #[test]
    fn the_browser_offers_groups_that_look_like_options_and_makes_them_when_asked() {
        let mut app = test_app();
        let walls = with_groups(&mut app);
        let g = app.groups_group().expect("a Groups section");
        assert_eq!(g["rows"].as_array().unwrap().len(), 3);
        let offer = &g["offer"];
        assert!(
            offer["text"].as_str().unwrap().contains("1_A, 1_B"),
            "{offer}"
        );
        assert_eq!(offer["acts"][0]["id"], "groups.make:1,2");
        // nothing is made until asked
        assert!(phases::questions(app.doc.model()).is_empty());
        // a click selects a group's pieces
        app.pick_group(1, false);
        assert_eq!(app.selection.all().len(), 2);
        // asked: options, the phases started for them
        assert!(app.groups_action("groups.make", "1,2"));
        let m = app.doc.model();
        let q = &phases::questions(m)[0];
        assert_eq!(
            q.options
                .iter()
                .map(|o| o.name.as_str())
                .collect::<Vec<_>>(),
            ["A", "B"]
        );
        assert_eq!(
            m.get(walls[0]).unwrap().phase.option,
            None,
            "in both: shared"
        );
        assert_eq!(m.get(walls[1]).unwrap().phase.option, Some(q.options[0].id));
        assert_eq!(m.get(walls[2]).unwrap().phase.option, Some(q.options[1].id));
        assert!(
            app.groups_group().unwrap()["offer"].is_null(),
            "the offer is answered"
        );
    }

    #[test]
    fn keep_as_groups_ends_the_offer_and_ctrl_click_picks_groups_by_hand() {
        let mut app = test_app();
        with_groups(&mut app);
        app.groups_action("groups.keep", "");
        assert!(app.groups_group().unwrap()["offer"].is_null());
        app.pick_group(1, true);
        app.pick_group(3, true);
        let g = app.groups_group().unwrap();
        assert_eq!(g["offer"]["acts"][0]["id"], "groups.make:1,3");
        assert_eq!(g["rows"][0]["sel"], true);
    }

    #[test]
    fn a_groups_eye_hides_and_shows_what_it_holds() {
        let mut app = test_app();
        let walls = with_groups(&mut app);
        app.groups_action("groups.eye", "3");
        assert!(app.hidden.contains(&walls[1]));
        assert_eq!(app.groups_group().unwrap()["rows"][2]["eye"], false);
        app.groups_action("groups.eye", "3");
        assert!(!app.hidden.contains(&walls[1]));
    }
}
