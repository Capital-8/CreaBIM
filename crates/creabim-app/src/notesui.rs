//! The one Notes area (Properties REV04, decision 9): the project's notes
//! as sections in a strip (General notes, Structural, Electrical,
//! Plumbing, the user's own, then Drafts and To-dos, kept apart because
//! they are the user's, not the drawing's). Each is Markdown with
//! `{fields}`; any can be placed on a sheet as a note that stays linked,
//! so the same section on five sheets is one text.

use super::*;
use crate::ui_model::Item;
use creabim_core::project::{self, NoteSection, ProjectInfo, DRAFTS, TODOS};
use serde_json::{json, Value};

/// Project time's work notes, shown as a section of their own (owner
/// 2026-11-18, Mockup_Notes_Hub_REV00): kept in the time book beside the
/// project, never in the file.
pub(super) const WORKLOG: &str = "worklog";

/// The heading levels and lists the toolbar starts a line with.
const LINE_STARTS: [(&str, &str, &str); 4] = [
    ("text", "## ", "Heading: capitals, underlined on paper"),
    ("align-left", "- ", "List"),
    ("hash", "1. ", "Numbered list"),
    ("check", "- [ ] ", "Checkbox: prints with its box"),
];

impl CreaBimApp {
    fn notes_info(&self) -> ProjectInfo {
        self.doc
            .model()
            .project_info()
            .map(|(_, p)| p.clone())
            .unwrap_or_default()
    }

    /// The project's notes, written back as one undo step per edit run.
    fn set_notes_info(&mut self, info: ProjectInfo) {
        let tx = creabim_core::set_project(self.doc.model(), info);
        if let Err(e) = self.doc.commit_merged(tx) {
            self.status = format!("Couldn't do that: {e}");
        }
    }

    /// Opens the section `key` where notes are written: an area showing
    /// Notes if there is one, else the Notes tab of Properties.
    pub(super) fn open_notes(&mut self, key: &str) {
        self.notes_section = key.to_string();
        let shown = self
            .areas
            .root
            .areas()
            .iter()
            .any(|s| matches!(s, crate::areas::Shows::Notes));
        if !shown && !self.notes_popped {
            self.prop_tab = PropTab::Notes;
        }
    }

    /// Notes actions from the browser, menus and the notes' buttons.
    pub(super) fn notes_action(&mut self, ctx: &egui::Context, head: &str, arg: &str) -> bool {
        match head {
            "notes.open" => self.open_notes(arg),
            "notes.new" => {
                let key = self.add_section();
                self.open_notes(&key);
            }
            // from the notes themselves: named at once, where it was asked
            "notes.add" => {
                let key = self.add_section();
                self.notes_section = key.clone();
                self.notes_renaming = Some((key, NotesAt::from_tag(arg)));
            }
            "notes.pick" => {
                self.notes_section = arg.to_string();
                self.notes_renaming = None;
            }
            "notes.rename" => {
                self.notes_renaming = Some((self.notes_section.clone(), NotesAt::from_tag(arg)));
            }
            "notes.delete" => {
                let key = self.notes_section.clone();
                self.delete_section(&key);
            }
            "notes.line" | "notes.field" => {
                let (at, what) = arg.split_once(':').unwrap_or(("area", arg));
                let insert = if head == "notes.line" {
                    let i: usize = what.parse().unwrap_or(0);
                    (
                        LINE_STARTS[i.min(LINE_STARTS.len() - 1)].1.to_string(),
                        true,
                    )
                } else {
                    (format!("{{{what}}}"), false)
                };
                self.notes_insert = Some((NotesAt::from_tag(at), insert.0, insert.1));
            }
            "notes.copy" => {
                let text =
                    project::linked_text(self.doc.model(), &self.notes_section).unwrap_or_default();
                ctx.copy_text(text);
                self.status = "Section copied, as Markdown.".into();
            }
            "notes.pop" => self.notes_popped = true,
            // an area showing the notes: the one there is, else a new one
            // beside the area in front (MU89)
            "notes.area" => {
                let shows = self.areas.root.areas();
                if let Some(i) = shows
                    .iter()
                    .position(|s| matches!(s, crate::areas::Shows::Notes))
                {
                    self.areas.focus = i;
                } else {
                    let i = self.areas.focus.min(shows.len().saturating_sub(1));
                    self.areas.focus = self.areas.root.divide(i, true, crate::areas::Shows::Notes);
                    self.areas.max = None;
                    self.store_areas();
                }
            }
            "notes.goto" => {
                let id = self
                    .doc
                    .model()
                    .iter()
                    .find(|e| e.id.0.to_string() == arg)
                    .map(|e| e.id);
                if let Some(id) = id {
                    self.notes_go_to(id);
                }
            }
            "notes.pickplaced" => {
                let ids: Vec<ElementId> = self.notes_ticked.iter().copied().collect();
                self.selection.extend(ids, false);
            }
            "notes.unplace" => {
                let model = self.doc.model();
                let mut tx = Transaction::new("Take notes off sheets");
                let mut n = 0;
                for id in &self.notes_ticked {
                    if let Some(e) = model.get(*id) {
                        tx = tx.remove_cascade(model, e.id);
                        n += 1;
                    }
                }
                let r = self.doc.commit(tx);
                self.notes_ticked.clear();
                self.report(
                    r,
                    format!("{n} block(s) taken off their sheets. The section is as it was."),
                );
            }
            "notes.preview" => {
                let at = NotesAt::from_tag(arg);
                if !self.notes_preview.remove(&at) {
                    self.notes_preview.insert(at);
                }
            }
            "notes.place" => {
                let key = self.notes_section.clone();
                self.place_notes(&key);
            }
            _ => return false,
        }
        true
    }

    /// A new section of the user's own, after the others; its key.
    fn add_section(&mut self) -> String {
        let mut info = self.notes_info();
        let key = info.new_section_key();
        let mut all = info.note_sections();
        let n = all.len() + 1;
        all.push(NoteSection::new(&key, &format!("Section {n}")));
        info.sections = all;
        self.set_notes_info(info);
        key
    }

    /// The sheet a section is placed on: the one in front, else the first
    /// open in an area.
    fn notes_target_sheet(&self) -> Option<ElementId> {
        if let Some(v) = self.active_view.filter(|v| self.is_sheet(*v)) {
            return Some(v);
        }
        self.areas.root.areas().into_iter().find_map(|s| match s {
            crate::areas::Shows::View(id) if self.is_sheet(id) => Some(id),
            _ => None,
        })
    }

    /// Places the section `key` on a sheet, at its top right, as a note
    /// that stays linked.
    fn place_notes(&mut self, key: &str) {
        let Some(sheet) = self.notes_target_sheet() else {
            self.status = "Open a sheet first: the section goes on the sheet in front.".into();
            return;
        };
        let model = self.doc.model();
        let Some(ElementKind::Sheet(s)) = model.get(sheet).map(|e| &e.kind) else {
            return;
        };
        let width = 100.0_f64.min(s.width * 0.3);
        let at = glam::DVec2::new(s.width - width - 25.0, s.height - 22.0);
        let mut n = creabim_core::Note::linked(sheet, at, key, width);
        // "General notes", else a plain style made for notes, else the
        // default: never a bold title style such as "Room name" (MU88)
        let plain = |t: &creabim_core::TextStyle| {
            t.purpose == creabim_core::StyleUse::Notes && !t.bold && t.size <= 2.6
        };
        n.style = model
            .iter()
            .find(|e| e.name == "General notes" && matches!(e.kind, ElementKind::TextStyle(_)))
            .or_else(|| {
                model
                    .iter()
                    .find(|e| matches!(&e.kind, ElementKind::TextStyle(t) if plain(t)))
            })
            .map(|e| e.id);
        let name = self
            .notes_info()
            .section_name(key)
            .unwrap_or_else(|| "Notes".into());
        let number = sheets::sheet_of(model, sheet)
            .map(|s| s.number.clone())
            .unwrap_or_default();
        let e = Element::new(&name, ElementKind::Note(n));
        let id = e.id;
        let r = self
            .doc
            .commit(Transaction::new("Place notes on a sheet").insert(e));
        if r.is_ok() {
            self.selection.only(id);
            self.active_view = Some(sheet);
        }
        self.report(
            r,
            format!("{name} placed on {number}. It follows the section: edit it here and every sheet updates."),
        );
    }

    /// The section open, made sure to exist. A note picked on a sheet
    /// opens the section it shows, once.
    fn notes_current(&mut self) -> String {
        let picked =
            self.selection
                .active
                .and_then(|id| match self.doc.model().get(id).map(|e| &e.kind) {
                    Some(ElementKind::Note(n)) => n.section.clone().map(|k| (id, k)),
                    _ => None,
                });
        if let Some((id, key)) = &picked {
            if self.notes_synced != Some(*id) {
                self.notes_section = key.clone();
            }
        }
        self.notes_synced = picked.map(|(id, _)| id);
        let sections = self.notes_info().note_sections();
        let known = sections.iter().any(|s| s.key == self.notes_section)
            || self.notes_section == DRAFTS
            || self.notes_section == TODOS
            || self.notes_section == WORKLOG;
        if !known {
            self.notes_section = sections
                .first()
                .map_or(TODOS.to_string(), |s| s.key.clone());
        }
        self.notes_section.clone()
    }

