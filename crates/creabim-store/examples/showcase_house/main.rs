//! Ridgeline House, saved as a milestone after each stage it is built in:
//!
//! `cargo run -p creabim-store --example showcase_house -- <folder>`
//!
//! writes `01_site.creabim` … `06_sheets.creabim` and
//! `Ridgeline House.creabim` into the folder. A stage's name as a second
//! word stops after it (`site`, `structure`, `envelope`, `interiors`,
//! `details`). The house itself is `creabim_core::ridgeline`, the same
//! code *New project* opens the starter house with.

use creabim_core::ridgeline::{self, Builder};

fn main() {
    let dir = std::env::args().nth(1).expect("the folder to write into");
    let until = std::env::args().nth(2).unwrap_or_default();
    std::fs::create_dir_all(&dir).unwrap();
    let save = |b: &Builder, name: &str| {
        let path = format!("{dir}/{name}.creabim");
        let _ = std::fs::remove_file(&path);
        creabim_store::save(&path, &b.doc).unwrap();
        println!("saved {path}  ({} elements)", b.doc.model().len());
    };
    let b = ridgeline::build(&until, |b, stage| {
        let file = ridgeline::STAGES.iter().find(|s| s.0 == stage).unwrap().1;
        save(b, file);
    });
    if !until.is_empty() && until != "sheets" {
        return;
    }
    save(&b, ridgeline::PROJECT);
    // nothing may sit exactly on a copy of itself
    for id in creabim_core::checks::stacked_copies(b.doc.model()) {
        let e = b.doc.model().get(id).unwrap();
        println!(
            "  note: a stacked copy: {} ({:?})",
            e.name,
            std::mem::discriminant(&e.kind)
        );
    }
}
