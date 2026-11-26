//! `creabim template-set`: one of CreaBIM's template sets made into a
//! project file a new project starts from (Template sets REV01). The
//! work is `creabim_geom::templateset::apply`, the same New project does.

use creabim_core::{Document, Model};

pub fn run(set_path: &str, out: &str, flags: &[&str]) -> Result<(), String> {
    let (us, empty) = (flags.contains(&"us"), flags.contains(&"empty"));
    if let Some(bad) = flags
        .iter()
        .find(|f| !["us", "empty", "plain", "maple"].contains(f))
    {
        return Err(format!(
            "unknown option {bad}; the options are: us, empty, maple, plain"
        ));
    }
    let text = std::fs::read_to_string(set_path).map_err(|e| format!("{set_path}: {e}"))?;
    // onto this version's starter house, unless told the starter of 1.1
    // or an empty project
    let base = if empty {
        Model::default()
    } else if flags.contains(&"maple") {
        creabim_core::starter::starter_house()
    } else {
        creabim_core::ridgeline::ridgeline_house()
    };
    let made = creabim_geom::templateset::apply(&text, base, us, empty, !flags.contains(&"plain"))
        .map_err(|e| format!("{set_path}: {e}"))?;
    // saved from a fresh document, so the file has no history to undo into
    let fresh = Document::from_model(made.model);
    if out.to_ascii_lowercase().ends_with(".json") {
        std::fs::write(out, fresh.model().to_json()).map_err(|e| format!("{out}: {e}"))?;
    } else {
        let _ = std::fs::remove_file(out);
        creabim_store::save(out, &fresh).map_err(|e| format!("{out}: {e}"))?;
    }
    println!(
        "{out}: {} ({}), {} masters",
        made.name,
        if us { "US" } else { "ISO" },
        made.masters
    );
    for r in made.report {
        println!("  {r}");
    }
    Ok(())
}