    /// The open section's name as the switch shows it: To-dos with its
    /// count.
    pub(super) fn notes_title(&self) -> String {
        let info = self.notes_info();
        if self.notes_section == WORKLOG {
            return "Work log".into();
        }
        if self.notes_section == TODOS {
            let all = info.pad_items().iter().filter(|i| i.todo).count();
            let done = info.pad_items().iter().filter(|i| i.todo && i.done).count();
            return format!("To-dos {done}/{all}");
        }
        info.section_name(&self.notes_section)
            .unwrap_or_else(|| "Notes".into())
    }

    /// The menus of the notes (MU87): the section switch, which holds what
    /// the strip of tabs did, and the fields.
    pub(super) fn notes_menu(&self, key: &str) -> Option<Vec<Item>> {
        let (head, at) = key.split_once(':').unwrap_or((key, "area"));
        let info = self.notes_info();
        let cur = self.notes_section.as_str();
        Some(match head {
            "notes.pick" => {
                // only what to show: making, renaming and deleting are the
                // buttons beside it (MU88)
                let _ = at;
                let mut items: Vec<Item> = info
                    .note_sections()
                    .iter()
                    .map(|s| {
                        Item::new(format!("notes.pick:{}", s.key), &s.name)
                            .checked(cur == s.key)
                            .icon("file-text")
                            .hint("Drawing notes: place them on a sheet and they follow")
                    })
                    .collect();
                items.push(Item::Separator);
                let all = info.pad_items().iter().filter(|i| i.todo).count();
                let done = info.pad_items().iter().filter(|i| i.todo && i.done).count();
                items.push(
                    Item::new(format!("notes.pick:{DRAFTS}"), "Drafts")
                        .checked(cur == DRAFTS)
                        .icon("sticky-note")
                        .hint("Yours: whatever you are thinking. Placeable too"),
                );
                items.push(
                    Item::new(
                        format!("notes.pick:{TODOS}"),
                        format!("To-dos {done}/{all}"),
                    )
                    .checked(cur == TODOS)
                    .icon("check")
                    .hint("Yours: tick them off. A placed list prints with its boxes"),
                );
                items.push(
                    Item::new(format!("notes.pick:{WORKLOG}"), "Work log")
                        .checked(cur == WORKLOG)
                        .icon("clock")
                        .hint("Yours: every stretch of Project time and its note. Kept beside the project, never in the file"),
                );
                items
            }
            "notes.placed" => {
                let on = self.notes_placed_on(cur);
                if on.is_empty() {
                    return Some(vec![
                        Item::new("notes.none", "On no sheet yet").enabled(false)
                    ]);
                }
                let model = self.doc.model();
                on.iter()
                    .map(|(view, name, note)| {
                        let sheet = model.get(*view).map(|e| e.name.clone()).unwrap_or_default();
                        Item::new(
                            format!("notes.goto:{}", note.0),
                            format!("{name} \u{b7} {sheet}"),
                        )
                        .icon("file-text")
                        .hint("Open it with this block picked")
                    })
                    .collect()
            }
            "notes.fields" => {
                let fields = project::note_fields(self.doc.model());
                project::NOTE_FIELDS
                    .iter()
                    .map(|(name, what)| {
                        let now = fields
                            .get(*name)
                            .map_or("not set yet".to_string(), |v| v.clone());
                        Item::new(format!("notes.field:{at}:{name}"), format!("{{{name}}}"))
                            .hint(format!("{what}: {now}"))
                    })
                    .collect()
            }
            _ => return None,
        })
    }

    /// The Notes area's header controls, after the switch and + (MU87):
    /// what writes the text, then what is done with it.
    pub(super) fn notes_controls(&self) -> Vec<Value> {
        let text = self.notes_section != DRAFTS && self.notes_section != TODOS;
        let preview = self.notes_preview.contains(&NotesAt::Area);
        let mut v = vec![];
        // the work log is the time book's: it goes on no sheet
        if self.notes_section == WORKLOG {
            v.push(json!({"kind": "button", "id": "time.area", "icon": "clock", "label": "Open the Time area"}));
            return v;
        }
        if text {
            v.push(json!({"kind": "toggle", "id": "notes.preview:area", "icon": "markdown",
                          "label": "Preview: as it prints", "on": preview,
                          "hint": "Off: the Markdown you type. On: the section as the sheet prints it, fields filled"}));
        }
        if text && !preview {
            for (i, (icon, _, tip)) in LINE_STARTS.iter().enumerate() {
                v.push(
                    json!({"kind": "button", "id": format!("notes.line:area:{i}"),
                              "icon": icon, "label": tip}),
                );
            }
            v.push(
                json!({"kind": "menu", "key": "notes.fields:area", "icon": "",
                          "label": "Field", "value": "{ } Field",
                          "hint": "A field that fills itself on the sheet, as in title blocks"}),
            );
            v.push(json!({"kind": "sep"}));
        }
        v.push(json!({"kind": "button", "id": "notes.copy", "icon": "copy",
                      "label": "Copy all of this section, as Markdown"}));
        v.push(
            json!({"kind": "button", "id": "notes.pop", "icon": "square-arrow-out-up-right",
                      "label": "Pop out: the same notes in a window of their own"}),
        );
        v.push(json!({"kind": "button", "id": "notes.place", "icon": "file-plus", "label": "Place on a sheet",
                      "off": self.notes_target_sheet().is_none(),
                      "hint": "A text block on the sheet in front that shows this section and follows it"}));
        // where it is placed, as a menu with its count (MU89)
        let placed = self.notes_placed_on(&self.notes_section).len();
        v.push(
            json!({"kind": "menu", "key": "notes.placed", "icon": "map-pin", "label": "Placed on",
                      "badge": placed,
                      "hint": "The sheets that show this section: pick one to open it"}),
        );
        v.push(json!({"kind": "sep"}));
        v.push(
            json!({"kind": "button", "id": "help:notes", "icon": "circle-help",
                      "label": "How to write notes: Markdown, fields, placing"}),
        );
        v
    }

    /// The open section's rename and delete buttons' tips, or None for
    /// Drafts and To-dos, which are always there.
    pub(super) fn notes_edit_tips(&self) -> Option<(String, String)> {
        let cur = self.notes_section.as_str();
        if cur == DRAFTS || cur == TODOS || cur == WORKLOG {
            return None;
        }
        let name = self.notes_info().section_name(cur).unwrap_or_default();
        let n = self.notes_placed(cur);
        let del = if n > 0 {
            format!("Delete {name}: its {n} sheet notes keep the text they show now (Ctrl+Z brings it back)")
        } else {
            format!("Delete {name} (Ctrl+Z brings it back)")
        };
        Some((format!("Rename {name} (or double-click its name)"), del))
    }

    /// Renames the section `key`.
    fn rename_section(&mut self, key: &str, name: &str) {
        let mut info = self.notes_info();
        let mut all = info.note_sections();
        if let Some(s) = all.iter_mut().find(|s| s.key == key) {
            // the heading it started with follows the name (MU87)
            let old = NoteSection::new(key, &s.name).text;
            let new = NoteSection::new(key, name).text;
            let (old, new) = (
                old.lines().next().unwrap_or(""),
                new.lines().next().unwrap_or(""),
            );
            if !old.is_empty() && s.text.starts_with(old) {
                s.text = format!("{new}{}", &s.text[old.len()..]);
            }
            s.name = name.to_string();
        }
        info.sections = all;
        self.set_notes_info(info);
    }

    /// Deletes the section `key`; sheet notes showing it keep its text.
    fn delete_section(&mut self, key: &str) {
        let mut info = self.notes_info();
        let mut all = info.note_sections();
        all.retain(|s| s.key != key);
        // none left would bring the starting four back: keep one
        if all.is_empty() {
            all.push(NoteSection::new("general", "General notes"));
        }
        info.sections = all;
        self.unlink_notes(key);
        self.set_notes_info(info);
    }

