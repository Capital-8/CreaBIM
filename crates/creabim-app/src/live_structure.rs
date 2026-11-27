use creabim_core::{Beam, Column, Document, Element, ElementId, ElementKind, Profile, Transaction};
use glam::DVec2;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LevelArgs {
    name: String,
    elevation: f64,
    height: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ColumnArgs {
    name: String,
    level_id: ElementId,
    position: [f64; 2],
    width: f64,
    depth: f64,
    height: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BeamArgs {
    name: String,
    level_id: ElementId,
    start: [f64; 2],
    end: [f64; 2],
    width: f64,
    depth: f64,
    #[serde(default)]
    top_offset: f64,
}

pub(super) fn handles(command: &str) -> bool {
    matches!(command, "create_level" | "create_column" | "create_beam")
}

fn parse<T: DeserializeOwned>(args: Value) -> Result<T, String> {
    serde_json::from_value(args).map_err(|e| format!("Invalid arguments: {e}"))
}

fn valid_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        Err("Name must not be empty".into())
    } else {
        Ok(())
    }
}

fn finite(values: &[f64]) -> Result<(), String> {
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err("Numbers must be finite".into())
    }
}

fn level_elevation(doc: &Document, id: ElementId) -> Result<f64, String> {
    doc.model()
        .level(id)
        .map(|level| level.elevation)
        .ok_or_else(|| "level_id must refer to an existing level".into())
}

fn check_frame(points: &[DVec2], radius: f64) -> Result<(), String> {
    finite(&[radius])?;
    for p in points {
        finite(&[p.x - radius, p.x + radius, p.y - radius, p.y + radius])?;
    }
    Ok(())
}

fn commit_insert(doc: &mut Document, label: &str, element: Element) -> Result<Value, String> {
    element.validate()?;
    let id = element.id;
    doc.commit(Transaction::new(label).insert(element))
        .map_err(|e| e.to_string())?;
    let created = doc.model().get(id).ok_or("Created element was not found")?;
    Ok(json!({"id": id, "element": created}))
}

pub(super) fn execute(doc: &mut Document, command: &str, args: Value) -> Result<Value, String> {
    match command {
        "create_level" => {
            let a: LevelArgs = parse(args)?;
            valid_name(&a.name)?;
            finite(&[a.elevation, a.height, a.elevation + a.height])?;
            if a.elevation + a.height == a.elevation {
                return Err("Level height cannot be represented at this elevation".into());
            }
            for (_, level) in doc.model().levels() {
                finite(&[level.elevation - a.elevation, a.elevation - level.elevation])?;
            }
            let (mut tx, id, plan) =
                creabim_core::levels::add(doc.model(), &a.name, a.elevation, a.height, true);
            tx.label = "Create level (MCP)".into();
            doc.commit(tx).map_err(|e| e.to_string())?;
            let element = doc.model().get(id).ok_or("Created level was not found")?;
            Ok(json!({"id": id, "element": element, "plan_view_id": plan}))
        }
        "create_column" => {
            let a: ColumnArgs = parse(args)?;
            valid_name(&a.name)?;
            let elevation = level_elevation(doc, a.level_id)?;
            let position = DVec2::from_array(a.position);
            check_frame(&[position], a.width.hypot(a.depth) / 2.0)?;
            finite(&[a.height, elevation + a.height])?;
            let mut column = Column::new(
                a.level_id,
                position,
                Profile::Rectangle {
                    width: a.width,
                    depth: a.depth,
                },
            );
            column.height = Some(a.height);
            commit_insert(
                doc,
                "Create column (MCP)",
                Element::new(a.name, ElementKind::Column(column)),
            )
        }
        "create_beam" => {
            let a: BeamArgs = parse(args)?;
            valid_name(&a.name)?;
            level_elevation(doc, a.level_id)?;
            let start = DVec2::from_array(a.start);
            let end = DVec2::from_array(a.end);
            check_frame(&[start, end], a.width / 2.0)?;
            let mut beam = Beam::new(a.level_id, start, end);
            beam.width = a.width;
            beam.depth = a.depth;
            beam.top_offset = a.top_offset;
            let top = beam.top(doc.model());
            finite(&[top, top - beam.depth])?;
            commit_insert(
                doc,
                "Create beam (MCP)",
                Element::new(a.name, ElementKind::Beam(beam)),
            )
        }
        _ => Err("Unknown live MCP structure command".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::test_app;
    use crate::app::{phasesui::PhaseOf, PlanPlacement};
    use creabim_core::phases;

    #[test]
    fn live_structure_respects_document_guards_and_active_phase() {
        let mut app = test_app();
        app.doc
            .commit(phases::start(app.doc.model(), true).unwrap())
            .unwrap();
        let (tx, [_, option]) = phases::add_question(app.doc.model(), "Extension").unwrap();
        app.doc.commit(tx).unwrap();
        let plan = PhaseOf::View(app.active_view.unwrap());
        app.placement = PlanPlacement::Overlay;
        app.set_phase(PhaseOf::Window3d, 3);
        app.set_option(PhaseOf::Window3d, option, true);
        app.set_phase(plan, 2);
        app.work_in_phase(plan);
        assert_eq!(app.doc.working_phase(), Some(2));
        assert_eq!(app.doc.working_option(), None);

        let info = app
            .live_command("inspect_document", json!({}), None, None)
            .unwrap();
        let document = info["document_id"].as_str();
        let mut revision = info["revision"].as_u64();
        let level = app
            .live_command(
                "create_level",
                json!({"name": "Mezzanine", "elevation": 4.0, "height": 3.2}),
                document,
                revision,
            )
            .unwrap();
        assert!(level["plan_view_id"].is_string());
        let column_args = json!({"level_id": level["id"], "name": "MCP column",
            "position": [8, 0], "width": 0.3, "depth": 0.3, "height": 2.6});
        assert!(app
            .live_command("create_column", column_args.clone(), document, revision)
            .is_err());

        revision = level["revision"].as_u64();
        let column = app
            .live_command("create_column", column_args, document, revision)
            .unwrap();
        let element: Element = serde_json::from_value(column["element"].clone()).unwrap();
        assert_eq!(
            (element.phase.built, element.phase.option),
            (Some(3), Some(option))
        );
        let beam = app
            .live_command(
                "create_beam",
                json!({"level_id": level["id"], "name": "MCP beam",
                    "start": [8, 0], "end": [12, 0], "width": 0.3, "depth": 0.4}),
                document,
                column["revision"].as_u64(),
            )
            .unwrap();
        let beam_id = serde_json::from_value(beam["id"].clone()).unwrap();
        assert!(app.doc.model().get(beam_id).is_some());
        assert!(app.doc.undo());
        assert!(app.doc.model().get(beam_id).is_none());
    }
}
