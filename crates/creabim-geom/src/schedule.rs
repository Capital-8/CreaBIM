//! Schedules: tables of doors, windows, walls or slabs read live from the
//! model, and counts of furniture and fixtures. The same table is drawn on
//! sheets, edited in the app and exported as CSV.

use crate::paper::Anchor;
use crate::plan::{Pen, PlanItem, PlanView};
use creabim_core::{Element, ElementId, ElementKind, Model, OpeningKind, ScheduleCategory};
use glam::DVec2;

/// An element property a schedule cell shows and can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Name,
    Width,
    Height,
    Sill,
    Thickness,
    WallHeight,
    Material,
    /// A material's take-off rate (a plain number, not a length).
    Rate,
    /// What a material's rate is priced per.
    RatePer,
}

impl Field {
    /// Lengths, shown and edited in the project's units.
    pub fn is_length(self) -> bool {
        matches!(
            self,
            Field::Width | Field::Height | Field::Sill | Field::Thickness | Field::WallHeight
        )
    }
}

#[derive(Clone, Debug)]
pub struct Column {
    pub title: &'static str,
    /// The editable property behind this column, if any.
    pub field: Option<Field>,
    /// Width on paper, mm.
    pub width: f64,
    pub numeric: bool,
}

#[derive(Clone, Debug)]
pub struct Row {
    /// The element listed, or for a group heading the type grouped.
    pub id: ElementId,
    pub cells: Vec<String>,
    /// A heading over the rows of one type, when grouped by type.
    pub group: bool,
}

#[derive(Clone, Debug)]
pub struct Schedule {
    pub category: ScheduleCategory,
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    /// The last row: counts and sums.
    pub totals: Vec<String>,
    /// How many elements it lists (for furniture and fixtures, how many
    /// objects its counts add up to).
    pub listed: usize,
}

fn col(title: &'static str, field: Option<Field>, width: f64, numeric: bool) -> Column {
    Column {
        title,
        field,
        width,
        numeric,
    }
}

/// The current value of a numeric field, metres.
pub fn value(e: &Element, field: Field) -> Option<f64> {
    Some(match (&e.kind, field) {
        (ElementKind::Opening(o), Field::Width) => o.width,
        (ElementKind::Opening(o), Field::Height) => o.height,
        (ElementKind::Opening(o), Field::Sill) => o.sill,
        (ElementKind::Wall(w), Field::Thickness) => w.thickness,
        (ElementKind::Wall(w), Field::WallHeight) => w.height,
        (ElementKind::Slab(s), Field::Thickness) => s.thickness,
        _ => return None,
    })
}

/// Change a numeric field, metres.
pub fn set_value(e: &mut Element, field: Field, v: f64) {
    match (&mut e.kind, field) {
        (ElementKind::Opening(o), Field::Width) => o.width = v,
        (ElementKind::Opening(o), Field::Height) => o.height = v,
        (ElementKind::Opening(o), Field::Sill) => o.sill = v,
        (ElementKind::Wall(w), Field::Thickness) => w.thickness = v,
        (ElementKind::Wall(w), Field::WallHeight) => w.height = v,
        (ElementKind::Slab(s), Field::Thickness) => s.thickness = v,
        _ => {}
    }
}

/// Change what an element is made of.
pub fn set_material(e: &mut Element, material: Option<ElementId>) {
    match &mut e.kind {
        ElementKind::Wall(w) => w.material = material,
        ElementKind::Slab(s) => s.material = material,
        ElementKind::FreeMesh(m) => m.material = material,
        k => {
            if let Some(m) = creabim_core::members::material_mut(k) {
                *m = material;
            }
        }
    }
}

/// The schedule of one category, rows in mark or model order.
pub fn schedule(model: &Model, category: ScheduleCategory) -> Schedule {
    schedule_by(model, category, false)
}

/// The schedule of one category; `by_type` groups the rows by element
/// type, each group under a heading row naming the type and its count.
/// A phased project's schedule ends with its totals split by status:
/// what is kept, what is taken down and what is new.
pub fn schedule_by(model: &Model, category: ScheduleCategory, by_type: bool) -> Schedule {
    let mut s = schedule_all(model, category, by_type);
    if category == ScheduleCategory::Materials || creabim_core::phases::list(model).is_empty() {
        return s;
    }
    let listed: Vec<ElementId> = s.rows.iter().filter(|r| !r.group).map(|r| r.id).collect();
    let group_of = |id: ElementId| {
        let e = model.get(id).filter(|e| creabim_core::phases::phased(e))?;
        Some(if e.phase.gone.is_some() {
            1
        } else if e.phase.built.is_some() {
            2
        } else {
            0
        })
    };
    if !listed.iter().any(|id| group_of(*id).is_some()) {
        return s;
    }
    for (g, label) in ["Existing, kept", "Taken down", "New"]
        .into_iter()
        .enumerate()
    {
        let n = listed.iter().filter(|id| group_of(**id) == Some(g)).count();
        if n == 0 {
            continue;
        }
        // the same schedule of a model without the listed elements of
        // the other statuses, and without what hangs on them
        let mut out: Vec<ElementId> = listed
            .iter()
            .copied()
            .filter(|id| group_of(*id).is_some_and(|x| x != g))
            .collect();
        let mut i = 0;
        while i < out.len() {
            for d in model.dependents(out[i]) {
                if !out.contains(&d) {
                    out.push(d);
                }
            }
            i += 1;
        }
        let gone: std::collections::HashSet<ElementId> = out.into_iter().collect();
        let mut part = model.clone();
        part.retain(|e| !gone.contains(&e.id));
        let sub = schedule_all(&part, category, false);
        let mut cells = vec![String::new(); s.columns.len()];
        for (k, c) in s.columns.iter().enumerate().skip(1) {
            if let Some(j) = sub.columns.iter().position(|x| x.title == c.title) {
                cells[k] = sub.totals.get(j).cloned().unwrap_or_default();
            }
        }
        cells[0] = format!("{label} ({n})");
        s.rows.push(Row {
            id: ElementId::new(),
            cells,
            group: true,
        });
    }
    s
}