    /// The open section's name, as a field to type a new one in, where
    /// the switch was. Enter or a click away keeps it, Esc leaves it.
    pub(super) fn notes_rename_field(&mut self, ui: &mut egui::Ui, w: f32) {
        let Some((key, _)) = self.notes_renaming.clone() else {
            return;
        };
        let id = egui::Id::new(("notes.rename.text", &key));
        let mut name: String = ui
            .ctx()
            .data(|d| d.get_temp(id))
            .unwrap_or_else(|| self.notes_info().section_name(&key).unwrap_or_default());
        let r = crate::props::text_field(
            ui,
            egui::TextEdit::singleline(&mut name).id(id.with("edit")),
            w,
        )
        .on_hover_text("The section's name: Enter keeps it, Esc leaves it as it was");
        if !r.has_focus() && !r.lost_focus() {
            r.request_focus();
        }
        // the whole name picked when the field first shows, so typing
        // replaces it
        let seen = id.with("seen");
        if !ui.ctx().data(|d| d.get_temp::<bool>(seen)).unwrap_or(false) {
            ui.ctx().data_mut(|d| d.insert_temp(seen, true));
            let edit = id.with("edit");
            let mut state = egui::TextEdit::load_state(ui.ctx(), edit).unwrap_or_default();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(name.chars().count()),
                )));
            state.store(ui.ctx(), edit);
        }
        ui.ctx().data_mut(|d| d.insert_temp(id, name.clone()));
        if r.lost_focus() {
            if !ui.input(|i| i.key_pressed(egui::Key::Escape)) && !name.trim().is_empty() {
                self.rename_section(&key, name.trim());
            }
            ui.ctx().data_mut(|d| {
                d.remove::<String>(id);
                d.remove::<bool>(seen);
            });
            self.notes_renaming = None;
        }
    }

    /// Whether the section is being renamed where `at` draws it.
    pub(super) fn notes_renaming_at(&self, at: NotesAt) -> bool {
        self.notes_renaming.as_ref().is_some_and(|(_, a)| *a == at)
    }

    /// The notes' body. In an area the switch and every button are in the
    /// area's header and the text has the whole body (MU87, MU89). In
    /// Properties two folds: Note (the switch, + and delete, the buttons
    /// used most, a short box that scrolls) and Placed on (MU89). The
    /// popped-out window has the full set above its text.
    pub(super) fn notes_sections(&mut self, ui: &mut egui::Ui, at: NotesAt) {
        let mut run: Option<String> = None;
        if at == NotesAt::Panel {
            let open = self.prop_header(ui, "notes_note", "Note", true, Some("notes"));
            crate::props::fold(ui, open, |ui| self.notes_body(ui, at, &mut run));
            let current = self.notes_current();
            let n = self.notes_placed_on(&current).len();
            let title = if n == 0 {
                "Placed on".to_string()
            } else {
                format!("Placed on \u{b7} {n}")
            };
            let open = self.prop_header(
                ui,
                "notes_placed",
                &title,
                true,
                Some("notes#notes-on-sheets"),
            );
            crate::props::fold(ui, open, |ui| {
                self.notes_placed_list(ui, &current, &mut run)
            });
        } else {
            self.notes_body(ui, at, &mut run);
        }
        if let Some(id) = run {
            let ctx = ui.ctx().clone();
            self.run_action(&ctx, &id);
        }
    }

    /// The switch, the buttons and the text, for `at`.
    fn notes_body(&mut self, ui: &mut egui::Ui, at: NotesAt, run: &mut Option<String>) {
        let current = self.notes_current();
        let text = current != DRAFTS && current != TODOS && current != WORKLOG;
        let tag = at.tag();
        let preview = self.notes_preview.contains(&at);

        if at == NotesAt::Window {
            self.notes_window_bar(ui, text, preview, run);
        } else if at == NotesAt::Panel {
            let p = self.shell.theme.palette();
            // the switch as wide as the row allows, + and delete beside it
            // (MU89: delete next to +, rename with the other buttons)
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                // + and delete at the row's right end, the name all the room
                // left (MU90)
                let w = (ui.available_width() - 62.0).max(80.0);
                if self.notes_renaming_at(at) {
                    self.notes_rename_field(ui, w);
                } else {
                    let r = super::areasui::name_button_w(ui, &self.notes_title(), w, true, &p)
                        .on_hover_text(
                            "The section shown: click to show another, double-click to rename it",
                        );
                    if r.double_clicked() && self.notes_edit_tips().is_some() {
                        *run = Some(format!("notes.rename:{tag}"));
                    }
                    let items = self
                        .notes_menu(&format!("notes.pick:{tag}"))
                        .unwrap_or_default();
                    egui::Popup::menu(&r).show(|ui| {
                        if let Some(id) = crate::ui_model::show(ui, &items) {
                            *run = Some(id);
                        }
                    });
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((_, del)) = self.notes_edit_tips() {
                        if crate::props::icon_button(ui, "trash-2", &del).clicked() {
                            *run = Some("notes.delete".into());
                        }
                    }
                    if crate::props::icon_button(ui, "plus", "A new section of your own").clicked()
                    {
                        *run = Some(format!("notes.add:{tag}"));
                    }
                });
            });
            // the buttons used most here; the window has them all
            if text {
                crate::props::flow(ui, |ui| {
                    if super::areasui::icon_btn(ui, "markdown", preview, true, &p)
                        .on_hover_text("Preview: the section as the sheet prints it, fields filled. Off: the Markdown you type")
                        .clicked()
                    {
                        *run = Some(format!("notes.preview:{tag}"));
                    }
                    if let Some((ren, _)) = self.notes_edit_tips() {
                        if crate::props::icon_button(ui, "pencil", &ren).clicked() {
                            *run = Some(format!("notes.rename:{tag}"));
                        }
                    }
                    if !preview {
                        let most: &[usize] = if at == NotesAt::Panel {
                            &[2, 3]
                        } else {
                            &[0, 1, 2, 3]
                        };
                        for &i in most {
                            let (icon, _, tip) = LINE_STARTS[i];
                            if crate::props::icon_button(ui, icon, tip).clicked() {
                                *run = Some(format!("notes.line:{tag}:{i}"));
                            }
                        }
                        if at == NotesAt::Window {
                            ui.add_space(6.0);
                            let r = ui.button("{ } Field").on_hover_text(
                                "A field that fills itself on the sheet, as in title blocks",
                            );
                            let items = self
                                .notes_menu(&format!("notes.fields:{tag}"))
                                .unwrap_or_default();
                            egui::Popup::menu(&r).show(|ui| {
                                if let Some(id) = crate::ui_model::show(ui, &items) {
                                    *run = Some(id);
                                }
                            });
                        }
                    }
                    if at == NotesAt::Panel {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if crate::props::icon_button(
                                ui,
                                "square-arrow-out-up-right",
                                "Open the notes in an area of their own: every button and the whole text",
                            )
                            .clicked()
                            {
                                *run = Some("notes.area".into());
                            }
                        });
                    }
                });
            }
            ui.add_space(2.0);
        }

        let mut info = self.notes_info();
        if text {
            // written back only when typed in: opening it changes nothing
            let before = info.note_sections();
            let mut all = before.clone();
            let insert = match &self.notes_insert {
                Some((a, _, _)) if *a == at => self.notes_insert.take().map(|(_, w, l)| (w, l)),
                _ => None,
            };
            if preview {
                let shown = project::linked_text(self.doc.model(), &current).unwrap_or_default();
                if section_preview(ui, &shown, at) {
                    // a click on it goes back to typing
                    self.notes_preview.remove(&at);
                }
            } else if let Some(s) = all.iter_mut().find(|s| s.key == current) {
                section_editor(ui, s, at, insert);
            }
            if all != before {
                info.sections = all;
                self.set_notes_info(info.clone());
            }
        } else if current == WORKLOG {
            self.time_worklog(ui);
        } else {
            self.notes_pad(ui, Some(current == TODOS));
        }

        if at == NotesAt::Panel && current != WORKLOG {
            // what is done with it: place, copy
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let can = self.notes_target_sheet().is_some();
                if ui
                    .add_enabled(can, egui::Button::new("Place on a sheet"))
                    .on_hover_text(
                        "A text block on the sheet in front that shows this section and follows it",
                    )
                    .on_disabled_hover_text("Open a sheet to place this section on it")
                    .clicked()
                {
                    *run = Some("notes.place".into());
                }
                if crate::props::icon_button(ui, "copy", "Copy all of this section, as Markdown")
                    .clicked()
                {
                    *run = Some("notes.copy".into());
                }
            });
        }
    }

    /// The popped-out window's one bar (MU89): the switch, + and the
    /// writing buttons on the left; where it is placed, copy, place and
    /// delete on the right. Narrow, the writing buttons fold into a menu
    /// rather than wrap.
    fn notes_window_bar(
        &mut self,
        ui: &mut egui::Ui,
        text: bool,
        preview: bool,
        run: &mut Option<String>,
    ) {
        let p = self.shell.theme.palette();
        let tag = NotesAt::Window.tag();
        let room = ui.available_width();
        let folded = room < 620.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let w = (room * 0.28).clamp(90.0, 200.0);
            if self.notes_renaming_at(NotesAt::Window) {
                self.notes_rename_field(ui, w);
            } else {
                let r = super::areasui::name_button(ui, &self.notes_title(), w, &p)
                    .on_hover_text("The section shown: click to show another, double-click to rename it");
                if r.double_clicked() && self.notes_edit_tips().is_some() {
                    *run = Some(format!("notes.rename:{tag}"));
                }
                let items = self.notes_menu(&format!("notes.pick:{tag}")).unwrap_or_default();
                egui::Popup::menu(&r).show(|ui| {
                    if let Some(id) = crate::ui_model::show(ui, &items) {
                        *run = Some(id);
                    }
                });
            }
            if crate::props::icon_button(ui, "plus", "A new section of your own").clicked() {
                *run = Some(format!("notes.add:{tag}"));
            }
            if text {
                ui.add_space(6.0);
                if super::areasui::icon_btn(ui, "markdown", preview, true, &p)
                    .on_hover_text("Preview: the section as the sheet prints it, fields filled. Off: the Markdown you type")
                    .clicked()
                {
                    *run = Some(format!("notes.preview:{tag}"));
                }
                if let Some((ren, _)) = self.notes_edit_tips() {
                    if crate::props::icon_button(ui, "pencil", &ren).clicked() {
                        *run = Some(format!("notes.rename:{tag}"));
                    }
                }
                if !preview {
                    let fields = self.notes_menu(&format!("notes.fields:{tag}")).unwrap_or_default();
                    if folded {
                        // the writing buttons in one menu
                        let r = crate::props::icon_button(ui, "align-left", "Lines and fields");
                        egui::Popup::menu(&r).show(|ui| {
                            for (i, (_, _, tip)) in LINE_STARTS.iter().enumerate() {
                                if ui.button(*tip).clicked() {
                                    *run = Some(format!("notes.line:{tag}:{i}"));
                                    ui.close();
                                }
                            }
                            ui.separator();
                            if let Some(id) = crate::ui_model::show(ui, &fields) {
                                *run = Some(id);
                            }
                        });
                    } else {
                        for (i, (icon, _, tip)) in LINE_STARTS.iter().enumerate() {
                            if crate::props::icon_button(ui, icon, tip).clicked() {
                                *run = Some(format!("notes.line:{tag}:{i}"));
                            }
                        }
                        let r = ui
                            .button("{ } Field")
                            .on_hover_text("A field that fills itself on the sheet, as in title blocks");
                        egui::Popup::menu(&r).show(|ui| {
                            if let Some(id) = crate::ui_model::show(ui, &fields) {
                                *run = Some(id);
                            }
                        });
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some((_, del)) = self.notes_edit_tips() {
                    if crate::props::icon_button(ui, "trash-2", &del).clicked() {
                        *run = Some("notes.delete".into());
                    }
                }
                let can = self.notes_target_sheet().is_some();
                ui.add_enabled_ui(can, |ui| {
                    let tip = if can {
                        "Place on a sheet: a text block on the sheet in front that follows this section"
                    } else {
                        "Open a sheet to place this section on it"
                    };
                    if crate::props::icon_button(ui, "file-plus", tip).clicked() {
                        *run = Some("notes.place".into());
                    }
                });
                if crate::props::icon_button(ui, "copy", "Copy all of this section, as Markdown").clicked() {
                    *run = Some("notes.copy".into());
                }
                let key = self.notes_section.clone();
                let n = self.notes_placed_on(&key).len();
                let r = crate::props::icon_button(
                    ui,
                    "map-pin",
                    &format!("Placed on {n} sheet block(s): pick one to open it"),
                );
                let items = self.notes_menu("notes.placed").unwrap_or_default();
                egui::Popup::menu(&r).show(|ui| {
                    if let Some(id) = crate::ui_model::show(ui, &items) {
                        *run = Some(id);
                    }
                });
                if n > 0 {
                    ui.label(egui::RichText::new(n.to_string()).small().weak());
                }
            });
        });
        ui.add_space(4.0);
    }

    /// Where the section `key` is placed: each block's view (a sheet), its
    /// number or name, and the block.
    fn notes_placed_on(&self, key: &str) -> Vec<(ElementId, String, ElementId)> {
        let model = self.doc.model();
        let mut out: Vec<(ElementId, String, ElementId)> = model
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Note(n) if n.section.as_deref() == Some(key) => {
                    let name = sheets::sheet_of(model, n.view)
                        .map(|s| s.number.clone())
                        .filter(|s| !s.is_empty())
                        .or_else(|| model.get(n.view).map(|v| v.name.clone()))
                        .unwrap_or_default();
                    Some((n.view, name, e.id))
                }
                _ => None,
            })
            .collect();
        out.sort_by(|a, b| a.1.cmp(&b.1));
        out
    }

    /// Opens the sheet a block is on with the block picked.
    fn notes_go_to(&mut self, note: ElementId) {
        let Some(ElementKind::Note(n)) = self.doc.model().get(note).map(|e| &e.kind) else {
            return;
        };
        self.active_view = Some(n.view);
        self.selection.only(note);
        self.notes_synced = Some(note);
    }

    /// The Placed on fold (MU89): a row per block, ticked or not, its sheet
    /// a pill that opens it; then what to do with the ticked ones.
    fn notes_placed_list(&mut self, ui: &mut egui::Ui, key: &str, run: &mut Option<String>) {
        let on = self.notes_placed_on(key);
        if on.is_empty() {
            crate::props::empty(ui, "On no sheet yet. Open a sheet, then Place on a sheet.");
            return;
        }
        self.notes_ticked
            .retain(|id| on.iter().any(|(_, _, n)| n == id));
        let mut go = None;
        for (_, name, note) in &on {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let mut on = self.notes_ticked.contains(note);
                if crate::props::tickbox(ui, &mut on, "").changed() {
                    if on {
                        self.notes_ticked.insert(*note);
                    } else {
                        self.notes_ticked.remove(note);
                    }
                }
                let sheet_name = self
                    .doc
                    .model()
                    .get(*note)
                    .and_then(|e| match &e.kind {
                        ElementKind::Note(n) => {
                            self.doc.model().get(n.view).map(|v| v.name.clone())
                        }
                        _ => None,
                    })
                    .unwrap_or_default();
                if ui
                    .selectable_label(self.selection.contains(*note), name)
                    .on_hover_text(format!("Open {name} with this block picked"))
                    .clicked()
                {
                    go = Some(*note);
                }
                let part = match self.doc.model().get(*note).map(|e| &e.kind) {
                    Some(ElementKind::Note(n)) => {
                        n.follows.map(|f| format!(" \u{b7} part {}", f.part + 1))
                    }
                    _ => None,
                }
                .unwrap_or_default();
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("{sheet_name}{part}"))
                            .small()
                            .weak(),
                    )
                    .truncate(),
                );
            });
        }
        let ticked = self.notes_ticked.len();
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let all = ticked == on.len();
            if ui
                .button(if all { "None" } else { "All" })
                .on_hover_text("Tick every block, or none")
                .clicked()
            {
                if all {
                    self.notes_ticked.clear();
                } else {
                    self.notes_ticked = on.iter().map(|(_, _, n)| *n).collect();
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(ticked > 0, |ui| {
                    if crate::props::icon_button(
                        ui,
                        "trash-2",
                        &format!(
                            "Take the {ticked} ticked block(s) off their sheets; the section stays"
                        ),
                    )
                    .clicked()
                    {
                        *run = Some("notes.unplace".into());
                    }
                    if crate::props::icon_button(
                        ui,
                        "square-dashed-mouse-pointer",
                        "Pick the ticked blocks",
                    )
                    .clicked()
                    {
                        *run = Some("notes.pickplaced".into());
                    }
                });
            });
        });
        if let Some(n) = go {
            self.notes_go_to(n);
        }
    }

    /// How many sheet notes show the section `key`.
    fn notes_placed(&self, key: &str) -> usize {
        self.doc
            .model()
            .iter()
            .filter(
                |e| matches!(&e.kind, ElementKind::Note(n) if n.section.as_deref() == Some(key)),
            )
            .count()
    }

    /// Notes showing the section `key` take its text as their own, so
    /// deleting the section leaves them as they were.
    fn unlink_notes(&mut self, key: &str) {
        let model = self.doc.model();
        let text = project::linked_text(model, key).unwrap_or_default();
        let mut tx = Transaction::new("Delete notes section");
        for e in model.iter() {
            if let ElementKind::Note(n) = &e.kind {
                if n.section.as_deref() == Some(key) {
                    let mut after = e.clone();
                    let mut n = n.clone();
                    n.section = None;
                    n.text = text.clone();
                    n.markdown = true;
                    after.kind = ElementKind::Note(n);
                    tx = tx.replace(e.clone(), after);
                }
            }
        }
        if let Err(e) = self.doc.commit(tx) {
            self.status = format!("Couldn't do that: {e}");
        }
    }

    /// A note's long-note rows on a sheet (Mockup_Long_Notes_REV00): its
    /// height, Whole or Continued and Continue on a first block; a later
    /// block's place, title and sheet. Edits go into `n`; making a new
    /// block or jumping to one goes into `act`.
    pub(super) fn note_long_ui(
        &self,
        ui: &mut egui::Ui,
        id: ElementId,
        n: &mut creabim_core::Note,
        act: &mut Option<LongAct>,
    ) -> Option<&'static str> {
        if n.symbol.is_some() || !self.is_sheet(n.view) {
            return None;
        }
        let mut label = None;
        let model = self.doc.model();
        let red = egui::Color32::from_rgb(214, 72, 59);
        if let Some(mut f) = n.follows {
            let total = creabim_geom::annotate::chain_blocks(model, f.first).len();
            crate::props::row(ui, "Part", |ui| {
                ui.label(format!("{} of {total}", f.part + 1));
                let h = ui.spacing().interact_size.y;
                let w = ui.available_width().max(40.0);
                if ui
                    .add_sized([w, h], egui::Button::new("Go to part 1"))
                    .on_hover_text("Pick part 1 and show its sheet")
                    .clicked()
                {
                    *act = Some(LongAct::GoTo(f.first));
                }
            });
            crate::app::helpui::combo(ui, "Title")
                .selected_text(f.title.label())
                .show_ui(ui, |ui| {
                    for t in creabim_core::PartTitle::ALL {
                        if ui.selectable_value(&mut f.title, t, t.label()).changed() {
                            label = Some("Part title");
                        }
                    }
                });
            n.follows = Some(f);
            // carried on to another sheet: the same place on that sheet
            let sheets = self.sheet_list();
            let now = sheets
                .iter()
                .find(|(s, _)| *s == n.view)
                .map_or(String::new(), |(_, name)| name.clone());
            crate::app::helpui::combo(ui, "Sheet")
                .selected_text(now)
                .show_ui(ui, |ui| {
                    for (s, name) in &sheets {
                        if ui.selectable_value(&mut n.view, *s, name).changed() {
                            label = Some("Move to sheet");
                        }
                    }
                });
        } else {
            let first_parts = creabim_geom::annotate::chain_blocks(model, id).len();
            if first_parts > 1 || n.height.is_some() {
                // the house segmented control (owner, after MU92)
                crate::props::row(ui, "Long note", |ui| {
                    if crate::props::segment(
                        ui,
                        &mut n.continued,
                        &[
                            (false, "Whole", "All of its text here, past its height; the blocks that carry it on stay empty"),
                            (true, "Continued", "Its text stops at its height and goes on in the blocks that carry it on"),
                        ],
                    ) {
                        label = Some(if n.continued { "Continued note" } else { "Whole note" });
                    }
                });
            }
        }
        // the block's height: where it stops
        // the tick on its row, the value under it, as Wrap at (MU89: one
        // row pushed the panel past its edge)
        let mut stop = n.height.is_some();
        crate::props::row(ui, "Height", |ui| {
            if crate::props::tickbox(ui, &mut stop, "Stop at")
                .on_hover_text("A height for the block: the text stops there when it is Continued")
                .changed()
            {
                n.height = stop.then_some(120.0);
                label = Some("Note height");
            }
        });
        if let Some(h) = &mut n.height {
            crate::props::row(ui, "", |ui| {
                if crate::props::fill(ui, crate::units::paper(h).range(5.0..=2000.0)).changed() {
                    label = Some("Note height");
                }
            });
        }
        let past = creabim_geom::annotate::note_runs_past(model, id);
        // which way the parts go, kept on the first block
        let first_id = n.follows.map_or(id, |f| f.first);
        let left_now = if n.follows.is_none() {
            n.continue_left
        } else {
            matches!(model.get(first_id).map(|e| &e.kind), Some(ElementKind::Note(f)) if f.continue_left)
        };
        // the house segmented control, as Leader shape (owner: two loose
        // labels were sloppy and didn't line up with the fields)
        crate::props::row(ui, "Next part", |ui| {
            let mut left = left_now;
            if crate::props::segment(
                ui,
                &mut left,
                &[
                    (true, "Left", "New parts go to the left of the last one"),
                    (false, "Right", "New parts go to the right of the last one"),
                ],
            ) {
                *act = Some(LongAct::Side(first_id, left));
            }
        });
        // nothing left to carry on: Continue says so (MU91: it made an
        // empty block)
        let blocks = creabim_geom::annotate::chain_blocks(model, first_id);
        let last = blocks.last().copied().unwrap_or(id);
        let lines = {
            let look = creabim_geom::annotate::look_of(model, n.style, n.color);
            let text = n
                .section
                .as_deref()
                .and_then(|k| creabim_core::project::linked_text(model, k))
                .unwrap_or_else(|| n.text.clone());
            creabim_geom::annotation::lay_out_note(&text, &look, n.width, true)
                .split('\n')
                .count()
        };
        let more = if n.follows.is_none() && n.height.is_none() {
            lines > 1
        } else {
            creabim_geom::annotate::note_runs_past(model, last) > 0
                || creabim_geom::annotate::note_runs_past(model, first_id) > 0
        };
        if crate::props::action_icon(
            ui,
            "",
            "file-plus",
            "Continue\u{2026}",
            if more {
                "Carry the text on in a new block beside this one: drag it anywhere, or move it to another sheet"
            } else {
                "Nothing left to carry on: every line already fits"
            },
            more,
        )
        .clicked()
        {
            *act = Some(LongAct::Continue(n.follows.map_or(id, |f| f.first), id));
        }
        // what doesn't fit, across the panel and wrapping, never pushing it
        // wider (MU90)
        if past > 0 {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!(
                        "{past} line(s) don't fit: make the block taller or Continue it"
                    ))
                    .small()
                    .color(red),
                )
                .wrap(),
            );
        }
        label
    }

    /// Carries the long note whose first block is `first` on in a new
    /// block: a column to the left of its last block, the same height,
    /// titled "(continued)"; the first block turns Continued.
    pub(super) fn continue_note(&mut self, first: ElementId, from: ElementId) {
        let model = self.doc.model();
        let blocks = creabim_geom::annotate::chain_blocks(model, first);
        let (Some(first_e), Some(last_e)) = (
            model.get(first).cloned(),
            // beside the block pressed, on its sheet (MU90: the third part
            // went to the last part's sheet, out of sight)
            model
                .get(from)
                .or_else(|| blocks.last().and_then(|b| model.get(*b)))
                .cloned(),
        ) else {
            return;
        };
        let (ElementKind::Note(f), ElementKind::Note(last)) = (&first_e.kind, &last_e.kind) else {
            return;
        };
        let width = f.width.unwrap_or(100.0);
        // no height yet: down to 25 mm above the sheet's foot
        // no height yet: half of what it holds, so the text splits in two
        // (MU89: the whole sheet's height left nothing to carry on)
        let height = f.height.unwrap_or_else(|| {
            let look = creabim_geom::annotate::look_of(model, f.style, f.color);
            let text = f
                .section
                .as_deref()
                .and_then(|k| creabim_core::project::linked_text(model, k))
                .unwrap_or_else(|| f.text.clone());
            let lines = creabim_geom::annotation::lay_out_note(&text, &look, f.width, true)
                .split('\n')
                .count() as f64;
            ((lines / 2.0).ceil() * look.size * 1.5).clamp(10.0, (f.at.y - 25.0).max(10.0))
        });
        let mut after = first_e.clone();
        if let ElementKind::Note(a) = &mut after.kind {
            a.continued = true;
            a.height = Some(height);
        }
        let mut part = last.clone();
        // always the side the first block says (owner after MU91: Right
        // still went left, because the note sat off the paper's right edge
        // and the old "keep it on the paper" rule flipped it)
        let w = last.width.unwrap_or(width);
        let x = if f.continue_left {
            last.at.x - w - 6.0
        } else {
            last.at.x + w + 6.0
        };
        part.at = glam::DVec2::new(x, last.at.y);
        part.leader = None;
        part.elbow = None;
        part.continued = false;
        part.height = Some(last.height.unwrap_or(height));
        part.follows = Some(creabim_core::NoteFollow {
            first,
            part: blocks.len() as u32,
            title: creabim_core::PartTitle::Continued,
        });
        let name = format!("{} (part {})", first_e.name, blocks.len() + 1);
        let e = Element::new(&name, ElementKind::Note(part));
        let new = e.id;
        let tx = Transaction::new("Continue the note")
            .replace(first_e.clone(), after)
            .insert(e);
        let r = self.doc.commit(tx);
        if r.is_ok() {
            self.selection.only(new);
        }
        self.report(r, "The note goes on in a new block: drag it where you want it, or pick another sheet for it.");
    }

    /// The notes in a window of their own (Pop out).
    pub(super) fn notes_window(&mut self, ctx: &egui::Context) {
        // what a note's long-note rows asked for last frame
        match self.notes_long.take() {
            Some(LongAct::Continue(first, from)) => self.continue_note(first, from),
            Some(LongAct::GoTo(b)) => self.notes_go_to(b),
            Some(LongAct::Side(first, left)) => {
                let r = self.doc.edit(first, "Next part side", |e| {
                    if let ElementKind::Note(n) = &mut e.kind {
                        n.continue_left = left;
                    }
                });
                if let Err(e) = r {
                    self.status = format!("Couldn't do that: {e}");
                }
            }
            None => {}
        }
        if !self.notes_popped {
            return;
        }
        let mut open = true;
        // on screen whatever its size, and never taller than the window
        // (MU89: it ran off the screen and could not be resized)
        let screen = ctx.content_rect().shrink(24.0);
        crate::app::shell::window("Notes")
            .id(egui::Id::new("notes-window"))
            .open(&mut open)
            .default_size([
                720.0_f32.min(screen.width()),
                520.0_f32.min(screen.height()),
            ])
            .max_size(screen.size())
            .constrain_to(screen)
            .scroll([false, true])
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| self.notes_sections(ui, NotesAt::Window));
        if !open {
            self.notes_popped = false;
        }
    }

    /// Link the reading note `n` to `id`: it reads `id` whatever its arrow
    /// does; an arrow not attached to anything moves onto it, in a plan.
    pub(super) fn link_note(&self, n: &mut creabim_core::Note, id: ElementId) {
        let r = n.reads.get_or_insert_with(Default::default);
        r.follow = false;
        r.fixed = Some(id);
        let model = self.doc.model();
        if n.leader.is_some_and(|l| l.host.is_none()) {
            if let (Some(v), Some(c)) =
                (self.view(n.view), creabim_geom::annotate::centre(model, id))
            {
                if matches!(v.kind, creabim_core::ViewKind::Plan { .. }) {
                    n.leader = Some(creabim_geom::annotate::attach(
                        model,
                        v,
                        Some(id),
                        c.truncate(),
                    ));
                }
            }
        }
    }

    /// A reading note's rows (Mockup_Linked_Leaders_REV00, MU135): the
    /// object it reads, chosen from a list searched by name, mark, IFC
    /// class or GlobalId, or picked in a view with the pipette; the fields
    /// it can add to its text; Follow the arrow and Capitals. `picking` is
    /// whether the pipette is on; `pick` turns it on or off.
    fn note_reads_ui(
        &self,
        ui: &mut egui::Ui,
        n: &mut creabim_core::Note,
        picking: bool,
        pick: &mut bool,
    ) -> Option<&'static str> {
        let model = self.doc.model();
        let mut label = None;
        let from = n.reads_from();
        let linked = from.and_then(|id| model.get(id));
        let shown = match linked {
            Some(e) => format!("{} · {}", e.name, e.ifc_class()),
            None if from.is_some() => "The object it read is gone".into(),
            None => "Choose an object".into(),
        };
        let mut chosen = None;
        crate::props::row(ui, "Object", |ui| {
            let h = ui.spacing().interact_size.y.max(24.0);
            let w = (ui.available_width() - h - ui.spacing().item_spacing.x).max(40.0);
            let text = if linked.is_some() {
                egui::RichText::new(shown.clone())
            } else {
                egui::RichText::new(shown.clone()).color(ui.visuals().warn_fg_color)
            };
            // in a space of its own, as wide as it is to be: egui cuts a
            // combo's words to the room left in the row, so with the
            // pipette still to come they ran on and pushed it, and every
            // row under it, past the panel's edge (MU141)
            let r = ui
                .allocate_ui(egui::vec2(w, h), |ui| {
                    egui::ComboBox::from_id_salt("note.object")
                        .icon(crate::props::combo_icon)
                        .width(w)
                        .truncate()
                        .height(360.0)
                        .selected_text(text)
                        .show_ui(ui, |ui| {
                            ui.set_min_width(260.0);
                            let key = egui::Id::new("note.object.search");
                            let mut q: String =
                                ui.data_mut(|d| d.get_temp(key).unwrap_or_default());
                            let edit = ui.add(
                                egui::TextEdit::singleline(&mut q)
                                    .hint_text("Name, mark, IFC class or GlobalId")
                                    .desired_width(f32::INFINITY),
                            );
                            edit.request_focus();
                            ui.data_mut(|d| d.insert_temp(key, q.clone()));
                            let found = note_objects(model, &q);
                            if found.is_empty() {
                                ui.weak("Nothing matches");
                            }
                            for (id, words) in found.iter().take(80) {
                                if ui.selectable_label(Some(*id) == from, words).clicked() {
                                    chosen = Some(*id);
                                }
                            }
                            if found.len() > 80 {
                                ui.weak(format!("{} more: type to narrow", found.len() - 80));
                            }
                        })
                })
                .inner;
            r.response.on_hover_text(match linked {
                Some(e) => format!("{}\nGlobalId {}", e.name, e.id.ifc_guid()),
                None => "The object whose name, mark, material… the note shows".into(),
            });
            let b = crate::props::tool_button(
                ui,
                "pipette",
                if picking {
                    "Picking: click an object in a view (Esc to stop)"
                } else {
                    "Pick the object in a view"
                },
            );
            if picking {
                ui.painter().rect_stroke(
                    b.rect,
                    4.0,
                    egui::Stroke::new(1.5, ui.visuals().selection.bg_fill),
                    egui::StrokeKind::Inside,
                );
            }
            if b.clicked() {
                *pick = !picking;
            }
        });
        if let Some(id) = chosen {
            self.link_note(n, id);
            label = Some("Note object");
        }
        crate::props::row(ui, "Add", |ui| {
            crate::props::flow(ui, |ui| {
                for (_, word) in creabim_core::NOTE_FIELDS {
                    let value = from
                        .zip(creabim_core::note_field_key(word))
                        .and_then(|(id, k)| creabim_geom::annotate::note_field(model, id, k))
                        .unwrap_or_else(|| "none here".into());
                    if ui
                        .small_button(word)
                        .on_hover_text(format!("Adds {{{word}}} to the text: {value}"))
                        .clicked()
                    {
                        if !n.text.is_empty() && !n.text.ends_with([' ', '\n']) {
                            n.text.push(' ');
                        }
                        n.text.push_str(&format!("{{{word}}}"));
                        label = Some("Note field");
                    }
                }
            });
        });
        let Some(r) = n.reads.as_mut() else {
            return label;
        };
        let mut follow = r.follow;
        if crate::props::switch(ui, &mut follow, "Follow the arrow")
            .on_hover_text("On, the object is whatever the arrow is attached to: drag the arrow onto another and the note reads it. Off, the object chosen above")
            .changed()
        {
            r.follow = follow;
            r.fixed = if follow { None } else { from };
            label = Some("Note follows");
        }
        if crate::props::switch(ui, &mut r.caps, "Capitals")
            .on_hover_text("The words in capitals, as drawing notes often are")
            .changed()
        {
            label = Some("Note capitals");
        }
        label
    }

    /// A note's Shows row (Properties REV04, 3e): its own text or a notes
    /// section; linked, a preview of the section and Edit in Notes, typing
    /// happening in the Notes area. `edit_in` is the section to open.
    pub(super) fn note_shows_ui(
        &self,
        ui: &mut egui::Ui,
        n: &mut creabim_core::Note,
        edit_in: &mut Option<String>,
        picking: bool,
        pick: &mut bool,
    ) -> Option<&'static str> {
        if n.symbol.is_some() {
            return None;
        }
        let info = self.notes_info();
        let mut choices: Vec<(Option<String>, String)> = vec![(None, "Its own text".into())];
        for s in info.note_sections() {
            choices.push((Some(s.key), s.name));
        }
        choices.push((Some(DRAFTS.into()), "Drafts".into()));
        choices.push((Some(TODOS.into()), "To-dos".into()));
        const READS: &str = "An object's data";
        let reading = n.reads.is_some() && n.section.is_none();
        let now = if reading {
            READS.to_string()
        } else {
            choices
                .iter()
                .find(|(k, _)| *k == n.section)
                .map_or("A deleted section".to_string(), |(_, name)| name.clone())
        };
        let mut label = None;
        crate::props::Combo::new("Shows")
            .selected_text(now)
            .show_ui(ui, |ui| {
                for (i, (k, name)) in choices.iter().enumerate() {
                    let on = !reading && n.section == *k;
                    if ui.selectable_label(on, name).clicked() && !on {
                        n.section = k.clone();
                        n.reads = None;
                        if k.is_some() {
                            n.markdown = true;
                            n.width.get_or_insert(100.0);
                        }
                        label = Some("Note shows");
                    }
                    // after its own text: words read from an object (MU135)
                    if i == 0
                        && ui
                            .selectable_label(reading, READS)
                            .on_hover_text("Words read from an object you choose: {Name}, {Mark}, {IFC class}, {Layers}…")
                            .clicked()
                        && !reading
                    {
                        n.section = None;
                        n.reads = Some(creabim_core::NoteReads::default());
                        if !n.text.contains('{') {
                            if n.text.trim().is_empty() {
                                n.text = "{Name}".into();
                            } else {
                                n.text.push_str("\n{Name}");
                            }
                        }
                        // its arrow's object when it is attached to one;
                        // else the object is chosen, and the pipette is on
                        // to pick it straight away
                        match n.leader.and_then(|l| l.host) {
                            Some(id) => self.link_note(n, id),
                            None => *pick = true,
                        }
                        label = Some("Note shows");
                    }
                }
            })
            .response
            .on_hover_text("A notes section is one text on every sheet that shows it; An object's data shows its name, mark, IFC class, layers…");
        if n.reads.is_some() && n.section.is_none() {
            if let Some(l) = self.note_reads_ui(ui, n, picking, pick) {
                label = Some(l);
            }
            return label;
        }
        let Some(key) = n.section.clone() else {
            return label;
        };
        // a preview: typing happens in the Notes area
        let text = project::linked_text(self.doc.model(), &key).unwrap_or_default();
        egui::Frame::new()
            .fill(ui.visuals().extreme_bg_color)
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .corner_radius(4.0)
            // the scroll bar against the frame's edge, not the text (MU90)
            .inner_margin(egui::Margin {
                left: 8,
                right: 1,
                top: 6,
                bottom: 6,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().scroll.bar_outer_margin = 0.0;
                ui.spacing_mut().scroll.bar_inner_margin = 2.0;
                egui::ScrollArea::vertical()
                    .id_salt("note.linked")
                    .auto_shrink([false, true])
                    .max_height(130.0)
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(text).small()).wrap());
                    });
            });
        if crate::props::action_icon(
            ui,
            "",
            "pencil",
            "Edit in Notes",
            "Write it in the Notes area (or double-click the note)",
            true,
        )
        .clicked()
        {
            *edit_in = Some(key.clone());
        }
        label
    }

    /// The browser's Notes group: every section, drafts and to-dos.
    pub(super) fn notes_group(&self) -> Value {
        let info = self.notes_info();
        let mut rows: Vec<Value> = info
            .note_sections()
            .iter()
            .map(|s| {
                let n = self.notes_placed(&s.key);
                json!({
                    "id": format!("notes:{}", s.key), "name": s.name, "icon": "file-text",
                    "sel": self.notes_section == s.key,
                    "mark": if n > 0 { Some(format!("{n}")) } else { None },
                    "hint": if n > 0 {
                        format!("{}: click to write it in Notes; on {n} sheets", s.name)
                    } else {
                        format!("{}: click to write it in Notes", s.name)
                    },
                })
            })
            .collect();
        let todos = info.pad_items();
        let open = todos.iter().filter(|i| i.todo && !i.done).count();
        for (key, name, hint) in [
            (
                DRAFTS,
                "Drafts".to_string(),
                "Your drafts: click to open them in Notes",
            ),
            (
                TODOS,
                format!("To-dos ({open} open)"),
                "Your to-dos: click to open them in Notes",
            ),
        ] {
            rows.push(json!({
                "id": format!("notes:{key}"), "name": name, "icon": "sticky-note",
                "sel": self.notes_section == key, "hint": hint,
            }));
        }
        json!({
            "key": "notes", "title": "Notes", "icon": "sticky-note", "open": false,
            "rows": rows,
            "acts": [
                {"id": "notes.new", "icon": "plus", "hint": "A new notes section of your own"},
            ],
        })
    }
}

