//! Tool families (tool families mockup REV04, decided 2026-11-15, MU57):
//! related tools behind one split button on the tools row. The button is
//! the family's last tool used (one click brings it back) and stays lit
//! while one of its tools is on; its caret opens a drop-down panel
//! (AMD-011) with headings, icons and keys. A pin keeps a member on the
//! bar beside its family.
//!
//! The rule the families serve: the tools row says *what* you make, the
//! options strip under it says *how* (Draw by) and the values.

use crate::partstool::{GroundDraw, AREAS};
use crate::planview::{PlanState, Tool};
use creabim_core::PlantForm;

/// A family of tools on the tools row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Structure,
    Stairs,
    Furnishing,
    Drafting,
    Rooms,
    Site,
}

/// One tool of a family, as its panel lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    /// The action that starts it (see `run_action`).
    pub id: String,
    pub label: String,
    pub icon: &'static str,
    /// Its key, where it has one.
    pub key: &'static str,
    pub hint: String,
}

fn tool_id(t: Tool) -> String {
    format!(
        "tool:{}",
        Tool::ALL.iter().position(|x| *x == t).unwrap_or(0)
    )
}

fn tool_member(t: Tool) -> Member {
    Member {
        id: tool_id(t),
        label: t.label().to_string(),
        icon: crate::app::shell::tool_icon(t),
        key: "",
        hint: t.hint().to_string(),
    }
}

fn member(id: &str, label: &str, icon: &'static str, key: &'static str, hint: &str) -> Member {
    Member {
        id: id.to_string(),
        label: label.to_string(),
        icon,
        key,
        hint: hint.to_string(),
    }
}

fn ground_icon(g: GroundDraw) -> &'static str {
    match g {
        GroundDraw::Outline => "terrain",
        GroundDraw::Contour => "contour",
        GroundDraw::Spot => "spot-level",
        GroundDraw::Property => "land-plot",
        GroundDraw::Pad => "pad",
        GroundDraw::Pit => "pit",
        GroundDraw::Trench => "trench",
        GroundDraw::Road => "road",
        GroundDraw::Path => "site-path",
        GroundDraw::Drain => "drain",
        GroundDraw::Tank => "tank",
        GroundDraw::Investigation => "circle-dot",
    }
}

fn ground_member(g: GroundDraw) -> Member {
    // the terrain itself reads "Ground" in the Site panel (REV04)
    let label = match g {
        GroundDraw::Outline => "Ground",
        g => g.label(),
    };
    member(
        &format!("site:ground.{}", g.label()),
        label,
        ground_icon(g),
        "",
        g.hint(),
    )
}

fn plant_icon(p: PlantForm) -> &'static str {
    match p {
        PlantForm::Tree => "plant-tree",
        PlantForm::Conifer => "plant-conifer",
        PlantForm::Palm => "plant-palm",
        PlantForm::Shrub => "plant-shrub",
    }
}

impl Family {
    pub const ALL: [Family; 6] = [
        Family::Structure,
        Family::Stairs,
        Family::Furnishing,
        Family::Drafting,
        Family::Rooms,
        Family::Site,
    ];