/// [`schedule_by`] without the totals by status.
fn schedule_all(model: &Model, category: ScheduleCategory, by_type: bool) -> Schedule {
    if category == ScheduleCategory::Materials {
        return takeoff_schedule(model, by_type);
    }
    let units = model.units();
    let mm =
        |v: f64| creabim_core::write_number(model, creabim_core::NumberKind::Schedules, None, v);
    let area = |v: f64| units.area(v);
    let name = |id: ElementId| model.get(id).map(|e| e.name.clone()).unwrap_or_default();
    let material = |e: &Element| {
        // A layered element is listed by its core layer's material.
        let core = creabim_core::assembly_of(model, e)
            .and_then(|(_, a)| a.layers.get(a.core().0)?.material)
            .and_then(|m| model.get(m));
        if let Some(m) = core {
            return m.name.clone();
        }
        creabim_core::material_of(model, e)
            .map(|(m, _)| m.name.clone())
            .unwrap_or_else(|| "-".into())
    };
    let level_of = |e: &Element| match &e.kind {
        ElementKind::Wall(w) => Some(w.level),
        ElementKind::Slab(s) => Some(s.level),
        ElementKind::Opening(o) => match model.get(o.host).map(|h| &h.kind) {
            Some(ElementKind::Wall(w)) => Some(w.level),
            Some(ElementKind::Roof(r)) => Some(r.level),
            _ => None,
        },
        k => creabim_core::members::level_of(k),
    };
    let elevation = |e: &Element| {
        level_of(e)
            .and_then(|l| model.level(l))
            .map_or(0.0, |l| l.elevation)
    };
    let mut rows: Vec<(f64, String, Row)> = Vec::new();
    let mut listed = None;
    let (columns, totals) = match category {
        ScheduleCategory::Furniture | ScheduleCategory::Fixtures => {
            let (columns, totals, n) = object_rows(model, category, by_type, &mut rows);
            listed = Some(n);
            (columns, totals)
        }
        ScheduleCategory::Doors | ScheduleCategory::Windows => {
            let kind = if category == ScheduleCategory::Doors {
                OpeningKind::Door
            } else {
                OpeningKind::Window
            };
            let marks = creabim_core::marks(model);
            let mut columns = vec![
                col("Mark", None, 16.0, false),
                col("Level", None, 28.0, false),
                col("Wall", None, 26.0, false),
                col("Width", Some(Field::Width), 18.0, true),
                col("Height", Some(Field::Height), 18.0, true),
                col("Sill", Some(Field::Sill), 16.0, true),
            ];
            let doors = kind == OpeningKind::Door;
            if doors {
                // hinge side seen from where it opens to, into or out of the building
                columns.push(col("Swing", None, 22.0, false));
                // how it opens: single or double, and a pair's leaf widths
                columns.push(col("Operation", None, 34.0, false));
            } else {
                // how each of its cells opens: fixed, casement, awning
                columns.push(col("Operation", None, 34.0, false));
            }
            let op_col = columns.len() - 1;
            // how they are made and what is declared for them, as columns
            // only once some door or window has them, so other schedules
            // keep their old table
            let these: Vec<&creabim_core::construction::Construction> = model
                .iter()
                .filter_map(|e| match &e.kind {
                    ElementKind::Opening(o) if o.kind == kind => Some(&o.frame.construction),
                    _ => None,
                })
                .collect();
            type Get = fn(&creabim_core::construction::Construction) -> String;
            let extras: Vec<(&str, f64, Get)> = [
                (
                    "Leaf",
                    22.0,
                    (|c| {
                        use creabim_core::construction::LeafStyle as L;
                        if c.leaf == L::Flush {
                            String::new()
                        } else if c.leaf == L::AllGlass {
                            format!("All glass, {}", c.fittings.code())
                        } else {
                            c.leaf.label().to_string()
                        }
                    }) as Get,
                ),
                ("Fire", 16.0, |c| c.ratings.fire.clone()),
                ("Acoustic", 20.0, |c| c.ratings.acoustic.clone()),
                ("Security", 18.0, |c| c.ratings.security.clone()),
                ("U-value", 16.0, |c| {
                    c.ratings
                        .u_value
                        .map_or(String::new(), |u| format!("{u:.2}"))
                }),
                ("Accessible", 18.0, |c| {
                    if c.ratings.accessible {
                        "Yes".into()
                    } else {
                        String::new()
                    }
                }),
            ]
            .into_iter()
            .filter(|(_, _, get)| these.iter().any(|c| !get(c).is_empty()))
            .collect();
            for (name, width, _) in &extras {
                // a window's is how it is made: French, slim steel
                let name = if !doors && *name == "Leaf" {
                    "Make"
                } else {
                    name
                };
                columns.push(col(name, None, *width, false));
            }
            // the ribbon or corner a window is in (part 9), as a column only
            // once some window is in one
            let arranged = !doors
                && model.iter().any(|e| {
                    matches!(&e.kind, ElementKind::Opening(o) if o.kind == kind && o.arrangement.is_some())
                });
            if arranged {
                columns.push(col("Arrangement", None, 26.0, false));
            }
            for e in model.iter() {
                let ElementKind::Opening(o) = &e.kind else {
                    continue;
                };
                if o.kind != kind {
                    continue;
                }
                let mark = marks.get(&e.id).cloned().unwrap_or_default();
                let mut cells = vec![
                    mark.clone(),
                    level_of(e).map(name).unwrap_or_default(),
                    name(o.host),
                    mm(o.width),
                    mm(o.height),
                    // a roof window has no sill height: it lies in the slope
                    if o.roof_at.is_some() {
                        "-".into()
                    } else {
                        mm(o.sill)
                    },
                ];
                if doors {
                    let host = match model.get(o.host).map(|h| &h.kind) {
                        Some(ElementKind::Wall(w)) => Some(w),
                        _ => None,
                    };
                    cells.push(host.map_or_else(String::new, |w| crate::opening::door_hand(o, w)));
                    cells.push(host.map_or_else(
                        || o.frame.operation(o.kind).label(o.kind).into(),
                        |w| crate::opening::operation_text(o, w),
                    ));
                } else {
                    cells.push(crate::opening::window_text(o));
                }
                for (_, _, get) in &extras {
                    cells.push(get(&o.frame.construction));
                }
                if arranged {
                    cells.push(creabim_core::arrangement::label(o));
                }
                rows.push((
                    0.0,
                    mark,
                    Row {
                        id: e.id,
                        cells,
                        group: false,
                    },
                ));
            }
            // the operation column wide enough for its longest entry (a
            // door with lights round it says what they are)
            let longest = rows
                .iter()
                .filter_map(|(_, _, r)| r.cells.get(op_col))
                .map(|t| t.chars().count())
                .max()
                .unwrap_or(0);
            // (none from before cells is longer than 28 characters, so
            // those schedules keep their width)
            if let Some(c) = columns.get_mut(op_col).filter(|_| longest > 28) {
                c.width = c.width.max(longest as f64 * 1.3);
            }
            let what = if kind == OpeningKind::Door {
                "doors"
            } else {
                "windows"
            };
            (columns, vec![format!("{} {what}", rows.len())])
        }
        ScheduleCategory::Walls => {
            // The assembly column shows up once any wall has layers, so
            // single-material projects keep their old table.
            let layered = model
                .iter()
                .any(|e| matches!(&e.kind, ElementKind::Wall(w) if w.assembly.is_some()));
            let mut columns = vec![
                col("Name", Some(Field::Name), 30.0, false),
                col("Level", None, 28.0, false),
                col("Material", Some(Field::Material), 30.0, false),
                col("Length", None, 18.0, true),
                col("Thickness", Some(Field::Thickness), 20.0, true),
                col("Height", Some(Field::WallHeight), 18.0, true),
                col("Area", None, 22.0, true),
            ];
            if layered {
                columns.insert(3, col("Assembly", None, 44.0, false));
            }
            let mut total_area = 0.0;
            let mut total_length = 0.0;
            for e in model.iter() {
                let ElementKind::Wall(w) = &e.kind else {
                    continue;
                };
                let openings: f64 = model
                    .dependents(e.id)
                    .into_iter()
                    .filter_map(|d| match &model.get(d)?.kind {
                        ElementKind::Opening(o) => Some(o.width * o.height),
                        _ => None,
                    })
                    .sum();
                let a = (w.length() * w.height - openings).max(0.0);
                total_area += a;
                total_length += w.length();
                let mut cells = vec![
                    e.name.clone(),
                    name(w.level),
                    material(e),
                    mm(w.length()),
                    mm(w.thickness),
                    mm(w.height),
                    area(a),
                ];
                if layered {
                    let asm = w
                        .assembly
                        .and_then(|id| model.get(id))
                        .map_or_else(|| "-".to_string(), |a| a.name.clone());
                    cells.insert(3, asm);
                }
                rows.push((
                    elevation(e),
                    String::new(),
                    Row {
                        id: e.id,
                        cells,
                        group: false,
                    },
                ));
            }
            let mut totals = vec![
                format!("{} walls", rows.len()),
                String::new(),
                String::new(),
                mm(total_length),
                String::new(),
                String::new(),
                area(total_area),
            ];
            if layered {
                totals.insert(3, String::new());
            }
            (columns, totals)
        }
        ScheduleCategory::Slabs => {
            let columns = vec![
                col("Name", Some(Field::Name), 30.0, false),
                col("Level", None, 28.0, false),
                col("Material", Some(Field::Material), 30.0, false),
                col("Thickness", Some(Field::Thickness), 20.0, true),
                col("Area", None, 22.0, true),
            ];
            let mut total = 0.0;
            for e in model.iter() {
                let ElementKind::Slab(s) = &e.kind else {
                    continue;
                };
                let a = s.area();
                total += a;
                rows.push((
                    elevation(e),
                    String::new(),
                    Row {
                        id: e.id,
                        group: false,
                        cells: vec![
                            e.name.clone(),
                            name(s.level),
                            material(e),
                            mm(s.thickness),
                            area(a),
                        ],
                    },
                ));
            }
            (
                columns,
                vec![
                    format!("{} slabs", rows.len()),
                    String::new(),
                    String::new(),
                    String::new(),
                    area(total),
                ],
            )
        }
        ScheduleCategory::Drainage => {
            let columns = vec![
                col("Ref", None, 14.0, false),
                col("Run", Some(Field::Name), 34.0, false),
                col("Is", None, 30.0, false),
                col("Cover level", None, 22.0, true),
                col("Invert", None, 22.0, true),
                col("Depth", None, 18.0, true),
                col("Size", None, 22.0, false),
                col("Pipe out", None, 30.0, false),
            ];
            let (mut chambers, mut tanks, mut gullies, mut dug) = (0, 0, 0, 0.0);
            for e in model.iter() {
                let ElementKind::Site(s) = &e.kind else {
                    continue;
                };
                match &s.form {
                    creabim_core::SiteForm::Drain(d) => {
                        let lv = crate::services::drain_levels(model, s, d);
                        dug += crate::services::drain_dig(model, s, d).dug;
                        let pts = crate::services::chamber_points(d);
                        for (n, k) in pts.iter().enumerate() {
                            chambers += 1;
                            let out = if k + 1 < d.path.len() {
                                format!("Ø{:.0} {}", d.diameter * 1000.0, d.gradient_text())
                            } else {
                                "End".into()
                            };
                            let rf = format!("{}{}", d.service.letter(), n + 1);
                            rows.push((
                                0.0,
                                format!("{}{:04}", e.name, n),
                                Row {
                                    id: e.id,
                                    group: false,
                                    cells: vec![
                                        rf,
                                        e.name.clone(),
                                        "Chamber".into(),
                                        creabim_core::site_level(model, lv.cover[*k]),
                                        creabim_core::site_level(model, lv.invert[*k]),
                                        mm(lv.cover[*k] - lv.invert[*k]),
                                        format!("{0:.0}×{0:.0}", d.chamber * 1000.0),
                                        out,
                                    ],
                                },
                            ));
                        }
                    }
                    creabim_core::SiteForm::Road(r) => {
                        for (n, g) in crate::services::gullies(model, s, r).iter().enumerate() {
                            gullies += 1;
                            let to = g.to.map_or("None near".to_string(), |(d, k, _)| {
                                let letter = match model.get(d).map(|e| &e.kind) {
                                    Some(ElementKind::Site(o)) => match &o.form {
                                        creabim_core::SiteForm::Drain(d) => d.service.letter(),
                                        _ => "",
                                    },
                                    _ => "",
                                };
                                format!("To {letter}{}", k + 1)
                            });
                            rows.push((
                                0.5,
                                format!("{}{:04}", e.name, n),
                                Row {
                                    id: e.id,
                                    group: false,
                                    cells: vec![
                                        format!("G{}", n + 1),
                                        e.name.clone(),
                                        "Road gully".into(),
                                        creabim_core::site_level(model, g.cover),
                                        String::new(),
                                        String::new(),
                                        "450×300".into(),
                                        to,
                                    ],
                                },
                            ));
                        }
                    }
                    creabim_core::SiteForm::Tank(t) => {
                        tanks += 1;
                        let lv = crate::services::tank_levels(model, s, t);
                        dug += crate::services::tank_dig(model, s, t).dug;
                        rows.push((
                            1.0,
                            e.name.clone(),
                            Row {
                                id: e.id,
                                group: false,
                                cells: vec![
                                    format!("T{tanks}"),
                                    e.name.clone(),
                                    format!("{} {:.1} m³", t.service.label(), t.capacity()),
                                    creabim_core::site_level(model, lv.ground),
                                    creabim_core::site_level(model, lv.floor),
                                    mm(lv.ground - lv.floor),
                                    format!("{:.0}×{:.0}", t.length * 1000.0, t.width * 1000.0),
                                    String::new(),
                                ],
                            },
                        ));
                    }
                    _ => {}
                }
            }
            (
                columns,
                vec![
                    if gullies > 0 {
                        format!("{chambers} chambers, {gullies} gullies, {tanks} tanks")
                    } else {
                        format!("{chambers} chambers, {tanks} tanks")
                    },
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    format!("dug {}", units.volume(dug)),
                ],
            )
        }
        ScheduleCategory::Parking => {
            use creabim_core::carpark::BayKind;
            let columns = vec![
                col("Car park", Some(Field::Name), 40.0, false),
                col("Bay type", None, 34.0, false),
                col("Size", None, 30.0, false),
                col("Bays", None, 40.0, false),
                col("Provided", None, 20.0, true),
                col("Required", None, 20.0, true),
            ];
            let (mut cars, mut asked, mut any_asked) = (0u32, 0u32, false);
            let mut k = 0.0;
            for e in model.iter() {
                let ElementKind::Site(s) = &e.kind else {
                    continue;
                };
                let (Some(cp), Some(lay)) = (s.as_car_park(), s.car_park_layout(model)) else {
                    continue;
                };
                let here = lay.cars() as u32;
                cars += here;
                let need = cp.required.required();
                if let Some(n) = need {
                    asked += n;
                    any_asked = true;
                }
                let need_acc = (cp.accessible_per > 0).then(|| {
                    (here as f64 / cp.accessible_per as f64 - 1e-9)
                        .ceil()
                        .max(0.0) as u32
                });
                for kind in BayKind::LISTED {
                    let c = lay.count(kind);
                    if c == 0 && !(kind == BayKind::Accessible && need_acc.is_some_and(|n| n > 0)) {
                        continue;
                    }
                    let size = if kind == BayKind::Accessible {
                        format!(
                            "{:.0}×{:.0} + {:.0}",
                            cp.bay_width * 1000.0,
                            cp.bay_length * 1000.0,
                            cp.access_zone * 1000.0
                        )
                    } else {
                        format!("{:.0}×{:.0}", cp.bay_width * 1000.0, cp.bay_length * 1000.0)
                    };
                    k += 1.0;
                    rows.push((
                        k,
                        format!("{}{k:04}", e.name),
                        Row {
                            id: e.id,
                            group: false,
                            cells: vec![
                                e.name.clone(),
                                kind.label().to_string(),
                                size,
                                lay.labels(kind),
                                c.to_string(),
                                match (kind, need_acc) {
                                    (BayKind::Accessible, Some(n)) => n.to_string(),
                                    _ => String::new(),
                                },
                            ],
                        },
                    ));
                }
                // the car park's own total against what it is asked
                k += 1.0;
                rows.push((
                    k,
                    format!("{}{k:04}", e.name),
                    Row {
                        id: e.id,
                        group: false,
                        cells: vec![
                            e.name.clone(),
                            "All car bays".into(),
                            String::new(),
                            format!("{} rows", lay.rows),
                            here.to_string(),
                            need.map_or(String::new(), |n| n.to_string()),
                        ],
                    },
                ));
            }
            (
                columns,
                vec![
                    "Total".into(),
                    String::new(),
                    String::new(),
                    String::new(),
                    cars.to_string(),
                    if any_asked {
                        asked.to_string()
                    } else {
                        String::new()
                    },
                ],
            )
        }
        ScheduleCategory::Stairs => {
            let columns = vec![
                col("Name", Some(Field::Name), 40.0, false),
                col("Level", None, 24.0, false),
                col("Risers", None, 20.0, true),
                col("Goings", None, 20.0, true),
                col("2R + G", None, 14.0, true),
                col("Pitch", None, 13.0, true),
                col("Width", None, 14.0, true),
                col("Headroom", None, 18.0, true),
                col("Rule set", None, 34.0, false),
                col("Checks", None, 40.0, false),
            ];
            let (mut stairs, mut failing) = (0, 0);
            for e in model.iter() {
                let ElementKind::Stair(s) = &e.kind else {
                    continue;
                };
                stairs += 1;
                let lay = s.layout(s.rise(model));
                let r = lay.riser;
                let checks = creabim_core::stairrules::check_stair(model, e.id);
                let fails: Vec<&str> = checks
                    .iter()
                    .filter(|c| c.verdict == creabim_core::stairrules::Verdict::Fails)
                    .map(|c| c.what)
                    .collect();
                if !fails.is_empty() {
                    failing += 1;
                }
                let head = checks
                    .iter()
                    .find(|c| c.what == "Headroom")
                    .map_or_else(|| "-".to_string(), |c| c.value.clone());
                rows.push((
                    elevation(e),
                    e.name.clone(),
                    Row {
                        id: e.id,
                        group: false,
                        cells: vec![
                            e.name.clone(),
                            name(s.level),
                            if s.is_ramp() {
                                crate::members::ramp_tag(s.tread / r.max(1e-9)).replace(" · ", ", ")
                            } else {
                                format!("{} × {}", lay.risers, mm(r))
                            },
                            if s.is_ramp() {
                                // a ramp's run, end to end
                                units.length(lay.treads() as f64 * s.tread)
                            } else {
                                format!("{} × {}", lay.treads(), mm(s.tread))
                            },
                            if s.is_ramp() {
                                "-".into()
                            } else {
                                mm(2.0 * r + s.tread)
                            },
                            format!("{:.0}°", r.atan2(s.tread).to_degrees()),
                            mm(s.width),
                            head,
                            s.rules.label(),
                            if fails.is_empty() {
                                "Passes".into()
                            } else {
                                format!("Fails: {}", fails.join(", "))
                            },
                        ],
                    },
                ));
            }
            let mut totals = vec![String::new(); columns.len()];
            totals[0] = if stairs == 1 {
                "1 stair".into()
            } else {
                format!("{stairs} stairs")
            };
            totals[9] = if failing == 0 {
                "All pass".into()
            } else {
                format!("{failing} failing")
            };
            (columns, totals)
        }
        ScheduleCategory::Rooms => {
            let columns = vec![
                col("Number", None, 16.0, false),
                col("Name", Some(Field::Name), 34.0, false),
                col("Level", None, 28.0, false),
                col("Area", None, 20.0, true),
                col("Perimeter", None, 20.0, true),
                col("Height", None, 18.0, true),
                col("Floor finish", None, 34.0, false),
                col("Occupancy", None, 26.0, false),
            ];
            let mut total = 0.0;
            for e in model.iter() {
                let ElementKind::Room(r) = &e.kind else {
                    continue;
                };
                let shape = creabim_core::room::shape(model, e.id);
                let (a, p) = shape.as_ref().map_or((0.0, 0.0), |s| {
                    (creabim_core::room::net_area(model, r, s), s.perimeter())
                });
                total += a;
                let dash = |s: &str| {
                    if s.is_empty() {
                        "-".to_string()
                    } else {
                        s.to_string()
                    }
                };
                rows.push((
                    model.level(r.level).map_or(0.0, |l| l.elevation),
                    r.number.clone(),
                    Row {
                        id: e.id,
                        group: false,
                        cells: vec![
                            r.number.clone(),
                            e.name.clone(),
                            name(r.level),
                            if shape.is_some() {
                                area(a)
                            } else {
                                "Not enclosed".into()
                            },
                            mm(p),
                            mm(creabim_core::room::height(model, r)),
                            dash(&r.floor_finish),
                            dash(&r.occupancy),
                        ],
                    },
                ));
            }
            (
                columns,
                vec![
                    format!("{} rooms", rows.len()),
                    String::new(),
                    String::new(),
                    area(total),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                ],
            )
        }
        ScheduleCategory::Structure => {
            let columns = vec![
                col("Name", Some(Field::Name), 30.0, false),
                col("Member", None, 16.0, false),
                col("Level", None, 28.0, false),
                col("Material", Some(Field::Material), 30.0, false),
                col("Section", None, 22.0, false),
                col("Length", None, 18.0, true),
            ];
            let mut total = 0.0;
            for e in model.iter() {
                let (kind, section, length) = match &e.kind {
                    ElementKind::Column(c) => {
                        ("Column", c.profile.label_in(&units), c.height_in(model))
                    }
                    ElementKind::Beam(b) => (
                        "Beam",
                        format!("{} × {}", mm(b.width), mm(b.depth)),
                        b.length(),
                    ),
                    ElementKind::Footing(f) => match f.kind {
                        creabim_core::FootingKind::Strip => (
                            "Strip footing",
                            format!("{} × {}", mm(f.width), mm(f.thickness)),
                            f.strip_length(model),
                        ),
                        // a pad's length is its longer side
                        creabim_core::FootingKind::Pad => (
                            "Pad footing",
                            format!("{} × {} × {}", mm(f.width), mm(f.length), mm(f.thickness)),
                            f.width.max(f.length),
                        ),
                    },
                    _ => continue,
                };
                total += length;
                rows.push((
                    elevation(e),
                    kind.to_string(),
                    Row {
                        id: e.id,
                        group: false,
                        cells: vec![
                            e.name.clone(),
                            kind.into(),
                            level_of(e).map(name).unwrap_or_default(),
                            material(e),
                            section,
                            mm(length),
                        ],
                    },
                ));
            }
            (
                columns,
                vec![
                    format!("{} members", rows.len()),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    mm(total),
                ],
            )
        }
        // answered above, before the element schedules
        ScheduleCategory::Materials => unreachable!("the take-off is built on its own"),
    };
    // marks sort as text (D01 < D02); walls and slabs by level, then model order
    rows.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut columns = columns;
    let mut totals = totals;
    // walls, slabs and structure get a mark column once one of them has a
    // mark, typed or numbered by the project's rules
    if matches!(
        category,
        ScheduleCategory::Walls | ScheduleCategory::Slabs | ScheduleCategory::Structure
    ) {
        let marks = creabim_core::marks(model);
        if rows.iter().any(|(_, _, r)| marks.contains_key(&r.id)) {
            columns.insert(1, col("Mark", None, 16.0, false));
            for (_, _, r) in &mut rows {
                r.cells
                    .insert(1, marks.get(&r.id).cloned().unwrap_or_default());
            }
            if totals.len() > 1 {
                totals.insert(1, String::new());
            }
        }
    }
    // the type column shows up once anything listed has a type
    let type_of = |id: ElementId| {
        model
            .get(id)
            .and_then(|e| creabim_core::types::type_of(model, e))
            .map(|(t, _)| (t.id, t.name.clone()))
    };
    let typed = rows.iter().any(|(_, _, r)| type_of(r.id).is_some());
    if typed {
        columns.insert(1, col("Type", None, 44.0, false));
        for (_, _, r) in &mut rows {
            let t = type_of(r.id).map_or_else(|| "-".to_string(), |(_, n)| n);
            r.cells.insert(1, t);
        }
        if totals.len() > 1 {
            totals.insert(1, String::new());
        }
    }
    // a phased project says when each thing is built, and when it comes
    // down: "Existing", "Existing, down in Demolition", "Proposed"
    let phases = creabim_core::phases::list(model);
    let phase_of = |id: ElementId| {
        let e = model.get(id).filter(|e| creabim_core::phases::phased(e))?;
        let name = |p: Option<u32>| {
            p.and_then(|p| phases.iter().find(|x| x.id == p))
                .map(|x| x.name.as_str())
        };
        let built = name(e.phase.built).or(phases.first().map(|p| p.name.as_str()))?;
        Some(match name(e.phase.gone) {
            Some(g) => format!("{built}, down in {g}"),
            None => built.to_string(),
        })
    };
    if !phases.is_empty() && rows.iter().any(|(_, _, r)| phase_of(r.id).is_some()) {
        columns.push(col("Phase", None, 40.0, false));
        for (_, _, r) in &mut rows {
            r.cells.push(phase_of(r.id).unwrap_or_default());
        }
        if totals.len() > 1 {
            totals.push(String::new());
        }
    }
    let mut rows: Vec<Row> = rows.into_iter().map(|(_, _, r)| r).collect();
    if by_type {
        // typed groups in name order, untyped last, rows keeping their order
        let key = |r: &Row| match type_of(r.id) {
            Some((_, n)) => (0, n),
            None => (1, String::new()),
        };
        rows.sort_by_key(key);
        let id_of = |r: &Row| type_of(r.id).map(|(id, _)| id);
        let mut grouped = Vec::with_capacity(rows.len() * 2);
        for group in rows.chunk_by(|a, b| id_of(a) == id_of(b)) {
            let t = type_of(group[0].id);
            let mut cells = vec![String::new(); columns.len()];
            cells[0] = match &t {
                Some((_, name)) => format!("{name} ({})", group.len()),
                None => format!("No type ({})", group.len()),
            };
            grouped.push(Row {
                id: t.map_or(ElementId::NIL, |(id, _)| id),
                cells,
                group: true,
            });
            grouped.extend(group.iter().cloned());
        }
        rows = grouped;
    }
    if category == ScheduleCategory::Rooms {
        rows = with_level_totals(model, rows);
    }
    // feet and inches take more room than millimetres
    if units.is_imperial() {
        for c in columns.iter_mut().filter(|c| c.numeric) {
            c.width *= 1.4;
        }
    }
    let listed = listed.unwrap_or_else(|| rows.iter().filter(|r| !r.group).count());
    Schedule {
        category,
        columns,
        rows,
        totals,
        listed,
    }
}

