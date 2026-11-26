//! The tools row's tabs (brief 02, 2.1): Model, Annotate, View and Manage
//! each show their own tools and commands, icons only, while Select and
//! Align stay at the left on every tab. A tool picked by its key shows its
//! tab. A tool can be on two tabs where it belongs to both: the callout.

use crate::planview::Tool;

/// One button of a tab.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Entry {
    Tool(Tool),
    /// A command of the menus, by its action id.
    Act {
        id: &'static str,
        label: &'static str,
        icon: &'static str,
        hint: &'static str,
    },
    /// A short menu of commands (see [`menu`]).
    Menu {
        key: &'static str,
        label: &'static str,
        icon: &'static str,
        hint: &'static str,
    },
    /// A family of tools behind one split button (tool families REV04).
    Family(crate::families::Family),
    /// A divider between groups.
    Sep,
}

pub const TABS: [&str; 4] = ["Model", "Annotate", "View", "Manage"];

/// On every tab, before it.
pub const ALWAYS: [Tool; 2] = [Tool::Select, Tool::Align];

/// What tab `tab` holds.
pub fn entries(tab: usize) -> Vec<Entry> {
    use crate::families::Family as F;
    use Entry::{Family as Fam, Sep, Tool as T};
    let act = |id, label, icon, hint| Entry::Act {
        id,
        label,
        icon,
        hint,
    };
    match tab {
        // tool families REV04, Option A (MU57): single tools stay single;
        // related ones share a split button, the last one used on top
        0 => vec![
            T(Tool::Wall),
            act(
                "tool.curtain",
                "Curtain wall",
                "curtain-wall",
                "The Wall tool drawing a curtain wall type: mullions and glass on a grid",
            ),
            T(Tool::Door),
            T(Tool::Window),
            T(Tool::Opening),
            Sep,
            T(Tool::Slab),
            T(Tool::Shaft),
            T(Tool::Roof),
            Sep,
            // shapes (MU43): draw, then Pull. Sketch, Mass and Extrusion
            // are in Create > Earlier tools; Sweep and Void stay until
            // Follow and Cut with shape replace them
            Fam(F::Drafting),
            act(
                "cmd:Pull",
                "Pull",
                "pull",
                "Pull the face under the pointer out or in; a drawn outline becomes a solid (P)",
            ),
            T(Tool::Sweep),
            T(Tool::Void),
            Sep,
            Fam(F::Structure),
            Fam(F::Stairs),
            Fam(F::Furnishing),
            Sep,
            Fam(F::Rooms),
            T(Tool::Ceiling),
            Sep,
            Fam(F::Site),
        ],
        1 => vec![
            T(Tool::Dimension),
            T(Tool::Angle),
            T(Tool::Radius),
            T(Tool::Diameter),
            T(Tool::ArcLength),
            T(Tool::SpotLevel),
            Sep,
            T(Tool::Tag),
            act(
                "tag.all",
                "Tag all",
                "tags",
                "Tag every door, window, fixture and room in the view that has no tag yet (Shift+T)",
            ),
            T(Tool::Note),
            T(Tool::Symbol),
            Sep,
            T(Tool::Detail),
            T(Tool::Callout),
            Sep,
            act(
                "styles",
                "Annotation styles",
                "anno-styles",
                "Named styles for dimensions, notes and tags: text, ends, tag shape, colour",
            ),
        ],
        2 => vec![
            // levels are what plans are cut from, so they sit with the
            // views that come from them (MU20)
            T(Tool::Level),
            T(Tool::Section),
            T(Tool::Elevation),
            T(Tool::Callout),
            Sep,
            act(
                "crop.add",
                "Crop box",
                "crop-box",
                "Add a crop box to the 3D view: it crops everything outside it",
            ),
            act(
                "view.reference",
                "Reference",
                "underlay",
                "Trace over another view, a picture or a DXF drawing: shown faintly under this view (Alt+F2 hides it)",
            ),
            act("new.sheet", "New sheet", "file-text", "A new drawing sheet"),
            Entry::Menu {
                key: "tab:schedules",
                label: "New schedule",
                icon: "table-2",
                hint: "A schedule of doors, windows, walls, rooms and more",
            },
            Sep,
            act(
                "graphicstyles",
                "Graphic styles",
                "align-left",
                "Style packs: pens by scale, object styles, hatch patterns and view templates",
            ),
            act(
                "templates",
                "Masters",
                "sheet-template",
                "Title blocks, frames and paper for sheets, in the Masters view",
            ),
        ],
        _ => vec![
            // the Levels manager is project set-up; the Level tool, which
            // makes a level view, is on the View tab (MU21)
            act(
                "levels",
                "Levels",
                "level",
                "Add, duplicate, rename, move and delete levels",
            ),
            T(Tool::Grid),
            T(Tool::Guide),
            Sep,
            act(
                "project.info",
                "Project information",
                "info",
                "Name, client, address, site and building: they fill the title blocks and the IFC export",
            ),
            // project rules, with the other project set-up (MU66: it was on
            // Annotate, where tags only show the marks)
            act(
                "marks",
                "Mark numbering",
                // not the Grid tool's # beside it: the tag, as its menu entry
                "tag",
                "How doors, windows and other elements are numbered; renumber, and check for marks used twice",
            ),

            act(
                "types.browser",
                "Types",
                "element-types",
                "Wall, curtain wall, door, window, slab, roof, column, beam and object types",
            ),
            act(
                "profiles.browser",
                "Profiles",
                "profile",
                "The profile library: presets and the project's own, for sweeps",
            ),
            act(
                "mat.browser",
                "Materials",
                "palette",
                "Add, edit, delete and assign the project's materials",
            ),
            act(
                "mat.patterns",
                "Patterns",
                "brick-wall",
                "Hatches and surface patterns: brick bonds, tiles, boards and tiles you draw",
            ),
            act(
                "libraries",
                "Libraries",
                "library",
                "Every library in one place: patterns, symbols, line styles, palettes, materials, profiles, masters",
            ),
            act(
                "graphicstyles",
                "Style packs",
                "align-left",
                "Pens by scale, object styles, hatch patterns and view templates",
            ),
            act(
                "units",
                "Project units",
                "units",
                "Metric or imperial: how lengths are shown and typed",
            ),
            act(
                "cleanup.stacked",
                "Copies on originals",
                "copy",
                "Find and remove elements sitting exactly on a copy of themselves (left by a Duplicate that never moved)",
            ),
            act(
                "cleanup.overlap",
                "Overlapping walls",
                "walls-overlap",
                "Find walls running along each other and overlapping: the plan merges them, but in 3D they stay two solids. Merge keeps one wall covering both",
            ),
            // the model checks, moved here from the Create menu (tool
            // families REV04, decision 5)
            act(
                "check.doorways",
                "Check doorways",
                "door-open",
                "List and select the furniture and fixtures standing in a doorway, a window or a door's swing",
            ),
            act(
                "check.stairs",
                "Check stairs",
                "stair",
                "List and select the stairs failing a check of their rule set: riser, going, winders, width, landings, handrails and guards",
            ),
            act(
                "check.layers",
                "Check layer joins",
                "layers",
                "List and select the walls with a layer held back from a join (a gap) or run on into the wall it meets (an overlap)",
            ),
            Sep,
            Entry::Menu {
                key: "tab:ifc",
                label: "IFC",
                icon: "upload",
                hint: "Open an IFC file, or export this project as IFC4 or IFC5",
            },
        ],
    }
}