/// What a note's long-note rows ask for, done once Properties is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum LongAct {
    /// Carry the note whose first block this is on in a new block.
    /// (the first block, the block whose Continue was pressed)
    Continue(ElementId, ElementId),
    /// Pick this block and show its sheet.
    GoTo(ElementId),
    /// The side new parts go, set on the first block (left when true).
    Side(ElementId, bool),
}

/// Where the notes are drawn: each has its own editor, so typing in one
/// never reaches the other (MU87: Properties could not be typed in while
/// the area was open).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum NotesAt {
    Area,
    Panel,
    Window,
}

impl NotesAt {
    fn tag(self) -> &'static str {
        match self {
            NotesAt::Area => "area",
            NotesAt::Panel => "panel",
            NotesAt::Window => "window",
        }
    }

    fn from_tag(t: &str) -> Self {
        match t {
            "panel" => NotesAt::Panel,
            "window" => NotesAt::Window,
            _ => NotesAt::Area,
        }
    }
}

/// A drawing-notes section's Markdown. In Properties a short box that
/// scrolls (MU87); in an area or window as tall as there is room.
/// `insert` is what a toolbar button puts in: (text, at the line's start).
fn section_editor(
    ui: &mut egui::Ui,
    s: &mut NoteSection,
    at: NotesAt,
    insert: Option<(String, bool)>,
) {
    // under the place it is drawn in: two Notes areas showing the same
    // section are two editors, not one (owner 2026-11-18: egui marked
    // "First use of widget ID" in red on both)
    let id = ui.id().with(("notes.text", &s.key, at));
    // what the right-click menu asked for last frame: cut, copy, paste
    // reach the editor as the keys would
    let pending: Vec<egui::Event> = ui
        .ctx()
        .data_mut(|d| d.remove_temp(id.with("pending")))
        .unwrap_or_default();
    if !pending.is_empty() {
        ui.memory_mut(|m| m.request_focus(id));
        ui.input_mut(|i| i.events.extend(pending));
    }
    // pasted from Word or a web page: its headings, bold, italic and
    // lists kept, as Markdown (MU89)
    if ui.memory(|m| m.has_focus(id))
        && ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))))
    {
        if let Some(md) = clipboard_html()
            .map(|h| html_to_markdown(&h))
            .filter(|m| !m.is_empty())
        {
            ui.input_mut(|i| {
                for e in &mut i.events {
                    if let egui::Event::Paste(t) = e {
                        *t = md.clone();
                    }
                }
            });
        }
    }
    let s_view = &NoteSection {
        key: s.key.clone(),
        name: String::new(),
        text: s.text.clone(),
    };
    let mut resp: Option<egui::Response> = None;
    let edit = egui::TextEdit::multiline(&mut s.text)
        .id(id)
        .font(egui::TextStyle::Monospace)
        .hint_text("1. Do not scale from this drawing.");
    if at == NotesAt::Panel {
        // one frame round the text and its scroll bar, as wide as the
        // fields above (MU88: the bar sat outside, the frame's foot was
        // cut, the right edge looked padded)
        let focused = ui.memory(|m| m.has_focus(id));
        let v = ui.visuals();
        let stroke = if focused {
            v.selection.stroke
        } else {
            v.widgets.inactive.bg_stroke
        };
        egui::Frame::new()
            .fill(v.extreme_bg_color)
            .stroke(stroke)
            .corner_radius(v.widgets.inactive.corner_radius)
            // the scroll bar hugs the frame's right edge (MU90)
            .inner_margin(egui::Margin {
                left: 4,
                right: 1,
                top: 3,
                bottom: 3,
            })
            .show(ui, |ui| {
                ui.spacing_mut().scroll.bar_outer_margin = 0.0;
                ui.spacing_mut().scroll.bar_inner_margin = 2.0;
                egui::ScrollArea::vertical()
                    .id_salt(("notes.scroll", at))
                    .max_height(214.0)
                    .min_scrolled_height(214.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        let r = ui.add_sized(
                            [ui.available_width(), 214.0],
                            edit.frame(egui::Frame::NONE).desired_rows(12),
                        );
                        paint_guide(ui, r.rect, s_view);
                        resp = Some(r);
                    });
            });
    } else {
        let h = (ui.available_height() - 4.0).max(220.0);
        let r = ui.add_sized([ui.available_width(), h], edit);
        paint_guide(ui, r.rect, s_view);
        resp = Some(r);
    }
    if let Some((what, line)) = insert {
        insert_at_cursor(ui.ctx(), id, &mut s.text, &what, line);
    }
    // right-click: what a text box offers, then what writes notes (MU91)
    let Some(r) = resp else { return };
    let mut send: Vec<egui::Event> = vec![];
    let mut put: Option<(String, bool)> = None;
    let mut all = false;
    r.context_menu(|ui| {
        ui.set_min_width(190.0);
        if ui.button("Cut").clicked() {
            send.push(egui::Event::Cut);
            ui.close();
        }
        if ui.button("Copy").clicked() {
            send.push(egui::Event::Copy);
            ui.close();
        }
        if ui.button("Paste").clicked() {
            let text = arboard::Clipboard::new()
                .ok()
                .and_then(|mut c| c.get_text().ok());
            if let Some(t) = text {
                send.push(egui::Event::Paste(t));
            }
            ui.close();
        }
        if ui.button("Select all").clicked() {
            all = true;
            ui.close();
        }
        ui.separator();
        for (_, start, tip) in LINE_STARTS {
            let name = tip.split(':').next().unwrap_or(tip);
            if ui.button(name).clicked() {
                put = Some((start.to_string(), true));
                ui.close();
            }
        }
        ui.menu_button("Field", |ui| {
            for (name, what) in project::NOTE_FIELDS {
                if ui
                    .button(format!("{{{name}}}"))
                    .on_hover_text(what)
                    .clicked()
                {
                    put = Some((format!("{{{name}}}"), false));
                    ui.close();
                }
            }
        });
    });
    if !send.is_empty() {
        ui.ctx()
            .data_mut(|d| d.insert_temp(id.with("pending"), send));
        ui.ctx().request_repaint();
    }
    if let Some((what, line)) = put {
        insert_at_cursor(ui.ctx(), id, &mut s.text, &what, line);
    }
    if all {
        let mut state = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(s.text.chars().count()),
            )));
        state.store(ui.ctx(), id);
        ui.memory_mut(|m| m.request_focus(id));
    }
}

