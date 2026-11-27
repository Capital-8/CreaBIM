use super::CreaBimApp;
use crate::planview::{self, PlanState, Tool};
use crate::selection::Selection;
use crate::{memberstool, outline};
use creabim_core::{
    polygon, walls, Element, ElementId, ElementKind, Model, Opening, OpeningFrame, OpeningKind,
    Slab, Transaction, TypeCategory,
};
use glam::DVec2;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(untagged)]
enum OpeningRequest {
    Typed(TypedOpeningRequest),
    Sized(SizedOpeningRequest),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TypedOpeningRequest {
    wall_id: ElementId,
    center: f64,
    opening_type_id: ElementId,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SizedOpeningRequest {
    wall_id: ElementId,
    name: String,
    offset: f64,
    width: f64,
    height: f64,
    sill: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpeningEdit {
    element_id: ElementId,
    material_id: Option<ElementId>,
    #[serde(default)]
    clear_material: bool,
    frame_width: Option<f64>,
    frame_depth: Option<f64>,
    flip_hand: Option<bool>,
    flip_facing: Option<bool>,
    panes_across: Option<u32>,
    panes_up: Option<u32>,
}

impl OpeningEdit {
    fn apply_details(&self, model: &Model, opening: &mut Opening) -> Result<(), String> {
        if let Some(id) = self.material_id {
            if self.clear_material {
                return Err("Choose either material_id or clear_material".into());
            }
            if !matches!(
                model.get(id).map(|e| &e.kind),
                Some(ElementKind::Material(_))
            ) {
                return Err("Choose an existing material".into());
            }
            opening.material = Some(id);
        } else if self.clear_material {
            opening.material = None;
        }
        if let Some(width) = self.frame_width {
            if !(0.01..=0.5).contains(&width) {
                return Err("Frame width must be between 0.01 and 0.5 m".into());
            }
            opening.frame.width = width;
        }
        if let Some(depth) = self.frame_depth {
            if !(0.02..=1.0).contains(&depth) {
                return Err("Frame depth must be between 0.02 and 1 m".into());
            }
            opening.frame.depth = depth;
        }
        if self.flip_hand.is_some() && opening.kind != OpeningKind::Door {
            return Err("Only doors have a hinge side".into());
        }
        for (axis, count) in [self.panes_across, self.panes_up].into_iter().enumerate() {
            if let Some(count) = count {
                if opening.kind != OpeningKind::Window || !(1..=12).contains(&count) {
                    return Err("Choose 1 to 12 panes per direction on a window".into());
                }
                for cell in &mut opening.frame.grid.cells {
                    cell.panes[axis] = count;
                }
            }
        }
        if self.flip_hand.is_some_and(|hand| hand != opening.flip_hand) {
            walls::flip_hand(opening);
        }
        if self
            .flip_facing
            .is_some_and(|facing| facing != opening.flip_facing)
        {
            walls::flip_facing(opening);
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateSlabRequest {
    level_id: ElementId,
    boundary: Vec<[f64; 2]>,
    name: Option<String>,
    thickness: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateSlabRequest {
    element_id: ElementId,
    thickness: Option<f64>,
    top_offset: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StairRequest {
    level_id: ElementId,
    start: [f64; 2],
    toward: [f64; 2],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StairEdit {
    element_id: ElementId,
    width: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RailingRequest {
    level_id: ElementId,
    path: Vec<[f64; 2]>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RailingEdit {
    element_id: ElementId,
    height: f64,
}

fn valid_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        Err("Name must not be empty".into())
    } else {
        Ok(())
    }
}

type Operation = fn(&mut CreaBimApp, Value) -> Result<ElementId, String>;

pub(super) fn operation(command: &str) -> Option<Operation> {
    Some(match command {
        "create_slab" => |app, args| {
            app.live_create_slab(serde_json::from_value(args).map_err(|e| e.to_string())?)
        },
        "update_slab" => |app, args| {
            app.live_update_slab(serde_json::from_value(args).map_err(|e| e.to_string())?)
        },
        "create_door" => |app, args| {
            app.create_live_opening(args, Tool::Door, OpeningKind::Door, TypeCategory::Door)
        },
        "create_window" => |app, args| {
            app.create_live_opening(
                args,
                Tool::Window,
                OpeningKind::Window,
                TypeCategory::Window,
            )
        },
        "update_door" => |app, args| app.update_live_opening(args, OpeningKind::Door, "Edit door"),
        "update_window" => {
            |app, args| app.update_live_opening(args, OpeningKind::Window, "Edit window")
        }
        "create_stair" => CreaBimApp::live_create_stair,
        "update_stair" => CreaBimApp::live_update_stair,
        "create_railing" => CreaBimApp::live_railing_create,
        "update_railing" => CreaBimApp::live_railing_update,
        _ => return None,
    })
}

impl CreaBimApp {
    fn create_live_opening(
        &mut self,
        args: Value,
        tool: Tool,
        kind: OpeningKind,
        category: TypeCategory,
    ) -> Result<ElementId, String> {
        let request: OpeningRequest = serde_json::from_value(args).map_err(|e| e.to_string())?;
        let model = self.doc.model();
        let host = match &request {
            OpeningRequest::Typed(r) => r.wall_id,
            OpeningRequest::Sized(r) => r.wall_id,
        };
        let Some(ElementKind::Wall(wall)) = model.get(host).map(|e| &e.kind) else {
            return Err("The opening host must be an existing wall".into());
        };
        let wall_height = walls::constrained_height(model, wall).unwrap_or(wall.height);
        let element = match request {
            OpeningRequest::Typed(r) => {
                if !(0.0..=wall.length()).contains(&r.center) {
                    return Err("The opening centre must be along its wall".into());
                }
                if !matches!(model.get(r.opening_type_id).map(|e| &e.kind),
                    Some(ElementKind::ElementType(t)) if t.category() == category)
                {
                    return Err("Choose a type for the opening from live_inspect".into());
                }
                let mut state = PlanState::default();
                state.types.insert(category, r.opening_type_id);
                planview::opening_on_wall(model, r.wall_id, r.center, tool, &state)
                    .ok_or("The opening must fit within its wall")?
            }
            OpeningRequest::Sized(r) => {
                valid_name(&r.name)?;
                if ![r.offset, r.sill, r.width, r.height]
                    .into_iter()
                    .all(f64::is_finite)
                    || r.offset < 0.0
                    || r.sill < 0.0
                    || r.width <= 0.0
                    || r.height <= 0.0
                    || r.offset + r.width > wall.length() + 1e-9
                    || r.sill + r.height > wall_height + 1e-9
                {
                    return Err(
                        "Opening must have positive dimensions and fit within its wall".into(),
                    );
                }
                let opening = Opening {
                    host,
                    kind,
                    offset: r.offset,
                    sill: r.sill,
                    width: r.width,
                    height: r.height,
                    material: None,
                    frame: OpeningFrame::for_kind(kind),
                    flip_hand: false,
                    flip_facing: false,
                    roof_at: None,
                    roof_build: Default::default(),
                    arrangement: None,
                };
                if model.iter().any(|e| {
                    matches!(&e.kind, ElementKind::Opening(other)
                        if other.host == host && r.offset < other.offset + other.width - 1e-9
                            && r.offset + r.width > other.offset + 1e-9
                            && r.sill < other.sill + other.height - 1e-9
                            && r.sill + r.height > other.sill + 1e-9
                    )
                }) {
                    return Err("Opening overlaps an existing opening in this wall".into());
                }
                Element::new(r.name, ElementKind::Opening(opening))
            }
        };
        if let ElementKind::Opening(opening) = &element.kind {
            if !opening.sill.is_finite() || opening.sill + opening.height > wall_height + 1e-9 {
                return Err("The opening must fit within its wall".into());
            }
        }
        let id = element.id;
        self.doc
            .commit(
                Transaction::new(format!("Place {}", tool.label().to_lowercase())).insert(element),
            )
            .map_err(|e| e.to_string())?;
        Ok(id)
    }

    fn update_live_opening(
        &mut self,
        args: Value,
        kind: OpeningKind,
        label: &str,
    ) -> Result<ElementId, String> {
        let edit: OpeningEdit = serde_json::from_value(args).map_err(|e| e.to_string())?;
        let model = self.doc.model();
        let Some(ElementKind::Opening(opening)) = model.get(edit.element_id).map(|e| &e.kind)
        else {
            return Err("Choose an opening of the requested kind".into());
        };
        if opening.kind != kind {
            return Err("Choose an opening of the requested kind".into());
        }
        let mut updated = opening.clone();
        edit.apply_details(model, &mut updated)?;
        self.doc
            .edit(edit.element_id, label, |e| {
                e.kind = ElementKind::Opening(updated)
            })
            .map_err(|e| e.to_string())?;
        Ok(edit.element_id)
    }

    fn live_create_slab(&mut self, request: CreateSlabRequest) -> Result<ElementId, String> {
        if self.doc.model().level(request.level_id).is_none() {
            return Err("Choose a level from live_inspect".into());
        }
        if !(3..=1000).contains(&request.boundary.len()) {
            return Err("A slab outline needs 3 through 1000 corners".into());
        }
        let points: Vec<_> = request
            .boundary
            .into_iter()
            .map(DVec2::from_array)
            .collect();
        if points.iter().enumerate().any(|(i, p)| {
            !p.is_finite()
                || p.abs().max_element() > 1e9
                || points[..i].iter().any(|q| p.distance_squared(*q) < 1e-12)
        }) || polygon::signed_area2(&points).abs() < 1e-8
            || polygon::self_intersects(&points, true)
        {
            return Err(
                "Give a simple slab polygon with distinct finite corners within ±1e9 m".into(),
            );
        }
        let name = request.name.as_deref().unwrap_or("Slab");
        valid_name(name)?;
        let thickness = request.thickness.unwrap_or(outline::NEW_SLAB_THICKNESS);
        if !thickness.is_finite() || thickness <= 0.0 {
            return Err("Slab thickness must be positive and finite".into());
        }
        let mut slab = Slab::new(request.level_id, points, thickness);
        slab.make_ccw();
        let e = Element::new(name, ElementKind::Slab(slab));
        let id = e.id;
        self.doc
            .commit(Transaction::new("Draw slab").insert(e))
            .map_err(|e| e.to_string())?;
        Ok(id)
    }

    fn live_update_slab(&mut self, request: UpdateSlabRequest) -> Result<ElementId, String> {
        let id = request.element_id;
        let Some(ElementKind::Slab(slab)) = self.doc.model().get(id).map(|e| &e.kind) else {
            return Err("Choose an existing slab".into());
        };
        if request.thickness.is_none() && request.top_offset.is_none() {
            return Err("Specify a slab thickness or top offset".into());
        }
        let mut edited = slab.clone();
        if let Some(thickness) = request.thickness {
            if slab.assembly.is_some() {
                return Err("A layered slab's thickness is set by its assembly".into());
            }
            if !(0.02..=3.0).contains(&thickness) {
                return Err("Slab thickness must be between 0.02 and 3 m".into());
            }
            edited.set_thickness(thickness, false);
        }
        if let Some(offset) = request.top_offset {
            if !(-100.0..=100.0).contains(&offset) {
                return Err("Slab top offset must be within ±100 m".into());
            }
            edited.top_offset = offset;
        }
        self.doc
            .edit(id, "Edit slab", |element| {
                element.kind = ElementKind::Slab(edited)
            })
            .map_err(|e| e.to_string())?;
        Ok(id)
    }

    fn live_create_stair(&mut self, args: Value) -> Result<ElementId, String> {
        let request: StairRequest = serde_json::from_value(args).map_err(|e| e.to_string())?;
        let model = self.doc.model();
        if model.level(request.level_id).is_none() {
            return Err("Choose a level from live_inspect".into());
        }
        let start = DVec2::from_array(request.start);
        let toward = DVec2::from_array(request.toward);
        if !request
            .start
            .into_iter()
            .chain(request.toward)
            .all(|v| v.abs() <= 1e9)
            || start.distance(toward) <= 0.05
        {
            return Err(
                "Stair points must be finite and within ±1e9 m, and more than 0.05 m apart".into(),
            );
        }
        let mut state = PlanState::default();
        state.members.stair_line = vec![start, toward];
        let mut selection = Selection::default();
        memberstool::finish_stair(&mut self.doc, request.level_id, &mut state, &mut selection)
            .map_err(|e| e.to_string())?;
        selection
            .active
            .ok_or_else(|| "The Stair tool did not place a stair".into())
    }

    fn live_update_stair(&mut self, args: Value) -> Result<ElementId, String> {
        let request: StairEdit = serde_json::from_value(args).map_err(|e| e.to_string())?;
        if !matches!(
            self.doc.model().get(request.element_id).map(|e| &e.kind),
            Some(ElementKind::Stair(stair)) if stair.landing.is_none()
                && stair.line.len() <= 2 && stair.bends.iter().all(|bend| bend.abs() < 1e-9)
                && !stair.is_ramp() && !stair.split
        ) {
            return Err("Choose a straight stair without landings".into());
        }
        if !(0.5..=5.0).contains(&request.width) {
            return Err("Stair width must be between 0.5 and 5 m".into());
        }
        self.doc
            .edit(request.element_id, "Stair width", |element| {
                if let ElementKind::Stair(stair) = &mut element.kind {
                    stair.width = request.width;
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(request.element_id)
    }

    fn live_railing_create(&mut self, args: Value) -> Result<ElementId, String> {
        let railing: RailingRequest = serde_json::from_value(args).map_err(|e| e.to_string())?;
        if self.doc.model().level(railing.level_id).is_none() {
            return Err("Choose a level from live_inspect".into());
        }
        let path: Vec<_> = railing.path.into_iter().map(DVec2::from_array).collect();
        if path.len() < 2 || path.iter().any(|p| !(p.abs().max_element() <= 1e9)) {
            return Err("Provide at least two railing points within ±1e9 m".into());
        }
        let mut length = 0.0;
        for pair in path.windows(2) {
            let span = pair[0].distance(pair[1]);
            if span < 0.05 {
                return Err("Successive railing points must be at least 0.05 m apart".into());
            }
            length += span;
        }
        // Posts are allocated for the whole path when the scene rebuilds.
        if length / 1.2 + path.len() as f64 > 10_000.0 {
            return Err("The railing path would create too many posts".into());
        }
        let mut state = PlanState::default();
        let last = *path.last().unwrap();
        state.members.path = path;
        let mut selection = Selection::default();
        memberstool::click(
            &mut self.doc,
            railing.level_id,
            Tool::Railing,
            last,
            &mut state,
            &mut selection,
        )
        .map_err(|e| e.to_string())?;
        selection
            .active
            .ok_or_else(|| "Railing was not created".into())
    }

    fn live_railing_update(&mut self, args: Value) -> Result<ElementId, String> {
        let edit: RailingEdit = serde_json::from_value(args).map_err(|e| e.to_string())?;
        if !matches!(
            self.doc.model().get(edit.element_id).map(|e| &e.kind),
            Some(ElementKind::Railing(_))
        ) {
            return Err("Choose a railing element to update".into());
        }
        if !(0.2..=3.0).contains(&edit.height) {
            return Err("Railing height must be between 0.2 and 3.0 m".into());
        }
        self.doc
            .edit(edit.element_id, "Edit railing", |element| {
                if let ElementKind::Railing(railing) = &mut element.kind {
                    railing.height = edit.height;
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(edit.element_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::test_app;
    use serde_json::json;

    fn edit(app: &mut CreaBimApp, command: &str, args: Value) -> Element {
        let info = app
            .live_command("inspect_document", json!({}), None, None)
            .unwrap();
        let before = app.doc.undo_steps().len();
        let reply = app
            .live_command(
                command,
                args,
                info["document_id"].as_str(),
                info["revision"].as_u64(),
            )
            .unwrap_or_else(|error| panic!("{command}: {error}"));
        assert_eq!(app.doc.undo_steps().len(), before + 1, "{command}");
        serde_json::from_value(reply["element"].clone()).unwrap()
    }

    #[test]
    fn live_building_element_edits_use_native_undo() {
        let mut app = test_app();
        let info = app
            .live_command("inspect_document", json!({}), None, None)
            .unwrap();
        let level = &info["levels"][0]["id"];
        let wall = edit(
            &mut app,
            "create_wall",
            json!({"level_id": level, "start": [80.0, 80.0], "end": [86.0, 80.0]}),
        );
        let door = edit(
            &mut app,
            "create_door",
            json!({"wall_id": wall.id, "name": "MCP door", "offset": 0.7,
                   "width": 0.9, "height": 2.1, "sill": 0.0}),
        );
        let door = edit(
            &mut app,
            "update_door",
            json!({"element_id": door.id, "frame_width": 0.07, "flip_hand": true}),
        );
        assert!(matches!(door.kind, ElementKind::Opening(o)
            if o.host == wall.id && o.kind == OpeningKind::Door && o.flip_hand && o.frame.width == 0.07));

        let window = edit(
            &mut app,
            "create_window",
            json!({"wall_id": wall.id, "name": "MCP window", "offset": 3.0,
                   "width": 1.2, "height": 1.0, "sill": 0.8}),
        );
        let window = edit(
            &mut app,
            "update_window",
            json!({"element_id": window.id, "frame_depth": 0.1, "panes_across": 3}),
        );
        assert!(matches!(window.kind, ElementKind::Opening(o)
            if o.host == wall.id && o.kind == OpeningKind::Window && o.frame.depth == 0.1
                && !o.frame.grid.cells.is_empty() && o.frame.grid.cells.iter().all(|cell| cell.panes[0] == 3)));

        let slab_args = json!({"level_id": level, "boundary": [[80.0, 81.0], [85.0, 81.0],
                                                               [85.0, 86.0], [80.0, 86.0]]});
        let stale = app.live_command(
            "create_slab",
            slab_args.clone(),
            info["document_id"].as_str(),
            info["revision"].as_u64(),
        );
        assert!(stale.unwrap_err().contains("revision changed"));
        let slab = edit(&mut app, "create_slab", slab_args);
        let slab = edit(
            &mut app,
            "update_slab",
            json!({"element_id": slab.id, "thickness": 0.28, "top_offset": 0.12}),
        );
        assert!(matches!(slab.kind, ElementKind::Slab(s)
            if (s.thickness - 0.28).abs() < 1e-9 && s.top_offset == 0.12));

        let stair = edit(
            &mut app,
            "create_stair",
            json!({"level_id": level, "start": [88.0, 81.0], "toward": [88.0, 87.0]}),
        );
        let stair = edit(
            &mut app,
            "update_stair",
            json!({"element_id": stair.id, "width": 1.35}),
        );
        assert!(matches!(stair.kind, ElementKind::Stair(s) if s.width == 1.35));

        let railing = edit(
            &mut app,
            "create_railing",
            json!({"level_id": level, "path": [[80.0, 86.0], [85.0, 86.0], [85.0, 81.0]]}),
        );
        let updated = edit(
            &mut app,
            "update_railing",
            json!({"element_id": railing.id, "height": 1.1}),
        );
        assert!(matches!(updated.kind, ElementKind::Railing(r) if r.height == 1.1));
        assert!(app.doc.undo());
        assert_eq!(app.doc.model().get(railing.id), Some(&railing));
    }
}