/// The take-off as a schedule: a row for each material with what it adds
/// up to and what it costs at its rate. `by_level` puts each level's
/// materials under a heading with the level's totals. Quantities are in
/// metres, square metres and cubic metres, the units rates are priced in.
fn takeoff_schedule(model: &Model, by_level: bool) -> Schedule {
    use creabim_core::takeoff;
    let columns = vec![
        col("Material", None, 40.0, false),
        col("Elements", None, 16.0, true),
        col("Area", None, 22.0, true),
        col("Volume", None, 22.0, true),
        col("Length", None, 20.0, true),
        col("Rate", Some(Field::Rate), 18.0, true),
        col("Per", Some(Field::RatePer), 16.0, false),
        col("Cost", None, 22.0, true),
    ];
    let name = |id: Option<ElementId>, none: &str| {
        id.and_then(|id| model.get(id))
            .map_or_else(|| none.to_string(), |e| e.name.clone())
    };
    let money = |v: Option<f64>| v.map_or_else(|| "-".to_string(), |v| format!("{v:.2}"));
    let cells = |label: String, t: &takeoff::Total| {
        vec![
            label,
            t.elements.to_string(),
            format!("{:.2} m²", t.quantity.area),
            format!("{:.2} m³", t.quantity.volume),
            format!("{:.2} m", t.quantity.length),
            t.rate
                .map_or_else(|| "-".to_string(), |r| format!("{:.2}", r.amount)),
            t.rate
                .map_or_else(|| "-".to_string(), |r| r.per.label().to_string()),
            money(t.cost),
        ]
    };
    let lines = takeoff::lines(model);
    let materials = takeoff::by_material(model, &lines);
    let mut rows = Vec::new();
    let material_rows = |lines: &[takeoff::Line], rows: &mut Vec<Row>| {
        for t in takeoff::by_material(model, lines) {
            rows.push(Row {
                id: t.key.unwrap_or(ElementId::NIL),
                cells: cells(name(t.key, "No material"), &t),
                group: false,
            });
        }
    };
    if by_level {
        for level in takeoff::by_level(model, &lines) {
            let mine: Vec<takeoff::Line> = lines
                .iter()
                .filter(|l| l.level == level.key)
                .cloned()
                .collect();
            let mut head = cells(name(level.key, "No level"), &level);
            // a level has no rate of its own
            head[5] = String::new();
            head[6] = String::new();
            rows.push(Row {
                id: level.key.unwrap_or(ElementId::NIL),
                cells: head,
                group: true,
            });
            material_rows(&mine, &mut rows);
        }
    } else {
        material_rows(&lines, &mut rows);
    }
    let sum =
        |f: fn(&takeoff::Quantity) -> f64| materials.iter().map(|t| f(&t.quantity)).sum::<f64>();
    let priced = materials.iter().any(|t| t.cost.is_some());
    let totals = vec![
        format!("{} materials", materials.len()),
        String::new(),
        format!("{:.2} m²", sum(|q| q.area)),
        format!("{:.2} m³", sum(|q| q.volume)),
        format!("{:.2} m", sum(|q| q.length)),
        String::new(),
        String::new(),
        if priced {
            money(Some(takeoff::total_cost(model, &lines)))
        } else {
            "-".to_string()
        },
    ];
    Schedule {
        category: ScheduleCategory::Materials,
        columns,
        rows,
        totals,
        listed: materials.len(),
    }
}