/// A short Markdown guide, faded under the heading of a section that has
/// nothing else yet: it goes as soon as anything is written (MU89).
const GUIDE: &str = "1. A numbered line starts with \"1. \"\n\
- A list starts with \"- \"\n\
- [ ] A box to tick starts with \"- [ ] \"\n\
**Bold** and *italic* print as such\n\
{project} fills itself in: see { } Field\n\
## A heading starts with \"## \"";

/// Paints the guide in `r`, the editor, under its first line.
fn paint_guide(ui: &egui::Ui, r: egui::Rect, s: &NoteSection) {
    if s.text.lines().filter(|l| !l.trim().is_empty()).count() > 1 {
        return;
    }
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let line = ui.fonts_mut(|f| f.row_height(&font));
    let at = r.min + egui::vec2(6.0, 4.0 + line * 2.5);
    let c = ui.visuals().weak_text_color().gamma_multiply(0.8);
    let g = ui
        .painter()
        .layout(GUIDE.to_string(), font, c, (r.width() - 12.0).max(40.0));
    ui.painter().with_clip_rect(r).galley(at, g, c);
}

/// The section as the sheet prints it (MU88, Raw / Preview): the heading in
/// capitals and underlined, lists, numbers and checkboxes drawn, fields
/// already filled. True when clicked, to go back to typing.
fn section_preview(ui: &mut egui::Ui, md: &str, at: NotesAt) -> bool {
    let v = ui.visuals().clone();
    let size = 13.0;
    let ink = v.text_color();
    let body = |ui: &mut egui::Ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        for line in md.lines() {
            let t = line.trim_end();
            if let Some(h) = t.strip_prefix("## ").or_else(|| t.strip_prefix("# ")) {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(h.trim().to_uppercase())
                        .size(size + 1.0)
                        .strong()
                        .underline(),
                );
                continue;
            }
            if t.is_empty() {
                ui.add_space(size * 0.5);
                continue;
            }
            let indent = (line.len() - line.trim_start().len()) as f32 * 6.0;
            let s = t.trim_start();
            // a box (ticked or not), a mark ("•", "3."), or none
            let (tick, mark, rest): (Option<bool>, Option<String>, &str) =
                if let Some(r) = s.strip_prefix("- [ ] ") {
                    (Some(false), None, r)
                } else if let Some(r) = s
                    .strip_prefix("- [x] ")
                    .or_else(|| s.strip_prefix("- [X] "))
                {
                    (Some(true), None, r)
                } else if let Some(r) = s.strip_prefix("- ").or_else(|| s.strip_prefix("* ")) {
                    (None, Some("\u{2022}".to_string()), r)
                } else if let Some((n, r)) = s
                    .split_once(". ")
                    .filter(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
                {
                    (None, Some(format!("{n}.")), r)
                } else {
                    (None, None, s)
                };
            ui.horizontal_wrapped(|ui| {
                ui.add_space(indent);
                if let Some(done) = tick {
                    let (r, _) =
                        ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
                    let b = egui::Rect::from_center_size(
                        r.center(),
                        egui::vec2(size * 0.75, size * 0.75),
                    );
                    ui.painter().rect_stroke(
                        b,
                        1.5,
                        egui::Stroke::new(1.2, ink),
                        egui::StrokeKind::Inside,
                    );
                    if done {
                        crate::svgicon::paint(ui.painter(), b.shrink(1.0), "check", ink);
                    }
                }
                if let Some(m) = &mark {
                    ui.add_sized(
                        [size * 1.4, size],
                        egui::Label::new(egui::RichText::new(m).size(size)),
                    );
                }
                ui.add(egui::Label::new(egui::RichText::new(rest).size(size)).wrap());
            });
        }
    };
    let h = if at == NotesAt::Panel {
        214.0
    } else {
        (ui.available_height() - 20.0).max(220.0)
    };
    let r = egui::Frame::new()
        .fill(v.extreme_bg_color)
        .stroke(v.widgets.inactive.bg_stroke)
        .corner_radius(v.widgets.inactive.corner_radius)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical()
                .id_salt(("notes.preview", at))
                .max_height(h)
                .min_scrolled_height(h)
                .auto_shrink([false, false])
                .show(ui, body);
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_text("As it prints. Click to go back to the Markdown");
    r.clicked()
}