/// Does tab `tab` show `tool`?
pub fn holds(tab: usize, tool: Tool) -> bool {
    ALWAYS.contains(&tool)
        || entries(tab).iter().any(|e| match e {
            Entry::Tool(t) => *t == tool,
            Entry::Family(f) => f.holds(tool),
            _ => false,
        })
}

/// The first tab that shows `tool`.
pub fn home_of(tool: Tool) -> usize {
    (0..TABS.len()).find(|t| holds(*t, tool)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_buttons_on_a_tab_share_an_icon() {
        // A5-1: two buttons that look the same read as one
        for (tab, tab_name) in TABS.iter().enumerate() {
            let mut seen: Vec<(&str, String)> = vec![];
            for e in entries(tab) {
                let (icon, name) = match e {
                    Entry::Tool(t) => (crate::app::shell::tool_icon(t), t.label().to_string()),
                    Entry::Act { icon, label, .. } | Entry::Menu { icon, label, .. } => {
                        (icon, label.to_string())
                    }
                    Entry::Family(f) => (f.first().icon, f.label().to_string()),
                    Entry::Sep => continue,
                };
                if let Some((_, other)) = seen.iter().find(|(i, _)| *i == icon) {
                    panic!("{tab_name}: {name} and {other} both use the {icon} icon");
                }
                seen.push((icon, name));
            }
        }
    }

    #[test]
    fn every_tool_is_on_a_tab() {
        for t in Tool::ALL {
            // the shape tools took their place (MU43): they are in
            // Create > Earlier tools for one release, on no tab
            // (the Drafting family's Line, Arc, Rectangle and Circle run the
            // drawing tool, so it is on the Model tab through them)
            if t == Tool::Sketch {
                assert!(holds(0, t));
                continue;
            }
            if matches!(t, Tool::Mass | Tool::Extrusion | Tool::Pad) {
                assert!(!(0..TABS.len()).any(|tab| holds(tab, t)));
                continue;
            }
            assert!(
                (0..TABS.len()).any(|tab| holds(tab, t)),
                "{} is on no tab",
                t.label()
            );
        }
        assert_eq!(home_of(Tool::Wall), 0);
        assert_eq!(home_of(Tool::Dimension), 1);
        assert_eq!(home_of(Tool::Section), 2);
        assert_eq!(home_of(Tool::Grid), 3);
        // the callout is on both of its tabs
        assert!(holds(1, Tool::Callout) && holds(2, Tool::Callout));
        // Select is on every one
        assert!((0..TABS.len()).all(|t| holds(t, Tool::Select)));
    }
}