/// Rooms under a heading for each level, which gives the level's room
/// count and its rooms' area added up (in the area column).
fn with_level_totals(model: &Model, rows: Vec<Row>) -> Vec<Row> {
    let level = |r: &Row| match model.get(r.id).map(|e| &e.kind) {
        Some(ElementKind::Room(room)) => room.level,
        _ => ElementId::NIL,
    };
    let mut out = Vec::with_capacity(rows.len() + 4);
    for group in rows.chunk_by(|a, b| level(a) == level(b)) {
        let l = level(&group[0]);
        let total: f64 = group
            .iter()
            .filter_map(|r| match model.get(r.id).map(|e| &e.kind) {
                Some(ElementKind::Room(room)) => creabim_core::room::shape(model, r.id)
                    .map(|s| creabim_core::room::net_area(model, room, &s)),
                _ => None,
            })
            .sum();
        let mut cells = vec![String::new(); group[0].cells.len()];
        let level_name = model.get(l).map_or("No level".into(), |e| e.name.clone());
        let n = group.len();
        cells[0] = format!(
            "{level_name} ({n} {})",
            if n == 1 { "room" } else { "rooms" }
        );
        if let Some(c) = cells.get_mut(3) {
            *c = model.units().area(total);
        }
        out.push(Row {
            id: l,
            cells,
            group: true,
        });
        out.extend(group.iter().cloned());
    }
    out
}