/// Puts `what` in `text` at its editor's cursor, or at the start of the
/// cursor's line when `line`, and keeps typing there.
fn insert_at_cursor(ctx: &egui::Context, id: egui::Id, text: &mut String, what: &str, line: bool) {
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    let chars = text.chars().count();
    let at = state
        .cursor
        .char_range()
        .map_or(chars, |r| usize::from(r.primary.index).min(chars));
    let byte = |c: usize| text.char_indices().nth(c).map_or(text.len(), |(b, _)| b);
    let mut b = byte(at);
    if line {
        b = text[..b].rfind('\n').map_or(0, |i| i + 1);
        // a line already started so is left as it is
        if text[b..].starts_with(what) {
            return;
        }
    }
    text.insert_str(b, what);
    let after = text[..b + what.len()].chars().count();
    let after = if line {
        at + what.chars().count()
    } else {
        after
    };
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(after),
        )));
    state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/// What Word (or a web page) put on the clipboard as HTML, if anything.
fn clipboard_html() -> Option<String> {
    arboard::Clipboard::new()
        .ok()?
        .get()
        .html()
        .ok()
        .filter(|h| !h.trim().is_empty())
}

/// HTML from Word or a web page as the notes' Markdown: headings as `## `,
/// bold as `**`, italic as `*`, lists as `- ` and `1. `, paragraphs and
/// line breaks as new lines; everything else dropped, words kept.
pub(super) fn html_to_markdown(html: &str) -> String {
    // only the copied part, when the clipboard marks it
    let html = match (
        html.find("<!--StartFragment-->"),
        html.find("<!--EndFragment-->"),
    ) {
        (Some(a), Some(b)) if b > a => &html[a + "<!--StartFragment-->".len()..b],
        _ => html,
    };
    let mut out = String::new();
    let mut lists: Vec<(bool, u32)> = vec![];
    let mut skip = 0usize;
    let mut i = 0;
    let b = html.as_bytes();
    let line_start = |out: &mut String| {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
    };
    while i < b.len() {
        if html[i..].starts_with("<!--") {
            i = html[i..].find("-->").map_or(b.len(), |e| i + e + 3);
            continue;
        }
        if b[i] == b'<' {
            let end = html[i..].find('>').map_or(b.len(), |e| i + e + 1);
            let tag = html[i + 1..end.saturating_sub(1)]
                .trim()
                .to_ascii_lowercase();
            let close = tag.starts_with('/');
            let name: String = tag
                .trim_start_matches('/')
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == ':')
                .collect();
            let class = tag.contains("mso-list:ignore") || tag.contains("mso-list: ignore");
            match name.as_str() {
                "style" | "script" | "head" | "title" => {
                    if close {
                        skip = skip.saturating_sub(1);
                    } else {
                        skip += 1;
                    }
                }
                "h1" | "h2" | "h3" | "h4" => {
                    line_start(&mut out);
                    if !close {
                        out.push_str("## ");
                    } else {
                        out.push('\n');
                    }
                }
                "b" | "strong" => out.push_str("**"),
                "i" | "em" => out.push('*'),
                "ul" | "ol" => {
                    if close {
                        lists.pop();
                    } else {
                        lists.push((name == "ol", 0));
                    }
                    line_start(&mut out);
                }
                "li" if !close => {
                    line_start(&mut out);
                    let depth = lists.len().saturating_sub(1);
                    out.push_str(&"  ".repeat(depth));
                    match lists.last_mut() {
                        Some((true, n)) => {
                            *n += 1;
                            out.push_str(&format!("{n}. "));
                        }
                        _ => out.push_str("- "),
                    }
                }
                "p" | "div" | "tr" => {
                    line_start(&mut out);
                    if close && name == "p" && !out.ends_with("\n\n") {
                        out.push('\n');
                    }
                }
                "br" => out.push('\n'),
                "td" | "th" if close => out.push(' '),
                // Word writes its list numbers and bullets in a span it
                // marks to be ignored: kept, they become the list mark
                "span" if class && !close => {}
                _ => {}
            }
            i = end;
            continue;
        }
        let next = html[i..].find('<').map_or(b.len(), |e| i + e);
        if skip == 0 {
            let words = html[i..next]
                .replace("&nbsp;", " ")
                .replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&#39;", "'")
                .replace(['\r', '\n'], " ");
            out.push_str(&words);
        }
        i = next;
    }
    // tidy: Word's bullets and spacing, no runs of blank lines, no stray
    // ** round nothing
    let mut lines: Vec<String> = vec![];
    for l in out.lines() {
        let mut l = l.split_whitespace().collect::<Vec<_>>().join(" ");
        for bullet in ["\u{b7} ", "\u{2022} ", "o ", "\u{a7} "] {
            if let Some(r) = l.strip_prefix(bullet) {
                l = format!("- {r}");
            }
        }
        let l = l.replace("****", "").replace("** **", " ");
        if l.is_empty() && lines.last().is_some_and(|p: &String| p.is_empty()) {
            continue;
        }
        lines.push(l);
    }
    lines.join("\n").trim().to_string()
}

