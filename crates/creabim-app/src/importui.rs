//! The import report: after an IFC file is opened, what came in as native
//! elements and as meshes, the import's notes grouped by kind, and every
//! element it could not bring across natively, by class, name and
//! GlobalId with the reason. The whole report can be copied, to fix the
//! exporting model or to send with a bug report.

use super::*;
use std::collections::BTreeMap;

/// One import's report.
#[derive(Clone, Debug, Default)]
pub(super) struct ImportReport {
    pub open: bool,
    pub file: String,
    pub schema: String,
    /// Per IFC class: elements, and how many of them are meshes.
    pub counts: Vec<(String, usize, usize)>,
    pub summary: Vec<String>,
    pub details: Vec<String>,
    /// For each of `details`, the index of its note in `summary`.
    pub detail_notes: Vec<usize>,
    /// How long the file took to open, when it was timed.
    pub secs: Option<f32>,
}

impl ImportReport {
    pub fn new(file: &str, imported: &creabim_ifc::ImportResult) -> Self {
        Self {
            open: !imported.warnings.is_empty(),
            file: file.to_string(),
            schema: imported.schema.clone(),
            counts: class_counts(&imported.model),
            summary: imported.warnings.clone(),
            details: imported.details.clone(),
            detail_notes: imported.detail_notes.clone(),
            secs: None,
        }
    }

    /// Each note with the elements it names, in the notes' order; elements
    /// whose note isn't known come last, under no note.
    pub fn grouped(&self) -> Vec<(Option<&str>, Vec<&str>)> {
        let mut out: Vec<(Option<&str>, Vec<&str>)> = self
            .summary
            .iter()
            .map(|s| (Some(s.as_str()), Vec::new()))
            .collect();
        let mut loose = Vec::new();
        for (i, d) in self.details.iter().enumerate() {
            match self.detail_notes.get(i).and_then(|n| out.get_mut(*n)) {
                Some(g) => g.1.push(d.as_str()),
                None => loose.push(d.as_str()),
            }
        }
        if !loose.is_empty() {
            out.push((None, loose));
        }
        out
    }

    /// "in 3.6 s", or nothing when it wasn't timed.
    pub fn took(&self) -> String {
        self.secs
            .map(|s| format!(" in {:.1} s", s.max(0.1)))
            .unwrap_or_default()
    }

    /// The report as plain text, as copied: each note, then the elements
    /// it names, indented under it.
    pub fn text(&self) -> String {
        let mut out = format!(
            "Import of {} ({}){}\n\nElements:\n",
            self.file,
            self.schema,
            self.took()
        );
        for (class, n, meshes) in &self.counts {
            out.push_str(&format!("  {class}: {}\n", count_label(*n, *meshes)));
        }
        if !self.summary.is_empty() || !self.details.is_empty() {
            out.push_str("\nNotes:\n");
            for (note, items) in self.grouped() {
                out.push_str(&format!("  {}\n", note.unwrap_or("Other elements")));
                for d in items {
                    out.push_str(&format!("      {d}\n"));
                }
            }
        }
        out
    }
}

/// Elements per IFC class, with how many are meshes; levels, views and
/// the like are left out.
fn class_counts(model: &Model) -> Vec<(String, usize, usize)> {
    let mut c: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for e in model.iter() {
        let class = e.ifc_class();
        if !class.starts_with("Ifc") || class == "IfcBuildingStorey" || class == "IfcGrid" {
            continue;
        }
        let entry = c.entry(class.to_string()).or_default();
        entry.0 += 1;
        entry.1 += usize::from(matches!(e.kind, ElementKind::FreeMesh(_)));
    }
    c.into_iter().map(|(k, (n, m))| (k, n, m)).collect()
}

fn count_label(n: usize, meshes: usize) -> String {
    match (n - meshes, meshes) {
        (_, 0) => format!("{n} native"),
        (0, _) => format!("{n} as meshes"),
        (native, m) => format!("{native} native, {m} as meshes"),
    }
}

impl CreaBimApp {
    /// The report window of the last IFC import.
    pub(super) fn import_report_window(&mut self, ctx: &egui::Context) {
        let area = self.mat.area.unwrap_or_else(|| ctx.content_rect());
        let Some(report) = &mut self.import_report else {
            return;
        };
        if !report.open {
            return;
        }
        let mut open = true;
        let mut copy = false;
        crate::app::shell::window("Import report")
            .open(&mut open)
            .default_pos(area.min + Vec2::new(80.0, 70.0))
            .default_size(Vec2::new(640.0, 460.0))
            .constrain_to(area)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{} ({}){}",
                        report.file,
                        report.schema,
                        report.took()
                    ))
                    .strong(),
                );
                ui.horizontal_wrapped(|ui| {
                    for (class, n, meshes) in &report.counts {
                        let text = format!("{class}: {}", count_label(*n, *meshes));
                        ui.label(egui::RichText::new(text).small());
                        ui.separator();
                    }
                });
                ui.separator();
                if report.summary.is_empty() {
                    ui.label("Everything came in as it was written.");
                }
                // each note, folding open on the elements it names
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for (i, (note, items)) in report.grouped().into_iter().enumerate() {
                            let note = note.unwrap_or("Other elements");
                            if items.is_empty() {
                                ui.label(format!("• {note}"));
                                continue;
                            }
                            egui::CollapsingHeader::new(note)
                                .id_salt(("import-note", i))
                                .show(ui, |ui| {
                                    for d in items {
                                        let text = egui::RichText::new(d).small();
                                        ui.add(egui::Label::new(text).selectable(true));
                                    }
                                });
                        }
                    });
                ui.add_space(6.0);
                crate::props::flow(ui, |ui| {
                    copy = ui
                        .button("Copy report")
                        .on_hover_text(
                            "Copy the whole report as text. File › Import report shows it again",
                        )
                        .clicked();
                });
            });
        if copy {
            ctx.copy_text(report.text());
            self.status = "Import report copied.".into();
        }
        if !open {
            report.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_lists_each_element_under_the_note_it_explains() {
        let r = ImportReport {
            file: "a.ifc".into(),
            schema: "IFC2X3".into(),
            summary: vec![
                "walls as meshes (×2)".into(),
                "spaces left out".into(),
                "doors clipped".into(),
            ],
            details: vec![
                "IfcWall 'a' sloped".into(),
                "IfcDoor 'd' clipped".into(),
                "IfcWall 'b' notched".into(),
            ],
            detail_notes: vec![0, 2, 0],
            secs: Some(3.64),
            ..Default::default()
        };
        let g = r.grouped();
        assert_eq!(g[0].1, ["IfcWall 'a' sloped", "IfcWall 'b' notched"]);
        assert!(g[1].1.is_empty());
        assert_eq!(g[2].1, ["IfcDoor 'd' clipped"]);
        let t = r.text();
        assert!(t.starts_with("Import of a.ifc (IFC2X3) in 3.6 s"), "{t}");
        let walls = t.find("walls as meshes").unwrap();
        let b = t.find("IfcWall 'b' notched").unwrap();
        let spaces = t.find("spaces left out").unwrap();
        assert!(
            walls < b && b < spaces,
            "the second wall sits under its note:\n{t}"
        );
        assert!(!t.contains("Element by element"));
    }

    #[test]
    fn elements_with_no_known_note_still_come_out() {
        let r = ImportReport {
            summary: vec!["x".into()],
            details: vec!["IfcWall 'w'".into()],
            ..Default::default()
        };
        let g = r.grouped();
        assert_eq!(g.last().unwrap().0, None);
        assert!(r.text().contains("IfcWall 'w'"));
    }
}