/// Is a placed object listed in the furniture schedule, rather than the
/// fixtures one? Those IFC calls furniture are.
pub fn is_furniture(t: &creabim_core::ObjectType) -> bool {
    matches!(
        t.ifc.normalized().class.as_str(),
        "IfcFurniture" | "IfcSystemFurnitureElement"
    )
}

/// How high an object is mounted, as a schedule reads it.
fn mounting(units: &creabim_core::Units, hung: bool, offset: f64) -> String {
    match (hung, offset.abs() < 1e-6) {
        (true, true) => "At the ceiling".into(),
        (true, false) => format!("{} below ceiling", units.dimension(offset)),
        (false, true) => "On the floor".into(),
        (false, false) => format!("{} above floor", units.dimension(offset)),
    }
}

/// The rows of a furniture or fixture schedule: with `by_type` one row an
/// object (the caller groups them by type), else one row for each type on
/// each level with how many there are, under a heading for each category.
/// Returns the columns, the totals and how many objects are listed.
fn object_rows(
    model: &Model,
    category: ScheduleCategory,
    by_type: bool,
    rows: &mut Vec<(f64, String, Row)>,
) -> (Vec<Column>, Vec<String>, usize) {
    use creabim_core::objects::{self, ObjectCategory};
    let units = model.units();
    let marks = creabim_core::marks::object_marks(model);
    let level_name = |l: ElementId| model.get(l).map(|e| e.name.clone()).unwrap_or_default();
    let elevation = |l: ElementId| model.level(l).map_or(0.0, |l| l.elevation);
    let furniture = category == ScheduleCategory::Furniture;
    // (category, type, its element, the object, its element)
    let mut found = vec![];
    for e in model.iter() {
        let ElementKind::Object(o) = &e.kind else {
            continue;
        };
        let Some(r) = &e.of_type else { continue };
        let Some(t) = objects::type_of_id(model, r.id) else {
            continue;
        };
        if is_furniture(t) != furniture {
            continue;
        }
        found.push((t, r.id, o, e));
    }
    let ifc = |t: &creabim_core::ObjectType| {
        let k = t.ifc.normalized();
        format!("{} {}", k.class, k.predefined)
    };
    let mark = |id: ElementId| marks.get(&id).cloned().unwrap_or_default();
    let n = found.len();
    if by_type {
        let columns = vec![
            col("Mark", None, 16.0, false),
            col("Level", None, 28.0, false),
            col("Mounting", None, 34.0, false),
            col("IFC", None, 50.0, false),
        ];
        for (t, tid, o, e) in found {
            // an object with a mark of its own shows it
            let m = creabim_core::marks::manual(e).map_or_else(|| mark(tid), str::to_string);
            rows.push((
                elevation(o.level),
                m.clone(),
                Row {
                    id: e.id,
                    group: false,
                    cells: vec![
                        m,
                        level_name(o.level),
                        mounting(&units, t.mount == objects::Mount::Ceiling, o.offset),
                        ifc(t),
                    ],
                },
            ));
        }
        let totals = vec![
            format!("{n} objects"),
            String::new(),
            String::new(),
            String::new(),
        ];
        return (columns, totals, n);
    }
    let columns = vec![
        col("Mark", None, 16.0, false),
        col("Type", None, 44.0, false),
        col("Level", None, 28.0, false),
        col("Mounting", None, 34.0, false),
        col("IFC", None, 50.0, false),
        col("Count", None, 14.0, true),
    ];
    // one line a type and level: category, mark, level, then the line
    let mut lines: Vec<(usize, String, f64, ElementId, ElementId, Vec<f64>)> = vec![];
    for (t, tid, o, _) in &found {
        let c = ObjectCategory::ALL
            .iter()
            .position(|c| *c == t.category)
            .unwrap_or(0);
        match lines.iter_mut().find(|l| l.3 == *tid && l.4 == o.level) {
            Some(l) => l.5.push(o.offset),
            None => lines.push((
                c,
                mark(*tid),
                elevation(o.level),
                *tid,
                o.level,
                vec![o.offset],
            )),
        }
    }
    lines.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then(a.2.total_cmp(&b.2))
    });
    let mut out: Vec<Row> = vec![];
    for group in lines.chunk_by(|a, b| a.0 == b.0) {
        let count: usize = group.iter().map(|l| l.5.len()).sum();
        let mut cells = vec![String::new(); columns.len()];
        cells[0] = format!("{} ({count})", ObjectCategory::ALL[group[0].0].label());
        out.push(Row {
            id: ElementId::NIL,
            cells,
            group: true,
        });
        for (_, m, _, tid, level, offsets) in group {
            let Some(t) = objects::type_of_id(model, *tid) else {
                continue;
            };
            let same = offsets.iter().all(|v| (v - offsets[0]).abs() < 1e-6);
            let hung = t.mount == objects::Mount::Ceiling;
            out.push(Row {
                id: *tid,
                group: false,
                cells: vec![
                    m.clone(),
                    model.get(*tid).map(|e| e.name.clone()).unwrap_or_default(),
                    level_name(*level),
                    if same {
                        mounting(&units, hung, offsets[0])
                    } else {
                        "Varies".into()
                    },
                    ifc(t),
                    offsets.len().to_string(),
                ],
            });
        }
    }
    // in this order: the shared sort below keeps it
    for (i, r) in out.into_iter().enumerate() {
        rows.push((0.0, format!("{i:06}"), r));
    }
    let what = if furniture { "pieces" } else { "fixtures" };
    let totals = vec![
        format!("{n} {what}"),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        n.to_string(),
    ];
    (columns, totals, n)
}