/// The model's objects a note can read, matching `q` in their name, mark,
/// IFC class or GlobalId (any case; a pasted GlobalId finds its one
/// object), each with the words its list shows: name, mark, class.
pub(super) fn note_objects(model: &Model, q: &str) -> Vec<(ElementId, String)> {
    let q = q.trim().to_lowercase();
    let mut out: Vec<(ElementId, String)> = model
        .iter()
        .filter(|e| {
            !matches!(
                e.kind,
                ElementKind::View(_)
                    | ElementKind::Note(_)
                    | ElementKind::Tag(_)
                    | ElementKind::Dimension(_)
            ) && e.ifc_class() != "IfcAnnotation"
        })
        .filter_map(|e| {
            let mark = creabim_geom::annotate::note_field(model, e.id, "mark").unwrap_or_default();
            let hay =
                format!("{} {} {} {}", e.name, mark, e.ifc_class(), e.id.ifc_guid()).to_lowercase();
            if !q.is_empty() && !hay.contains(&q) {
                return None;
            }
            let mut words = e.name.clone();
            if !mark.is_empty() && mark != e.name {
                words.push_str(&format!(" · {mark}"));
            }
            words.push_str(&format!(" · {}", e.ifc_class()));
            Some((e.id, words))
        })
        .collect();
    out.sort_by_key(|a| a.1.to_lowercase());
    out
}

#[cfg(test)]
mod tests {
    use super::html_to_markdown;

    #[test]
    fn word_html_becomes_markdown() {
        let html = "<html><head><style>p{}</style></head><body><!--StartFragment-->\
            <h1>General notes</h1><p class=MsoNormal>Use <b>C25/30</b> concrete, <i>unless noted</i>.</p>\
            <ol><li>Do not scale.</li><li>Read with &amp; check.</li></ol>\
            <ul><li>A bullet</li></ul><!--EndFragment--></body></html>";
        let md = html_to_markdown(html);
        assert!(md.starts_with("## General notes"), "{md}");
        assert!(
            md.contains("Use **C25/30** concrete, *unless noted*."),
            "{md}"
        );
        assert!(
            md.contains("1. Do not scale.") && md.contains("2. Read with & check."),
            "{md}"
        );
        assert!(md.contains("- A bullet"), "{md}");
        assert!(!md.contains("p{}"), "styles dropped: {md}");
    }
}
