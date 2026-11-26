//! Data (IFC) › Documents (Element documents REV00, owner: A on all five):
//! an element's data sheets, specs and links, its own, its type's and the
//! file's, each opened with one click; a file or a link added; its own
//! removed. Information only: CreaBIM keeps where a document is, never
//! what is in it.

use super::*;
use creabim_core::docrefs::{self, DocRef, From, Target};

impl CreaBimApp {
    /// The project's folder: where a document's path from the project
    /// starts. None until the project is saved.
    fn project_dir(&self) -> Option<std::path::PathBuf> {
        let p = std::path::Path::new(&self.path);
        (!self.path.trim().is_empty())
            .then(|| p.parent().map(|d| d.to_path_buf()))
            .flatten()
    }

    /// The Documents fold of element `e`'s Data (IFC) tab.
    pub(super) fn documents_section(&mut self, ui: &mut egui::Ui, e: &Element) {
        let docs = docrefs::of(self.doc.model(), e);
        let open = self.data_heading(ui, "data-docs", "Documents", None);
        let dir = self.project_dir();
        let mut remove: Option<usize> = None;
        let mut add: Option<DocRef> = None;
        let mut opened: Option<Result<(), String>> = None;
        crate::props::fold(ui, open, |ui| {
            if docs.is_empty() {
                crate::props::plain_note(ui, "None yet. Add a data sheet, a spec or a link.");
            }
            let mut own = 0usize;
            for (d, from) in &docs {
                let target = d.target(dir.as_deref());
                let missing = matches!(&target, Target::File(p) if !p.exists());
                let mine = *from == From::Own;
                let at = own;
                if mine {
                    own += 1;
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let icon = if d.is_link() { "link" } else { "file-text" };
                    let tip = if d.is_link() { "Open in your browser" } else { "Open with its program" };
                    let mut go = crate::props::icon_button(ui, icon, tip).clicked();
                    // the remove button on the right, then the text in
                    // what is left
                    let w = (ui.available_width() - 24.0).max(40.0);
                    ui.allocate_ui_with_layout(egui::vec2(w, 34.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let name = ui
                            .add(egui::Label::new(egui::RichText::new(&d.name)).truncate().sense(egui::Sense::click()))
                            .on_hover_text(format!("{}\n{tip}", d.name));
                        go |= name.clicked();
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 5.0;
                            match from {
                                From::File => {
                                    crate::props::chip(ui, "From the file", false)
                                        .on_hover_text("Came in with the IFC file: opened, not removed, written back as it came");
                                }
                                From::Type => {
                                    crate::props::chip(ui, "Type", true)
                                        .on_hover_text("Its type's: every element of the type has it. Changed on the type.");
                                }
                                From::Own => {}
                            }
                            // the file's own name (a link's site), the
                            // whole location on hover: a folder alone said
                            // nothing at the panel's width
                            let short = if d.is_link() {
                                d.location.split_once("://").map_or(d.location.as_str(), |x| x.1).trim_start_matches("www.").to_string()
                            } else {
                                d.location.rsplit(['/', '\\']).next().unwrap_or(&d.location).to_string()
                            };
                            let loc = if missing { format!("not found · {short}") } else { short };
                            let mut text = egui::RichText::new(loc).small();
                            text = if missing { text.color(ui.visuals().warn_fg_color) } else { text.weak() };
                            ui.add(egui::Label::new(text).truncate()).on_hover_text(if missing {
                                "Not where the record says: still written to IFC, as it may be beside the file on another machine"
                            } else {
                                d.location.as_str()
                            });
                        });
                    });
                    if mine
                        && crate::props::icon_button(ui, "x", "Remove this document").clicked() {
                            remove = Some(at);
                        }
                    if go {
                        opened = Some(open_target(ui.ctx(), &target));
                    }
                });
            }
            ui.add_space(4.0);
            let link_id = ui.id().with(("doc-link", e.id));
            let mut link: Option<String> = ui.data(|m| m.get_temp(link_id));
            let (file, links) = crate::props::actions2(
                ui,
                "Add",
                ("File…", "Pick a file: a PDF, a drawing, a photo", true),
                (
                    "Link…",
                    "A web address: a maker's page, a spec online",
                    true,
                ),
            );
            if file {
                if let Some(p) = crate::files::pick_document(dir.as_deref()) {
                    add = Some(DocRef::from_file(std::path::Path::new(&p), dir.as_deref()));
                }
            }
            if links {
                link = if link.is_some() {
                    None
                } else {
                    Some(String::new())
                };
            }
            if let Some(text) = link.as_mut() {
                // the address at the panel's full width, its own id so it
                // keeps the keyboard from frame to frame
                let done = ui
                    .horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let w = (ui.available_width() - 48.0).max(40.0);
                        let edit = egui::TextEdit::singleline(text)
                            .id(link_id.with("field"))
                            .hint_text("https://…");
                        let r = crate::props::text_field(ui, edit, w);
                        if links {
                            r.request_focus();
                        }
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        ui.add_enabled(!text.trim().is_empty(), egui::Button::new("Add"))
                            .clicked()
                            || enter
                    })
                    .inner;
                if done && !text.trim().is_empty() {
                    add = Some(DocRef::from_link(text));
                    link = None;
                }
            }
            ui.data_mut(|m| match &link {
                Some(t) => {
                    m.insert_temp(link_id, t.clone());
                }
                None => m.remove::<String>(link_id),
            });
            crate::props::info(
                ui,
                "Opened with your own programs. A file inside the project folder is kept by its path from the project, so it moves with it.",
                "ifc",
            );
        });
        if let Some(i) = remove {
            let _ = self.doc.edit(e.id, "Remove document", |x| {
                if i < x.documents.len() {
                    x.documents.remove(i);
                }
            });
        }
        if let Some(d) = add {
            let _ = self.doc.edit(e.id, "Add document", |x| x.documents.push(d));
        }
        if let Some(Err(err)) = opened {
            self.report(Err::<(), _>(err), "");
        }
    }
}

/// Open a document: a link in the browser, a file with its program.
fn open_target(ctx: &egui::Context, t: &Target) -> Result<(), String> {
    match t {
        Target::Link(url) => {
            ctx.open_url(egui::OpenUrl::new_tab(url));
            Ok(())
        }
        Target::File(p) if !p.exists() => Err(format!("Not found: {}", p.display())),
        Target::File(p) => crate::files::open_file(p),
    }
}