pub const ROW: f64 = 6.0;
const TITLE_ROW: f64 = 8.0;
const PAD: f64 = 1.5;

/// The schedule as a table drawn in paper millimetres, its top-left corner
/// at the origin. Row text carries the row's element, so clicking a row on a
/// sheet selects that element.
pub fn schedule_drawing(view: ElementId, title: &str, s: &Schedule) -> PlanView {
    schedule_drawing_in(view, title, s, None)
}

/// Cells wrap onto more lines past this width, paper mm (MU131).
pub const WRAP_PAST: f64 = 34.0;
/// One more line in a cell adds this much to its row, paper mm.
const LINE: f64 = 3.4;
const TEXT: f64 = 2.5;
/// Between the blocks of a schedule continued beside itself, paper mm.
const BLOCK_GAP: f64 = 8.0;

/// `t` in lines no wider than `w` paper mm, broken at spaces (a word
/// longer than that stays whole).
fn wrapped(t: &str, w: f64) -> Vec<String> {
    let mut lines: Vec<String> = vec![];
    let mut cur = String::new();
    for word in t.split(' ') {
        let next = if cur.is_empty() {
            word.to_string()
        } else {
            format!("{cur} {word}")
        };
        if !cur.is_empty() && crate::paper::text_width(&next, TEXT) > w {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        } else {
            cur = next;
        }
    }
    lines.push(cur);
    lines
}

/// Each column's width on paper: its header, its longest cell, at most
/// [`WRAP_PAST`] (numbers and group headings never wrap), with its padding.
pub fn column_widths(s: &Schedule) -> Vec<f64> {
    (0..s.columns.len())
        .map(|i| {
            let c = &s.columns[i];
            let head = crate::paper::text_width(c.title, TEXT);
            let longest = s
                .rows
                .iter()
                .filter(|r| !r.group)
                .map(|r| crate::paper::text_width(&r.cells[i], TEXT))
                .chain(s.totals.get(i).map(|t| crate::paper::text_width(t, TEXT)))
                .fold(0.0, f64::max);
            let fit = if c.numeric {
                longest
            } else {
                longest.min(WRAP_PAST)
            };
            fit.max(head) + 2.0 * PAD
        })
        .collect()
}

/// [`schedule_drawing`] fitted to a place `max_height` paper mm tall:
/// each column as wide as its longest text, up to [`WRAP_PAST`] past which
/// its cells wrap; a table taller than its place continues in blocks side
/// by side, its title and header repeated, its totals under the last (MU131:
/// fixed columns let text run into the next and a wide table was cut).
pub fn schedule_drawing_in(
    view: ElementId,
    title: &str,
    s: &Schedule,
    max_height: Option<f64>,
) -> PlanView {
    let n = s.columns.len();
    let widths = column_widths(s);
    let width: f64 = widths.iter().sum();
    // each row's lines per cell, and its height
    let lines: Vec<Vec<Vec<String>>> = s
        .rows
        .iter()
        .map(|r| {
            r.cells
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    if r.group || s.columns[i].numeric {
                        vec![t.clone()]
                    } else {
                        wrapped(t, widths[i] - 2.0 * PAD)
                    }
                })
                .collect()
        })
        .collect();
    let heights: Vec<f64> = lines
        .iter()
        .map(|cells| {
            ROW + (cells.iter().map(|c| c.len()).max().unwrap_or(1).max(1) - 1) as f64 * LINE
        })
        .collect();
    let totals = s.totals.iter().any(|t| !t.is_empty()) || !s.totals.is_empty();
    // rows into blocks that fit the place
    let head = TITLE_ROW + ROW;
    let foot = if totals { ROW } else { 0.0 };
    let mut blocks: Vec<Vec<usize>> = vec![vec![]];
    let mut h = head;
    for (r, rh) in heights.iter().enumerate() {
        let room = max_height.map_or(f64::INFINITY, |m| m - foot);
        if h + rh > room && !blocks.last().unwrap().is_empty() {
            blocks.push(vec![]);
            h = head;
        }
        blocks.last_mut().unwrap().push(r);
        h += rh;
    }
    let mut items = Vec::new();
    let line = |items: &mut Vec<PlanItem>, a: DVec2, b: DVec2, heavy: bool| {
        items.push(PlanItem::Line {
            id: view,
            pen: if heavy { Pen::Cut } else { Pen::Symbol },
            a,
            b,
        })
    };
    let text = |items: &mut Vec<PlanItem>, id, at, t: &str, size, align| {
        items.push(PlanItem::Text {
            angle: 0.0,
            id,
            at,
            text: t.to_string(),
            size,
            align,
            mm: 1.0,
            rgb: None,
            bold: false,
        })
    };
    let count = blocks.len();
    for (bi, rows) in blocks.iter().enumerate() {
        let ox = bi as f64 * (width + BLOCK_GAP);
        let last = bi + 1 == count;
        let name = if bi == 0 {
            title.to_uppercase()
        } else {
            format!("{}, CONTINUED", title.to_uppercase())
        };
        text(
            &mut items,
            view,
            DVec2::new(ox + width / 2.0, -TITLE_ROW / 2.0),
            &name,
            3.5,
            Anchor::Middle,
        );
        // the rows' tops, down from under the header
        let mut tops = vec![-TITLE_ROW - ROW];
        for r in rows {
            let t = *tops.last().unwrap() - heights[*r];
            tops.push(t);
        }
        let body_bottom = *tops.last().unwrap();
        let bottom = if last {
            body_bottom - foot
        } else {
            body_bottom
        };
        let mut x = ox;
        for i in 0..n {
            let c = &s.columns[i];
            let (tx, align) = if c.numeric {
                (x + widths[i] - PAD, Anchor::End)
            } else {
                (x + PAD, Anchor::Start)
            };
            text(
                &mut items,
                view,
                DVec2::new(tx, -TITLE_ROW - ROW / 2.0),
                c.title,
                TEXT,
                align,
            );
            for (k, r) in rows.iter().enumerate() {
                let top = tops[k];
                for (li, t) in lines[*r][i].iter().enumerate() {
                    let y = top - ROW / 2.0 - li as f64 * LINE;
                    text(&mut items, s.rows[*r].id, DVec2::new(tx, y), t, TEXT, align);
                }
            }
            if last {
                if let Some(t) = s.totals.get(i).filter(|t| !t.is_empty()) {
                    text(
                        &mut items,
                        view,
                        DVec2::new(tx, bottom + ROW / 2.0),
                        t,
                        TEXT,
                        align,
                    );
                }
            }
            if i > 0 {
                // a group heading runs on across the empty cells after it,
                // so the column lines break there
                let spans = |row: &Row| row.group && row.cells[1..=i].iter().all(|c| c.is_empty());
                let mut top = -TITLE_ROW;
                for (k, r) in rows.iter().enumerate() {
                    if spans(&s.rows[*r]) {
                        let y = tops[k];
                        if y < top {
                            line(&mut items, DVec2::new(x, top), DVec2::new(x, y), false);
                        }
                        top = tops[k + 1];
                    }
                }
                line(&mut items, DVec2::new(x, top), DVec2::new(x, bottom), false);
            }
            x += widths[i];
        }
        // the title's and the header's lines heavy, then each row's
        line(
            &mut items,
            DVec2::new(ox, -TITLE_ROW),
            DVec2::new(ox + width, -TITLE_ROW),
            true,
        );
        for (k, y) in tops.iter().enumerate() {
            let heavy = k == 0 || (last && totals && k + 1 == tops.len());
            line(
                &mut items,
                DVec2::new(ox, *y),
                DVec2::new(ox + width, *y),
                heavy,
            );
        }
        if last && totals {
            line(
                &mut items,
                DVec2::new(ox, bottom),
                DVec2::new(ox + width, bottom),
                true,
            );
        }
        // outer frame
        for (a, b) in [
            (DVec2::new(ox, 0.0), DVec2::new(ox + width, 0.0)),
            (DVec2::new(ox, 0.0), DVec2::new(ox, bottom)),
            (DVec2::new(ox + width, 0.0), DVec2::new(ox + width, bottom)),
        ] {
            line(&mut items, a, b, true);
        }
    }
    PlanView { items }
}