    /// Its popup's key and the settings' key.
    pub fn key(self) -> &'static str {
        match self {
            Family::Structure => "structure",
            Family::Stairs => "stairs",
            Family::Furnishing => "furnishing",
            Family::Drafting => "drafting",
            Family::Rooms => "rooms",
            Family::Site => "site",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Family::Structure => "Structure",
            Family::Stairs => "Stairs",
            Family::Furnishing => "Furnishing",
            Family::Drafting => "Drafting",
            Family::Rooms => "Rooms",
            Family::Site => "Site",
        }
    }

    pub fn from_key(k: &str) -> Option<Family> {
        Family::ALL.into_iter().find(|f| f.key() == k)
    }

    /// Its tools, under their headings ("" for none), in panel order.
    pub fn groups(self) -> Vec<(&'static str, Vec<Member>)> {
        match self {
            Family::Structure => vec![(
                "",
                [Tool::Column, Tool::Beam, Tool::Footing]
                    .into_iter()
                    .map(tool_member)
                    .collect(),
            )],
            // MU58: Room and Room separator share a button
            Family::Rooms => vec![(
                "",
                [Tool::Room, Tool::RoomSeparator]
                    .into_iter()
                    .map(tool_member)
                    .collect(),
            )],
            Family::Stairs => vec![(
                "",
                [Tool::Stair, Tool::Railing].into_iter().map(tool_member).collect(),
            )],
            Family::Furnishing => vec![(
                "",
                vec![
                    tool_member(Tool::Object),
                    member(
                        "catalogue",
                        "Catalogue",
                        "sofa",
                        "",
                        "Furniture and fixtures to place, and your own from Blender, FreeCAD or glTF",
                    ),
                ],
            )],
            Family::Drafting => vec![(
                "",
                vec![
                    member(
                        "shape:line",
                        "Line",
                        "shape-line",
                        "L",
                        "Draw lines, click after click; back on the first point closes the outline",
                    ),
                    member(
                        "shape:arc",
                        "Arc",
                        "spline",
                        "A",
                        "Draw an arc: its two ends, then a point on it",
                    ),
                    member(
                        "shape:rect",
                        "Rectangle",
                        "shape-rect",
                        "R",
                        "Draw a rectangle by two opposite corners",
                    ),
                    member(
                        "shape:circle",
                        "Circle",
                        "circle-dot",
                        "C",
                        "Draw a circle: its centre, then a point on it",
                    ),
                ],
            )],
            Family::Site => {
                let g = |list: &[GroundDraw]| list.iter().map(|g| ground_member(*g)).collect();
                vec![
                    (
                        "Ground",
                        g(&[GroundDraw::Outline, GroundDraw::Contour, GroundDraw::Spot, GroundDraw::Property, GroundDraw::Investigation]),
                    ),
                    (
                        "Earthworks",
                        g(&[GroundDraw::Pad, GroundDraw::Pit, GroundDraw::Trench]),
                    ),
                    ("Ways", g(&[GroundDraw::Road, GroundDraw::Path])),
                    (
                        "Hard landscape",
                        AREAS
                            .iter()
                            .enumerate()
                            .map(|(i, a)| {
                                member(
                                    &format!("site:area.{i}"),
                                    a.label,
                                    a.icon,
                                    "",
                                    if a.parking {
                                        "A car park: draw its outline and it lays its own aisles and bays"
                                    } else {
                                        "An area of paving, gravel or decking: draw its outline"
                                    },
                                )
                            })
                            .collect(),
                    ),
                    ("Services", g(&[GroundDraw::Drain, GroundDraw::Tank])),
                    (
                        "Planting",
                        PlantForm::ALL
                            .iter()
                            .map(|p| {
                                member(
                                    &format!("site:plant.{}", p.label()),
                                    p.label(),
                                    plant_icon(*p),
                                    "",
                                    "Click where it grows; its crown and height are in the strip and in its Tree tab",
                                )
                            })
                            .collect(),
                    ),
                ]
            }
        }
    }

    /// Every member, in order.
    pub fn members(self) -> Vec<Member> {
        self.groups().into_iter().flat_map(|(_, m)| m).collect()
    }

    /// The member shown on the button when none was used yet.
    pub fn first(self) -> Member {
        self.members().remove(0)
    }

    /// The family a member's action belongs to.
    pub fn of_action(id: &str) -> Option<Family> {
        Family::ALL
            .into_iter()
            .find(|f| f.members().iter().any(|m| m.id == id))
    }

    /// The member in use now, if a tool of this family is on: the tool,
    /// and for the Site and Drafting tools what they draw.
    pub fn active(self, plan: &PlanState, shape: crate::sketch::Shape) -> Option<String> {
        use crate::sketch::Shape;
        let t = plan.tool;
        let id = match self {
            Family::Structure if matches!(t, Tool::Column | Tool::Beam | Tool::Footing) => {
                tool_id(t)
            }
            Family::Stairs if matches!(t, Tool::Stair | Tool::Railing) => tool_id(t),
            Family::Rooms if matches!(t, Tool::Room | Tool::RoomSeparator) => tool_id(t),
            Family::Furnishing if t == Tool::Object => tool_id(t),
            Family::Drafting if t == Tool::Sketch => match shape {
                Shape::Line => "shape:line",
                Shape::Arc => "shape:arc",
                Shape::Rectangle => "shape:rect",
                Shape::Circle => "shape:circle",
            }
            .to_string(),
            Family::Site if t == Tool::Terrain => {
                format!("site:ground.{}", plan.parts.ground.label())
            }
            Family::Site if t == Tool::Paving => format!("site:area.{}", plan.parts.area),
            Family::Site if t == Tool::Planting => {
                format!("site:plant.{}", plan.parts.plant.label())
            }
            _ => return None,
        };
        Some(id)
    }

    /// Whether `tool` is one of this family's tools (for the tools row's
    /// tabs: a tool in a family is on its tab).
    pub fn holds(self, tool: Tool) -> bool {
        match self {
            Family::Structure => matches!(tool, Tool::Column | Tool::Beam | Tool::Footing),
            Family::Stairs => matches!(tool, Tool::Stair | Tool::Railing),
            Family::Rooms => matches!(tool, Tool::Room | Tool::RoomSeparator),
            Family::Furnishing => tool == Tool::Object,
            Family::Drafting => tool == Tool::Sketch,
            Family::Site => matches!(tool, Tool::Terrain | Tool::Paving | Tool::Planting),
        }
    }
}

/// A member by its action id, in any family.
pub fn member_of(id: &str) -> Option<Member> {
    Family::ALL
        .into_iter()
        .flat_map(|f| f.members())
        .find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_member_belongs_to_one_family_and_ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for f in Family::ALL {
            assert!(!f.members().is_empty(), "{f:?}");
            for m in f.members() {
                assert!(seen.insert(m.id.clone()), "{} twice", m.id);
                assert_eq!(Family::of_action(&m.id), Some(f));
            }
        }
    }

    #[test]
    fn the_site_family_has_the_mockups_headings() {
        let heads: Vec<&str> = Family::Site.groups().iter().map(|g| g.0).collect();
        assert_eq!(
            heads,
            [
                "Ground",
                "Earthworks",
                "Ways",
                "Hard landscape",
                "Services",
                "Planting"
            ]
        );
        // every ground kind is reachable from the panel
        for g in GroundDraw::ALL {
            assert!(
                member_of(&format!("site:ground.{}", g.label())).is_some(),
                "{g:?}"
            );
        }
    }

    #[test]
    fn the_active_member_follows_the_tool() {
        let mut plan = PlanState::default();
        plan.tool = Tool::Beam;
        assert_eq!(
            Family::Structure.active(&plan, Default::default()),
            Some(tool_id(Tool::Beam))
        );
        assert_eq!(Family::Site.active(&plan, Default::default()), None);
        plan.tool = Tool::Terrain;
        plan.parts.ground = GroundDraw::Pad;
        assert_eq!(
            Family::Site.active(&plan, Default::default()),
            Some("site:ground.Pad".into())
        );
    }
}