/// The schedule as CSV, header first, totals last.
pub fn to_csv(s: &Schedule) -> String {
    let quote = |v: &str| {
        if v.contains([',', '"', '\n']) {
            format!("\"{}\"", v.replace('"', "\"\""))
        } else {
            v.to_string()
        }
    };
    let line = |cells: Vec<&str>| cells.into_iter().map(quote).collect::<Vec<_>>().join(",");
    let mut out = line(s.columns.iter().map(|c| c.title).collect()) + "\n";
    for r in &s.rows {
        out += &(line(r.cells.iter().map(String::as_str).collect()) + "\n");
    }
    let mut totals: Vec<&str> = s.totals.iter().map(String::as_str).collect();
    totals.resize(s.columns.len(), "");
    out += &(line(totals) + "\n");
    out
}

#[cfg(test)]
mod tests {
    /// Part 9: a window schedule says which ribbon or corner a window is
    /// in, once one is; before, it keeps its old columns.
    #[test]
    fn window_schedules_name_the_arrangement_once_there_is_one() {
        use creabim_core::{arrangement, ElementKind, OpeningKind};
        let mut m = creabim_core::sample::sample_house();
        let titles = |m: &creabim_core::Model| -> Vec<&'static str> {
            schedule(m, ScheduleCategory::Windows)
                .columns
                .iter()
                .map(|c| c.title)
                .collect()
        };
        assert!(!titles(&m).contains(&"Arrangement"));
        let tx = m
            .iter()
            .filter(|e| matches!(&e.kind, ElementKind::Opening(o) if o.kind == OpeningKind::Window && o.roof_at.is_none()))
            .find_map(|e| arrangement::add_beside(&m, e.id).ok())
            .unwrap();
        tx.apply(&mut m).unwrap();
        let s = schedule(&m, ScheduleCategory::Windows);
        let at = s
            .columns
            .iter()
            .position(|c| c.title == "Arrangement")
            .unwrap();
        let named: Vec<&str> = s
            .rows
            .iter()
            .map(|r| r.cells[at].as_str())
            .filter(|t| !t.is_empty())
            .collect();
        assert_eq!(named, ["Ribbon 1", "Ribbon 1"]);
        // the door schedule has none
        let d = schedule(&m, ScheduleCategory::Doors);
        assert!(!d.columns.iter().any(|c| c.title == "Arrangement"));
    }

    use super::*;
    use creabim_core::sample::sample_house;

    #[test]
    fn door_schedule_lists_marks_in_order_and_follows_edits() {
        let mut doc = creabim_core::Document::from_model(sample_house());
        let doors = schedule(doc.model(), ScheduleCategory::Doors);
        let marks: Vec<&str> = doors.rows.iter().map(|r| r.cells[0].as_str()).collect();
        assert_eq!(marks, ["D01", "D02"]);
        assert_eq!(doors.totals[0], "2 doors");
        let front = doors.rows[0].id;
        assert_eq!(doors.rows[0].cells[3], "1000");
        doc.edit(front, "Widen", |e| set_value(e, Field::Width, 1.2))
            .unwrap();
        let doors = schedule(doc.model(), ScheduleCategory::Doors);
        assert_eq!(doors.rows[0].cells[3], "1200");
    }

    #[test]
    fn door_schedule_says_how_each_door_opens() {
        let mut doc = creabim_core::Document::from_model(sample_house());
        let doors = schedule(doc.model(), ScheduleCategory::Doors);
        let at = doors
            .columns
            .iter()
            .position(|c| c.title == "Operation")
            .unwrap();
        assert_eq!(doors.rows[0].cells[at], "Single hinged");
        let front = doors.rows[0].id;
        doc.edit(front, "Pair", |e| {
            if let ElementKind::Opening(o) = &mut e.kind {
                o.width = 1.6;
                o.frame
                    .set_operation(o.kind, creabim_core::Operation::DoubleSwing);
                o.frame.set_active_share(2.0 / 3.0);
            }
        })
        .unwrap();
        let doors = schedule(doc.model(), ScheduleCategory::Doors);
        // the active leaf first, both in millimetres
        let text = &doors.rows[0].cells[at];
        assert!(text.starts_with("Double hinged, 9"), "{text}");
        let widths: Vec<f64> = text["Double hinged, ".len()..]
            .split(" + ")
            .map(|w| w.parse().unwrap())
            .collect();
        assert!(
            widths[0] > widths[1] * 1.9 && widths[0] < widths[1] * 2.1,
            "{text}"
        );
    }

    #[test]
    fn wall_schedule_totals_areas_and_exports_csv() {
        let m = sample_house();
        let walls = schedule(&m, ScheduleCategory::Walls);
        assert_eq!(walls.rows.len(), 9);
        let csv = to_csv(&walls);
        assert!(csv.starts_with("Name,Level,Material,Length"));
        assert!(csv.contains("GF Wall 1,Ground Floor,Brick,10000,300,3000"));
        assert!(csv.trim_end().ends_with("m²"));
    }

    #[test]
    fn layered_walls_list_their_assembly() {
        let m = creabim_core::sample::layered_house();
        let walls = schedule(&m, ScheduleCategory::Walls);
        assert_eq!(walls.columns[3].title, "Assembly");
        let row = walls
            .rows
            .iter()
            .find(|r| r.cells[0] == "GF Wall 1")
            .unwrap();
        assert_eq!(row.cells[2], "Block, concrete", "listed by its core");
        assert_eq!(row.cells[3], "Exterior wall 365, brick cavity");
        assert_eq!(row.cells[5], "365");
        assert_eq!(walls.totals.len(), walls.columns.len());
    }

    #[test]
    fn an_imperial_project_schedules_in_feet_and_inches() {
        let mut doc = creabim_core::Document::from_model(sample_house());
        let tx = creabim_core::units::set_units(doc.model(), creabim_core::Units::imperial());
        doc.commit(tx).unwrap();
        let walls = schedule(doc.model(), ScheduleCategory::Walls);
        // 10 m long, 300 thick, 3 m high
        assert_eq!(
            walls.rows[0].cells[..6],
            [
                "GF Wall 1",
                "Ground Floor",
                "Brick",
                "32'-9 3/4\"",
                "0'-11 3/4\"",
                "9'-10 1/8\""
            ]
        );
        assert!(walls.totals.last().unwrap().ends_with("ft²"));
        // inch marks are quoted in the CSV
        assert!(to_csv(&walls).contains(",\"32'-9 3/4\"\"\","));
    }

    #[test]
    fn typed_elements_list_and_group_by_their_type() {
        let m = creabim_core::sample::typed_house();
        let windows = schedule(&m, ScheduleCategory::Windows);
        assert_eq!(windows.columns[1].title, "Type");
        assert!(windows
            .rows
            .iter()
            .all(|r| r.cells[1] == "Window 1200 × 1300, two panes"));
        let walls = schedule_by(&m, ScheduleCategory::Walls, true);
        assert_eq!(walls.totals.len(), walls.columns.len());
        let groups: Vec<&str> = walls
            .rows
            .iter()
            .filter(|r| r.group)
            .map(|r| r.cells[0].as_str())
            .collect();
        assert_eq!(
            groups,
            [
                "Exterior wall 365, brick cavity (7)",
                "Interior wall 126, block (1)",
                "Storefront, 4 bays (1)"
            ]
        );
        // each group heading names its type, the rows under it its instances
        let first = &walls.rows[0];
        assert!(matches!(
            m.get(first.id).map(|e| &e.kind),
            Some(ElementKind::ElementType(_))
        ));
        assert_eq!(walls.rows.len(), 9 + 3);
        // an untyped project keeps its old table
        let plain = schedule(&sample_house(), ScheduleCategory::Walls);
        assert!(plain.columns.iter().all(|c| c.title != "Type"));
    }

    #[test]
    fn furniture_and_fixtures_are_counted_by_type_and_level() {
        let m = creabim_core::sample::furnished_house();
        let s = schedule(&m, ScheduleCategory::Fixtures);
        let titles: Vec<&str> = s.columns.iter().map(|c| c.title).collect();
        assert_eq!(
            titles,
            ["Mark", "Type", "Level", "Mounting", "IFC", "Count"]
        );
        // a heading for each category, in catalogue order
        let headings: Vec<&str> = s
            .rows
            .iter()
            .filter(|r| r.group)
            .map(|r| r.cells[0].split(" (").next().unwrap())
            .collect();
        assert_eq!(
            headings,
            [
                "Bathroom",
                "Kitchen",
                "Electrical",
                "Lighting",
                "Plumbing",
                "Heating and ventilation"
            ]
        );
        let line = |ty: &str, level: &str| {
            s.rows
                .iter()
                .find(|r| !r.group && r.cells[1] == ty && r.cells[2] == level)
                .unwrap_or_else(|| panic!("{ty} on {level}"))
        };
        let sockets = line("Double socket", "Ground Floor");
        assert_eq!(sockets.cells[5], "5");
        assert!(sockets.cells[0].starts_with('E'), "{:?}", sockets.cells);
        assert_eq!(sockets.cells[3], "300 above floor");
        assert_eq!(sockets.cells[4], "IfcOutlet POWEROUTLET");
        assert_eq!(line("Double socket", "First Floor").cells[5], "3");
        let lights = line("Recessed downlight", "Ground Floor");
        assert_eq!(
            (lights.cells[3].as_str(), lights.cells[5].as_str()),
            ("At the ceiling", "3")
        );
        assert!(lights.cells[0].starts_with('L'));
        // the counts add up to every fixture, and furniture is apart
        let counted: usize = s
            .rows
            .iter()
            .filter(|r| !r.group)
            .map(|r| r.cells[5].parse::<usize>().unwrap())
            .sum();
        assert_eq!(counted, s.listed);
        assert_eq!(s.totals[5], s.listed.to_string());
        let f = schedule(&m, ScheduleCategory::Furniture);
        let objects = m
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::Object(_)))
            .count();
        assert_eq!(f.listed + s.listed, objects);
        assert!(f
            .rows
            .iter()
            .all(|r| r.group || r.cells[4].starts_with("IfcFurniture")));
        // grouped by type instead: one row an object, under its type
        let by = schedule_by(&m, ScheduleCategory::Fixtures, true);
        assert_eq!(by.rows.iter().filter(|r| !r.group).count(), s.listed);
        assert!(by
            .rows
            .iter()
            .any(|r| r.group && r.cells[0] == "Double socket (8)"));
        assert_eq!(by.totals.len(), by.columns.len());
        assert!(to_csv(&s).starts_with("Mark,Type,Level,Mounting,IFC,Count"));
    }

    #[test]
    fn a_group_heading_runs_on_across_the_empty_cells_after_it() {
        let m = creabim_core::starter::starter_house();
        let s = schedule(&m, ScheduleCategory::Rooms);
        let r = s.rows.iter().position(|r| r.group).unwrap();
        let d = schedule_drawing(ElementId::NIL, "Room Schedule", &s);
        let w = column_widths(&s);
        let x1 = w[0];
        let mid = -TITLE_ROW - (r as f64 + 1.5) * ROW;
        let crosses = |x: f64| {
            d.items.iter().any(|it| {
                matches!(it, PlanItem::Line { a, b, .. }
                if (a.x - x).abs() < 1e-9 && (b.x - x).abs() < 1e-9
                    && a.y.max(b.y) > mid && a.y.min(b.y) < mid)
            })
        };
        assert!(
            !crosses(x1),
            "the level heading is cut by the first column line"
        );
        // the area column's line still runs through it
        let area = s.columns.iter().position(|c| c.title == "Area").unwrap();
        let xa: f64 = w[..area].iter().sum();
        assert!(crosses(xa));
    }

    #[test]
    fn schedule_drawing_is_a_table() {
        let m = sample_house();
        let s = schedule(&m, ScheduleCategory::Windows);
        let d = schedule_drawing(ElementId::NIL, "Window Schedule", &s);
        let (lo, hi) = d.bounds().unwrap();
        let width: f64 = column_widths(&s).iter().sum();
        assert!((hi.x - lo.x - width).abs() < 1e-9);
        assert!((lo.y + TITLE_ROW + (s.rows.len() + 2) as f64 * ROW).abs() < 1e-9);
    }

    #[test]
    fn a_schedule_fits_its_text_and_continues_beside_itself() {
        let col = |title: &'static str, numeric| Column {
            title,
            field: None,
            width: 20.0,
            numeric,
        };
        let long = "Exterior wall R-31, cedar rainscreen over strapping";
        let s = Schedule {
            category: creabim_core::ScheduleCategory::Walls,
            listed: 30,
            columns: vec![
                col("Name", false),
                col("Assembly", false),
                col("Length", true),
            ],
            rows: (0..30)
                .map(|i| Row {
                    id: ElementId::NIL,
                    cells: vec![format!("Wall {i}"), long.into(), "36'-0\"".into()],
                    group: false,
                })
                .collect(),
            totals: vec!["30 walls".into(), String::new(), String::new()],
        };
        let texts = |d: &PlanView| -> Vec<(DVec2, String)> {
            d.items
                .iter()
                .filter_map(|i| match i {
                    PlanItem::Text { at, text, .. } => Some((*at, text.clone())),
                    _ => None,
                })
                .collect()
        };
        let one = schedule_drawing(ElementId::NIL, "Wall schedule", &s);
        // the long assembly wraps: no line of it is wider than the limit
        for (_, t) in texts(&one) {
            if long.contains(t.as_str()) && t.contains(' ') {
                assert!(
                    crate::paper::text_width(&t, TEXT) <= WRAP_PAST + 1e-6,
                    "{t}"
                );
            }
        }
        assert!(!texts(&one).iter().any(|(_, t)| t == long), "it wrapped");
        // in a 120 mm place it continues beside itself, header repeated,
        // totals once, under the last block
        let split = schedule_drawing_in(ElementId::NIL, "Wall schedule", &s, Some(120.0));
        let t = texts(&split);
        assert!(t.iter().any(|(_, x)| x == "WALL SCHEDULE, CONTINUED"));
        assert!(t.iter().filter(|(_, x)| x == "Assembly").count() >= 2);
        assert_eq!(t.iter().filter(|(_, x)| x == "30 walls").count(), 1);
        let lowest = split
            .items
            .iter()
            .filter_map(|i| i.bounds())
            .map(|(lo, _)| lo.y)
            .fold(0.0, f64::min);
        assert!(lowest >= -120.0 - 1e-6, "{lowest}");
    }
}
